//! Subcommands module for Rustible CLI
//!
//! This module contains all the subcommand implementations.

pub mod check;
pub mod inventory;
pub mod run;
pub mod vault;

use crate::cli::output::OutputFormatter;
use crate::config::Config;
use anyhow::Result;
use std::path::PathBuf;

/// Common context shared between commands
pub struct CommandContext {
    /// Configuration
    pub config: Config,
    /// Output formatter
    pub output: OutputFormatter,
    /// Inventory path
    pub inventory_path: Option<PathBuf>,
    /// Extra variables
    pub extra_vars: Vec<String>,
    /// Verbosity level
    pub verbosity: u8,
    /// Check mode (dry-run)
    pub check_mode: bool,
    /// Diff mode
    pub diff_mode: bool,
    /// Limit pattern
    pub limit: Option<String>,
    /// Number of parallel forks
    pub forks: usize,
    /// Connection timeout
    pub timeout: u64,
}

impl CommandContext {
    /// Create a new command context from CLI arguments
    pub fn new(cli: &crate::cli::Cli, config: Config) -> Self {
        let output = OutputFormatter::new(
            !cli.no_color,
            cli.is_json(),
            cli.verbosity(),
        );

        Self {
            config,
            output,
            inventory_path: cli.inventory.clone(),
            extra_vars: cli.extra_vars.clone(),
            verbosity: cli.verbosity(),
            check_mode: cli.check_mode,
            diff_mode: cli.diff_mode,
            limit: cli.limit.clone(),
            forks: cli.forks,
            timeout: cli.timeout,
        }
    }

    /// Get the effective inventory path
    pub fn inventory(&self) -> Option<&PathBuf> {
        self.inventory_path.as_ref()
            .or(self.config.defaults.inventory.as_ref())
    }

    /// Parse extra variables into a HashMap
    pub fn parse_extra_vars(&self) -> Result<std::collections::HashMap<String, serde_yaml::Value>> {
        use std::collections::HashMap;

        let mut vars = HashMap::new();

        for var in &self.extra_vars {
            if let Some(file_path) = var.strip_prefix('@') {
                // Load from file
                let content = std::fs::read_to_string(file_path)?;
                let file_vars: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)?;
                vars.extend(file_vars);
            } else if let Some((key, value)) = var.split_once('=') {
                // Parse key=value
                let parsed_value: serde_yaml::Value = serde_yaml::from_str(value)
                    .unwrap_or_else(|_| serde_yaml::Value::String(value.to_string()));
                vars.insert(key.to_string(), parsed_value);
            }
        }

        Ok(vars)
    }
}

/// Trait for runnable commands
#[async_trait::async_trait]
pub trait Runnable {
    /// Execute the command
    async fn run(&self, ctx: &mut CommandContext) -> Result<i32>;
}
