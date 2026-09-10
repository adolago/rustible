//! Transport selection for `rustible run`.
//!
//! The CLI builds a connection factory from the inventory. Without one every
//! remote task reports "requires an established connection" no matter how the
//! host is configured, so these tests pin the wiring: an inventory host that
//! asks for the local transport runs, and an SSH host reports the real
//! connection failure.

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

/// A port nothing listens on, so the connection attempt fails immediately.
const CLOSED_PORT: u16 = 2201;

#[test]
fn inventory_host_with_local_connection_runs_under_any_name() {
    let temp = tempdir().unwrap();
    let marker = temp.path().join("ran");
    let inventory = write(
        temp.path(),
        "inventory.yml",
        r#"---
all:
  hosts:
    web1:
      ansible_connection: local
"#,
    );
    let playbook = write(
        temp.path(),
        "site.yml",
        &format!(
            r#"---
- name: Local transport
  hosts: all
  gather_facts: false
  tasks:
    - name: Create the marker
      file:
        path: {}
        state: touch
"#,
            marker.display()
        ),
    );

    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .success();

    assert!(
        marker.exists(),
        "a host named web1 with ansible_connection: local should execute locally"
    );
}

#[test]
fn ssh_host_reports_the_connection_failure() {
    let temp = tempdir().unwrap();
    let inventory = write(
        temp.path(),
        "inventory.yml",
        &format!(
            r#"---
all:
  hosts:
    remote1:
      ansible_host: 127.0.0.1
      ansible_port: {}
      ansible_user: nobody
"#,
            CLOSED_PORT
        ),
    );
    let playbook = write(
        temp.path(),
        "site.yml",
        r#"---
- name: Remote transport
  hosts: all
  gather_facts: false
  tasks:
    - name: Say hello
      command: echo hello
"#,
    );

    let assert = rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert();

    // The inventory port must be the one dialled, and the reported reason must
    // be the transport error rather than "no connection was established".
    let dialled = format!("127.0.0.1:{}", CLOSED_PORT);
    assert.stdout(
        predicate::str::contains("Failed to connect").and(predicate::str::contains(dialled)),
    );
}

#[test]
fn mixed_inventory_runs_local_hosts_and_reports_unreachable_ones() {
    let temp = tempdir().unwrap();
    let marker = temp.path().join("ran");
    let inventory = write(
        temp.path(),
        "inventory.yml",
        &format!(
            r#"---
all:
  hosts:
    local1:
      ansible_connection: local
    remote1:
      ansible_host: 127.0.0.1
      ansible_port: {}
      ansible_user: nobody
"#,
            CLOSED_PORT
        ),
    );
    let playbook = write(
        temp.path(),
        "site.yml",
        &format!(
            r#"---
- name: Mixed transports
  hosts: all
  gather_facts: false
  tasks:
    - name: Create the marker
      file:
        path: {}
        state: touch
"#,
            marker.display()
        ),
    );

    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .stdout(predicate::str::contains("local1").and(predicate::str::contains("unreachable=1")));

    assert!(
        marker.exists(),
        "the local host should still run when another host is unreachable"
    );
}
