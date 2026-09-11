//! Role dependencies resolve once, and a cycle is reported rather than fatal.
//!
//! `meta/main.yml` dependencies were expanded by plain recursion with no
//! memory of what had already been visited. Two roles that share a dependency
//! ran it twice, which is wrong for anything that is not idempotent, and two
//! roles that depend on each other recursed until the process aborted with a
//! stack overflow — before a single task ran.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

fn rustible_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("rustible")
}

/// Write a role with one debug task and optional `meta/main.yml` contents.
fn write_role(root: &Path, name: &str, meta: Option<&str>) {
    let role = root.join("roles").join(name);
    fs::create_dir_all(role.join("tasks")).unwrap();
    fs::write(
        role.join("tasks").join("main.yml"),
        format!("---\n- name: {} task\n  debug:\n    msg: {}\n", name, name),
    )
    .unwrap();

    if let Some(meta) = meta {
        fs::create_dir_all(role.join("meta")).unwrap();
        fs::write(role.join("meta").join("main.yml"), meta).unwrap();
    }
}

fn write_inventory(root: &Path) -> PathBuf {
    let path = root.join("inventory.yml");
    fs::write(
        &path,
        "---\nall:\n  hosts:\n    node1:\n      ansible_connection: local\n",
    )
    .unwrap();
    path
}

fn write_playbook(root: &Path, roles: &[&str]) -> PathBuf {
    let path = root.join("site.yml");
    let entries: String = roles
        .iter()
        .map(|role| format!("    - {}\n", role))
        .collect();
    fs::write(
        &path,
        format!(
            "---\n- name: Roles\n  hosts: all\n  gather_facts: false\n  roles:\n{}",
            entries
        ),
    )
    .unwrap();
    path
}

/// A tree where `web` and `db` both depend on `common`.
fn shared_dependency_tree() -> TempDir {
    let temp = tempdir().unwrap();
    write_role(temp.path(), "common", None);
    write_role(temp.path(), "web", Some("---\ndependencies:\n  - common\n"));
    write_role(temp.path(), "db", Some("---\ndependencies:\n  - common\n"));
    temp
}

#[test]
fn a_shared_dependency_runs_once() {
    let temp = shared_dependency_tree();
    let inventory = write_inventory(temp.path());
    let playbook = write_playbook(temp.path(), &["web", "db"]);

    let output = rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();

    let common_runs = output.matches("common task").count();
    assert_eq!(
        common_runs, 1,
        "a dependency shared by two roles must run once, not once per dependent:\n{}",
        output
    );
    assert!(output.contains("web task") && output.contains("db task"));
}

#[test]
fn allow_duplicates_opts_back_into_repeated_runs() {
    let temp = tempdir().unwrap();
    write_role(temp.path(), "common", Some("---\nallow_duplicates: true\n"));
    write_role(temp.path(), "web", Some("---\ndependencies:\n  - common\n"));
    write_role(temp.path(), "db", Some("---\ndependencies:\n  - common\n"));

    let inventory = write_inventory(temp.path());
    let playbook = write_playbook(temp.path(), &["web", "db"]);

    let output = rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();

    assert_eq!(
        output.matches("common task").count(),
        2,
        "a role that opts in must still run per dependent:\n{}",
        output
    );
}

#[test]
fn a_dependency_cycle_is_an_error_not_a_crash() {
    let temp = tempdir().unwrap();
    write_role(temp.path(), "a", Some("---\ndependencies:\n  - b\n"));
    write_role(temp.path(), "b", Some("---\ndependencies:\n  - a\n"));

    let inventory = write_inventory(temp.path());
    let playbook = write_playbook(temp.path(), &["a"]);

    rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Role dependency cycle"));
}

#[test]
fn a_dependency_still_runs_before_its_dependent() {
    let temp = shared_dependency_tree();
    let inventory = write_inventory(temp.path());
    let playbook = write_playbook(temp.path(), &["web"]);

    let output = rustible_cmd()
        .arg("run")
        .arg("-i")
        .arg(&inventory)
        .arg(&playbook)
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();

    let common = output.find("common task").expect("dependency should run");
    let web = output.find("web task").expect("role should run");
    assert!(
        common < web,
        "a dependency runs before the role that declares it:\n{}",
        output
    );
}
