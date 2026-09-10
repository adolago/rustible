//! Cross-run task state cache tests (`rustible run --cache-state`).
//!
//! The cache exists so a repeat run of an unchanged playbook does not redo
//! work. These tests drive the real CLI against a local playbook and assert on
//! the persisted cache file, because the value of the feature is what survives
//! between two separate processes.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn rustible_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("rustible")
}

fn task_cache_path(playbook: &Path) -> PathBuf {
    playbook
        .parent()
        .unwrap()
        .join(".rustible")
        .join("state")
        .join("task-cache.json")
}

/// A playbook that creates a directory: changed on the first run, unchanged on
/// every run after that.
fn write_playbook(dir: &Path, target: &Path) -> PathBuf {
    let playbook = dir.join("cacheable.yml");
    fs::write(
        &playbook,
        format!(
            r#"---
- name: Cacheable playbook
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Create a directory
      file:
        path: {}
        state: directory
"#,
            target.display()
        ),
    )
    .unwrap();
    playbook
}

fn cache_entries(playbook: &Path) -> serde_json::Value {
    let contents = fs::read_to_string(task_cache_path(playbook))
        .expect("the cache file should exist after a run with --cache-state");
    serde_json::from_str(&contents).expect("cache file should be valid JSON")
}

#[test]
fn cache_state_records_unchanged_tasks_and_reuses_them() {
    let temp = tempdir().unwrap();
    let target = temp.path().join("managed-dir");
    let playbook = write_playbook(temp.path(), &target);

    // First run: the directory is created, so nothing is cacheable yet.
    rustible_cmd()
        .arg("run")
        .arg(&playbook)
        .arg("--cache-state")
        .assert()
        .success();
    assert!(target.is_dir(), "the playbook should create the directory");

    let after_first = cache_entries(&playbook);
    assert_eq!(after_first["version"], 1);
    assert_eq!(
        after_first["entries"].as_object().unwrap().len(),
        0,
        "a task that changed something must not be cached"
    );

    // Second run: the task is a no-op, which is what makes it cacheable.
    rustible_cmd()
        .arg("run")
        .arg(&playbook)
        .arg("--cache-state")
        .assert()
        .success();

    let after_second = cache_entries(&playbook);
    let entries = after_second["entries"].as_object().unwrap();
    assert_eq!(
        entries.len(),
        1,
        "the unchanged task should now have a cache entry"
    );
    let entry = entries.values().next().unwrap();
    assert_eq!(entry["changed"], serde_json::json!(false));
    assert_eq!(entry["status"], serde_json::json!("ok"));

    // Third run: the cached entry is reused. Removing the directory first
    // makes the skip observable — a task that ran would recreate it.
    fs::remove_dir(&target).unwrap();
    rustible_cmd()
        .arg("run")
        .arg(&playbook)
        .arg("--cache-state")
        .assert()
        .success();
    assert!(
        !target.exists(),
        "the cached task should have been skipped instead of re-running"
    );

    // Without the cache the task runs again and restores the directory.
    rustible_cmd().arg("run").arg(&playbook).assert().success();
    assert!(target.is_dir());
}

#[test]
fn cache_state_is_off_by_default() {
    let temp = tempdir().unwrap();
    let target = temp.path().join("managed-dir");
    let playbook = write_playbook(temp.path(), &target);

    rustible_cmd().arg("run").arg(&playbook).assert().success();
    rustible_cmd().arg("run").arg(&playbook).assert().success();

    assert!(
        !task_cache_path(&playbook).exists(),
        "no cache file should be written without --cache-state"
    );
}

#[test]
fn changed_arguments_do_not_reuse_a_cached_result() {
    let temp = tempdir().unwrap();
    let first_target = temp.path().join("first-dir");
    let playbook = write_playbook(temp.path(), &first_target);

    // Two runs so the no-op result is cached.
    for _ in 0..2 {
        rustible_cmd()
            .arg("run")
            .arg(&playbook)
            .arg("--cache-state")
            .assert()
            .success();
    }
    assert_eq!(
        cache_entries(&playbook)["entries"]
            .as_object()
            .unwrap()
            .len(),
        1
    );

    // Point the same task at a different directory: the arguments are part of
    // the hash, so the cached entry must not apply.
    let second_target = temp.path().join("second-dir");
    write_playbook(temp.path(), &second_target);
    rustible_cmd()
        .arg("run")
        .arg(&playbook)
        .arg("--cache-state")
        .assert()
        .success();

    assert!(
        second_target.is_dir(),
        "the task must run again when its arguments change"
    );
}

#[test]
fn check_mode_ignores_the_cache() {
    let temp = tempdir().unwrap();
    let target = temp.path().join("managed-dir");
    let playbook = write_playbook(temp.path(), &target);

    for _ in 0..2 {
        rustible_cmd()
            .arg("run")
            .arg(&playbook)
            .arg("--cache-state")
            .assert()
            .success();
    }

    let before = fs::read_to_string(task_cache_path(&playbook)).unwrap();

    rustible_cmd()
        .arg("run")
        .arg(&playbook)
        .arg("--check")
        .arg("--cache-state")
        .assert()
        .success();

    let after = fs::read_to_string(task_cache_path(&playbook)).unwrap();
    assert_eq!(
        before, after,
        "check mode must neither read nor rewrite the cache"
    );
}
