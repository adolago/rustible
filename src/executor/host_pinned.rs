//! Host-pinned execution strategy with dedicated workers
//!
//! This module provides a host-pinned execution strategy where each host
//! gets a dedicated worker thread. This maximizes connection reuse and
//! eliminates connection pooling overhead.
//!
//! ## Benefits
//! - Each host has a persistent SSH connection
//! - No connection pool contention
//! - Optimal for long-running playbooks with many tasks
//! - Predictable memory usage (one connection per host)

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tracing::{debug, info, trace};

/// Configuration for host-pinned execution
#[derive(Debug, Clone)]
pub struct HostPinnedConfig {
    /// Maximum concurrent hosts (each gets a worker)
    pub max_hosts: usize,
    /// Task queue depth per host
    pub queue_depth: usize,
    /// Worker idle timeout before cleanup
    pub idle_timeout: Duration,
    /// Enable task coalescing within a worker
    pub enable_coalescing: bool,
    /// Connection keepalive interval
    pub keepalive_interval: Duration,
}

impl Default for HostPinnedConfig {
    fn default() -> Self {
        Self {
            max_hosts: 50,
            queue_depth: 100,
            idle_timeout: Duration::from_secs(60),
            enable_coalescing: true,
            keepalive_interval: Duration::from_secs(30),
        }
    }
}

/// A task to be executed on a specific host
#[derive(Debug)]
pub struct HostTask<T, R> {
    /// Task payload
    pub payload: T,
    /// Priority (higher = more urgent)
    pub priority: u8,
    /// Response channel
    pub response_tx: oneshot::Sender<R>,
}

impl<T, R> HostTask<T, R> {
    pub fn new(payload: T, response_tx: oneshot::Sender<R>) -> Self {
        Self {
            payload,
            priority: 0,
            response_tx,
        }
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    pub fn respond(self, result: R) {
        let _ = self.response_tx.send(result);
    }
}

/// Statistics for a single host worker
#[derive(Debug, Clone, Default)]
pub struct HostWorkerStats {
    /// Tasks executed
    pub tasks_executed: u64,
    /// Tasks failed
    pub tasks_failed: u64,
    /// Total execution time (ms)
    pub total_execution_ms: u64,
    /// Average task time (ms)
    pub avg_task_time_ms: u64,
    /// Last active time
    pub last_active: Option<Instant>,
    /// Connection established time
    pub connected_at: Option<Instant>,
    /// Number of reconnections
    pub reconnections: u64,
}

/// A dedicated worker for a single host
pub struct HostWorker<T: Send + 'static, R: Send + 'static> {
    /// Host identifier
    host: String,
    /// Task receiver
    task_rx: mpsc::Receiver<HostTask<T, R>>,
    /// Shutdown flag
    shutdown: Arc<AtomicBool>,
    /// Statistics
    stats: Arc<RwLock<HostWorkerStats>>,
    /// Configuration
    config: HostPinnedConfig,
}

impl<T: Send + 'static, R: Send + 'static> HostWorker<T, R> {
    /// Run the worker loop
    pub async fn run<F, Fut>(mut self, executor: F)
    where
        F: Fn(String, T) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = R> + Send,
    {
        info!("Starting dedicated worker for host: {}", self.host);

        // Record connection time
        {
            let mut stats = self.stats.write();
            stats.connected_at = Some(Instant::now());
            stats.last_active = Some(Instant::now());
        }

        loop {
            // Check for shutdown
            if self.shutdown.load(Ordering::SeqCst) {
                debug!("Worker for {} received shutdown signal", self.host);
                break;
            }

            // Wait for task with idle timeout
            let task = tokio::select! {
                task = self.task_rx.recv() => task,
                _ = tokio::time::sleep(self.config.idle_timeout) => {
                    trace!("Worker for {} idle timeout", self.host);
                    // Could trigger cleanup here
                    continue;
                }
            };

            let task = match task {
                Some(t) => t,
                None => {
                    debug!("Task channel closed for {}", self.host);
                    break;
                }
            };

            // Execute the task - extract payload and response channel before moving
            let payload = task.payload;
            let response_tx = task.response_tx;

            let start = Instant::now();
            let result = executor(self.host.clone(), payload).await;
            let elapsed = start.elapsed();

            // Update statistics
            {
                let mut stats = self.stats.write();
                stats.tasks_executed += 1;
                stats.total_execution_ms += elapsed.as_millis() as u64;
                stats.avg_task_time_ms = stats.total_execution_ms / stats.tasks_executed;
                stats.last_active = Some(Instant::now());
            }

            // Send response
            let _ = response_tx.send(result);
        }

        info!("Worker for {} shutting down", self.host);
    }
}

/// Handle to submit tasks to a host worker
pub struct HostWorkerHandle<T: Send + 'static, R: Send + 'static> {
    /// Host identifier
    host: String,
    /// Task sender
    task_tx: mpsc::Sender<HostTask<T, R>>,
    /// Statistics
    stats: Arc<RwLock<HostWorkerStats>>,
}

impl<T: Send + 'static, R: Send + 'static> Clone for HostWorkerHandle<T, R> {
    fn clone(&self) -> Self {
        Self {
            host: self.host.clone(),
            task_tx: self.task_tx.clone(),
            stats: Arc::clone(&self.stats),
        }
    }
}

impl<T: Send + 'static, R: Send + 'static> HostWorkerHandle<T, R> {
    /// Submit a task and wait for result
    pub async fn submit(&self, payload: T) -> Result<R, String> {
        let (response_tx, response_rx) = oneshot::channel();
        let task = HostTask::new(payload, response_tx);

        self.task_tx
            .send(task)
            .await
            .map_err(|_| "Worker channel closed".to_string())?;

        response_rx
            .await
            .map_err(|_| "Response channel closed".to_string())
    }

    /// Submit a task without waiting (fire and forget)
    pub fn submit_nowait(&self, payload: T) -> Result<oneshot::Receiver<R>, String> {
        let (response_tx, response_rx) = oneshot::channel();
        let task = HostTask::new(payload, response_tx);

        self.task_tx
            .try_send(task)
            .map_err(|_| "Worker queue full".to_string())?;

        Ok(response_rx)
    }

    /// Get current statistics
    pub fn stats(&self) -> HostWorkerStats {
        self.stats.read().clone()
    }

    /// Get the host this handle is for
    pub fn host(&self) -> &str {
        &self.host
    }
}

/// Pool of host-pinned workers
pub struct HostPinnedPool<T: Send + 'static, R: Send + 'static> {
    /// Configuration
    config: HostPinnedConfig,
    /// Worker handles by host
    workers: Arc<RwLock<HashMap<String, HostWorkerHandle<T, R>>>>,
    /// Global shutdown flag
    shutdown: Arc<AtomicBool>,
    /// Semaphore to limit total workers
    worker_semaphore: Arc<Semaphore>,
    /// Total tasks submitted
    total_tasks: Arc<AtomicU64>,
    /// Total tasks completed
    tasks_completed: Arc<AtomicU64>,
}

impl<T: Send + Sync + 'static, R: Send + 'static> HostPinnedPool<T, R> {
    /// Create a new host-pinned pool
    pub fn new(config: HostPinnedConfig) -> Self {
        let max_hosts = config.max_hosts;
        Self {
            config,
            workers: Arc::new(RwLock::new(HashMap::new())),
            shutdown: Arc::new(AtomicBool::new(false)),
            worker_semaphore: Arc::new(Semaphore::new(max_hosts)),
            total_tasks: Arc::new(AtomicU64::new(0)),
            tasks_completed: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get or create a worker handle for a host
    pub fn get_or_create_worker<F, Fut>(
        &self,
        host: &str,
        executor_factory: F,
    ) -> HostWorkerHandle<T, R>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<
                Output = Box<
                    dyn Fn(
                            String,
                            T,
                        )
                            -> std::pin::Pin<Box<dyn std::future::Future<Output = R> + Send>>
                        + Send
                        + Sync,
                >,
            > + Send
            + 'static,
    {
        // Check if worker already exists
        {
            let workers = self.workers.read();
            if let Some(handle) = workers.get(host) {
                return handle.clone();
            }
        }

        // Create new worker
        let (task_tx, task_rx) = mpsc::channel(self.config.queue_depth);
        let stats = Arc::new(RwLock::new(HostWorkerStats::default()));

        let worker = HostWorker {
            host: host.to_string(),
            task_rx,
            shutdown: self.shutdown.clone(),
            stats: stats.clone(),
            config: self.config.clone(),
        };

        let handle = HostWorkerHandle {
            host: host.to_string(),
            task_tx,
            stats,
        };

        // Store handle
        {
            let mut workers = self.workers.write();
            workers.insert(host.to_string(), handle.clone());
        }

        // Spawn worker task
        let host_clone = host.to_string();
        tokio::spawn(async move {
            let executor = executor_factory().await;
            worker
                .run(move |host, payload| {
                    let exec = executor.as_ref();
                    exec(host, payload)
                })
                .await;
            debug!("Worker for {} has exited", host_clone);
        });

        handle
    }

    /// Submit a task to a specific host
    pub async fn submit(&self, host: &str, payload: T) -> Result<R, String>
    where
        T: Clone,
    {
        self.total_tasks.fetch_add(1, Ordering::Relaxed);

        let handle = {
            let workers = self.workers.read();
            workers.get(host).cloned()
        };

        if let Some(handle) = handle {
            let result = handle.submit(payload).await;
            self.tasks_completed.fetch_add(1, Ordering::Relaxed);
            result
        } else {
            Err(format!("No worker for host: {}", host))
        }
    }

    /// Get all worker handles
    pub fn handles(&self) -> Vec<HostWorkerHandle<T, R>> {
        self.workers.read().values().cloned().collect()
    }

    /// Get statistics for all workers
    pub fn all_stats(&self) -> HashMap<String, HostWorkerStats> {
        self.workers
            .read()
            .iter()
            .map(|(host, handle)| (host.clone(), handle.stats()))
            .collect()
    }

    /// Get aggregate statistics
    pub fn aggregate_stats(&self) -> PoolStats {
        let worker_stats: Vec<_> = self.workers.read().values().map(|h| h.stats()).collect();

        let total_executed: u64 = worker_stats.iter().map(|s| s.tasks_executed).sum();
        let total_failed: u64 = worker_stats.iter().map(|s| s.tasks_failed).sum();
        let total_exec_ms: u64 = worker_stats.iter().map(|s| s.total_execution_ms).sum();

        PoolStats {
            worker_count: worker_stats.len(),
            total_tasks_executed: total_executed,
            total_tasks_failed: total_failed,
            avg_task_time_ms: if total_executed > 0 {
                total_exec_ms / total_executed
            } else {
                0
            },
            pending_tasks: self.total_tasks.load(Ordering::Relaxed)
                - self.tasks_completed.load(Ordering::Relaxed),
        }
    }

    /// Shutdown all workers
    pub fn shutdown(&self) {
        info!("Shutting down host-pinned pool");
        self.shutdown.store(true, Ordering::SeqCst);
        self.workers.write().clear();
    }

    /// Remove idle workers
    pub fn cleanup_idle(&self) {
        let now = Instant::now();
        let timeout = self.config.idle_timeout;

        let mut workers = self.workers.write();
        workers.retain(|host, handle| {
            let stats = handle.stats();
            if let Some(last_active) = stats.last_active {
                if now.duration_since(last_active) > timeout {
                    debug!("Removing idle worker for {}", host);
                    return false;
                }
            }
            true
        });
    }

    /// Get the number of active workers
    pub fn worker_count(&self) -> usize {
        self.workers.read().len()
    }
}

/// Aggregate pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Number of active workers
    pub worker_count: usize,
    /// Total tasks executed across all workers
    pub total_tasks_executed: u64,
    /// Total tasks failed
    pub total_tasks_failed: u64,
    /// Average task time across all workers
    pub avg_task_time_ms: u64,
    /// Tasks currently pending
    pub pending_tasks: u64,
}

/// Builder for host-pinned execution
pub struct HostPinnedExecutor {
    config: HostPinnedConfig,
}

impl HostPinnedExecutor {
    /// Create a new executor builder
    pub fn new() -> Self {
        Self {
            config: HostPinnedConfig::default(),
        }
    }

    /// Set maximum concurrent hosts
    pub fn max_hosts(mut self, max: usize) -> Self {
        self.config.max_hosts = max;
        self
    }

    /// Set queue depth per host
    pub fn queue_depth(mut self, depth: usize) -> Self {
        self.config.queue_depth = depth;
        self
    }

    /// Set idle timeout
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.config.idle_timeout = timeout;
        self
    }

    /// Enable task coalescing
    pub fn enable_coalescing(mut self, enable: bool) -> Self {
        self.config.enable_coalescing = enable;
        self
    }

    /// Build the configuration
    pub fn build(self) -> HostPinnedConfig {
        self.config
    }
}

impl Default for HostPinnedExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_host_worker_stats() {
        let stats = HostWorkerStats {
            tasks_executed: 100,
            tasks_failed: 5,
            total_execution_ms: 5000,
            avg_task_time_ms: 50,
            last_active: Some(Instant::now()),
            connected_at: Some(Instant::now()),
            reconnections: 0,
        };

        assert_eq!(stats.tasks_executed, 100);
        assert_eq!(stats.avg_task_time_ms, 50);
    }

    #[tokio::test]
    async fn test_pool_stats_aggregation() {
        let stats = PoolStats {
            worker_count: 5,
            total_tasks_executed: 500,
            total_tasks_failed: 10,
            avg_task_time_ms: 25,
            pending_tasks: 0,
        };

        assert_eq!(stats.worker_count, 5);
        assert_eq!(stats.total_tasks_executed, 500);
    }

    #[test]
    fn test_executor_builder() {
        let config = HostPinnedExecutor::new()
            .max_hosts(100)
            .queue_depth(200)
            .idle_timeout(Duration::from_secs(120))
            .enable_coalescing(true)
            .build();

        assert_eq!(config.max_hosts, 100);
        assert_eq!(config.queue_depth, 200);
        assert_eq!(config.idle_timeout, Duration::from_secs(120));
        assert!(config.enable_coalescing);
    }
}
