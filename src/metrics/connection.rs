//! Connection latency metrics
//!
//! This module provides metrics for tracking SSH connection latency,
//! connection establishment time, and per-host connection statistics.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use super::types::{Counter, Gauge, Histogram, LATENCY_BUCKETS_MS};

// Type alias for per-host metrics stored in Arc for shared ownership
type PerHostMetrics = Arc<HostConnectionMetrics>;

// ============================================================================
// Connection Latency Tracker
// ============================================================================

/// Tracks connection latency metrics per host
#[derive(Debug)]
pub struct ConnectionMetrics {
    /// Connection establishment latency histogram (milliseconds)
    pub connection_latency: Histogram,
    /// Total connection attempts
    pub connection_attempts: Counter,
    /// Successful connections
    pub connection_successes: Counter,
    /// Failed connections
    pub connection_failures: Counter,
    /// Current active connections gauge
    pub active_connections: Gauge,
    /// Connection reuse count
    pub connection_reuses: Counter,
    /// Per-host metrics
    per_host: RwLock<HashMap<String, PerHostMetrics>>,
}

impl Default for ConnectionMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionMetrics {
    /// Create new connection metrics
    pub fn new() -> Self {
        Self {
            connection_latency: Histogram::with_buckets(
                "rustible_connection_latency_ms",
                "SSH connection establishment latency in milliseconds",
                LATENCY_BUCKETS_MS,
            ),
            connection_attempts: Counter::new(
                "rustible_connection_attempts_total",
                "Total number of SSH connection attempts",
            ),
            connection_successes: Counter::new(
                "rustible_connection_successes_total",
                "Total number of successful SSH connections",
            ),
            connection_failures: Counter::new(
                "rustible_connection_failures_total",
                "Total number of failed SSH connections",
            ),
            active_connections: Gauge::new(
                "rustible_active_connections",
                "Current number of active SSH connections",
            ),
            connection_reuses: Counter::new(
                "rustible_connection_reuses_total",
                "Total number of connection reuses from pool",
            ),
            per_host: RwLock::new(HashMap::new()),
        }
    }

    /// Record a connection attempt starting
    pub fn start_connection(&self, host: &str) -> ConnectionTimer {
        self.connection_attempts.inc();
        self.get_or_create_host_metrics(host).attempts.inc();
        ConnectionTimer::new(host.to_string())
    }

    /// Record a successful connection
    pub fn record_connection_success(&self, timer: ConnectionTimer) {
        let duration = timer.elapsed();
        let host = timer.host.clone();

        // Record global metrics
        self.connection_latency.observe(duration.as_secs_f64() * 1000.0);
        self.connection_successes.inc();
        self.active_connections.inc();

        // Record per-host metrics
        let host_metrics = self.get_or_create_host_metrics(&host);
        host_metrics.latency.observe(duration.as_secs_f64() * 1000.0);
        host_metrics.successes.inc();
        host_metrics.active.inc();
    }

    /// Record a failed connection
    pub fn record_connection_failure(&self, timer: ConnectionTimer, error: &str) {
        let host = timer.host.clone();

        // Record global metrics
        self.connection_failures.inc();

        // Record per-host metrics
        let host_metrics = self.get_or_create_host_metrics(&host);
        host_metrics.failures.inc();
        host_metrics.last_error.write().replace(error.to_string());
    }

    /// Record a connection being closed
    pub fn record_connection_closed(&self, host: &str) {
        self.active_connections.dec();
        self.get_or_create_host_metrics(host).active.dec();
    }

    /// Record a connection reuse from pool
    pub fn record_connection_reuse(&self, host: &str) {
        self.connection_reuses.inc();
        self.get_or_create_host_metrics(host).reuses.inc();
    }

    /// Get metrics for a specific host
    pub fn host_metrics(&self, host: &str) -> Option<HostConnectionMetricsSnapshot> {
        self.per_host.read().get(host).map(|m| m.snapshot())
    }

    /// Get all host metrics
    pub fn all_host_metrics(&self) -> Vec<HostConnectionMetricsSnapshot> {
        self.per_host.read().values().map(|m| m.snapshot()).collect()
    }

    /// Get or create per-host metrics
    fn get_or_create_host_metrics(&self, host: &str) -> PerHostMetrics {
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
                .or_insert_with(|| Arc::new(HostConnectionMetrics::new(host))),
        )
    }

    /// Get summary statistics
    pub fn summary(&self) -> ConnectionMetricsSummary {
        let per_host = self.per_host.read();
        ConnectionMetricsSummary {
            total_attempts: self.connection_attempts.get() as u64,
            total_successes: self.connection_successes.get() as u64,
            total_failures: self.connection_failures.get() as u64,
            active_connections: self.active_connections.get() as u64,
            total_reuses: self.connection_reuses.get() as u64,
            avg_latency_ms: if self.connection_latency.count() > 0 {
                self.connection_latency.sum() / self.connection_latency.count() as f64
            } else {
                0.0
            },
            p50_latency_ms: self.connection_latency.quantile(0.5),
            p95_latency_ms: self.connection_latency.quantile(0.95),
            p99_latency_ms: self.connection_latency.quantile(0.99),
            unique_hosts: per_host.len(),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.connection_latency.reset();
        self.connection_attempts.reset();
        self.connection_successes.reset();
        self.connection_failures.reset();
        self.active_connections.set(0.0);
        self.connection_reuses.reset();
        self.per_host.write().clear();
    }
}

// ============================================================================
// Per-Host Metrics
// ============================================================================

/// Metrics for a specific host
#[derive(Debug, Clone)]
pub struct HostConnectionMetrics {
    /// Host identifier
    pub host: String,
    /// Connection latency histogram
    pub latency: Histogram,
    /// Connection attempts
    pub attempts: Counter,
    /// Successful connections
    pub successes: Counter,
    /// Failed connections
    pub failures: Counter,
    /// Active connections
    pub active: Gauge,
    /// Connection reuses
    pub reuses: Counter,
    /// Last error message
    pub last_error: Arc<RwLock<Option<String>>>,
}

impl HostConnectionMetrics {
    /// Create new host metrics
    pub fn new(host: &str) -> Self {
        let labels = {
            let mut l = HashMap::new();
            l.insert("host".to_string(), host.to_string());
            l
        };

        Self {
            host: host.to_string(),
            latency: Histogram::with_labels(
                "rustible_host_connection_latency_ms",
                "SSH connection latency per host in milliseconds",
                LATENCY_BUCKETS_MS,
                labels.clone(),
            ),
            attempts: Counter::with_labels(
                "rustible_host_connection_attempts_total",
                "Connection attempts per host",
                labels.clone(),
            ),
            successes: Counter::with_labels(
                "rustible_host_connection_successes_total",
                "Successful connections per host",
                labels.clone(),
            ),
            failures: Counter::with_labels(
                "rustible_host_connection_failures_total",
                "Failed connections per host",
                labels.clone(),
            ),
            active: Gauge::with_labels(
                "rustible_host_active_connections",
                "Active connections per host",
                labels.clone(),
            ),
            reuses: Counter::with_labels(
                "rustible_host_connection_reuses_total",
                "Connection reuses per host",
                labels,
            ),
            last_error: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a snapshot of current metrics
    pub fn snapshot(&self) -> HostConnectionMetricsSnapshot {
        HostConnectionMetricsSnapshot {
            host: self.host.clone(),
            attempts: self.attempts.get() as u64,
            successes: self.successes.get() as u64,
            failures: self.failures.get() as u64,
            active: self.active.get() as u64,
            reuses: self.reuses.get() as u64,
            avg_latency_ms: if self.latency.count() > 0 {
                self.latency.sum() / self.latency.count() as f64
            } else {
                0.0
            },
            p50_latency_ms: self.latency.quantile(0.5),
            p95_latency_ms: self.latency.quantile(0.95),
            p99_latency_ms: self.latency.quantile(0.99),
            last_error: self.last_error.read().clone(),
        }
    }
}

/// Snapshot of host connection metrics
#[derive(Debug, Clone)]
pub struct HostConnectionMetricsSnapshot {
    /// Host identifier
    pub host: String,
    /// Total connection attempts
    pub attempts: u64,
    /// Successful connections
    pub successes: u64,
    /// Failed connections
    pub failures: u64,
    /// Current active connections
    pub active: u64,
    /// Connection reuses
    pub reuses: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// P50 latency in milliseconds
    pub p50_latency_ms: f64,
    /// P95 latency in milliseconds
    pub p95_latency_ms: f64,
    /// P99 latency in milliseconds
    pub p99_latency_ms: f64,
    /// Last error message
    pub last_error: Option<String>,
}

// ============================================================================
// Connection Timer
// ============================================================================

/// Timer for measuring connection establishment time
#[derive(Debug)]
pub struct ConnectionTimer {
    /// Host being connected to
    pub host: String,
    /// Start time
    start: Instant,
}

impl ConnectionTimer {
    /// Create a new connection timer
    pub fn new(host: String) -> Self {
        Self {
            host,
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

/// Summary of connection metrics
#[derive(Debug, Clone)]
pub struct ConnectionMetricsSummary {
    /// Total connection attempts
    pub total_attempts: u64,
    /// Total successful connections
    pub total_successes: u64,
    /// Total failed connections
    pub total_failures: u64,
    /// Current active connections
    pub active_connections: u64,
    /// Total connection reuses
    pub total_reuses: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// P50 latency in milliseconds
    pub p50_latency_ms: f64,
    /// P95 latency in milliseconds
    pub p95_latency_ms: f64,
    /// P99 latency in milliseconds
    pub p99_latency_ms: f64,
    /// Number of unique hosts
    pub unique_hosts: usize,
}

impl ConnectionMetricsSummary {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_attempts == 0 {
            // No attempts means no failures, consider as 100% success
            100.0
        } else {
            self.total_successes as f64 / self.total_attempts as f64 * 100.0
        }
    }

    /// Calculate cache hit rate (reuse rate)
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.total_successes + self.total_reuses;
        if total == 0 {
            0.0
        } else {
            self.total_reuses as f64 / total as f64 * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_connection_metrics() {
        let metrics = ConnectionMetrics::new();

        // Simulate a successful connection
        let timer = metrics.start_connection("host1.example.com");
        sleep(Duration::from_millis(10));
        metrics.record_connection_success(timer);

        let summary = metrics.summary();
        assert_eq!(summary.total_attempts, 1);
        assert_eq!(summary.total_successes, 1);
        assert_eq!(summary.active_connections, 1);
    }

    #[test]
    fn test_connection_failure() {
        let metrics = ConnectionMetrics::new();

        let timer = metrics.start_connection("host2.example.com");
        metrics.record_connection_failure(timer, "Connection refused");

        let summary = metrics.summary();
        assert_eq!(summary.total_attempts, 1);
        assert_eq!(summary.total_failures, 1);
        assert_eq!(summary.active_connections, 0);
    }

    #[test]
    fn test_per_host_metrics() {
        let metrics = ConnectionMetrics::new();

        // Connect to host1 twice
        for _ in 0..2 {
            let timer = metrics.start_connection("host1.example.com");
            metrics.record_connection_success(timer);
        }

        // Connect to host2 once
        let timer = metrics.start_connection("host2.example.com");
        metrics.record_connection_success(timer);

        let host1_metrics = metrics.host_metrics("host1.example.com").unwrap();
        assert_eq!(host1_metrics.successes, 2);

        let host2_metrics = metrics.host_metrics("host2.example.com").unwrap();
        assert_eq!(host2_metrics.successes, 1);

        assert_eq!(metrics.summary().unique_hosts, 2);
    }
}
