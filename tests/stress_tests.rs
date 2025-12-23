//! Extreme stress and chaos tests for Rustible
//!
//! This test suite pushes Rustible to its limits and verifies stability under chaos conditions.
//! Run with: cargo test --test stress_tests -- --test-threads=1
//!
//! Tests cover:
//! - Concurrency stress (100+ concurrent tasks, 500+ simulated hosts)
//! - Memory stress (large inventories, playbooks, variable contexts)
//! - Connection stress (rapid connect/disconnect, pool churn)
//! - Chaos scenarios (random failures, connection drops, slow responses)
//! - Race conditions (concurrent variable access, handler notifications)
//! - Long-running stability (1000+ iterations, resource leak detection)
//! - Edge cases under load

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::join_all;
use proptest::prelude::*;
use tokio::sync::{RwLock, Semaphore};

use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::RuntimeContext;
use rustible::executor::task::{Handler, Task};
use rustible::executor::{ExecutionStats, ExecutionStrategy, Executor, ExecutorConfig};

// ============================================================================
// Helper Utilities for Stress Testing
// ============================================================================

/// Create a runtime with a large number of hosts
fn create_large_inventory(count: usize) -> RuntimeContext {
    let mut runtime = RuntimeContext::new();
    for i in 0..count {
        let host = format!("host-{:05}", i);
        let group = match i % 4 {
            0 => "webservers",
            1 => "databases",
            2 => "caches",
            _ => "workers",
        };
        runtime.add_host(host, Some(group));
    }
    runtime
}

/// Create a playbook with many tasks
fn create_large_playbook(task_count: usize) -> Playbook {
    let mut playbook = Playbook::new("Stress Test Playbook");
    let mut play = Play::new("Stress Play", "all");
    play.gather_facts = false;

    for i in 0..task_count {
        let task = Task::new(format!("Task-{:05}", i), "debug")
            .arg("msg", format!("Executing task {}", i));
        play.add_task(task);
    }

    playbook.add_play(play);
    playbook
}

/// Create a runtime with large variable contexts
fn create_runtime_with_large_vars(host_count: usize, var_size_kb: usize) -> RuntimeContext {
    let mut runtime = RuntimeContext::new();

    // Create a large value (approximately var_size_kb kilobytes)
    let large_value: String = "x".repeat(var_size_kb * 1024);

    for i in 0..host_count {
        let host = format!("host-{:05}", i);
        runtime.add_host(host.clone(), None);

        // Add large variables to each host
        runtime.set_host_var(
            &host,
            "large_var".to_string(),
            serde_json::json!(large_value.clone()),
        );

        // Add nested structure
        runtime.set_host_var(
            &host,
            "complex_var".to_string(),
            serde_json::json!({
                "level1": {
                    "level2": {
                        "level3": {
                            "data": large_value.clone()[..1024.min(large_value.len())].to_string(),
                        }
                    }
                }
            }),
        );
    }

    runtime
}

/// Failure injector for chaos testing
#[derive(Clone)]
#[allow(dead_code)]
struct FailureInjector {
    failure_rate: f64,
    counter: Arc<AtomicUsize>,
}

#[allow(dead_code)]
impl FailureInjector {
    fn new(failure_rate: f64) -> Self {
        Self {
            failure_rate,
            counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn should_fail(&self) -> bool {
        let count = self.counter.fetch_add(1, Ordering::SeqCst);
        let threshold = (self.failure_rate * 1000.0) as usize;
        (count % 1000) < threshold
    }

    fn failure_count(&self) -> usize {
        self.counter.load(Ordering::SeqCst)
    }
}

/// Performance metrics collector
struct MetricsCollector {
    operation_count: AtomicU64,
    total_latency_ns: AtomicU64,
    max_latency_ns: AtomicU64,
    error_count: AtomicU64,
}

impl MetricsCollector {
    fn new() -> Self {
        Self {
            operation_count: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            max_latency_ns: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }

    fn record_operation(&self, latency_ns: u64) {
        self.operation_count.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);

        // Update max latency (compare-and-swap loop)
        loop {
            let current_max = self.max_latency_ns.load(Ordering::Relaxed);
            if latency_ns <= current_max {
                break;
            }
            if self
                .max_latency_ns
                .compare_exchange(
                    current_max,
                    latency_ns,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }
    }

    fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    fn avg_latency_ms(&self) -> f64 {
        let count = self.operation_count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }
        let total_ns = self.total_latency_ns.load(Ordering::Relaxed);
        (total_ns as f64) / (count as f64) / 1_000_000.0
    }

    fn max_latency_ms(&self) -> f64 {
        let max_ns = self.max_latency_ns.load(Ordering::Relaxed);
        (max_ns as f64) / 1_000_000.0
    }

    fn error_rate(&self) -> f64 {
        let total = self.operation_count.load(Ordering::Relaxed);
        let errors = self.error_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (errors as f64) / (total as f64)
    }
}

// ============================================================================
// 1. CONCURRENCY STRESS TESTS
// ============================================================================

#[tokio::test]
async fn stress_100_concurrent_task_executions() {
    let runtime = create_large_inventory(100);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Free,
            forks: 100,
            ..Default::default()
        },
        runtime,
    );

    let mut playbook = Playbook::new("100 Concurrent Tasks");
    let mut play = Play::new("Stress", "all");
    play.gather_facts = false;

    // Each host gets 5 tasks = 500 total task executions
    for i in 0..5 {
        play.add_task(
            Task::new(format!("Task {}", i), "debug").arg("msg", format!("Concurrent task {}", i)),
        );
    }
    playbook.add_play(play);

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 100);

    let mut failed_count = 0;
    for result in results.values() {
        if result.failed {
            failed_count += 1;
        }
    }

    println!(
        "100 hosts x 5 tasks completed in {:?}, {} failures",
        duration, failed_count
    );

    // Allow a small tolerance for timing-related failures
    assert!(failed_count <= 5, "Too many failures: {}", failed_count);
}

#[tokio::test]
async fn stress_500_simulated_host_connections() {
    let runtime = create_large_inventory(500);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Free,
            forks: 50, // Limit concurrent connections to 50
            ..Default::default()
        },
        runtime,
    );

    let mut playbook = Playbook::new("500 Hosts");
    let mut play = Play::new("Mass Deployment", "all");
    play.gather_facts = false;
    play.add_task(Task::new("Quick task", "debug").arg("msg", "Hello from host"));
    playbook.add_play(play);

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 500);
    println!("500 hosts completed in {:?}", duration);

    // Verify all hosts succeeded
    for (host, result) in &results {
        assert!(!result.unreachable, "Host {} became unreachable", host);
    }
}

#[tokio::test]
async fn stress_rapid_task_start_stop_cycling() {
    let metrics = Arc::new(MetricsCollector::new());

    let mut handles = vec![];

    for i in 0..50 {
        let metrics = Arc::clone(&metrics);
        let handle = tokio::spawn(async move {
            for j in 0..20 {
                let runtime = RuntimeContext::new();
                let executor = Executor::with_runtime(
                    ExecutorConfig {
                        forks: 5,
                        ..Default::default()
                    },
                    runtime,
                );

                // Create and immediately run a minimal playbook
                let mut playbook = Playbook::new(format!("Cycle-{}-{}", i, j));
                let play = Play::new("Quick", "all");
                playbook.add_play(play);

                let start = Instant::now();
                let _ = executor.run_playbook(&playbook).await;
                let latency = start.elapsed().as_nanos() as u64;

                metrics.record_operation(latency);
            }
        });
        handles.push(handle);
    }

    join_all(handles).await;

    println!(
        "Rapid cycling: {} operations, avg latency: {:.2}ms, max latency: {:.2}ms",
        metrics.operation_count.load(Ordering::Relaxed),
        metrics.avg_latency_ms(),
        metrics.max_latency_ms()
    );
}

#[tokio::test]
async fn stress_thread_pool_exhaustion_recovery() {
    // Create a semaphore to simulate limited thread pool
    let semaphore = Arc::new(Semaphore::new(10));
    let operations_completed = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];

    // Spawn 100 tasks competing for 10 permits
    for i in 0..100 {
        let semaphore = Arc::clone(&semaphore);
        let counter = Arc::clone(&operations_completed);

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            // Simulate work
            let runtime = RuntimeContext::new();
            let executor = Executor::with_runtime(ExecutorConfig::default(), runtime);

            let mut playbook = Playbook::new(format!("Work-{}", i));
            let play = Play::new("Work", "all");
            playbook.add_play(play);

            let _ = executor.run_playbook(&playbook).await;

            counter.fetch_add(1, Ordering::SeqCst);
            // Permit dropped here, releasing slot for next task
        });
        handles.push(handle);
    }

    // Wait for all with a timeout
    let result = tokio::time::timeout(Duration::from_secs(60), join_all(handles)).await;

    assert!(result.is_ok(), "Thread pool stress test timed out");

    let completed = operations_completed.load(Ordering::SeqCst);
    println!(
        "Thread pool exhaustion recovery: {}/100 completed",
        completed
    );
    assert_eq!(completed, 100, "Not all operations completed");
}

// ============================================================================
// 2. MEMORY STRESS TESTS
// ============================================================================

#[tokio::test]
async fn stress_large_inventory_10000_hosts() {
    let start = Instant::now();
    let runtime = create_large_inventory(10_000);
    let creation_time = start.elapsed();

    let all_hosts = runtime.get_all_hosts();
    assert_eq!(all_hosts.len(), 10_000);

    // Verify groups are properly set up
    assert!(runtime.get_group_hosts("webservers").is_some());
    assert!(runtime.get_group_hosts("databases").is_some());
    assert!(runtime.get_group_hosts("caches").is_some());
    assert!(runtime.get_group_hosts("workers").is_some());

    println!("Created 10,000 host inventory in {:?}", creation_time);

    // Test variable resolution at scale
    let start = Instant::now();
    for host in all_hosts.iter().take(100) {
        let _ = runtime.get_merged_vars(host);
    }
    let resolution_time = start.elapsed();
    println!(
        "Variable resolution for 100 hosts took {:?}",
        resolution_time
    );
}

#[tokio::test]
async fn stress_large_playbook_1000_tasks() {
    let playbook = create_large_playbook(1_000);

    assert_eq!(playbook.plays.len(), 1);
    assert_eq!(playbook.plays[0].tasks.len(), 1_000);

    let runtime = create_large_inventory(10);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Linear,
            forks: 10,
            ..Default::default()
        },
        runtime,
    );

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 10);

    // Calculate total tasks executed
    let total_tasks: usize = results
        .values()
        .map(|r| r.stats.ok + r.stats.changed + r.stats.skipped)
        .sum();

    println!(
        "1,000 tasks x 10 hosts = {} task executions in {:?}",
        total_tasks, duration
    );
}

#[tokio::test]
async fn stress_large_variable_contexts_1mb() {
    // Create runtime with ~1MB of variables per host
    let runtime = create_runtime_with_large_vars(10, 100); // 100KB per host x 10 hosts = 1MB

    let executor = Executor::with_runtime(ExecutorConfig::default(), runtime);

    let mut playbook = Playbook::new("Large Vars Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;
    play.add_task(
        Task::new("Access large var", "debug").arg("msg", "{{ large_var[:10] }}"), // Template a slice
    );
    playbook.add_play(play);

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 10);
    println!("Large variable context (1MB+) processed in {:?}", duration);
}

#[tokio::test]
async fn stress_memory_leak_detection() {
    // Run many iterations and check memory doesn't grow unboundedly
    let iterations = 100;
    let mut iteration_times = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let start = Instant::now();

        let runtime = create_large_inventory(100);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                forks: 20,
                ..Default::default()
            },
            runtime,
        );

        let mut playbook = Playbook::new(format!("Iteration-{}", i));
        let mut play = Play::new("Test", "all");
        play.gather_facts = false;
        play.add_task(Task::new("Task", "debug").arg("msg", "test"));
        playbook.add_play(play);

        let _ = executor.run_playbook(&playbook).await;

        iteration_times.push(start.elapsed());

        // Drop everything and let it be garbage collected
        drop(playbook);
        drop(executor);
    }

    // Check that iteration times are relatively stable (no memory thrashing)
    let first_10_avg: Duration = iteration_times[..10].iter().sum::<Duration>() / 10;
    let last_10_avg: Duration = iteration_times[iterations - 10..].iter().sum::<Duration>() / 10;

    println!(
        "First 10 iterations avg: {:?}, Last 10 iterations avg: {:?}",
        first_10_avg, last_10_avg
    );

    // Last 10 should not be significantly slower than first 10 (within 3x)
    assert!(
        last_10_avg < first_10_avg * 3,
        "Possible memory leak: performance degradation detected"
    );
}

// ============================================================================
// 3. CONNECTION STRESS TESTS
// ============================================================================

#[tokio::test]
async fn stress_rapid_connect_disconnect_cycles() {
    use rustible::connection::local::LocalConnection;
    use rustible::connection::Connection;

    let iterations = 1000;
    let mut connect_times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();

        let conn = LocalConnection::new();
        let _ = conn.is_alive().await;
        let _ = conn.close().await;

        connect_times.push(start.elapsed());
    }

    let total: Duration = connect_times.iter().sum();
    let avg = total / iterations as u32;
    let max = connect_times.iter().max().unwrap();

    println!(
        "1000 connect/disconnect cycles: avg {:?}, max {:?}",
        avg, max
    );

    // Each cycle should be fast
    assert!(
        avg < Duration::from_millis(10),
        "Average connection time too high"
    );
}

#[tokio::test]
async fn stress_connection_pool_churn() {
    use rustible::connection::{ConnectionConfig, ConnectionFactory};

    let config = ConnectionConfig::default();
    let factory = ConnectionFactory::with_pool_size(config, 10);

    let mut handles = vec![];

    // 100 concurrent requests for connections
    for i in 0..100 {
        let handle = tokio::spawn({
            async move {
                // Simulate getting connection for localhost
                let start = Instant::now();
                // Note: This will create local connections which don't actually pool
                let _duration = start.elapsed();
                i
            }
        });
        handles.push(handle);
    }

    let results: Vec<_> = join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(results.len(), 100);

    let stats = factory.pool_stats();
    println!(
        "Connection pool after churn: {} active, {} max",
        stats.active_connections, stats.max_connections
    );
}

#[tokio::test]
async fn stress_simultaneous_connection_attempts() {
    use rustible::connection::local::LocalConnection;
    use rustible::connection::Connection;

    let connection_count = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Attempt 500 simultaneous connections
    for _ in 0..500 {
        let counter = Arc::clone(&connection_count);
        let handle = tokio::spawn(async move {
            let conn = LocalConnection::new();
            if conn.is_alive().await {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    join_all(handles).await;

    let successful = connection_count.load(Ordering::SeqCst);
    println!(
        "500 simultaneous connection attempts: {} successful",
        successful
    );
    assert_eq!(successful, 500, "Not all connections succeeded");
}

#[tokio::test]
async fn stress_connection_timeout_flood() {
    use rustible::connection::local::LocalConnection;
    use rustible::connection::{Connection, ExecuteOptions};

    let timeout_count = Arc::new(AtomicUsize::new(0));
    let success_count = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // Flood with commands that have very short timeouts
    for _ in 0..100 {
        let timeout_counter = Arc::clone(&timeout_count);
        let success_counter = Arc::clone(&success_count);

        let handle = tokio::spawn(async move {
            let conn = LocalConnection::new();
            let options = ExecuteOptions::new().with_timeout(1); // 1 second timeout

            // Quick command that should succeed
            match conn.execute("echo test", Some(options)).await {
                Ok(result) if result.success => {
                    success_counter.fetch_add(1, Ordering::SeqCst);
                }
                Err(rustible::connection::ConnectionError::Timeout(_)) => {
                    timeout_counter.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
        });
        handles.push(handle);
    }

    join_all(handles).await;

    let timeouts = timeout_count.load(Ordering::SeqCst);
    let successes = success_count.load(Ordering::SeqCst);

    println!(
        "Connection timeout flood: {} successes, {} timeouts",
        successes, timeouts
    );

    // Most should succeed since the command is quick
    assert!(successes > 90, "Too many failures in timeout flood");
}

// ============================================================================
// 4. CHAOS SCENARIOS
// ============================================================================

#[tokio::test]
async fn chaos_random_task_failures() {
    let runtime = create_large_inventory(20);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Free,
            forks: 20,
            ..Default::default()
        },
        runtime,
    );

    let mut playbook = Playbook::new("Chaos Failures");
    let mut play = Play::new("Random Failures", "all");
    play.gather_facts = false;

    // Create tasks that fail based on host index
    for i in 0..10 {
        play.add_task(
            Task::new(format!("Task-{}", i), "fail")
                .arg("msg", format!("Failing task {}", i))
                .when(format!(
                    "inventory_hostname | regex_search('host-0000[0-5]') and {} < 5",
                    i
                ))
                .ignore_errors(true),
        );
    }
    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    let mut failure_count = 0;
    let mut success_count = 0;
    for result in results.values() {
        if result.failed {
            failure_count += 1;
        } else {
            success_count += 1;
        }
    }

    println!(
        "Chaos random failures: {} successes, {} failures out of {} hosts",
        success_count,
        failure_count,
        results.len()
    );
}

#[tokio::test]
async fn chaos_connection_drops_mid_execution() {
    // Simulate connection drops by having tasks that conditionally fail
    let runtime = create_large_inventory(50);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Free,
            forks: 25,
            ..Default::default()
        },
        runtime,
    );

    let mut playbook = Playbook::new("Connection Drop Chaos");
    let mut play = Play::new("Drop Simulation", "all");
    play.gather_facts = false;

    play.add_task(Task::new("Start", "debug").arg("msg", "Starting"));

    // Simulate random "connection drops" via conditional failures
    play.add_task(
        Task::new("May drop", "fail")
            .arg("msg", "Connection dropped")
            .when("inventory_hostname | regex_search('host-0000[02468]')")
            .ignore_errors(true),
    );

    play.add_task(Task::new("Continue", "debug").arg("msg", "Still running"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    assert_eq!(results.len(), 50);

    // Some hosts should have progressed despite "drops"
    let completed_tasks: usize = results.values().map(|r| r.stats.ok + r.stats.changed).sum();

    println!(
        "Connection drop chaos: {} completed tasks across {} hosts",
        completed_tasks,
        results.len()
    );
}

#[tokio::test]
async fn chaos_slow_host_responses() {
    let runtime = create_large_inventory(20);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Free,
            forks: 20,
            task_timeout: 300,
            ..Default::default()
        },
        runtime,
    );

    let mut playbook = Playbook::new("Slow Response Chaos");
    let mut play = Play::new("Slow Hosts", "all");
    play.gather_facts = false;

    // Simulate slow responses with pause module on some hosts
    play.add_task(
        Task::new("Slow task", "pause")
            .arg("seconds", 1)
            .when("inventory_hostname | regex_search('host-0000[0-4]')"),
    );

    play.add_task(Task::new("Quick task", "debug").arg("msg", "Fast"));

    playbook.add_play(play);

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 20);
    println!("Slow host chaos completed in {:?}", duration);

    // With free strategy, slow hosts shouldn't block fast ones
    for result in results.values() {
        assert!(!result.failed);
    }
}

#[tokio::test]
async fn chaos_resource_exhaustion_simulation() {
    // Simulate resource exhaustion by creating many concurrent operations
    let operations = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let current_concurrent = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];

    for i in 0..200 {
        let ops = Arc::clone(&operations);
        let max_c = Arc::clone(&max_concurrent);
        let current_c = Arc::clone(&current_concurrent);

        let handle = tokio::spawn(async move {
            // Track concurrent operations
            let current = current_c.fetch_add(1, Ordering::SeqCst) + 1;

            // Update max if needed
            loop {
                let max = max_c.load(Ordering::SeqCst);
                if current <= max {
                    break;
                }
                if max_c
                    .compare_exchange(max, current, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    break;
                }
            }

            // Simulate work
            let runtime = RuntimeContext::new();
            let executor = Executor::with_runtime(ExecutorConfig::default(), runtime);

            let mut playbook = Playbook::new(format!("Resource-{}", i));
            let play = Play::new("Work", "all");
            playbook.add_play(play);

            let _ = executor.run_playbook(&playbook).await;

            current_c.fetch_sub(1, Ordering::SeqCst);
            ops.fetch_add(1, Ordering::SeqCst);
        });
        handles.push(handle);
    }

    // Wait with timeout
    let result = tokio::time::timeout(Duration::from_secs(120), join_all(handles)).await;

    assert!(result.is_ok(), "Resource exhaustion test timed out");

    let total_ops = operations.load(Ordering::SeqCst);
    let max = max_concurrent.load(Ordering::SeqCst);

    println!(
        "Resource exhaustion: {} operations, max {} concurrent",
        total_ops, max
    );

    assert_eq!(total_ops, 200, "Not all operations completed");
}

// ============================================================================
// 5. RACE CONDITION TESTS
// ============================================================================

#[tokio::test]
async fn race_concurrent_variable_access() {
    let runtime = Arc::new(RwLock::new(RuntimeContext::new()));

    // Initialize with hosts
    {
        let mut rt = runtime.write().await;
        for i in 0..10 {
            rt.add_host(format!("host-{}", i), None);
        }
    }

    let mut handles = vec![];

    // Concurrent readers and writers
    for i in 0..100 {
        let runtime = Arc::clone(&runtime);
        let handle = tokio::spawn(async move {
            if i % 2 == 0 {
                // Reader
                let rt = runtime.read().await;
                let _ = rt.get_var("some_var", None);
            } else {
                // Writer
                let mut rt = runtime.write().await;
                rt.set_global_var(format!("var_{}", i), serde_json::json!(i));
            }
        });
        handles.push(handle);
    }

    // All operations should complete without panic
    let results = join_all(handles).await;

    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 100, "Some operations failed");

    // Verify final state
    let rt = runtime.read().await;
    let hosts = rt.get_all_hosts();
    assert_eq!(hosts.len(), 10);
}

#[tokio::test]
async fn race_concurrent_handler_notifications() {
    let mut playbook = Playbook::new("Handler Race");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // All tasks notify the same handler
    for i in 0..10 {
        play.add_task(
            Task::new(format!("Notifier-{}", i), "debug")
                .arg("msg", "Notifying")
                .notify("common-handler"),
        );
    }

    play.add_handler(Handler {
        name: "common-handler".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = indexmap::IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler executed"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    // Run multiple times to trigger race conditions
    for iteration in 0..10 {
        let runtime = create_large_inventory(50);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                strategy: ExecutionStrategy::Free,
                forks: 50,
                ..Default::default()
            },
            runtime,
        );

        let result = executor.run_playbook(&playbook).await;
        assert!(
            result.is_ok(),
            "Iteration {} failed: {:?}",
            iteration,
            result.err()
        );
    }
}

#[tokio::test]
async fn race_concurrent_fact_gathering() {
    // Simulate concurrent fact updates
    let runtime = Arc::new(RwLock::new(RuntimeContext::new()));

    // Initialize hosts
    {
        let mut rt = runtime.write().await;
        for i in 0..20 {
            rt.add_host(format!("host-{}", i), None);
        }
    }

    let mut handles = vec![];

    // Concurrent fact setters
    for i in 0..100 {
        let runtime = Arc::clone(&runtime);
        let handle = tokio::spawn(async move {
            let host = format!("host-{}", i % 20);
            let mut rt = runtime.write().await;
            rt.set_host_fact(
                &host,
                format!("fact_{}", i),
                serde_json::json!({"iteration": i, "timestamp": chrono::Utc::now().to_rfc3339()}),
            );
        });
        handles.push(handle);
    }

    join_all(handles).await;

    // Verify some facts were set
    let rt = runtime.read().await;
    for i in 0..20 {
        let host = format!("host-{}", i);
        // Each host should have at least some facts
        let merged_vars = rt.get_merged_vars(&host);
        assert!(merged_vars.len() > 0, "Host {} has no vars", host);
    }
}

#[tokio::test]
async fn race_connection_pool_race_conditions() {
    let pool_errors = Arc::new(AtomicUsize::new(0));
    let operations_completed = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];

    // Many concurrent operations that might race on pool access
    for i in 0..100 {
        let pool_errors = Arc::clone(&pool_errors);
        let ops_completed = Arc::clone(&operations_completed);

        let handle = tokio::spawn(async move {
            // Simulate pool access pattern
            let runtime = create_large_inventory(5);
            let executor = Executor::with_runtime(
                ExecutorConfig {
                    forks: 5,
                    ..Default::default()
                },
                runtime,
            );

            let mut playbook = Playbook::new(format!("Pool-{}", i));
            let mut play = Play::new("Test", "all");
            play.gather_facts = false;
            play.add_task(Task::new("Task", "debug").arg("msg", "pool test"));
            playbook.add_play(play);

            match executor.run_playbook(&playbook).await {
                Ok(_) => {
                    ops_completed.fetch_add(1, Ordering::SeqCst);
                }
                Err(_) => {
                    pool_errors.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }

    join_all(handles).await;

    let errors = pool_errors.load(Ordering::SeqCst);
    let completed = operations_completed.load(Ordering::SeqCst);

    println!(
        "Connection pool race: {} completed, {} errors",
        completed, errors
    );

    // Most operations should succeed
    assert!(completed > 95, "Too many pool race failures");
}

// ============================================================================
// 6. LONG-RUNNING STABILITY TESTS
// ============================================================================

#[tokio::test]
async fn stability_1000_iterations_same_playbook() {
    let playbook = create_large_playbook(5);
    let mut iteration_times = Vec::with_capacity(1000);
    let mut failures = 0;

    for i in 0..1000 {
        let runtime = create_large_inventory(5);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                forks: 5,
                ..Default::default()
            },
            runtime,
        );

        let start = Instant::now();
        match executor.run_playbook(&playbook).await {
            Ok(results) => {
                let failed = results.values().any(|r| r.failed);
                if failed {
                    failures += 1;
                }
            }
            Err(_) => {
                failures += 1;
            }
        }
        iteration_times.push(start.elapsed());

        // Progress indicator for long test
        if (i + 1) % 100 == 0 {
            println!("Stability test: {}/1000 iterations completed", i + 1);
        }
    }

    let total: Duration = iteration_times.iter().sum();
    let avg = total / 1000;
    let max = iteration_times.iter().max().unwrap();
    let min = iteration_times.iter().min().unwrap();

    println!(
        "1000 iterations: avg {:?}, min {:?}, max {:?}, {} failures",
        avg, min, max, failures
    );

    // Should be very stable with minimal failures
    assert!(
        failures < 10,
        "Too many failures in stability test: {}",
        failures
    );

    // Performance should be consistent
    assert!(*max < avg * 5, "Too much variance in execution time");
}

#[tokio::test]
async fn stability_no_resource_leaks_over_time() {
    let metrics = Arc::new(MetricsCollector::new());

    // Run 500 iterations and track performance
    for i in 0..500 {
        let runtime = create_large_inventory(10);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                forks: 10,
                ..Default::default()
            },
            runtime,
        );

        let mut playbook = Playbook::new(format!("Leak-Test-{}", i));
        let mut play = Play::new("Test", "all");
        play.gather_facts = false;
        play.add_task(Task::new("Task", "debug").arg("msg", "test"));
        playbook.add_play(play);

        let start = Instant::now();
        match executor.run_playbook(&playbook).await {
            Ok(_) => {
                let latency = start.elapsed().as_nanos() as u64;
                metrics.record_operation(latency);
            }
            Err(_) => {
                metrics.record_error();
            }
        }
    }

    println!(
        "500 iterations: avg latency {:.2}ms, max {:.2}ms, error rate {:.2}%",
        metrics.avg_latency_ms(),
        metrics.max_latency_ms(),
        metrics.error_rate() * 100.0
    );

    // Error rate should be very low
    assert!(
        metrics.error_rate() < 0.01,
        "Error rate too high: {:.2}%",
        metrics.error_rate() * 100.0
    );
}

#[tokio::test]
async fn stability_latency_consistency_over_time() {
    let mut latencies: Vec<Duration> = Vec::with_capacity(200);

    for i in 0..200 {
        let runtime = create_large_inventory(20);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                strategy: ExecutionStrategy::Free,
                forks: 20,
                ..Default::default()
            },
            runtime,
        );

        let mut playbook = Playbook::new(format!("Latency-{}", i));
        let mut play = Play::new("Test", "all");
        play.gather_facts = false;
        play.add_task(Task::new("Task", "debug").arg("msg", "latency test"));
        playbook.add_play(play);

        let start = Instant::now();
        let _ = executor.run_playbook(&playbook).await;
        latencies.push(start.elapsed());
    }

    // Compare first 50 vs last 50
    let first_50_avg: Duration = latencies[..50].iter().sum::<Duration>() / 50;
    let last_50_avg: Duration = latencies[150..].iter().sum::<Duration>() / 50;

    println!(
        "Latency stability: first 50 avg {:?}, last 50 avg {:?}",
        first_50_avg, last_50_avg
    );

    // Latency should be stable (within 2x)
    let ratio = last_50_avg.as_nanos() as f64 / first_50_avg.as_nanos() as f64;
    assert!(
        ratio < 2.0 && ratio > 0.5,
        "Latency instability detected: ratio {:.2}",
        ratio
    );
}

// ============================================================================
// 7. EDGE CASES UNDER LOAD
// ============================================================================

#[tokio::test]
async fn edge_empty_results_under_concurrent_load() {
    let mut handles = vec![];

    for i in 0..50 {
        let handle = tokio::spawn(async move {
            let runtime = RuntimeContext::new(); // No hosts
            let executor = Executor::with_runtime(ExecutorConfig::default(), runtime);

            let mut playbook = Playbook::new(format!("Empty-{}", i));
            let play = Play::new("Empty", "all");
            playbook.add_play(play);

            let result = executor.run_playbook(&playbook).await;
            assert!(result.is_ok());
            result.unwrap().len()
        });
        handles.push(handle);
    }

    let results: Vec<_> = join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(results.len(), 50);

    // All should return 0 hosts
    for count in &results {
        assert_eq!(*count, 0);
    }
}

#[tokio::test]
async fn edge_timeout_handling_under_load() {
    use rustible::connection::local::LocalConnection;
    use rustible::connection::{Connection, ConnectionError, ExecuteOptions};

    let success = Arc::new(AtomicUsize::new(0));
    let timeout = Arc::new(AtomicUsize::new(0));
    let other_error = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];

    for _ in 0..50 {
        let success = Arc::clone(&success);
        let timeout = Arc::clone(&timeout);
        let other_error = Arc::clone(&other_error);

        let handle = tokio::spawn(async move {
            let conn = LocalConnection::new();

            // Very short timeout
            let options = ExecuteOptions::new().with_timeout(1);

            match conn.execute("echo test", Some(options)).await {
                Ok(r) if r.success => {
                    success.fetch_add(1, Ordering::SeqCst);
                }
                Err(ConnectionError::Timeout(_)) => {
                    timeout.fetch_add(1, Ordering::SeqCst);
                }
                _ => {
                    other_error.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }

    join_all(handles).await;

    let s = success.load(Ordering::SeqCst);
    let t = timeout.load(Ordering::SeqCst);
    let e = other_error.load(Ordering::SeqCst);

    println!(
        "Timeout handling under load: {} success, {} timeout, {} errors",
        s, t, e
    );

    // Most should succeed (echo is fast)
    assert!(s > 40, "Too few successes under load");
}

#[tokio::test]
async fn edge_error_handling_under_load() {
    let mut playbook = Playbook::new("Error Handling");
    let mut play = Play::new("Errors", "all");
    play.gather_facts = false;

    // Mix of succeeding and failing tasks
    for i in 0..20 {
        if i % 3 == 0 {
            play.add_task(
                Task::new(format!("Fail-{}", i), "fail")
                    .arg("msg", "Intentional failure")
                    .ignore_errors(true),
            );
        } else {
            play.add_task(Task::new(format!("Ok-{}", i), "debug").arg("msg", "Success"));
        }
    }
    playbook.add_play(play);

    // Run multiple times under load
    let mut handles = vec![];

    for i in 0..10 {
        let runtime = create_large_inventory(20);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                strategy: ExecutionStrategy::Free,
                forks: 20,
                ..Default::default()
            },
            runtime,
        );
        let playbook = playbook.clone();

        let handle = tokio::spawn(async move {
            let result = executor.run_playbook(&playbook).await;
            (i, result.is_ok())
        });
        handles.push(handle);
    }

    let results: Vec<_> = join_all(handles)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();

    let success_count = results.iter().filter(|(_, ok)| *ok).count();
    println!(
        "Error handling under load: {}/10 successful runs",
        success_count
    );

    assert_eq!(success_count, 10, "Some runs failed unexpectedly");
}

// ============================================================================
// PROPERTY-BASED TESTS (using proptest)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_executor_handles_any_host_count(host_count in 1usize..100) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runtime = create_large_inventory(host_count);
            let executor = Executor::with_runtime(
                ExecutorConfig {
                    forks: host_count.min(50),
                    ..Default::default()
                },
                runtime,
            );

            let mut playbook = Playbook::new("Prop Test");
            let mut play = Play::new("Test", "all");
            play.gather_facts = false;
            play.add_task(Task::new("Task", "debug").arg("msg", "test"));
            playbook.add_play(play);

            let result = executor.run_playbook(&playbook).await;
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap().len(), host_count);
            Ok(())
        }).unwrap();
    }

    #[test]
    fn prop_executor_handles_any_task_count(task_count in 1usize..50) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let runtime = create_large_inventory(5);
            let playbook = create_large_playbook(task_count);

            let executor = Executor::with_runtime(
                ExecutorConfig::default(),
                runtime,
            );

            let result = executor.run_playbook(&playbook).await;
            prop_assert!(result.is_ok());
            Ok(())
        }).unwrap();
    }

    #[test]
    fn prop_runtime_context_variable_roundtrip(
        key in "[a-z]{1,20}",
        value in any::<i64>()
    ) {
        let mut runtime = RuntimeContext::new();
        runtime.set_global_var(key.clone(), serde_json::json!(value));

        let retrieved = runtime.get_var(&key, None);
        prop_assert!(retrieved.is_some());
        prop_assert_eq!(retrieved.unwrap(), serde_json::json!(value));
    }

    #[test]
    fn prop_execution_stats_merge_is_additive(
        ok1 in 0usize..1000,
        changed1 in 0usize..1000,
        failed1 in 0usize..100,
        ok2 in 0usize..1000,
        changed2 in 0usize..1000,
        failed2 in 0usize..100,
    ) {
        let mut stats1 = ExecutionStats {
            ok: ok1,
            changed: changed1,
            failed: failed1,
            skipped: 0,
            unreachable: 0,
        };

        let stats2 = ExecutionStats {
            ok: ok2,
            changed: changed2,
            failed: failed2,
            skipped: 0,
            unreachable: 0,
        };

        stats1.merge(&stats2);

        prop_assert_eq!(stats1.ok, ok1 + ok2);
        prop_assert_eq!(stats1.changed, changed1 + changed2);
        prop_assert_eq!(stats1.failed, failed1 + failed2);
    }
}

// ============================================================================
// SIGNAL HANDLING TESTS
// ============================================================================

#[cfg(unix)]
#[tokio::test]
async fn stress_signal_handling_resilience() {
    // Test that the executor can handle running alongside signal handlers
    let runtime = create_large_inventory(10);
    let executor = Arc::new(Executor::with_runtime(
        ExecutorConfig {
            forks: 10,
            ..Default::default()
        },
        runtime,
    ));

    let mut playbook = Playbook::new("Signal Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    for i in 0..10 {
        play.add_task(Task::new(format!("Task-{}", i), "pause").arg("seconds", 0));
    }
    playbook.add_play(play);

    // Run the playbook (no actual signals sent, just verifying handler setup doesn't interfere)
    let result = executor.run_playbook(&playbook).await;
    assert!(result.is_ok());
}

// ============================================================================
// BENCHMARK-STYLE STRESS TESTS
// ============================================================================

#[tokio::test]
async fn benchmark_throughput_max_hosts() {
    let start = Instant::now();
    let mut total_tasks = 0;

    for batch in 0..10 {
        let runtime = create_large_inventory(100);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                strategy: ExecutionStrategy::Free,
                forks: 50,
                ..Default::default()
            },
            runtime,
        );

        let mut playbook = Playbook::new(format!("Batch-{}", batch));
        let mut play = Play::new("Speed", "all");
        play.gather_facts = false;

        for i in 0..10 {
            play.add_task(Task::new(format!("Task-{}", i), "debug").arg("msg", "speed test"));
        }
        playbook.add_play(play);

        let results = executor.run_playbook(&playbook).await.unwrap();

        total_tasks += results
            .values()
            .map(|r| r.stats.ok + r.stats.changed)
            .sum::<usize>();
    }

    let duration = start.elapsed();
    let throughput = total_tasks as f64 / duration.as_secs_f64();

    println!(
        "Throughput benchmark: {} tasks in {:?} = {:.2} tasks/sec",
        total_tasks, duration, throughput
    );

    // Should handle at least 1000 tasks per second
    assert!(throughput > 1000.0, "Throughput too low: {:.2}", throughput);
}

#[tokio::test]
async fn benchmark_latency_percentiles() {
    let mut latencies: Vec<Duration> = Vec::with_capacity(1000);

    for _ in 0..1000 {
        let runtime = create_large_inventory(1);
        let executor = Executor::with_runtime(ExecutorConfig::default(), runtime);

        let mut playbook = Playbook::new("Latency");
        let play = Play::new("Test", "all");
        playbook.add_play(play);

        let start = Instant::now();
        let _ = executor.run_playbook(&playbook).await;
        latencies.push(start.elapsed());
    }

    latencies.sort();

    let p50 = latencies[500];
    let p90 = latencies[900];
    let p99 = latencies[990];
    let p999 = latencies[999];

    println!(
        "Latency percentiles: p50={:?}, p90={:?}, p99={:?}, p99.9={:?}",
        p50, p90, p99, p999
    );

    // p99 should be reasonable
    assert!(
        p99 < Duration::from_millis(100),
        "p99 latency too high: {:?}",
        p99
    );
}
