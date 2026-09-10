//! Remote module execution over a real SSH connection.
//!
//! The modules on the executor's remote allowlist are the ones that route all
//! of their work through `ModuleContext.connection`. This suite proves that on
//! a real target: it builds a throwaway sshd container, runs a playbook twice,
//! and checks both the effect inside the container and that the second run
//! reports no change.
//!
//! # Running
//!
//! ```bash
//! RUSTIBLE_TEST_SSH_DOCKER=1 cargo test --test remote_modules_ssh_tests -- --test-threads=1
//! ```
//!
//! Without `RUSTIBLE_TEST_SSH_DOCKER=1` every test reports success and does
//! nothing, so the default suite stays free of Docker and network
//! dependencies.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use tempfile::{tempdir, TempDir};

/// Image and container name for the throwaway target.
const IMAGE: &str = "rustible-remote-modules-test:local";
const CONTAINER: &str = "rustible-remote-modules-test";
/// Loopback port the container's sshd is published on.
const SSH_PORT: u16 = 2223;

fn enabled() -> bool {
    std::env::var("RUSTIBLE_TEST_SSH_DOCKER").as_deref() == Ok("1")
}

fn run(program: &str, args: &[&str]) -> (bool, String, String) {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {}", program, e));
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn rustible_binary() -> PathBuf {
    // The integration test binary lives next to the CLI binary.
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("rustible")
}

/// A disposable sshd container with a key-based root login.
struct SshTarget {
    dir: TempDir,
}

impl SshTarget {
    fn start() -> Self {
        let dir = tempdir().expect("temp dir");
        let key = dir.path().join("id_test");

        let (ok, _, stderr) = run(
            "ssh-keygen",
            &["-t", "ed25519", "-N", "", "-q", "-f", key.to_str().unwrap()],
        );
        assert!(ok, "ssh-keygen failed: {}", stderr);

        fs::write(
            dir.path().join("Dockerfile"),
            r#"FROM debian:13-slim
RUN apt-get update && apt-get install -y --no-install-recommends openssh-server cron tzdata \
    && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /root/.ssh /run/sshd \
    && sed -i 's/^#\?PermitRootLogin.*/PermitRootLogin prohibit-password/' /etc/ssh/sshd_config
COPY id_test.pub /root/.ssh/authorized_keys
RUN chmod 700 /root/.ssh && chmod 600 /root/.ssh/authorized_keys
CMD ["/usr/sbin/sshd", "-D", "-e"]
"#,
        )
        .expect("write Dockerfile");

        let (ok, _, stderr) = run(
            "docker",
            &["build", "-q", "-t", IMAGE, dir.path().to_str().unwrap()],
        );
        assert!(ok, "docker build failed: {}", stderr);

        // A leftover container from an interrupted run would hold the port.
        let _ = run("docker", &["rm", "-f", CONTAINER]);

        let publish = format!("127.0.0.1:{}:22", SSH_PORT);
        let (ok, _, stderr) = run(
            "docker",
            &["run", "-d", "--name", CONTAINER, "-p", &publish, IMAGE],
        );
        assert!(ok, "docker run failed: {}", stderr);

        let target = Self { dir };
        target.wait_for_sshd();
        target
    }

    fn wait_for_sshd(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            let (ok, stdout, _) = run("docker", &["exec", CONTAINER, "sh", "-c", "echo ready"]);
            if ok && stdout.trim() == "ready" {
                // sshd needs a moment more to bind after the container starts.
                std::thread::sleep(Duration::from_millis(500));
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        panic!("the ssh container did not become ready");
    }

    fn key_path(&self) -> PathBuf {
        self.dir.path().join("id_test")
    }

    fn write_inventory(&self) -> PathBuf {
        let path = self.dir.path().join("inventory.yml");
        fs::write(
            &path,
            format!(
                r#"---
all:
  hosts:
    container1:
      ansible_host: 127.0.0.1
      ansible_port: {}
      ansible_user: root
      ansible_ssh_private_key_file: {}
"#,
                SSH_PORT,
                self.key_path().display()
            ),
        )
        .expect("write inventory");
        path
    }

    fn write_playbook(&self, body: &str) -> PathBuf {
        let path = self.dir.path().join("site.yml");
        fs::write(&path, body).expect("write playbook");
        path
    }

    /// Run a command inside the container and return its stdout.
    fn exec(&self, command: &str) -> String {
        let (_, stdout, _) = run("docker", &["exec", CONTAINER, "sh", "-c", command]);
        stdout
    }
}

impl Drop for SshTarget {
    fn drop(&mut self) {
        let _ = run("docker", &["rm", "-f", CONTAINER]);
        let _ = run("docker", &["rmi", "-f", IMAGE]);
    }
}

/// Run a playbook and return the CLI's combined output.
fn run_playbook(inventory: &Path, playbook: &Path) -> String {
    let output = Command::new(rustible_binary())
        .arg("run")
        .arg("-i")
        .arg(inventory)
        .arg(playbook)
        .output()
        .expect("failed to run rustible");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn remote_modules_apply_and_are_idempotent() {
    if !enabled() {
        eprintln!("skipping: set RUSTIBLE_TEST_SSH_DOCKER=1 to run");
        return;
    }

    let target = SshTarget::start();
    let inventory = target.write_inventory();
    let playbook = target.write_playbook(
        r#"---
- name: Remote module coverage
  hosts: all
  gather_facts: true
  tasks:
    - name: Create a group
      group:
        name: deployers
        state: present

    - name: Create a user
      user:
        name: deployer
        group: deployers
        shell: /bin/bash
        state: present

    - name: Add a cron job
      cron:
        name: rustible-heartbeat
        job: /bin/true
        minute: "5"
        state: present

    - name: Set the timezone
      timezone:
        name: Europe/Lisbon

    - name: Run a command
      command: id deployer
"#,
    );

    let first = run_playbook(&inventory, &playbook);
    assert!(
        first.contains("failed=0") && first.contains("unreachable=0"),
        "first run should succeed:\n{}",
        first
    );

    // The effects must exist on the target, not on the control node.
    assert!(
        target.exec("getent group deployers").contains("deployers"),
        "the group should exist in the container"
    );
    assert!(
        target.exec("getent passwd deployer").contains("deployer"),
        "the user should exist in the container"
    );
    assert!(
        target.exec("crontab -l").contains("rustible-heartbeat"),
        "the cron job should exist in the container"
    );
    // Debian slim images carry no /etc/timezone, so the symlink is the
    // authoritative signal.
    assert!(
        target
            .exec("readlink /etc/localtime")
            .contains("Europe/Lisbon"),
        "the timezone should be set in the container"
    );
    assert!(
        !Path::new("/home/deployer").exists(),
        "nothing should have been created on the control node"
    );

    let second = run_playbook(&inventory, &playbook);
    assert!(
        second.contains("changed=1"),
        "only the command task should report a change on a repeat run:\n{}",
        second
    );
    assert!(
        second.contains("failed=0") && second.contains("unreachable=0"),
        "second run should succeed:\n{}",
        second
    );
}

#[test]
fn modules_without_a_verified_remote_transport_are_refused() {
    if !enabled() {
        eprintln!("skipping: set RUSTIBLE_TEST_SSH_DOCKER=1 to run");
        return;
    }

    let target = SshTarget::start();
    let inventory = target.write_inventory();
    let playbook = target.write_playbook(
        r#"---
- name: Unverified module
  hosts: all
  gather_facts: false
  tasks:
    - name: Touch a remote file
      file:
        path: /tmp/rustible-should-not-exist
        state: touch
"#,
    );

    let output = run_playbook(&inventory, &playbook);
    assert!(
        output.contains("does not have a verified remote transport"),
        "a module that operates on the local filesystem must be refused, not \
         silently run on the control node:\n{}",
        output
    );
    assert!(
        target
            .exec("test -e /tmp/rustible-should-not-exist && echo present")
            .trim()
            .is_empty(),
        "the refused task must not have touched the target"
    );
}
