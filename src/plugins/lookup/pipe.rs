//! Pipe Lookup Plugin
//!
//! Executes commands and reads output.

use super::{LookupContext, LookupError, LookupOptions, LookupPlugin, LookupResult};
use std::process::Command;

/// Pipe lookup plugin for executing commands
#[derive(Debug, Clone, Default)]
pub struct PipeLookup;

impl PipeLookup {
    /// Create a new PipeLookup instance
    pub fn new() -> Self {
        Self
    }
}

impl LookupPlugin for PipeLookup {
    fn name(&self) -> &'static str {
        "pipe"
    }

    fn description(&self) -> &'static str {
        "Executes commands and reads output"
    }

    fn lookup(
        &self,
        terms: &[String],
        _options: &LookupOptions,
        _context: &LookupContext,
    ) -> LookupResult<Vec<serde_json::Value>> {
        let mut results = Vec::new();

        for term in terms {
            let output = Command::new("sh")
                .arg("-c")
                .arg(term)
                .output()?;

            if !output.status.success() {
                return Err(LookupError::CommandFailed {
                    code: output.status.code().unwrap_or(-1),
                    message: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            results.push(serde_json::Value::String(stdout));
        }

        Ok(results)
    }
}
