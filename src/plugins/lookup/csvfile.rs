//! CSV File Lookup Plugin
//!
//! Parses CSV files and extracts data.

use super::{LookupContext, LookupError, LookupOptions, LookupPlugin, LookupResult};
use std::fs;
use std::path::PathBuf;

/// CSV file lookup plugin
#[derive(Debug, Clone, Default)]
pub struct CsvFileLookup;

impl CsvFileLookup {
    /// Create a new CsvFileLookup instance
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

impl LookupPlugin for CsvFileLookup {
    fn name(&self) -> &'static str {
        "csvfile"
    }

    fn description(&self) -> &'static str {
        "Parses CSV files and extracts data"
    }

    fn lookup(
        &self,
        terms: &[String],
        options: &LookupOptions,
        context: &LookupContext,
    ) -> LookupResult<Vec<serde_json::Value>> {
        let mut results = Vec::new();
        let delimiter = options.get_string("delimiter").unwrap_or_else(|| ",".to_string());
        let delimiter_char = delimiter.chars().next().unwrap_or(',');

        for term in terms {
            let path = self.resolve_path(term, context);

            if !path.exists() {
                return Err(LookupError::FileNotFound(path));
            }

            let content = fs::read_to_string(&path)?;
            let mut rows: Vec<serde_json::Value> = Vec::new();
            let mut lines = content.lines();

            // First line is headers
            let headers: Vec<&str> = if let Some(header_line) = lines.next() {
                header_line.split(delimiter_char).collect()
            } else {
                return Ok(vec![serde_json::json!([])]);
            };

            // Parse remaining lines as data
            for line in lines {
                let values: Vec<&str> = line.split(delimiter_char).collect();
                let mut row = serde_json::Map::new();

                for (i, header) in headers.iter().enumerate() {
                    let value = values.get(i).copied().unwrap_or("");
                    row.insert(header.to_string(), serde_json::Value::String(value.to_string()));
                }

                rows.push(serde_json::Value::Object(row));
            }

            results.push(serde_json::Value::Array(rows));
        }

        Ok(results)
    }
}
