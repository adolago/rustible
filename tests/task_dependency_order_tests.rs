//! Declared task dependencies (`provides` / `requires`) end to end.
//!
//! These run the real CLI so the whole path is covered: YAML parsing, the
//! ordering pass in the executor, and the error messages an author sees.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn rustible_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("rustible")
}

fn write_playbook(dir: &Path, body: &str) -> PathBuf {
    let playbook = dir.join("dag.yml");
    fs::write(&playbook, body).unwrap();
    playbook
}

/// Task order is observable through the order of appended lines.
#[test]
fn requires_reorders_tasks_before_execution() {
    let temp = tempdir().unwrap();
    let log = temp.path().join("order.log");
    let playbook = write_playbook(
        temp.path(),
        &format!(
            r#"---
- name: Dependency ordered play
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Configure app
      requires: database
      shell: echo configure-app >> {log}

    - name: Install database
      provides: database
      shell: echo install-database >> {log}
"#,
            log = log.display()
        ),
    );

    rustible_cmd().arg("run").arg(&playbook).assert().success();

    let contents = fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(
        lines,
        vec!["install-database", "configure-app"],
        "the provider must run before the task that requires it"
    );
}

#[test]
fn independent_tasks_keep_their_authored_order() {
    let temp = tempdir().unwrap();
    let log = temp.path().join("order.log");
    let playbook = write_playbook(
        temp.path(),
        &format!(
            r#"---
- name: Mixed play
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Install database
      provides: database
      shell: echo install-database >> {log}

    - name: Install app
      provides: app
      shell: echo install-app >> {log}

    - name: Configure app
      requires:
        - database
        - app
      shell: echo configure-app >> {log}
"#,
            log = log.display()
        ),
    );

    rustible_cmd().arg("run").arg(&playbook).assert().success();

    let contents = fs::read_to_string(&log).unwrap();
    assert_eq!(
        contents.lines().collect::<Vec<_>>(),
        vec!["install-database", "install-app", "configure-app"]
    );
}

#[test]
fn a_requirement_nobody_provides_fails_the_play() {
    let temp = tempdir().unwrap();
    let playbook = write_playbook(
        temp.path(),
        r#"---
- name: Missing provider
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Configure app
      requires: database
      debug:
        msg: never runs
"#,
    );

    rustible_cmd()
        .arg("run")
        .arg(&playbook)
        .assert()
        .failure()
        .stderr(predicate::str::contains("which no task provides"));
}

#[test]
fn a_dependency_cycle_fails_the_play() {
    let temp = tempdir().unwrap();
    let playbook = write_playbook(
        temp.path(),
        r#"---
- name: Cyclic dependencies
  hosts: localhost
  gather_facts: false
  tasks:
    - name: First
      provides: first_done
      requires: second_done
      debug:
        msg: never runs

    - name: Second
      provides: second_done
      requires: first_done
      debug:
        msg: never runs
"#,
    );

    rustible_cmd()
        .arg("run")
        .arg(&playbook)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Circular dependency"));
}
