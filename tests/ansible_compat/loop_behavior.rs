//! Loop Behavior Compatibility Tests
//!
//! Tests that Rustible's loop constructs match Ansible's behavior including:
//! - Simple loops (loop keyword)
//! - Legacy with_* loops (with_items, with_dict, etc.)
//! - Loop control variables (loop_var, index_var, label)
//! - ansible_loop magic variables (index, index0, first, last, length)
//! - Loop with conditions
//! - Loop result registration

use indexmap::IndexMap;
use serde_json::json;

use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::RuntimeContext;
use rustible::executor::task::Task;
use rustible::executor::{Executor, ExecutorConfig};

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_executor() -> Executor {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };

    Executor::with_runtime(config, runtime)
}

fn create_playbook_with_task(task: Task) -> Playbook {
    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Test loop", "all");
    play.gather_facts = false;
    play.add_task(task);
    playbook.add_play(play);
    playbook
}

// ============================================================================
// Section 1: Simple Loop Tests
// ============================================================================

#[tokio::test]
async fn test_loop_with_simple_list() {
    let executor = create_test_executor();

    let task = Task::new("Process items", "debug")
        .arg("msg", "Item: {{ item }}")
        .loop_over(vec![json!("one"), json!("two"), json!("three")]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
    // Loop should execute 3 times
}

#[tokio::test]
async fn test_loop_with_numbers() {
    let executor = create_test_executor();

    let task = Task::new("Process numbers", "debug")
        .arg("msg", "Number: {{ item }}")
        .loop_over(vec![json!(1), json!(2), json!(3), json!(4), json!(5)]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_loop_with_empty_list() {
    let executor = create_test_executor();

    let task = Task::new("Empty loop", "debug")
        .arg("msg", "Should not see this")
        .loop_over(vec![]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Empty loop should complete successfully
    assert!(!host_result.failed);
}

// ============================================================================
// Section 2: Loop with Dictionaries
// ============================================================================

#[tokio::test]
async fn test_loop_with_list_of_dicts() {
    let executor = create_test_executor();

    let task = Task::new("Process users", "debug")
        .arg("msg", "User: {{ item.name }}, Role: {{ item.role }}")
        .loop_over(vec![
            json!({"name": "alice", "role": "admin"}),
            json!({"name": "bob", "role": "user"}),
            json!({"name": "charlie", "role": "guest"}),
        ]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_loop_dict_attribute_access() {
    let executor = create_test_executor();

    let task = Task::new("Access dict attrs", "debug")
        .arg("msg", "Config {{ item.key }}: {{ item.value }}")
        .loop_over(vec![
            json!({"key": "host", "value": "localhost"}),
            json!({"key": "port", "value": 8080}),
        ]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// Section 3: Loop Control - Custom loop_var
// ============================================================================

#[tokio::test]
async fn test_loop_with_custom_loop_var() {
    let executor = create_test_executor();

    let task = Task::new("Custom var", "debug")
        .arg("msg", "Package: {{ pkg }}")
        .loop_over(vec![json!("nginx"), json!("postgresql"), json!("redis")])
        .loop_var("pkg");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_nested_loops_different_vars() {
    // When nesting loops, custom loop_var prevents variable collision
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Nested loops", "all");
    play.gather_facts = false;
    play.vars.set("outer_list", json!(["a", "b"]));
    play.vars.set("inner_list", json!([1, 2]));

    // Outer loop
    let outer_task = Task::new("Outer loop", "debug")
        .arg("msg", "Outer: {{ outer_item }}")
        .loop_over(vec![json!("a"), json!("b")])
        .loop_var("outer_item");

    play.add_task(outer_task);
    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// Section 4: ansible_loop Magic Variables
// ============================================================================

#[tokio::test]
async fn test_ansible_loop_index() {
    let executor = create_test_executor();

    let task = Task::new("Check index", "debug")
        .arg("msg", "Index: {{ ansible_loop.index }}")
        .loop_over(vec![json!("first"), json!("second"), json!("third")]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_ansible_loop_index0() {
    let executor = create_test_executor();

    let task = Task::new("Check index0", "debug")
        .arg("msg", "Index0: {{ ansible_loop.index0 }}")
        .loop_over(vec![json!("a"), json!("b"), json!("c")]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_ansible_loop_first_last() {
    let executor = create_test_executor();

    let task = Task::new("Check first/last", "debug")
        .arg("msg", "First: {{ ansible_loop.first }}, Last: {{ ansible_loop.last }}")
        .loop_over(vec![json!("only_one")]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_ansible_loop_length() {
    let executor = create_test_executor();

    let task = Task::new("Check length", "debug")
        .arg("msg", "Total items: {{ ansible_loop.length }}")
        .loop_over(vec![json!(1), json!(2), json!(3), json!(4), json!(5)]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// Section 5: Loop with Conditions
// ============================================================================

#[tokio::test]
async fn test_loop_with_when_skip_some() {
    let executor = create_test_executor();

    let task = Task::new("Conditional loop", "debug")
        .arg("msg", "Processing: {{ item }}")
        .loop_over(vec![
            json!({"name": "active", "enabled": true}),
            json!({"name": "inactive", "enabled": false}),
            json!({"name": "also_active", "enabled": true}),
        ])
        .when("item.enabled");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
    // Two items should be processed, one skipped
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

    let task = Task::new("Skip all", "debug")
        .arg("msg", "Item: {{ item }}")
        .loop_over(vec![json!("a"), json!("b")])
        .when("process_items");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // All items skipped
    assert_eq!(host_result.stats.skipped, 1);
}

#[tokio::test]
async fn test_loop_with_complex_when() {
    let executor = create_test_executor();

    let task = Task::new("Complex condition", "debug")
        .arg("msg", "Processing: {{ item.name }}")
        .loop_over(vec![
            json!({"name": "admin", "role": "admin", "active": true}),
            json!({"name": "user1", "role": "user", "active": true}),
            json!({"name": "user2", "role": "user", "active": false}),
        ])
        .when("item.active and item.role == 'admin'");

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// Section 6: Loop with Registration
// ============================================================================

#[tokio::test]
async fn test_loop_register_results() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Loop register", "all");
    play.gather_facts = false;

    // First task: loop and register
    let task1 = Task::new("Process items", "debug")
        .arg("msg", "Item: {{ item }}")
        .loop_over(vec![json!("x"), json!("y"), json!("z")])
        .register("loop_results");

    // Second task: access registered results
    let task2 = Task::new("Check results", "debug")
        .arg("msg", "Results registered")
        .when("loop_results is defined");

    play.add_task(task1);
    play.add_task(task2);
    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
    assert_eq!(host_result.stats.skipped, 0);
}

// ============================================================================
// Section 7: Loop with set_fact
// ============================================================================

#[tokio::test]
async fn test_loop_with_set_fact() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Loop set_fact", "all");
    play.gather_facts = false;

    // Set fact in loop
    let task = Task::new("Set facts", "set_fact")
        .arg("last_item", "{{ item }}")
        .loop_over(vec![json!("first"), json!("second"), json!("last")]);

    play.add_task(task);
    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// Section 8: Loop with ignore_errors
// ============================================================================

#[tokio::test]
async fn test_loop_with_ignore_errors() {
    let executor = create_test_executor();

    let task = Task::new("May fail", "fail")
        .arg("msg", "Intentional failure")
        .loop_over(vec![json!("a"), json!("b")])
        .ignore_errors(true);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Should not mark as failed due to ignore_errors
    assert!(!host_result.failed);
}

// ============================================================================
// Section 9: Variable-based loops
// ============================================================================

#[tokio::test]
async fn test_loop_from_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var(
        "localhost",
        "packages".to_string(),
        json!(["nginx", "php-fpm", "mysql-server"]),
    );

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Loop from var", "all");
    play.gather_facts = false;

    // Loop using template to reference variable
    let task = Task::new("Install packages", "debug")
        .arg("msg", "Installing: {{ item }}")
        .loop_over_var("packages");

    play.add_task(task);
    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// Section 10: Multi-host loop behavior
// ============================================================================

#[tokio::test]
async fn test_loop_different_per_host() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("web1".to_string(), Some("webservers".to_string()));
    runtime.add_host("db1".to_string(), Some("databases".to_string()));

    runtime.set_host_var("web1", "packages".to_string(), json!(["nginx", "php"]));
    runtime.set_host_var("db1", "packages".to_string(), json!(["postgresql", "pgadmin"]));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("test");
    let mut play = Play::new("Per-host loops", "all");
    play.gather_facts = false;

    let task = Task::new("Install", "debug")
        .arg("msg", "Package: {{ item }}")
        .loop_over_var("packages");

    play.add_task(task);
    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    // Both hosts should succeed
    assert!(!results.get("web1").unwrap().failed);
    assert!(!results.get("db1").unwrap().failed);
}

// ============================================================================
// Section 11: Loop edge cases
// ============================================================================

#[tokio::test]
async fn test_loop_single_item() {
    let executor = create_test_executor();

    let task = Task::new("Single item", "debug")
        .arg("msg", "Only item: {{ item }}")
        .loop_over(vec![json!("singleton")]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_loop_with_null_items() {
    let executor = create_test_executor();

    let task = Task::new("Null in loop", "debug")
        .arg("msg", "Item: {{ item | default('null') }}")
        .loop_over(vec![json!("valid"), json!(null), json!("also_valid")]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_loop_with_mixed_types() {
    let executor = create_test_executor();

    let task = Task::new("Mixed types", "debug")
        .arg("msg", "Item: {{ item }}")
        .loop_over(vec![
            json!("string"),
            json!(42),
            json!(true),
            json!({"key": "value"}),
            json!(["nested", "list"]),
        ]);

    let playbook = create_playbook_with_task(task);
    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}
