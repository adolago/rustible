//! System facts gathering for Rustible

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Facts gathered from a host
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Facts {
    data: IndexMap<String, serde_json::Value>,
}

impl Facts {
    /// Create empty facts
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a fact
    pub fn set(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.data.insert(key.into(), value);
    }

    /// Get a fact
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.data.get(key)
    }

    /// Get all facts
    pub fn all(&self) -> &IndexMap<String, serde_json::Value> {
        &self.data
    }

    /// Gather local system facts
    pub fn gather_local() -> Self {
        let mut facts = Self::new();

        // OS info
        facts.set("os_family", serde_json::json!(std::env::consts::OS));
        facts.set("os_arch", serde_json::json!(std::env::consts::ARCH));

        // Hostname
        if let Ok(hostname) = hostname::get() {
            facts.set("hostname", serde_json::json!(hostname.to_string_lossy()));
        }

        // User
        if let Ok(user) = std::env::var("USER") {
            facts.set("user", serde_json::json!(user));
        }

        facts
    }
}
