//! Handler definitions for Rustible

use serde::{Deserialize, Serialize};

/// A handler that can be notified by tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handler {
    /// Handler name
    pub name: String,
    /// Module to execute
    pub module: String,
    /// Module arguments
    #[serde(default)]
    pub args: std::collections::HashMap<String, serde_json::Value>,
    /// Optional when condition
    pub when: Option<String>,
}

impl Handler {
    /// Create a new handler
    pub fn new(name: impl Into<String>, module: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            module: module.into(),
            args: std::collections::HashMap::new(),
            when: None,
        }
    }
}
