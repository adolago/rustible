//! Metric type definitions for the metrics module
//!
//! This module provides the core metric types used throughout the metrics system:
//! - Counter: Monotonically increasing value
//! - Gauge: Value that can go up and down
//! - Histogram: Distribution of values with configurable buckets
//! - Summary: Statistical summary with quantiles

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

// ============================================================================
// Metric Labels
// ============================================================================

/// Labels attached to a metric for dimensional data
pub type Labels = HashMap<String, String>;

/// Helper to create labels from key-value pairs
#[macro_export]
macro_rules! labels {
    () => {
        std::collections::HashMap::new()
    };
    ($($key:expr => $value:expr),+ $(,)?) => {{
        let mut map = std::collections::HashMap::new();
        $(map.insert($key.to_string(), $value.to_string());)+
        map
    }};
}

/// Trait for types that can provide labels
pub trait Labeled {
    fn labels(&self) -> Labels;
}

// ============================================================================
// Counter
// ============================================================================

/// A monotonically increasing counter
///
/// Counters are used for values that only ever increase, such as
/// request counts or bytes transferred.
#[derive(Debug)]
pub struct Counter {
    /// Counter value (stored as fixed-point with 3 decimal places)
    value: AtomicU64,
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Labels attached to this counter
    labels: Labels,
}

impl Counter {
    /// Create a new counter
    pub fn new(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self {
            value: AtomicU64::new(0),
            name: name.into(),
            help: help.into(),
            labels: HashMap::new(),
        }
    }

    /// Create a counter with labels
    pub fn with_labels(name: impl Into<String>, help: impl Into<String>, labels: Labels) -> Self {
        Self {
            value: AtomicU64::new(0),
            name: name.into(),
            help: help.into(),
            labels,
        }
    }

    /// Increment the counter by 1
    pub fn inc(&self) {
        self.value.fetch_add(1000, Ordering::Relaxed);
    }

    /// Increment the counter by a specific amount
    pub fn inc_by(&self, amount: f64) {
        let fixed = (amount * 1000.0) as u64;
        self.value.fetch_add(fixed, Ordering::Relaxed);
    }

    /// Get the current value
    pub fn get(&self) -> f64 {
        self.value.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get the metric name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the help text
    pub fn help(&self) -> &str {
        &self.help
    }

    /// Get the labels
    pub fn labels(&self) -> &Labels {
        &self.labels
    }

    /// Reset the counter (mainly for testing)
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

impl Clone for Counter {
    fn clone(&self) -> Self {
        Self {
            value: AtomicU64::new(self.value.load(Ordering::Relaxed)),
            name: self.name.clone(),
            help: self.help.clone(),
            labels: self.labels.clone(),
        }
    }
}

// ============================================================================
// Gauge
// ============================================================================

/// A gauge that can go up and down
///
/// Gauges are used for values that can increase or decrease,
/// such as current connections or memory usage.
#[derive(Debug)]
pub struct Gauge {
    /// Gauge value (stored as fixed-point with 3 decimal places)
    value: AtomicU64,
    /// Whether the value is negative
    negative: std::sync::atomic::AtomicBool,
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Labels attached to this gauge
    labels: Labels,
}

impl Gauge {
    /// Create a new gauge
    pub fn new(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self {
            value: AtomicU64::new(0),
            negative: std::sync::atomic::AtomicBool::new(false),
            name: name.into(),
            help: help.into(),
            labels: HashMap::new(),
        }
    }

    /// Create a gauge with labels
    pub fn with_labels(name: impl Into<String>, help: impl Into<String>, labels: Labels) -> Self {
        Self {
            value: AtomicU64::new(0),
            negative: std::sync::atomic::AtomicBool::new(false),
            name: name.into(),
            help: help.into(),
            labels,
        }
    }

    /// Set the gauge to a specific value
    pub fn set(&self, value: f64) {
        let is_negative = value < 0.0;
        let abs_value = value.abs();
        let fixed = (abs_value * 1000.0) as u64;
        self.value.store(fixed, Ordering::Relaxed);
        self.negative.store(is_negative, Ordering::Relaxed);
    }

    /// Increment the gauge by 1
    pub fn inc(&self) {
        self.value.fetch_add(1000, Ordering::Relaxed);
    }

    /// Increment the gauge by a specific amount
    pub fn inc_by(&self, amount: f64) {
        let fixed = (amount * 1000.0) as u64;
        self.value.fetch_add(fixed, Ordering::Relaxed);
    }

    /// Decrement the gauge by 1
    pub fn dec(&self) {
        // Simple decrement - in practice would need more sophisticated handling
        let current = self.value.load(Ordering::Relaxed);
        if current >= 1000 {
            self.value.fetch_sub(1000, Ordering::Relaxed);
        } else {
            self.value.store(0, Ordering::Relaxed);
        }
    }

    /// Decrement the gauge by a specific amount
    pub fn dec_by(&self, amount: f64) {
        let fixed = (amount * 1000.0) as u64;
        let current = self.value.load(Ordering::Relaxed);
        if current >= fixed {
            self.value.fetch_sub(fixed, Ordering::Relaxed);
        } else {
            self.value.store(0, Ordering::Relaxed);
        }
    }

    /// Get the current value
    pub fn get(&self) -> f64 {
        let value = self.value.load(Ordering::Relaxed) as f64 / 1000.0;
        if self.negative.load(Ordering::Relaxed) {
            -value
        } else {
            value
        }
    }

    /// Get the metric name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the help text
    pub fn help(&self) -> &str {
        &self.help
    }

    /// Get the labels
    pub fn labels(&self) -> &Labels {
        &self.labels
    }
}

impl Clone for Gauge {
    fn clone(&self) -> Self {
        Self {
            value: AtomicU64::new(self.value.load(Ordering::Relaxed)),
            negative: std::sync::atomic::AtomicBool::new(self.negative.load(Ordering::Relaxed)),
            name: self.name.clone(),
            help: self.help.clone(),
            labels: self.labels.clone(),
        }
    }
}

// ============================================================================
// Histogram
// ============================================================================

/// Default histogram buckets (in seconds)
pub const DEFAULT_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Latency buckets optimized for connection timing (in milliseconds)
pub const LATENCY_BUCKETS_MS: &[f64] = &[
    1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
];

/// A histogram for tracking distributions of values
///
/// Histograms are used for measuring distributions, such as
/// request latencies or response sizes.
#[derive(Debug)]
pub struct Histogram {
    /// Bucket boundaries
    buckets: Vec<f64>,
    /// Counts per bucket (atomic for thread safety)
    counts: Vec<AtomicU64>,
    /// Sum of all observed values
    sum: AtomicU64,
    /// Total count of observations
    count: AtomicU64,
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Labels attached to this histogram
    labels: Labels,
}

impl Histogram {
    /// Create a new histogram with default buckets
    pub fn new(name: impl Into<String>, help: impl Into<String>) -> Self {
        Self::with_buckets(name, help, DEFAULT_BUCKETS)
    }

    /// Create a histogram with custom buckets
    pub fn with_buckets(name: impl Into<String>, help: impl Into<String>, buckets: &[f64]) -> Self {
        let counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            buckets: buckets.to_vec(),
            counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
            name: name.into(),
            help: help.into(),
            labels: HashMap::new(),
        }
    }

    /// Create a histogram with labels
    pub fn with_labels(
        name: impl Into<String>,
        help: impl Into<String>,
        buckets: &[f64],
        labels: Labels,
    ) -> Self {
        let counts = buckets.iter().map(|_| AtomicU64::new(0)).collect();
        Self {
            buckets: buckets.to_vec(),
            counts,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
            name: name.into(),
            help: help.into(),
            labels,
        }
    }

    /// Observe a value
    pub fn observe(&self, value: f64) {
        // Update sum and count
        let fixed = (value * 1000.0) as u64;
        self.sum.fetch_add(fixed, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);

        // Find the bucket and increment
        for (i, bucket) in self.buckets.iter().enumerate() {
            if value <= *bucket {
                self.counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Observe a duration (converts to seconds)
    pub fn observe_duration(&self, duration: Duration) {
        self.observe(duration.as_secs_f64());
    }

    /// Get the sum of all observed values
    pub fn sum(&self) -> f64 {
        self.sum.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// Get the count of observations
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get the bucket boundaries
    pub fn buckets(&self) -> &[f64] {
        &self.buckets
    }

    /// Get the cumulative count for each bucket
    pub fn bucket_counts(&self) -> Vec<u64> {
        self.counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .collect()
    }

    /// Get the metric name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the help text
    pub fn help(&self) -> &str {
        &self.help
    }

    /// Get the labels
    pub fn labels(&self) -> &Labels {
        &self.labels
    }

    /// Calculate approximate quantile (e.g., p50, p95, p99)
    pub fn quantile(&self, q: f64) -> f64 {
        let total = self.count.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }

        let target = (total as f64 * q) as u64;
        let counts = self.bucket_counts();

        for (i, count) in counts.iter().enumerate() {
            if *count >= target {
                return self.buckets[i];
            }
        }

        // Return the last bucket if we didn't find it
        *self.buckets.last().unwrap_or(&0.0)
    }

    /// Reset the histogram (mainly for testing)
    pub fn reset(&self) {
        self.sum.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
        for count in &self.counts {
            count.store(0, Ordering::Relaxed);
        }
    }
}

impl Clone for Histogram {
    fn clone(&self) -> Self {
        Self {
            buckets: self.buckets.clone(),
            counts: self
                .counts
                .iter()
                .map(|c| AtomicU64::new(c.load(Ordering::Relaxed)))
                .collect(),
            sum: AtomicU64::new(self.sum.load(Ordering::Relaxed)),
            count: AtomicU64::new(self.count.load(Ordering::Relaxed)),
            name: self.name.clone(),
            help: self.help.clone(),
            labels: self.labels.clone(),
        }
    }
}

// ============================================================================
// Timer Guard for automatic timing
// ============================================================================

/// A guard that automatically records duration when dropped
pub struct TimerGuard<'a> {
    histogram: &'a Histogram,
    start: Instant,
}

impl<'a> TimerGuard<'a> {
    /// Create a new timer guard
    pub fn new(histogram: &'a Histogram) -> Self {
        Self {
            histogram,
            start: Instant::now(),
        }
    }
}

impl<'a> Drop for TimerGuard<'a> {
    fn drop(&mut self) {
        self.histogram.observe_duration(self.start.elapsed());
    }
}

impl Histogram {
    /// Start a timer that records when dropped
    pub fn start_timer(&self) -> TimerGuard<'_> {
        TimerGuard::new(self)
    }
}

// ============================================================================
// Metric Snapshot for serialization
// ============================================================================

/// Serializable snapshot of a counter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterSnapshot {
    pub name: String,
    pub help: String,
    pub labels: Labels,
    pub value: f64,
}

/// Serializable snapshot of a gauge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaugeSnapshot {
    pub name: String,
    pub help: String,
    pub labels: Labels,
    pub value: f64,
}

/// Serializable snapshot of a histogram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramSnapshot {
    pub name: String,
    pub help: String,
    pub labels: Labels,
    pub buckets: Vec<f64>,
    pub counts: Vec<u64>,
    pub sum: f64,
    pub count: u64,
}

impl From<&Counter> for CounterSnapshot {
    fn from(counter: &Counter) -> Self {
        Self {
            name: counter.name.clone(),
            help: counter.help.clone(),
            labels: counter.labels.clone(),
            value: counter.get(),
        }
    }
}

impl From<&Gauge> for GaugeSnapshot {
    fn from(gauge: &Gauge) -> Self {
        Self {
            name: gauge.name.clone(),
            help: gauge.help.clone(),
            labels: gauge.labels.clone(),
            value: gauge.get(),
        }
    }
}

impl From<&Histogram> for HistogramSnapshot {
    fn from(histogram: &Histogram) -> Self {
        Self {
            name: histogram.name.clone(),
            help: histogram.help.clone(),
            labels: histogram.labels.clone(),
            buckets: histogram.buckets.clone(),
            counts: histogram.bucket_counts(),
            sum: histogram.sum(),
            count: histogram.count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_basic() {
        let counter = Counter::new("test_counter", "A test counter");
        assert_eq!(counter.get(), 0.0);

        counter.inc();
        assert_eq!(counter.get(), 1.0);

        counter.inc_by(2.5);
        assert!((counter.get() - 3.5).abs() < 0.01);
    }

    #[test]
    fn test_gauge_basic() {
        let gauge = Gauge::new("test_gauge", "A test gauge");
        assert_eq!(gauge.get(), 0.0);

        gauge.set(10.0);
        assert_eq!(gauge.get(), 10.0);

        gauge.inc();
        assert_eq!(gauge.get(), 11.0);

        gauge.dec();
        assert_eq!(gauge.get(), 10.0);
    }

    #[test]
    fn test_histogram_basic() {
        let histogram =
            Histogram::with_buckets("test_histogram", "A test histogram", &[1.0, 5.0, 10.0]);

        histogram.observe(0.5);
        histogram.observe(3.0);
        histogram.observe(7.0);

        assert_eq!(histogram.count(), 3);
        assert!((histogram.sum() - 10.5).abs() < 0.01);
    }

    #[test]
    fn test_histogram_buckets() {
        let histogram =
            Histogram::with_buckets("test_histogram", "A test histogram", &[1.0, 5.0, 10.0]);

        histogram.observe(0.5); // <= 1.0, <= 5.0, <= 10.0
        histogram.observe(3.0); // <= 5.0, <= 10.0
        histogram.observe(7.0); // <= 10.0
        histogram.observe(15.0); // > 10.0

        let counts = histogram.bucket_counts();
        assert_eq!(counts, vec![1, 2, 3]);
    }
}
