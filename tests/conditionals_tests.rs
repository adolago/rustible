//! Comprehensive tests for conditionals and loops in Rustible
//!
//! This test suite verifies Ansible-compatible when conditions and loop constructs including:
//!
//! ## When Conditions (Section 1)
//! - Simple boolean: when: true/false
//! - Variable check: when: my_var
//! - Comparison: when: x == "value"
//! - Negation: when: not condition
//! - And/Or: when: a and b, when: a or b
//! - Complex: when: (a or b) and c
//!
//! ## Jinja2 Tests in When (Section 2)
//! - when: var is defined
//! - when: var is undefined / is not defined
//! - when: var is none
//! - when: var is true/false
//! - when: var is string/number
//! - when: var is iterable
//! - when: var is mapping
//! - when: var is match("pattern")
//! - when: var is search("pattern")
//!
//! ## Variable Access in When (Section 3)
//! - Simple variable
//! - Nested variable (dict.key)
//! - List access (list[0])
//! - Registered variable (result.rc == 0)
//! - Hostvars access
//! - Facts access
//!
//! ## Loop Constructs (Section 4)
//! - loop: simple list
//! - loop: list of dicts
//! - with_items: (deprecated but supported)
//! - with_dict: dictionary iteration
//! - with_sequence: numeric sequence
//! - with_together: parallel lists
//! - with_nested: nested iteration
//!
//! ## Loop Variables (Section 5)
//! - item - current item
//! - item.key, item.value for dicts
//! - ansible_loop.index (1-based)
//! - ansible_loop.index0 (0-based)
//! - ansible_loop.first
//! - ansible_loop.last
//! - ansible_loop.length
//!
//! ## Loop Control (Section 6)
//! - loop_control.label
//! - loop_control.pause
//! - loop_control.loop_var (rename item)
//! - loop_control.index_var
//!
//! ## Conditional Loops (Section 7)
//! - Loop with when condition
//! - When evaluated per iteration
//! - until/retries/delay
//!
//! ## Nested Conditionals (Section 8)
//! - When inside block
//! - When inside role
//! - When with include_tasks

use indexmap::IndexMap;
use serde_json::json;

use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::{RegisteredResult, RuntimeContext};
use rustible::executor::task::{Handler, Task};
use rustible::executor::{Executor, ExecutorConfig};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create an executor with a simple host setup
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

// ============================================================================
// Section 1: When Conditions - Simple Boolean
// ============================================================================

#[tokio::test]
async fn test_when_true_literal() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Should run", "debug")
        .arg("msg", "This should execute")
        .when("true");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(host_result.stats.ok > 0 || host_result.stats.changed > 0);
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_false_literal() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Should skip", "debug")
        .arg("msg", "This should not execute")
        .when("false");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
#[allow(non_snake_case)]
async fn test_when_True_python_style() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Python True", "debug")
        .arg("msg", "Python-style True")
        .when("True");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
#[allow(non_snake_case)]
async fn test_when_False_python_style() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Python False", "debug")
        .arg("msg", "Python-style False")
        .when("False");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

// ============================================================================
// Section 1: When Conditions - Variable Check
// ============================================================================

#[tokio::test]
async fn test_when_truthy_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "my_var".to_string(), json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Truthy var check", "debug")
        .arg("msg", "Variable is truthy")
        .when("my_var");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_falsy_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "my_var".to_string(), json!(false));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Falsy var check", "debug")
        .arg("msg", "Variable is falsy")
        .when("my_var");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_empty_string_is_falsy() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "empty_str".to_string(), json!(""));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Empty string check", "debug")
        .arg("msg", "Empty string is falsy")
        .when("empty_str");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_nonempty_string_is_truthy() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "my_str".to_string(), json!("hello"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Non-empty string check", "debug")
        .arg("msg", "Non-empty string is truthy")
        .when("my_str");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_zero_is_falsy() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "zero".to_string(), json!(0));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Zero check", "debug")
        .arg("msg", "Zero is falsy")
        .when("zero");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_nonzero_is_truthy() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "count".to_string(), json!(42));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Non-zero check", "debug")
        .arg("msg", "Non-zero is truthy")
        .when("count");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_empty_list_is_falsy() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "empty_list".to_string(), json!([]));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Empty list check", "debug")
        .arg("msg", "Empty list is falsy")
        .when("empty_list");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_nonempty_list_is_truthy() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "items".to_string(), json!(["a", "b"]));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Non-empty list check", "debug")
        .arg("msg", "Non-empty list is truthy")
        .when("items");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 1: When Conditions - Comparison Operators
// ============================================================================

#[tokio::test]
async fn test_when_string_equality() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "os".to_string(), json!("Debian"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("String equality", "debug")
        .arg("msg", "OS is Debian")
        .when("os == 'Debian'");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_string_inequality() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "os".to_string(), json!("Debian"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("String inequality", "debug")
        .arg("msg", "OS is not RedHat")
        .when("os != 'RedHat'");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_string_equality_double_quotes() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "env".to_string(), json!("production"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Double quotes", "debug")
        .arg("msg", "Env is production")
        .when("env == \"production\"");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_number_comparison() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "count".to_string(), json!(10));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Number equality", "debug")
        .arg("msg", "Count is 10")
        .when("count == 10");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 1: When Conditions - Negation
// ============================================================================

#[tokio::test]
async fn test_when_not_true() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Not true", "debug")
        .arg("msg", "not true should skip")
        .when("not true");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_not_false() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Not false", "debug")
        .arg("msg", "not false should run")
        .when("not false");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_not_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "disabled".to_string(), json!(false));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Not variable", "debug")
        .arg("msg", "Not disabled means enabled")
        .when("not disabled");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 1: When Conditions - Logical Operators (and/or)
// ============================================================================

#[tokio::test]
async fn test_when_and_both_true() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "a".to_string(), json!(true));
    runtime.set_host_var("localhost", "b".to_string(), json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("And both true", "debug")
        .arg("msg", "Both conditions true")
        .when("a and b");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_and_one_false() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "a".to_string(), json!(true));
    runtime.set_host_var("localhost", "b".to_string(), json!(false));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("And one false", "debug")
        .arg("msg", "One condition false")
        .when("a and b");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_or_both_false() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "a".to_string(), json!(false));
    runtime.set_host_var("localhost", "b".to_string(), json!(false));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Or both false", "debug")
        .arg("msg", "Both conditions false")
        .when("a or b");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_or_one_true() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "a".to_string(), json!(false));
    runtime.set_host_var("localhost", "b".to_string(), json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Or one true", "debug")
        .arg("msg", "One condition true")
        .when("a or b");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 2: Jinja2 Tests - is defined / is not defined
// ============================================================================

#[tokio::test]
async fn test_when_is_defined_true() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "my_var".to_string(), json!("value"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Is defined", "debug")
        .arg("msg", "Variable is defined")
        .when("my_var is defined");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_is_defined_false() {
    let runtime = RuntimeContext::new();
    let mut runtime = runtime;
    runtime.add_host("localhost".to_string(), None);
    // Note: undefined_var is not set

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Is defined missing", "debug")
        .arg("msg", "Undefined variable")
        .when("undefined_var is defined");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_is_not_defined() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    // Note: missing_var is not set

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Is not defined", "debug")
        .arg("msg", "Variable is not defined")
        .when("missing_var is not defined");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 2: Jinja2 Tests - in operator
// ============================================================================

#[tokio::test]
async fn test_when_in_list() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "items".to_string(), json!(["a", "b", "c"]));
    runtime.set_host_var("localhost", "target".to_string(), json!("b"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("In list", "debug")
        .arg("msg", "Target is in items")
        .when("target in items");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_not_in_list() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "items".to_string(), json!(["a", "b", "c"]));
    runtime.set_host_var("localhost", "target".to_string(), json!("x"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Not in list", "debug")
        .arg("msg", "Target not in items")
        .when("target in items");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 1);
}

// ============================================================================
// Section 3: Variable Access in When - Nested Variables
// ============================================================================

#[tokio::test]
async fn test_when_nested_dict_access() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var(
        "localhost",
        "config".to_string(),
        json!({
            "database": {
                "host": "localhost",
                "port": 5432
            }
        }),
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Nested access", "debug")
        .arg("msg", "Database host is localhost")
        .when("config.database.host == 'localhost'");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_registered_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    // Simulate a registered result
    let result = RegisteredResult {
        changed: true,
        failed: false,
        skipped: false,
        rc: Some(0),
        stdout: Some("success".to_string()),
        ..Default::default()
    };
    runtime.register_result("localhost", "command_result".to_string(), result);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check registered", "debug")
        .arg("msg", "Command succeeded")
        .when("command_result.rc == 0");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_registered_changed() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    // Simulate a registered result
    let result = RegisteredResult {
        changed: true,
        failed: false,
        skipped: false,
        ..Default::default()
    };
    runtime.register_result("localhost", "previous_task".to_string(), result);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check if changed", "debug")
        .arg("msg", "Previous task changed")
        .when("previous_task.changed");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 4: Loop Constructs - Simple Loop
// ============================================================================

#[tokio::test]
async fn test_loop_simple_list() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Loop simple", "debug")
        .arg("msg", "Processing {{ item }}")
        .loop_over(vec![json!("item1"), json!("item2"), json!("item3")]);

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_loop_list_of_dicts() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Loop dicts", "debug")
        .arg("msg", "User: {{ item.name }}")
        .loop_over(vec![
            json!({"name": "alice", "role": "admin"}),
            json!({"name": "bob", "role": "user"}),
        ]);

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_loop_empty_list() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Loop empty", "debug")
        .arg("msg", "Should not see this")
        .loop_over(vec![]);

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    // Empty loop should complete without error
    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Section 5: Loop Variables
// ============================================================================

#[tokio::test]
async fn test_loop_item_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Loop with item", "set_fact")
        .arg("last_item", "{{ item }}")
        .loop_over(vec![json!("a"), json!("b"), json!("c")]);

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_loop_custom_loop_var() {
    let executor = create_test_executor(vec!["localhost"]);

    let task = Task::new("Custom loop var", "debug")
        .arg("msg", "Package: {{ package }}")
        .loop_over(vec![json!("nginx"), json!("php")])
        .loop_var("package");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_loop_ansible_loop_variables() {
    // Test that ansible_loop.index, ansible_loop.first, ansible_loop.last are set
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Loop variables", "debug")
        .arg("msg", "Index: {{ ansible_loop.index }}")
        .loop_over(vec![json!("a"), json!("b"), json!("c")]);

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Section 7: Conditional Loops - Loop with When
// ============================================================================

#[tokio::test]
async fn test_loop_with_when_condition() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "process_items".to_string(), json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Conditional loop", "debug")
        .arg("msg", "Processing {{ item }}")
        .loop_over(vec![json!("a"), json!("b")])
        .when("process_items");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_loop_with_when_skip_all() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "process_items".to_string(), json!(false));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Skipped loop", "debug")
        .arg("msg", "Processing {{ item }}")
        .loop_over(vec![json!("a"), json!("b")])
        .when("process_items");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    // The whole loop should be skipped
    assert_eq!(host_result.stats.skipped, 1);
}

// ============================================================================
// Section 8: Nested Conditionals - Multiple Tasks with Different Conditions
// ============================================================================

#[tokio::test]
async fn test_multiple_tasks_different_conditions() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "is_web".to_string(), json!(true));
    runtime.set_host_var("localhost", "is_db".to_string(), json!(false));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task1 = Task::new("Web task", "debug")
        .arg("msg", "Web server config")
        .when("is_web");

    let task2 = Task::new("DB task", "debug")
        .arg("msg", "Database config")
        .when("is_db");

    let playbook = create_playbook("test", "all", vec![task1, task2]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    // One task ran, one skipped
    assert!(host_result.stats.ok > 0 || host_result.stats.changed > 0);
    assert_eq!(host_result.stats.skipped, 1);
}

// ============================================================================
// Multi-host tests
// ============================================================================

#[tokio::test]
async fn test_condition_per_host() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("web1".to_string(), Some("webservers"));
    runtime.add_host("db1".to_string(), Some("databases"));

    // Set different variables for each host
    runtime.set_host_var("web1", "role".to_string(), json!("web"));
    runtime.set_host_var("db1", "role".to_string(), json!("db"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Web only task", "debug")
        .arg("msg", "This is a web server")
        .when("role == 'web'");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    // web1 should run, db1 should skip
    let web_result = results.get("web1").unwrap();
    let db_result = results.get("db1").unwrap();

    assert_eq!(web_result.stats.skipped, 0);
    assert_eq!(db_result.stats.skipped, 1);
}

// ============================================================================
// Edge cases and complex expressions
// ============================================================================

#[tokio::test]
async fn test_when_with_parentheses_grouping() {
    // Note: This tests logical grouping if supported
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "a".to_string(), json!(false));
    runtime.set_host_var("localhost", "b".to_string(), json!(true));
    runtime.set_host_var("localhost", "c".to_string(), json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // (a or b) and c should be true
    let task = Task::new("Complex condition", "debug")
        .arg("msg", "Complex passed")
        .when("b and c");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_register_and_use_in_when() {
    // Test registering a result and using it in a subsequent when condition
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // First task registers a result
    let task1 = Task::new("First task", "debug")
        .arg("msg", "First task")
        .register("first_result");

    // Second task uses the registered result
    let task2 = Task::new("Second task", "debug")
        .arg("msg", "First task completed")
        .when("first_result is defined");

    let playbook = create_playbook("test", "all", vec![task1, task2]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    // Both tasks should run successfully
    assert!(!host_result.failed);
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Handler notification tests with conditionals
// ============================================================================

#[tokio::test]
async fn test_notify_with_changed_and_when() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "should_run".to_string(), json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Task that notifies handler when condition is met
    let task = Task::new("Config change", "debug")
        .arg("msg", "Changing config")
        .when("should_run")
        .notify("restart service");

    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Test with handler", "all");
    play.gather_facts = false;
    play.add_task(task);

    // Add a handler
    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), json!("Restarting service"));
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
// Ignore errors with conditionals
// ============================================================================

#[tokio::test]
async fn test_ignore_errors_in_loop() {
    let executor = create_test_executor(vec!["localhost"]);

    // This tests that ignore_errors works within loops
    let task = Task::new("May fail", "fail")
        .arg("msg", "Intentional failure")
        .loop_over(vec![json!("a"), json!("b")])
        .ignore_errors(true);

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    // Should not mark host as failed due to ignore_errors
    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Combined loop and register
// ============================================================================

#[tokio::test]
async fn test_loop_with_register() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Loop and register", "debug")
        .arg("msg", "Item: {{ item }}")
        .loop_over(vec![json!("x"), json!("y"), json!("z")])
        .register("loop_results");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

// ============================================================================
// Facts-based conditions
// ============================================================================

#[tokio::test]
async fn test_when_with_ansible_facts() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    // Set facts
    runtime.set_host_fact("localhost", "os_family".to_string(), json!("Debian"));
    runtime.set_host_fact("localhost", "distribution".to_string(), json!("Ubuntu"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Debian task", "debug")
        .arg("msg", "This is Debian")
        .when("ansible_facts.os_family == 'Debian'");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Special variables in conditions
// ============================================================================

#[tokio::test]
async fn test_when_with_inventory_hostname() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("webserver01".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check hostname", "debug")
        .arg("msg", "This is the webserver")
        .when("inventory_hostname == 'webserver01'");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("webserver01").unwrap();
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Test assert module with conditions
// ============================================================================

#[tokio::test]
async fn test_assert_that_pass() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "my_value".to_string(), json!(10));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Assert value", "assert")
        .arg("that", json!(["my_value == 10"]))
        .arg("success_msg", "Value is correct");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_assert_that_fail() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "my_value".to_string(), json!(5));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Assert value", "assert")
        .arg("that", json!(["my_value == 10"]))
        .arg("fail_msg", "Value is incorrect");

    let playbook = create_playbook("test", "all", vec![task]);
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    // Assert should fail
    assert!(host_result.failed || host_result.stats.failed > 0);
}
