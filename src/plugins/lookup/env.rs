//! Environment Variable Lookup Plugin
//!
//! Reads environment variables.

use super::{LookupContext, LookupError, LookupOptions, LookupPlugin, LookupResult};
use std::env;

/// Environment variable lookup plugin
#[derive(Debug, Clone, Default)]
pub struct EnvLookup;

impl EnvLookup {
    /// Create a new EnvLookup instance
    pub fn new() -> Self {
        Self
    }
}

impl LookupPlugin for EnvLookup {
    fn name(&self) -> &'static str {
        "env"
    }

    fn description(&self) -> &'static str {
        "Reads environment variables"
    }

    fn lookup(
        &self,
        terms: &[String],
        options: &LookupOptions,
        _context: &LookupContext,
    ) -> LookupResult<Vec<serde_json::Value>> {
        let mut results = Vec::new();
        let default_value = options.default.clone();

        for term in terms {
            match env::var(term) {
                Ok(value) => results.push(serde_json::Value::String(value)),
                Err(_) => {
                    if let Some(ref default) = default_value {
                        results.push(default.clone());
                    } else {
                        return Err(LookupError::EnvNotFound(term.clone()));
                    }
                }
            }
        }

        Ok(results)
    }
}
