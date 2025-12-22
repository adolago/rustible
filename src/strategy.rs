//! Execution strategies for Rustible

use serde::{Deserialize, Serialize};

/// Execution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Strategy {
    /// Linear - run each task on all hosts before next task
    Linear,
    /// Free - each host runs independently as fast as possible
    Free,
    /// Host pinned - dedicated worker per host
    HostPinned,
}

impl Default for Strategy {
    fn default() -> Self {
        Self::Linear
    }
}

impl std::fmt::Display for Strategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linear => write!(f, "linear"),
            Self::Free => write!(f, "free"),
            Self::HostPinned => write!(f, "host_pinned"),
        }
    }
}
