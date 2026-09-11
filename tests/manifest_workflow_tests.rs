//! Host manifests are recorded by a run and re-checked against the target.
//!
//! `HostManifest` and `ManifestStore` existed but nothing produced or consumed
//! them, so the "state manifest" feature was a library with no user. These
//! tests pin the whole loop: `run --manifest` writes what it applied,
//! `drift manifest check` compares it against the host, and an edit made
//! behind Rustible's back comes back as drift.

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

fn local_inventory(dir: &Path) -> PathBuf {
    write(
        dir,
        "inventory.yml",
        r#"---
all:
  hosts:
    node1:
      ansible_connection: local
"#,
    )
}

/// A playbook that manages a directory and a file inside the temp dir.
fn managed_playbook(dir: &Path, managed: &Path) -> PathBuf {
    write(
        dir,
        "site.yml",
        &format!(
            r#"---
- name: Managed resources
  hosts: all
  gather_facts: false
  tasks:
    - name: Managed directory
      file:
        path: {}
        state: directory

    - name: Managed file
      copy:
        content: "port = 8080\n"
        dest: {}/app.conf
        mode: "0640"
"#,
            managed.display(),
            managed.display()
        ),
    )
}

fn manifest_dir(temp: &Path) -> PathBuf {
    temp.join(".rustible").join("manifests")
}

#[test]
fn a_run_records_a_manifest_of_what_it_applied() {
    let temp = tempdir().unwrap();
    let managed = temp.path().join("managed");
    let inventory = local_inventory(temp.path());
    let playbook = managed_playbook(temp.path(), &managed);

    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .arg("--manifest")
        .assert()
        .code(0);

    let path = manifest_dir(temp.path()).join("node1.manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("manifest written")).unwrap();

    let resources = manifest["resources"].as_object().expect("resources");
    assert_eq!(
        resources.len(),
        2,
        "both managed resources belong in the manifest: {:#?}",
        resources
    );
    assert!(
        resources.contains_key(&format!("file::{}", managed.display())),
        "the directory is keyed by its path: {:?}",
        resources.keys().collect::<Vec<_>>()
    );
    assert_eq!(manifest["hostname"], "node1");
}

#[test]
fn an_unchanged_task_still_reaches_the_manifest() {
    let temp = tempdir().unwrap();
    let managed = temp.path().join("managed");
    let inventory = local_inventory(temp.path());
    let playbook = managed_playbook(temp.path(), &managed);

    // The first run changes everything; the second must change nothing.
    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .arg("--manifest")
        .assert()
        .code(0);

    // Throw away the first run's manifest, so the second run has to produce
    // one from scratch out of tasks that report no change. Without this the
    // assertion below passes on the file the first run left behind.
    fs::remove_dir_all(manifest_dir(temp.path())).unwrap();

    let second = rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .arg("--manifest")
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let second = String::from_utf8(second).unwrap();
    assert!(
        second.contains("changed=0"),
        "the second run must be a no-op, or this test is not exercising \
         unchanged tasks at all:\n{}",
        second
    );

    let path = manifest_dir(temp.path()).join("node1.manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        manifest["resources"].as_object().unwrap().len(),
        2,
        "a no-op run still describes the resources it verified"
    );
}

#[test]
fn checking_a_manifest_reports_in_sync_then_drift() {
    let temp = tempdir().unwrap();
    let managed = temp.path().join("managed");
    let inventory = local_inventory(temp.path());
    let playbook = managed_playbook(temp.path(), &managed);
    let manifests = manifest_dir(temp.path());

    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .arg("--manifest")
        .assert()
        .code(0);

    rustible_cmd()
        .arg("drift")
        .arg("manifest")
        .arg("check")
        .arg("--dir")
        .arg(&manifests)
        .arg("-i")
        .arg(&inventory)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("2 in sync"));

    // Edit the file behind Rustible's back.
    fs::write(managed.join("app.conf"), "port = 9999\n").unwrap();

    rustible_cmd()
        .arg("drift")
        .arg("manifest")
        .arg("check")
        .arg("--dir")
        .arg(&manifests)
        .arg("-i")
        .arg(&inventory)
        .assert()
        .code(2)
        .stdout(predicate::str::contains("1 drifted"));
}

#[test]
fn a_manifest_can_be_listed_and_shown() {
    let temp = tempdir().unwrap();
    let managed = temp.path().join("managed");
    let inventory = local_inventory(temp.path());
    let playbook = managed_playbook(temp.path(), &managed);
    let manifests = manifest_dir(temp.path());

    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .arg("--manifest")
        .assert()
        .code(0);

    rustible_cmd()
        .arg("drift")
        .arg("manifest")
        .arg("list")
        .arg("--dir")
        .arg(&manifests)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("node1: 2 resource(s)"));

    rustible_cmd()
        .arg("drift")
        .arg("manifest")
        .arg("show")
        .arg("node1")
        .arg("--dir")
        .arg(&manifests)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("app.conf"));
}

#[test]
fn an_unreachable_host_leaves_its_resources_unknown() {
    let temp = tempdir().unwrap();
    let managed = temp.path().join("managed");
    let inventory = local_inventory(temp.path());
    let playbook = managed_playbook(temp.path(), &managed);
    let manifests = manifest_dir(temp.path());

    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .arg("--manifest")
        .assert()
        .code(0);

    // Point the same host at a port nothing listens on. The resources still
    // exist on this machine, so a check that inspected the control node would
    // report them in sync.
    let unreachable = write(
        temp.path(),
        "unreachable.yml",
        r#"---
all:
  hosts:
    node1:
      ansible_host: 127.0.0.1
      ansible_port: 9
      ansible_user: nobody
      ansible_ssh_retries: 0
"#,
    );

    rustible_cmd()
        .arg("drift")
        .arg("manifest")
        .arg("check")
        .arg("--dir")
        .arg(&manifests)
        .arg("-i")
        .arg(&unreachable)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("2 unknown"));
}

#[test]
fn a_run_without_the_flag_records_nothing() {
    let temp = tempdir().unwrap();
    let managed = temp.path().join("managed");
    let inventory = local_inventory(temp.path());
    let playbook = managed_playbook(temp.path(), &managed);

    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .code(0);

    assert!(
        !manifest_dir(temp.path()).exists(),
        "manifests are opt-in; a plain run must not write state next to the playbook"
    );
}
