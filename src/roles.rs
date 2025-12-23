//! Role definitions for Rustible

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A role containing reusable automation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role name
    pub name: String,
    /// Role path
    pub path: PathBuf,
    /// Role metadata
    #[serde(default)]
    pub meta: RoleMeta,
}

/// Role metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleMeta {
    /// Role dependencies
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Supported platforms
    #[serde(default)]
    pub platforms: Vec<String>,
}

impl Role {
    /// Create a new role
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            meta: RoleMeta::default(),
        }
    }
}
