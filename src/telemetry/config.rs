//! Telemetry configuration types.
//!
//! This module provides configuration structures for all telemetry components
//! including tracing, logging, and metrics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Main telemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    /// Enable telemetry globally
    pub enabled: bool,

    /// Service name for tracing/metrics identification
    pub service_name: String,

    /// Service version (defaults to crate version)
    pub service_version: Option<String>,

    /// Service instance ID (for distinguishing multiple instances)
    pub instance_id: Option<String>,

    /// Environment (e.g., production, staging, development)
    pub environment: String,

    /// Tracing configuration
    pub tracing: TracingConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Metrics configuration
    pub metrics: MetricsConfig,

    /// Resource attributes to attach to all telemetry data
    #[serde(default)]
    pub resource_attributes: HashMap<String, String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service_name: "rustible".to_string(),
            service_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            instance_id: None,
            environment: "development".to_string(),
            tracing: TracingConfig::default(),
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            resource_attributes: HashMap::new(),
        }
    }
}

impl TelemetryConfig {
    /// Create a new telemetry configuration with the given service name.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            ..Default::default()
        }
    }

    /// Create a production-ready configuration.
    pub fn production() -> Self {
        Self {
            enabled: true,
            service_name: "rustible".to_string(),
            service_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            instance_id: None,
            environment: "production".to_string(),
            tracing: TracingConfig::production(),
            logging: LoggingConfig::production(),
            metrics: MetricsConfig::production(),
            resource_attributes: HashMap::new(),
        }
    }

    /// Create a development configuration with verbose output.
    pub fn development() -> Self {
        Self {
            enabled: true,
            service_name: "rustible".to_string(),
            service_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            instance_id: None,
            environment: "development".to_string(),
            tracing: TracingConfig::development(),
            logging: LoggingConfig::development(),
            metrics: MetricsConfig::default(),
            resource_attributes: HashMap::new(),
        }
    }

    /// Set the service name.
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.service_name = name.into();
        self
    }

    /// Set the environment.
    pub fn with_environment(mut self, env: impl Into<String>) -> Self {
        self.environment = env.into();
        self
    }

    /// Add a resource attribute.
    pub fn with_resource_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.resource_attributes.insert(key.into(), value.into());
        self
    }

    /// Set tracing configuration.
    pub fn with_tracing(mut self, config: TracingConfig) -> Self {
        self.tracing = config;
        self
    }

    /// Set logging configuration.
    pub fn with_logging(mut self, config: LoggingConfig) -> Self {
        self.logging = config;
        self
    }

    /// Set metrics configuration.
    pub fn with_metrics(mut self, config: MetricsConfig) -> Self {
        self.metrics = config;
        self
    }
}

/// Tracing/distributed tracing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TracingConfig {
    /// Enable distributed tracing
    pub enabled: bool,

    /// Exporter type (otlp, jaeger, zipkin, stdout)
    pub exporter: TracingExporter,

    /// OTLP endpoint URL (for OTLP exporter)
    pub otlp_endpoint: Option<String>,

    /// Jaeger agent endpoint (for Jaeger exporter)
    pub jaeger_endpoint: Option<String>,

    /// Sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,

    /// Batch export configuration
    pub batch: BatchConfig,

    /// Propagation format (w3c, b3, jaeger)
    pub propagation_format: PropagationFormat,

    /// Maximum attributes per span
    pub max_attributes_per_span: u32,

    /// Maximum events per span
    pub max_events_per_span: u32,

    /// Maximum links per span
    pub max_links_per_span: u32,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            exporter: TracingExporter::Stdout,
            otlp_endpoint: None,
            jaeger_endpoint: None,
            sampling_ratio: 1.0,
            batch: BatchConfig::default(),
            propagation_format: PropagationFormat::W3C,
            max_attributes_per_span: 128,
            max_events_per_span: 128,
            max_links_per_span: 128,
        }
    }
}

impl TracingConfig {
    /// Create a production-ready tracing configuration.
    pub fn production() -> Self {
        Self {
            enabled: true,
            exporter: TracingExporter::Otlp,
            otlp_endpoint: Some("http://localhost:4317".to_string()),
            jaeger_endpoint: None,
            sampling_ratio: 0.1, // Sample 10% in production
            batch: BatchConfig::production(),
            propagation_format: PropagationFormat::W3C,
            max_attributes_per_span: 128,
            max_events_per_span: 128,
            max_links_per_span: 128,
        }
    }

    /// Create a development tracing configuration.
    pub fn development() -> Self {
        Self {
            enabled: true,
            exporter: TracingExporter::Stdout,
            otlp_endpoint: None,
            jaeger_endpoint: None,
            sampling_ratio: 1.0, // Sample everything in dev
            batch: BatchConfig::default(),
            propagation_format: PropagationFormat::W3C,
            max_attributes_per_span: 128,
            max_events_per_span: 128,
            max_links_per_span: 128,
        }
    }

    /// Set the OTLP endpoint.
    pub fn with_otlp_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.exporter = TracingExporter::Otlp;
        self.otlp_endpoint = Some(endpoint.into());
        self
    }

    /// Set the Jaeger endpoint.
    pub fn with_jaeger_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.exporter = TracingExporter::Jaeger;
        self.jaeger_endpoint = Some(endpoint.into());
        self
    }

    /// Set the sampling ratio.
    pub fn with_sampling_ratio(mut self, ratio: f64) -> Self {
        self.sampling_ratio = ratio.clamp(0.0, 1.0);
        self
    }
}

/// Tracing exporter type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TracingExporter {
    /// No tracing export
    None,
    /// Print traces to stdout (for development)
    Stdout,
    /// OpenTelemetry Protocol (OTLP) exporter
    Otlp,
    /// Jaeger exporter
    Jaeger,
    /// Zipkin exporter
    Zipkin,
}

/// Trace context propagation format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PropagationFormat {
    /// W3C Trace Context
    W3C,
    /// B3 (Zipkin) format
    B3,
    /// Jaeger format
    Jaeger,
}

/// Batch export configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BatchConfig {
    /// Maximum queue size
    pub max_queue_size: usize,

    /// Maximum export batch size
    pub max_export_batch_size: usize,

    /// Scheduled delay between exports
    #[serde(with = "humantime_serde")]
    pub scheduled_delay: Duration,

    /// Maximum time to wait for export
    #[serde(with = "humantime_serde")]
    pub max_export_timeout: Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 2048,
            max_export_batch_size: 512,
            scheduled_delay: Duration::from_secs(5),
            max_export_timeout: Duration::from_secs(30),
        }
    }
}

impl BatchConfig {
    /// Create a production batch configuration.
    pub fn production() -> Self {
        Self {
            max_queue_size: 4096,
            max_export_batch_size: 1024,
            scheduled_delay: Duration::from_secs(1),
            max_export_timeout: Duration::from_secs(10),
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Enable structured logging
    pub enabled: bool,

    /// Log level filter
    pub level: LogLevel,

    /// Log format
    pub format: LogFormat,

    /// Include span information in logs
    pub with_spans: bool,

    /// Include target in logs
    pub with_target: bool,

    /// Include file/line information
    pub with_file: bool,

    /// Include thread information
    pub with_thread_ids: bool,

    /// Include thread names
    pub with_thread_names: bool,

    /// Include ANSI colors (for console output)
    pub ansi_colors: bool,

    /// Log file path (None for stdout)
    pub file: Option<PathBuf>,

    /// Log rotation configuration
    pub rotation: Option<LogRotation>,

    /// Filter directives (e.g., "rustible=debug,hyper=warn")
    pub filter: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: LogLevel::Info,
            format: LogFormat::Pretty,
            with_spans: true,
            with_target: true,
            with_file: false,
            with_thread_ids: false,
            with_thread_names: false,
            ansi_colors: true,
            file: None,
            rotation: None,
            filter: None,
        }
    }
}

impl LoggingConfig {
    /// Create a production logging configuration.
    pub fn production() -> Self {
        Self {
            enabled: true,
            level: LogLevel::Info,
            format: LogFormat::Json,
            with_spans: true,
            with_target: true,
            with_file: false,
            with_thread_ids: false,
            with_thread_names: false,
            ansi_colors: false,
            file: None,
            rotation: None,
            filter: Some("rustible=info,warn".to_string()),
        }
    }

    /// Create a development logging configuration.
    pub fn development() -> Self {
        Self {
            enabled: true,
            level: LogLevel::Debug,
            format: LogFormat::Pretty,
            with_spans: true,
            with_target: true,
            with_file: true,
            with_thread_ids: false,
            with_thread_names: false,
            ansi_colors: true,
            file: None,
            rotation: None,
            filter: None,
        }
    }

    /// Set the log level.
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// Set the log format.
    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the filter directive.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }
}

/// Log level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Convert to tracing Level.
    pub fn to_tracing_level(self) -> tracing::Level {
        match self {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }

    /// Convert from verbosity level (0-3+).
    pub fn from_verbosity(verbosity: u8) -> Self {
        match verbosity {
            0 => LogLevel::Warn,
            1 => LogLevel::Info,
            2 => LogLevel::Debug,
            _ => LogLevel::Trace,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

/// Log output format.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Pretty console output with colors
    Pretty,
    /// Compact single-line output
    Compact,
    /// JSON structured output
    Json,
    /// Full format with all details
    Full,
}

/// Log rotation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotation {
    /// Rotation strategy
    pub strategy: RotationStrategy,

    /// Maximum number of log files to keep
    pub max_files: usize,
}

/// Log rotation strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RotationStrategy {
    /// Rotate daily
    Daily,
    /// Rotate hourly
    Hourly,
    /// Rotate when file exceeds size (in bytes)
    Size(u64),
    /// Never rotate
    Never,
}

/// Metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,

    /// Metrics exporter type
    pub exporter: MetricsExporterType,

    /// Prometheus endpoint port (for Prometheus exporter)
    pub prometheus_port: Option<u16>,

    /// Prometheus endpoint path
    pub prometheus_path: String,

    /// OTLP endpoint (for OTLP metrics exporter)
    pub otlp_endpoint: Option<String>,

    /// Export interval
    #[serde(with = "humantime_serde")]
    pub export_interval: Duration,

    /// Default histogram buckets
    pub histogram_buckets: Vec<f64>,

    /// Include host metrics
    pub include_host_metrics: bool,

    /// Include runtime metrics
    pub include_runtime_metrics: bool,

    /// Metric prefix
    pub prefix: Option<String>,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            exporter: MetricsExporterType::None,
            prometheus_port: None,
            prometheus_path: "/metrics".to_string(),
            otlp_endpoint: None,
            export_interval: Duration::from_secs(60),
            histogram_buckets: vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
            include_host_metrics: false,
            include_runtime_metrics: true,
            prefix: Some("rustible".to_string()),
        }
    }
}

impl MetricsConfig {
    /// Create a production metrics configuration.
    pub fn production() -> Self {
        Self {
            enabled: true,
            exporter: MetricsExporterType::Prometheus,
            prometheus_port: Some(9090),
            prometheus_path: "/metrics".to_string(),
            otlp_endpoint: None,
            export_interval: Duration::from_secs(15),
            histogram_buckets: vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ],
            include_host_metrics: true,
            include_runtime_metrics: true,
            prefix: Some("rustible".to_string()),
        }
    }

    /// Set the Prometheus port.
    pub fn with_prometheus_port(mut self, port: u16) -> Self {
        self.exporter = MetricsExporterType::Prometheus;
        self.prometheus_port = Some(port);
        self
    }

    /// Set the OTLP endpoint for metrics.
    pub fn with_otlp_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.exporter = MetricsExporterType::Otlp;
        self.otlp_endpoint = Some(endpoint.into());
        self
    }
}

/// Metrics exporter type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MetricsExporterType {
    /// No metrics export
    None,
    /// Prometheus pull-based exporter
    Prometheus,
    /// OTLP push-based exporter
    Otlp,
    /// Print metrics to stdout (for development)
    Stdout,
}

/// Helper module for humantime serde support.
mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = humantime::format_duration(*duration).to_string();
        s.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        humantime::parse_duration(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TelemetryConfig::default();
        assert!(config.enabled);
        assert_eq!(config.service_name, "rustible");
        assert_eq!(config.environment, "development");
    }

    #[test]
    fn test_production_config() {
        let config = TelemetryConfig::production();
        assert!(config.tracing.enabled);
        assert_eq!(config.tracing.sampling_ratio, 0.1);
        assert_eq!(config.logging.format, LogFormat::Json);
    }

    #[test]
    fn test_log_level_from_verbosity() {
        assert_eq!(LogLevel::from_verbosity(0), LogLevel::Warn);
        assert_eq!(LogLevel::from_verbosity(1), LogLevel::Info);
        assert_eq!(LogLevel::from_verbosity(2), LogLevel::Debug);
        assert_eq!(LogLevel::from_verbosity(3), LogLevel::Trace);
        assert_eq!(LogLevel::from_verbosity(10), LogLevel::Trace);
    }

    #[test]
    fn test_config_builder() {
        let config = TelemetryConfig::new("my-service")
            .with_environment("staging")
            .with_resource_attribute("deployment.id", "abc123")
            .with_tracing(TracingConfig::production())
            .with_metrics(MetricsConfig::production());

        assert_eq!(config.service_name, "my-service");
        assert_eq!(config.environment, "staging");
        assert!(config.resource_attributes.contains_key("deployment.id"));
        assert!(config.tracing.enabled);
        assert!(config.metrics.enabled);
    }
}
