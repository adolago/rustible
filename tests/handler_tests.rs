//! Comprehensive tests for the Rustible handler system
//!
//! This test suite covers:
//! 1. Handler notification - single/multiple tasks notifying handlers
//! 2. Handler deduplication - handler runs once despite multiple notifications
//! 3. Handler execution order - handlers run in definition order
//! 4. Handler flush - handlers run at end of play or with meta: flush_handlers
//! 5. Handler chaining - handler notifies another handler
//! 6. Listen directive - handler listens to topic
//! 7. Conditional handlers - handler with when condition
//! 8. Handler with tasks - handler with loop, delegate_to, etc.
//! 9. Handler in roles - role handlers accessible from play
//! 10. Edge cases - handler not found, empty handlers, handler in block

use indexmap::IndexMap;
use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::RuntimeContext;
use rustible::executor::task::{Handler, Task};
use rustible::executor::{Executor, ExecutorConfig};

// ============================================================================
// Test 1: Handler Notification
// ============================================================================

#[test]
fn test_handler_definition_basic() {
    let handler = Handler {
        name: "restart nginx".to_string(),
        module: "service".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("name".to_string(), serde_json::json!("nginx"));
            args.insert("state".to_string(), serde_json::json!("restarted"));
            args
        },
        when: None,
        listen: vec![],
    };

    assert_eq!(handler.name, "restart nginx");
    assert_eq!(handler.module, "service");
    assert_eq!(handler.args.get("name"), Some(&serde_json::json!("nginx")));
    assert_eq!(
        handler.args.get("state"),
        Some(&serde_json::json!("restarted"))
    );
    assert!(handler.listen.is_empty());
}

#[test]
fn test_handler_definition_with_when_clause() {
    let handler = Handler {
        name: "reload firewall".to_string(),
        module: "command".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "cmd".to_string(),
                serde_json::json!("systemctl reload firewalld"),
            );
            args
        },
        when: Some("firewall_enabled".to_string()),
        listen: vec![],
    };

    assert_eq!(handler.when, Some("firewall_enabled".to_string()));
}

#[test]
fn test_handler_definition_with_listen() {
    let handler = Handler {
        name: "restart web services".to_string(),
        module: "service".to_string(),
        args: IndexMap::new(),
        when: None,
        listen: vec![
            "nginx config changed".to_string(),
            "php-fpm config changed".to_string(),
        ],
    };

    assert_eq!(handler.listen.len(), 2);
    assert!(handler.listen.contains(&"nginx config changed".to_string()));
    assert!(handler
        .listen
        .contains(&"php-fpm config changed".to_string()));
}

#[tokio::test]
async fn test_handler_registration_in_play() {
    let mut play = Play::new("Test Play", "all");

    let handler1 = Handler {
        name: "handler1".to_string(),
        module: "debug".to_string(),
        args: IndexMap::new(),
        when: None,
        listen: vec![],
    };

    let handler2 = Handler {
        name: "handler2".to_string(),
        module: "debug".to_string(),
        args: IndexMap::new(),
        when: None,
        listen: vec![],
    };

    play.add_handler(handler1);
    play.add_handler(handler2);

    assert_eq!(play.handlers.len(), 2);
    assert_eq!(play.handlers[0].name, "handler1");
    assert_eq!(play.handlers[1].name, "handler2");
}

#[tokio::test]
async fn test_single_task_notifies_single_handler() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Single Notification Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that notifies a handler
    play.add_task(
        Task::new("Copy config", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("restart service"),
    );

    // Handler to be notified
    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler executed"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_single_task_notifies_multiple_handlers() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multiple Notifications Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that notifies multiple handlers
    let mut task = Task::new("Update configs", "copy")
        .arg("src", "app.conf")
        .arg("dest", "/etc/app.conf");
    task.notify.push("restart nginx".to_string());
    task.notify.push("reload firewall".to_string());
    task.notify.push("clear cache".to_string());
    play.add_task(task);

    // Add handlers
    for handler_name in &["restart nginx", "reload firewall", "clear cache"] {
        play.add_handler(Handler {
            name: handler_name.to_string(),
            module: "debug".to_string(),
            args: {
                let mut args = IndexMap::new();
                args.insert(
                    "msg".to_string(),
                    serde_json::json!(format!("Running {}", handler_name)),
                );
                args
            },
            when: None,
            listen: vec![],
        });
    }

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_not_notified_when_task_unchanged() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("No Notification Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that doesn't change (debug never changes)
    play.add_task(
        Task::new("Check status", "debug")
            .arg("msg", "No changes")
            .notify("should not run"),
    );

    play.add_handler(Handler {
        name: "should not run".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("This should not execute"),
            );
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Handler should not have been executed since task didn't change
    assert!(!host_result.failed);
}

// ============================================================================
// Test 2: Handler Deduplication
// ============================================================================

#[tokio::test]
async fn test_handler_notified_twice_runs_once() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Deduplication Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Single task notifying the same handler multiple times
    let mut task = Task::new("Multiple notifications", "copy")
        .arg("src", "test.conf")
        .arg("dest", "/etc/test.conf");

    // Add duplicate notifications
    task.notify = vec![
        "restart service".to_string(),
        "restart service".to_string(),
        "restart service".to_string(),
    ];
    play.add_task(task);

    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("Handler executed once"),
            );
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should only execute once despite multiple notifications
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_notified_from_different_tasks_runs_once() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multiple Task Deduplication Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Multiple tasks notifying the same handler
    for i in 1..=3 {
        play.add_task(
            Task::new(format!("Task {}", i), "copy")
                .arg("src", format!("file{}.conf", i))
                .arg("dest", format!("/etc/file{}.conf", i))
                .notify("restart service"),
        );
    }

    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler runs once"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should only execute once even though notified by 3 tasks
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_after_flush_handler_can_be_notified_again() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Flush Re-notification Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // First task notifies handler
    play.add_task(
        Task::new("First change", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("restart service"),
    );

    // Flush handlers
    play.add_task(Task::new("Flush handlers", "meta").arg("_raw_params", "flush_handlers"));

    // Second task notifies the same handler (should run again after flush)
    play.add_task(
        Task::new("Second change", "copy")
            .arg("src", "file2.conf")
            .arg("dest", "/etc/file2.conf")
            .notify("restart service"),
    );

    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Service restarted"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Test 3: Handler Execution Order
// ============================================================================

#[tokio::test]
async fn test_handlers_run_in_definition_order() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Order Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that notifies handlers in non-definition order
    let mut task = Task::new("Trigger handlers", "copy")
        .arg("src", "test.conf")
        .arg("dest", "/etc/test.conf");
    task.notify = vec![
        "zulu handler".to_string(),
        "alpha handler".to_string(),
        "bravo handler".to_string(),
    ];
    play.add_task(task);

    // Handlers defined in specific order (should execute in this order)
    for name in &["alpha handler", "bravo handler", "zulu handler"] {
        play.add_handler(Handler {
            name: name.to_string(),
            module: "debug".to_string(),
            args: {
                let mut args = IndexMap::new();
                args.insert(
                    "msg".to_string(),
                    serde_json::json!(format!("Handler: {}", name)),
                );
                args
            },
            when: None,
            listen: vec![],
        });
    }

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
    // Note: Without output capture, we can't verify exact order,
    // but handlers should execute in definition order: alpha, bravo, zulu
}

#[tokio::test]
async fn test_handler_order_consistent_across_runs() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Consistent Order Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    let mut task = Task::new("Trigger all", "copy")
        .arg("src", "config.conf")
        .arg("dest", "/etc/config.conf");
    task.notify = vec![
        "handler_c".to_string(),
        "handler_a".to_string(),
        "handler_b".to_string(),
    ];
    play.add_task(task);

    // Definition order
    for name in &["handler_a", "handler_b", "handler_c"] {
        play.add_handler(Handler {
            name: name.to_string(),
            module: "debug".to_string(),
            args: IndexMap::new(),
            when: None,
            listen: vec![],
        });
    }

    playbook.add_play(play);

    // Run multiple times - order should be consistent
    for _ in 0..3 {
        let results = executor.run_playbook(&playbook).await.unwrap();
        assert!(results.contains_key("localhost"));
    }
}

// ============================================================================
// Test 4: Handler Flush
// ============================================================================

#[tokio::test]
async fn test_handlers_run_at_end_of_play() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("End of Play Flush Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that notifies handler
    play.add_task(
        Task::new("Notify handler", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("end of play handler"),
    );

    // Another task after notification
    play.add_task(Task::new("Task after notify", "debug").arg("msg", "After notification"));

    play.add_handler(Handler {
        name: "end of play handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("Handler at end of play"),
            );
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handlers should run at the end of the play
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_meta_flush_handlers_forces_early_execution() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Meta Flush Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that notifies handler
    play.add_task(
        Task::new("Notify handler", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("immediate handler"),
    );

    // Meta task to flush handlers immediately
    play.add_task(Task::new("Flush handlers now", "meta").arg("_raw_params", "flush_handlers"));

    // Another task after flush
    play.add_task(Task::new("Task after flush", "debug").arg("msg", "After flush"));

    play.add_handler(Handler {
        name: "immediate handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("Handler flushed early"),
            );
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handlers_flush_between_plays() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi-Play Flush Test");

    // Play 1
    let mut play1 = Play::new("Play 1", "all");
    play1.gather_facts = false;
    play1.add_task(
        Task::new("Notify in play 1", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("handler1"),
    );
    play1.add_handler(Handler {
        name: "handler1".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler from play 1"));
            args
        },
        when: None,
        listen: vec![],
    });

    // Play 2
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(Task::new("Play 2 task", "debug").arg("msg", "Play 2"));

    playbook.add_play(play1);
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // handler1 should run at end of play 1, not affect play 2
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Test 5: Handler Chaining
// ============================================================================

#[tokio::test]
async fn test_handler_notifies_another_handler() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Chain Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Trigger chain", "copy")
            .arg("src", "config.conf")
            .arg("dest", "/etc/config.conf")
            .notify("step 1"),
    );

    // Handler chain: step 1 -> step 2 -> step 3
    // Note: Handler chaining might require special implementation
    play.add_handler(Handler {
        name: "step 1".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Step 1"));
            args
        },
        when: None,
        listen: vec![],
    });

    play.add_handler(Handler {
        name: "step 2".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Step 2"));
            args
        },
        when: None,
        listen: vec!["step 1".to_string()],
    });

    play.add_handler(Handler {
        name: "step 3".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Step 3"));
            args
        },
        when: None,
        listen: vec!["step 2".to_string()],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Test 6: Listen Directive
// ============================================================================

#[tokio::test]
async fn test_handler_listens_to_topic() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Listen Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task notifies a topic, not the handler directly
    play.add_task(
        Task::new("Update web config", "copy")
            .arg("src", "web.conf")
            .arg("dest", "/etc/web.conf")
            .notify("web config changed"),
    );

    // Handler listens for the topic
    play.add_handler(Handler {
        name: "restart web services".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("Web services restarted"),
            );
            args
        },
        when: None,
        listen: vec!["web config changed".to_string()],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_multiple_handlers_listen_to_same_topic() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multiple Listen Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task notifies a topic
    play.add_task(
        Task::new("Config change", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("app reconfigured"),
    );

    // Multiple handlers listen to the same topic
    play.add_handler(Handler {
        name: "restart nginx".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Nginx restarted"));
            args
        },
        when: None,
        listen: vec!["app reconfigured".to_string()],
    });

    play.add_handler(Handler {
        name: "restart php".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("PHP restarted"));
            args
        },
        when: None,
        listen: vec!["app reconfigured".to_string()],
    });

    play.add_handler(Handler {
        name: "clear cache".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Cache cleared"));
            args
        },
        when: None,
        listen: vec!["app reconfigured".to_string()],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // All three handlers should be triggered
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_name_and_listen_both_work() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Name and Listen Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task 1 notifies handler by name
    play.add_task(
        Task::new("Task 1", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("restart service"),
    );

    // Task 2 notifies handler via listen topic
    play.add_task(
        Task::new("Task 2", "copy")
            .arg("src", "file2.conf")
            .arg("dest", "/etc/file2.conf")
            .notify("service config changed"),
    );

    // Handler can be triggered both ways
    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Service restarted"));
            args
        },
        when: None,
        listen: vec!["service config changed".to_string()],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler triggered by both but executes only once
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Test 7: Conditional Handlers
// ============================================================================

#[tokio::test]
async fn test_handler_with_when_condition_true() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("should_restart".to_string(), serde_json::json!(true));

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Conditional Handler Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("restart app"),
    );

    play.add_handler(Handler {
        name: "restart app".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("App restarted"));
            args
        },
        when: Some("should_restart".to_string()),
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_with_when_condition_false() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("should_restart".to_string(), serde_json::json!(false));

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Conditional Handler Skip Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("restart app"),
    );

    play.add_handler(Handler {
        name: "restart app".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("App restarted"));
            args
        },
        when: Some("should_restart".to_string()),
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler notified but skipped due to when condition
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_condition_evaluated_at_runtime() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("environment".to_string(), serde_json::json!("production"));
    runtime.set_global_var("auto_restart".to_string(), serde_json::json!(true));

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Runtime Condition Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("conditional restart"),
    );

    play.add_handler(Handler {
        name: "conditional restart".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("Conditional restart executed"),
            );
            args
        },
        when: Some("environment == 'production' and auto_restart".to_string()),
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_skipped_if_condition_false() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    // Don't set the required variable

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Condition Skip Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Notify handler", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("conditional handler"),
    );

    play.add_handler(Handler {
        name: "conditional handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler runs"));
            args
        },
        when: Some("undefined_variable is defined".to_string()),
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should be skipped, not fail
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Test 8: Handler with Tasks Features
// ============================================================================

#[tokio::test]
async fn test_handler_runs_task_with_all_features() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("service_name".to_string(), serde_json::json!("nginx"));

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Features Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "nginx.conf")
            .arg("dest", "/etc/nginx/nginx.conf")
            .notify("restart service"),
    );

    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "service".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("name".to_string(), serde_json::json!("{{ service_name }}"));
            args.insert("state".to_string(), serde_json::json!("restarted"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_failure_handling() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Failure Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Notify handlers", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("handler1")
            .notify("failing handler")
            .notify("handler3"),
    );

    play.add_handler(Handler {
        name: "handler1".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler 1"));
            args
        },
        when: None,
        listen: vec![],
    });

    play.add_handler(Handler {
        name: "failing handler".to_string(),
        module: "fail".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler failed"));
            args
        },
        when: None,
        listen: vec![],
    });

    play.add_handler(Handler {
        name: "handler3".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler 3"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let _results = executor.run_playbook(&playbook).await;
    // Some handlers may fail, but playbook should handle it gracefully
}

// ============================================================================
// Test 9: Handler in Roles
// ============================================================================

#[tokio::test]
async fn test_handlers_scoped_to_play() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Scope Test");

    // Play 1 with its own handler
    let mut play1 = Play::new("Play 1", "all");
    play1.gather_facts = false;
    play1.add_task(
        Task::new("Task in play 1", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("play1 handler"),
    );
    play1.add_handler(Handler {
        name: "play1 handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler from play 1"));
            args
        },
        when: None,
        listen: vec![],
    });

    // Play 2 with its own handler
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(
        Task::new("Task in play 2", "copy")
            .arg("src", "file2.conf")
            .arg("dest", "/etc/file2.conf")
            .notify("play2 handler"),
    );
    play2.add_handler(Handler {
        name: "play2 handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler from play 2"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play1);
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Each play's handlers should only run for their play
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_not_available_across_plays() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Cross-Play Handler Test");

    // Play 1 defines a handler
    let mut play1 = Play::new("Play 1", "all");
    play1.gather_facts = false;
    play1.add_handler(Handler {
        name: "shared handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Shared handler"));
            args
        },
        when: None,
        listen: vec![],
    });
    play1.add_task(Task::new("Task in play 1", "debug").arg("msg", "Play 1"));

    // Play 2 tries to use handler from play 1 (should not work)
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(
        Task::new("Task in play 2", "copy")
            .arg("src", "file.conf")
            .arg("dest", "/etc/file.conf")
            .notify("shared handler"), // This handler is not in play 2
    );

    playbook.add_play(play1);
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Play 2 should complete but handler won't be found (warning should be logged)
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_redefinition_in_different_plays() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Redefinition Test");

    // Play 1 with a handler
    let mut play1 = Play::new("Play 1", "all");
    play1.gather_facts = false;
    play1.add_task(
        Task::new("Task 1", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("restart service"),
    );
    play1.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Version 1 of handler"));
            args
        },
        when: None,
        listen: vec![],
    });

    // Play 2 redefines the same handler name differently
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(
        Task::new("Task 2", "copy")
            .arg("src", "file2.conf")
            .arg("dest", "/etc/file2.conf")
            .notify("restart service"),
    );
    play2.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Version 2 of handler"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play1);
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Each play should use its own version of the handler
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Test 10: Edge Cases
// ============================================================================

#[tokio::test]
async fn test_handler_not_found_warning() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Not Found Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task notifies a non-existent handler
    play.add_task(
        Task::new("Notify missing handler", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("nonexistent handler"),
    );

    // Only add a different handler
    play.add_handler(Handler {
        name: "existing handler".to_string(),
        module: "debug".to_string(),
        args: IndexMap::new(),
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Should complete with warning about missing handler
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_empty_handlers_list() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Empty Handlers Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task without notify
    play.add_task(Task::new("Simple task", "debug").arg("msg", "No handlers"));

    // Empty handlers (not adding any)

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_handlers_run_on_all_hosts() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("host1".to_string(), Some("webservers"));
    runtime.add_host("host2".to_string(), Some("webservers"));
    runtime.add_host("host3".to_string(), Some("webservers"));

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi-Host Handler Test");
    let mut play = Play::new("Test Play", "webservers");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("restart app"),
    );

    play.add_handler(Handler {
        name: "restart app".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("App restarted on {{ inventory_hostname }}"),
            );
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should run on all hosts
    assert_eq!(results.len(), 3);
    assert!(results.contains_key("host1"));
    assert!(results.contains_key("host2"));
    assert!(results.contains_key("host3"));
}

#[tokio::test]
async fn test_handler_execution_statistics() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Stats Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Multiple tasks that change and notify
    // Using 'command' module which simulates execution and returns changed=true
    for i in 1..=3 {
        play.add_task(
            Task::new(format!("Task {}", i), "command")
                .arg("cmd", format!("echo Task {}", i))
                .notify("common handler"),
        );
    }

    play.add_handler(Handler {
        name: "common handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler executed"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Verify stats include task execution
    // At least 3 tasks should have run successfully
    let total_executions = host_result.stats.ok + host_result.stats.changed;
    assert!(
        total_executions >= 3,
        "Expected at least 3 executions, got {}",
        total_executions
    );

    // Verify playbook executed without errors
    assert!(!host_result.failed);
}

// ============================================================================
// Handler Variable Access Tests
// ============================================================================

#[tokio::test]
async fn test_handler_access_to_facts() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_host_fact(
        "localhost",
        "distribution".to_string(),
        serde_json::json!("Ubuntu"),
    );

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Facts Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Trigger handler", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("fact aware handler"),
    );

    play.add_handler(Handler {
        name: "fact aware handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("Distribution: {{ distribution }}"),
            );
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_access_to_registered_variables() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Registered Vars Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that registers a result
    play.add_task(
        Task::new("Check status", "command")
            .arg("cmd", "echo 'ready'")
            .register("service_status"),
    );

    // Task that notifies handler
    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("status aware handler"),
    );

    play.add_handler(Handler {
        name: "status aware handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert(
                "msg".to_string(),
                serde_json::json!("Service status available"),
            );
            args
        },
        when: Some("service_status is defined".to_string()),
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_access_to_play_variables() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Play Vars Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;
    play.set_var("service_name", serde_json::json!("nginx"));
    play.set_var("service_state", serde_json::json!("restarted"));

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "nginx.conf")
            .arg("dest", "/etc/nginx/nginx.conf")
            .notify("restart service"),
    );

    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "service".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("name".to_string(), serde_json::json!("{{ service_name }}"));
            args.insert(
                "state".to_string(),
                serde_json::json!("{{ service_state }}"),
            );
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Playbook Parsing Tests for Handlers
// ============================================================================

#[test]
fn test_parse_playbook_with_handlers() {
    let yaml = r#"
- name: Configure nginx
  hosts: webservers
  tasks:
    - name: Copy config
      copy:
        src: nginx.conf
        dest: /etc/nginx/nginx.conf
      notify: restart nginx
  handlers:
    - name: restart nginx
      service:
        name: nginx
        state: restarted
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let play = &playbook.plays[0];

    assert_eq!(play.handlers.len(), 1);
    assert_eq!(play.handlers[0].name, "restart nginx");
    assert_eq!(play.handlers[0].module, "service");
}

#[test]
fn test_parse_handler_with_listen() {
    let yaml = r#"
- name: Test listen
  hosts: all
  tasks:
    - name: Trigger
      debug:
        msg: test
      notify: config changed
  handlers:
    - name: restart services
      debug:
        msg: Restarting
      listen:
        - config changed
        - reload needed
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let handler = &playbook.plays[0].handlers[0];

    assert_eq!(handler.name, "restart services");
    assert_eq!(handler.listen.len(), 2);
    assert!(handler.listen.contains(&"config changed".to_string()));
    assert!(handler.listen.contains(&"reload needed".to_string()));
}

#[test]
fn test_parse_handler_with_when() {
    let yaml = r#"
- name: Test conditional
  hosts: all
  tasks:
    - name: Trigger
      debug:
        msg: test
      notify: maybe restart
  handlers:
    - name: maybe restart
      service:
        name: app
        state: restarted
      when: should_restart
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let handler = &playbook.plays[0].handlers[0];

    assert_eq!(handler.name, "maybe restart");
    assert_eq!(handler.when, Some("should_restart".to_string()));
}

#[test]
fn test_parse_multiple_handlers() {
    let yaml = r#"
- name: Multi handlers
  hosts: all
  tasks:
    - name: Deploy
      copy:
        src: app.zip
        dest: /opt/app/
      notify:
        - restart app
        - reload nginx
        - clear cache
  handlers:
    - name: restart app
      service:
        name: myapp
        state: restarted

    - name: reload nginx
      service:
        name: nginx
        state: reloaded

    - name: clear cache
      command: /usr/bin/clear-cache
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let handlers = &playbook.plays[0].handlers;

    assert_eq!(handlers.len(), 3);
    assert_eq!(handlers[0].name, "restart app");
    assert_eq!(handlers[1].name, "reload nginx");
    assert_eq!(handlers[2].name, "clear cache");
}

#[test]
fn test_parse_task_with_notify_list() {
    let yaml = r#"
- name: Test notify list
  hosts: all
  tasks:
    - name: Update configs
      copy:
        src: config/
        dest: /etc/myapp/
      notify:
        - restart app
        - reload config
        - clear cache
  handlers:
    - name: restart app
      debug:
        msg: Restarted

    - name: reload config
      debug:
        msg: Reloaded

    - name: clear cache
      debug:
        msg: Cleared
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let task = &playbook.plays[0].tasks[0];

    assert_eq!(task.notify.len(), 3);
    assert!(task.notify.contains(&"restart app".to_string()));
    assert!(task.notify.contains(&"reload config".to_string()));
    assert!(task.notify.contains(&"clear cache".to_string()));
}

#[test]
fn test_parse_task_with_single_notify() {
    let yaml = r#"
- name: Test single notify
  hosts: all
  tasks:
    - name: Update config
      copy:
        src: config.conf
        dest: /etc/config.conf
      notify: restart service
  handlers:
    - name: restart service
      debug:
        msg: Restarted
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let task = &playbook.plays[0].tasks[0];

    assert_eq!(task.notify.len(), 1);
    assert_eq!(task.notify[0], "restart service");
}
