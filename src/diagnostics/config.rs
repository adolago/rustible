//! Debug configuration for diagnostic tools.
//!
//! This module provides configuration options for controlling debug behavior,
//! including verbosity levels, trace settings, and output destinations.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Debug mode determines the overall debugging behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugMode {
    /// Debugging is disabled (default)
    #[default]
    Disabled,
    /// Basic debugging with essential information
    Basic,
    /// Verbose debugging with detailed tracing
    Verbose,
    /// Step-by-step execution mode
    StepByStep,
    /// Full debugging with all features enabled
    Full,
}

impl DebugMode {
    /// Check if debugging is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self, DebugMode::Disabled)
    }

    /// Check if step mode should be enabled
    pub fn is_step_mode(&self) -> bool {
        matches!(self, DebugMode::StepByStep | DebugMode::Full)
    }

    /// Check if verbose tracing should be enabled
    pub fn is_verbose(&self) -> bool {
        matches!(self, DebugMode::Verbose | DebugMode::Full)
    }
}

/// Configuration for debug and diagnostic features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    /// Debug mode
    pub mode: DebugMode,
    /// Verbosity level (0-5)
    pub verbosity: u8,
    /// Enable connection tracing
    pub trace_connections: bool,
    /// Trace level for connection events
    pub trace_level: super::TraceLevel,
    /// Enable step-by-step execution
    pub step_mode: bool,
    /// Dump state on failure
    pub dump_on_failure: bool,
    /// Path for state dumps
    pub dump_path: Option<PathBuf>,
    /// Maximum history entries to keep
    pub max_history: usize,
    /// Enable variable watches
    pub watch_variables: bool,
    /// Variables to watch by default
    pub default_watches: Vec<String>,
    /// Break on first failure
    pub break_on_failure: bool,
    /// Break on any change
    pub break_on_change: bool,
    /// Enable colored output
    pub color_output: bool,
    /// Show timestamps in output
    pub show_timestamps: bool,
    /// Log file path (if any)
    pub log_file: Option<PathBuf>,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            mode: DebugMode::Disabled,
            verbosity: 0,
            trace_connections: false,
            trace_level: super::TraceLevel::Info,
            step_mode: false,
            dump_on_failure: false,
            dump_path: None,
            max_history: 1000,
            watch_variables: false,
            default_watches: Vec::new(),
            break_on_failure: false,
            break_on_change: false,
            color_output: true,
            show_timestamps: true,
            log_file: None,
        }
    }
}

impl DebugConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration for verbose debugging
    pub fn verbose() -> Self {
        Self {
            mode: DebugMode::Verbose,
            verbosity: 3,
            trace_connections: true,
            trace_level: super::TraceLevel::Debug,
            dump_on_failure: true,
            show_timestamps: true,
            ..Default::default()
        }
    }

    /// Create a configuration for step-by-step execution
    pub fn step_by_step() -> Self {
        Self {
            mode: DebugMode::StepByStep,
            verbosity: 2,
            step_mode: true,
            watch_variables: true,
            dump_on_failure: true,
            show_timestamps: true,
            ..Default::default()
        }
    }

    /// Create a configuration for full debugging
    pub fn full() -> Self {
        Self {
            mode: DebugMode::Full,
            verbosity: 5,
            trace_connections: true,
            trace_level: super::TraceLevel::Trace,
            step_mode: true,
            dump_on_failure: true,
            watch_variables: true,
            break_on_failure: true,
            show_timestamps: true,
            max_history: 10000,
            ..Default::default()
        }
    }

    /// Create a builder for constructing configuration
    pub fn builder() -> DebugConfigBuilder {
        DebugConfigBuilder::new()
    }
}

/// Builder for constructing DebugConfig
#[derive(Debug, Default)]
pub struct DebugConfigBuilder {
    config: DebugConfig,
}

impl DebugConfigBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            config: DebugConfig::default(),
        }
    }

    /// Set the debug mode
    pub fn with_mode(mut self, mode: DebugMode) -> Self {
        self.config.mode = mode;
        self
    }

    /// Set the verbosity level (0-5)
    pub fn with_verbosity(mut self, level: u8) -> Self {
        self.config.verbosity = level.min(5);
        if level > 0 && self.config.mode == DebugMode::Disabled {
            self.config.mode = DebugMode::Basic;
        }
        self
    }

    /// Enable connection tracing
    pub fn with_trace_connections(mut self, enabled: bool) -> Self {
        self.config.trace_connections = enabled;
        self
    }

    /// Set the trace level
    pub fn with_trace_level(mut self, level: super::TraceLevel) -> Self {
        self.config.trace_level = level;
        self
    }

    /// Enable step-by-step execution mode
    pub fn with_step_mode(mut self, enabled: bool) -> Self {
        self.config.step_mode = enabled;
        if enabled && self.config.mode == DebugMode::Disabled {
            self.config.mode = DebugMode::StepByStep;
        }
        self
    }

    /// Enable state dump on failure
    pub fn with_dump_on_failure(mut self, enabled: bool) -> Self {
        self.config.dump_on_failure = enabled;
        self
    }

    /// Set the dump path
    pub fn with_dump_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.dump_path = Some(path.into());
        self
    }

    /// Set maximum history entries
    pub fn with_max_history(mut self, max: usize) -> Self {
        self.config.max_history = max;
        self
    }

    /// Enable variable watching
    pub fn with_watch_variables(mut self, enabled: bool) -> Self {
        self.config.watch_variables = enabled;
        self
    }

    /// Add default variables to watch
    pub fn with_default_watches(mut self, watches: Vec<String>) -> Self {
        self.config.default_watches = watches;
        self
    }

    /// Enable break on failure
    pub fn with_break_on_failure(mut self, enabled: bool) -> Self {
        self.config.break_on_failure = enabled;
        self
    }

    /// Enable break on change
    pub fn with_break_on_change(mut self, enabled: bool) -> Self {
        self.config.break_on_change = enabled;
        self
    }

    /// Enable colored output
    pub fn with_color_output(mut self, enabled: bool) -> Self {
        self.config.color_output = enabled;
        self
    }

    /// Enable timestamps in output
    pub fn with_timestamps(mut self, enabled: bool) -> Self {
        self.config.show_timestamps = enabled;
        self
    }

    /// Set log file path
    pub fn with_log_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.log_file = Some(path.into());
        self
    }

    /// Build the configuration
    pub fn build(self) -> DebugConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_mode_is_enabled() {
        assert!(!DebugMode::Disabled.is_enabled());
        assert!(DebugMode::Basic.is_enabled());
        assert!(DebugMode::Verbose.is_enabled());
        assert!(DebugMode::StepByStep.is_enabled());
        assert!(DebugMode::Full.is_enabled());
    }

    #[test]
    fn test_debug_mode_is_step_mode() {
        assert!(!DebugMode::Disabled.is_step_mode());
        assert!(!DebugMode::Basic.is_step_mode());
        assert!(!DebugMode::Verbose.is_step_mode());
        assert!(DebugMode::StepByStep.is_step_mode());
        assert!(DebugMode::Full.is_step_mode());
    }

    #[test]
    fn test_debug_config_default() {
        let config = DebugConfig::default();
        assert_eq!(config.mode, DebugMode::Disabled);
        assert_eq!(config.verbosity, 0);
        assert!(!config.trace_connections);
        assert!(!config.step_mode);
    }

    #[test]
    fn test_debug_config_verbose() {
        let config = DebugConfig::verbose();
        assert_eq!(config.mode, DebugMode::Verbose);
        assert!(config.verbosity >= 2);
        assert!(config.trace_connections);
    }

    #[test]
    fn test_debug_config_builder() {
        let config = DebugConfig::builder()
            .with_verbosity(3)
            .with_trace_connections(true)
            .with_step_mode(true)
            .with_break_on_failure(true)
            .build();

        assert!(config.mode.is_enabled());
        assert_eq!(config.verbosity, 3);
        assert!(config.trace_connections);
        assert!(config.step_mode);
        assert!(config.break_on_failure);
    }

    #[test]
    fn test_builder_auto_enables_mode() {
        let config = DebugConfig::builder().with_verbosity(2).build();
        assert!(config.mode.is_enabled());

        let config = DebugConfig::builder().with_step_mode(true).build();
        assert!(config.mode.is_step_mode());
    }
}
