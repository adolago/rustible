//! File Lookup Plugin
//!
//! Reads file contents from the filesystem.

use super::{LookupContext, LookupError, LookupOptions, LookupPlugin, LookupResult};
use std::fs;
use std::path::PathBuf;

/// File lookup plugin for reading file contents
#[derive(Debug, Clone, Default)]
pub struct FileLookup;

impl FileLookup {
    /// Create a new FileLookup instance
    pub fn new() -> Self {
        Self
    }

    /// Resolve a path relative to the context
    fn resolve_path(&self, path: &str, context: &LookupContext) -> PathBuf {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else if let Some(ref work_dir) = context.work_dir {
            work_dir.join(&path)
        } else {
            path
        }
    }
}

impl LookupPlugin for FileLookup {
    fn name(&self) -> &'static str {
        "file"
    }

    fn description(&self) -> &'static str {
        "Reads file contents from the filesystem"
    }

    fn lookup(
        &self,
        terms: &[String],
        options: &LookupOptions,
        context: &LookupContext,
    ) -> LookupResult<Vec<serde_json::Value>> {
        let mut results = Vec::new();
        let lstrip = options.get_bool_or("lstrip", false);
        let rstrip = options.get_bool_or("rstrip", true);

        for term in terms {
            let path = self.resolve_path(term, context);

            if !path.exists() {
                return Err(LookupError::FileNotFound(path));
            }

            let mut content = fs::read_to_string(&path)?;

            if lstrip {
                content = content.trim_start().to_string();
            }
            if rstrip {
                content = content.trim_end().to_string();
            }

            results.push(serde_json::Value::String(content));
        }

        Ok(results)
    }
}
