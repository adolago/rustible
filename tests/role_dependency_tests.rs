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

/// How many times a task actually ran, counted from its `TASK [...]` banner.
///
/// CI sets `RUST_LOG=debug` and the tracing output goes to stdout, where it
/// names the running task several times per execution. Counting a bare
/// substring therefore reports nine runs for one, so only the banner line
/// counts — a timestamped log line never starts with `TASK [`.
fn task_runs(output: &str, task_name: &str) -> usize {
    output
        .lines()
        .filter(|line| line.trim_start().starts_with("TASK ["))
        .filter(|line| line.contains(task_name))
        .count()
}

/// The position of a task's banner, for asserting execution order.
fn task_banner_index(output: &str, task_name: &str) -> Option<usize> {
    output
        .lines()
        .position(|line| line.trim_start().starts_with("TASK [") && line.contains(task_name))
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

    assert_eq!(
        task_runs(&output, "common task"),
        1,
        "a dependency shared by two roles must run once, not once per dependent:\n{}",
        output
    );
    assert_eq!(task_runs(&output, "web task"), 1);
    assert_eq!(task_runs(&output, "db task"), 1);
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
        task_runs(&output, "common task"),
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

    let common = task_banner_index(&output, "common task").expect("dependency should run");
    let web = task_banner_index(&output, "web task").expect("role should run");
    assert!(
        common < web,
        "a dependency runs before the role that declares it:\n{}",
        output
    );
}
