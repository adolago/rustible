//! Conditional Evaluation Compatibility Tests
//!
//! Tests that Rustible evaluates when conditions the same way as Ansible.
//! This includes:
//! - Boolean literals (true/false, True/False)
//! - Variable truthiness
//! - Comparison operators (==, !=, <, >, <=, >=)
//! - Logical operators (and, or, not)
//! - Jinja2 tests (is defined, is not defined, is none, etc.)
//! - Complex expressions with parentheses
//! - Multiple conditions as list (implicit AND)

use indexmap::IndexMap;
use serde_json::json;

use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::{RegisteredResult, RuntimeContext};
use rustible::executor::task::Task;
use rustible::executor::{Executor, ExecutorConfig};

// ============================================================================
// Test Helpers
// ============================================================================

fn create_executor_with_vars(vars: Vec<(&str, serde_json::Value)>) -> Executor {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    for (key, value) in vars {
        runtime.set_host_var("localhost", key.to_string(), value);
    }

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };

    Executor::with_runtime(config, runtime)
}

fn create_playbook_with_task(task: Task) -> Playbook {
    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Conditional test", "all");
    play.gather_facts = false;
    play.add_task(task);
    playbook.add_play(play);
    playbook
}

// ============================================================================
// Section 1: Boolean Literals
// ============================================================================

#[tokio::test]
async fn test_when_true_literal() {
    let executor = create_executor_with_vars(vec![]);

    let task = Task::new("Should run", "debug")
        .arg("msg", "Condition is true")
        .when("true");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_false_literal() {
    let executor = create_executor_with_vars(vec![]);

    let task = Task::new("Should skip", "debug")
        .arg("msg", "Condition is false")
        .when("false");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_python_true() {
    let executor = create_executor_with_vars(vec![]);

    let task = Task::new("Python True", "debug")
        .arg("msg", "Python-style True")
        .when("True");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_python_false() {
    let executor = create_executor_with_vars(vec![]);

    let task = Task::new("Python False", "debug")
        .arg("msg", "Python-style False")
        .when("False");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

// ============================================================================
// Section 2: Variable Truthiness
// ============================================================================

#[tokio::test]
async fn test_when_truthy_variable() {
    let executor = create_executor_with_vars(vec![("enabled", json!(true))]);

    let task = Task::new("Truthy check", "debug")
        .arg("msg", "enabled is true")
        .when("enabled");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_falsy_variable() {
    let executor = create_executor_with_vars(vec![("disabled", json!(false))]);

    let task = Task::new("Falsy check", "debug")
        .arg("msg", "disabled is false")
        .when("disabled");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_empty_string_falsy() {
    let executor = create_executor_with_vars(vec![("empty", json!(""))]);

    let task = Task::new("Empty string", "debug")
        .arg("msg", "Empty string is falsy")
        .when("empty");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_nonempty_string_truthy() {
    let executor = create_executor_with_vars(vec![("text", json!("hello"))]);

    let task = Task::new("Non-empty string", "debug")
        .arg("msg", "Non-empty string is truthy")
        .when("text");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_zero_falsy() {
    let executor = create_executor_with_vars(vec![("zero", json!(0))]);

    let task = Task::new("Zero check", "debug")
        .arg("msg", "Zero is falsy")
        .when("zero");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_nonzero_truthy() {
    let executor = create_executor_with_vars(vec![("count", json!(42))]);

    let task = Task::new("Non-zero check", "debug")
        .arg("msg", "Non-zero is truthy")
        .when("count");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_empty_list_falsy() {
    let executor = create_executor_with_vars(vec![("items", json!([]))]);

    let task = Task::new("Empty list", "debug")
        .arg("msg", "Empty list is falsy")
        .when("items");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_nonempty_list_truthy() {
    let executor = create_executor_with_vars(vec![("items", json!(["a", "b"]))]);

    let task = Task::new("Non-empty list", "debug")
        .arg("msg", "Non-empty list is truthy")
        .when("items");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 3: Comparison Operators
// ============================================================================

#[tokio::test]
async fn test_when_equality() {
    let executor = create_executor_with_vars(vec![("value", json!(10))]);

    let task = Task::new("Equality", "debug")
        .arg("msg", "value equals 10")
        .when("value == 10");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_inequality() {
    let executor = create_executor_with_vars(vec![("value", json!(10))]);

    let task = Task::new("Inequality", "debug")
        .arg("msg", "value not equals 5")
        .when("value != 5");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_greater_than() {
    let executor = create_executor_with_vars(vec![("count", json!(10))]);

    let task = Task::new("Greater than", "debug")
        .arg("msg", "count > 5")
        .when("count > 5");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_less_than_or_equal() {
    let executor = create_executor_with_vars(vec![("count", json!(5))]);

    let task = Task::new("Less or equal", "debug")
        .arg("msg", "count <= 5")
        .when("count <= 5");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_string_comparison() {
    let executor = create_executor_with_vars(vec![("env", json!("production"))]);

    let task = Task::new("String compare", "debug")
        .arg("msg", "env is production")
        .when("env == 'production'");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_string_double_quotes() {
    let executor = create_executor_with_vars(vec![("env", json!("staging"))]);

    let task = Task::new("Double quotes", "debug")
        .arg("msg", "env is staging")
        .when("env == \"staging\"");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 4: Logical Operators
// ============================================================================

#[tokio::test]
async fn test_when_and_both_true() {
    let executor = create_executor_with_vars(vec![("a", json!(true)), ("b", json!(true))]);

    let task = Task::new("And both true", "debug")
        .arg("msg", "a and b")
        .when("a and b");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_and_one_false() {
    let executor = create_executor_with_vars(vec![("a", json!(true)), ("b", json!(false))]);

    let task = Task::new("And one false", "debug")
        .arg("msg", "a and b")
        .when("a and b");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_or_both_false() {
    let executor = create_executor_with_vars(vec![("a", json!(false)), ("b", json!(false))]);

    let task = Task::new("Or both false", "debug")
        .arg("msg", "a or b")
        .when("a or b");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_or_one_true() {
    let executor = create_executor_with_vars(vec![("a", json!(false)), ("b", json!(true))]);

    let task = Task::new("Or one true", "debug")
        .arg("msg", "a or b")
        .when("a or b");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_not_true() {
    let executor = create_executor_with_vars(vec![]);

    let task = Task::new("Not true", "debug")
        .arg("msg", "not true is false")
        .when("not true");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_not_false() {
    let executor = create_executor_with_vars(vec![]);

    let task = Task::new("Not false", "debug")
        .arg("msg", "not false is true")
        .when("not false");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_not_variable() {
    let executor = create_executor_with_vars(vec![("disabled", json!(false))]);

    let task = Task::new("Not variable", "debug")
        .arg("msg", "not disabled")
        .when("not disabled");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 5: Jinja2 Tests
// ============================================================================

#[tokio::test]
async fn test_when_is_defined() {
    let executor = create_executor_with_vars(vec![("my_var", json!("value"))]);

    let task = Task::new("Is defined", "debug")
        .arg("msg", "my_var is defined")
        .when("my_var is defined");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_is_defined_missing() {
    let executor = create_executor_with_vars(vec![]);

    let task = Task::new("Is defined missing", "debug")
        .arg("msg", "undefined_var is defined")
        .when("undefined_var is defined");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_when_is_not_defined() {
    let executor = create_executor_with_vars(vec![]);

    let task = Task::new("Is not defined", "debug")
        .arg("msg", "missing_var is not defined")
        .when("missing_var is not defined");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_is_none() {
    let executor = create_executor_with_vars(vec![("null_var", json!(null))]);

    let task = Task::new("Is none", "debug")
        .arg("msg", "null_var is none")
        .when("null_var is none");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 6: In Operator
// ============================================================================

#[tokio::test]
async fn test_when_in_list() {
    let executor = create_executor_with_vars(vec![
        ("items", json!(["a", "b", "c"])),
        ("target", json!("b")),
    ]);

    let task = Task::new("In list", "debug")
        .arg("msg", "target in items")
        .when("target in items");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_not_in_list() {
    let executor = create_executor_with_vars(vec![
        ("items", json!(["a", "b", "c"])),
        ("target", json!("x")),
    ]);

    let task = Task::new("Not in list", "debug")
        .arg("msg", "target not in items")
        .when("target not in items");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 7: Nested Variable Access
// ============================================================================

#[tokio::test]
async fn test_when_nested_dict() {
    let executor = create_executor_with_vars(vec![(
        "config",
        json!({
            "database": {
                "host": "localhost",
                "port": 5432
            }
        }),
    )]);

    let task = Task::new("Nested dict", "debug")
        .arg("msg", "config.database.host == localhost")
        .when("config.database.host == 'localhost'");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_nested_boolean() {
    let executor = create_executor_with_vars(vec![(
        "settings",
        json!({
            "cache": {
                "enabled": true
            }
        }),
    )]);

    let task = Task::new("Nested boolean", "debug")
        .arg("msg", "settings.cache.enabled")
        .when("settings.cache.enabled");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 8: Registered Variables
// ============================================================================

#[tokio::test]
async fn test_when_registered_defined() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let result = RegisteredResult {
        changed: true,
        failed: false,
        skipped: false,
        rc: Some(0),
        stdout: Some("success".to_string()),
        ..Default::default()
    };
    runtime.register_result("localhost", "cmd_result".to_string(), result);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check registered", "debug")
        .arg("msg", "cmd_result is defined")
        .when("cmd_result is defined");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_registered_rc() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let result = RegisteredResult {
        changed: true,
        failed: false,
        skipped: false,
        rc: Some(0),
        ..Default::default()
    };
    runtime.register_result("localhost", "cmd_result".to_string(), result);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check rc", "debug")
        .arg("msg", "cmd_result.rc == 0")
        .when("cmd_result.rc == 0");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_registered_changed() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let result = RegisteredResult {
        changed: true,
        failed: false,
        skipped: false,
        ..Default::default()
    };
    runtime.register_result("localhost", "prev_task".to_string(), result);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check changed", "debug")
        .arg("msg", "prev_task.changed")
        .when("prev_task.changed");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 9: Multiple Conditions (Implicit AND)
// ============================================================================

#[tokio::test]
async fn test_when_multiple_all_true() {
    let executor = create_executor_with_vars(vec![
        ("enabled", json!(true)),
        ("count", json!(10)),
        ("env", json!("prod")),
    ]);

    let task = Task::new("Multiple conditions", "debug")
        .arg("msg", "All conditions met")
        .when_multiple(vec!["enabled", "count > 5", "env == 'prod'"]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

#[tokio::test]
async fn test_when_multiple_one_false() {
    let executor = create_executor_with_vars(vec![
        ("enabled", json!(true)),
        ("count", json!(3)),
        ("env", json!("prod")),
    ]);

    let task = Task::new("Multiple conditions", "debug")
        .arg("msg", "All conditions met")
        .when_multiple(vec!["enabled", "count > 5", "env == 'prod'"]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // count > 5 is false, so task should be skipped
    assert_eq!(host_result.stats.skipped, 1);
}

// ============================================================================
// Section 10: Special Variables
// ============================================================================

#[tokio::test]
async fn test_when_inventory_hostname() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("webserver01".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check hostname", "debug")
        .arg("msg", "This is webserver01")
        .when("inventory_hostname == 'webserver01'");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("webserver01").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 11: Facts-based Conditions
// ============================================================================

#[tokio::test]
async fn test_when_ansible_facts() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_fact("localhost", "os_family".to_string(), json!("Debian"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Check OS", "debug")
        .arg("msg", "This is Debian")
        .when("ansible_facts.os_family == 'Debian'");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 12: Per-Host Condition Evaluation
// ============================================================================

#[tokio::test]
async fn test_condition_per_host() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("web1".to_string(), Some("webservers".to_string()));
    runtime.add_host("db1".to_string(), Some("databases".to_string()));

    runtime.set_host_var("web1", "role".to_string(), json!("web"));
    runtime.set_host_var("db1", "role".to_string(), json!("database"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let task = Task::new("Web only", "debug")
        .arg("msg", "This is a web server")
        .when("role == 'web'");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();

    // web1 should run
    assert_eq!(results.get("web1").unwrap().stats.skipped, 0);
    // db1 should skip
    assert_eq!(results.get("db1").unwrap().stats.skipped, 1);
}
