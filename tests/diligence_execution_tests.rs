//! End-to-end regressions for the live CLI, not a parallel parser or mock executor.
#![cfg(unix)]

use std::path::Path;
use std::time::Duration;

use assert_cmd::Command;
use serde_json::Value;

const TOKEN: &str = "CORE_AUDIT_SYNTHETIC_NOT_A_SECRET_7429";

struct Run {
    scratch: tempfile::TempDir,
    output: std::process::Output,
    events: Vec<Value>,
    artifacts: String,
}

fn read_artifacts(path: &Path, output: &mut String, events: &mut Vec<Value>) {
    if !path.exists() {
        return;
    }
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            read_artifacts(&path, output, events);
        } else {
            let content = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                events.extend(
                    content
                        .lines()
                        .map(|line| serde_json::from_str::<Value>(line).unwrap()),
                );
            }
            output.push_str(&content);
        }
    }
}

fn run(fixture: &str, flags: &[&str], remote: bool, missing_inventory: bool) -> Run {
    let scratch = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/diligence");
    let playbook = std::fs::read_to_string(source.join(fixture))
        .unwrap()
        .replace("__SCRATCH__", scratch.path().to_str().unwrap());
    std::fs::write(scratch.path().join("playbook.yml"), playbook).unwrap();
    let included = std::fs::read_to_string(source.join("included-remote.yml"))
        .unwrap()
        .replace("__SCRATCH__", scratch.path().to_str().unwrap());
    std::fs::write(scratch.path().join("included-remote.yml"), included).unwrap();
    std::fs::write(
        scratch.path().join("rustible.toml"),
        "[defaults]\nforks = 2\ntimeout = 1\n",
    )
    .unwrap();
    if !missing_inventory {
        let inventory = if remote {
            "remote-inventory.yml"
        } else if fixture == "group-local.yml" {
            "group-inventory.yml"
        } else {
            "inventory.yml"
        };
        std::fs::copy(source.join(inventory), scratch.path().join("inventory.yml")).unwrap();
    }
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("rustible"));
    cmd.current_dir(scratch.path())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("NO_COLOR", "1")
        .timeout(Duration::from_secs(15))
        .args([
            "--no-color",
            "--config",
            "rustible.toml",
            "--inventory",
            "inventory.yml",
            "--timeout",
            "1",
            "run",
            "playbook.yml",
            "--output-bundle",
            "bundle",
        ])
        .args(flags);
    // Remote cases must be rejected before any connection attempt. No connection
    // factory is configured by this CLI, and the only file target is this tempdir.
    let output = cmd.output().unwrap();
    let mut artifacts = String::new();
    let mut events = Vec::new();
    read_artifacts(&scratch.path().join("bundle"), &mut artifacts, &mut events);
    read_artifacts(
        &scratch.path().join(".rustible"),
        &mut artifacts,
        &mut events,
    );
    Run {
        scratch,
        output,
        events,
        artifacts,
    }
}

fn tasks(run: &Run) -> Vec<(&str, &str, &str)> {
    run.events
        .iter()
        .filter(|event| event["type"] == "task_result")
        .map(|event| {
            (
                event["task"].as_str().unwrap(),
                event["host"].as_str().unwrap(),
                event["status"].as_str().unwrap(),
            )
        })
        .collect()
}

#[test]
fn limit_excludes_hosts_from_execution() {
    let result = run("hosts.yml", &["--limit", "selected"], false, false);
    assert!(result.output.status.success());
    let tasks = tasks(&result);
    assert!(!tasks.is_empty());
    assert!(tasks.iter().all(|(_, host, _)| *host == "selected"));
}

#[test]
fn missing_remote_connection_cannot_touch_controller_files() {
    let result = run("remote-file.yml", &[], true, false);
    assert!(!result.output.status.success());
    assert!(!result
        .scratch
        .path()
        .join("remote-controller-sentinel")
        .exists());
}

#[test]
fn native_command_exit_and_registered_stdout_are_real() {
    let failure = run("command-false.yml", &[], false, false);
    assert!(!failure.output.status.success());
    assert!(tasks(&failure)
        .iter()
        .any(|(task, _, status)| *task == "command-must-fail" && *status == "failed"));
    let success = run("command-register.yml", &[], false, false);
    assert!(
        success.output.status.success(),
        "{}",
        String::from_utf8_lossy(&success.output.stderr)
    );
    assert!(tasks(&success)
        .iter()
        .any(|(task, _, status)| *task == "verify-stdout" && *status == "ok"));
}

#[test]
fn no_log_is_rejected_without_leaking_in_any_output() {
    for flags in [&[][..], &["-vvv"][..]] {
        let result = run("no-log.yml", flags, false, false);
        assert!(!result.output.status.success());
        assert!(!String::from_utf8_lossy(&result.output.stdout).contains(TOKEN));
        assert!(!String::from_utf8_lossy(&result.output.stderr).contains(TOKEN));
        assert!(!result.artifacts.contains(TOKEN));
        assert!(tasks(&result).is_empty());
    }
}

#[test]
fn unrescued_block_fails_and_stops_following_tasks() {
    let result = run("unrescued-block.yml", &[], false, false);
    assert!(!result.output.status.success());
    assert!(!tasks(&result)
        .iter()
        .any(|(name, _, _)| *name == "should-not-run-after-block"));
}

#[test]
fn live_task_selection_respects_tags_skip_tags_and_start() {
    for flags in [
        ["--tags", "wanted"],
        ["--skip-tags", "unwanted"],
        ["--start-at-task", "wanted-task"],
    ] {
        let result = run("tags.yml", &flags, false, false);
        assert!(result.output.status.success(), "selection {flags:?}");
        assert!(tasks(&result)
            .iter()
            .any(|(name, _, _)| *name == "wanted-task"));
        assert!(!tasks(&result)
            .iter()
            .any(|(name, _, _)| *name == "unwanted-task"));
    }
}

#[test]
fn unsatisfied_until_and_missing_inventory_fail() {
    assert!(!run("retries.yml", &[], false, false)
        .output
        .status
        .success());
    let result = run("hosts.yml", &[], false, true);
    assert!(!result.output.status.success());
    assert!(tasks(&result).is_empty());
}

#[test]
fn handlers_are_host_scoped_ordered_and_failure_is_terminal() {
    assert!(!run("handler-failure.yml", &[], false, false)
        .output
        .status
        .success());
    let scoped = run("handler-hosts.yml", &[], false, false);
    assert!(scoped.output.status.success());
    assert!(tasks(&scoped)
        .iter()
        .all(|(_, host, _)| *host == "selected"));
    assert_eq!(
        tasks(&scoped)
            .iter()
            .filter(|(name, _, _)| *name == "host-handler")
            .count(),
        1
    );
    let ordered = run("handler-order.yml", &[], false, false);
    assert!(ordered.output.status.success());
    let handlers: Vec<_> = tasks(&ordered)
        .into_iter()
        .filter(|(name, _, _)| *name == "z-first-handler" || *name == "a-second-handler")
        .map(|(name, _, _)| name)
        .collect();
    assert_eq!(handlers, ["z-first-handler", "a-second-handler"]);
}

#[test]
fn unreachable_loop_and_include_never_become_success() {
    for fixture in ["remote-failed-when.yml", "remote-until.yml"] {
        let result = run(fixture, &[], true, false);
        assert!(!result.output.status.success());
        assert!(tasks(&result)
            .iter()
            .any(|(_, _, status)| *status == "unreachable"));
    }
    for flags in [&[][..], &["--no-pipelining"][..]] {
        let result = run("remote-loop.yml", flags, true, false);
        assert!(!result.output.status.success());
        assert!(tasks(&result)
            .iter()
            .any(|(_, _, status)| *status == "unreachable"));
        assert!(!result
            .scratch
            .path()
            .join("remote-controller-sentinel")
            .exists());
    }
    let included = run("remote-include.yml", &[], true, false);
    assert!(!included.output.status.success());
    assert!(tasks(&included)
        .iter()
        .any(|(_, _, status)| *status == "unreachable"));
    assert!(!included
        .scratch
        .path()
        .join("remote-controller-sentinel")
        .exists());
}

#[test]
fn loop_notifications_keep_the_notifying_host() {
    let result = run("loop-handler.yml", &[], false, false);
    assert!(result.output.status.success());
    let handlers: Vec<_> = tasks(&result)
        .into_iter()
        .filter(|(name, _, _)| *name == "loop-handler")
        .collect();
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].1, "selected");
}

#[test]
fn play_privilege_escalation_cannot_fall_through_to_local_handlers() {
    let result = run("become-handler.yml", &[], false, false);
    assert!(!result.output.status.success());
    assert!(tasks(&result).is_empty());
}

#[test]
fn false_meta_condition_defers_handler_until_end_of_play() {
    let result = run("conditional-flush.yml", &[], false, false);
    assert!(result.output.status.success());
    let names: Vec<_> = tasks(&result)
        .into_iter()
        .map(|(name, _, _)| name)
        .collect();
    let check = names
        .iter()
        .position(|name| *name == "handler-not-yet-run")
        .unwrap();
    let handler = names
        .iter()
        .position(|name| *name == "delayed-handler")
        .unwrap();
    assert!(check < handler);
}

#[test]
fn explicit_transport_settings_control_execution_identity() {
    let local = run("group-local.yml", &[], false, false);
    assert!(local.output.status.success());
    assert!(tasks(&local)
        .iter()
        .any(|(name, _, status)| *name == "group-local" && *status == "changed"));
    for fixture in ["play-ssh.yml", "task-ssh.yml"] {
        let remote = run(fixture, &[], false, false);
        assert!(!remote.output.status.success(), "{fixture}");
        assert!(tasks(&remote)
            .iter()
            .any(|(_, _, status)| *status == "unreachable"));
    }
}
