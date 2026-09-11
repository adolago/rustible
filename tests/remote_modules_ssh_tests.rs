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

fn binary_dir() -> PathBuf {
    // The integration test binary lives next to the CLI binaries.
    let mut path = std::env::current_exe().expect("test binary path");
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path
}

fn rustible_binary() -> PathBuf {
    binary_dir().join("rustible")
}

fn agent_binary() -> PathBuf {
    binary_dir().join("rustible-agent")
}

/// Run a rustible subcommand and return its combined output.
fn run_cli(args: &[&str]) -> (bool, String) {
    let output = Command::new(rustible_binary())
        .args(args)
        .output()
        .expect("failed to run rustible");
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
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
RUN apt-get update && apt-get install -y --no-install-recommends openssh-server cron tzdata git sudo \
    && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /root/.ssh /run/sshd \
    && sed -i 's/^#\?PermitRootLogin.*/PermitRootLogin prohibit-password/' /etc/ssh/sshd_config \
    && useradd -m -s /bin/bash runner \
    && mkdir -p /home/runner/.ssh \
    && echo 'runner ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/runner \
    && chmod 440 /etc/sudoers.d/runner
COPY id_test.pub /root/.ssh/authorized_keys
COPY id_test.pub /home/runner/.ssh/authorized_keys
RUN chmod 700 /root/.ssh && chmod 600 /root/.ssh/authorized_keys \
    && chown -R runner:runner /home/runner/.ssh \
    && chmod 700 /home/runner/.ssh && chmod 600 /home/runner/.ssh/authorized_keys
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
        // Each container generates its own host key, so the run gets a
        // throwaway known_hosts file instead of touching the user's.
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
      ansible_ssh_known_hosts_file: {}
"#,
                SSH_PORT,
                self.key_path().display(),
                self.dir.path().join("known_hosts").display()
            ),
        )
        .expect("write inventory");
        path
    }

    /// Inventory that logs in as an unprivileged user with passwordless sudo.
    fn write_runner_inventory(&self) -> PathBuf {
        let path = self.dir.path().join("runner-inventory.yml");
        fs::write(
            &path,
            format!(
                r#"---
all:
  hosts:
    container1:
      ansible_host: 127.0.0.1
      ansible_port: {}
      ansible_user: runner
      ansible_ssh_private_key_file: {}
      ansible_ssh_known_hosts_file: {}
"#,
                SSH_PORT,
                self.key_path().display(),
                self.dir.path().join("known_hosts").display()
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

    - name: Create a directory
      file:
        path: /opt/rustible-test/dir
        state: directory
        mode: "0750"

    - name: Link the directory
      file:
        path: /opt/rustible-test/link
        src: /opt/rustible-test/dir
        state: link

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
        target
            .exec("stat -c '%a' /opt/rustible-test/dir")
            .trim()
            .contains("750"),
        "the directory mode should be applied on the target"
    );
    assert!(
        target
            .exec("readlink /opt/rustible-test/link")
            .contains("/opt/rustible-test/dir"),
        "the symlink should point at the directory"
    );
    assert!(
        !Path::new("/home/deployer").exists() && !Path::new("/opt/rustible-test").exists(),
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
    // archive still writes through std::fs, so it must be refused rather than
    // creating an archive on the control node.
    let playbook = target.write_playbook(
        r#"---
- name: Unverified module
  hosts: all
  gather_facts: false
  tasks:
    - name: Archive a remote directory
      archive:
        path: /etc
        dest: /tmp/rustible-should-not-exist
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

#[test]
fn file_editing_modules_act_on_the_target() {
    if !enabled() {
        eprintln!("skipping: set RUSTIBLE_TEST_SSH_DOCKER=1 to run");
        return;
    }

    let target = SshTarget::start();
    let inventory = target.write_inventory();

    let script = target.dir.path().join("hello.sh");
    fs::write(
        &script,
        "#!/bin/sh\ntouch /tmp/rustible-script-marker\necho ran\n",
    )
    .expect("write script");

    let playbook = target.write_playbook(&format!(
        r#"---
- name: File editing modules
  hosts: all
  gather_facts: false
  tasks:
    - name: Stat a known file
      stat:
        path: /etc/os-release
      register: os_release

    - name: The stat must come from the target
      assert:
        that:
          - os_release.stat.exists

    - name: Ensure a configuration line
      lineinfile:
        path: /etc/rustible-test.conf
        line: "managed=true"
        create: true

    - name: Authorize a key
      authorized_key:
        user: root
        key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA rustible-test"
        state: present

    - name: Ensure a managed block
      blockinfile:
        path: /etc/rustible-block.conf
        block: |
          setting_one = 1
          setting_two = 2
        create: true

    - name: Run a transferred script
      script: {}
"#,
        script.display()
    ));

    let first = run_playbook(&inventory, &playbook);
    assert!(
        first.contains("failed=0") && first.contains("unreachable=0"),
        "first run should succeed:\n{}",
        first
    );

    assert!(
        target
            .exec("cat /etc/rustible-test.conf")
            .contains("managed=true"),
        "lineinfile should have written to the target"
    );
    assert!(
        target
            .exec("grep -c rustible-test /root/.ssh/authorized_keys")
            .trim()
            == "1",
        "the key should be authorized exactly once on the target"
    );
    assert!(
        target
            .exec("test -e /tmp/rustible-script-marker && echo present")
            .contains("present"),
        "the script should have run on the target"
    );
    assert!(
        target
            .exec("cat /etc/rustible-block.conf")
            .contains("setting_two = 2"),
        "blockinfile should have written the managed block on the target"
    );
    assert!(
        !Path::new("/etc/rustible-test.conf").exists(),
        "nothing should have been written on the control node"
    );

    // Only the script reports a change on a repeat run; scripts carry no
    // idempotency contract, exactly as in Ansible.
    let second = run_playbook(&inventory, &playbook);
    assert!(
        second.contains("changed=1") && second.contains("failed=0"),
        "second run should be idempotent apart from the script:\n{}",
        second
    );
}

#[test]
fn git_clones_on_the_target() {
    if !enabled() {
        eprintln!("skipping: set RUSTIBLE_TEST_SSH_DOCKER=1 to run");
        return;
    }

    let target = SshTarget::start();
    let inventory = target.write_inventory();

    // Seed a bare repository inside the container so the clone needs no
    // network access.
    target.exec(
        "git init -q --bare /srv/origin/demo.git \
         && git clone -q /srv/origin/demo.git /tmp/seed \
         && cd /tmp/seed \
         && git config user.email t@example.com && git config user.name test \
         && echo hello > README && git add README && git commit -qm init \
         && git push -q origin HEAD:main",
    );

    let playbook = target.write_playbook(
        r#"---
- name: Clone a repository
  hosts: all
  gather_facts: false
  tasks:
    - name: Clone
      git:
        repo: /srv/origin/demo.git
        dest: /opt/demo
        version: main
"#,
    );

    let first = run_playbook(&inventory, &playbook);
    assert!(
        first.contains("changed=1") && first.contains("failed=0"),
        "the clone should happen on the target:\n{}",
        first
    );
    assert!(
        target.exec("cat /opt/demo/README").contains("hello"),
        "the working tree should exist inside the container"
    );
    assert!(
        !Path::new("/opt/demo").exists(),
        "nothing should have been cloned on the control node"
    );

    let second = run_playbook(&inventory, &playbook);
    assert!(
        second.contains("changed=0"),
        "a second clone of the same version should report no change:\n{}",
        second
    );
}

#[test]
fn become_escalates_on_the_target() {
    if !enabled() {
        eprintln!("skipping: set RUSTIBLE_TEST_SSH_DOCKER=1 to run");
        return;
    }

    let target = SshTarget::start();
    let inventory = target.write_runner_inventory();
    let playbook = target.write_playbook(
        r#"---
- name: Escalated tasks
  hosts: all
  gather_facts: true
  become: true
  tasks:
    - name: Report the effective user
      command: id -un
      register: whoami

    - name: Escalation must reach root
      assert:
        that:
          - whoami.stdout | trim == "root"

    - name: Create a root-owned directory
      file:
        path: /opt/become-test
        state: directory
        mode: "0700"
        owner: root

    - name: Create a group as root
      group:
        name: escalated
        state: present
"#,
    );

    let output = run_playbook(&inventory, &playbook);
    assert!(
        output.contains("failed=0") && output.contains("unreachable=0"),
        "the escalated play should succeed:\n{}",
        output
    );
    assert!(
        target
            .exec("stat -c '%U %a' /opt/become-test")
            .trim()
            .starts_with("root 700"),
        "the directory should be owned by root on the target"
    );
    assert!(
        target.exec("getent group escalated").contains("escalated"),
        "the group should exist on the target"
    );
}

#[test]
fn become_is_refused_for_modules_that_cannot_escalate() {
    if !enabled() {
        eprintln!("skipping: set RUSTIBLE_TEST_SSH_DOCKER=1 to run");
        return;
    }

    let target = SshTarget::start();
    let inventory = target.write_runner_inventory();
    // copy writes over SFTP, which has no way to escalate, so asking for
    // become must fail rather than write as the login user.
    let playbook = target.write_playbook(
        r#"---
- name: Escalated copy
  hosts: all
  gather_facts: false
  become: true
  tasks:
    - name: Write a root-owned file
      copy:
        content: "escalated\n"
        dest: /opt/become-copy.txt
"#,
    );

    let output = run_playbook(&inventory, &playbook);
    assert!(
        output.contains("cannot pass privilege escalation"),
        "an escalated copy must be refused:\n{}",
        output
    );
    assert!(
        target
            .exec("test -e /opt/become-copy.txt && echo present")
            .trim()
            .is_empty(),
        "the refused task must not have written to the target"
    );
}

#[test]
fn agent_deploys_and_reports_status() {
    if !enabled() {
        eprintln!("skipping: set RUSTIBLE_TEST_SSH_DOCKER=1 to run");
        return;
    }
    let agent = agent_binary();
    assert!(
        agent.is_file(),
        "the rustible-agent binary should be built alongside the CLI"
    );

    let target = SshTarget::start();
    let inventory = target.write_inventory();

    let (ok, output) = run_cli(&[
        "agent",
        "deploy",
        "-i",
        inventory.to_str().unwrap(),
        "--binary",
        agent.to_str().unwrap(),
    ]);
    assert!(ok, "agent deploy should succeed:\n{}", output);
    assert!(
        target
            .exec("test -x /usr/local/bin/rustible-agent && echo present")
            .contains("present"),
        "the agent binary should be executable on the target"
    );

    let (ok, output) = run_cli(&["agent", "status", "-i", inventory.to_str().unwrap()]);
    assert!(ok, "agent status should succeed:\n{}", output);
    assert!(
        output.contains("version"),
        "status should report the agent version:\n{}",
        output
    );

    // Stopping with no listening agent is a no-op, not a failure.
    let (ok, output) = run_cli(&["agent", "stop", "-i", inventory.to_str().unwrap()]);
    assert!(ok, "agent stop should succeed:\n{}", output);
}

#[test]
fn agent_mode_runs_tasks_through_the_agent() {
    if !enabled() {
        eprintln!("skipping: set RUSTIBLE_TEST_SSH_DOCKER=1 to run");
        return;
    }

    let target = SshTarget::start();
    let inventory = target.write_inventory();

    let (ok, output) = run_cli(&[
        "agent",
        "deploy",
        "-i",
        inventory.to_str().unwrap(),
        "--binary",
        agent_binary().to_str().unwrap(),
    ]);
    assert!(ok, "agent deploy should succeed:\n{}", output);

    let playbook = target.write_playbook(
        r#"---
- name: Agent mode
  hosts: all
  gather_facts: true
  tasks:
    - name: Run a command through the agent
      command: hostname

    - name: Create a marker
      file:
        path: /tmp/agent-mode-marker
        state: touch
"#,
    );

    let (ok, output) = run_cli(&[
        "run",
        "-i",
        inventory.to_str().unwrap(),
        playbook.to_str().unwrap(),
        "--agent-mode",
    ]);
    assert!(ok, "the agent-mode run should succeed:\n{}", output);
    assert!(
        output.contains("failed=0") && output.contains("unreachable=0"),
        "no task should fail in agent mode:\n{}",
        output
    );
    assert!(
        target
            .exec("test -e /tmp/agent-mode-marker && echo present")
            .contains("present"),
        "the task should have run on the target"
    );
    // The agent counts what it executed, which is how we know commands went
    // through it rather than straight over SSH.
    let status = target.exec("/usr/local/bin/rustible-agent --status");
    assert!(
        status.contains("\"tasks_executed\""),
        "the agent should report its own status:\n{}",
        status
    );
}
