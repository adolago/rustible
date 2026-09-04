//! Pure process-builder inspection and check-mode TempDir guards; never spawn.
use super::*;
use serde_json::json;
use std::ffi::OsStr;
use tempfile::TempDir;

fn params() -> ModuleParams {
    [
        ("cmd".into(), json!("synthetic-command")),
        ("shell_type".into(), json!("posix")),
    ]
    .into_iter()
    .collect()
}

#[test]
fn quoted_words_and_empty_arguments_are_preserved() {
    for field in ["cmd", "_raw_params"] {
        let mut request = params();
        request.remove("cmd");
        request.insert(
            field.into(),
            json!("synthetic-command 'two words' \"\" tail"),
        );
        let process = CommandModule
            .build_command(&request, &ModuleContext::default())
            .unwrap();
        assert_eq!(process.get_program(), "synthetic-command");
        assert_eq!(
            process.get_args().collect::<Vec<_>>(),
            [OsStr::new("two words"), OsStr::new(""), OsStr::new("tail")]
        );
    }
}

#[test]
fn argv_precedes_freeform_and_preserves_literal_values() {
    let mut request = params();
    request.insert("cmd".into(), json!("unused 'unterminated"));
    request.insert(
        "argv".into(),
        json!(["synthetic-command", "", "two words", "$literal"]),
    );
    let process = CommandModule
        .build_command(&request, &ModuleContext::default())
        .unwrap();
    assert_eq!(process.get_program(), "synthetic-command");
    assert_eq!(
        process.get_args().collect::<Vec<_>>(),
        [
            OsStr::new(""),
            OsStr::new("two words"),
            OsStr::new("$literal")
        ]
    );
}

#[test]
fn malformed_quotes_and_empty_programs_are_rejected() {
    for cmd in [
        "synthetic-command 'unfinished",
        "synthetic-command \"unfinished",
        "'' argument",
        "\"\" argument",
        " ",
    ] {
        let mut request = params();
        request.insert("cmd".into(), json!(cmd));
        assert!(
            CommandModule
                .build_command(&request, &ModuleContext::default())
                .is_err(),
            "{cmd:?}"
        );
    }
    let mut request = params();
    request.insert("argv".into(), json!(["", "argument"]));
    assert!(CommandModule
        .build_command(&request, &ModuleContext::default())
        .is_err());
}

#[test]
fn check_mode_rejects_malformed_quotes_without_spawning() {
    let mut request = params();
    request.insert("cmd".into(), json!("synthetic-command 'unfinished"));
    assert!(CommandModule
        .execute(&request, &ModuleContext::default().with_check_mode(true))
        .is_err());
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
                CommandModule.execute(&request, &context).unwrap().changed,
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
    assert!(CommandModule.execute(&request, &context).unwrap().changed);
    request.insert("creates".into(), json!(guard));
    assert!(!CommandModule.execute(&request, &context).unwrap().changed);
}

#[test]
fn relative_work_directory_is_resolved_from_the_process_directory() {
    let current_dir = std::env::current_dir().unwrap();
    let temp = tempfile::Builder::new()
        .prefix(".command-guard-")
        .tempdir_in(&current_dir)
        .unwrap();
    let relative_dir = temp.path().strip_prefix(&current_dir).unwrap();
    assert!(relative_dir.is_relative());
    std::fs::write(temp.path().join("guard"), "marker").unwrap();
    let mut request = params();
    request.insert("creates".into(), json!("guard"));
    let mut context = ModuleContext::default().with_check_mode(true);
    context.work_dir = Some(relative_dir.to_str().unwrap().to_string());
    assert!(!CommandModule.execute(&request, &context).unwrap().changed);
}
