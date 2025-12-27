//! Metrics collector for aggregating all metrics
//!
//! This module provides a central collector that aggregates all metrics
//! from different subsystems (connections, pool, commands) and provides
//! a unified interface for exporting metrics.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use super::command::{CommandMetrics, CommandMetricsSummary};
use super::connection::{ConnectionMetrics, ConnectionMetricsSummary};
use super::pool::{PoolMetrics, PoolMetricsSummary};

// ============================================================================
// Global Metrics Collector
// ============================================================================

/// Global metrics collector singleton
static GLOBAL_COLLECTOR: std::sync::OnceLock<Arc<MetricsCollector>> = std::sync::OnceLock::new();

/// Get the global metrics collector
pub fn global() -> Arc<MetricsCollector> {
    GLOBAL_COLLECTOR
        .get_or_init(|| Arc::new(MetricsCollector::new()))
        .clone()
}

/// Initialize the global metrics collector with custom configuration
pub fn init_global(config: MetricsConfig) -> Arc<MetricsCollector> {
    let collector = Arc::new(MetricsCollector::with_config(config));
    let _ = GLOBAL_COLLECTOR.set(collector.clone());
    collector
}

// ============================================================================
// Metrics Configuration
// ============================================================================

/// Configuration for metrics collection
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Whether metrics collection is enabled
    pub enabled: bool,
    /// Prefix for all metric names
    pub prefix: String,
    /// Additional labels to add to all metrics
    pub global_labels: std::collections::HashMap<String, String>,
    /// How often to collect metrics (for background collection)
    pub collection_interval: Duration,
    /// Whether to include per-host metrics
    pub per_host_metrics: bool,
    /// Whether to include per-module metrics
    pub per_module_metrics: bool,
    /// Maximum number of hosts to track individually
    pub max_tracked_hosts: usize,
    /// Maximum number of modules to track individually
    pub max_tracked_modules: usize,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefix: "rustible".to_string(),
            global_labels: std::collections::HashMap::new(),
            collection_interval: Duration::from_secs(15),
            per_host_metrics: true,
            per_module_metrics: true,
            max_tracked_hosts: 1000,
            max_tracked_modules: 100,
        }
    }
}

// ============================================================================
// Metrics Collector
// ============================================================================

/// Central metrics collector
#[derive(Debug)]
pub struct MetricsCollector {
    /// Configuration
    pub config: MetricsConfig,
    /// Connection metrics
    pub connection: ConnectionMetrics,
    /// Pool metrics
    pub pool: PoolMetrics,
    /// Command metrics
    pub command: CommandMetrics,
    /// Collector creation time
    created_at: Instant,
    /// Last snapshot time
    last_snapshot: RwLock<Option<Instant>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector with default configuration
    pub fn new() -> Self {
        Self::with_config(MetricsConfig::default())
    }

    /// Create a new metrics collector with custom configuration
    pub fn with_config(config: MetricsConfig) -> Self {
        Self {
            config,
            connection: ConnectionMetrics::new(),
            pool: PoolMetrics::new(),
            command: CommandMetrics::new(),
            created_at: Instant::now(),
            last_snapshot: RwLock::new(None),
        }
    }

    /// Get uptime of the collector
    pub fn uptime(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Get a full snapshot of all metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut last_snapshot = self.last_snapshot.write();
        *last_snapshot = Some(Instant::now());

        MetricsSnapshot {
            timestamp: Instant::now(),
            uptime: self.uptime(),
            connection: self.connection.summary(),
            pool: self.pool.summary(),
            command: self.command.summary(),
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.connection.reset();
        self.pool.reset();
        self.command.reset();
    }

    /// Check if metrics collection is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

// ============================================================================
// Metrics Snapshot
// ============================================================================

/// Complete snapshot of all metrics at a point in time
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    /// Snapshot timestamp
    pub timestamp: Instant,
    /// Collector uptime
    pub uptime: Duration,
    /// Connection metrics summary
    pub connection: ConnectionMetricsSummary,
    /// Pool metrics summary
    pub pool: PoolMetricsSummary,
    /// Command metrics summary
    pub command: CommandMetricsSummary,
}

impl MetricsSnapshot {
    /// Get overall health status
    pub fn health_status(&self) -> HealthStatus {
        let connection_health = self.connection.success_rate();
        let command_success = self.command.success_rate();
        let pool_healthy = self.pool.is_healthy();

        if connection_health >= 99.0 && command_success >= 99.0 && pool_healthy {
            HealthStatus::Healthy
        } else if connection_health >= 95.0 && command_success >= 95.0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        }
    }

    /// Generate a text summary
    pub fn summary_text(&self) -> String {
        format!(
            "Rustible Metrics Summary\n\
            =========================\n\
            Uptime: {:?}\n\
            \n\
            Connections:\n\
            - Total Attempts: {}\n\
            - Success Rate: {:.2}%\n\
            - Active: {}\n\
            - Avg Latency: {:.2}ms (p95: {:.2}ms, p99: {:.2}ms)\n\
            \n\
            Pool:\n\
            - Size: {} / {} (capacity)\n\
            - Utilization: {:.1}%\n\
            - Health Rate: {:.2}%\n\
            \n\
            Commands:\n\
            - Total Executed: {}\n\
            - Success Rate: {:.2}%\n\
            - In Progress: {}\n\
            - Avg Duration: {:.2}ms (p95: {:.2}ms, p99: {:.2}ms)\n",
            self.uptime,
            self.connection.total_attempts,
            self.connection.success_rate(),
            self.connection.active_connections,
            self.connection.avg_latency_ms,
            self.connection.p95_latency_ms,
            self.connection.p99_latency_ms,
            self.pool.pool_size,
            self.pool.pool_capacity,
            self.pool.utilization_percent,
            self.pool.health_success_rate(),
            self.command.total_executed,
            self.command.success_rate(),
            self.command.in_progress,
            self.command.avg_duration_ms,
            self.command.p95_duration_ms,
            self.command.p99_duration_ms,
        )
    }
}

/// Overall health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// All systems operating normally
    Healthy,
    /// Some issues but still operational
    Degraded,
    /// Significant issues affecting operation
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collector_creation() {
        let collector = MetricsCollector::new();
        assert!(collector.is_enabled());
    }

    #[test]
    fn test_snapshot() {
        let collector = MetricsCollector::new();
        let snapshot = collector.snapshot();
        assert!(snapshot.uptime.as_nanos() > 0);
    }

    #[test]
    fn test_global_collector() {
        let collector1 = global();
        let collector2 = global();
        // Should be the same instance
        assert!(Arc::ptr_eq(&collector1, &collector2));
    }

    #[test]
    fn test_health_status() {
        let collector = MetricsCollector::new();
        let snapshot = collector.snapshot();
        // With no data, should be healthy
        assert_eq!(snapshot.health_status(), HealthStatus::Healthy);
    }
}
