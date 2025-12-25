//! Fuzz target for plugin name resolution.
//!
//! This fuzzer tests plugin name matching and resolution with arbitrary input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};

/// Known plugin names for resolution testing
const KNOWN_PLUGINS: &[&str] = &[
    "default",
    "minimal",
    "oneline",
    "json",
    "yaml",
    "timer",
    "tree",
    "diff",
    "junit",
    "notification",
    "dense",
    "forked",
    "selective",
    "counter",
    "null",
    "profile_tasks",
];

/// Arbitrary plugin name variations for fuzzing
#[derive(Debug, Clone, Arbitrary)]
struct FuzzPluginName {
    name: String,
    namespace: Option<String>,
    version: Option<String>,
}

/// Plugin matching strategy
#[derive(Debug, Clone, Arbitrary)]
enum FuzzMatchStrategy {
    Exact,
    CaseInsensitive,
    Prefix,
    Suffix,
    Contains,
    Regex,
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut unstructured = Unstructured::new(data);

    // Test plugin name resolution
    if let Ok(plugin) = FuzzPluginName::arbitrary(&mut unstructured) {
        let name = plugin.name.trim().to_lowercase();

        // Check for exact match
        let exact_match = KNOWN_PLUGINS.iter().any(|&p| p == name);

        // Check for case-insensitive match
        let case_insensitive_match = KNOWN_PLUGINS.iter()
            .any(|&p| p.eq_ignore_ascii_case(&name));

        // Check for prefix match
        let prefix_match = KNOWN_PLUGINS.iter()
            .any(|&p| p.starts_with(&name) || name.starts_with(p));

        // Check for suffix match
        let suffix_match = KNOWN_PLUGINS.iter()
            .any(|&p| p.ends_with(&name) || name.ends_with(p));

        // Check for contains match
        let contains_match = KNOWN_PLUGINS.iter()
            .any(|&p| p.contains(&name) || name.contains(p));

        let _ = (exact_match, case_insensitive_match, prefix_match, suffix_match, contains_match);

        // Handle namespaced plugin names (e.g., "rustible.callback.json")
        if let Some(namespace) = &plugin.namespace {
            let full_name = format!("{}.{}", namespace.trim(), name);
            let _ = full_name.split('.').collect::<Vec<_>>();
        }

        // Handle versioned plugin names (e.g., "json@2.0")
        if let Some(version) = &plugin.version {
            let versioned_name = format!("{}@{}", name, version.trim());
            let _ = versioned_name.split('@').collect::<Vec<_>>();

            // Try to parse version
            let version_parts: Vec<&str> = version.split('.').collect();
            if version_parts.len() >= 2 {
                let _ = version_parts[0].parse::<u32>();
                let _ = version_parts[1].parse::<u32>();
            }
        }

        // Test various name normalizations
        let normalized_underscore = name.replace('-', "_");
        let normalized_hyphen = name.replace('_', "-");
        let _ = (normalized_underscore, normalized_hyphen);

        // Test plugin name validation rules
        let is_valid_name = !name.is_empty()
            && name.len() <= 64
            && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            && name.chars().next().map(|c| c.is_alphabetic()).unwrap_or(false);
        let _ = is_valid_name;
    }

    // Test matching strategy application
    if let Ok(strategy) = FuzzMatchStrategy::arbitrary(&mut unstructured) {
        if let Ok(pattern) = String::arbitrary(&mut unstructured) {
            let pattern = pattern.trim();
            if !pattern.is_empty() && pattern.len() <= 256 {
                for &plugin in KNOWN_PLUGINS {
                    let matched = match strategy {
                        FuzzMatchStrategy::Exact => plugin == pattern,
                        FuzzMatchStrategy::CaseInsensitive => {
                            plugin.eq_ignore_ascii_case(pattern)
                        }
                        FuzzMatchStrategy::Prefix => plugin.starts_with(pattern),
                        FuzzMatchStrategy::Suffix => plugin.ends_with(pattern),
                        FuzzMatchStrategy::Contains => plugin.contains(pattern),
                        FuzzMatchStrategy::Regex => {
                            // Simple pattern matching without full regex
                            // to avoid regex compilation DoS
                            if pattern.contains('*') {
                                let parts: Vec<&str> = pattern.split('*').collect();
                                if parts.len() == 2 {
                                    plugin.starts_with(parts[0]) && plugin.ends_with(parts[1])
                                } else {
                                    false
                                }
                            } else {
                                plugin.contains(pattern)
                            }
                        }
                    };
                    let _ = matched;
                }
            }
        }
    }

    // Test alias resolution
    if let Ok(alias) = String::arbitrary(&mut unstructured) {
        let alias = alias.trim().to_lowercase();
        let resolved = match alias.as_str() {
            "min" | "quiet" => Some("minimal"),
            "line" | "single" => Some("oneline"),
            "jsn" | "machine" => Some("json"),
            "yml" | "human" => Some("yaml"),
            "time" | "timing" => Some("timer"),
            "hier" | "hierarchy" => Some("tree"),
            "changes" | "delta" => Some("diff"),
            "xml" | "test-report" => Some("junit"),
            "notify" | "alert" => Some("notification"),
            "compact" | "brief" => Some("dense"),
            "parallel" | "multi" => Some("forked"),
            "filter" | "filtered" => Some("selective"),
            "count" | "stats" => Some("counter"),
            "noop" | "silent" => Some("null"),
            "profile" | "perf" => Some("profile_tasks"),
            _ => None,
        };
        let _ = resolved;
    }
});
