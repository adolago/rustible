//! Command execution timing metrics
//!
//! This module provides metrics for tracking command execution times,
//! including per-module and per-host command statistics.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use super::types::{Counter, Gauge, Histogram};

// ============================================================================
// Command Execution Metrics
// ============================================================================

/// Metrics for tracking command execution
#[derive(Debug)]
pub struct CommandMetrics {
    /// Command execution duration histogram (milliseconds)
    pub execution_duration: Histogram,
    /// Total commands executed
    pub commands_executed: Counter,
    /// Successful commands
    pub commands_succeeded: Counter,
    /// Failed commands
    pub commands_failed: Counter,
    /// Commands currently executing
    pub commands_in_progress: Gauge,
    /// Total bytes transferred (stdout)
    pub bytes_stdout: Counter,
    /// Total bytes transferred (stderr)
    pub bytes_stderr: Counter,
    /// Per-module metrics
    per_module: RwLock<HashMap<String, Arc<ModuleMetrics>>>,
    /// Per-host command metrics
    per_host: RwLock<HashMap<String, Arc<HostCommandMetrics>>>,
}

impl Default for CommandMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandMetrics {
    /// Create new command metrics
    pub fn new() -> Self {
        Self {
            execution_duration: Histogram::with_buckets(
                "rustible_command_duration_ms",
                "Command execution duration in milliseconds",
                &[
                    10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 30000.0,
                    60000.0,
                ],
            ),
            commands_executed: Counter::new(
                "rustible_commands_executed_total",
                "Total number of commands executed",
            ),
            commands_succeeded: Counter::new(
                "rustible_commands_succeeded_total",
                "Total number of successful commands",
            ),
            commands_failed: Counter::new(
                "rustible_commands_failed_total",
                "Total number of failed commands",
            ),
            commands_in_progress: Gauge::new(
                "rustible_commands_in_progress",
                "Number of commands currently executing",
            ),
            bytes_stdout: Counter::new(
                "rustible_command_stdout_bytes_total",
                "Total bytes written to stdout",
            ),
            bytes_stderr: Counter::new(
                "rustible_command_stderr_bytes_total",
                "Total bytes written to stderr",
            ),
            per_module: RwLock::new(HashMap::new()),
            per_host: RwLock::new(HashMap::new()),
        }
    }

    /// Start timing a command execution
    pub fn start_command(&self, host: &str, module: &str) -> CommandTimer {
        self.commands_in_progress.inc();
        CommandTimer::new(host.to_string(), module.to_string())
    }

    /// Record a successful command execution
    pub fn record_success(&self, timer: CommandTimer, stdout_len: usize, stderr_len: usize) {
        let duration = timer.elapsed();
        let host = timer.host.clone();
        let module = timer.module.clone();

        // Record global metrics
        self.execution_duration
            .observe(duration.as_secs_f64() * 1000.0);
        self.commands_executed.inc();
        self.commands_succeeded.inc();
        self.commands_in_progress.dec();
        self.bytes_stdout.inc_by(stdout_len as f64);
        self.bytes_stderr.inc_by(stderr_len as f64);

        // Record per-module metrics
        let module_metrics = self.get_or_create_module_metrics(&module);
        module_metrics
            .duration
            .observe(duration.as_secs_f64() * 1000.0);
        module_metrics.executed.inc();
        module_metrics.succeeded.inc();

        // Record per-host metrics
        let host_metrics = self.get_or_create_host_metrics(&host);
        host_metrics
            .duration
            .observe(duration.as_secs_f64() * 1000.0);
        host_metrics.executed.inc();
        host_metrics.succeeded.inc();
    }

    /// Record a failed command execution
    pub fn record_failure(&self, timer: CommandTimer, exit_code: i32, stderr_len: usize) {
        let duration = timer.elapsed();
        let host = timer.host.clone();
        let module = timer.module.clone();

        // Record global metrics
        self.execution_duration
            .observe(duration.as_secs_f64() * 1000.0);
        self.commands_executed.inc();
        self.commands_failed.inc();
        self.commands_in_progress.dec();
        self.bytes_stderr.inc_by(stderr_len as f64);

        // Record per-module metrics
        let module_metrics = self.get_or_create_module_metrics(&module);
        module_metrics
            .duration
            .observe(duration.as_secs_f64() * 1000.0);
        module_metrics.executed.inc();
        module_metrics.failed.inc();
        module_metrics.last_exit_code.set(exit_code as f64);

        // Record per-host metrics
        let host_metrics = self.get_or_create_host_metrics(&host);
        host_metrics
            .duration
            .observe(duration.as_secs_f64() * 1000.0);
        host_metrics.executed.inc();
        host_metrics.failed.inc();
    }

    /// Cancel a command (e.g., due to timeout)
    pub fn record_cancelled(&self, timer: CommandTimer) {
        self.commands_in_progress.dec();
        let module_metrics = self.get_or_create_module_metrics(&timer.module);
        module_metrics.cancelled.inc();
        let host_metrics = self.get_or_create_host_metrics(&timer.host);
        host_metrics.cancelled.inc();
    }

    /// Get summary of command metrics
    pub fn summary(&self) -> CommandMetricsSummary {
        let per_module = self.per_module.read();
        let per_host = self.per_host.read();
        CommandMetricsSummary {
            total_executed: self.commands_executed.get() as u64,
            total_succeeded: self.commands_succeeded.get() as u64,
            total_failed: self.commands_failed.get() as u64,
            in_progress: self.commands_in_progress.get() as u64,
            avg_duration_ms: if self.execution_duration.count() > 0 {
                self.execution_duration.sum() / self.execution_duration.count() as f64
            } else {
                0.0
            },
            p50_duration_ms: self.execution_duration.quantile(0.5),
            p95_duration_ms: self.execution_duration.quantile(0.95),
            p99_duration_ms: self.execution_duration.quantile(0.99),
            total_stdout_bytes: self.bytes_stdout.get() as u64,
            total_stderr_bytes: self.bytes_stderr.get() as u64,
            unique_modules: per_module.len(),
            unique_hosts: per_host.len(),
        }
    }

    /// Get metrics for a specific module
    pub fn module_metrics(&self, module: &str) -> Option<ModuleMetricsSnapshot> {
        self.per_module.read().get(module).map(|m| m.snapshot())
    }

    /// Get all module metrics
    pub fn all_module_metrics(&self) -> Vec<ModuleMetricsSnapshot> {
        self.per_module
            .read()
            .values()
            .map(|m| m.snapshot())
            .collect()
    }

    /// Get metrics for a specific host
    pub fn host_metrics(&self, host: &str) -> Option<HostCommandMetricsSnapshot> {
        self.per_host.read().get(host).map(|m| m.snapshot())
    }

    /// Get all host command metrics
    pub fn all_host_metrics(&self) -> Vec<HostCommandMetricsSnapshot> {
        self.per_host
            .read()
            .values()
            .map(|m| m.snapshot())
            .collect()
    }

    /// Get or create per-module metrics
    fn get_or_create_module_metrics(&self, module: &str) -> Arc<ModuleMetrics> {
        {
            let read_guard = self.per_module.read();
            if let Some(metrics) = read_guard.get(module) {
                return Arc::clone(metrics);
            }
        }

        let mut write_guard = self.per_module.write();
        Arc::clone(
            write_guard
                .entry(module.to_string())
                .or_insert_with(|| Arc::new(ModuleMetrics::new(module))),
        )
    }

    /// Get or create per-host metrics
    fn get_or_create_host_metrics(&self, host: &str) -> Arc<HostCommandMetrics> {
        {
            let read_guard = self.per_host.read();
            if let Some(metrics) = read_guard.get(host) {
                return Arc::clone(metrics);
            }
        }

        let mut write_guard = self.per_host.write();
        Arc::clone(
            write_guard
                .entry(host.to_string())
                .or_insert_with(|| Arc::new(HostCommandMetrics::new(host))),
        )
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.execution_duration.reset();
        self.commands_executed.reset();
        self.commands_succeeded.reset();
        self.commands_failed.reset();
        self.commands_in_progress.set(0.0);
        self.bytes_stdout.reset();
        self.bytes_stderr.reset();
        self.per_module.write().clear();
        self.per_host.write().clear();
    }
}

// ============================================================================
// Per-Module Metrics
// ============================================================================

/// Metrics for a specific module
#[derive(Debug, Clone)]
pub struct ModuleMetrics {
    /// Module name
    pub module: String,
    /// Execution duration histogram
    pub duration: Histogram,
    /// Total executions
    pub executed: Counter,
    /// Successful executions
    pub succeeded: Counter,
    /// Failed executions
    pub failed: Counter,
    /// Cancelled executions
    pub cancelled: Counter,
    /// Last exit code
    pub last_exit_code: Gauge,
}

impl ModuleMetrics {
    /// Create new module metrics
    pub fn new(module: &str) -> Self {
        let labels = {
            let mut l = HashMap::new();
            l.insert("module".to_string(), module.to_string());
            l
        };

        Self {
            module: module.to_string(),
            duration: Histogram::with_labels(
                "rustible_module_duration_ms",
                "Module execution duration in milliseconds",
                &[
                    10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
                ],
                labels.clone(),
            ),
            executed: Counter::with_labels(
                "rustible_module_executed_total",
                "Module executions",
                labels.clone(),
            ),
            succeeded: Counter::with_labels(
                "rustible_module_succeeded_total",
                "Successful module executions",
                labels.clone(),
            ),
            failed: Counter::with_labels(
                "rustible_module_failed_total",
                "Failed module executions",
                labels.clone(),
            ),
            cancelled: Counter::with_labels(
                "rustible_module_cancelled_total",
                "Cancelled module executions",
                labels.clone(),
            ),
            last_exit_code: Gauge::with_labels(
                "rustible_module_last_exit_code",
                "Last exit code for module",
                labels,
            ),
        }
    }

    /// Create a snapshot of current metrics
    pub fn snapshot(&self) -> ModuleMetricsSnapshot {
        ModuleMetricsSnapshot {
            module: self.module.clone(),
            executed: self.executed.get() as u64,
            succeeded: self.succeeded.get() as u64,
            failed: self.failed.get() as u64,
            cancelled: self.cancelled.get() as u64,
            avg_duration_ms: if self.duration.count() > 0 {
                self.duration.sum() / self.duration.count() as f64
            } else {
                0.0
            },
            p50_duration_ms: self.duration.quantile(0.5),
            p95_duration_ms: self.duration.quantile(0.95),
            p99_duration_ms: self.duration.quantile(0.99),
            last_exit_code: self.last_exit_code.get() as i32,
        }
    }
}

/// Snapshot of module metrics
#[derive(Debug, Clone)]
pub struct ModuleMetricsSnapshot {
    /// Module name
    pub module: String,
    /// Total executions
    pub executed: u64,
    /// Successful executions
    pub succeeded: u64,
    /// Failed executions
    pub failed: u64,
    /// Cancelled executions
    pub cancelled: u64,
    /// Average duration in milliseconds
    pub avg_duration_ms: f64,
    /// P50 duration in milliseconds
    pub p50_duration_ms: f64,
    /// P95 duration in milliseconds
    pub p95_duration_ms: f64,
    /// P99 duration in milliseconds
    pub p99_duration_ms: f64,
    /// Last exit code
    pub last_exit_code: i32,
}

impl ModuleMetricsSnapshot {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.executed == 0 {
            0.0
        } else {
            self.succeeded as f64 / self.executed as f64 * 100.0
        }
    }
}

// ============================================================================
// Per-Host Command Metrics
// ============================================================================

/// Command metrics for a specific host
#[derive(Debug, Clone)]
pub struct HostCommandMetrics {
    /// Host identifier
    pub host: String,
    /// Execution duration histogram
    pub duration: Histogram,
    /// Total executions
    pub executed: Counter,
    /// Successful executions
    pub succeeded: Counter,
    /// Failed executions
    pub failed: Counter,
    /// Cancelled executions
    pub cancelled: Counter,
}

impl HostCommandMetrics {
    /// Create new host command metrics
    pub fn new(host: &str) -> Self {
        let labels = {
            let mut l = HashMap::new();
            l.insert("host".to_string(), host.to_string());
            l
        };

        Self {
            host: host.to_string(),
            duration: Histogram::with_labels(
                "rustible_host_command_duration_ms",
                "Command execution duration per host in milliseconds",
                &[
                    10.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
                ],
                labels.clone(),
            ),
            executed: Counter::with_labels(
                "rustible_host_commands_executed_total",
                "Commands executed per host",
                labels.clone(),
            ),
            succeeded: Counter::with_labels(
                "rustible_host_commands_succeeded_total",
                "Successful commands per host",
                labels.clone(),
            ),
            failed: Counter::with_labels(
                "rustible_host_commands_failed_total",
                "Failed commands per host",
                labels.clone(),
            ),
            cancelled: Counter::with_labels(
                "rustible_host_commands_cancelled_total",
                "Cancelled commands per host",
                labels,
            ),
        }
    }

    /// Create a snapshot of current metrics
    pub fn snapshot(&self) -> HostCommandMetricsSnapshot {
        HostCommandMetricsSnapshot {
            host: self.host.clone(),
            executed: self.executed.get() as u64,
            succeeded: self.succeeded.get() as u64,
            failed: self.failed.get() as u64,
            cancelled: self.cancelled.get() as u64,
            avg_duration_ms: if self.duration.count() > 0 {
                self.duration.sum() / self.duration.count() as f64
            } else {
                0.0
            },
            p50_duration_ms: self.duration.quantile(0.5),
            p95_duration_ms: self.duration.quantile(0.95),
            p99_duration_ms: self.duration.quantile(0.99),
        }
    }
}

/// Snapshot of host command metrics
#[derive(Debug, Clone)]
pub struct HostCommandMetricsSnapshot {
    /// Host identifier
    pub host: String,
    /// Total executions
    pub executed: u64,
    /// Successful executions
    pub succeeded: u64,
    /// Failed executions
    pub failed: u64,
    /// Cancelled executions
    pub cancelled: u64,
    /// Average duration in milliseconds
    pub avg_duration_ms: f64,
    /// P50 duration in milliseconds
    pub p50_duration_ms: f64,
    /// P95 duration in milliseconds
    pub p95_duration_ms: f64,
    /// P99 duration in milliseconds
    pub p99_duration_ms: f64,
}

// ============================================================================
// Command Timer
// ============================================================================

/// Timer for measuring command execution time
#[derive(Debug)]
pub struct CommandTimer {
    /// Host the command is running on
    pub host: String,
    /// Module being executed
    pub module: String,
    /// Start time
    start: Instant,
}

impl CommandTimer {
    /// Create a new command timer
    pub fn new(host: String, module: String) -> Self {
        Self {
            host,
            module,
            start: Instant::now(),
        }
    }

    /// Get elapsed time since timer started
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

// ============================================================================
// Summary Types
// ============================================================================

/// Summary of command metrics
#[derive(Debug, Clone)]
pub struct CommandMetricsSummary {
    /// Total commands executed
    pub total_executed: u64,
    /// Total successful commands
    pub total_succeeded: u64,
    /// Total failed commands
    pub total_failed: u64,
    /// Commands currently in progress
    pub in_progress: u64,
    /// Average duration in milliseconds
    pub avg_duration_ms: f64,
    /// P50 duration in milliseconds
    pub p50_duration_ms: f64,
    /// P95 duration in milliseconds
    pub p95_duration_ms: f64,
    /// P99 duration in milliseconds
    pub p99_duration_ms: f64,
    /// Total stdout bytes
    pub total_stdout_bytes: u64,
    /// Total stderr bytes
    pub total_stderr_bytes: u64,
    /// Number of unique modules
    pub unique_modules: usize,
    /// Number of unique hosts
    pub unique_hosts: usize,
}

impl CommandMetricsSummary {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_executed == 0 {
            // No commands executed means no failures, consider as 100% success
            100.0
        } else {
            self.total_succeeded as f64 / self.total_executed as f64 * 100.0
        }
    }

    /// Calculate commands per second (based on average duration)
    pub fn commands_per_second(&self) -> f64 {
        if self.avg_duration_ms <= 0.0 {
            0.0
        } else {
            1000.0 / self.avg_duration_ms
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_command_metrics() {
        let metrics = CommandMetrics::new();

        // Execute a successful command
        let timer = metrics.start_command("host1", "shell");
        sleep(Duration::from_millis(10));
        metrics.record_success(timer, 100, 0);

        let summary = metrics.summary();
        assert_eq!(summary.total_executed, 1);
        assert_eq!(summary.total_succeeded, 1);
        assert_eq!(summary.total_stdout_bytes, 100);
    }

    #[test]
    fn test_failed_command() {
        let metrics = CommandMetrics::new();

        let timer = metrics.start_command("host1", "shell");
        metrics.record_failure(timer, 1, 50);

        let summary = metrics.summary();
        assert_eq!(summary.total_executed, 1);
        assert_eq!(summary.total_failed, 1);
        assert_eq!(summary.total_stderr_bytes, 50);
    }

    #[test]
    fn test_per_module_metrics() {
        let metrics = CommandMetrics::new();

        // Execute shell module 3 times
        for _ in 0..3 {
            let timer = metrics.start_command("host1", "shell");
            metrics.record_success(timer, 0, 0);
        }

        // Execute apt module once
        let timer = metrics.start_command("host1", "apt");
        metrics.record_success(timer, 0, 0);

        let shell_metrics = metrics.module_metrics("shell").unwrap();
        assert_eq!(shell_metrics.executed, 3);

        let apt_metrics = metrics.module_metrics("apt").unwrap();
        assert_eq!(apt_metrics.executed, 1);
    }
}
