//! Scale and Stress Tests for Rustible (TEST-08)
//!
//! This test suite validates Rustible's ability to handle large-scale operations:
//!
//! 1. 1000+ Host Inventory - Tests inventory creation, parsing, and operations at scale
//! 2. 500+ Task Playbook - Tests playbook execution with many tasks
//! 3. Concurrent Execution Limits - Tests fork limiting and semaphore behavior
//! 4. Memory Under Sustained Load - Tests for memory leaks and stability
//! 5. Connection Pool Under Pressure - Tests pool exhaustion and recovery
//!
//! Run with: cargo test --test scale_stress_tests -- --test-threads=1
//!
//! For extreme tests (marked #[ignore]):
//!   cargo test --test scale_stress_tests extreme_ -- --ignored --test-threads=1

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::join_all;
use tokio::sync::{RwLock, Semaphore};

use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::RuntimeContext;
use rustible::executor::task::{Handler, Task};
use rustible::executor::{ExecutionStats, ExecutionStrategy, Executor, ExecutorConfig};
use rustible::inventory::{Group, Host, Inventory};

// ============================================================================
// Helper Utilities
// ============================================================================

/// Performance metrics collector for tracking operations
#[derive(Debug)]
struct MetricsCollector {
    operation_count: AtomicUsize,
    total_latency_ns: AtomicUsize,
    max_latency_ns: AtomicUsize,
    min_latency_ns: AtomicUsize,
    error_count: AtomicUsize,
}

impl MetricsCollector {
    fn new() -> Self {
        Self {
            operation_count: AtomicUsize::new(0),
            total_latency_ns: AtomicUsize::new(0),
            max_latency_ns: AtomicUsize::new(0),
            min_latency_ns: AtomicUsize::new(usize::MAX),
            error_count: AtomicUsize::new(0),
        }
    }

    fn record_operation(&self, latency_ns: u64) {
        let latency = latency_ns as usize;
        self.operation_count.fetch_add(1, Ordering::Relaxed);
        self.total_latency_ns.fetch_add(latency, Ordering::Relaxed);

        // Update max latency
        loop {
            let current_max = self.max_latency_ns.load(Ordering::Relaxed);
            if latency <= current_max {
                break;
            }
            if self
                .max_latency_ns
                .compare_exchange(current_max, latency, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        // Update min latency
        loop {
            let current_min = self.min_latency_ns.load(Ordering::Relaxed);
            if latency >= current_min {
                break;
            }
            if self
                .min_latency_ns
                .compare_exchange(current_min, latency, Ordering::Relaxed, Ordering::Relaxed)
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

    fn min_latency_ms(&self) -> f64 {
        let min_ns = self.min_latency_ns.load(Ordering::Relaxed);
        if min_ns == usize::MAX {
            return 0.0;
        }
        (min_ns as f64) / 1_000_000.0
    }

    fn error_rate(&self) -> f64 {
        let total = self.operation_count.load(Ordering::Relaxed);
        let errors = self.error_count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        (errors as f64) / (total as f64)
    }

    fn operations(&self) -> usize {
        self.operation_count.load(Ordering::Relaxed)
    }

    fn errors(&self) -> usize {
        self.error_count.load(Ordering::Relaxed)
    }
}

/// Create a runtime context with the specified number of hosts
fn create_large_runtime(host_count: usize) -> RuntimeContext {
    let mut runtime = RuntimeContext::new();
    for i in 0..host_count {
        let host = format!("host-{:05}", i);
        let group = match i % 4 {
            0 => "webservers",
            1 => "databases",
            2 => "caches",
            _ => "workers",
        };
        runtime.add_host(host.clone(), Some(group));

        // Add some variables to each host
        runtime.set_host_var(
            &host,
            "http_port".to_string(),
            serde_json::json!(8080 + (i % 100)),
        );
        runtime.set_host_var(
            &host,
            "priority".to_string(),
            serde_json::json!(i % 10),
        );
    }
    runtime
}

/// Create a large inventory with the specified number of hosts and groups
fn create_large_inventory(host_count: usize, group_count: usize) -> Inventory {
    let mut inventory = Inventory::new();

    // Create groups
    let hosts_per_group = (host_count / group_count).max(1);
    for g in 0..group_count {
        let group_name = format!("group_{:04}", g);
        let mut group = Group::new(&group_name);

        // Add group variables
        for v in 0..5 {
            group.set_var(
                &format!("group_var_{}", v),
                serde_yaml::Value::String(format!("value_{}", v)),
            );
        }

        let _ = inventory.add_group(group);
    }

    // Create hosts
    for h in 0..host_count {
        let host_name = format!("host{:05}", h);
        let mut host = Host::new(&host_name);

        // Assign to group
        let group_idx = h / hosts_per_group;
        let group_name = format!("group_{:04}", group_idx.min(group_count - 1));
        host.add_to_group(group_name);
        host.add_to_group("all".to_string());

        // Add host variables
        for v in 0..3 {
            host.set_var(
                &format!("host_var_{}", v),
                serde_yaml::Value::String(format!("host_value_{}", v)),
            );
        }

        let _ = inventory.add_host(host);
    }

    inventory
}

/// Create a playbook with the specified number of tasks
fn create_large_playbook(task_count: usize) -> Playbook {
    let mut playbook = Playbook::new("Large Playbook");
    let mut play = Play::new("Large Play", "all");
    play.gather_facts = false;

    for i in 0..task_count {
        let task = Task::new(format!("Task-{:05}", i), "debug")
            .arg("msg", format!("Executing task {} of {}", i + 1, task_count));
        play.add_task(task);
    }

    playbook.add_play(play);
    playbook
}

/// Create a runtime with large variable contexts
fn create_runtime_with_large_vars(host_count: usize, var_size_kb: usize) -> RuntimeContext {
    let mut runtime = RuntimeContext::new();

    // Create a large value string
    let large_value: String = "x".repeat(var_size_kb * 1024);

    for i in 0..host_count {
        let host = format!("host-{:05}", i);
        runtime.add_host(host.clone(), None);

        // Add large variable
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
                            "data": large_value[..1024.min(large_value.len())].to_string(),
                            "index": i,
                        }
                    }
                }
            }),
        );
    }

    runtime
}

// ============================================================================
// 1. 1000+ HOST INVENTORY TESTS
// ============================================================================

/// Test creating and operating on a 1000 host inventory
#[test]
fn test_inventory_1000_hosts_creation() {
    let start = Instant::now();
    let inventory = create_large_inventory(1000, 20);
    let creation_time = start.elapsed();

    assert_eq!(inventory.host_count(), 1000);
    println!(
        "Created 1000-host inventory in {:?} ({:.2} hosts/ms)",
        creation_time,
        1000.0 / creation_time.as_millis() as f64
    );

    // Verify groups
    assert!(inventory.get_group("group_0000").is_some());
    assert!(inventory.get_group("group_0019").is_some());

    // Test pattern matching performance
    let pattern_start = Instant::now();
    let all_hosts = inventory.get_hosts_for_pattern("all").unwrap();
    let pattern_time = pattern_start.elapsed();

    assert_eq!(all_hosts.len(), 1000);
    println!(
        "Pattern 'all' matched 1000 hosts in {:?}",
        pattern_time
    );
}

/// Test inventory variable resolution at 1000+ hosts scale
#[test]
fn test_inventory_1000_hosts_variable_resolution() {
    let inventory = create_large_inventory(1000, 20);

    let start = Instant::now();
    let mut total_vars = 0;

    // Resolve variables for every 10th host
    for i in (0..1000).step_by(10) {
        let host_name = format!("host{:05}", i);
        if let Some(host) = inventory.get_host(&host_name) {
            let vars = inventory.get_host_vars(host);
            total_vars += vars.len();
        }
    }

    let duration = start.elapsed();
    println!(
        "Variable resolution for 100 hosts: {:?}, avg {} vars/host",
        duration,
        total_vars / 100
    );

    // Should complete in reasonable time
    assert!(
        duration < Duration::from_secs(5),
        "Variable resolution too slow: {:?}",
        duration
    );
}

/// Test 1500 host inventory creation and operations
#[test]
fn test_inventory_1500_hosts() {
    let start = Instant::now();
    let inventory = create_large_inventory(1500, 30);
    let creation_time = start.elapsed();

    assert_eq!(inventory.host_count(), 1500);
    println!("Created 1500-host inventory in {:?}", creation_time);

    // Test wildcard pattern matching
    let pattern_start = Instant::now();
    let web_hosts = inventory.get_hosts_for_pattern("host0*").unwrap();
    let pattern_time = pattern_start.elapsed();

    println!(
        "Wildcard pattern matched {} hosts in {:?}",
        web_hosts.len(),
        pattern_time
    );

    assert!(web_hosts.len() > 0);
}

/// Test 2000 host inventory (extreme)
#[test]
fn test_inventory_2000_hosts() {
    let start = Instant::now();
    let inventory = create_large_inventory(2000, 40);
    let creation_time = start.elapsed();

    assert_eq!(inventory.host_count(), 2000);
    println!(
        "Created 2000-host inventory in {:?} ({:.2} hosts/ms)",
        creation_time,
        2000.0 / creation_time.as_millis() as f64
    );

    // Performance should scale linearly
    assert!(
        creation_time < Duration::from_secs(10),
        "2000 host inventory creation too slow: {:?}",
        creation_time
    );
}

// ============================================================================
// 2. 500+ TASK PLAYBOOK TESTS
// ============================================================================

/// Test creating and executing a 500 task playbook
#[tokio::test]
async fn test_playbook_500_tasks() {
    let playbook = create_large_playbook(500);

    assert_eq!(playbook.plays.len(), 1);
    assert_eq!(playbook.plays[0].tasks.len(), 500);

    let runtime = create_large_runtime(10);
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

    let total_tasks: usize = results
        .values()
        .map(|r| r.stats.ok + r.stats.changed + r.stats.skipped)
        .sum();

    let throughput = total_tasks as f64 / duration.as_secs_f64();

    println!(
        "500 tasks x 10 hosts = {} task executions in {:?} ({:.1} tasks/sec)",
        total_tasks, duration, throughput
    );
}

/// Test 750 task playbook execution
#[tokio::test]
async fn test_playbook_750_tasks() {
    let playbook = create_large_playbook(750);

    assert_eq!(playbook.plays[0].tasks.len(), 750);

    let runtime = create_large_runtime(5);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Free,
            forks: 5,
            ..Default::default()
        },
        runtime,
    );

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    let total_tasks: usize = results
        .values()
        .map(|r| r.stats.ok + r.stats.changed)
        .sum();

    println!(
        "750 tasks x 5 hosts completed in {:?}, {} total task executions",
        duration, total_tasks
    );

    // Verify all hosts succeeded
    for (host, result) in &results {
        assert!(!result.failed, "Host {} failed", host);
    }
}

/// Test 1000 task playbook on single host (sequential)
#[tokio::test]
async fn test_playbook_1000_tasks_single_host() {
    let playbook = create_large_playbook(1000);

    let runtime = create_large_runtime(1);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Linear,
            forks: 1,
            ..Default::default()
        },
        runtime,
    );

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 1);

    let stats = results.values().next().unwrap();
    let total_ok = stats.stats.ok + stats.stats.changed;

    println!(
        "1000 sequential tasks completed in {:?}, {} successful",
        duration, total_ok
    );

    assert!(
        total_ok >= 900,
        "Too few successful tasks: {}",
        total_ok
    );
}

/// Test 500+ tasks with handlers
#[tokio::test]
async fn test_playbook_500_tasks_with_handlers() {
    let mut playbook = Playbook::new("Tasks With Handlers");
    let mut play = Play::new("Handler Test", "all");
    play.gather_facts = false;

    // Add 500 tasks, every 10th one notifies a handler
    for i in 0..500 {
        let mut task = Task::new(format!("Task-{:03}", i), "debug")
            .arg("msg", format!("Task {}", i));

        if i % 10 == 0 {
            task = task.notify("common-handler");
        }

        play.add_task(task);
    }

    // Add handler
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

    let runtime = create_large_runtime(5);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Linear,
            forks: 5,
            ..Default::default()
        },
        runtime,
    );

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 5);
    println!(
        "500 tasks with handlers completed in {:?}",
        duration
    );
}

// ============================================================================
// 3. CONCURRENT EXECUTION LIMIT TESTS
// ============================================================================

/// Test fork limiting with 50 concurrent operations
#[tokio::test]
async fn test_concurrent_fork_limit_50() {
    let runtime = create_large_runtime(100);
    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Free,
            forks: 50, // Limit to 50 concurrent
            ..Default::default()
        },
        runtime,
    );

    let mut playbook = Playbook::new("Fork Limit Test");
    let mut play = Play::new("Concurrent Test", "all");
    play.gather_facts = false;
    play.add_task(
        Task::new("Quick task", "debug")
            .arg("msg", "Hello from concurrent execution"),
    );
    playbook.add_play(play);

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 100);
    println!(
        "100 hosts with 50 fork limit completed in {:?}",
        duration
    );
}

/// Test semaphore-based resource limiting
#[tokio::test]
async fn test_semaphore_resource_limiting() {
    let semaphore = Arc::new(Semaphore::new(10));
    let operations_completed = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let current_concurrent = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];

    // Spawn 200 tasks competing for 10 permits
    for i in 0..200 {
        let sem = Arc::clone(&semaphore);
        let counter = Arc::clone(&operations_completed);
        let max_c = Arc::clone(&max_concurrent);
        let current_c = Arc::clone(&current_concurrent);

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            // Track concurrent operations
            let current = current_c.fetch_add(1, Ordering::SeqCst) + 1;

            // Update max
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

            let mut playbook = Playbook::new(format!("Task-{}", i));
            let play = Play::new("Work", "all");
            playbook.add_play(play);

            let _ = executor.run_playbook(&playbook).await;

            current_c.fetch_sub(1, Ordering::SeqCst);
            counter.fetch_add(1, Ordering::SeqCst);
        });
        handles.push(handle);
    }

    let result = tokio::time::timeout(Duration::from_secs(120), join_all(handles)).await;

    assert!(result.is_ok(), "Semaphore test timed out");

    let completed = operations_completed.load(Ordering::SeqCst);
    let max = max_concurrent.load(Ordering::SeqCst);

    println!(
        "Semaphore limiting: {}/200 completed, max concurrent: {}",
        completed, max
    );

    assert_eq!(completed, 200, "Not all operations completed");
    assert!(max <= 10, "Max concurrent exceeded limit: {}", max);
}

/// Test concurrent executor creation and destruction
#[tokio::test]
async fn test_concurrent_executor_lifecycle() {
    let metrics = Arc::new(MetricsCollector::new());
    let mut handles = vec![];

    // Spawn 100 concurrent executor lifecycles
    for i in 0..100 {
        let metrics = Arc::clone(&metrics);
        let handle = tokio::spawn(async move {
            let start = Instant::now();

            let runtime = create_large_runtime(5);
            let executor = Executor::with_runtime(
                ExecutorConfig {
                    forks: 5,
                    ..Default::default()
                },
                runtime,
            );

            let mut playbook = Playbook::new(format!("Lifecycle-{}", i));
            let mut play = Play::new("Test", "all");
            play.gather_facts = false;
            play.add_task(Task::new("Task", "debug").arg("msg", "lifecycle test"));
            playbook.add_play(play);

            match executor.run_playbook(&playbook).await {
                Ok(_) => {
                    let latency = start.elapsed().as_nanos() as u64;
                    metrics.record_operation(latency);
                }
                Err(_) => {
                    metrics.record_error();
                }
            }
        });
        handles.push(handle);
    }

    join_all(handles).await;

    println!(
        "Executor lifecycle: {} operations, {} errors",
        metrics.operations(),
        metrics.errors()
    );
    println!(
        "  Avg latency: {:.2}ms, Max: {:.2}ms",
        metrics.avg_latency_ms(),
        metrics.max_latency_ms()
    );

    assert!(
        metrics.error_rate() < 0.05,
        "Error rate too high: {:.2}%",
        metrics.error_rate() * 100.0
    );
}

/// Test concurrent variable access with readers and writers
#[tokio::test]
async fn test_concurrent_variable_access() {
    let runtime = Arc::new(RwLock::new(RuntimeContext::new()));

    // Initialize hosts
    {
        let mut rt = runtime.write().await;
        for i in 0..50 {
            rt.add_host(format!("host-{}", i), None);
        }
    }

    let mut handles = vec![];

    // 100 concurrent readers
    for i in 0..100 {
        let rt = Arc::clone(&runtime);
        handles.push(tokio::spawn(async move {
            for _ in 0..10 {
                let ctx = rt.read().await;
                let _ = ctx.get_var("some_var", None);
                let _ = ctx.get_merged_vars(&format!("host-{}", i % 50));
            }
        }));
    }

    // 20 concurrent writers
    for i in 0..20 {
        let rt = Arc::clone(&runtime);
        handles.push(tokio::spawn(async move {
            for j in 0..5 {
                let mut ctx = rt.write().await;
                ctx.set_global_var(format!("var_{}_{}", i, j), serde_json::json!(i * j));
            }
        }));
    }

    let results = join_all(handles).await;
    let success_count = results.iter().filter(|r| r.is_ok()).count();

    println!(
        "Concurrent variable access: {}/120 operations succeeded",
        success_count
    );

    assert_eq!(success_count, 120, "Some operations failed");
}

// ============================================================================
// 4. MEMORY UNDER SUSTAINED LOAD TESTS
// ============================================================================

/// Test memory stability over 200 iterations
#[tokio::test]
async fn test_memory_sustained_load_200_iterations() {
    let iterations = 200;
    let mut iteration_times = Vec::with_capacity(iterations);
    let mut failures = 0;

    for i in 0..iterations {
        let start = Instant::now();

        let runtime = create_large_runtime(50);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                forks: 25,
                ..Default::default()
            },
            runtime,
        );

        let mut playbook = Playbook::new(format!("Memory-Test-{}", i));
        let mut play = Play::new("Test", "all");
        play.gather_facts = false;
        play.add_task(Task::new("Task", "debug").arg("msg", "memory test"));
        playbook.add_play(play);

        match executor.run_playbook(&playbook).await {
            Ok(_) => {}
            Err(_) => failures += 1,
        }

        iteration_times.push(start.elapsed());

        // Drop resources explicitly
        drop(playbook);
        drop(executor);

        // Progress indicator
        if (i + 1) % 50 == 0 {
            println!("Completed {}/{} iterations", i + 1, iterations);
        }
    }

    // Analyze for memory leak patterns
    let first_50_avg: Duration = iteration_times[..50].iter().sum::<Duration>() / 50;
    let last_50_avg: Duration = iteration_times[iterations - 50..].iter().sum::<Duration>() / 50;

    println!("Memory sustained load test results:");
    println!("  First 50 avg: {:?}", first_50_avg);
    println!("  Last 50 avg: {:?}", last_50_avg);
    println!("  Failures: {}", failures);

    // Performance should be stable (within 3x)
    let ratio = last_50_avg.as_nanos() as f64 / first_50_avg.as_nanos() as f64;
    assert!(
        ratio < 3.0,
        "Possible memory leak: performance degraded {:.2}x",
        ratio
    );
}

/// Test memory with large variable contexts
#[tokio::test]
async fn test_memory_large_variable_contexts() {
    // 20 hosts with 100KB variables each = ~2MB total
    let runtime = create_runtime_with_large_vars(20, 100);

    let executor = Executor::with_runtime(
        ExecutorConfig {
            strategy: ExecutionStrategy::Free,
            forks: 10,
            ..Default::default()
        },
        runtime,
    );

    let mut playbook = Playbook::new("Large Vars Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;
    play.add_task(
        Task::new("Access large var", "debug")
            .arg("msg", "{{ large_var[:50] }}"),
    );
    playbook.add_play(play);

    let start = Instant::now();
    let results = executor.run_playbook(&playbook).await.unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 20);
    println!(
        "Large variable context (2MB+) processed in {:?}",
        duration
    );
}

/// Test memory leak detection over 300 iterations
#[tokio::test]
async fn test_memory_leak_detection_extended() {
    let iterations = 300;
    let mut iteration_times: Vec<Duration> = Vec::with_capacity(iterations);

    for i in 0..iterations {
        let start = Instant::now();

        let runtime = create_large_runtime(30);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                forks: 15,
                ..Default::default()
            },
            runtime,
        );

        let playbook = create_large_playbook(10);
        let _ = executor.run_playbook(&playbook).await;

        iteration_times.push(start.elapsed());

        // Progress indicator
        if (i + 1) % 100 == 0 {
            let recent_avg: Duration = iteration_times[i.saturating_sub(9)..=i]
                .iter()
                .sum::<Duration>()
                / 10;
            println!(
                "Iteration {}/{}: recent avg {:?}",
                i + 1,
                iterations,
                recent_avg
            );
        }
    }

    // Compare first, middle, and last segments
    let first_avg: Duration = iteration_times[..50].iter().sum::<Duration>() / 50;
    let mid_avg: Duration = iteration_times[125..175].iter().sum::<Duration>() / 50;
    let last_avg: Duration = iteration_times[iterations - 50..].iter().sum::<Duration>() / 50;

    println!("Memory leak detection ({} iterations):", iterations);
    println!("  First 50 avg: {:?}", first_avg);
    println!("  Middle 50 avg: {:?}", mid_avg);
    println!("  Last 50 avg: {:?}", last_avg);

    // Check for consistent performance (no degradation)
    let ratio = last_avg.as_nanos() as f64 / first_avg.as_nanos() as f64;
    assert!(
        ratio < 2.5,
        "Performance degraded over time: {:.2}x (possible memory leak)",
        ratio
    );
}

/// Test sustained parallel workload
#[tokio::test]
async fn test_memory_sustained_parallel_workload() {
    let metrics = Arc::new(MetricsCollector::new());
    let target_duration = Duration::from_secs(30);
    let start = Instant::now();
    let mut batch_count = 0;

    while start.elapsed() < target_duration {
        batch_count += 1;
        let batch_start = Instant::now();

        let runtime = create_large_runtime(20);
        let executor = Executor::with_runtime(
            ExecutorConfig {
                strategy: ExecutionStrategy::Free,
                forks: 20,
                ..Default::default()
            },
            runtime,
        );

        let playbook = create_large_playbook(5);

        match executor.run_playbook(&playbook).await {
            Ok(_) => {
                metrics.record_operation(batch_start.elapsed().as_nanos() as u64);
            }
            Err(_) => {
                metrics.record_error();
            }
        }
    }

    println!(
        "Sustained parallel workload: {} batches over {:?}",
        batch_count,
        start.elapsed()
    );
    println!(
        "  Avg latency: {:.2}ms, Max: {:.2}ms",
        metrics.avg_latency_ms(),
        metrics.max_latency_ms()
    );
    println!("  Error rate: {:.2}%", metrics.error_rate() * 100.0);

    assert!(
        metrics.error_rate() < 0.01,
        "Error rate too high: {:.2}%",
        metrics.error_rate() * 100.0
    );
}

// ============================================================================
// 5. CONNECTION POOL UNDER PRESSURE TESTS
// ============================================================================

/// Test local connection rapid cycling
#[tokio::test]
async fn test_connection_pool_rapid_cycling() {
    use rustible::connection::local::LocalConnection;
    use rustible::connection::Connection;

    let iterations = 500;
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
        "{} connect/disconnect cycles: avg {:?}, max {:?}",
        iterations, avg, max
    );

    assert!(
        avg < Duration::from_millis(10),
        "Average connection time too high: {:?}",
        avg
    );
}

/// Test connection pool concurrent access
#[tokio::test]
async fn test_connection_pool_concurrent_access() {
    use rustible::connection::{ConnectionConfig, ConnectionFactory};

    let config = ConnectionConfig::default();
    let factory = ConnectionFactory::with_pool_size(config, 20);

    let success = Arc::new(AtomicUsize::new(0));
    let failure = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    // 100 concurrent connection requests
    for i in 0..100 {
        let factory_clone = factory.clone();
        let success_clone = Arc::clone(&success);
        let failure_clone = Arc::clone(&failure);

        let handle = tokio::spawn(async move {
            match factory_clone.get_connection("localhost").await {
                Ok(conn) => {
                    if conn.is_alive().await {
                        success_clone.fetch_add(1, Ordering::SeqCst);
                    } else {
                        failure_clone.fetch_add(1, Ordering::SeqCst);
                    }
                }
                Err(_) => {
                    failure_clone.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }

    join_all(handles).await;

    let total_success = success.load(Ordering::SeqCst);
    let total_failure = failure.load(Ordering::SeqCst);

    println!(
        "Connection pool concurrent access: {} success, {} failures",
        total_success, total_failure
    );

    // Most should succeed
    assert!(
        total_success > 90,
        "Too many failures: {}",
        total_failure
    );
}

/// Test connection pool exhaustion and recovery
#[tokio::test]
async fn test_connection_pool_exhaustion_recovery() {
    use rustible::connection::{ConnectionConfig, ConnectionFactory};

    let config = ConnectionConfig::default();
    let factory = ConnectionFactory::with_pool_size(config, 5);

    let mut handles = vec![];
    let acquired = Arc::new(AtomicUsize::new(0));
    let released = Arc::new(AtomicUsize::new(0));

    // 50 concurrent attempts on a pool of 5
    for i in 0..50 {
        let factory_clone = factory.clone();
        let acq = Arc::clone(&acquired);
        let rel = Arc::clone(&released);

        let handle = tokio::spawn(async move {
            // Stagger connection attempts
            tokio::time::sleep(Duration::from_millis(i as u64 * 10)).await;

            if let Ok(conn) = factory_clone.get_connection("localhost").await {
                acq.fetch_add(1, Ordering::SeqCst);

                // Hold connection briefly
                tokio::time::sleep(Duration::from_millis(50)).await;
                let _ = conn.is_alive().await;

                rel.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    let result = tokio::time::timeout(Duration::from_secs(30), join_all(handles)).await;

    assert!(result.is_ok(), "Pool exhaustion test timed out");

    let stats = factory.pool_stats();
    println!(
        "Pool exhaustion recovery: acquired={}, released={}, pool_stats={:?}",
        acquired.load(Ordering::SeqCst),
        released.load(Ordering::SeqCst),
        stats
    );
}

/// Test connection pool with mixed workloads
#[tokio::test]
async fn test_connection_pool_mixed_workload() {
    use rustible::connection::local::LocalConnection;
    use rustible::connection::Connection;

    let metrics = Arc::new(MetricsCollector::new());
    let mut handles = vec![];

    // Short-lived connections
    for i in 0..50 {
        let metrics = Arc::clone(&metrics);
        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let conn = LocalConnection::new();
            let _ = conn.execute("echo short", None).await;
            let _ = conn.close().await;
            metrics.record_operation(start.elapsed().as_nanos() as u64);
        }));
    }

    // Medium-lived connections
    for i in 0..30 {
        let metrics = Arc::clone(&metrics);
        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let conn = LocalConnection::new();
            for j in 0..5 {
                let _ = conn.execute(&format!("echo medium-{}", j), None).await;
            }
            let _ = conn.close().await;
            metrics.record_operation(start.elapsed().as_nanos() as u64);
        }));
    }

    // Long-lived connections
    for i in 0..20 {
        let metrics = Arc::clone(&metrics);
        handles.push(tokio::spawn(async move {
            let start = Instant::now();
            let conn = LocalConnection::new();
            for j in 0..10 {
                let _ = conn.execute(&format!("echo long-{}", j), None).await;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let _ = conn.close().await;
            metrics.record_operation(start.elapsed().as_nanos() as u64);
        }));
    }

    join_all(handles).await;

    println!(
        "Mixed workload: {} operations, {} errors",
        metrics.operations(),
        metrics.errors()
    );
    println!(
        "  Min: {:.2}ms, Avg: {:.2}ms, Max: {:.2}ms",
        metrics.min_latency_ms(),
        metrics.avg_latency_ms(),
        metrics.max_latency_ms()
    );

    assert_eq!(metrics.operations(), 100);
    assert_eq!(metrics.errors(), 0);
}

// ============================================================================
// EXTREME STRESS TESTS (run manually with --ignored)
// ============================================================================

/// Extreme test: 2000 host inventory with full operations
#[test]
#[ignore = "Extreme stress test - run with: cargo test extreme_inventory_2000_hosts -- --ignored"]
fn extreme_inventory_2000_hosts() {
    let start = Instant::now();
    let inventory = create_large_inventory(2000, 50);
    let creation_time = start.elapsed();

    assert_eq!(inventory.host_count(), 2000);
    println!("Created 2000-host inventory in {:?}", creation_time);

    // Pattern matching at scale
    let all_hosts = inventory.get_hosts_for_pattern("all").unwrap();
    assert_eq!(all_hosts.len(), 2000);

    // Variable resolution at scale
    let var_start = Instant::now();
    for i in 0..100 {
        let host_name = format!("host{:05}", i * 20);
        if let Some(host) = inventory.get_host(&host_name) {
            let _ = inventory.get_host_vars(host);
        }
    }
    let var_time = var_start.elapsed();
    println!("Variable resolution for 100 hosts: {:?}", var_time);
}

/// Extreme test: 1000 tasks across 50 hosts
#[tokio::test]
#[ignore = "Extreme stress test - run with: cargo test extreme_playbook_1000_tasks_50_hosts -- --ignored"]
async fn extreme_playbook_1000_tasks_50_hosts() {
    let test_result = tokio::time::timeout(Duration::from_secs(600), async {
        let playbook = create_large_playbook(1000);
        let runtime = create_large_runtime(50);

        let executor = Executor::with_runtime(
            ExecutorConfig {
                strategy: ExecutionStrategy::Free,
                forks: 25,
                ..Default::default()
            },
            runtime,
        );

        let start = Instant::now();
        let results = executor.run_playbook(&playbook).await;
        let duration = start.elapsed();

        match results {
            Ok(results) => {
                let total_tasks: usize = results
                    .values()
                    .map(|r| r.stats.ok + r.stats.changed)
                    .sum();
                let throughput = total_tasks as f64 / duration.as_secs_f64();

                println!(
                    "1000 tasks x 50 hosts: {} task executions in {:?} ({:.1} tasks/sec)",
                    total_tasks, duration, throughput
                );

                assert!(results.len() == 50);
            }
            Err(e) => {
                panic!("Extreme playbook test failed: {:?}", e);
            }
        }
    })
    .await;

    assert!(test_result.is_ok(), "Extreme playbook test timed out");
}

/// Extreme test: Memory sustained load for 500 iterations
#[tokio::test]
#[ignore = "Extreme stress test - run with: cargo test extreme_memory_500_iterations -- --ignored"]
async fn extreme_memory_500_iterations() {
    let test_result = tokio::time::timeout(Duration::from_secs(900), async {
        let iterations = 500;
        let mut iteration_times: Vec<Duration> = Vec::with_capacity(iterations);
        let mut failures = 0;

        for i in 0..iterations {
            let start = Instant::now();

            let runtime = create_large_runtime(100);
            let executor = Executor::with_runtime(
                ExecutorConfig {
                    forks: 50,
                    ..Default::default()
                },
                runtime,
            );

            let playbook = create_large_playbook(20);

            match executor.run_playbook(&playbook).await {
                Ok(_) => {}
                Err(_) => failures += 1,
            }

            iteration_times.push(start.elapsed());

            if (i + 1) % 100 == 0 {
                let recent_avg: Duration = iteration_times[i.saturating_sub(9)..=i]
                    .iter()
                    .sum::<Duration>()
                    / 10;
                println!(
                    "Iteration {}/{}: recent avg {:?}, failures: {}",
                    i + 1,
                    iterations,
                    recent_avg,
                    failures
                );
            }
        }

        let first_avg: Duration = iteration_times[..50].iter().sum::<Duration>() / 50;
        let last_avg: Duration = iteration_times[iterations - 50..].iter().sum::<Duration>() / 50;
        let ratio = last_avg.as_nanos() as f64 / first_avg.as_nanos() as f64;

        println!("Extreme memory test ({} iterations):", iterations);
        println!("  First 50 avg: {:?}", first_avg);
        println!("  Last 50 avg: {:?}", last_avg);
        println!("  Ratio: {:.2}x", ratio);
        println!("  Failures: {}", failures);

        assert!(ratio < 3.0, "Performance degraded: {:.2}x", ratio);
        assert!(failures < iterations / 50, "Too many failures: {}", failures);
    })
    .await;

    assert!(test_result.is_ok(), "Extreme memory test timed out");
}

/// Extreme test: Combined stress with all factors
#[tokio::test]
#[ignore = "Extreme stress test - run with: cargo test extreme_combined_stress -- --ignored"]
async fn extreme_combined_stress() {
    let test_result = tokio::time::timeout(Duration::from_secs(600), async {
        let metrics = Arc::new(MetricsCollector::new());

        println!("Starting extreme combined stress test...");

        // Phase 1: Large inventory + Large playbook
        println!("\nPhase 1: Large inventory (500 hosts) + Large playbook (200 tasks)");
        let phase1_start = Instant::now();
        {
            let runtime = create_large_runtime(500);
            let playbook = create_large_playbook(200);
            let executor = Executor::with_runtime(
                ExecutorConfig {
                    strategy: ExecutionStrategy::Free,
                    forks: 100,
                    ..Default::default()
                },
                runtime,
            );

            match executor.run_playbook(&playbook).await {
                Ok(_) => metrics.record_operation(phase1_start.elapsed().as_nanos() as u64),
                Err(_) => metrics.record_error(),
            }
        }
        println!("  Phase 1 completed in {:?}", phase1_start.elapsed());

        // Phase 2: Many concurrent executors
        println!("\nPhase 2: 50 concurrent executors");
        let phase2_start = Instant::now();
        {
            let mut handles = vec![];
            for i in 0..50 {
                let m = Arc::clone(&metrics);
                handles.push(tokio::spawn(async move {
                    let op_start = Instant::now();
                    let runtime = create_large_runtime(20);
                    let executor = Executor::with_runtime(
                        ExecutorConfig {
                            forks: 10,
                            ..Default::default()
                        },
                        runtime,
                    );

                    let playbook = create_large_playbook(10);
                    match executor.run_playbook(&playbook).await {
                        Ok(_) => m.record_operation(op_start.elapsed().as_nanos() as u64),
                        Err(_) => m.record_error(),
                    }
                }));
            }
            join_all(handles).await;
        }
        println!("  Phase 2 completed in {:?}", phase2_start.elapsed());

        // Phase 3: Large variable contexts
        println!("\nPhase 3: Large variable contexts (50 hosts x 200KB each)");
        let phase3_start = Instant::now();
        {
            let runtime = create_runtime_with_large_vars(50, 200);
            let executor = Executor::with_runtime(
                ExecutorConfig {
                    strategy: ExecutionStrategy::Free,
                    forks: 25,
                    ..Default::default()
                },
                runtime,
            );

            let playbook = create_large_playbook(50);
            match executor.run_playbook(&playbook).await {
                Ok(_) => metrics.record_operation(phase3_start.elapsed().as_nanos() as u64),
                Err(_) => metrics.record_error(),
            }
        }
        println!("  Phase 3 completed in {:?}", phase3_start.elapsed());

        println!("\nCombined stress test summary:");
        println!("  Total operations: {}", metrics.operations());
        println!("  Total errors: {}", metrics.errors());
        println!("  Avg latency: {:.2}ms", metrics.avg_latency_ms());
        println!("  Max latency: {:.2}ms", metrics.max_latency_ms());
        println!("  Error rate: {:.2}%", metrics.error_rate() * 100.0);

        assert!(
            metrics.error_rate() < 0.05,
            "Error rate too high: {:.2}%",
            metrics.error_rate() * 100.0
        );
    })
    .await;

    assert!(test_result.is_ok(), "Extreme combined stress test timed out");
}

// ============================================================================
// THROUGHPUT BENCHMARKS
// ============================================================================

/// Measure task throughput across hosts
#[tokio::test]
async fn benchmark_task_throughput() {
    let mut results = Vec::new();

    for host_count in [10, 25, 50, 100] {
        let runtime = create_large_runtime(host_count);
        let playbook = create_large_playbook(50);

        let executor = Executor::with_runtime(
            ExecutorConfig {
                strategy: ExecutionStrategy::Free,
                forks: host_count.min(50),
                ..Default::default()
            },
            runtime,
        );

        let start = Instant::now();
        let exec_results = executor.run_playbook(&playbook).await.unwrap();
        let duration = start.elapsed();

        let total_tasks: usize = exec_results
            .values()
            .map(|r| r.stats.ok + r.stats.changed)
            .sum();
        let throughput = total_tasks as f64 / duration.as_secs_f64();

        results.push((host_count, total_tasks, duration, throughput));
    }

    println!("\nTask Throughput Benchmark:");
    println!("┌──────────┬────────────┬──────────────┬──────────────────┐");
    println!("│  Hosts   │  Tasks     │   Duration   │  Throughput      │");
    println!("├──────────┼────────────┼──────────────┼──────────────────┤");
    for (hosts, tasks, duration, throughput) in results {
        println!(
            "│ {:>8} │ {:>10} │ {:>10.2?} │ {:>12.1} t/s │",
            hosts, tasks, duration, throughput
        );
    }
    println!("└──────────┴────────────┴──────────────┴──────────────────┘");
}

/// Measure inventory operations at scale
#[test]
fn benchmark_inventory_operations() {
    let mut results = Vec::new();

    for host_count in [100, 500, 1000, 2000] {
        // Creation
        let create_start = Instant::now();
        let inventory = create_large_inventory(host_count, host_count / 50);
        let create_time = create_start.elapsed();

        // Pattern matching
        let pattern_start = Instant::now();
        let _ = inventory.get_hosts_for_pattern("all").unwrap();
        let pattern_time = pattern_start.elapsed();

        // Variable resolution (sample)
        let var_start = Instant::now();
        for i in (0..host_count).step_by(host_count / 10) {
            let host_name = format!("host{:05}", i);
            if let Some(host) = inventory.get_host(&host_name) {
                let _ = inventory.get_host_vars(host);
            }
        }
        let var_time = var_start.elapsed();

        results.push((host_count, create_time, pattern_time, var_time));
    }

    println!("\nInventory Operations Benchmark:");
    println!("┌──────────┬──────────────┬──────────────┬──────────────┐");
    println!("│  Hosts   │  Creation    │  Pattern     │  Variables   │");
    println!("├──────────┼──────────────┼──────────────┼──────────────┤");
    for (hosts, create, pattern, vars) in results {
        println!(
            "│ {:>8} │ {:>10.2?} │ {:>10.2?} │ {:>10.2?} │",
            hosts, create, pattern, vars
        );
    }
    println!("└──────────┴──────────────┴──────────────┴──────────────┘");
}

// ============================================================================
// LATENCY PERCENTILE TESTS
// ============================================================================

/// Measure latency percentiles for task execution
#[tokio::test]
async fn test_latency_percentiles() {
    let mut latencies: Vec<Duration> = Vec::with_capacity(500);

    for _ in 0..500 {
        let runtime = create_large_runtime(1);
        let executor = Executor::with_runtime(ExecutorConfig::default(), runtime);

        let mut playbook = Playbook::new("Latency Test");
        let mut play = Play::new("Test", "all");
        play.gather_facts = false;
        play.add_task(Task::new("Quick", "debug").arg("msg", "test"));
        playbook.add_play(play);

        let start = Instant::now();
        let _ = executor.run_playbook(&playbook).await;
        latencies.push(start.elapsed());
    }

    latencies.sort();

    let p50 = latencies[250];
    let p90 = latencies[450];
    let p95 = latencies[475];
    let p99 = latencies[495];
    let p999 = latencies[499];

    println!("\nTask Execution Latency Percentiles (500 samples):");
    println!("  p50:   {:?}", p50);
    println!("  p90:   {:?}", p90);
    println!("  p95:   {:?}", p95);
    println!("  p99:   {:?}", p99);
    println!("  p99.9: {:?}", p999);

    // p99 should be reasonable for local execution
    assert!(
        p99 < Duration::from_millis(200),
        "p99 latency too high: {:?}",
        p99
    );
}
