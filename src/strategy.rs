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

impl Strategy {
    /// OPTIMIZATION: Quick strategy selection for small workloads
    ///
    /// For small workloads (< 10 hosts and < 10 tasks), returns a recommended
    /// strategy without complex analysis. This avoids overhead of strategy
    /// selection logic for trivial cases.
    ///
    /// Returns `None` if the workload is large enough to warrant analysis.
    #[inline]
    pub fn quick_select_for_small_workload(host_count: usize, task_count: usize) -> Option<Self> {
        // For very small workloads, Linear is optimal - avoids overhead
        if host_count <= 1 || task_count <= 1 {
            return Some(Self::Linear);
        }

        // For small workloads (< 10 hosts, < 10 tasks), use simple heuristic
        if host_count < 10 && task_count < 10 {
            // Free strategy has lowest overhead for small parallel execution
            return Some(Self::Free);
        }

        // Large workload - needs proper analysis
        None
    }

    /// Check if this is a small workload that benefits from fast-path execution
    #[inline]
    pub fn is_small_workload(host_count: usize, task_count: usize) -> bool {
        host_count < 10 && task_count < 10
    }
}
