//! Task definitions and execution

use serde::{Deserialize, Serialize};

/// Task execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Whether the task changed anything
    pub changed: bool,
    /// Whether the task failed
    pub failed: bool,
    /// Whether the task was skipped
    pub skipped: bool,
    /// Message from the task
    pub msg: Option<String>,
}

impl TaskResult {
    /// Create a successful result
    pub fn ok() -> Self {
        Self {
            changed: false,
            failed: false,
            skipped: false,
            msg: None,
        }
    }

    /// Create a changed result
    pub fn changed() -> Self {
        Self {
            changed: true,
            failed: false,
            skipped: false,
            msg: None,
        }
    }

    /// Create a failed result
    pub fn failed(msg: impl Into<String>) -> Self {
        Self {
            changed: false,
            failed: true,
            skipped: false,
            msg: Some(msg.into()),
        }
    }

    /// Create a skipped result
    pub fn skipped(msg: impl Into<String>) -> Self {
        Self {
            changed: false,
            failed: false,
            skipped: true,
            msg: Some(msg.into()),
        }
    }
}
