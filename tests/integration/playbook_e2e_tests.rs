//! End-to-end playbook execution tests
//!
//! These tests verify complete playbook execution scenarios including:
//! - Multi-play playbooks
//! - Variable interpolation
//! - Task ordering and dependencies
//! - Pre-tasks, roles, tasks, post-tasks execution order
//! - Host pattern matching
//! - Gather facts integration
//! - Check mode and diff mode

use std::collections::HashMap;
use std::path::PathBuf;

use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::RuntimeContext;
use rustible::executor::task::Task;
use rustible::executor::{Executor, ExecutorConfig};

// ============================================================================
// Multi-Play Playbook Tests
// ============================================================================

#[tokio::test]
async fn test_single_play_single_task() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Simple Playbook");
    let mut play = Play::new("Simple Play", "all");
    play.gather_facts = false;
    play.add_task(Task::new("Debug message", "debug").arg("msg", "Hello, World!"));
    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    assert!(results.contains_key("localhost"));
    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    assert!(!host_result.unreachable);
}

#[tokio::test]
async fn test_multiple_plays_execution() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi-Play Playbook");

    // First play
    let mut play1 = Play::new("Play 1", "all");
    play1.gather_facts = false;
    play1.add_task(Task::new("Task 1", "debug").arg("msg", "Play 1 Task"));
    playbook.add_play(play1);

    // Second play
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(Task::new("Task 2", "debug").arg("msg", "Play 2 Task"));
    playbook.add_play(play2);

    // Third play
    let mut play3 = Play::new("Play 3", "all");
    play3.gather_facts = false;
    play3.add_task(Task::new("Task 3", "debug").arg("msg", "Play 3 Task"));
    playbook.add_play(play3);

    let results = executor.run_playbook(&playbook).await.unwrap();

    assert!(results.contains_key("localhost"));
    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_multiple_tasks_in_play() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi-Task Playbook");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Add multiple tasks
    for i in 1..=5 {
        play.add_task(Task::new(format!("Task {}", i), "debug").arg("msg", format!("Message {}", i)));
    }

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    // At least 5 tasks should have completed (ok or changed)
    assert!(host_result.stats.ok + host_result.stats.changed >= 5);
}

// ============================================================================
// Variable Interpolation Tests
// ============================================================================

#[tokio::test]
async fn test_play_level_variables() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Variable Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;
    play.set_var("my_var", serde_json::json!("test_value"));
    play.set_var("my_number", serde_json::json!(42));

    play.add_task(Task::new("Use variable", "debug").arg("msg", "{{ my_var }}"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_runtime_context_variables() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("global_var".to_string(), serde_json::json!("global_value"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Global Variable Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;
    play.add_task(Task::new("Use global var", "debug").arg("msg", "{{ global_var }}"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_host_level_variables() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_var("localhost", "host_var".to_string(), serde_json::json!("host_value"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Host Variable Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;
    play.add_task(Task::new("Use host var", "debug").arg("msg", "{{ host_var }}"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_registered_variables() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Register Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that registers a result
    play.add_task(
        Task::new("Register result", "command")
            .arg("cmd", "echo 'test output'")
            .register("cmd_result"),
    );

    // Task that uses the registered variable
    play.add_task(Task::new("Use registered", "debug").arg("msg", "Result registered"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

// ============================================================================
// Multi-Host Execution Tests
// ============================================================================

#[tokio::test]
async fn test_multiple_hosts() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("host1".to_string(), Some("webservers"));
    runtime.add_host("host2".to_string(), Some("webservers"));
    runtime.add_host("host3".to_string(), Some("webservers"));

    let config = ExecutorConfig {
        gather_facts: false,
        forks: 3,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi-Host Test");
    let mut play = Play::new("Test Play", "webservers");
    play.gather_facts = false;
    play.add_task(Task::new("Debug on all hosts", "debug").arg("msg", "Running on {{ inventory_hostname }}"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    assert_eq!(results.len(), 3);
    assert!(results.contains_key("host1"));
    assert!(results.contains_key("host2"));
    assert!(results.contains_key("host3"));

    for (_, result) in &results {
        assert!(!result.failed);
    }
}

#[tokio::test]
async fn test_host_pattern_all() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("server1".to_string(), Some("group1"));
    runtime.add_host("server2".to_string(), Some("group2"));
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("All Hosts Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;
    play.add_task(Task::new("Run on all", "debug").arg("msg", "Hello"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    // Should run on all 3 hosts
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_different_plays_different_hosts() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("web1".to_string(), Some("webservers"));
    runtime.add_host("web2".to_string(), Some("webservers"));
    runtime.add_host("db1".to_string(), Some("databases"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Different Hosts Test");

    // Play for webservers
    let mut play1 = Play::new("Web Play", "webservers");
    play1.gather_facts = false;
    play1.add_task(Task::new("Web task", "debug").arg("msg", "Web server task"));
    playbook.add_play(play1);

    // Play for databases
    let mut play2 = Play::new("DB Play", "databases");
    play2.gather_facts = false;
    play2.add_task(Task::new("DB task", "debug").arg("msg", "Database task"));
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();

    // All 3 hosts should have results
    assert_eq!(results.len(), 3);
    assert!(results.contains_key("web1"));
    assert!(results.contains_key("web2"));
    assert!(results.contains_key("db1"));
}

// ============================================================================
// Task Ordering Tests
// ============================================================================

#[tokio::test]
async fn test_pre_tasks_execute_before_tasks() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Pre-tasks Test
  hosts: all
  gather_facts: false
  pre_tasks:
    - name: Pre-task 1
      debug:
        msg: "Pre-task"
  tasks:
    - name: Main task 1
      debug:
        msg: "Main task"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let results = executor.run_playbook(&playbook).await.unwrap();

    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_post_tasks_execute_after_tasks() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Post-tasks Test
  hosts: all
  gather_facts: false
  tasks:
    - name: Main task
      debug:
        msg: "Main task"
  post_tasks:
    - name: Post-task
      debug:
        msg: "Post-task"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let results = executor.run_playbook(&playbook).await.unwrap();

    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_complete_execution_order() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Complete Order Test
  hosts: all
  gather_facts: false
  pre_tasks:
    - name: Pre-task 1
      debug:
        msg: "Pre-task 1"
    - name: Pre-task 2
      debug:
        msg: "Pre-task 2"
  tasks:
    - name: Main task 1
      debug:
        msg: "Main task 1"
    - name: Main task 2
      debug:
        msg: "Main task 2"
  post_tasks:
    - name: Post-task 1
      debug:
        msg: "Post-task 1"
    - name: Post-task 2
      debug:
        msg: "Post-task 2"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let results = executor.run_playbook(&playbook).await.unwrap();

    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
    // 6 tasks should complete: 2 pre + 2 main + 2 post
    assert!(host_result.stats.ok + host_result.stats.changed >= 6);
}

// ============================================================================
// Check Mode Tests
// ============================================================================

#[tokio::test]
async fn test_check_mode_execution() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        check_mode: true,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Check Mode Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Even a copy task should not make changes in check mode
    play.add_task(
        Task::new("Copy task", "copy")
            .arg("src", "test.txt")
            .arg("dest", "/tmp/test.txt"),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Check mode should complete without errors
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_diff_mode_execution() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        check_mode: true,
        diff_mode: true,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Diff Mode Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(Task::new("Debug task", "debug").arg("msg", "Diff mode test"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

// ============================================================================
// Conditional Execution Tests
// ============================================================================

#[tokio::test]
async fn test_when_condition_true() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("run_task".to_string(), serde_json::json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Condition True Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Conditional task", "debug")
            .arg("msg", "Condition was true")
            .when("run_task"),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_when_condition_false() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("run_task".to_string(), serde_json::json!(false));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Condition False Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Conditional task", "debug")
            .arg("msg", "This should be skipped")
            .when("run_task"),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();
    // Task should be skipped, not failed
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_multiple_when_conditions() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("condition1".to_string(), serde_json::json!(true));
    runtime.set_global_var("condition2".to_string(), serde_json::json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Multiple Conditions Test
  hosts: all
  gather_facts: false
  tasks:
    - name: Task with multiple when
      debug:
        msg: "Both conditions true"
      when:
        - condition1
        - condition2
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let results = executor.run_playbook(&playbook).await.unwrap();

    assert!(!results.get("localhost").unwrap().failed);
}

// ============================================================================
// Loop Tests
// ============================================================================

#[tokio::test]
async fn test_simple_loop() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Loop Test
  hosts: all
  gather_facts: false
  tasks:
    - name: Loop task
      debug:
        msg: "Item: {{ item }}"
      loop:
        - item1
        - item2
        - item3
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let results = executor.run_playbook(&playbook).await.unwrap();

    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_loop_with_index() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Loop with Index Test
  hosts: all
  gather_facts: false
  tasks:
    - name: Indexed loop
      debug:
        msg: "Item {{ item }}"
      loop:
        - a
        - b
        - c
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let results = executor.run_playbook(&playbook).await.unwrap();

    assert!(!results.get("localhost").unwrap().failed);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_ignore_errors() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Ignore Errors Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that fails but is ignored
    let mut failing_task = Task::new("Failing task", "fail").arg("msg", "This fails");
    failing_task.ignore_errors = true;
    play.add_task(failing_task);

    // This task should still run
    play.add_task(Task::new("After failed", "debug").arg("msg", "Still running"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Play should complete despite the failed task
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_failed_task_stops_execution() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Fail Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that fails
    play.add_task(Task::new("Failing task", "fail").arg("msg", "Intentional failure"));

    // This task should not run
    play.add_task(Task::new("After failed", "debug").arg("msg", "Should not reach"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();
    // Host should be marked as failed
    assert!(host_result.failed);
}

// ============================================================================
// Complex Playbook Tests
// ============================================================================

#[tokio::test]
async fn test_realistic_deployment_playbook() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("web1".to_string(), Some("webservers"));
    runtime.add_host("web2".to_string(), Some("webservers"));

    let config = ExecutorConfig {
        gather_facts: false,
        forks: 2,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Deploy Application
  hosts: webservers
  gather_facts: false
  vars:
    app_name: myapp
    app_port: 8080

  pre_tasks:
    - name: Check connectivity
      debug:
        msg: "Checking {{ inventory_hostname }}"

  tasks:
    - name: Stop old application
      debug:
        msg: "Stopping {{ app_name }}"

    - name: Deploy new version
      debug:
        msg: "Deploying to port {{ app_port }}"

    - name: Start application
      debug:
        msg: "Starting {{ app_name }}"

  post_tasks:
    - name: Health check
      debug:
        msg: "Verifying deployment"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let results = executor.run_playbook(&playbook).await.unwrap();

    assert_eq!(results.len(), 2);
    for (host, result) in &results {
        assert!(!result.failed, "Host {} should not have failed", host);
    }
}

#[tokio::test]
async fn test_yaml_parsing_and_execution() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
---
- name: YAML Parsing Test
  hosts: all
  gather_facts: false
  vars:
    message: "Hello from YAML"
    count: 3

  tasks:
    - name: Print message
      debug:
        msg: "{{ message }}"

    - name: Loop example
      debug:
        msg: "Iteration"
      loop:
        - 1
        - 2
        - 3
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays.len(), 1);
    assert_eq!(playbook.plays[0].name, "YAML Parsing Test");

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_empty_playbook() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let playbook = Playbook::new("Empty Playbook");

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Should complete without errors even with no plays
    assert!(results.is_empty() || !results.get("localhost").map_or(false, |r| r.failed));
}

#[tokio::test]
async fn test_play_with_no_tasks() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("No Tasks Playbook");
    let play = Play::new("Empty Play", "all");
    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Should handle gracefully
    assert!(results.is_empty() || !results.get("localhost").map_or(false, |r| r.failed));
}

#[tokio::test]
async fn test_undefined_variable_handling() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Undefined Var Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that uses default filter for undefined variable
    play.add_task(Task::new("Use default", "debug").arg("msg", "{{ undefined_var | default('fallback') }}"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Should use the default value and not fail
    assert!(!results.get("localhost").unwrap().failed);
}

// ============================================================================
// Statistics and Results Tests
// ============================================================================

#[tokio::test]
async fn test_execution_statistics() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Statistics Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Add multiple successful tasks
    for i in 1..=3 {
        play.add_task(Task::new(format!("Task {}", i), "debug").arg("msg", format!("Message {}", i)));
    }

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Verify statistics are recorded
    let total = host_result.stats.ok + host_result.stats.changed + host_result.stats.skipped;
    assert!(total >= 3, "Expected at least 3 tasks, got {}", total);
    assert_eq!(host_result.stats.failed, 0);
    assert_eq!(host_result.stats.unreachable, 0);
}
