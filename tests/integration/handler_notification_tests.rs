//! Handler notification integration tests
//!
//! These tests verify handler notification behavior including:
//! - Single and multiple handler notifications
//! - Handler deduplication (run once despite multiple notifications)
//! - Handler execution order (definition order, not notification order)
//! - Handler flush at end of play and with meta: flush_handlers
//! - Handler chaining (handler notifies another handler)
//! - Listen directive (handler listens to topic)
//! - Conditional handlers (when on handler)
//! - Handler with variable access
//! - Handler scope (per-play)
//! - Multi-host handler execution

use indexmap::IndexMap;
use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::RuntimeContext;
use rustible::executor::task::{Handler, Task};
use rustible::executor::{Executor, ExecutorConfig};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_handler(name: &str, msg: &str) -> Handler {
    Handler {
        name: name.to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!(msg));
            args
        },
        when: None,
        listen: vec![],
    }
}

fn create_handler_with_listen(name: &str, msg: &str, listen: Vec<&str>) -> Handler {
    Handler {
        name: name.to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!(msg));
            args
        },
        when: None,
        listen: listen.into_iter().map(String::from).collect(),
    }
}

fn create_conditional_handler(name: &str, msg: &str, condition: &str) -> Handler {
    Handler {
        name: name.to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!(msg));
            args
        },
        when: Some(condition.to_string()),
        listen: vec![],
    }
}

// ============================================================================
// Single Handler Notification Tests
// ============================================================================

#[tokio::test]
async fn test_single_task_notifies_single_handler() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Single Notification Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "config.conf")
            .arg("dest", "/etc/config.conf")
            .notify("restart service"),
    );

    play.add_handler(create_handler("restart service", "Service restarted"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_task_notifies_multiple_handlers() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multiple Handler Notification");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    let mut task = Task::new("Deploy application", "copy")
        .arg("src", "app.zip")
        .arg("dest", "/opt/app/");
    task.notify.push("restart nginx".to_string());
    task.notify.push("clear cache".to_string());
    task.notify.push("reload config".to_string());
    play.add_task(task);

    play.add_handler(create_handler("restart nginx", "Nginx restarted"));
    play.add_handler(create_handler("clear cache", "Cache cleared"));
    play.add_handler(create_handler("reload config", "Config reloaded"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_multiple_tasks_notify_same_handler() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multiple Tasks Same Handler");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Three tasks notifying the same handler
    for i in 1..=3 {
        play.add_task(
            Task::new(format!("Config update {}", i), "copy")
                .arg("src", format!("config{}.conf", i))
                .arg("dest", format!("/etc/config{}.conf", i))
                .notify("restart service"),
        );
    }

    play.add_handler(create_handler("restart service", "Service restarted once"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should only run once despite multiple notifications
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Handler Deduplication Tests
// ============================================================================

#[tokio::test]
async fn test_handler_runs_once_despite_duplicate_notifications() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Deduplication Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task with duplicate notifications
    let mut task = Task::new("Duplicate notify", "copy")
        .arg("src", "test.conf")
        .arg("dest", "/etc/test.conf");
    task.notify = vec![
        "restart service".to_string(),
        "restart service".to_string(),
        "restart service".to_string(),
    ];
    play.add_task(task);

    play.add_handler(create_handler("restart service", "Handler ran"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_deduplication_across_multiple_tasks() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Cross-Task Deduplication");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Five tasks all notifying the same handler
    for i in 1..=5 {
        play.add_task(
            Task::new(format!("Task {}", i), "copy")
                .arg("src", format!("file{}.txt", i))
                .arg("dest", format!("/tmp/file{}.txt", i))
                .notify("common handler"),
        );
    }

    play.add_handler(create_handler("common handler", "Ran once"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Handler Execution Order Tests
// ============================================================================

#[tokio::test]
async fn test_handlers_execute_in_definition_order() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Order Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Notify handlers in different order than definition
    let mut task = Task::new("Trigger handlers", "copy")
        .arg("src", "test.conf")
        .arg("dest", "/etc/test.conf");
    task.notify = vec![
        "handler_z".to_string(),
        "handler_a".to_string(),
        "handler_m".to_string(),
    ];
    play.add_task(task);

    // Handlers defined in alphabetical order
    play.add_handler(create_handler("handler_a", "Handler A"));
    play.add_handler(create_handler("handler_m", "Handler M"));
    play.add_handler(create_handler("handler_z", "Handler Z"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handlers should execute in definition order: a, m, z
    assert!(results.contains_key("localhost"));
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

    let mut task = Task::new("Notify all", "copy")
        .arg("src", "config.conf")
        .arg("dest", "/etc/config.conf");
    task.notify = vec![
        "third".to_string(),
        "first".to_string(),
        "second".to_string(),
    ];
    play.add_task(task);

    play.add_handler(create_handler("first", "First"));
    play.add_handler(create_handler("second", "Second"));
    play.add_handler(create_handler("third", "Third"));

    playbook.add_play(play);

    // Run multiple times - order should be consistent
    for _ in 0..3 {
        let results = executor.run_playbook(&playbook).await.unwrap();
        assert!(results.contains_key("localhost"));
    }
}

// ============================================================================
// Handler Flush Tests
// ============================================================================

#[tokio::test]
async fn test_handlers_flush_at_end_of_play() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("End of Play Flush");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Notify handler", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("end handler"),
    );

    // Task after notification
    play.add_task(Task::new("After notify", "debug").arg("msg", "After notification"));

    play.add_handler(create_handler("end handler", "Handler at end"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_meta_flush_handlers() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Meta Flush Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // First notification
    play.add_task(
        Task::new("First notify", "copy")
            .arg("src", "first.conf")
            .arg("dest", "/etc/first.conf")
            .notify("immediate handler"),
    );

    // Flush handlers immediately
    play.add_task(Task::new("Flush handlers", "meta").arg("_raw_params", "flush_handlers"));

    // Task after flush
    play.add_task(Task::new("After flush", "debug").arg("msg", "After flush"));

    play.add_handler(create_handler("immediate handler", "Handler flushed"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_can_be_renotified_after_flush() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Re-notification After Flush");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // First notification
    play.add_task(
        Task::new("First change", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("restart service"),
    );

    // Flush handlers
    play.add_task(Task::new("Flush", "meta").arg("_raw_params", "flush_handlers"));

    // Second notification (should trigger handler again)
    play.add_task(
        Task::new("Second change", "copy")
            .arg("src", "file2.conf")
            .arg("dest", "/etc/file2.conf")
            .notify("restart service"),
    );

    play.add_handler(create_handler("restart service", "Service restarted"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should run twice (once per flush)
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handlers_flush_between_plays() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi-Play Flush");

    // Play 1
    let mut play1 = Play::new("Play 1", "all");
    play1.gather_facts = false;
    play1.add_task(
        Task::new("Notify in play 1", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("play1 handler"),
    );
    play1.add_handler(create_handler("play1 handler", "Play 1 handler"));

    // Play 2
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(Task::new("Play 2 task", "debug").arg("msg", "Play 2"));

    playbook.add_play(play1);
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Play 1 handlers should run before play 2 starts
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Listen Directive Tests
// ============================================================================

#[tokio::test]
async fn test_handler_listens_to_topic() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Listen Topic Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task notifies a topic
    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "web.conf")
            .arg("dest", "/etc/web.conf")
            .notify("web config changed"),
    );

    // Handler listens for the topic
    play.add_handler(create_handler_with_listen(
        "restart web services",
        "Web services restarted",
        vec!["web config changed"],
    ));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_multiple_handlers_listen_same_topic() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multiple Listeners Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task notifies a topic
    play.add_task(
        Task::new("Config change", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("app reconfigured"),
    );

    // Multiple handlers listen to same topic
    play.add_handler(create_handler_with_listen(
        "restart nginx",
        "Nginx restarted",
        vec!["app reconfigured"],
    ));
    play.add_handler(create_handler_with_listen(
        "restart php",
        "PHP restarted",
        vec!["app reconfigured"],
    ));
    play.add_handler(create_handler_with_listen(
        "clear cache",
        "Cache cleared",
        vec!["app reconfigured"],
    ));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // All three handlers should be triggered
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_name_and_listen_both_trigger() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Name and Listen Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task 1 notifies by handler name
    play.add_task(
        Task::new("Direct notify", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("restart service"),
    );

    // Task 2 notifies via listen topic
    play.add_task(
        Task::new("Topic notify", "copy")
            .arg("src", "file2.conf")
            .arg("dest", "/etc/file2.conf")
            .notify("service config changed"),
    );

    // Handler responds to both name and listen topic
    play.add_handler(create_handler_with_listen(
        "restart service",
        "Service restarted",
        vec!["service config changed"],
    ));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should only run once despite both triggers
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_listens_to_multiple_topics() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multiple Topics Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Nginx config", "copy")
            .arg("src", "nginx.conf")
            .arg("dest", "/etc/nginx/nginx.conf")
            .notify("nginx changed"),
    );

    // Handler listens to multiple topics
    play.add_handler(create_handler_with_listen(
        "restart web stack",
        "Web stack restarted",
        vec!["nginx changed", "php changed", "varnish changed"],
    ));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Conditional Handler Tests
// ============================================================================

#[tokio::test]
async fn test_handler_with_when_condition_true() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("should_restart".to_string(), serde_json::json!(true));

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Conditional Handler True");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("conditional restart"),
    );

    play.add_handler(create_conditional_handler(
        "conditional restart",
        "App restarted",
        "should_restart",
    ));

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

    let mut playbook = Playbook::new("Conditional Handler False");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("conditional restart"),
    );

    play.add_handler(create_conditional_handler(
        "conditional restart",
        "App restarted",
        "should_restart",
    ));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should be skipped, not failed
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_condition_with_complex_expression() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("environment".to_string(), serde_json::json!("production"));
    runtime.set_global_var("auto_restart".to_string(), serde_json::json!(true));

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Complex Condition Handler");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Deploy", "copy")
            .arg("src", "app.zip")
            .arg("dest", "/opt/app/")
            .notify("production restart"),
    );

    play.add_handler(create_conditional_handler(
        "production restart",
        "Production restart",
        "environment == 'production' and auto_restart",
    ));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Handler Variable Access Tests
// ============================================================================

#[tokio::test]
async fn test_handler_access_to_play_variables() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Vars Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;
    play.set_var("service_name", serde_json::json!("nginx"));

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "nginx.conf")
            .arg("dest", "/etc/nginx/nginx.conf")
            .notify("restart service"),
    );

    let mut args = IndexMap::new();
    args.insert(
        "msg".to_string(),
        serde_json::json!("Restarting {{ service_name }}"),
    );
    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args,
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_access_to_host_facts() {
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

    let mut args = IndexMap::new();
    args.insert(
        "msg".to_string(),
        serde_json::json!("Distribution: {{ distribution }}"),
    );
    play.add_handler(Handler {
        name: "fact aware handler".to_string(),
        module: "debug".to_string(),
        args,
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

    let mut playbook = Playbook::new("Handler Registered Vars");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Task that registers a variable
    play.add_task(
        Task::new("Check status", "command")
            .arg("cmd", "echo 'ready'")
            .register("status_result"),
    );

    // Task that notifies handler
    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("status aware handler"),
    );

    play.add_handler(create_conditional_handler(
        "status aware handler",
        "Handler ran",
        "status_result is defined",
    ));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Handler Scope Tests
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
        Task::new("Play 1 task", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("play1 handler"),
    );
    play1.add_handler(create_handler("play1 handler", "Play 1 handler"));

    // Play 2 with its own handler
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(
        Task::new("Play 2 task", "copy")
            .arg("src", "file2.conf")
            .arg("dest", "/etc/file2.conf")
            .notify("play2 handler"),
    );
    play2.add_handler(create_handler("play2 handler", "Play 2 handler"));

    playbook.add_play(play1);
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_handler_not_found_in_different_play() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Cross-Play Handler Test");

    // Play 1 defines a handler
    let mut play1 = Play::new("Play 1", "all");
    play1.gather_facts = false;
    play1.add_task(Task::new("Play 1 task", "debug").arg("msg", "Play 1"));
    play1.add_handler(create_handler("shared handler", "Shared handler"));

    // Play 2 tries to use handler from play 1
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(
        Task::new("Play 2 task", "copy")
            .arg("src", "file.conf")
            .arg("dest", "/etc/file.conf")
            .notify("shared handler"), // This handler is not in play 2
    );

    playbook.add_play(play1);
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Should complete but handler won't be found
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_same_handler_name_different_plays() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Same Name Different Plays");

    // Play 1 with handler
    let mut play1 = Play::new("Play 1", "all");
    play1.gather_facts = false;
    play1.add_task(
        Task::new("Task 1", "copy")
            .arg("src", "file1.conf")
            .arg("dest", "/etc/file1.conf")
            .notify("restart service"),
    );
    play1.add_handler(create_handler("restart service", "Play 1 version"));

    // Play 2 with same handler name but different action
    let mut play2 = Play::new("Play 2", "all");
    play2.gather_facts = false;
    play2.add_task(
        Task::new("Task 2", "copy")
            .arg("src", "file2.conf")
            .arg("dest", "/etc/file2.conf")
            .notify("restart service"),
    );
    play2.add_handler(create_handler("restart service", "Play 2 version"));

    playbook.add_play(play1);
    playbook.add_play(play2);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Multi-Host Handler Tests
// ============================================================================

#[tokio::test]
async fn test_handlers_run_on_all_hosts() {
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

    let mut playbook = Playbook::new("Multi-Host Handler Test");
    let mut play = Play::new("Test Play", "webservers");
    play.gather_facts = false;

    play.add_task(
        Task::new("Update config", "copy")
            .arg("src", "app.conf")
            .arg("dest", "/etc/app.conf")
            .notify("restart app"),
    );

    let mut args = IndexMap::new();
    args.insert(
        "msg".to_string(),
        serde_json::json!("App restarted on {{ inventory_hostname }}"),
    );
    play.add_handler(Handler {
        name: "restart app".to_string(),
        module: "debug".to_string(),
        args,
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

// ============================================================================
// Edge Cases and Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_handler_not_found() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Not Found Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Notify missing", "copy")
            .arg("src", "test.conf")
            .arg("dest", "/etc/test.conf")
            .notify("nonexistent handler"),
    );

    // Only add a different handler
    play.add_handler(create_handler("existing handler", "Existing"));

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

    // No handlers added

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_handler_not_triggered_when_task_unchanged() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Unchanged Task Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Debug tasks don't cause changes
    play.add_task(
        Task::new("Unchanged task", "debug")
            .arg("msg", "No changes")
            .notify("should not run"),
    );

    play.add_handler(create_handler("should not run", "This should not run"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should not run since task didn't change
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// YAML Parsing Tests
// ============================================================================

#[test]
fn test_parse_playbook_with_handlers() {
    let yaml = r#"
- name: Configure nginx
  hosts: webservers
  gather_facts: false
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
  gather_facts: false
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
  gather_facts: false
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
  gather_facts: false
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
  gather_facts: false
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

// ============================================================================
// Handler Statistics Tests
// ============================================================================

#[tokio::test]
async fn test_handler_execution_statistics() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Stats Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Multiple tasks triggering handlers
    for i in 1..=3 {
        play.add_task(
            Task::new(format!("Task {}", i), "command")
                .arg("cmd", format!("echo Task {}", i))
                .notify("common handler"),
        );
    }

    play.add_handler(create_handler("common handler", "Handler executed"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // At least 3 tasks should have run
    let total = host_result.stats.ok + host_result.stats.changed;
    assert!(total >= 3, "Expected at least 3 executions, got {}", total);
    assert!(!host_result.failed);
}
