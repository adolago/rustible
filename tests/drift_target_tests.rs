//! `rustible drift detect` inspects the target, not the control node.
//!
//! Drift used to run every module's check mode with no connection, so a report
//! about a remote host described whatever happened to be true on the machine
//! running Rustible. These tests pin the two halves of the fix: a host that is
//! reachable is really inspected, and a host that is not is reported as
//! unknown rather than as "in sync".

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn rustible_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("rustible")
}

fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path
}

/// A playbook whose desired state is a directory under the temp dir.
fn drift_playbook(dir: &Path, managed: &Path) -> PathBuf {
    write(
        dir,
        "site.yml",
        &format!(
            r#"---
- name: Drift target
  hosts: all
  gather_facts: false
  tasks:
    - name: Managed directory
      file:
        path: {}
        state: directory
        mode: "0755"
"#,
            managed.display()
        ),
    )
}

#[test]
fn drift_reports_state_of_a_local_host() {
    let temp = tempdir().unwrap();
    let managed = temp.path().join("managed-dir");
    let inventory = write(
        temp.path(),
        "inventory.yml",
        r#"---
all:
  hosts:
    node1:
      ansible_connection: local
"#,
    );
    let playbook = drift_playbook(temp.path(), &managed);

    // The directory does not exist yet, so the resource has drifted.
    rustible_cmd()
        .arg("drift")
        .arg("detect")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .code(2)
        .stdout(predicate::str::contains("Managed directory"));

    fs::create_dir(&managed).unwrap();
    // The mode is part of the desired state, so set it explicitly rather than
    // relying on the umask.
    fs::set_permissions(
        &managed,
        <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .unwrap();

    // With the desired state in place the report must come back clean.
    rustible_cmd()
        .arg("drift")
        .arg("detect")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("No changes"));
}

#[test]
fn drift_does_not_claim_a_match_for_an_unreachable_host() {
    let temp = tempdir().unwrap();
    let managed = temp.path().join("managed-dir");
    // Create the directory locally: if drift inspected the control node it
    // would report this host as in sync.
    fs::create_dir(&managed).unwrap();

    let inventory = write(
        temp.path(),
        "inventory.yml",
        r#"---
all:
  hosts:
    remote1:
      ansible_host: 127.0.0.1
      ansible_port: 2201
      ansible_user: nobody
"#,
    );
    let playbook = drift_playbook(temp.path(), &managed);

    rustible_cmd()
        .arg("drift")
        .arg("detect")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .code(1)
        // Warnings go to stderr; the summary must not claim a match.
        .stderr(
            predicate::str::contains("could not be checked")
                .and(predicate::str::contains("no connection to the host")),
        )
        .stdout(predicate::str::contains("No changes").not());
}
