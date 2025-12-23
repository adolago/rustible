//! # Rustible - A Modern Configuration Management Tool
//!
//! Rustible is an async-first, type-safe configuration management and automation tool
//! written in Rust. It serves as a modern alternative to Ansible with improved performance,
//! better error handling, and parallel execution by default.
//!
//! ## Core Concepts
//!
//! - **Playbooks**: YAML-defined automation workflows containing plays and tasks
//! - **Inventory**: Collection of hosts organized into groups with variables
//! - **Modules**: Units of work that execute actions on target hosts
//! - **Tasks**: Individual units of execution that invoke modules
//! - **Handlers**: Special tasks triggered by notifications from other tasks
//! - **Roles**: Reusable collections of tasks, handlers, files, and templates
//! - **Facts**: System information gathered from target hosts
//! - **Connections**: Transport layer for communicating with hosts (SSH, local, etc.)
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                           CLI Interface                              │
//! │                    (clap-based command parsing)                      │
//! └─────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         Playbook Engine                              │
//! │              (Async execution with tokio runtime)                    │
//! └─────────────────────────────────────────────────────────────────────┘
//!                                    │
//!          ┌─────────────────────────┼─────────────────────────┐
//!          ▼                         ▼                         ▼
//! ┌─────────────────┐   ┌─────────────────────┐   ┌─────────────────────┐
//! │    Inventory    │   │   Module Registry   │   │   Template Engine   │
//! │    (hosts +     │   │   (built-in +       │   │   (Jinja2-compat    │
//! │     groups)     │   │    custom)          │   │    via minijinja)   │
//! └─────────────────┘   └─────────────────────┘   └─────────────────────┘
//!          │                         │                         │
//!          └─────────────────────────┼─────────────────────────┘
//!                                    ▼
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                      Connection Manager                              │
//! │          (SSH, Local, Docker, Kubernetes connections)                │
//! └─────────────────────────────────────────────────────────────────────┘
//!                                    │
//!                                    ▼
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         Target Hosts                                 │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Example
//!
//! ```rust,ignore
//! use rustible::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Load inventory
//!     let inventory = Inventory::from_file("inventory.yml").await?;
//!
//!     // Load and parse playbook
//!     let playbook = Playbook::from_file("playbook.yml").await?;
//!
//!     // Create executor with default settings
//!     let executor = PlaybookExecutor::new()
//!         .with_inventory(inventory)
//!         .with_parallelism(10)
//!         .build()?;
//!
//!     // Execute playbook
//!     let result = executor.run(&playbook).await?;
//!
//!     // Report results
//!     println!("{}", result.summary());
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

// Re-export commonly used items in prelude
pub mod prelude {
    //! Convenient re-exports of commonly used types and traits.

    pub use crate::connection::config::RetryConfig;
    pub use crate::connection::docker::DockerConnection;
    pub use crate::connection::local::LocalConnection;
    pub use crate::connection::ssh::SshConnection;
    pub use crate::connection::{
        CommandResult, Connection, ConnectionBuilder, ConnectionConfig, ConnectionError,
        ConnectionFactory, ConnectionResult, ConnectionType, ExecuteOptions, FileStat, HostConfig,
        TransferOptions,
    };
    pub use crate::error::{Error, Result};
    pub use crate::executor::{PlaybookExecutor, TaskExecutor};
    pub use crate::facts::Facts;
    pub use crate::handlers::Handler;
    pub use crate::inventory::{Group, Host, Inventory};
    pub use crate::modules::{Module, ModuleRegistry, ModuleResult};
    pub use crate::playbook::{Play, Playbook, Task};
    pub use crate::roles::Role;
    pub use crate::traits::*;
    pub use crate::vars::Variables;
}

// ============================================================================
// Core Modules
// ============================================================================

pub mod error;
pub mod traits;
pub mod vars;

// ============================================================================
// Playbook Components
// ============================================================================

pub mod handlers;
pub mod playbook;
pub mod roles;
pub mod tasks;

// ============================================================================
// Infrastructure
// ============================================================================

pub mod connection;
pub mod facts;
pub mod inventory;

// ============================================================================
// Execution Engine
// ============================================================================

pub mod executor;
pub mod strategy;

// ============================================================================
// Modules (Built-in task implementations)
// ============================================================================

pub mod modules;

// ============================================================================
// Templating and Variables
// ============================================================================

pub mod template;

// ============================================================================
// Vault (Encrypted secrets management)
// ============================================================================

pub mod vault;

// ============================================================================
// Configuration
// ============================================================================

pub mod config;

// ============================================================================
// Reporting and Output
// ============================================================================

pub mod output;

// ============================================================================
// Version Information
// ============================================================================

/// Returns the current version of Rustible.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Returns detailed version information including build metadata.
pub fn version_info() -> VersionInfo {
    VersionInfo {
        version: env!("CARGO_PKG_VERSION"),
        rust_version: option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown"),
        target: std::env::consts::ARCH,
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    }
}

/// Detailed version information for the Rustible build.
#[derive(Debug, Clone)]
pub struct VersionInfo {
    /// Semantic version string
    pub version: &'static str,
    /// Minimum Rust version required
    pub rust_version: &'static str,
    /// Target triple for the build
    pub target: &'static str,
    /// Build profile (debug or release)
    pub profile: &'static str,
}

impl std::fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rustible {} ({}, {})",
            self.version, self.target, self.profile
        )
    }
}
