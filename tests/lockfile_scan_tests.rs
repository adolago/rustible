//! `rustible lock` records the files a playbook depends on.
//!
//! The lockfile exists so a repeat run can prove it uses the same inputs. That
//! only works if creating one actually scans the playbook, which it did not:
//! the scan was a stub that always locked zero items.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::{tempdir, TempDir};

fn rustible_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("rustible")
}

/// A playbook directory with a template, a copied file and a nested block.
fn playbook_project() -> TempDir {
    let temp = tempdir().unwrap();
    fs::create_dir(temp.path().join("templates")).unwrap();
    fs::create_dir(temp.path().join("files")).unwrap();

    fs::write(
        temp.path().join("site.yml"),
        r#"---
- name: Lock demo
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Render a template
      template:
        src: app.conf.j2
        dest: /tmp/rustible-lock-app.conf

    - name: Copy a file
      copy:
        src: payload.txt
        dest: /tmp/rustible-lock-payload.txt

    - name: Block with a nested copy
      block:
        - name: Nested copy
          copy:
            src: nested.txt
            dest: /tmp/rustible-lock-nested.txt

    - name: A templated source cannot be locked
      copy:
        src: "{{ dynamic_source }}"
        dest: /tmp/rustible-lock-dynamic.txt
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("templates/app.conf.j2"),
        "port = {{ port }}\n",
    )
    .unwrap();
    fs::write(temp.path().join("files/payload.txt"), "payload\n").unwrap();
    fs::write(temp.path().join("files/nested.txt"), "nested\n").unwrap();
    temp
}

fn lock(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = rustible_cmd();
    command.current_dir(dir).arg("lock").arg("site.yml");
    for arg in args {
        command.arg(arg);
    }
    command.assert()
}

#[test]
fn lock_records_local_file_dependencies() {
    let project = playbook_project();

    lock(project.path(), &[])
        .success()
        .stdout(predicate::str::contains("3 locked items"));

    let lockfile = fs::read_to_string(project.path().join("rustible.lock")).unwrap();
    for expected in ["app.conf.j2", "payload.txt", "nested.txt"] {
        assert!(
            lockfile.contains(expected),
            "{} should be locked:\n{}",
            expected,
            lockfile
        );
    }
    assert!(
        !lockfile.contains("dynamic_source"),
        "a templated source cannot be resolved at lock time:\n{}",
        lockfile
    );

    lock(project.path(), &["verify"])
        .success()
        .stdout(predicate::str::contains("verification successful"));
}

#[test]
fn verify_detects_an_edited_dependency() {
    let project = playbook_project();
    lock(project.path(), &[]).success();

    fs::write(
        project.path().join("templates/app.conf.j2"),
        "port = 8080\n",
    )
    .unwrap();

    lock(project.path(), &["verify"]).stdout(predicate::str::contains("Integrity check failed"));
}

#[test]
fn info_reports_what_was_locked() {
    let project = playbook_project();
    lock(project.path(), &[]).success();

    lock(project.path(), &["info"])
        .success()
        .stdout(predicate::str::contains("Resources: 3"));
}
