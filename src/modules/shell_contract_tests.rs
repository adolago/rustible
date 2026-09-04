//! Pure process-builder inspection and check-mode TempDir guards; never spawn.
use super::*;
use serde_json::json;
use std::ffi::OsStr;
use tempfile::TempDir;

fn params() -> ModuleParams {
    [("cmd".into(), json!("synthetic body"))]
        .into_iter()
        .collect()
}

#[test]
fn executable_arguments_and_opaque_body_are_preserved() {
    let mut request = params();
    request.insert(
        "executable".into(),
        json!("/synthetic/env 'shell with spaces' -u"),
    );
    let process = ShellModule
        .build_command(&request, &ModuleContext::default(), "opaque body")
        .unwrap();
    assert_eq!(process.get_program(), "/synthetic/env");
    assert_eq!(
        process.get_args().collect::<Vec<_>>(),
        [
            OsStr::new("shell with spaces"),
            OsStr::new("-u"),
            OsStr::new("-c"),
            OsStr::new("opaque body")
        ]
    );
}

#[test]
fn quoted_program_and_empty_argument_are_supported() {
    let mut request = params();
    request.insert("executable".into(), json!("'/synthetic path/sh' ''"));
    let process = ShellModule
        .build_command(&request, &ModuleContext::default(), "body")
        .unwrap();
    assert_eq!(process.get_program(), "/synthetic path/sh");
    assert_eq!(
        process.get_args().collect::<Vec<_>>(),
        [OsStr::new(""), OsStr::new("-c"), OsStr::new("body")]
    );
}

#[test]
fn malformed_and_empty_executables_fail_without_spawning() {
    for executable in ["", "   ", "'' argument", "\"\"", "'/synthetic/unfinished"] {
        let mut request = params();
        request.insert("executable".into(), json!(executable));
        assert!(
            ShellModule
                .build_command(&request, &ModuleContext::default(), "body")
                .is_err(),
            "{executable:?}"
        );
        assert!(ShellModule
            .execute(&request, &ModuleContext::default().with_check_mode(true))
            .is_err());
    }
}

#[test]
fn managed_command_flags_are_rejected_by_the_builder() {
    for flag in ["-c", "/c", "/C"] {
        let mut request = params();
        request.insert("executable".into(), json!(format!("/synthetic/sh {flag}")));
        assert!(ShellModule
            .build_command(&request, &ModuleContext::default(), "body")
            .is_err());
    }
}

#[test]
fn default_shell_and_existing_cmd_flag_are_preserved() {
    let mut request = params();
    let process = ShellModule
        .build_command(&request, &ModuleContext::default(), "body")
        .unwrap();
    assert_eq!(process.get_program(), "/bin/sh");
    assert_eq!(
        process.get_args().collect::<Vec<_>>(),
        [OsStr::new("-c"), OsStr::new("body")]
    );
    request.insert("executable".into(), json!("cmd.exe"));
    let process = ShellModule
        .build_command(&request, &ModuleContext::default(), "body")
        .unwrap();
    assert_eq!(process.get_program(), "cmd.exe");
    assert_eq!(
        process.get_args().collect::<Vec<_>>(),
        [OsStr::new("/c"), OsStr::new("body")]
    );
}

#[test]
fn guards_use_chdir_and_context_work_dir() {
    let temp = TempDir::new().unwrap();
    std::fs::write(temp.path().join("guard"), "marker").unwrap();
    for use_chdir in [false, true] {
        for (guard, expected_changed) in [("creates", false), ("removes", true)] {
            let mut request = params();
            request.insert(guard.into(), json!("guard"));
            let mut context = ModuleContext::default().with_check_mode(true);
            if use_chdir {
                request.insert("chdir".into(), json!(temp.path()));
            } else {
                context.work_dir = Some(temp.path().to_str().unwrap().to_string());
            }
            assert_eq!(
                ShellModule.execute(&request, &context).unwrap().changed,
                expected_changed
            );
        }
    }
}

#[test]
fn chdir_overrides_context_and_absolute_guards_ignore_both() {
    let with_guard = TempDir::new().unwrap();
    let empty = TempDir::new().unwrap();
    let guard = with_guard.path().join("guard");
    std::fs::write(&guard, "marker").unwrap();
    let mut request = params();
    request.insert("creates".into(), json!("guard"));
    request.insert("chdir".into(), json!(empty.path()));
    let mut context = ModuleContext::default().with_check_mode(true);
    context.work_dir = Some(with_guard.path().to_str().unwrap().to_string());
    assert!(ShellModule.execute(&request, &context).unwrap().changed);
    request.insert("creates".into(), json!(guard));
    assert!(!ShellModule.execute(&request, &context).unwrap().changed);
}

#[test]
fn relative_work_directory_is_resolved_from_the_process_directory() {
    let current_dir = std::env::current_dir().unwrap();
    let temp = tempfile::Builder::new()
        .prefix(".shell-guard-")
        .tempdir_in(&current_dir)
        .unwrap();
    let relative_dir = temp.path().strip_prefix(&current_dir).unwrap();
    assert!(relative_dir.is_relative());
    std::fs::write(temp.path().join("guard"), "marker").unwrap();
    let mut request = params();
    request.insert("creates".into(), json!("guard"));
    let mut context = ModuleContext::default().with_check_mode(true);
    context.work_dir = Some(relative_dir.to_str().unwrap().to_string());
    assert!(!ShellModule.execute(&request, &context).unwrap().changed);
}
