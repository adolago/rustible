//! Test the repository test runner without running Cargo recursively.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

fn run_runner(temp: &Path, flags: &[&str], fail: Option<&str>) -> (Output, String) {
    let bin = temp.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let log = temp.join("commands.log");
    let mock = r#"#!/bin/bash
printf '%s\n' "$*" >> "$DILIGENCE_CALL_LOG"
if [[ "$1" == "--version" ]]; then
    printf 'mock tool 1.0\n'
fi
if [[ -n "${DILIGENCE_FAIL_MATCH:-}" && "$*" == *"$DILIGENCE_FAIL_MATCH"* ]]; then
    exit 1
fi
exit 0
"#;
    for tool in ["cargo", "rustc"] {
        let file = bin.join(tool);
        fs::write(&file, mock).unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let output = Command::new("/bin/bash")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/run_tests.sh"))
        .args(flags)
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("DILIGENCE_CALL_LOG", &log)
        .env("DILIGENCE_FAIL_MATCH", fail.unwrap_or(""))
        .output()
        .unwrap();
    (output, fs::read_to_string(log).unwrap())
}

#[test]
fn test_runner_reaches_summary_after_successful_categories() {
    let temp = tempfile::tempdir().unwrap();
    let (output, calls) = run_runner(temp.path(), &["--quick"], None);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(calls.contains("test --test cli_smoke_tests"));
    assert!(calls.contains("test --test module_tests"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("All tests passed!"));
}

#[test]
fn test_runner_accumulates_optional_failures_and_finishes_remaining_categories() {
    let temp = tempfile::tempdir().unwrap();
    let (output, calls) = run_runner(temp.path(), &["--bench"], Some("fmt --all"));
    assert!(!output.status.success());
    assert!(calls.contains("test --test executor_comprehensive_tests"));
    assert!(calls.contains("bench --bench strategy_benchmark"));
    assert!(calls.contains("bench --bench performance_benchmark"));
    assert!(!calls.contains("--test error_tests"));
    assert!(!calls.contains("--bench execution_benchmark"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Some tests failed!"));
}

#[test]
fn test_runner_stops_on_required_build_failure() {
    let temp = tempfile::tempdir().unwrap();
    let (output, calls) = run_runner(temp.path(), &["--quick"], Some("build --all-features"));
    assert!(!output.status.success());
    assert!(!calls.contains("test --lib"));
}
