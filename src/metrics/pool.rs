//! Connection pool utilization metrics
//!
//! This module provides metrics for tracking connection pool utilization,
//! including pool size, availability, and efficiency metrics.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use super::types::{Counter, Gauge, Histogram};

// ============================================================================
// Pool Utilization Metrics
// ============================================================================

/// Metrics for tracking connection pool utilization
#[derive(Debug)]
pub struct PoolMetrics {
    /// Current pool size
    pub pool_size: Gauge,
    /// Maximum pool capacity
    pub pool_capacity: Gauge,
    /// Available connections in pool
    pub available_connections: Gauge,
    /// Connections currently in use
    pub in_use_connections: Gauge,
    /// Pool utilization percentage
    pub utilization: Gauge,
    /// Total connections created
    pub connections_created: Counter,
    /// Total connections destroyed
    pub connections_destroyed: Counter,
    /// Connection wait time histogram (when pool is exhausted)
    pub wait_time: Histogram,
    /// Pool exhaustion events (when no connections available)
    pub pool_exhaustions: Counter,
    /// Connection checkout count
    pub checkouts: Counter,
    /// Connection checkin count
    pub checkins: Counter,
    /// Health check passes
    pub health_check_passes: Counter,
    /// Health check failures
    pub health_check_failures: Counter,
    /// Per-host pool metrics
    per_host: RwLock<HashMap<String, HostPoolMetrics>>,
    /// Pool creation time
    created_at: Instant,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl PoolMetrics {
    /// Create new pool metrics
    pub fn new() -> Self {
        Self {
            pool_size: Gauge::new(
                "rustible_pool_size",
                "Current number of connections in pool",
            ),
            pool_capacity: Gauge::new("rustible_pool_capacity", "Maximum pool capacity"),
            available_connections: Gauge::new(
                "rustible_pool_available",
                "Number of available connections in pool",
            ),
            in_use_connections: Gauge::new(
                "rustible_pool_in_use",
                "Number of connections currently in use",
            ),
            utilization: Gauge::new(
                "rustible_pool_utilization_percent",
                "Pool utilization percentage",
            ),
            connections_created: Counter::new(
                "rustible_pool_connections_created_total",
                "Total connections created",
            ),
            connections_destroyed: Counter::new(
                "rustible_pool_connections_destroyed_total",
                "Total connections destroyed",
            ),
            wait_time: Histogram::with_buckets(
                "rustible_pool_wait_time_ms",
                "Time spent waiting for an available connection",
                &[
                    1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 5000.0,
                ],
            ),
            pool_exhaustions: Counter::new(
                "rustible_pool_exhaustions_total",
                "Number of times pool was exhausted",
            ),
            checkouts: Counter::new(
                "rustible_pool_checkouts_total",
                "Total connection checkouts",
            ),
            checkins: Counter::new("rustible_pool_checkins_total", "Total connection checkins"),
            health_check_passes: Counter::new(
                "rustible_pool_health_checks_passed_total",
                "Total health checks passed",
            ),
            health_check_failures: Counter::new(
                "rustible_pool_health_checks_failed_total",
                "Total health checks failed",
            ),
            per_host: RwLock::new(HashMap::new()),
            created_at: Instant::now(),
        }
    }

    /// Set pool capacity
    pub fn set_capacity(&self, capacity: usize) {
        self.pool_capacity.set(capacity as f64);
    }

    /// Update pool state metrics
    pub fn update_pool_state(&self, total: usize, available: usize, in_use: usize) {
        self.pool_size.set(total as f64);
        self.available_connections.set(available as f64);
        self.in_use_connections.set(in_use as f64);

        // Calculate utilization
        let capacity = self.pool_capacity.get();
        if capacity > 0.0 {
            self.utilization.set((in_use as f64 / capacity) * 100.0);
        }
    }

    /// Record connection creation
    pub fn record_connection_created(&self, host: &str) {
        self.connections_created.inc();
        self.pool_size.inc();
        self.available_connections.inc();
        self.get_or_create_host_metrics(host).created.inc();
    }

    /// Record connection destruction
    pub fn record_connection_destroyed(&self, host: &str) {
        self.connections_destroyed.inc();
        self.pool_size.dec();
        self.get_or_create_host_metrics(host).destroyed.inc();
    }

    /// Record a connection checkout (borrow from pool)
    pub fn record_checkout(&self, host: &str, wait_time: Option<Duration>) {
        self.checkouts.inc();
        self.available_connections.dec();
        self.in_use_connections.inc();

        if let Some(wait) = wait_time {
            self.wait_time.observe(wait.as_secs_f64() * 1000.0);
        }

        self.get_or_create_host_metrics(host).checkouts.inc();
        self.update_utilization();
    }

    /// Record a connection checkin (return to pool)
    pub fn record_checkin(&self, host: &str) {
        self.checkins.inc();
        self.available_connections.inc();
        self.in_use_connections.dec();
        self.get_or_create_host_metrics(host).checkins.inc();
        self.update_utilization();
    }

    /// Record pool exhaustion event
    pub fn record_pool_exhaustion(&self) {
        self.pool_exhaustions.inc();
    }

    /// Record health check result
    pub fn record_health_check(&self, host: &str, passed: bool) {
        if passed {
            self.health_check_passes.inc();
            self.get_or_create_host_metrics(host).health_passes.inc();
        } else {
            self.health_check_failures.inc();
            self.get_or_create_host_metrics(host).health_failures.inc();
        }
    }

    /// Get summary of pool metrics
    pub fn summary(&self) -> PoolMetricsSummary {
        let per_host = self.per_host.read();
        PoolMetricsSummary {
            pool_size: self.pool_size.get() as usize,
            pool_capacity: self.pool_capacity.get() as usize,
            available: self.available_connections.get() as usize,
            in_use: self.in_use_connections.get() as usize,
            utilization_percent: self.utilization.get(),
            total_created: self.connections_created.get() as u64,
            total_destroyed: self.connections_destroyed.get() as u64,
            total_checkouts: self.checkouts.get() as u64,
            total_checkins: self.checkins.get() as u64,
            exhaustion_count: self.pool_exhaustions.get() as u64,
            health_passes: self.health_check_passes.get() as u64,
            health_failures: self.health_check_failures.get() as u64,
            avg_wait_time_ms: if self.wait_time.count() > 0 {
                self.wait_time.sum() / self.wait_time.count() as f64
            } else {
                0.0
            },
            p95_wait_time_ms: self.wait_time.quantile(0.95),
            unique_hosts: per_host.len(),
            uptime: self.created_at.elapsed(),
        }
    }

    /// Get metrics for a specific host
    pub fn host_metrics(&self, host: &str) -> Option<HostPoolMetricsSnapshot> {
        self.per_host.read().get(host).map(|m| m.snapshot())
    }

    /// Get all host metrics
    pub fn all_host_metrics(&self) -> Vec<HostPoolMetricsSnapshot> {
        self.per_host
            .read()
            .values()
            .map(|m| m.snapshot())
            .collect()
    }

    /// Get or create per-host metrics
    fn get_or_create_host_metrics(&self, host: &str) -> HostPoolMetrics {
        {
            let read_guard = self.per_host.read();
            if let Some(metrics) = read_guard.get(host) {
                return metrics.clone();
            }
        }

        let mut write_guard = self.per_host.write();
        write_guard
            .entry(host.to_string())
            .or_insert_with(|| HostPoolMetrics::new(host))
            .clone()
    }

    /// Update utilization metric
    fn update_utilization(&self) {
        let capacity = self.pool_capacity.get();
        if capacity > 0.0 {
            let in_use = self.in_use_connections.get();
            self.utilization.set((in_use / capacity) * 100.0);
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.pool_size.set(0.0);
        self.available_connections.set(0.0);
        self.in_use_connections.set(0.0);
        self.utilization.set(0.0);
        self.connections_created.reset();
        self.connections_destroyed.reset();
        self.wait_time.reset();
        self.pool_exhaustions.reset();
        self.checkouts.reset();
        self.checkins.reset();
        self.health_check_passes.reset();
        self.health_check_failures.reset();
        self.per_host.write().clear();
    }
}

// ============================================================================
// Per-Host Pool Metrics
// ============================================================================

/// Pool metrics for a specific host
#[derive(Debug, Clone)]
pub struct HostPoolMetrics {
    /// Host identifier
    pub host: String,
    /// Connections created for this host
    pub created: Counter,
    /// Connections destroyed for this host
    pub destroyed: Counter,
    /// Checkouts for this host
    pub checkouts: Counter,
    /// Checkins for this host
    pub checkins: Counter,
    /// Health check passes
    pub health_passes: Counter,
    /// Health check failures
    pub health_failures: Counter,
}

impl HostPoolMetrics {
    /// Create new host pool metrics
    pub fn new(host: &str) -> Self {
        let labels = {
            let mut l = HashMap::new();
            l.insert("host".to_string(), host.to_string());
            l
        };

        Self {
            host: host.to_string(),
            created: Counter::with_labels(
                "rustible_host_pool_created_total",
                "Connections created per host",
                labels.clone(),
            ),
            destroyed: Counter::with_labels(
                "rustible_host_pool_destroyed_total",
                "Connections destroyed per host",
                labels.clone(),
            ),
            checkouts: Counter::with_labels(
                "rustible_host_pool_checkouts_total",
                "Connection checkouts per host",
                labels.clone(),
            ),
            checkins: Counter::with_labels(
                "rustible_host_pool_checkins_total",
                "Connection checkins per host",
                labels.clone(),
            ),
            health_passes: Counter::with_labels(
                "rustible_host_pool_health_passes_total",
                "Health checks passed per host",
                labels.clone(),
            ),
            health_failures: Counter::with_labels(
                "rustible_host_pool_health_failures_total",
                "Health checks failed per host",
                labels,
            ),
        }
    }

    /// Create a snapshot of current metrics
    pub fn snapshot(&self) -> HostPoolMetricsSnapshot {
        HostPoolMetricsSnapshot {
            host: self.host.clone(),
            created: self.created.get() as u64,
            destroyed: self.destroyed.get() as u64,
            checkouts: self.checkouts.get() as u64,
            checkins: self.checkins.get() as u64,
            health_passes: self.health_passes.get() as u64,
            health_failures: self.health_failures.get() as u64,
            active: (self.checkouts.get() - self.checkins.get()) as i64,
        }
    }
}

/// Snapshot of host pool metrics
#[derive(Debug, Clone)]
pub struct HostPoolMetricsSnapshot {
    /// Host identifier
    pub host: String,
    /// Connections created
    pub created: u64,
    /// Connections destroyed
    pub destroyed: u64,
    /// Total checkouts
    pub checkouts: u64,
    /// Total checkins
    pub checkins: u64,
    /// Health check passes
    pub health_passes: u64,
    /// Health check failures
    pub health_failures: u64,
    /// Currently active (checked out) connections
    pub active: i64,
}

// ============================================================================
// Summary Types
// ============================================================================

/// Summary of pool metrics
#[derive(Debug, Clone)]
pub struct PoolMetricsSummary {
    /// Current pool size
    pub pool_size: usize,
    /// Maximum pool capacity
    pub pool_capacity: usize,
    /// Available connections
    pub available: usize,
    /// Connections in use
    pub in_use: usize,
    /// Utilization percentage
    pub utilization_percent: f64,
    /// Total connections created
    pub total_created: u64,
    /// Total connections destroyed
    pub total_destroyed: u64,
    /// Total checkouts
    pub total_checkouts: u64,
    /// Total checkins
    pub total_checkins: u64,
    /// Pool exhaustion count
    pub exhaustion_count: u64,
    /// Health check passes
    pub health_passes: u64,
    /// Health check failures
    pub health_failures: u64,
    /// Average wait time in milliseconds
    pub avg_wait_time_ms: f64,
    /// P95 wait time in milliseconds
    pub p95_wait_time_ms: f64,
    /// Number of unique hosts
    pub unique_hosts: usize,
    /// Pool uptime
    pub uptime: Duration,
}

impl PoolMetricsSummary {
    /// Calculate connection churn rate (creates + destroys per hour)
    pub fn churn_rate_per_hour(&self) -> f64 {
        let hours = self.uptime.as_secs_f64() / 3600.0;
        if hours > 0.0 {
            (self.total_created + self.total_destroyed) as f64 / hours
        } else {
            0.0
        }
    }

    /// Calculate health check success rate
    pub fn health_success_rate(&self) -> f64 {
        let total = self.health_passes + self.health_failures;
        if total == 0 {
            100.0
        } else {
            self.health_passes as f64 / total as f64 * 100.0
        }
    }

    /// Check if pool is healthy
    pub fn is_healthy(&self) -> bool {
        // Pool is healthy if:
        // - Utilization is under 90%
        // - Health success rate is above 95%
        // - No recent exhaustions
        self.utilization_percent < 90.0 && self.health_success_rate() > 95.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_metrics_basic() {
        let metrics = PoolMetrics::new();
        metrics.set_capacity(10);

        // Create some connections
        metrics.record_connection_created("host1");
        metrics.record_connection_created("host2");

        let summary = metrics.summary();
        assert_eq!(summary.total_created, 2);
        assert_eq!(summary.pool_size, 2);
    }

    #[test]
    fn test_checkout_checkin() {
        let metrics = PoolMetrics::new();
        metrics.set_capacity(10);
        metrics.pool_size.set(5.0);
        metrics.available_connections.set(5.0);

        metrics.record_checkout("host1", None);
        assert_eq!(metrics.in_use_connections.get(), 1.0);
        assert_eq!(metrics.available_connections.get(), 4.0);

        metrics.record_checkin("host1");
        assert_eq!(metrics.in_use_connections.get(), 0.0);
        assert_eq!(metrics.available_connections.get(), 5.0);
    }

    #[test]
    fn test_utilization() {
        let metrics = PoolMetrics::new();
        metrics.set_capacity(10);
        metrics.pool_size.set(10.0);
        metrics.available_connections.set(10.0);

        for _ in 0..5 {
            metrics.record_checkout("host1", None);
        }

        assert!((metrics.utilization.get() - 50.0).abs() < 1.0);
    }
}
