//! Async tests for the Rustible callback system.
//!
//! This test suite verifies async behavior of callbacks:
//! 1. Callbacks run without blocking the executor
//! 2. Multiple concurrent callbacks
//! 3. Slow callbacks don't slow execution
//! 4. Proper timeout handling
//! 5. Cancellation behavior

use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustible::facts::Facts;
use rustible::traits::{ExecutionCallback, ExecutionResult, ModuleResult};

// ============================================================================
// Test Infrastructure - Async Callback Implementations
// ============================================================================

/// A callback that tracks timing information to verify non-blocking behavior.
#[derive(Debug, Default)]
pub struct TimingCallback {
    /// Start time of each callback invocation
    pub invocation_times: RwLock<Vec<Instant>>,
    /// End time of each callback invocation
    pub completion_times: RwLock<Vec<Instant>>,
    /// Total time spent in callbacks
    pub total_callback_time_ms: AtomicU64,
    /// Number of callback invocations
    pub invocation_count: AtomicU32,
    /// Whether callbacks are running concurrently
    pub concurrent_execution_detected: AtomicBool,
    /// Current number of callbacks executing
    pub current_executing: AtomicU32,
    /// Maximum concurrent callbacks observed
    pub max_concurrent: AtomicU32,
}

impl TimingCallback {
    pub fn new() -> Self {
        Self::default()
    }

    fn record_start(&self) {
        self.invocation_times.write().push(Instant::now());
        let current = self.current_executing.fetch_add(1, Ordering::SeqCst) + 1;

        // Update max concurrent if this is a new high
        let mut max = self.max_concurrent.load(Ordering::SeqCst);
        while current > max {
            match self.max_concurrent.compare_exchange_weak(
                max,
                current,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(actual) => max = actual,
            }
        }

        if current > 1 {
            self.concurrent_execution_detected
                .store(true, Ordering::SeqCst);
        }
    }

    fn record_end(&self, start: Instant) {
        let elapsed = start.elapsed();
        self.completion_times.write().push(Instant::now());
        self.total_callback_time_ms
            .fetch_add(elapsed.as_millis() as u64, Ordering::SeqCst);
        self.current_executing.fetch_sub(1, Ordering::SeqCst);
        self.invocation_count.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl ExecutionCallback for TimingCallback {
    async fn on_playbook_start(&self, _name: &str) {
        let start = Instant::now();
        self.record_start();
        self.record_end(start);
    }

    async fn on_playbook_end(&self, _name: &str, _success: bool) {
        let start = Instant::now();
        self.record_start();
        self.record_end(start);
    }

    async fn on_play_start(&self, _name: &str, _hosts: &[String]) {
        let start = Instant::now();
        self.record_start();
        self.record_end(start);
    }

    async fn on_play_end(&self, _name: &str, _success: bool) {
        let start = Instant::now();
        self.record_start();
        self.record_end(start);
    }

    async fn on_task_start(&self, _name: &str, _host: &str) {
        let start = Instant::now();
        self.record_start();
        self.record_end(start);
    }

    async fn on_task_complete(&self, _result: &ExecutionResult) {
        let start = Instant::now();
        self.record_start();
        self.record_end(start);
    }

    async fn on_handler_triggered(&self, _name: &str) {
        let start = Instant::now();
        self.record_start();
        self.record_end(start);
    }

    async fn on_facts_gathered(&self, _host: &str, _facts: &Facts) {
        let start = Instant::now();
        self.record_start();
        self.record_end(start);
    }
}

/// A slow callback that simulates expensive callback operations.
#[derive(Debug)]
pub struct SlowCallback {
    /// Delay in milliseconds for each callback
    pub delay_ms: u64,
    /// Track invocation counts
    pub invocation_count: AtomicU32,
    /// Track total execution time
    pub total_time_ms: AtomicU64,
    /// Flag to track if callbacks complete
    pub callbacks_completed: AtomicBool,
}

impl SlowCallback {
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            invocation_count: AtomicU32::new(0),
            total_time_ms: AtomicU64::new(0),
            callbacks_completed: AtomicBool::new(false),
        }
    }

    async fn slow_operation(&self) {
        let start = Instant::now();
        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        self.total_time_ms
            .fetch_add(start.elapsed().as_millis() as u64, Ordering::SeqCst);
        self.invocation_count.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl ExecutionCallback for SlowCallback {
    async fn on_playbook_start(&self, _name: &str) {
        self.slow_operation().await;
    }

    async fn on_playbook_end(&self, _name: &str, _success: bool) {
        self.slow_operation().await;
        self.callbacks_completed.store(true, Ordering::SeqCst);
    }

    async fn on_play_start(&self, _name: &str, _hosts: &[String]) {
        self.slow_operation().await;
    }

    async fn on_play_end(&self, _name: &str, _success: bool) {
        self.slow_operation().await;
    }

    async fn on_task_start(&self, _name: &str, _host: &str) {
        self.slow_operation().await;
    }

    async fn on_task_complete(&self, _result: &ExecutionResult) {
        self.slow_operation().await;
    }

    async fn on_handler_triggered(&self, _name: &str) {
        self.slow_operation().await;
    }

    async fn on_facts_gathered(&self, _host: &str, _facts: &Facts) {
        self.slow_operation().await;
    }
}

/// A callback that can be cancelled via a flag.
#[derive(Debug)]
pub struct CancellableCallback {
    /// Cancellation flag
    pub cancelled: AtomicBool,
    /// Count of callbacks started
    pub started_count: AtomicU32,
    /// Count of callbacks completed (not cancelled)
    pub completed_count: AtomicU32,
    /// Count of callbacks that were cancelled
    pub cancelled_count: AtomicU32,
}

impl CancellableCallback {
    pub fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            started_count: AtomicU32::new(0),
            completed_count: AtomicU32::new(0),
            cancelled_count: AtomicU32::new(0),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    async fn check_cancelled(&self) {
        self.started_count.fetch_add(1, Ordering::SeqCst);

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(10)).await;

        if self.cancelled.load(Ordering::SeqCst) {
            self.cancelled_count.fetch_add(1, Ordering::SeqCst);
        } else {
            self.completed_count.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl Default for CancellableCallback {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionCallback for CancellableCallback {
    async fn on_playbook_start(&self, _name: &str) {
        self.check_cancelled().await;
    }

    async fn on_playbook_end(&self, _name: &str, _success: bool) {
        self.check_cancelled().await;
    }

    async fn on_task_start(&self, _name: &str, _host: &str) {
        self.check_cancelled().await;
    }

    async fn on_task_complete(&self, _result: &ExecutionResult) {
        self.check_cancelled().await;
    }
}

/// A callback with timeout detection.
#[derive(Debug)]
pub struct TimeoutAwareCallback {
    /// Timeout duration for callbacks
    pub timeout_ms: u64,
    /// Operation duration (may exceed timeout)
    pub operation_ms: u64,
    /// Count of callbacks that timed out
    pub timeout_count: AtomicU32,
    /// Count of callbacks that completed in time
    pub success_count: AtomicU32,
    /// Track which operations timed out
    pub timed_out_operations: RwLock<Vec<String>>,
}

impl TimeoutAwareCallback {
    pub fn new(timeout_ms: u64, operation_ms: u64) -> Self {
        Self {
            timeout_ms,
            operation_ms,
            timeout_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            timed_out_operations: RwLock::new(Vec::new()),
        }
    }

    async fn timed_operation(&self, operation_name: &str) {
        let result = tokio::time::timeout(
            Duration::from_millis(self.timeout_ms),
            tokio::time::sleep(Duration::from_millis(self.operation_ms)),
        )
        .await;

        match result {
            Ok(_) => {
                self.success_count.fetch_add(1, Ordering::SeqCst);
            }
            Err(_) => {
                self.timeout_count.fetch_add(1, Ordering::SeqCst);
                self.timed_out_operations
                    .write()
                    .push(operation_name.to_string());
            }
        }
    }
}

#[async_trait]
impl ExecutionCallback for TimeoutAwareCallback {
    async fn on_playbook_start(&self, name: &str) {
        self.timed_operation(&format!("playbook_start:{}", name))
            .await;
    }

    async fn on_playbook_end(&self, name: &str, _success: bool) {
        self.timed_operation(&format!("playbook_end:{}", name))
            .await;
    }

    async fn on_task_start(&self, name: &str, host: &str) {
        self.timed_operation(&format!("task_start:{}:{}", name, host))
            .await;
    }

    async fn on_task_complete(&self, result: &ExecutionResult) {
        self.timed_operation(&format!(
            "task_complete:{}:{}",
            result.task_name, result.host
        ))
        .await;
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_execution_result(host: &str, task_name: &str, success: bool) -> ExecutionResult {
    ExecutionResult {
        host: host.to_string(),
        task_name: task_name.to_string(),
        result: if success {
            ModuleResult::ok("Success")
        } else {
            ModuleResult::failed("Failed")
        },
        duration: Duration::from_millis(100),
        notify: Vec::new(),
    }
}

// ============================================================================
// Test 1: Callbacks Run Without Blocking Executor
// ============================================================================

#[tokio::test]
async fn test_callback_does_not_block_executor() {
    let callback = Arc::new(TimingCallback::new());

    // Spawn multiple concurrent operations that invoke callbacks
    let start = Instant::now();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                cb.on_task_start(&format!("task_{}", i), "host1").await;
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();

    // All callbacks should complete quickly (under 100ms for simple operations)
    assert!(
        elapsed.as_millis() < 100,
        "Callbacks took too long: {:?}ms",
        elapsed.as_millis()
    );

    // All invocations should have been recorded
    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 10);
}

#[tokio::test]
async fn test_fast_callbacks_complete_quickly() {
    let callback = TimingCallback::new();

    let start = Instant::now();

    // Run a sequence of callbacks
    callback.on_playbook_start("test_playbook").await;
    for i in 0..100 {
        callback
            .on_task_start(&format!("task_{}", i), "localhost")
            .await;
        let result = create_execution_result("localhost", &format!("task_{}", i), true);
        callback.on_task_complete(&result).await;
    }
    callback.on_playbook_end("test_playbook", true).await;

    let elapsed = start.elapsed();

    // 202 callbacks (1 start + 100*2 task start/complete + 1 end) should complete quickly
    assert!(
        elapsed.as_millis() < 200,
        "Fast callbacks should complete in under 200ms, took {:?}ms",
        elapsed.as_millis()
    );

    // Verify all callbacks were invoked
    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 202);
}

#[tokio::test]
async fn test_executor_continues_during_callback() {
    // Verify that the tokio executor can handle other work while callbacks run
    let callback = Arc::new(SlowCallback::new(50));
    let work_completed = Arc::new(AtomicBool::new(false));

    let work_flag = Arc::clone(&work_completed);
    let cb = Arc::clone(&callback);

    // Start a slow callback
    let callback_handle = tokio::spawn(async move {
        cb.on_playbook_start("slow_playbook").await;
    });

    // Do other work while callback is running
    let work_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        work_flag.store(true, Ordering::SeqCst);
    });

    // Wait for both to complete
    let (callback_result, work_result) = tokio::join!(callback_handle, work_handle);

    callback_result.unwrap();
    work_result.unwrap();

    // Both should complete successfully
    assert!(work_completed.load(Ordering::SeqCst));
    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 1);
}

// ============================================================================
// Test 2: Multiple Concurrent Callbacks
// ============================================================================

#[tokio::test]
async fn test_concurrent_callbacks_all_complete() {
    let callback = Arc::new(TimingCallback::new());

    // Spawn many concurrent callback invocations
    let handles: Vec<_> = (0..50)
        .map(|i| {
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                cb.on_task_start(&format!("task_{}", i), &format!("host_{}", i % 5))
                    .await;
            })
        })
        .collect();

    // Wait for all handles
    for handle in handles {
        handle.await.unwrap();
    }

    // All callbacks should have been invoked
    assert_eq!(
        callback.invocation_count.load(Ordering::SeqCst),
        50,
        "Not all concurrent callbacks completed"
    );
}

#[tokio::test]
async fn test_concurrent_callbacks_on_different_events() {
    let callback = Arc::new(TimingCallback::new());

    let hosts = vec!["host1".to_string(), "host2".to_string()];

    // Spawn different types of callbacks concurrently
    let cb1 = Arc::clone(&callback);
    let cb2 = Arc::clone(&callback);
    let cb3 = Arc::clone(&callback);
    let hosts_clone = hosts.clone();

    let handles = vec![
        tokio::spawn(async move {
            cb1.on_playbook_start("playbook1").await;
        }),
        tokio::spawn(async move {
            cb2.on_play_start("play1", &hosts_clone).await;
        }),
        tokio::spawn(async move {
            cb3.on_task_start("task1", "host1").await;
        }),
    ];

    for handle in handles {
        handle.await.unwrap();
    }

    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_concurrent_callbacks_with_shared_state() {
    let callback = Arc::new(TimingCallback::new());

    // Many callbacks updating shared state simultaneously
    let handles: Vec<_> = (0..100)
        .map(|i| {
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                let result = create_execution_result(
                    &format!("host_{}", i % 10),
                    &format!("task_{}", i),
                    i % 3 != 0,
                );
                cb.on_task_complete(&result).await;
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    // All 100 callbacks should complete
    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 100);
}

#[tokio::test]
async fn test_concurrent_callbacks_detect_parallelism() {
    let callback = Arc::new(SlowCallback::new(50));

    // Start multiple slow callbacks at once
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                cb.on_task_start(&format!("task_{}", i), "host1").await;
            })
        })
        .collect();

    let start = Instant::now();

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();

    // If running in parallel, 5 x 50ms callbacks should complete much faster than 250ms
    // Allow some overhead, but should definitely be less than sequential execution
    assert!(
        elapsed.as_millis() < 200,
        "Concurrent callbacks should run in parallel, took {:?}ms",
        elapsed.as_millis()
    );

    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 5);
}

// ============================================================================
// Test 3: Slow Callbacks Don't Slow Execution
// ============================================================================

#[tokio::test]
async fn test_slow_callback_doesnt_block_fast_callbacks() {
    let slow_callback = Arc::new(SlowCallback::new(100));
    let fast_callback = Arc::new(TimingCallback::new());

    let slow = Arc::clone(&slow_callback);
    let fast = Arc::clone(&fast_callback);

    // Start slow callback
    let slow_handle = tokio::spawn(async move {
        slow.on_playbook_start("slow_playbook").await;
    });

    // Fast callbacks should complete while slow one is running
    let fast_start = Instant::now();

    let fast_handles: Vec<_> = (0..10)
        .map(|i| {
            let cb = Arc::clone(&fast);
            tokio::spawn(async move {
                cb.on_task_start(&format!("task_{}", i), "host1").await;
            })
        })
        .collect();

    for handle in fast_handles {
        handle.await.unwrap();
    }

    let fast_elapsed = fast_start.elapsed();

    // Fast callbacks should complete before slow one finishes
    assert!(
        fast_elapsed.as_millis() < 50,
        "Fast callbacks should not wait for slow callback"
    );

    // Wait for slow callback to complete
    slow_handle.await.unwrap();

    assert_eq!(slow_callback.invocation_count.load(Ordering::SeqCst), 1);
    assert_eq!(fast_callback.invocation_count.load(Ordering::SeqCst), 10);
}

#[tokio::test]
async fn test_execution_continues_during_slow_callback() {
    let callback = Arc::new(SlowCallback::new(200));

    let cb = Arc::clone(&callback);
    let execution_times = Arc::new(RwLock::new(Vec::<Instant>::new()));
    let times = Arc::clone(&execution_times);

    // Start slow callback in background
    let _callback_handle = tokio::spawn(async move {
        cb.on_playbook_start("slow_playbook").await;
    });

    // Simulate execution that should continue independently
    let start = Instant::now();
    for _ in 0..5 {
        times.write().push(Instant::now());
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let execution_elapsed = start.elapsed();

    // Execution should complete in ~50ms (5 x 10ms) regardless of slow callback
    assert!(
        execution_elapsed.as_millis() < 100,
        "Execution should not be blocked by slow callback, took {:?}ms",
        execution_elapsed.as_millis()
    );

    // Verify execution actually happened
    assert_eq!(execution_times.read().len(), 5);
}

#[tokio::test]
async fn test_multiple_slow_callbacks_parallel() {
    let callbacks: Vec<_> = (0..3).map(|_| Arc::new(SlowCallback::new(100))).collect();

    let start = Instant::now();

    // Start all slow callbacks in parallel
    let handles: Vec<_> = callbacks
        .iter()
        .enumerate()
        .map(|(i, cb)| {
            let callback = Arc::clone(cb);
            tokio::spawn(async move {
                callback.on_playbook_start(&format!("playbook_{}", i)).await;
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();

    // 3 parallel 100ms callbacks should complete in ~100ms, not 300ms
    assert!(
        elapsed.as_millis() < 200,
        "Parallel slow callbacks should not accumulate time, took {:?}ms",
        elapsed.as_millis()
    );

    // All callbacks should have invoked once
    for cb in &callbacks {
        assert_eq!(cb.invocation_count.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn test_slow_callback_chain() {
    let callback = Arc::new(SlowCallback::new(30));

    let start = Instant::now();

    // Chain of slow callbacks (would be 300ms if sequential)
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                cb.on_task_start(&format!("task_{}", i), "host1").await;
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();

    // Should complete much faster than 300ms (10 x 30ms)
    assert!(
        elapsed.as_millis() < 150,
        "Parallel slow callbacks should not accumulate, took {:?}ms",
        elapsed.as_millis()
    );
}

// ============================================================================
// Test 4: Proper Timeout Handling
// ============================================================================

#[tokio::test]
async fn test_callback_timeout_triggered() {
    // Operation takes longer than timeout
    let callback = TimeoutAwareCallback::new(50, 100);

    callback.on_playbook_start("timeout_test").await;

    // Should have timed out
    assert_eq!(
        callback.timeout_count.load(Ordering::SeqCst),
        1,
        "Callback should have timed out"
    );
    assert_eq!(callback.success_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_callback_timeout_not_triggered() {
    // Operation completes before timeout
    let callback = TimeoutAwareCallback::new(100, 50);

    callback.on_playbook_start("fast_test").await;

    // Should have completed successfully
    assert_eq!(callback.success_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback.timeout_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_mixed_timeout_results() {
    // Some operations will timeout, some won't
    let fast_callback = TimeoutAwareCallback::new(100, 50);
    let slow_callback = TimeoutAwareCallback::new(50, 100);

    // Fast callback should succeed (uses implemented methods only)
    fast_callback.on_playbook_start("test").await;
    fast_callback.on_task_start("task1", "host1").await;

    // Slow callback should timeout (uses implemented methods only)
    slow_callback.on_playbook_start("test").await;
    slow_callback.on_task_start("task1", "host1").await;

    assert_eq!(fast_callback.success_count.load(Ordering::SeqCst), 2);
    assert_eq!(fast_callback.timeout_count.load(Ordering::SeqCst), 0);

    assert_eq!(slow_callback.success_count.load(Ordering::SeqCst), 0);
    assert_eq!(slow_callback.timeout_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_timeout_tracks_operation_names() {
    let callback = TimeoutAwareCallback::new(10, 50);

    callback.on_playbook_start("test_playbook").await;
    callback.on_task_start("test_task", "test_host").await;

    let timed_out = callback.timed_out_operations.read().clone();

    assert!(timed_out.contains(&"playbook_start:test_playbook".to_string()));
    assert!(timed_out.contains(&"task_start:test_task:test_host".to_string()));
}

#[tokio::test]
async fn test_timeout_with_concurrent_callbacks() {
    let callback = Arc::new(TimeoutAwareCallback::new(100, 50));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                cb.on_task_start(&format!("task_{}", i), "host1").await;
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    // All should succeed (operation 50ms < timeout 100ms)
    assert_eq!(callback.success_count.load(Ordering::SeqCst), 10);
    assert_eq!(callback.timeout_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_timeout_boundary_conditions() {
    // Test with equal timeout and operation time
    let edge_callback = TimeoutAwareCallback::new(50, 50);

    edge_callback.on_playbook_start("edge_test").await;

    // Either could happen - just verify no panic
    let total = edge_callback.success_count.load(Ordering::SeqCst)
        + edge_callback.timeout_count.load(Ordering::SeqCst);
    assert_eq!(total, 1);
}

// ============================================================================
// Test 5: Cancellation Behavior
// ============================================================================

#[tokio::test]
async fn test_callback_respects_cancellation_flag() {
    let callback = Arc::new(CancellableCallback::new());

    // Start some callbacks
    let cb1 = Arc::clone(&callback);
    let handle1 = tokio::spawn(async move {
        cb1.on_playbook_start("test").await;
    });

    handle1.await.unwrap();

    // Set cancellation flag
    callback.cancel();

    // Start more callbacks after cancellation
    let cb2 = Arc::clone(&callback);
    let handle2 = tokio::spawn(async move {
        cb2.on_playbook_end("test", true).await;
    });

    handle2.await.unwrap();

    // First callback should have completed, second should be cancelled
    assert_eq!(callback.completed_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback.cancelled_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_cancellation_mid_execution() {
    let callback = Arc::new(CancellableCallback::new());

    // Start multiple callbacks
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                if i == 2 {
                    // Cancel mid-way through
                    cb.cancel();
                }
                cb.on_task_start(&format!("task_{}", i), "host1").await;
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    // Some should have completed, some cancelled
    let completed = callback.completed_count.load(Ordering::SeqCst);
    let cancelled = callback.cancelled_count.load(Ordering::SeqCst);
    let started = callback.started_count.load(Ordering::SeqCst);

    assert_eq!(started, 5);
    assert_eq!(completed + cancelled, 5);
    assert!(cancelled > 0, "Some callbacks should have been cancelled");
}

#[tokio::test]
async fn test_tokio_select_cancellation() {
    let callback = Arc::new(SlowCallback::new(500));

    let cb = Arc::clone(&callback);

    // Use tokio::select to cancel a slow callback
    let result = tokio::select! {
        _ = async {
            cb.on_playbook_start("slow").await;
        } => {
            "completed"
        }
        _ = tokio::time::sleep(Duration::from_millis(50)) => {
            "cancelled"
        }
    };

    // Should be cancelled by timeout
    assert_eq!(result, "cancelled");
}

#[tokio::test]
async fn test_spawn_abort_cancellation() {
    let callback = Arc::new(SlowCallback::new(500));

    let cb = Arc::clone(&callback);

    let handle = tokio::spawn(async move {
        cb.on_playbook_start("aborted").await;
    });

    // Let it start
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Abort the task
    handle.abort();

    // Wait a bit for cleanup
    tokio::time::sleep(Duration::from_millis(20)).await;

    // The callback invocation count may or may not have incremented
    // depending on when the abort happened - just verify no panic
}

#[tokio::test]
async fn test_cancellation_token_pattern() {
    let callback = Arc::new(TimingCallback::new());
    let cancel_flag = Arc::new(AtomicBool::new(false));

    let cb = Arc::clone(&callback);
    let flag = Arc::clone(&cancel_flag);

    // Simulate a cancellable execution loop
    let handle = tokio::spawn(async move {
        for i in 0..100 {
            if flag.load(Ordering::SeqCst) {
                break;
            }
            cb.on_task_start(&format!("task_{}", i), "host1").await;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });

    // Cancel after a short delay
    tokio::time::sleep(Duration::from_millis(50)).await;
    cancel_flag.store(true, Ordering::SeqCst);

    handle.await.unwrap();

    // Some callbacks should have run, but not all 100
    let count = callback.invocation_count.load(Ordering::SeqCst);
    assert!(count > 0, "Some callbacks should have run");
    assert!(count < 100, "Cancellation should have stopped execution");
}

// ============================================================================
// Integration Tests - Complex Async Scenarios
// ============================================================================

#[tokio::test]
async fn test_full_playbook_lifecycle_async() {
    let callback = Arc::new(TimingCallback::new());
    let hosts = vec!["host1".to_string(), "host2".to_string()];

    // Simulate full playbook execution
    callback.on_playbook_start("integration_test").await;

    callback.on_play_start("play1", &hosts).await;

    // Parallel task execution on multiple hosts
    let handles: Vec<_> = hosts
        .iter()
        .flat_map(|host| {
            (0..5).map(|i| {
                let cb = Arc::clone(&callback);
                let h = host.clone();
                tokio::spawn(async move {
                    cb.on_task_start(&format!("task_{}", i), &h).await;
                    let result = create_execution_result(&h, &format!("task_{}", i), true);
                    cb.on_task_complete(&result).await;
                })
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    callback.on_play_end("play1", true).await;
    callback.on_playbook_end("integration_test", true).await;

    // Verify all callbacks fired
    // 1 playbook_start + 1 play_start + 10 task_start + 10 task_complete + 1 play_end + 1 playbook_end = 24
    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 24);
}

#[tokio::test]
async fn test_mixed_speed_callbacks_integration() {
    let slow = Arc::new(SlowCallback::new(50));
    let fast = Arc::new(TimingCallback::new());

    let start = Instant::now();

    // Run both types of callbacks
    let slow_handle = {
        let cb = Arc::clone(&slow);
        tokio::spawn(async move {
            for i in 0..3 {
                cb.on_task_start(&format!("slow_task_{}", i), "host1").await;
            }
        })
    };

    let fast_handle = {
        let cb = Arc::clone(&fast);
        tokio::spawn(async move {
            for i in 0..100 {
                cb.on_task_start(&format!("fast_task_{}", i), "host1").await;
            }
        })
    };

    let (slow_result, fast_result) = tokio::join!(slow_handle, fast_handle);
    slow_result.unwrap();
    fast_result.unwrap();

    let elapsed = start.elapsed();

    // Fast callbacks should not be delayed by slow ones
    // Total time should be dominated by slow callbacks (3 * 50ms in parallel = ~50ms)
    assert!(
        elapsed.as_millis() < 200,
        "Mixed callbacks should complete efficiently, took {:?}ms",
        elapsed.as_millis()
    );

    assert_eq!(slow.invocation_count.load(Ordering::SeqCst), 3);
    assert_eq!(fast.invocation_count.load(Ordering::SeqCst), 100);
}

#[tokio::test]
async fn test_callback_stress_test() {
    let callback = Arc::new(TimingCallback::new());

    let start = Instant::now();

    // Spawn many concurrent callbacks
    let handles: Vec<_> = (0..1000)
        .map(|i| {
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                cb.on_task_start(&format!("task_{}", i), &format!("host_{}", i % 100))
                    .await;
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let elapsed = start.elapsed();

    // 1000 fast callbacks should complete quickly
    assert!(
        elapsed.as_millis() < 1000,
        "1000 callbacks should complete in under 1 second, took {:?}ms",
        elapsed.as_millis()
    );

    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 1000);
}

#[tokio::test]
async fn test_callback_with_async_mutex() {
    use tokio::sync::Mutex;

    // Verify callbacks work correctly with async mutex
    let state = Arc::new(Mutex::new(Vec::<String>::new()));
    let callback = Arc::new(TimingCallback::new());

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let s = Arc::clone(&state);
            let cb = Arc::clone(&callback);
            tokio::spawn(async move {
                cb.on_task_start(&format!("task_{}", i), "host1").await;
                let mut guard = s.lock().await;
                guard.push(format!("task_{}", i));
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let final_state = state.lock().await;
    assert_eq!(final_state.len(), 10);
    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 10);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_callback_panic_isolation() {
    let callback = Arc::new(TimingCallback::new());

    // Spawn a task that panics
    let cb = Arc::clone(&callback);
    let panic_handle = tokio::spawn(async move {
        cb.on_playbook_start("panic_test").await;
        panic!("Intentional panic");
    });

    // Spawn normal callbacks
    let cb2 = Arc::clone(&callback);
    let normal_handle = tokio::spawn(async move {
        cb2.on_playbook_start("normal_test").await;
    });

    // Panic should be caught
    let panic_result = panic_handle.await;
    assert!(panic_result.is_err());

    // Normal callback should still work
    normal_handle.await.unwrap();

    // At least one callback should have completed
    assert!(callback.invocation_count.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn test_callback_error_recovery() {
    let callback = Arc::new(TimingCallback::new());

    // Run callbacks that might fail
    for i in 0..5 {
        let cb = Arc::clone(&callback);
        let result = tokio::spawn(async move {
            cb.on_task_start(&format!("task_{}", i), "host1").await;
            if i == 2 {
                return Err("simulated error");
            }
            Ok(())
        })
        .await
        .unwrap();

        // Continue despite errors
        if result.is_err() {
            // Handle error but continue
        }
    }

    // All 5 callbacks should have been invoked
    assert_eq!(callback.invocation_count.load(Ordering::SeqCst), 5);
}
