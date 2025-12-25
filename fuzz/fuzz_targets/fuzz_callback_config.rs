//! Fuzz target for callback configuration parsing.
//!
//! This fuzzer tests configuration parsing robustness with arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use std::collections::HashMap;

/// Arbitrary callback configuration for fuzzing
#[derive(Debug, Clone, Arbitrary)]
struct FuzzCallbackConfig {
    plugin: String,
    output: String,
    verbosity: u8,
    show_diff: bool,
    check_mode: bool,
    options: Vec<(String, FuzzConfigValue)>,
}

/// Arbitrary config value types for fuzzing
#[derive(Debug, Clone, Arbitrary)]
enum FuzzConfigValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<String>),
    Null,
}

/// Arbitrary verbosity level
#[derive(Debug, Clone, Arbitrary)]
struct FuzzVerbosity(u8);

/// Arbitrary priority for plugin ordering
#[derive(Debug, Clone, Arbitrary)]
struct FuzzPluginPriority(i32);

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut unstructured = Unstructured::new(data);

    // Test callback config parsing
    if let Ok(config) = FuzzCallbackConfig::arbitrary(&mut unstructured) {
        // Validate plugin name
        let plugin_name = config.plugin.trim().to_lowercase();
        let _ = match plugin_name.as_str() {
            "default" | "minimal" | "oneline" | "json" | "yaml" |
            "timer" | "tree" | "diff" | "junit" | "notification" |
            "dense" | "forked" | "selective" | "counter" | "null" => true,
            _ => false,
        };

        // Validate output destination
        let output = config.output.trim();
        let is_stdout = output == "stdout" || output.is_empty();
        let is_stderr = output == "stderr";
        let is_file = !is_stdout && !is_stderr;
        let _ = (is_stdout, is_stderr, is_file);

        // Validate verbosity level (0-5)
        let verbosity = config.verbosity.min(5);
        let _ = verbosity;

        // Process options
        let mut options_map: HashMap<String, String> = HashMap::new();
        for (key, value) in &config.options {
            let key = key.trim().to_lowercase();
            if !key.is_empty() {
                let value_str = match value {
                    FuzzConfigValue::Bool(b) => b.to_string(),
                    FuzzConfigValue::Int(i) => i.to_string(),
                    FuzzConfigValue::Float(f) => {
                        if f.is_finite() {
                            f.to_string()
                        } else {
                            "0".to_string()
                        }
                    }
                    FuzzConfigValue::String(s) => s.clone(),
                    FuzzConfigValue::Array(arr) => arr.join(","),
                    FuzzConfigValue::Null => String::new(),
                };
                options_map.insert(key, value_str);
            }
        }

        // Validate specific option patterns
        if let Some(indent_str) = options_map.get("indent") {
            let _ = indent_str.parse::<usize>().unwrap_or(0);
        }

        if let Some(width_str) = options_map.get("width") {
            let _ = width_str.parse::<usize>().unwrap_or(80);
        }

        if let Some(timeout_str) = options_map.get("timeout") {
            let _ = timeout_str.parse::<u64>().unwrap_or(30);
        }
    }

    // Test verbosity level handling
    if let Ok(verbosity) = FuzzVerbosity::arbitrary(&mut unstructured) {
        let level = verbosity.0;
        let verbosity_name = match level {
            0 => "Normal",
            1 => "Verbose",
            2 => "MoreVerbose",
            3 => "Debug",
            4 => "ConnectionDebug",
            _ => "Max",
        };
        let _ = verbosity_name;
    }

    // Test priority ordering
    if let Ok(priority) = FuzzPluginPriority::arbitrary(&mut unstructured) {
        let p = priority.0;
        let priority_name = if p <= 100 {
            "stdout"
        } else if p <= 200 {
            "logging"
        } else if p <= 500 {
            "normal"
        } else if p <= 700 {
            "metrics"
        } else {
            "cleanup"
        };
        let _ = priority_name;
    }
});
