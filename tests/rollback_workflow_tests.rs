//! Checkpoint and rollback, end to end.
//!
//! Rolling back a run needs to know what each task found before it changed
//! anything. Nothing recorded that, so every rollback plan came out empty and
//! the feature was inert. These tests cover the whole loop: take a checkpoint,
//! run, then undo the run.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::{tempdir, TempDir};

fn rustible_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("rustible")
}

/// A project whose playbook creates one directory on the control node.
fn project() -> TempDir {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("inventory.yml"),
        r#"---
all:
  hosts:
    localhost:
      ansible_connection: local
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("site.yml"),
        format!(
            r#"---
- name: Rollback demo
  hosts: all
  gather_facts: false
  tasks:
    - name: Create a directory
      file:
        path: {}
        state: directory
"#,
            temp.path().join("managed").display()
        ),
    )
    .unwrap();
    temp
}

fn run_in(dir: &Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = rustible_cmd();
    command.current_dir(dir);
    for arg in args {
        command.arg(arg);
    }
    command.assert()
}

#[test]
fn checkpointed_run_can_be_rolled_back() {
    let project = project();
    let managed = project.path().join("managed");

    run_in(
        project.path(),
        &[
            "run",
            "-i",
            "inventory.yml",
            "site.yml",
            "--checkpoint",
            "before",
        ],
    )
    .success()
    .stdout(predicate::str::contains("Created checkpoint 'before'"));

    assert!(managed.is_dir(), "the run should create the directory");
    assert!(
        project
            .path()
            .join(".rustible/checkpoints/before.json")
            .exists(),
        "the checkpoint should be recorded next to the playbook"
    );

    // The plan names the action before anything is undone.
    run_in(
        project.path(),
        &["lock", "site.yml", "rollback", "before", "--dry-run"],
    )
    .success()
    .stdout(
        predicate::str::contains("remove created file")
            .and(predicate::str::contains("Run without --dry-run")),
    );
    assert!(managed.is_dir(), "a dry run must change nothing");

    run_in(project.path(), &["lock", "site.yml", "rollback", "before"])
        .success()
        .stdout(predicate::str::contains("Executed 1 rollback action"));

    assert!(
        !managed.exists(),
        "rollback should remove what the run created"
    );
}

#[test]
fn a_run_without_a_checkpoint_records_no_rollback_state() {
    let project = project();

    // Take a checkpoint, then run without --checkpoint: the run records no
    // before-state, so there is nothing to undo.
    run_in(
        project.path(),
        &["lock", "site.yml", "checkpoint", "-n", "manual"],
    )
    .success();
    run_in(project.path(), &["run", "-i", "inventory.yml", "site.yml"]).success();

    run_in(
        project.path(),
        &["lock", "site.yml", "rollback", "manual", "--dry-run"],
    )
    .success()
    .stdout(predicate::str::contains(
        "No rollback actions were generated",
    ));

    assert!(
        project.path().join("managed").is_dir(),
        "the directory stays; nothing was rolled back"
    );
}

#[test]
fn rollback_reports_an_unknown_checkpoint() {
    let project = project();

    run_in(
        project.path(),
        &["lock", "site.yml", "rollback", "does-not-exist"],
    )
    .failure()
    .stderr(predicate::str::contains("does-not-exist"));
}
