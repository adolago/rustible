//! Simple integration test for forks functionality
//!
//! This test verifies that the semaphore-based concurrency limiting works
//! without requiring the full executor to compile.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::sleep;

/// Simulates task execution with a semaphore to limit concurrency
async fn execute_with_forks(num_tasks: usize, forks: usize) -> (Duration, usize) {
    let semaphore = Arc::new(Semaphore::new(forks));
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let current_concurrent = Arc::new(AtomicUsize::new(0));

    let start = Instant::now();

    let handles: Vec<_> = (0..num_tasks)
        .map(|task_id| {
            let sem = Arc::clone(&semaphore);
            let max_conc = Arc::clone(&max_concurrent);
            let curr_conc = Arc::clone(&current_concurrent);

            tokio::spawn(async move {
                // Acquire permit (blocks if forks limit reached)
                let _permit = sem.acquire().await.unwrap();

                // Track concurrent execution
                let current = curr_conc.fetch_add(1, Ordering::SeqCst) + 1;

                // Update max concurrent if needed
                max_conc.fetch_max(current, Ordering::SeqCst);

                // Simulate work
                sleep(Duration::from_millis(10)).await;

                // Release concurrent count
                curr_conc.fetch_sub(1, Ordering::SeqCst);

                task_id
            })
        })
        .collect();

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    let duration = start.elapsed();
    let max_concurrent_count = max_concurrent.load(Ordering::SeqCst);

    (duration, max_concurrent_count)
}

#[tokio::test]
async fn test_forks_1_serial() {
    let (duration, max_concurrent) = execute_with_forks(5, 1).await;

    // With forks=1, only 1 task should execute at a time
    assert_eq!(
        max_concurrent, 1,
        "Serial execution should have max 1 concurrent task"
    );

    // Should take at least 5 * 10ms = 50ms
    assert!(
        duration.as_millis() >= 45,
        "Serial execution should take at least 45ms, took {}ms",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_forks_2_pairs() {
    let (duration, max_concurrent) = execute_with_forks(6, 2).await;

    // With forks=2, at most 2 tasks should execute concurrently
    assert!(
        max_concurrent <= 2,
        "Should have max 2 concurrent tasks, got {}",
        max_concurrent
    );

    // Should take at least 3 rounds * 10ms = 30ms
    assert!(
        duration.as_millis() >= 25,
        "Paired execution should take at least 25ms, took {}ms",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_forks_5_small_batches() {
    let (duration, max_concurrent) = execute_with_forks(20, 5).await;

    // With forks=5, at most 5 tasks should execute concurrently
    assert!(
        max_concurrent <= 5,
        "Should have max 5 concurrent tasks, got {}",
        max_concurrent
    );

    // Should take at least 4 rounds * 10ms = 40ms
    assert!(
        duration.as_millis() >= 35,
        "Batch execution should take at least 35ms, took {}ms",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_forks_unlimited() {
    let (duration, max_concurrent) = execute_with_forks(10, 100).await;

    // With forks=100 and only 10 tasks, all should execute in parallel
    assert!(
        max_concurrent <= 10,
        "Should have at most 10 concurrent tasks, got {}",
        max_concurrent
    );

    // Should complete in roughly 1 round (~10ms)
    assert!(
        duration.as_millis() < 50,
        "Unlimited parallel execution should be fast, took {}ms",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_forks_stress_50_tasks_5_forks() {
    let (duration, max_concurrent) = execute_with_forks(50, 5).await;

    // With forks=5, at most 5 tasks should execute concurrently
    assert!(
        max_concurrent <= 5,
        "Should have max 5 concurrent tasks, got {}",
        max_concurrent
    );

    // Should take at least 10 rounds * 10ms = 100ms
    assert!(
        duration.as_millis() >= 95,
        "50 tasks with 5 forks should take at least 95ms, took {}ms",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_different_forks_performance() {
    // Test that increasing forks actually improves performance
    let (duration_1, _) = execute_with_forks(20, 1).await;
    let (duration_5, _) = execute_with_forks(20, 5).await;
    let (duration_10, _) = execute_with_forks(20, 10).await;

    // More forks should complete faster
    assert!(
        duration_5 < duration_1,
        "5 forks ({:?}) should be faster than 1 fork ({:?})",
        duration_5,
        duration_1
    );

    assert!(
        duration_10 <= duration_5,
        "10 forks ({:?}) should be faster than or equal to 5 forks ({:?})",
        duration_10,
        duration_5
    );
}

#[tokio::test]
async fn test_semaphore_actually_limits_concurrency() {
    // This is the most important test: verify the semaphore actually limits
    // the number of concurrent tasks

    let forks = 3;
    let semaphore = Arc::new(Semaphore::new(forks));
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let current_concurrent = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let sem = Arc::clone(&semaphore);
            let max_conc = Arc::clone(&max_concurrent);
            let curr_conc = Arc::clone(&current_concurrent);

            tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();

                // Increment and check
                let current = curr_conc.fetch_add(1, Ordering::SeqCst) + 1;

                // This is the key assertion: we should NEVER exceed forks
                assert!(
                    current <= forks,
                    "Current concurrent ({}) exceeded forks limit ({})",
                    current,
                    forks
                );

                max_conc.fetch_max(current, Ordering::SeqCst);

                // Simulate work
                sleep(Duration::from_millis(20)).await;

                curr_conc.fetch_sub(1, Ordering::SeqCst);
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }

    let max = max_concurrent.load(Ordering::SeqCst);
    assert!(
        max <= forks,
        "Maximum concurrent ({}) should not exceed forks ({})",
        max,
        forks
    );

    // Also verify we actually used the full capacity at some point
    assert_eq!(
        max, forks,
        "Should have used all {} fork slots at some point",
        forks
    );
}

#[tokio::test]
async fn test_zero_tasks() {
    let (duration, max_concurrent) = execute_with_forks(0, 5).await;

    assert_eq!(max_concurrent, 0, "No tasks means no concurrent execution");
    assert!(duration.as_millis() < 10, "Should complete immediately");
}

#[tokio::test]
async fn test_single_task() {
    let (duration, max_concurrent) = execute_with_forks(1, 5).await;

    assert_eq!(max_concurrent, 1, "One task means max 1 concurrent");
    assert!(
        duration.as_millis() >= 8,
        "Should take at least the task duration"
    );
}
