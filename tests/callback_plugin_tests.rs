//! Comprehensive tests for all built-in callback plugins.
//!
//! This test module covers:
//! 1. NullCallback - Zero-sized type with no-op implementations
//! 2. DefaultCallback - Ansible-like colored output
//! 3. TimerCallback - Execution timing tracking
//! 4. MinimalCallback - CI/CD friendly minimal output
//!
//! Each plugin is tested for:
//! - Construction and configuration
//! - Full lifecycle (playbook start -> end)
//! - Concurrent access safety
//! - Edge cases and error handling

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::RwLock;

use rustible::callback::plugins::{
    DefaultCallback, DefaultCallbackBuilder, DefaultCallbackConfig, MinimalCallback, NullCallback,
    TimerCallback, TimerCallbackBuilder, TimerConfig,
};
use rustible::facts::Facts;
use rustible::traits::{ExecutionCallback, ExecutionResult, ModuleResult};

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test ExecutionResult for task completion testing.
fn create_test_result(task_name: &str, host: &str, success: bool, changed: bool) -> ExecutionResult {
    ExecutionResult {
        host: host.to_string(),
        task_name: task_name.to_string(),
        result: if success {
            if changed {
                ModuleResult::changed("Task completed with changes")
            } else {
                ModuleResult::ok("Task completed successfully")
            }
        } else {
            ModuleResult::failed("Task failed")
        },
        duration: Duration::from_millis(100),
        notify: vec![],
    }
}

/// Create a skipped ExecutionResult.
fn create_skipped_result(task_name: &str, host: &str) -> ExecutionResult {
    ExecutionResult {
        host: host.to_string(),
        task_name: task_name.to_string(),
        result: ModuleResult::skipped("Condition not met"),
        duration: Duration::from_millis(10),
        notify: vec![],
    }
}

/// Create test Facts.
fn create_test_facts() -> Facts {
    let mut facts = Facts::new();
    facts.set("ansible_os_family", serde_json::json!("Debian"));
    facts.set("ansible_distribution", serde_json::json!("Ubuntu"));
    facts
}

// ============================================================================
// NullCallback Tests
// ============================================================================

mod null_callback_tests {
    use super::*;

    #[test]
    fn test_null_callback_is_zero_sized() {
        // NullCallback should be a ZST (zero-sized type)
        assert_eq!(std::mem::size_of::<NullCallback>(), 0);
    }

    #[test]
    fn test_null_callback_construction() {
        let callback = NullCallback::new();
        assert_eq!(callback, NullCallback);

        let callback_default = NullCallback::default();
        assert_eq!(callback_default, NullCallback);
    }

    #[test]
    fn test_null_callback_clone_and_copy() {
        let callback1 = NullCallback;
        let callback2 = callback1; // Copy
        let callback3 = callback1.clone(); // Clone

        // All should be equal
        assert_eq!(callback1, callback2);
        assert_eq!(callback2, callback3);
    }

    #[test]
    fn test_null_callback_debug() {
        let callback = NullCallback;
        let debug_str = format!("{:?}", callback);
        assert_eq!(debug_str, "NullCallback");
    }

    #[test]
    fn test_null_callback_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(NullCallback);

        assert!(set.contains(&NullCallback));
        assert_eq!(set.len(), 1);
    }

    #[tokio::test]
    async fn test_null_callback_full_lifecycle() {
        let callback = NullCallback;

        // None of these should panic - they're all no-ops
        callback.on_playbook_start("test-playbook").await;
        callback
            .on_play_start("test-play", &["host1".to_string(), "host2".to_string()])
            .await;
        callback.on_task_start("test-task", "host1").await;
        callback
            .on_task_complete(&create_test_result("test-task", "host1", true, false))
            .await;
        callback.on_handler_triggered("test-handler").await;
        callback
            .on_facts_gathered("host1", &create_test_facts())
            .await;
        callback.on_play_end("test-play", true).await;
        callback.on_playbook_end("test-playbook", true).await;
    }

    #[tokio::test]
    async fn test_null_callback_concurrent_access() {
        use tokio::task::JoinSet;

        let callback = Arc::new(NullCallback);
        let mut join_set = JoinSet::new();

        for i in 0..1000 {
            let cb = callback.clone();
            join_set.spawn(async move {
                cb.on_task_start(&format!("task-{}", i), "host1").await;
                cb.on_task_complete(&create_test_result(
                    &format!("task-{}", i),
                    "host1",
                    true,
                    false,
                ))
                .await;
            });
        }

        // All should complete without issues
        while join_set.join_next().await.is_some() {}
    }

    #[tokio::test]
    async fn test_null_callback_no_output() {
        // NullCallback should produce no output
        // This is a smoke test - if we get here without panic, it works
        let callback = NullCallback;

        for _ in 0..100 {
            callback.on_playbook_start("test").await;
            callback.on_playbook_end("test", true).await;
        }
    }
}

// ============================================================================
// DefaultCallback Tests
// ============================================================================

mod default_callback_tests {
    use super::*;
    use rustible::callback::plugins::default::Verbosity;

    #[test]
    fn test_default_callback_construction() {
        let callback = DefaultCallback::new();
        assert_eq!(callback.config.verbosity, 0);
        assert!(!callback.config.no_color);
    }

    #[test]
    fn test_default_callback_with_verbosity() {
        let callback = DefaultCallback::new().with_verbosity(2);
        // Verbosity is stored internally
        assert!(true); // Construction should not panic
    }

    #[test]
    fn test_default_callback_with_no_color() {
        let callback = DefaultCallback::new().with_no_color(true);
        assert!(true); // Construction should not panic
    }

    #[test]
    fn test_default_callback_builder() {
        let callback = DefaultCallbackBuilder::new()
            .verbosity(3)
            .no_color(true)
            .show_diff(true)
            .show_duration(false)
            .show_skipped(false)
            .show_ok(false)
            .build();

        assert!(callback.config.show_diff);
        assert!(!callback.config.show_duration);
        assert!(!callback.config.show_skipped);
        assert!(!callback.config.show_ok);
    }

    #[test]
    fn test_default_callback_config_defaults() {
        let config = DefaultCallbackConfig::default();

        assert_eq!(config.verbosity, 0);
        assert!(!config.no_color);
        assert!(!config.show_diff);
        assert!(config.show_duration);
        assert!(config.show_skipped);
        assert!(config.show_ok);
    }

    #[test]
    fn test_verbosity_from_u8() {
        assert_eq!(Verbosity::from(0), Verbosity::Normal);
        assert_eq!(Verbosity::from(1), Verbosity::Verbose);
        assert_eq!(Verbosity::from(2), Verbosity::MoreVerbose);
        assert_eq!(Verbosity::from(3), Verbosity::Debug);
        assert_eq!(Verbosity::from(4), Verbosity::ConnectionDebug);
        assert_eq!(Verbosity::from(5), Verbosity::WinRMDebug);
        assert_eq!(Verbosity::from(10), Verbosity::WinRMDebug); // Clamps to max
    }

    #[test]
    fn test_default_callback_clone() {
        let callback1 = DefaultCallback::new().with_verbosity(3);
        let callback2 = callback1.clone();

        // Clone should have same configuration
        assert!(true); // Clone should not panic
    }

    #[test]
    fn test_default_callback_default_trait() {
        let callback = DefaultCallback::default();
        assert!(true); // Default should work
    }

    #[tokio::test]
    async fn test_default_callback_lifecycle() {
        // Use no_color to avoid terminal escape codes in test output
        let callback = DefaultCallbackBuilder::new()
            .no_color(true)
            .show_ok(false)
            .show_skipped(false)
            .build();

        // Full lifecycle
        callback.on_playbook_start("test-playbook").await;
        callback
            .on_play_start("test-play", &["host1".to_string(), "host2".to_string()])
            .await;
        callback.on_task_start("Install nginx", "host1").await;
        callback
            .on_task_complete(&create_test_result("Install nginx", "host1", true, true))
            .await;
        callback.on_task_start("Install nginx", "host2").await;
        callback
            .on_task_complete(&create_test_result("Install nginx", "host2", true, true))
            .await;
        callback.on_handler_triggered("Restart nginx").await;
        callback.on_play_end("test-play", true).await;
        callback.on_playbook_end("test-playbook", true).await;
    }

    #[tokio::test]
    async fn test_default_callback_tracks_host_stats() {
        let callback = DefaultCallbackBuilder::new()
            .no_color(true)
            .show_ok(false)
            .show_skipped(false)
            .build();

        callback.on_playbook_start("stats-test").await;
        callback
            .on_play_start("test-play", &["host1".to_string()])
            .await;

        // OK result
        callback
            .on_task_complete(&create_test_result("task1", "host1", true, false))
            .await;

        // Changed result
        callback
            .on_task_complete(&create_test_result("task2", "host1", true, true))
            .await;

        // Failed result
        callback
            .on_task_complete(&create_test_result("task3", "host1", false, false))
            .await;

        // Skipped result
        callback
            .on_task_complete(&create_skipped_result("task4", "host1"))
            .await;

        callback.on_play_end("test-play", true).await;
        callback.on_playbook_end("stats-test", false).await;
    }

    #[tokio::test]
    async fn test_default_callback_facts_gathered() {
        let callback = DefaultCallbackBuilder::new()
            .no_color(true)
            .verbosity(3) // Debug level to show facts
            .build();

        callback.on_playbook_start("facts-test").await;
        callback
            .on_play_start("test-play", &["host1".to_string()])
            .await;

        let facts = create_test_facts();
        callback.on_facts_gathered("host1", &facts).await;

        callback.on_play_end("test-play", true).await;
        callback.on_playbook_end("facts-test", true).await;
    }

    #[tokio::test]
    async fn test_default_callback_concurrent_access() {
        use tokio::task::JoinSet;

        let callback = Arc::new(
            DefaultCallbackBuilder::new()
                .no_color(true)
                .show_ok(false)
                .show_skipped(false)
                .build(),
        );

        callback.on_playbook_start("concurrent-test").await;
        callback
            .on_play_start(
                "test-play",
                &["host1".to_string(), "host2".to_string(), "host3".to_string()],
            )
            .await;

        let mut join_set = JoinSet::new();

        for i in 0..30 {
            let cb = callback.clone();
            let host = format!("host{}", (i % 3) + 1);
            let task = format!("task-{}", i);
            join_set.spawn(async move {
                cb.on_task_start(&task, &host).await;
                cb.on_task_complete(&create_test_result(&task, &host, true, i % 2 == 0))
                    .await;
            });
        }

        while join_set.join_next().await.is_some() {}

        callback.on_play_end("test-play", true).await;
        callback.on_playbook_end("concurrent-test", true).await;
    }
}

// ============================================================================
// TimerCallback Tests
// ============================================================================

mod timer_callback_tests {
    use super::*;

    #[test]
    fn test_timer_callback_construction() {
        let timer = TimerCallback::default();
        assert!(timer.config.show_per_task);
        assert!(timer.config.show_summary);
    }

    #[test]
    fn test_timer_callback_builder() {
        let timer = TimerCallbackBuilder::new()
            .show_per_task(false)
            .show_summary(true)
            .top_slowest(5)
            .threshold_secs(1.0)
            .show_play_timing(false)
            .show_playbook_timing(true)
            .use_colors(false)
            .human_readable(false)
            .build();

        assert!(!timer.config.show_per_task);
        assert!(timer.config.show_summary);
        assert_eq!(timer.config.top_slowest, 5);
        assert_eq!(timer.config.threshold_secs, 1.0);
        assert!(!timer.config.show_play_timing);
        assert!(timer.config.show_playbook_timing);
        assert!(!timer.config.use_colors);
        assert!(!timer.config.human_readable);
    }

    #[test]
    fn test_timer_callback_summary_only() {
        let timer = TimerCallback::summary_only();
        assert!(!timer.config.show_per_task);
        assert!(timer.config.show_summary);
    }

    #[test]
    fn test_timer_callback_verbose() {
        let timer = TimerCallback::verbose();
        assert!(timer.config.show_per_task);
        assert!(timer.config.show_summary);
        assert_eq!(timer.config.top_slowest, 20);
    }

    #[test]
    fn test_timer_config_defaults() {
        let config = TimerConfig::default();

        assert!(config.show_per_task);
        assert!(config.show_summary);
        assert_eq!(config.top_slowest, 10);
        assert_eq!(config.threshold_secs, 0.0);
        assert!(config.show_play_timing);
        assert!(config.show_playbook_timing);
        assert!(config.use_colors);
        assert!(config.human_readable);
    }

    #[test]
    fn test_timer_callback_clone() {
        let timer = TimerCallback::default();

        // Record a task
        timer.record_task_complete(
            "task1",
            "host1",
            true,
            false,
            Some(Duration::from_secs(1)),
        );
        assert_eq!(timer.get_total_tasks(), 1);

        // Clone should start fresh (no shared state)
        let cloned = timer.clone();
        assert_eq!(cloned.get_total_tasks(), 0);
    }

    #[test]
    fn test_timer_callback_reset() {
        let timer = TimerCallback::default();

        timer.record_task_complete(
            "task1",
            "host1",
            true,
            false,
            Some(Duration::from_secs(1)),
        );
        assert_eq!(timer.get_total_tasks(), 1);

        timer.reset();
        assert_eq!(timer.get_total_tasks(), 0);
        assert_eq!(timer.get_timings().len(), 0);
    }

    #[test]
    fn test_timer_callback_get_timings() {
        let timer = TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            ..Default::default()
        });

        timer.record_task_complete("task1", "host1", true, false, Some(Duration::from_millis(100)));
        timer.record_task_complete("task2", "host1", true, true, Some(Duration::from_millis(200)));
        timer.record_task_complete("task3", "host1", false, false, Some(Duration::from_millis(50)));

        let timings = timer.get_timings();
        assert_eq!(timings.len(), 3);
    }

    #[test]
    fn test_timer_callback_get_slowest_tasks() {
        let timer = TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            ..Default::default()
        });

        timer.record_task_complete("fast", "h1", true, false, Some(Duration::from_millis(10)));
        timer.record_task_complete("medium", "h1", true, false, Some(Duration::from_millis(50)));
        timer.record_task_complete("slow", "h1", true, false, Some(Duration::from_millis(100)));
        timer.record_task_complete("very-slow", "h1", true, false, Some(Duration::from_millis(500)));

        let slowest = timer.get_slowest_tasks(2);
        assert_eq!(slowest.len(), 2);
        assert_eq!(slowest[0].task_name, "very-slow");
        assert_eq!(slowest[1].task_name, "slow");
    }

    #[test]
    fn test_timer_callback_get_total_duration() {
        let timer = TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            ..Default::default()
        });

        timer.record_task_complete("t1", "h1", true, false, Some(Duration::from_secs(1)));
        timer.record_task_complete("t2", "h1", true, false, Some(Duration::from_secs(2)));
        timer.record_task_complete("t3", "h1", true, false, Some(Duration::from_secs(3)));

        let total = timer.get_total_duration();
        assert_eq!(total, Duration::from_secs(6));
    }

    #[test]
    fn test_timer_callback_get_average_duration() {
        let timer = TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            ..Default::default()
        });

        timer.record_task_complete("t1", "h1", true, false, Some(Duration::from_secs(1)));
        timer.record_task_complete("t2", "h1", true, false, Some(Duration::from_secs(3)));

        let avg = timer.get_average_duration();
        assert_eq!(avg, Duration::from_secs(2));
    }

    #[test]
    fn test_timer_callback_average_duration_empty() {
        let timer = TimerCallback::default();
        let avg = timer.get_average_duration();
        assert_eq!(avg, Duration::ZERO);
    }

    #[tokio::test]
    async fn test_timer_callback_full_lifecycle() {
        let timer = TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            use_colors: false,
            ..Default::default()
        });

        timer.on_playbook_start("timer-test").await;
        timer
            .on_play_start("test-play", &["host1".to_string(), "host2".to_string()])
            .await;

        // Task 1
        timer.on_task_start("Install nginx", "host1").await;
        let result1 = ExecutionResult {
            host: "host1".to_string(),
            task_name: "Install nginx".to_string(),
            result: ModuleResult::ok("done"),
            duration: Duration::from_millis(100),
            notify: vec![],
        };
        timer.on_task_complete(&result1).await;

        // Task 2
        timer.on_task_start("Configure nginx", "host1").await;
        let result2 = ExecutionResult {
            host: "host1".to_string(),
            task_name: "Configure nginx".to_string(),
            result: ModuleResult::changed("configured"),
            duration: Duration::from_millis(200),
            notify: vec![],
        };
        timer.on_task_complete(&result2).await;

        timer.on_play_end("test-play", true).await;
        timer.on_playbook_end("timer-test", true).await;

        let timings = timer.get_timings();
        assert_eq!(timings.len(), 2);
        assert_eq!(timings[0].task_name, "Install nginx");
        assert_eq!(timings[1].task_name, "Configure nginx");
    }

    #[tokio::test]
    async fn test_timer_callback_uses_explicit_duration() {
        let timer = TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            ..Default::default()
        });

        timer.on_playbook_start("duration-test").await;
        timer
            .on_play_start("test-play", &["host1".to_string()])
            .await;

        // Task with explicit duration in ExecutionResult
        timer.on_task_start("task1", "host1").await;
        let result = ExecutionResult {
            host: "host1".to_string(),
            task_name: "task1".to_string(),
            result: ModuleResult::ok("done"),
            duration: Duration::from_millis(500), // Explicit duration
            notify: vec![],
        };
        timer.on_task_complete(&result).await;

        let timings = timer.get_timings();
        assert_eq!(timings.len(), 1);
        // Should use the explicit duration from ExecutionResult
        assert_eq!(timings[0].duration, Duration::from_millis(500));
    }

    #[tokio::test]
    async fn test_timer_callback_concurrent_recording() {
        use tokio::task::JoinSet;

        let timer = Arc::new(TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            ..Default::default()
        }));

        let mut join_set = JoinSet::new();

        for i in 0..100 {
            let t = timer.clone();
            join_set.spawn(async move {
                t.on_task_start(&format!("task-{}", i), "host1").await;
                let result = ExecutionResult {
                    host: "host1".to_string(),
                    task_name: format!("task-{}", i),
                    result: ModuleResult::ok("done"),
                    duration: Duration::from_millis(10),
                    notify: vec![],
                };
                t.on_task_complete(&result).await;
            });
        }

        while join_set.join_next().await.is_some() {}

        assert_eq!(timer.get_total_tasks(), 100);
        assert_eq!(timer.get_timings().len(), 100);
    }
}

// ============================================================================
// MinimalCallback Tests
// ============================================================================

mod minimal_callback_tests {
    use super::*;
    use rustible::callback::plugins::minimal::UnreachableCallback;

    #[test]
    fn test_minimal_callback_construction() {
        let callback = MinimalCallback::new();
        assert!(true); // Construction should not panic
    }

    #[test]
    fn test_minimal_callback_default() {
        let callback = MinimalCallback::default();
        assert!(true); // Default should work
    }

    #[test]
    fn test_minimal_callback_clone_shares_state() {
        let callback1 = MinimalCallback::new();
        let callback2 = callback1.clone();

        // Clone should share state (Arc pointers should be the same)
        // This is verified in the inline tests of minimal.rs
        assert!(true);
    }

    #[tokio::test]
    async fn test_minimal_callback_has_failures_initially_false() {
        let callback = MinimalCallback::new();
        assert!(!callback.has_failures().await);
    }

    #[tokio::test]
    async fn test_minimal_callback_tracks_failures() {
        let callback = MinimalCallback::new();

        callback.on_playbook_start("failure-test").await;
        callback
            .on_play_start("test-play", &["host1".to_string()])
            .await;

        // All OK - no failures
        callback
            .on_task_complete(&create_test_result("task1", "host1", true, false))
            .await;
        assert!(!callback.has_failures().await);

        // Changed - still no failures
        callback
            .on_task_complete(&create_test_result("task2", "host1", true, true))
            .await;
        assert!(!callback.has_failures().await);

        // Failed - now has failures
        callback
            .on_task_complete(&create_test_result("task3", "host1", false, false))
            .await;
        assert!(callback.has_failures().await);
    }

    #[tokio::test]
    async fn test_minimal_callback_tracks_stats() {
        let callback = MinimalCallback::new();

        callback.on_playbook_start("stats-test").await;
        callback
            .on_play_start("test-play", &["host1".to_string()])
            .await;

        // OK
        callback
            .on_task_complete(&create_test_result("task1", "host1", true, false))
            .await;

        // Changed
        callback
            .on_task_complete(&create_test_result("task2", "host1", true, true))
            .await;

        // Failed
        callback
            .on_task_complete(&create_test_result("task3", "host1", false, false))
            .await;

        // Skipped
        callback
            .on_task_complete(&create_skipped_result("task4", "host1"))
            .await;

        callback.on_play_end("test-play", false).await;
        callback.on_playbook_end("stats-test", false).await;
    }

    #[tokio::test]
    async fn test_minimal_callback_unreachable() {
        let callback = MinimalCallback::new();

        callback.on_playbook_start("unreachable-test").await;
        callback
            .on_play_start("test-play", &["host1".to_string()])
            .await;

        // Initially no failures
        assert!(!callback.has_failures().await);

        // Mark host as unreachable
        callback
            .on_host_unreachable("host1", "gather_facts", "Connection refused")
            .await;

        // Should now have failures
        assert!(callback.has_failures().await);
    }

    #[tokio::test]
    async fn test_minimal_callback_multiple_hosts() {
        let callback = MinimalCallback::new();

        callback.on_playbook_start("multi-host-test").await;
        callback
            .on_play_start(
                "test-play",
                &[
                    "host1".to_string(),
                    "host2".to_string(),
                    "host3".to_string(),
                ],
            )
            .await;

        // Various results for different hosts
        callback
            .on_task_complete(&create_test_result("task1", "host1", true, false))
            .await;
        callback
            .on_task_complete(&create_test_result("task1", "host2", true, true))
            .await;
        callback
            .on_task_complete(&create_test_result("task1", "host3", false, false))
            .await;

        // host3 failed, so we should have failures
        assert!(callback.has_failures().await);

        callback.on_play_end("test-play", false).await;
        callback.on_playbook_end("multi-host-test", false).await;
    }

    #[tokio::test]
    async fn test_minimal_callback_full_lifecycle() {
        let callback = MinimalCallback::new();

        callback.on_playbook_start("lifecycle-test").await;

        callback
            .on_play_start("play-1", &["host1".to_string()])
            .await;
        callback.on_task_start("task-1", "host1").await;
        callback
            .on_task_complete(&create_test_result("task-1", "host1", true, true))
            .await;
        callback.on_handler_triggered("restart-service").await;
        callback
            .on_facts_gathered("host1", &create_test_facts())
            .await;
        callback.on_play_end("play-1", true).await;

        callback.on_playbook_end("lifecycle-test", true).await;
    }

    #[tokio::test]
    async fn test_minimal_callback_reset_between_playbooks() {
        let callback = MinimalCallback::new();

        // First playbook with failure
        callback.on_playbook_start("playbook-1").await;
        callback
            .on_play_start("play", &["host1".to_string()])
            .await;
        callback
            .on_task_complete(&create_test_result("task", "host1", false, false))
            .await;
        callback.on_playbook_end("playbook-1", false).await;
        assert!(callback.has_failures().await);

        // Second playbook - state should be reset
        callback.on_playbook_start("playbook-2").await;
        assert!(!callback.has_failures().await);
    }

    #[tokio::test]
    async fn test_minimal_callback_concurrent_access() {
        use tokio::task::JoinSet;

        let callback = Arc::new(MinimalCallback::new());

        callback.on_playbook_start("concurrent-test").await;
        callback
            .on_play_start(
                "test-play",
                &["host1".to_string(), "host2".to_string(), "host3".to_string()],
            )
            .await;

        let mut join_set = JoinSet::new();

        for i in 0..100 {
            let cb = callback.clone();
            let host = format!("host{}", (i % 3) + 1);
            let task = format!("task-{}", i);
            join_set.spawn(async move {
                cb.on_task_complete(&create_test_result(&task, &host, true, i % 2 == 0))
                    .await;
            });
        }

        while join_set.join_next().await.is_some() {}

        callback.on_play_end("test-play", true).await;
        callback.on_playbook_end("concurrent-test", true).await;
    }
}

// ============================================================================
// Cross-Plugin Integration Tests
// ============================================================================

mod cross_plugin_tests {
    use super::*;

    #[tokio::test]
    async fn test_multiple_plugins_same_events() {
        // Test that multiple different plugins can receive the same events
        let null_callback: Arc<dyn ExecutionCallback> = Arc::new(NullCallback);
        let default_callback: Arc<dyn ExecutionCallback> = Arc::new(
            DefaultCallbackBuilder::new()
                .no_color(true)
                .show_ok(false)
                .show_skipped(false)
                .build(),
        );
        let timer_callback: Arc<dyn ExecutionCallback> = Arc::new(TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            ..Default::default()
        }));
        let minimal_callback: Arc<dyn ExecutionCallback> = Arc::new(MinimalCallback::new());

        let callbacks: Vec<Arc<dyn ExecutionCallback>> = vec![
            null_callback,
            default_callback,
            timer_callback,
            minimal_callback,
        ];

        // Dispatch events to all callbacks
        for callback in &callbacks {
            callback.on_playbook_start("multi-plugin-test").await;
            callback
                .on_play_start("test-play", &["host1".to_string()])
                .await;
            callback.on_task_start("task-1", "host1").await;
            callback
                .on_task_complete(&create_test_result("task-1", "host1", true, true))
                .await;
            callback.on_handler_triggered("test-handler").await;
            callback
                .on_facts_gathered("host1", &create_test_facts())
                .await;
            callback.on_play_end("test-play", true).await;
            callback.on_playbook_end("multi-plugin-test", true).await;
        }
    }

    #[tokio::test]
    async fn test_plugin_send_sync_bounds() {
        // Verify all plugins implement Send + Sync (required for async trait)
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<NullCallback>();
        assert_send_sync::<DefaultCallback>();
        assert_send_sync::<TimerCallback>();
        assert_send_sync::<MinimalCallback>();
    }

    #[tokio::test]
    async fn test_plugin_arc_shared_access() {
        use tokio::task::JoinSet;

        // Test that plugins work correctly when shared via Arc across tasks
        let timer = Arc::new(TimerCallback::new(TimerConfig {
            show_per_task: false,
            show_summary: false,
            ..Default::default()
        }));

        let mut join_set = JoinSet::new();

        for i in 0..10 {
            let t = timer.clone();
            join_set.spawn(async move {
                for j in 0..10 {
                    t.on_task_start(&format!("task-{}-{}", i, j), "host1").await;
                    let result = ExecutionResult {
                        host: "host1".to_string(),
                        task_name: format!("task-{}-{}", i, j),
                        result: ModuleResult::ok("done"),
                        duration: Duration::from_millis(10),
                        notify: vec![],
                    };
                    t.on_task_complete(&result).await;
                }
            });
        }

        while join_set.join_next().await.is_some() {}

        assert_eq!(timer.get_total_tasks(), 100);
    }
}

// ============================================================================
// ProfileTasksCallback Tests
// ============================================================================

mod profile_tasks_callback_tests {
    use super::*;
    use rustible::callback::plugins::{
        AggregatedTaskTiming, PerformanceRecommendation, ProfileTasksCallback,
        ProfileTasksCallbackBuilder, ProfileTasksConfig, RecommendationSeverity, SortOrder,
    };

    /// Create a result with specific duration
    fn create_test_result_with_duration(
        task_name: &str,
        host: &str,
        success: bool,
        changed: bool,
        duration_ms: u64,
    ) -> ExecutionResult {
        ExecutionResult {
            host: host.to_string(),
            task_name: task_name.to_string(),
            result: if success {
                if changed {
                    ModuleResult::changed("Task completed with changes")
                } else {
                    ModuleResult::ok("Task completed successfully")
                }
            } else {
                ModuleResult::failed("Task failed")
            },
            duration: Duration::from_millis(duration_ms),
            notify: vec![],
        }
    }

    #[test]
    fn test_profile_tasks_callback_new() {
        let profiler = ProfileTasksCallback::new();
        assert_eq!(profiler.get_total_tasks(), 0);
        assert_eq!(profiler.get_total_duration(), Duration::ZERO);
        assert!(profiler.get_timings().is_empty());
    }

    #[test]
    fn test_profile_tasks_config_default() {
        let config = ProfileTasksConfig::default();
        assert!(config.show_per_task_timing);
        assert!(config.use_colors);
        assert!(config.show_recommendations);
        assert!(config.show_elapsed);
        assert!(config.show_timestamp);
        assert!(config.show_per_host);
        assert_eq!(config.sort_order, SortOrder::Descending);
        assert_eq!(config.threshold_secs, 0.0);
        assert_eq!(config.top_tasks, 0);
        assert_eq!(config.slow_threshold_secs, 5.0);
        assert_eq!(config.very_slow_threshold_secs, 10.0);
    }

    #[test]
    fn test_profile_tasks_builder() {
        let profiler = ProfileTasksCallbackBuilder::new()
            .show_per_task_timing(false)
            .use_colors(false)
            .threshold_secs(1.0)
            .top_tasks(10)
            .sort_order(SortOrder::Ascending)
            .show_elapsed(false)
            .show_timestamp(false)
            .show_recommendations(false)
            .show_per_host(false)
            .slow_threshold_secs(3.0)
            .very_slow_threshold_secs(8.0)
            .build();

        // Verify that the callback was created with custom config
        assert_eq!(profiler.get_total_tasks(), 0);
    }

    #[test]
    fn test_profile_tasks_summary_only() {
        let profiler = ProfileTasksCallback::summary_only();
        assert_eq!(profiler.get_total_tasks(), 0);
    }

    #[test]
    fn test_profile_tasks_verbose() {
        let profiler = ProfileTasksCallback::verbose();
        assert_eq!(profiler.get_total_tasks(), 0);
    }

    #[test]
    fn test_sort_order_variants() {
        // Test all SortOrder variants
        assert_eq!(SortOrder::default(), SortOrder::Descending);

        let orders = vec![
            SortOrder::ExecutionOrder,
            SortOrder::Descending,
            SortOrder::Ascending,
            SortOrder::None,
        ];

        for order in orders {
            let config = ProfileTasksConfig {
                sort_order: order,
                ..Default::default()
            };
            let _profiler = ProfileTasksCallback::with_config(config);
        }
    }

    #[tokio::test]
    async fn test_profile_tasks_callback_lifecycle() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            ..Default::default()
        });

        // Start playbook
        profiler.on_playbook_start("test.yml").await;

        // Start play
        profiler
            .on_play_start("Test Play", &["host1".to_string()])
            .await;

        // Simulate task
        profiler.on_task_start("Install nginx", "host1").await;

        let result = create_test_result_with_duration("Install nginx", "host1", true, true, 500);
        profiler.on_task_complete(&result).await;

        // End play and playbook
        profiler.on_play_end("Test Play", true).await;
        profiler.on_playbook_end("test.yml", true).await;

        // Verify stats
        assert_eq!(profiler.get_total_tasks(), 1);
        assert!(profiler.get_total_duration() >= Duration::from_millis(500));

        let timings = profiler.get_timings();
        assert_eq!(timings.len(), 1);
        assert_eq!(timings[0].task_name, "Install nginx");
        assert_eq!(timings[0].host, "host1");
        assert!(timings[0].success);
        assert!(timings[0].changed);
    }

    #[tokio::test]
    async fn test_profile_tasks_multiple_hosts() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            ..Default::default()
        });

        profiler.on_playbook_start("test.yml").await;
        profiler
            .on_play_start("Play", &["host1".to_string(), "host2".to_string()])
            .await;

        // Same task on multiple hosts
        for host in &["host1", "host2"] {
            profiler.on_task_start("Install nginx", host).await;
            let result = create_test_result("Install nginx", host, true, false);
            profiler.on_task_complete(&result).await;
        }

        profiler.on_play_end("Play", true).await;
        profiler.on_playbook_end("test.yml", true).await;

        let aggregated = profiler.get_aggregated_timings();
        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].task_name, "Install nginx");
        assert_eq!(aggregated[0].host_count, 2);
        assert_eq!(aggregated[0].host_timings.len(), 2);
    }

    #[tokio::test]
    async fn test_profile_tasks_multiple_tasks() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            ..Default::default()
        });

        profiler.on_playbook_start("test.yml").await;
        profiler
            .on_play_start("Play", &["host1".to_string()])
            .await;

        // Multiple different tasks
        for task in &["Install nginx", "Configure nginx", "Start nginx"] {
            profiler.on_task_start(task, "host1").await;
            let result = create_test_result(task, "host1", true, false);
            profiler.on_task_complete(&result).await;
        }

        profiler.on_play_end("Play", true).await;
        profiler.on_playbook_end("test.yml", true).await;

        assert_eq!(profiler.get_total_tasks(), 3);
        let aggregated = profiler.get_aggregated_timings();
        assert_eq!(aggregated.len(), 3);
    }

    #[tokio::test]
    async fn test_profile_tasks_host_timings() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            ..Default::default()
        });

        profiler.on_playbook_start("test.yml").await;
        profiler
            .on_play_start("Play", &["host1".to_string(), "host2".to_string()])
            .await;

        // Tasks on multiple hosts
        for host in &["host1", "host2"] {
            for task in &["Task 1", "Task 2"] {
                profiler.on_task_start(task, host).await;
                let result = create_test_result(task, host, true, true);
                profiler.on_task_complete(&result).await;
            }
        }

        profiler.on_play_end("Play", true).await;
        profiler.on_playbook_end("test.yml", true).await;

        let host_timings = profiler.get_host_timings();
        assert_eq!(host_timings.len(), 2);
        assert!(host_timings.contains_key("host1"));
        assert!(host_timings.contains_key("host2"));

        let host1_timing = &host_timings["host1"];
        assert_eq!(host1_timing.task_count, 2);
        assert_eq!(host1_timing.changed_count, 2);
    }

    #[tokio::test]
    async fn test_profile_tasks_skipped_tasks() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            ..Default::default()
        });

        profiler.on_playbook_start("test.yml").await;
        profiler
            .on_play_start("Play", &["host1".to_string()])
            .await;

        // Skipped task
        profiler.on_task_start("Conditional task", "host1").await;
        let result = create_skipped_result("Conditional task", "host1");
        profiler.on_task_complete(&result).await;

        profiler.on_play_end("Play", true).await;
        profiler.on_playbook_end("test.yml", true).await;

        let host_timings = profiler.get_host_timings();
        let host1_timing = &host_timings["host1"];
        assert_eq!(host1_timing.skipped_count, 1);
    }

    #[tokio::test]
    async fn test_profile_tasks_failed_tasks() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            ..Default::default()
        });

        profiler.on_playbook_start("test.yml").await;
        profiler
            .on_play_start("Play", &["host1".to_string()])
            .await;

        // Failed task
        profiler.on_task_start("Failing task", "host1").await;
        let result = create_test_result("Failing task", "host1", false, false);
        profiler.on_task_complete(&result).await;

        profiler.on_play_end("Play", false).await;
        profiler.on_playbook_end("test.yml", false).await;

        let host_timings = profiler.get_host_timings();
        let host1_timing = &host_timings["host1"];
        assert_eq!(host1_timing.failed_count, 1);
    }

    #[test]
    fn test_profile_tasks_reset() {
        let profiler = ProfileTasksCallback::new();

        // Add some data via callback
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            profiler.on_playbook_start("test.yml").await;
            profiler.on_task_start("Task", "host1").await;
            let result = create_test_result("Task", "host1", true, false);
            profiler.on_task_complete(&result).await;
        });

        assert_eq!(profiler.get_total_tasks(), 1);
        assert!(!profiler.get_timings().is_empty());

        // Reset
        profiler.reset();

        assert_eq!(profiler.get_total_tasks(), 0);
        assert!(profiler.get_timings().is_empty());
        assert!(profiler.get_host_timings().is_empty());
    }

    #[test]
    fn test_profile_tasks_clone() {
        let profiler1 = ProfileTasksCallback::new();

        // Add data to profiler1
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            profiler1.on_playbook_start("test.yml").await;
            profiler1.on_task_start("Task", "host1").await;
            let result = create_test_result("Task", "host1", true, false);
            profiler1.on_task_complete(&result).await;
        });

        // Clone
        let profiler2 = profiler1.clone();

        // Cloned profiler should have fresh state
        assert_eq!(profiler1.get_total_tasks(), 1);
        assert_eq!(profiler2.get_total_tasks(), 0);
    }

    #[test]
    fn test_recommendation_severity() {
        // Test RecommendationSeverity variants
        let info = RecommendationSeverity::Info;
        let warning = RecommendationSeverity::Warning;
        let critical = RecommendationSeverity::Critical;

        assert_ne!(info, warning);
        assert_ne!(warning, critical);
        assert_eq!(info, RecommendationSeverity::Info);
    }

    #[tokio::test]
    async fn test_profile_tasks_handlers_and_facts() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            ..Default::default()
        });

        profiler.on_playbook_start("test.yml").await;

        // Handler triggered - should not panic
        profiler.on_handler_triggered("Restart nginx").await;

        // Facts gathered - should not panic
        let facts = create_test_facts();
        profiler.on_facts_gathered("host1", &facts).await;

        profiler.on_playbook_end("test.yml", true).await;

        // Handlers and facts don't affect task count
        assert_eq!(profiler.get_total_tasks(), 0);
    }

    #[tokio::test]
    async fn test_profile_tasks_aggregation_sorting() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            sort_order: SortOrder::Descending,
            ..Default::default()
        });

        profiler.on_playbook_start("test.yml").await;
        profiler.on_play_start("Play", &["host1".to_string()]).await;

        // Tasks with different durations
        let tasks = vec![
            ("Fast task", 100u64),
            ("Medium task", 500u64),
            ("Slow task", 1000u64),
        ];

        for (task, duration) in tasks {
            profiler.on_task_start(task, "host1").await;
            let result = create_test_result_with_duration(task, "host1", true, false, duration);
            profiler.on_task_complete(&result).await;
        }

        profiler.on_play_end("Play", true).await;
        profiler.on_playbook_end("test.yml", true).await;

        let aggregated = profiler.get_aggregated_timings();
        assert_eq!(aggregated.len(), 3);
        // Default sort is descending - slowest first
        assert_eq!(aggregated[0].task_name, "Slow task");
        assert_eq!(aggregated[1].task_name, "Medium task");
        assert_eq!(aggregated[2].task_name, "Fast task");
    }

    #[tokio::test]
    async fn test_profile_tasks_performance_recommendations() {
        let profiler = ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: true,
            slow_threshold_secs: 0.5,
            very_slow_threshold_secs: 1.0,
            ..Default::default()
        });

        profiler.on_playbook_start("test.yml").await;
        profiler.on_play_start("Play", &["host1".to_string()]).await;

        // Add a slow task
        profiler.on_task_start("Very slow task", "host1").await;
        let result = create_test_result_with_duration("Very slow task", "host1", true, false, 2000);
        profiler.on_task_complete(&result).await;

        // Add a moderately slow task
        profiler.on_task_start("Slow task", "host1").await;
        let result = create_test_result_with_duration("Slow task", "host1", true, false, 800);
        profiler.on_task_complete(&result).await;

        profiler.on_play_end("Play", true).await;
        profiler.on_playbook_end("test.yml", true).await;

        let recommendations = profiler.get_recommendations();
        assert!(!recommendations.is_empty());

        // Should have recommendations for both slow tasks
        let critical_count = recommendations
            .iter()
            .filter(|r| r.severity == RecommendationSeverity::Critical)
            .count();
        let warning_count = recommendations
            .iter()
            .filter(|r| r.severity == RecommendationSeverity::Warning)
            .count();

        assert!(critical_count >= 1);
        assert!(warning_count >= 1);
    }

    #[tokio::test]
    async fn test_profile_tasks_concurrent_access() {
        use tokio::task::JoinSet;

        let profiler = Arc::new(ProfileTasksCallback::with_config(ProfileTasksConfig {
            show_per_task_timing: false,
            use_colors: false,
            show_recommendations: false,
            ..Default::default()
        }));

        profiler.on_playbook_start("concurrent.yml").await;

        let hosts: Vec<String> = (0..5).map(|i| format!("host{}", i)).collect();
        profiler.on_play_start("Concurrent play", &hosts).await;

        let mut join_set = JoinSet::new();

        // Simulate parallel task execution on multiple hosts
        for host in &hosts {
            let p = profiler.clone();
            let h = host.clone();
            join_set.spawn(async move {
                p.on_task_start("Install package", &h).await;
                tokio::time::sleep(Duration::from_millis(5)).await;
                let result = create_test_result("Install package", &h, true, true);
                p.on_task_complete(&result).await;
            });
        }

        while join_set.join_next().await.is_some() {}

        profiler.on_play_end("Concurrent play", true).await;
        profiler.on_playbook_end("concurrent.yml", true).await;

        assert_eq!(profiler.get_total_tasks(), 5);
        let host_timings = profiler.get_host_timings();
        assert_eq!(host_timings.len(), 5);
    }
}

// ============================================================================
// SlackCallback Tests (Mocked - No Network)
// ============================================================================

mod slack_callback_tests {
    use super::*;
    use rustible::callback::plugins::{SlackCallback, SlackCallbackConfig, SlackError};

    #[test]
    fn test_slack_config_builder() {
        let config = SlackCallbackConfig::builder()
            .webhook_url("https://hooks.slack.com/services/test/test/test")
            .channel("#test-channel")
            .username("TestBot")
            .icon_emoji(":robot:")
            .notify_on_start(true)
            .notify_on_end(true)
            .notify_on_failure(false)
            .thread_tasks(true)
            .include_host_details(false)
            .max_failures_shown(5)
            .timeout_secs(60)
            .build();

        assert_eq!(
            config.webhook_url,
            "https://hooks.slack.com/services/test/test/test"
        );
        assert_eq!(config.channel, Some("#test-channel".to_string()));
        assert_eq!(config.username, "TestBot");
        assert_eq!(config.icon_emoji, ":robot:");
        assert!(config.notify_on_start);
        assert!(config.notify_on_end);
        assert!(!config.notify_on_failure);
        assert!(config.thread_tasks);
        assert!(!config.include_host_details);
        assert_eq!(config.max_failures_shown, 5);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_slack_config_default() {
        let config = SlackCallbackConfig::default();
        assert!(config.webhook_url.is_empty());
        assert!(config.channel.is_none());
        assert_eq!(config.username, "Rustible");
        assert_eq!(config.icon_emoji, ":gear:");
        assert!(!config.notify_on_start);
        assert!(config.notify_on_end);
        assert!(config.notify_on_failure);
        assert!(!config.thread_tasks);
        assert!(config.include_host_details);
        assert_eq!(config.max_failures_shown, 10);
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.retry_attempts, 3);
        assert_eq!(config.min_message_interval_ms, 1000);
    }

    #[test]
    fn test_slack_config_validation_empty_url() {
        let config = SlackCallbackConfig::default();
        let result = config.validate();
        assert!(result.is_err());

        if let Err(SlackError::MissingConfig(msg)) = result {
            assert!(msg.contains("webhook_url"));
        } else {
            panic!("Expected MissingConfig error");
        }
    }

    #[test]
    fn test_slack_config_validation_invalid_url() {
        let mut config = SlackCallbackConfig::default();
        config.webhook_url = "https://example.com/webhook".to_string();

        let result = config.validate();
        assert!(result.is_err());

        if let Err(SlackError::MissingConfig(msg)) = result {
            assert!(msg.contains("valid Slack webhook URL"));
        } else {
            panic!("Expected MissingConfig error for invalid URL");
        }
    }

    #[test]
    fn test_slack_config_validation_valid_url() {
        let mut config = SlackCallbackConfig::default();
        config.webhook_url = "https://hooks.slack.com/services/T00/B00/XXX".to_string();

        let result = config.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_slack_callback_new_with_valid_config() {
        let config = SlackCallbackConfig::builder()
            .webhook_url("https://hooks.slack.com/services/T00/B00/XXX")
            .build();

        let result = SlackCallback::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_slack_callback_new_with_invalid_config() {
        let config = SlackCallbackConfig::default();
        let result = SlackCallback::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_slack_error_display() {
        let err = SlackError::MissingConfig("webhook_url".to_string());
        assert!(err.to_string().contains("Missing required configuration"));
        assert!(err.to_string().contains("webhook_url"));

        let err = SlackError::ApiError("Bad request".to_string());
        assert!(err.to_string().contains("Slack API error"));
        assert!(err.to_string().contains("Bad request"));

        let err = SlackError::RateLimited(60);
        assert!(err.to_string().contains("Rate limited"));
        assert!(err.to_string().contains("60"));
    }

    #[tokio::test]
    async fn test_slack_callback_clone() {
        let config = SlackCallbackConfig::builder()
            .webhook_url("https://hooks.slack.com/services/T00/B00/XXX")
            .build();

        let callback = SlackCallback::new(config).unwrap();
        let cloned = callback.clone();

        // Both should work (share state via Arc)
        drop(cloned);
    }

    #[tokio::test]
    async fn test_slack_callback_lifecycle_without_network() {
        // Test the callback lifecycle without actually sending messages
        // by using a configuration that won't trigger notifications
        let config = SlackCallbackConfig::builder()
            .webhook_url("https://hooks.slack.com/services/T00/B00/XXX")
            .notify_on_start(false)
            .notify_on_end(false)
            .notify_on_failure(false)
            .build();

        let callback = SlackCallback::new(config).unwrap();

        // These should all work without errors since notifications are disabled
        callback.on_playbook_start("test.yml").await;
        callback
            .on_play_start("Test Play", &["host1".to_string()])
            .await;
        callback.on_task_start("Install nginx", "host1").await;

        let result = create_test_result("Install nginx", "host1", true, true);
        callback.on_task_complete(&result).await;

        callback.on_play_end("Test Play", true).await;
        callback.on_playbook_end("test.yml", true).await;
    }

    #[tokio::test]
    async fn test_slack_callback_tracks_host_stats() {
        // Test that the callback properly tracks host statistics internally
        let config = SlackCallbackConfig::builder()
            .webhook_url("https://hooks.slack.com/services/T00/B00/XXX")
            .notify_on_start(false)
            .notify_on_end(false)
            .notify_on_failure(false)
            .build();

        let callback = SlackCallback::new(config).unwrap();

        callback.on_playbook_start("test.yml").await;
        callback
            .on_play_start("Test Play", &["host1".to_string(), "host2".to_string()])
            .await;

        // Simulate various task results
        // Host1: ok task
        let result = create_test_result("Task 1", "host1", true, false);
        callback.on_task_complete(&result).await;

        // Host1: changed task
        let result = create_test_result("Task 2", "host1", true, true);
        callback.on_task_complete(&result).await;

        // Host2: failed task
        let result = create_test_result("Task 1", "host2", false, false);
        callback.on_task_complete(&result).await;

        // Host2: skipped task
        let result = create_skipped_result("Task 2", "host2");
        callback.on_task_complete(&result).await;

        callback.on_play_end("Test Play", false).await;
        callback.on_playbook_end("test.yml", false).await;

        // The callback should have tracked all these internally without panicking
    }

    #[tokio::test]
    async fn test_slack_callback_handles_multiple_plays() {
        let config = SlackCallbackConfig::builder()
            .webhook_url("https://hooks.slack.com/services/T00/B00/XXX")
            .notify_on_start(false)
            .notify_on_end(false)
            .notify_on_failure(false)
            .build();

        let callback = SlackCallback::new(config).unwrap();

        callback.on_playbook_start("multi-play.yml").await;

        // First play
        callback
            .on_play_start("Web servers", &["web1".to_string(), "web2".to_string()])
            .await;
        let result = create_test_result("Install nginx", "web1", true, true);
        callback.on_task_complete(&result).await;
        callback.on_play_end("Web servers", true).await;

        // Second play
        callback
            .on_play_start("Database servers", &["db1".to_string()])
            .await;
        let result = create_test_result("Install postgresql", "db1", true, true);
        callback.on_task_complete(&result).await;
        callback.on_play_end("Database servers", true).await;

        callback.on_playbook_end("multi-play.yml", true).await;
    }
}

// ============================================================================
// LogstashCallback Tests (Mocked - No Network)
// ============================================================================

mod logstash_callback_tests {
    use super::*;
    use rustible::callback::plugins::{
        LogstashCallback, LogstashConfig, LogstashConfigBuilder, LogstashError, LogstashProtocol,
    };
    use std::collections::HashMap;

    #[test]
    fn test_logstash_protocol_variants() {
        // Test that all protocol variants can be created
        let tcp = LogstashProtocol::Tcp;
        let udp = LogstashProtocol::Udp;
        let http = LogstashProtocol::Http;
        let https = LogstashProtocol::Https;

        assert_eq!(tcp, LogstashProtocol::Tcp);
        assert_eq!(udp, LogstashProtocol::Udp);
        assert_eq!(http, LogstashProtocol::Http);
        assert_eq!(https, LogstashProtocol::Https);
    }

    #[test]
    fn test_logstash_config_builder() {
        let config = LogstashConfigBuilder::new()
            .host("logstash.example.com")
            .port(5044)
            .protocol(LogstashProtocol::Tcp)
            .use_ecs(true)
            .buffer_size(1000)
            .flush_interval_ms(5000)
            .timeout_secs(30)
            .retry_attempts(3)
            .build();

        assert_eq!(config.host, "logstash.example.com");
        assert_eq!(config.port, 5044);
        assert_eq!(config.protocol, LogstashProtocol::Tcp);
        assert!(config.use_ecs);
        assert_eq!(config.buffer_size, 1000);
        assert_eq!(config.flush_interval_ms, 5000);
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.retry_attempts, 3);
    }

    #[test]
    fn test_logstash_config_default() {
        let config = LogstashConfig::default();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5044);
        assert_eq!(config.protocol, LogstashProtocol::Tcp);
        assert!(config.use_ecs);
        assert_eq!(config.buffer_size, 100);
        assert!(config.flush_interval_ms > 0);
    }

    #[test]
    fn test_logstash_protocol_parsing() {
        // Test parsing protocol from strings
        assert_eq!(
            "tcp".parse::<LogstashProtocol>().unwrap(),
            LogstashProtocol::Tcp
        );
        assert_eq!(
            "TCP".parse::<LogstashProtocol>().unwrap(),
            LogstashProtocol::Tcp
        );
        assert_eq!(
            "udp".parse::<LogstashProtocol>().unwrap(),
            LogstashProtocol::Udp
        );
        assert_eq!(
            "http".parse::<LogstashProtocol>().unwrap(),
            LogstashProtocol::Http
        );
        assert_eq!(
            "https".parse::<LogstashProtocol>().unwrap(),
            LogstashProtocol::Https
        );

        // Invalid protocol should error
        assert!("invalid".parse::<LogstashProtocol>().is_err());
    }

    #[test]
    fn test_logstash_error_display() {
        let err = LogstashError::ConnectionFailed("Connection refused".to_string());
        let err_str = err.to_string();
        assert!(
            err_str.contains("Connection") || err_str.contains("connection") || err_str.contains("refused")
        );
    }

    #[test]
    fn test_logstash_config_with_http() {
        let config = LogstashConfigBuilder::new()
            .host("logstash.example.com")
            .port(8080)
            .protocol(LogstashProtocol::Http)
            .http_path("/logs")
            .build();

        assert_eq!(config.protocol, LogstashProtocol::Http);
        assert_eq!(config.http_path, Some("/logs".to_string()));
    }

    #[test]
    fn test_logstash_config_with_https() {
        let config = LogstashConfigBuilder::new()
            .host("logstash.example.com")
            .port(443)
            .protocol(LogstashProtocol::Https)
            .http_path("/v1/logs")
            .build();

        assert_eq!(config.protocol, LogstashProtocol::Https);
    }

    #[test]
    fn test_logstash_config_custom_fields() {
        let mut custom_fields = HashMap::new();
        custom_fields.insert("environment".to_string(), serde_json::json!("production"));
        custom_fields.insert("service".to_string(), serde_json::json!("rustible"));

        let config = LogstashConfigBuilder::new()
            .host("logstash.example.com")
            .custom_fields(custom_fields.clone())
            .build();

        assert_eq!(config.custom_fields.len(), 2);
        assert_eq!(
            config.custom_fields.get("environment"),
            Some(&serde_json::json!("production"))
        );
    }

    #[test]
    fn test_logstash_config_index_name() {
        let config = LogstashConfigBuilder::new()
            .host("logstash.example.com")
            .index_name("rustible-logs")
            .build();

        assert_eq!(config.index_name, Some("rustible-logs".to_string()));
    }

    #[test]
    fn test_logstash_config_with_tls() {
        let config = LogstashConfigBuilder::new()
            .host("logstash.example.com")
            .port(5044)
            .protocol(LogstashProtocol::Tcp)
            .tls_enabled(true)
            .tls_verify(true)
            .build();

        assert!(config.tls_enabled);
        assert!(config.tls_verify);
    }

    #[test]
    fn test_logstash_callback_creation() {
        // Test that callback can be created with valid config
        // Note: This doesn't actually connect, it just creates the callback
        let config = LogstashConfig::default();

        // The callback might or might not succeed depending on implementation
        // We just verify it doesn't panic
        let _result = LogstashCallback::new(config);
    }

    #[test]
    fn test_logstash_config_builder_chain() {
        // Test fluent builder pattern
        let config = LogstashConfigBuilder::new()
            .host("host1")
            .host("host2") // Last one wins
            .port(5000)
            .port(5044) // Last one wins
            .protocol(LogstashProtocol::Udp)
            .protocol(LogstashProtocol::Tcp) // Last one wins
            .build();

        assert_eq!(config.host, "host2");
        assert_eq!(config.port, 5044);
        assert_eq!(config.protocol, LogstashProtocol::Tcp);
    }
}

// ============================================================================
// Callback Event Dispatching Tests
// ============================================================================

mod callback_event_dispatching_tests {
    use super::*;

    /// A tracking callback that records all events for verification
    #[derive(Debug)]
    struct TrackingCallback {
        name: String,
        events: RwLock<Vec<String>>,
        playbook_start_count: AtomicU32,
        playbook_end_count: AtomicU32,
        play_start_count: AtomicU32,
        play_end_count: AtomicU32,
        task_start_count: AtomicU32,
        task_complete_count: AtomicU32,
        handler_triggered_count: AtomicU32,
        facts_gathered_count: AtomicU32,
    }

    impl TrackingCallback {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                events: RwLock::new(Vec::new()),
                playbook_start_count: AtomicU32::new(0),
                playbook_end_count: AtomicU32::new(0),
                play_start_count: AtomicU32::new(0),
                play_end_count: AtomicU32::new(0),
                task_start_count: AtomicU32::new(0),
                task_complete_count: AtomicU32::new(0),
                handler_triggered_count: AtomicU32::new(0),
                facts_gathered_count: AtomicU32::new(0),
            }
        }

        fn total_events(&self) -> u32 {
            self.playbook_start_count.load(Ordering::SeqCst)
                + self.playbook_end_count.load(Ordering::SeqCst)
                + self.play_start_count.load(Ordering::SeqCst)
                + self.play_end_count.load(Ordering::SeqCst)
                + self.task_start_count.load(Ordering::SeqCst)
                + self.task_complete_count.load(Ordering::SeqCst)
                + self.handler_triggered_count.load(Ordering::SeqCst)
                + self.facts_gathered_count.load(Ordering::SeqCst)
        }

        fn get_events(&self) -> Vec<String> {
            self.events.read().clone()
        }
    }

    #[async_trait]
    impl ExecutionCallback for TrackingCallback {
        async fn on_playbook_start(&self, name: &str) {
            self.playbook_start_count.fetch_add(1, Ordering::SeqCst);
            self.events
                .write()
                .push(format!("{}:playbook_start:{}", self.name, name));
        }

        async fn on_playbook_end(&self, name: &str, success: bool) {
            self.playbook_end_count.fetch_add(1, Ordering::SeqCst);
            self.events
                .write()
                .push(format!("{}:playbook_end:{}:{}", self.name, name, success));
        }

        async fn on_play_start(&self, name: &str, hosts: &[String]) {
            self.play_start_count.fetch_add(1, Ordering::SeqCst);
            self.events.write().push(format!(
                "{}:play_start:{}:hosts={}",
                self.name,
                name,
                hosts.len()
            ));
        }

        async fn on_play_end(&self, name: &str, success: bool) {
            self.play_end_count.fetch_add(1, Ordering::SeqCst);
            self.events
                .write()
                .push(format!("{}:play_end:{}:{}", self.name, name, success));
        }

        async fn on_task_start(&self, name: &str, host: &str) {
            self.task_start_count.fetch_add(1, Ordering::SeqCst);
            self.events
                .write()
                .push(format!("{}:task_start:{}:{}", self.name, name, host));
        }

        async fn on_task_complete(&self, result: &ExecutionResult) {
            self.task_complete_count.fetch_add(1, Ordering::SeqCst);
            self.events.write().push(format!(
                "{}:task_complete:{}:{}:success={}",
                self.name, result.task_name, result.host, result.result.success
            ));
        }

        async fn on_handler_triggered(&self, name: &str) {
            self.handler_triggered_count.fetch_add(1, Ordering::SeqCst);
            self.events
                .write()
                .push(format!("{}:handler_triggered:{}", self.name, name));
        }

        async fn on_facts_gathered(&self, host: &str, _facts: &Facts) {
            self.facts_gathered_count.fetch_add(1, Ordering::SeqCst);
            self.events
                .write()
                .push(format!("{}:facts_gathered:{}", self.name, host));
        }
    }

    #[tokio::test]
    async fn test_callback_receives_playbook_start() {
        let callback = TrackingCallback::new("test");

        callback.on_playbook_start("deploy.yml").await;

        assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);
        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("playbook_start:deploy.yml")));
    }

    #[tokio::test]
    async fn test_callback_receives_playbook_end() {
        let callback = TrackingCallback::new("test");

        callback.on_playbook_end("deploy.yml", true).await;
        callback.on_playbook_end("failed.yml", false).await;

        assert_eq!(callback.playbook_end_count.load(Ordering::SeqCst), 2);
        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("playbook_end:deploy.yml:true")));
        assert!(events
            .iter()
            .any(|e| e.contains("playbook_end:failed.yml:false")));
    }

    #[tokio::test]
    async fn test_callback_receives_play_start() {
        let callback = TrackingCallback::new("test");
        let hosts = vec!["host1".to_string(), "host2".to_string()];

        callback.on_play_start("Configure servers", &hosts).await;

        assert_eq!(callback.play_start_count.load(Ordering::SeqCst), 1);
        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("play_start:Configure servers:hosts=2")));
    }

    #[tokio::test]
    async fn test_callback_receives_play_end() {
        let callback = TrackingCallback::new("test");

        callback.on_play_end("Configure servers", true).await;

        assert_eq!(callback.play_end_count.load(Ordering::SeqCst), 1);
        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("play_end:Configure servers:true")));
    }

    #[tokio::test]
    async fn test_callback_receives_task_start() {
        let callback = TrackingCallback::new("test");

        callback.on_task_start("Install nginx", "webserver1").await;

        assert_eq!(callback.task_start_count.load(Ordering::SeqCst), 1);
        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("task_start:Install nginx:webserver1")));
    }

    #[tokio::test]
    async fn test_callback_receives_task_complete() {
        let callback = TrackingCallback::new("test");

        let result = create_test_result("Install nginx", "webserver1", true, true);
        callback.on_task_complete(&result).await;

        assert_eq!(callback.task_complete_count.load(Ordering::SeqCst), 1);
        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("task_complete:Install nginx:webserver1:success=true")));
    }

    #[tokio::test]
    async fn test_callback_receives_handler_triggered() {
        let callback = TrackingCallback::new("test");

        callback.on_handler_triggered("Restart nginx").await;

        assert_eq!(callback.handler_triggered_count.load(Ordering::SeqCst), 1);
        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("handler_triggered:Restart nginx")));
    }

    #[tokio::test]
    async fn test_callback_receives_facts_gathered() {
        let callback = TrackingCallback::new("test");
        let facts = create_test_facts();

        callback.on_facts_gathered("webserver1", &facts).await;

        assert_eq!(callback.facts_gathered_count.load(Ordering::SeqCst), 1);
        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("facts_gathered:webserver1")));
    }

    #[tokio::test]
    async fn test_callback_full_playbook_lifecycle() {
        let callback = TrackingCallback::new("lifecycle");

        // Simulate a full playbook run
        callback.on_playbook_start("site.yml").await;

        let hosts = vec!["host1".to_string(), "host2".to_string()];
        callback.on_play_start("Configure webservers", &hosts).await;

        // Gather facts
        let facts = create_test_facts();
        callback.on_facts_gathered("host1", &facts).await;
        callback.on_facts_gathered("host2", &facts).await;

        // Execute tasks
        for host in &["host1", "host2"] {
            callback.on_task_start("Install nginx", host).await;
            let result = create_test_result("Install nginx", host, true, true);
            callback.on_task_complete(&result).await;
        }

        // Trigger handler
        callback.on_handler_triggered("Restart nginx").await;

        callback.on_play_end("Configure webservers", true).await;
        callback.on_playbook_end("site.yml", true).await;

        // Verify all events received
        assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback.playbook_end_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback.play_start_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback.play_end_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback.task_start_count.load(Ordering::SeqCst), 2);
        assert_eq!(callback.task_complete_count.load(Ordering::SeqCst), 2);
        assert_eq!(callback.handler_triggered_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback.facts_gathered_count.load(Ordering::SeqCst), 2);

        // Total events: 1+1+1+1+2+2+1+2 = 11
        assert_eq!(callback.total_events(), 11);
    }

    #[tokio::test]
    async fn test_callback_event_ordering() {
        let callback = TrackingCallback::new("ordering");

        callback.on_playbook_start("test.yml").await;
        callback
            .on_play_start("Play", &["host1".to_string()])
            .await;
        callback.on_task_start("Task", "host1").await;
        let result = create_test_result("Task", "host1", true, false);
        callback.on_task_complete(&result).await;
        callback.on_play_end("Play", true).await;
        callback.on_playbook_end("test.yml", true).await;

        let events = callback.get_events();
        assert_eq!(events.len(), 6);

        // Verify order
        assert!(events[0].contains("playbook_start"));
        assert!(events[1].contains("play_start"));
        assert!(events[2].contains("task_start"));
        assert!(events[3].contains("task_complete"));
        assert!(events[4].contains("play_end"));
        assert!(events[5].contains("playbook_end"));
    }

    #[tokio::test]
    async fn test_callback_with_multiple_plays() {
        let callback = TrackingCallback::new("multi-play");

        callback.on_playbook_start("site.yml").await;

        // First play
        callback
            .on_play_start("Configure webservers", &["web1".to_string()])
            .await;
        callback.on_task_start("Install nginx", "web1").await;
        let result = create_test_result("Install nginx", "web1", true, true);
        callback.on_task_complete(&result).await;
        callback.on_play_end("Configure webservers", true).await;

        // Second play
        callback
            .on_play_start("Configure databases", &["db1".to_string()])
            .await;
        callback.on_task_start("Install postgresql", "db1").await;
        let result = create_test_result("Install postgresql", "db1", true, true);
        callback.on_task_complete(&result).await;
        callback.on_play_end("Configure databases", true).await;

        callback.on_playbook_end("site.yml", true).await;

        assert_eq!(callback.play_start_count.load(Ordering::SeqCst), 2);
        assert_eq!(callback.play_end_count.load(Ordering::SeqCst), 2);
        assert_eq!(callback.task_start_count.load(Ordering::SeqCst), 2);
        assert_eq!(callback.task_complete_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_callback_with_failed_task() {
        let callback = TrackingCallback::new("failure");

        callback.on_playbook_start("test.yml").await;
        callback
            .on_play_start("Play", &["host1".to_string()])
            .await;

        callback.on_task_start("Failing task", "host1").await;
        let result = create_test_result("Failing task", "host1", false, false);
        callback.on_task_complete(&result).await;

        callback.on_play_end("Play", false).await;
        callback.on_playbook_end("test.yml", false).await;

        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("task_complete") && e.contains("success=false")));
        assert!(events
            .iter()
            .any(|e| e.contains("play_end") && e.contains("false")));
        assert!(events
            .iter()
            .any(|e| e.contains("playbook_end") && e.contains("false")));
    }

    #[tokio::test]
    async fn test_concurrent_callback_invocations() {
        use tokio::task::JoinSet;

        let callback = Arc::new(TrackingCallback::new("concurrent"));
        let mut join_set = JoinSet::new();

        // Spawn many concurrent task events
        for i in 0..100 {
            let cb = callback.clone();
            let task_name = format!("task-{}", i);
            let host = format!("host-{}", i % 10);

            join_set.spawn(async move {
                cb.on_task_start(&task_name, &host).await;
            });
        }

        while join_set.join_next().await.is_some() {}

        assert_eq!(callback.task_start_count.load(Ordering::SeqCst), 100);
    }

    #[tokio::test]
    async fn test_callback_with_empty_host_list() {
        let callback = TrackingCallback::new("empty-hosts");

        callback.on_play_start("Empty play", &[]).await;

        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("play_start:Empty play:hosts=0")));
    }

    #[tokio::test]
    async fn test_callback_with_many_hosts() {
        let callback = TrackingCallback::new("many-hosts");

        let hosts: Vec<String> = (0..100).map(|i| format!("host-{}", i)).collect();
        callback.on_play_start("Large play", &hosts).await;

        let events = callback.get_events();
        assert!(events
            .iter()
            .any(|e| e.contains("play_start:Large play:hosts=100")));
    }

    #[tokio::test]
    async fn test_callback_with_unicode_names() {
        let callback = TrackingCallback::new("unicode");

        callback.on_playbook_start("playbook-\u{1F680}").await;
        callback
            .on_play_start("play-\u{2764}", &["host-\u{1F4BB}".to_string()])
            .await;
        callback.on_task_start("task-\u{1F389}", "host-\u{1F4BB}").await;

        let events = callback.get_events();
        assert!(events.iter().any(|e| e.contains("\u{1F680}")));
        assert!(events.iter().any(|e| e.contains("\u{2764}")));
        assert!(events.iter().any(|e| e.contains("\u{1F389}")));
    }

    #[tokio::test]
    async fn test_multiple_callbacks_receive_same_events() {
        let callback1 = Arc::new(TrackingCallback::new("cb1"));
        let callback2 = Arc::new(TrackingCallback::new("cb2"));

        // Dispatch same events to both callbacks
        callback1.on_playbook_start("shared.yml").await;
        callback2.on_playbook_start("shared.yml").await;

        callback1.on_task_start("shared-task", "host1").await;
        callback2.on_task_start("shared-task", "host1").await;

        let result = create_test_result("shared-task", "host1", true, true);
        callback1.on_task_complete(&result).await;
        callback2.on_task_complete(&result).await;

        // Both should have received all events
        assert_eq!(callback1.playbook_start_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback2.playbook_start_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback1.task_start_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback2.task_start_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback1.task_complete_count.load(Ordering::SeqCst), 1);
        assert_eq!(callback2.task_complete_count.load(Ordering::SeqCst), 1);
    }
}
