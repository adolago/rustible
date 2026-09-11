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

/// A project whose playbook uses two roles, one of them with a dependency.
fn role_project() -> TempDir {
    let temp = tempdir().unwrap();
    for name in ["common", "web"] {
        let role = temp.path().join("roles").join(name);
        fs::create_dir_all(role.join("tasks")).unwrap();
        fs::write(
            role.join("tasks").join("main.yml"),
            format!("---\n- name: {} task\n  debug:\n    msg: {}\n", name, name),
        )
        .unwrap();
    }
    let meta = temp.path().join("roles").join("web").join("meta");
    fs::create_dir_all(&meta).unwrap();
    fs::write(
        meta.join("main.yml"),
        "---\ndependencies:\n  - common\ngalaxy_info:\n  version: 1.2.3\n",
    )
    .unwrap();

    fs::write(
        temp.path().join("site.yml"),
        "---\n- name: Role lock demo\n  hosts: localhost\n  gather_facts: false\n  roles:\n    - web\n    - common\n",
    )
    .unwrap();
    temp
}

#[test]
fn roles_are_locked_with_their_tree_checksum() {
    let project = role_project();
    let playbook = project.path().join("site.yml");

    rustible_cmd()
        .arg("lock")
        .arg(&playbook)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("2 role(s)"));

    let lockfile: toml::Value = toml::from_str(
        &fs::read_to_string(project.path().join("rustible.lock")).expect("lockfile written"),
    )
    .expect("lockfile is TOML");

    let roles = lockfile["roles"].as_table().expect("roles recorded");
    assert_eq!(roles.len(), 2);
    assert_eq!(roles["web"]["version"].as_str(), Some("1.2.3"));
    // A role with no galaxy_info still gets locked, under a placeholder version.
    assert_eq!(roles["common"]["version"].as_str(), Some("local"));
    assert_eq!(roles["web"]["dependencies"][0].as_str(), Some("common"));
    assert!(
        roles["web"]["checksum"]
            .as_str()
            .is_some_and(|checksum| checksum.len() == 64),
        "a role is locked by a SHA256 over its tree"
    );
}

#[test]
fn verify_detects_an_edit_inside_a_role() {
    let project = role_project();
    let playbook = project.path().join("site.yml");

    rustible_cmd().arg("lock").arg(&playbook).assert().code(0);
    rustible_cmd()
        .arg("lock")
        .arg(&playbook)
        .arg("verify")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Integrity check passed"));

    // Editing a task file inside a role changes what the run will do.
    let task = project
        .path()
        .join("roles")
        .join("web")
        .join("tasks")
        .join("main.yml");
    fs::write(
        &task,
        "---\n- name: web task\n  debug:\n    msg: tampered\n",
    )
    .unwrap();

    rustible_cmd()
        .arg("lock")
        .arg(&playbook)
        .arg("verify")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Integrity check failed"));
}

#[test]
fn a_role_that_is_not_installed_is_not_locked() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("site.yml"),
        "---\n- name: Missing role\n  hosts: localhost\n  gather_facts: false\n  roles:\n    - community.general.absent\n",
    )
    .unwrap();

    // Nothing is on disk to hash, so nothing is recorded; `verify` must not
    // later claim it checked an artifact it never read.
    rustible_cmd()
        .arg("lock")
        .arg(temp.path().join("site.yml"))
        .assert()
        .code(0)
        .stdout(predicate::str::contains("0 role(s)"));
}
