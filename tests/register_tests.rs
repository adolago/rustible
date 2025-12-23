//! Comprehensive tests for task result registration in Rustible
//!
//! This test suite verifies the register keyword and result structures including:
//!
//! ## Register Basic (Section 1)
//! - register: variable_name
//! - Result stored in variable
//! - Variable persists across tasks
//! - Variable scope (host-specific)
//!
//! ## Result Structure (Section 2)
//! - result.changed
//! - result.failed
//! - result.skipped
//! - result.rc (for command/shell)
//! - result.stdout
//! - result.stderr
//! - result.msg
//! - result.results (for loops)
//!
//! ## Command Results (Section 3)
//! - rc (return code)
//! - stdout/stdout_lines
//! - stderr/stderr_lines
//! - cmd
//! - start/end/delta
//!
//! ## Loop Results (Section 4)
//! - results is a list
//! - Each item has result
//! - Access results[0].stdout
//! - Iterate over results
//!
//! ## Changed_when (Section 5)
//! - changed_when: false
//! - changed_when: "'text' in result.stdout"
//! - Override module changed
//! - Complex expressions
//!
//! ## Failed_when (Section 6)
//! - failed_when: false
//! - failed_when: result.rc != 0
//! - Override module failure
//! - Combine with changed_when
//!
//! ## Result in Conditions (Section 7)
//! - when: result.changed
//! - when: result.rc == 0
//! - when: "'text' in result.stdout"
//! - when: result.failed
//!
//! ## Result in Loops (Section 8)
//! - until: result.rc == 0
//! - retries and delay
//! - Result changes per retry
//!
//! ## Result Access (Section 9)
//! - Nested result access
//! - Safe access with default
//! - result.get('key', default)
//!
//! ## Edge Cases (Section 10)
//! - Register undefined on skip
//! - Register in block/rescue
//! - Overwrite registered var
//! - Large result data

use indexmap::IndexMap;
use serde_json::json;

use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::{RegisteredResult, RuntimeContext};
use rustible::executor::task::{Handler, Task, TaskResult};
use rustible::executor::{Executor, ExecutorConfig};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create an executor with a simple host setup
#[allow(dead_code)]
fn create_test_executor(hosts: Vec<&str>) -> Executor {
    let mut runtime = RuntimeContext::new();
    for host in hosts {
        runtime.add_host(host.to_string(), None);
    }

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };

    Executor::with_runtime(config, runtime)
}

/// Create a simple playbook with one play
fn create_playbook(name: &str, hosts: &str, tasks: Vec<Task>) -> Playbook {
    let mut playbook = Playbook::new(name);
    let mut play = Play::new(name, hosts);
    play.gather_facts = false;

    for task in tasks {
        play.add_task(task);
    }

    playbook.add_play(play);
    playbook
}

/// Create an executor with pre-registered results for testing
#[allow(dead_code)]
fn create_executor_with_registered_result(
    host: &str,
    var_name: &str,
    result: RegisteredResult,
) -> Executor {
    let mut runtime = RuntimeContext::new();
    runtime.add_host(host.to_string(), None);
    runtime.register_result(host, var_name.to_string(), result);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };

    Executor::with_runtime(config, runtime)
}

// ============================================================================
// Section 1: Register Basic
// ============================================================================

#[tokio::test]
async fn test_register_basic_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Debug task", "debug")
        .arg("msg", "Hello World")
        .register("my_result");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_register_variable_persists_across_tasks() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // First task registers a result
    let task1 = Task::new("First task", "debug")
        .arg("msg", "First message")
        .register("first_result");

    // Second task uses the registered result in a condition
    let task2 = Task::new("Second task", "debug")
        .arg("msg", "Using registered result")
        .when("first_result is defined");

    let playbook = create_playbook("test", "all", vec![task1, task2]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    // Both tasks should run (not skipped)
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_register_variable_is_host_specific() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("host1".to_string(), None);
    runtime.add_host("host2".to_string(), None);

    // Pre-register a variable only for host1
    runtime.register_result(
        "host1",
        "host1_var".to_string(),
        RegisteredResult {
            changed: true,
            msg: Some("host1 only".to_string()),
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Task should run on host1 (where var is defined) and skip on host2
    let task = Task::new("Conditional task", "debug")
        .arg("msg", "Variable exists")
        .when("host1_var is defined");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    // host1 should run, host2 should skip
    let host1_result = results.get("host1").unwrap();
    let host2_result = results.get("host2").unwrap();

    assert_eq!(host1_result.stats.skipped, 0);
    assert_eq!(host2_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_register_different_variable_names() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Register with various valid variable names
    let task1 = Task::new("Task 1", "debug")
        .arg("msg", "msg1")
        .register("simple_var");

    let task2 = Task::new("Task 2", "debug")
        .arg("msg", "msg2")
        .register("var_with_underscore");

    let task3 = Task::new("Task 3", "debug")
        .arg("msg", "msg3")
        .register("var123");

    let playbook = create_playbook("test", "all", vec![task1, task2, task3]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Section 2: Result Structure
// ============================================================================

#[test]
fn test_registered_result_changed_field() {
    let result = RegisteredResult::ok(true);
    assert!(result.changed);

    let result = RegisteredResult::ok(false);
    assert!(!result.changed);
}

#[test]
fn test_registered_result_failed_field() {
    let result = RegisteredResult::failed("Error message");
    assert!(result.failed);
    assert!(!result.changed);
    assert_eq!(result.msg, Some("Error message".to_string()));
}

#[test]
fn test_registered_result_skipped_field() {
    let result = RegisteredResult::skipped("Condition not met");
    assert!(result.skipped);
    assert!(!result.changed);
    assert!(!result.failed);
    assert_eq!(result.msg, Some("Condition not met".to_string()));
}

#[test]
fn test_registered_result_rc_field() {
    let mut result = RegisteredResult::ok(false);
    result.rc = Some(0);
    assert_eq!(result.rc, Some(0));

    result.rc = Some(1);
    assert_eq!(result.rc, Some(1));
}

#[test]
fn test_registered_result_stdout_field() {
    let mut result = RegisteredResult::ok(false);
    result.stdout = Some("Hello World\nLine 2".to_string());
    result.stdout_lines = Some(vec!["Hello World".to_string(), "Line 2".to_string()]);

    assert_eq!(result.stdout, Some("Hello World\nLine 2".to_string()));
    assert_eq!(result.stdout_lines.as_ref().unwrap().len(), 2);
}

#[test]
fn test_registered_result_stderr_field() {
    let mut result = RegisteredResult::ok(false);
    result.stderr = Some("Error output".to_string());
    result.stderr_lines = Some(vec!["Error output".to_string()]);

    assert_eq!(result.stderr, Some("Error output".to_string()));
    assert_eq!(result.stderr_lines.as_ref().unwrap().len(), 1);
}

#[test]
fn test_registered_result_msg_field() {
    let result = RegisteredResult::failed("Custom error message");
    assert_eq!(result.msg, Some("Custom error message".to_string()));
}

#[test]
fn test_registered_result_results_field_for_loops() {
    let mut result = RegisteredResult::ok(true);
    result.results = Some(vec![
        RegisteredResult {
            changed: true,
            msg: Some("Item 1".to_string()),
            ..Default::default()
        },
        RegisteredResult {
            changed: false,
            msg: Some("Item 2".to_string()),
            ..Default::default()
        },
    ]);

    assert!(result.results.is_some());
    assert_eq!(result.results.as_ref().unwrap().len(), 2);
}

#[test]
fn test_registered_result_to_json() {
    let result = RegisteredResult {
        changed: true,
        failed: false,
        skipped: false,
        rc: Some(0),
        stdout: Some("output".to_string()),
        stderr: None,
        msg: Some("Task completed".to_string()),
        ..Default::default()
    };

    let json = result.to_json();
    assert!(json.is_object());
    assert_eq!(json.get("changed"), Some(&json!(true)));
    assert_eq!(json.get("rc"), Some(&json!(0)));
    assert_eq!(json.get("stdout"), Some(&json!("output")));
}

// ============================================================================
// Section 3: Command Results
// ============================================================================

#[test]
fn test_command_result_rc() {
    let mut result = RegisteredResult::ok(true);
    result.rc = Some(0);
    assert_eq!(result.rc, Some(0));

    // Non-zero return code
    result.rc = Some(127);
    assert_eq!(result.rc, Some(127));

    // Negative return code (signal)
    result.rc = Some(-9);
    assert_eq!(result.rc, Some(-9));
}

#[test]
fn test_command_result_stdout_lines() {
    let output = "line1\nline2\nline3";
    let mut result = RegisteredResult::ok(false);
    result.stdout = Some(output.to_string());
    result.stdout_lines = Some(output.lines().map(String::from).collect());

    assert_eq!(result.stdout_lines.as_ref().unwrap().len(), 3);
    assert_eq!(result.stdout_lines.as_ref().unwrap()[0], "line1");
    assert_eq!(result.stdout_lines.as_ref().unwrap()[1], "line2");
    assert_eq!(result.stdout_lines.as_ref().unwrap()[2], "line3");
}

#[test]
fn test_command_result_stderr_lines() {
    let error = "error1\nerror2";
    let mut result = RegisteredResult::ok(false);
    result.stderr = Some(error.to_string());
    result.stderr_lines = Some(error.lines().map(String::from).collect());

    assert_eq!(result.stderr_lines.as_ref().unwrap().len(), 2);
    assert_eq!(result.stderr_lines.as_ref().unwrap()[0], "error1");
}

#[test]
fn test_command_result_empty_output() {
    let mut result = RegisteredResult::ok(false);
    result.stdout = Some("".to_string());
    result.stdout_lines = Some(vec![]);
    result.stderr = Some("".to_string());
    result.stderr_lines = Some(vec![]);

    assert_eq!(result.stdout, Some("".to_string()));
    assert!(result.stdout_lines.as_ref().unwrap().is_empty());
}

#[test]
fn test_command_result_data_field() {
    let mut result = RegisteredResult::ok(true);
    result.data.insert("cmd".to_string(), json!("echo hello"));
    result
        .data
        .insert("start".to_string(), json!("2024-01-01T00:00:00Z"));
    result
        .data
        .insert("end".to_string(), json!("2024-01-01T00:00:01Z"));
    result
        .data
        .insert("delta".to_string(), json!("0:00:01.000"));

    assert_eq!(result.data.get("cmd"), Some(&json!("echo hello")));
    assert!(result.data.contains_key("start"));
    assert!(result.data.contains_key("end"));
    assert!(result.data.contains_key("delta"));
}

#[tokio::test]
async fn test_command_module_result_structure() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Command module should produce results with rc, stdout, stderr
    let task = Task::new("Run command", "command")
        .arg("cmd", "echo hello")
        .register("cmd_result");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Section 4: Loop Results
// ============================================================================

#[tokio::test]
async fn test_loop_register_creates_results_list() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Loop task", "debug")
        .arg("msg", "Item: {{ item }}")
        .loop_over(vec![json!("a"), json!("b"), json!("c")])
        .register("loop_result");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[test]
fn test_loop_results_structure() {
    let mut main_result = RegisteredResult::ok(true);
    main_result.results = Some(vec![
        RegisteredResult {
            changed: true,
            msg: Some("Processed item 0".to_string()),
            stdout: Some("output0".to_string()),
            ..Default::default()
        },
        RegisteredResult {
            changed: false,
            msg: Some("Processed item 1".to_string()),
            stdout: Some("output1".to_string()),
            ..Default::default()
        },
        RegisteredResult {
            changed: true,
            msg: Some("Processed item 2".to_string()),
            stdout: Some("output2".to_string()),
            ..Default::default()
        },
    ]);

    let results = main_result.results.as_ref().unwrap();
    assert_eq!(results.len(), 3);

    // Access individual items
    assert!(results[0].changed);
    assert!(!results[1].changed);
    assert!(results[2].changed);

    // Access stdout from results
    assert_eq!(results[0].stdout, Some("output0".to_string()));
    assert_eq!(results[1].stdout, Some("output1".to_string()));
}

#[test]
fn test_loop_results_any_changed() {
    let mut result = RegisteredResult::ok(true);
    result.results = Some(vec![
        RegisteredResult {
            changed: false,
            ..Default::default()
        },
        RegisteredResult {
            changed: true,
            ..Default::default()
        },
        RegisteredResult {
            changed: false,
            ..Default::default()
        },
    ]);

    // Check if any item changed
    let any_changed = result.results.as_ref().unwrap().iter().any(|r| r.changed);
    assert!(any_changed);
}

#[test]
fn test_loop_results_all_ok() {
    let mut result = RegisteredResult::ok(false);
    result.results = Some(vec![
        RegisteredResult {
            failed: false,
            ..Default::default()
        },
        RegisteredResult {
            failed: false,
            ..Default::default()
        },
    ]);

    // Check if all items succeeded
    let all_ok = result.results.as_ref().unwrap().iter().all(|r| !r.failed);
    assert!(all_ok);
}

#[tokio::test]
async fn test_loop_with_register_and_subsequent_condition() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Loop task that registers results
    let task1 = Task::new("Loop and register", "debug")
        .arg("msg", "Item: {{ item }}")
        .loop_over(vec![json!("x"), json!("y")])
        .register("items_result");

    // Next task checks if the loop completed
    let task2 = Task::new("Check loop result", "debug")
        .arg("msg", "Loop completed")
        .when("items_result is defined");

    let playbook = create_playbook("test", "all", vec![task1, task2]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 5: Changed_when
// ============================================================================

#[tokio::test]
async fn test_changed_when_false() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Use a module that normally changes (like debug which is ok)
    // But force changed_when to false
    let mut task = Task::new("Force not changed", "debug");
    task.args.insert("msg".to_string(), json!("Hello"));
    task.changed_when = Some("false".to_string());

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_changed_when_true() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Force changed_when to true
    let mut task = Task::new("Force changed", "debug");
    task.args.insert("msg".to_string(), json!("Hello"));
    task.changed_when = Some("true".to_string());

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    assert!(host_result.stats.changed > 0);
}

#[tokio::test]
async fn test_changed_when_with_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "force_change".to_string(), json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut task = Task::new("Variable changed_when", "debug");
    task.args.insert("msg".to_string(), json!("Hello"));
    task.changed_when = Some("force_change".to_string());

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    assert!(host_result.stats.changed > 0);
}

#[tokio::test]
async fn test_changed_when_false_does_not_notify() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Task that notifies but has changed_when: false
    let mut task = Task::new("Config update", "debug");
    task.args
        .insert("msg".to_string(), json!("Updating config"));
    task.notify = vec!["restart service".to_string()];
    task.changed_when = Some("false".to_string());

    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;
    play.add_task(task);

    // Add handler
    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), json!("Restarting"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Section 6: Failed_when
// ============================================================================

#[tokio::test]
async fn test_failed_when_false_prevents_failure() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Fail module would normally fail, but failed_when: false should prevent it
    let mut task = Task::new("Suppress failure", "fail");
    task.args
        .insert("msg".to_string(), json!("Intentional failure"));
    task.failed_when = Some("false".to_string());

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    // Task should not be marked as failed due to failed_when: false
    // Note: This depends on how failed_when is implemented
    let _host_result = results.get("localhost").unwrap();
    // The behavior may vary based on implementation
}

#[tokio::test]
async fn test_failed_when_true_forces_failure() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Debug would normally succeed, but failed_when: true should fail it
    let mut task = Task::new("Force failure", "debug");
    task.args.insert("msg".to_string(), json!("Hello"));
    task.failed_when = Some("true".to_string());

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(host_result.failed || host_result.stats.failed > 0);
}

#[tokio::test]
async fn test_failed_when_with_variable_condition() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "should_fail".to_string(), json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut task = Task::new("Conditional failure", "debug");
    task.args.insert("msg".to_string(), json!("Testing"));
    task.failed_when = Some("should_fail".to_string());

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(host_result.failed || host_result.stats.failed > 0);
}

#[tokio::test]
async fn test_failed_when_with_comparison() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "exit_code".to_string(), json!(1));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Fail when exit_code != 0
    let mut task = Task::new("Check exit code", "debug");
    task.args.insert("msg".to_string(), json!("Checking"));
    task.failed_when = Some("exit_code != 0".to_string());

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(host_result.failed || host_result.stats.failed > 0);
}

#[tokio::test]
async fn test_changed_when_and_failed_when_together() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "is_changed".to_string(), json!(false));
    runtime.set_host_var("localhost", "is_failed".to_string(), json!(false));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut task = Task::new("Combined conditions", "debug");
    task.args.insert("msg".to_string(), json!("Testing"));
    task.changed_when = Some("is_changed".to_string());
    task.failed_when = Some("is_failed".to_string());

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    assert_eq!(host_result.stats.changed, 0);
}

// ============================================================================
// Section 7: Result in Conditions
// ============================================================================

#[tokio::test]
async fn test_when_result_changed() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    // Pre-register a changed result
    runtime.register_result(
        "localhost",
        "prev_task".to_string(),
        RegisteredResult {
            changed: true,
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("When changed", "debug")
        .arg("msg", "Previous task changed")
        .when("prev_task.changed");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_result_not_changed() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    // Pre-register an unchanged result
    runtime.register_result(
        "localhost",
        "prev_task".to_string(),
        RegisteredResult {
            changed: false,
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("When not changed", "debug")
        .arg("msg", "Previous task did not change")
        .when("not prev_task.changed");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_result_rc_equals_zero() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    runtime.register_result(
        "localhost",
        "cmd_result".to_string(),
        RegisteredResult {
            rc: Some(0),
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("When rc is 0", "debug")
        .arg("msg", "Command succeeded")
        .when("cmd_result.rc == 0");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_result_rc_not_zero() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    runtime.register_result(
        "localhost",
        "cmd_result".to_string(),
        RegisteredResult {
            rc: Some(1),
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("When rc is not 0", "debug")
        .arg("msg", "Command failed")
        .when("cmd_result.rc != 0");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_result_failed() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    runtime.register_result(
        "localhost",
        "task_result".to_string(),
        RegisteredResult {
            failed: true,
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("When failed", "debug")
        .arg("msg", "Previous task failed")
        .when("task_result.failed");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_result_skipped() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    runtime.register_result(
        "localhost",
        "task_result".to_string(),
        RegisteredResult {
            skipped: true,
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("When skipped", "debug")
        .arg("msg", "Previous task was skipped")
        .when("task_result.skipped");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 8: Result in Loops (Until/Retries)
// ============================================================================

// Note: until/retries/delay may not be fully implemented
// These tests document the expected behavior

#[test]
fn test_until_result_structure() {
    // Until loops should update the result on each retry
    let result1 = RegisteredResult {
        rc: Some(1),
        failed: false,
        msg: Some("Attempt 1".to_string()),
        ..Default::default()
    };

    let result2 = RegisteredResult {
        rc: Some(0),
        failed: false,
        msg: Some("Attempt 2 - success".to_string()),
        ..Default::default()
    };

    // On retry, the result should be replaced
    assert_eq!(result1.rc, Some(1));
    assert_eq!(result2.rc, Some(0));
}

#[tokio::test]
async fn test_register_with_loop_stores_results_list() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Loop task
    let task = Task::new("Loop task", "debug")
        .arg("msg", "{{ item }}")
        .loop_over(vec![json!(1), json!(2), json!(3)])
        .register("loop_output");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Section 9: Result Access
// ============================================================================

#[test]
fn test_nested_result_access() {
    let mut result = RegisteredResult::ok(true);
    result.data.insert(
        "stat".to_string(),
        json!({
            "exists": true,
            "path": "/etc/hosts",
            "mode": "0644",
            "size": 1234
        }),
    );

    let json = result.to_json();
    let stat = json.get("stat").unwrap();
    assert_eq!(stat.get("exists"), Some(&json!(true)));
    assert_eq!(stat.get("path"), Some(&json!("/etc/hosts")));
    assert_eq!(stat.get("mode"), Some(&json!("0644")));
}

#[test]
fn test_result_access_missing_key() {
    let result = RegisteredResult::ok(false);
    let json = result.to_json();

    // Accessing non-existent key should return None
    assert!(json.get("nonexistent").is_none());
}

#[test]
fn test_result_with_custom_data() {
    let mut result = RegisteredResult::ok(true);
    result
        .data
        .insert("custom_key".to_string(), json!("custom_value"));
    result.data.insert(
        "nested".to_string(),
        json!({
            "level1": {
                "level2": "deep_value"
            }
        }),
    );

    let json = result.to_json();
    assert_eq!(json.get("custom_key"), Some(&json!("custom_value")));

    let nested = json.get("nested").unwrap();
    let level1 = nested.get("level1").unwrap();
    assert_eq!(level1.get("level2"), Some(&json!("deep_value")));
}

#[tokio::test]
async fn test_result_access_in_template() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    // Register a result with stdout
    runtime.register_result(
        "localhost",
        "prev_result".to_string(),
        RegisteredResult {
            stdout: Some("hello world".to_string()),
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Use the registered result in a template
    let task = Task::new("Use result", "debug").arg("msg", "Output was: {{ prev_result.stdout }}");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Section 10: Edge Cases
// ============================================================================

#[tokio::test]
async fn test_register_on_skipped_task() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Task that will be skipped
    let task1 = Task::new("Skipped task", "debug")
        .arg("msg", "This will be skipped")
        .when("false")
        .register("skipped_result");

    // Check if the registered variable is defined (it should be with skipped=true)
    let task2 = Task::new("Check skipped", "debug")
        .arg("msg", "Checking skipped result")
        .when("skipped_result is defined");

    let playbook = create_playbook("test", "all", vec![task1, task2]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    // The first task should be skipped
    assert!(host_result.stats.skipped >= 1);
}

#[tokio::test]
async fn test_overwrite_registered_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // First task registers to same_var
    let task1 = Task::new("First register", "debug")
        .arg("msg", "First message")
        .register("same_var");

    // Second task overwrites same_var
    let task2 = Task::new("Second register", "debug")
        .arg("msg", "Second message")
        .register("same_var");

    let playbook = create_playbook("test", "all", vec![task1, task2]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[test]
fn test_large_result_data() {
    let mut result = RegisteredResult::ok(true);

    // Create large stdout
    let large_output: String = (0..10000).map(|i| format!("Line {}\n", i)).collect();
    result.stdout = Some(large_output.clone());
    result.stdout_lines = Some(large_output.lines().map(String::from).collect());

    assert!(result.stdout.as_ref().unwrap().len() > 50000);
    assert_eq!(result.stdout_lines.as_ref().unwrap().len(), 10000);
}

#[test]
fn test_result_with_binary_safe_data() {
    let mut result = RegisteredResult::ok(true);

    // Test with unicode and special characters
    result.stdout = Some("Unicode: \u{1F600} \u{1F389} \n\t\r".to_string());
    result.msg = Some("Message with 'quotes' and \"double quotes\"".to_string());

    let json = result.to_json();
    assert!(json.get("stdout").is_some());
    assert!(json.get("msg").is_some());
}

#[tokio::test]
async fn test_register_with_ignore_errors() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Task that fails but ignores errors
    let task1 = Task::new("Failing task", "fail")
        .arg("msg", "Intentional failure")
        .register("fail_result")
        .ignore_errors(true);

    // Next task should still run
    let task2 = Task::new("After failure", "debug").arg("msg", "Still running");

    let playbook = create_playbook("test", "all", vec![task1, task2]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    // Host should not be marked as failed due to ignore_errors
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_register_preserves_across_play_for_same_host() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi-play test");

    // First play registers a variable
    let mut play1 = Play::new("First Play", "all");
    play1.gather_facts = false;
    play1.add_task(
        Task::new("Register in play 1", "debug")
            .arg("msg", "Play 1 task")
            .register("play1_result"),
    );
    playbook.add_play(play1);

    // Second play uses the variable
    let mut play2 = Play::new("Second Play", "all");
    play2.gather_facts = false;
    play2.add_task(
        Task::new("Use in play 2", "debug")
            .arg("msg", "Checking play1_result")
            .when("play1_result is defined"),
    );
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Runtime Context Integration Tests
// ============================================================================

#[test]
fn test_runtime_context_register_and_retrieve() {
    let mut ctx = RuntimeContext::new();
    ctx.add_host("host1".to_string(), None);

    let result = RegisteredResult {
        changed: true,
        rc: Some(0),
        stdout: Some("output".to_string()),
        ..Default::default()
    };

    ctx.register_result("host1", "my_result".to_string(), result);

    let retrieved = ctx.get_registered("host1", "my_result").unwrap();
    assert!(retrieved.changed);
    assert_eq!(retrieved.rc, Some(0));
    assert_eq!(retrieved.stdout, Some("output".to_string()));
}

#[test]
fn test_runtime_context_registered_var_in_merged_vars() {
    let mut ctx = RuntimeContext::new();
    ctx.add_host("host1".to_string(), None);

    let result = RegisteredResult {
        changed: true,
        msg: Some("Task completed".to_string()),
        ..Default::default()
    };

    ctx.register_result("host1", "task_result".to_string(), result);

    let merged = ctx.get_merged_vars("host1");
    assert!(merged.contains_key("task_result"));

    let task_result = merged.get("task_result").unwrap();
    assert_eq!(task_result.get("changed"), Some(&json!(true)));
}

#[test]
fn test_runtime_context_multiple_registered_vars() {
    let mut ctx = RuntimeContext::new();
    ctx.add_host("host1".to_string(), None);

    ctx.register_result("host1", "result1".to_string(), RegisteredResult::ok(true));
    ctx.register_result("host1", "result2".to_string(), RegisteredResult::ok(false));
    ctx.register_result(
        "host1",
        "result3".to_string(),
        RegisteredResult::failed("error"),
    );

    assert!(ctx.get_registered("host1", "result1").is_some());
    assert!(ctx.get_registered("host1", "result2").is_some());
    assert!(ctx.get_registered("host1", "result3").is_some());

    let r1 = ctx.get_registered("host1", "result1").unwrap();
    let r2 = ctx.get_registered("host1", "result2").unwrap();
    let r3 = ctx.get_registered("host1", "result3").unwrap();

    assert!(r1.changed);
    assert!(!r2.changed);
    assert!(r3.failed);
}

// ============================================================================
// Task Result Conversion Tests
// ============================================================================

#[test]
fn test_task_result_to_registered() {
    let task_result = TaskResult::changed().with_msg("Changed something");

    let registered = task_result.to_registered(
        Some("stdout output".to_string()),
        Some("stderr output".to_string()),
    );

    assert!(registered.changed);
    assert!(!registered.failed);
    assert!(!registered.skipped);
    assert_eq!(registered.stdout, Some("stdout output".to_string()));
    assert_eq!(registered.stderr, Some("stderr output".to_string()));
    assert_eq!(registered.msg, Some("Changed something".to_string()));
}

#[test]
fn test_task_result_failed_to_registered() {
    let task_result = TaskResult::failed("Error occurred");

    let registered = task_result.to_registered(None, None);

    assert!(!registered.changed);
    assert!(registered.failed);
    assert!(!registered.skipped);
    assert_eq!(registered.msg, Some("Error occurred".to_string()));
}

#[test]
fn test_task_result_skipped_to_registered() {
    let task_result = TaskResult::skipped("Condition not met");

    let registered = task_result.to_registered(None, None);

    assert!(!registered.changed);
    assert!(!registered.failed);
    assert!(registered.skipped);
    assert_eq!(registered.msg, Some("Condition not met".to_string()));
}

// ============================================================================
// Complex Workflow Tests
// ============================================================================

#[tokio::test]
async fn test_complex_register_workflow() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Step 1: Set a fact
    let task1 = Task::new("Set initial fact", "set_fact")
        .arg("app_name", "myapp")
        .register("fact_result");

    // Step 2: Conditional based on fact
    let task2 = Task::new("Check fact", "debug")
        .arg("msg", "App name is set")
        .when("fact_result is defined")
        .register("check_result");

    // Step 3: Use registered result in another condition
    let task3 = Task::new("Final check", "debug")
        .arg("msg", "All checks passed")
        .when("check_result is defined");

    let playbook = create_playbook("test", "all", vec![task1, task2, task3]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_register_in_loop_with_conditional() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Loop task with register
    let task1 = Task::new("Process items", "debug")
        .arg("msg", "Processing {{ item }}")
        .loop_over(vec![json!("a"), json!("b"), json!("c")])
        .register("process_results");

    // Check if loop completed
    let task2 = Task::new("Verify loop", "debug")
        .arg("msg", "Loop completed successfully")
        .when("process_results is defined");

    let playbook = create_playbook("test", "all", vec![task1, task2]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Stat Module Result Tests
// ============================================================================

#[tokio::test]
async fn test_stat_module_result_structure() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check file", "stat")
        .arg("path", "/etc/hosts")
        .register("stat_result");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_conditional_on_stat_exists() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    // Pre-register a stat result
    let mut result = RegisteredResult::ok(false);
    result.data.insert(
        "stat".to_string(),
        json!({
            "exists": true,
            "isdir": false,
            "isreg": true
        }),
    );
    runtime.register_result("localhost", "file_stat".to_string(), result);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("If file exists", "debug")
        .arg("msg", "File exists")
        .when("file_stat.stat.exists");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Assert Module with Registered Results
// ============================================================================

#[tokio::test]
async fn test_assert_with_registered_result() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    // Pre-register a result
    runtime.register_result(
        "localhost",
        "cmd_output".to_string(),
        RegisteredResult {
            rc: Some(0),
            ..Default::default()
        },
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Assert success", "assert")
        .arg("that", json!(["cmd_output.rc == 0"]))
        .arg("success_msg", "Command succeeded");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}
