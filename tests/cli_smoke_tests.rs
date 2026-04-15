//! Targeted CLI smoke tests for the default CI baseline.
//!
//! These tests intentionally stay small and deterministic. They cover the
//! user-facing commands called out in the release checklists: `run`, `check`,
//! and `vault`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::{tempdir, NamedTempFile};

fn rustible_cmd() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("rustible")
}

fn create_test_playbook() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        r#"---
- name: Smoke playbook
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Smoke task
      debug:
        msg: "hello from smoke"
"#
    )
    .unwrap();
    file
}

#[test]
fn run_smoke_executes_local_playbook() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn check_smoke_reports_check_mode() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("check")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("CHECK MODE").or(predicate::str::contains("DRY RUN")));
}

#[test]
fn vault_smoke_round_trip_with_password_file() {
    let temp = tempdir().unwrap();
    let plaintext_path = temp.path().join("secret.txt");
    let encrypted_path = temp.path().join("secret.txt.vault");
    let decrypted_path = temp.path().join("secret.out");
    let password_path = temp.path().join(".vault_pass");

    fs::write(&plaintext_path, "top secret\n").unwrap();
    fs::write(&password_path, "test_password_123\n").unwrap();

    rustible_cmd()
        .arg("vault")
        .arg("encrypt")
        .arg(&plaintext_path)
        .arg("--output-file")
        .arg(&encrypted_path)
        .arg("--vault-password-file")
        .arg(&password_path)
        .assert()
        .success();

    let encrypted = fs::read_to_string(&encrypted_path).unwrap();
    assert!(
        encrypted.starts_with("$RUSTIBLE_VAULT"),
        "encrypted output should use the vault header"
    );

    rustible_cmd()
        .arg("vault")
        .arg("decrypt")
        .arg(&encrypted_path)
        .arg("--output-file")
        .arg(&decrypted_path)
        .arg("--vault-password-file")
        .arg(&password_path)
        .assert()
        .success();

    let decrypted = fs::read_to_string(&decrypted_path).unwrap();
    assert_eq!(decrypted, "top secret\n");
}
