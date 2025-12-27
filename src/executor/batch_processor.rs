//! Batch processor for loop operations
//!
//! This module provides batch processing for loop operations, addressing
//! Ansible's 87x loop slowdown by coalescing multiple operations into single calls.
//!
//! ## The Problem
//! In Ansible, a loop like:
//! ```yaml
//! - name: Install packages
//!   apt:
//!     name: "{{ item }}"
//!     state: present
//!   loop:
//!     - nginx
//!     - vim
//!     - htop
//! ```
//! Executes 3 separate SSH connections and 3 separate apt commands.
//!
//! ## The Solution
//! This batch processor detects batchable operations and coalesces them:
//! - Package installs: `apt install nginx vim htop` (single call)
//! - File operations: Batched transfers with parallel streams
//! - Command execution: Pipelined command execution

use indexmap::IndexMap;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, trace};

/// Represents a batch of similar operations that can be executed together
#[derive(Debug, Clone)]
pub struct OperationBatch {
    /// Module being executed
    pub module: String,
    /// Batch key for grouping (e.g., host + module + state)
    pub batch_key: String,
    /// Individual items in the batch
    pub items: Vec<BatchItem>,
    /// Common arguments shared by all items
    pub common_args: IndexMap<String, JsonValue>,
    /// Maximum batch size before splitting
    pub max_batch_size: usize,
}

/// A single item within a batch
#[derive(Debug, Clone)]
pub struct BatchItem {
    /// Loop index
    pub index: usize,
    /// Loop variable name
    pub loop_var: String,
    /// The item value
    pub value: JsonValue,
    /// Item-specific arguments (merged with common_args)
    pub args: IndexMap<String, JsonValue>,
}

/// Configuration for batch processing
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Enable batch processing
    pub enabled: bool,
    /// Maximum items per batch
    pub max_batch_size: usize,
    /// Minimum items to trigger batching
    pub min_batch_size: usize,
    /// Timeout for batch accumulation
    pub accumulation_timeout: Duration,
    /// Modules that support batching
    pub batchable_modules: Vec<String>,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_batch_size: 100,
            min_batch_size: 2,
            accumulation_timeout: Duration::from_millis(50),
            batchable_modules: vec![
                "apt".to_string(),
                "yum".to_string(),
                "dnf".to_string(),
                "pip".to_string(),
                "package".to_string(),
                "copy".to_string(),
                "file".to_string(),
                "template".to_string(),
                "command".to_string(),
                "shell".to_string(),
            ],
        }
    }
}

/// Result of batch analysis
#[derive(Debug, Clone)]
pub enum BatchAnalysis {
    /// Can be batched as a single operation
    Batchable(BatchStrategy),
    /// Must be executed sequentially (order matters)
    Sequential(String),
    /// Can be parallelized but not batched into single call
    Parallel,
}

/// Strategy for how to batch operations
#[derive(Debug, Clone)]
pub enum BatchStrategy {
    /// Package modules: combine names into single install
    PackageList,
    /// File operations: parallel transfers
    ParallelTransfer,
    /// Command pipelining: execute in single SSH session
    CommandPipeline,
    /// Generic batching: group and execute
    Generic,
}

/// Batch processor for coalescing loop operations
#[derive(Debug)]
pub struct BatchProcessor {
    config: BatchConfig,
    /// Pending batches by key
    pending: HashMap<String, OperationBatch>,
}

impl BatchProcessor {
    /// Create a new batch processor
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            pending: HashMap::new(),
        }
    }

    /// Analyze if a loop can be batched
    pub fn analyze_loop(
        &self,
        module: &str,
        args: &IndexMap<String, JsonValue>,
        items: &[JsonValue],
    ) -> BatchAnalysis {
        // Check if module supports batching
        if !self.config.batchable_modules.contains(&module.to_string()) {
            return BatchAnalysis::Parallel;
        }

        // Check minimum batch size
        if items.len() < self.config.min_batch_size {
            return BatchAnalysis::Sequential("Too few items to batch".to_string());
        }

        // Determine batch strategy based on module
        match module {
            "apt" | "yum" | "dnf" | "package" | "pip" => {
                // Package managers support installing multiple packages at once
                if Self::is_package_install(args) {
                    BatchAnalysis::Batchable(BatchStrategy::PackageList)
                } else {
                    BatchAnalysis::Parallel
                }
            }
            "copy" | "template" | "fetch" => {
                // File operations can be parallelized but not combined
                BatchAnalysis::Batchable(BatchStrategy::ParallelTransfer)
            }
            "command" | "shell" => {
                // Commands can be pipelined in single SSH session
                if Self::commands_are_independent(items) {
                    BatchAnalysis::Batchable(BatchStrategy::CommandPipeline)
                } else {
                    BatchAnalysis::Sequential("Commands may have dependencies".to_string())
                }
            }
            _ => BatchAnalysis::Batchable(BatchStrategy::Generic),
        }
    }

    /// Check if package operation is an install (batchable)
    fn is_package_install(args: &IndexMap<String, JsonValue>) -> bool {
        match args.get("state") {
            Some(JsonValue::String(s)) => matches!(s.as_str(), "present" | "installed" | "latest"),
            None => true, // Default is present
            _ => false,
        }
    }

    /// Check if commands are independent (can be pipelined)
    fn commands_are_independent(items: &[JsonValue]) -> bool {
        // Conservative check: if any item references previous output, don't batch
        for item in items {
            if let JsonValue::String(cmd) = item {
                // Check for common patterns that indicate dependencies
                if cmd.contains("$?")
                    || cmd.contains("$(")
                    || cmd.contains("&&")
                    || cmd.contains("||")
                {
                    return false;
                }
            }
        }
        true
    }

    /// Create a batched operation from loop items
    pub fn create_batch(
        &self,
        module: &str,
        args: &IndexMap<String, JsonValue>,
        items: &[JsonValue],
        loop_var: &str,
        strategy: BatchStrategy,
    ) -> Result<BatchedOperation, String> {
        match strategy {
            BatchStrategy::PackageList => {
                self.create_package_batch(module, args, items, loop_var)
            }
            BatchStrategy::CommandPipeline => {
                self.create_command_pipeline(module, args, items, loop_var)
            }
            BatchStrategy::ParallelTransfer => {
                self.create_parallel_transfer_batch(module, args, items, loop_var)
            }
            BatchStrategy::Generic => self.create_generic_batch(module, args, items, loop_var),
        }
    }

    /// Create a batch for package operations
    fn create_package_batch(
        &self,
        module: &str,
        args: &IndexMap<String, JsonValue>,
        items: &[JsonValue],
        _loop_var: &str,
    ) -> Result<BatchedOperation, String> {
        // Extract all package names
        let package_names: Vec<String> = items
            .iter()
            .filter_map(|item| match item {
                JsonValue::String(s) => Some(s.clone()),
                JsonValue::Object(obj) => obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                _ => None,
            })
            .collect();

        if package_names.is_empty() {
            return Err("No valid package names found".to_string());
        }

        // Create merged args with package list
        let mut merged_args = args.clone();

        // Different modules handle package lists differently
        match module {
            "apt" | "yum" | "dnf" | "package" => {
                // These accept name as a list
                merged_args.insert(
                    "name".to_string(),
                    JsonValue::Array(package_names.iter().map(|n| JsonValue::String(n.clone())).collect()),
                );
            }
            "pip" => {
                // pip accepts name as list or string
                merged_args.insert(
                    "name".to_string(),
                    JsonValue::Array(package_names.iter().map(|n| JsonValue::String(n.clone())).collect()),
                );
            }
            _ => {
                // Fallback: space-separated list
                merged_args.insert(
                    "name".to_string(),
                    JsonValue::String(package_names.join(" ")),
                );
            }
        }

        debug!(
            "Created package batch for {} packages: {:?}",
            package_names.len(),
            package_names
        );

        Ok(BatchedOperation {
            module: module.to_string(),
            args: merged_args,
            items: items.to_vec(),
            strategy: BatchStrategy::PackageList,
            estimated_speedup: package_names.len() as f64 * 0.8, // ~80% speedup per item
        })
    }

    /// Create a command pipeline batch
    fn create_command_pipeline(
        &self,
        module: &str,
        args: &IndexMap<String, JsonValue>,
        items: &[JsonValue],
        loop_var: &str,
    ) -> Result<BatchedOperation, String> {
        // Extract commands
        let commands: Vec<String> = items
            .iter()
            .filter_map(|item| match item {
                JsonValue::String(s) => Some(s.clone()),
                JsonValue::Object(obj) => {
                    // Try cmd or _raw_params
                    obj.get("cmd")
                        .or_else(|| obj.get("_raw_params"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                }
                _ => None,
            })
            .collect();

        if commands.is_empty() {
            return Err("No valid commands found".to_string());
        }

        // Create a combined script that runs all commands
        let script = commands
            .iter()
            .map(|cmd| format!("echo '=== {} ==='; {}", cmd, cmd))
            .collect::<Vec<_>>()
            .join("\n");

        let mut merged_args = args.clone();
        merged_args.insert("_raw_params".to_string(), JsonValue::String(script));

        debug!(
            "Created command pipeline with {} commands using loop_var '{}'",
            commands.len(),
            loop_var
        );

        Ok(BatchedOperation {
            module: module.to_string(),
            args: merged_args,
            items: items.to_vec(),
            strategy: BatchStrategy::CommandPipeline,
            estimated_speedup: commands.len() as f64 * 0.6, // ~60% speedup (SSH overhead saved)
        })
    }

    /// Create parallel transfer batch
    fn create_parallel_transfer_batch(
        &self,
        module: &str,
        args: &IndexMap<String, JsonValue>,
        items: &[JsonValue],
        _loop_var: &str,
    ) -> Result<BatchedOperation, String> {
        // For file transfers, we don't combine into single operation
        // but we mark them for parallel execution with connection reuse

        trace!(
            "Created parallel transfer batch with {} items for {}",
            items.len(),
            module
        );

        Ok(BatchedOperation {
            module: module.to_string(),
            args: args.clone(),
            items: items.to_vec(),
            strategy: BatchStrategy::ParallelTransfer,
            estimated_speedup: items.len() as f64 * 0.3, // ~30% speedup (connection reuse)
        })
    }

    /// Create generic batch
    fn create_generic_batch(
        &self,
        module: &str,
        args: &IndexMap<String, JsonValue>,
        items: &[JsonValue],
        _loop_var: &str,
    ) -> Result<BatchedOperation, String> {
        trace!(
            "Created generic batch with {} items for {}",
            items.len(),
            module
        );

        Ok(BatchedOperation {
            module: module.to_string(),
            args: args.clone(),
            items: items.to_vec(),
            strategy: BatchStrategy::Generic,
            estimated_speedup: items.len() as f64 * 0.2, // ~20% speedup
        })
    }

    /// Split batch into chunks if too large
    pub fn split_batch(&self, batch: BatchedOperation) -> Vec<BatchedOperation> {
        if batch.items.len() <= self.config.max_batch_size {
            return vec![batch];
        }

        let mut batches = Vec::new();
        for chunk in batch.items.chunks(self.config.max_batch_size) {
            batches.push(BatchedOperation {
                module: batch.module.clone(),
                args: batch.args.clone(),
                items: chunk.to_vec(),
                strategy: batch.strategy.clone(),
                estimated_speedup: chunk.len() as f64 * 0.5,
            });
        }

        debug!(
            "Split large batch into {} smaller batches of max {} items",
            batches.len(),
            self.config.max_batch_size
        );

        batches
    }
}

/// A batched operation ready for execution
#[derive(Debug, Clone)]
pub struct BatchedOperation {
    /// Module to execute
    pub module: String,
    /// Merged arguments for batch execution
    pub args: IndexMap<String, JsonValue>,
    /// Original items (for result mapping)
    pub items: Vec<JsonValue>,
    /// Strategy used for batching
    pub strategy: BatchStrategy,
    /// Estimated speedup factor
    pub estimated_speedup: f64,
}

impl BatchedOperation {
    /// Get the number of items in this batch
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Estimate time savings compared to sequential execution
    pub fn estimated_time_savings(&self, per_item_ms: u64) -> Duration {
        let sequential_time = self.items.len() as u64 * per_item_ms;
        let batched_time = per_item_ms + (self.items.len() as u64 - 1) * 10; // Overhead per item
        Duration::from_millis(sequential_time.saturating_sub(batched_time))
    }
}

/// Result from a batched operation
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// Overall success
    pub success: bool,
    /// Overall changed status
    pub changed: bool,
    /// Per-item results
    pub item_results: Vec<BatchItemResult>,
    /// Any error message
    pub error: Option<String>,
}

/// Result for a single item in a batch
#[derive(Debug, Clone)]
pub struct BatchItemResult {
    /// Item index
    pub index: usize,
    /// Item value
    pub item: JsonValue,
    /// Success
    pub success: bool,
    /// Changed
    pub changed: bool,
    /// Message
    pub msg: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_package_batch() {
        let processor = BatchProcessor::new(BatchConfig::default());

        let mut args = IndexMap::new();
        args.insert("state".to_string(), JsonValue::String("present".to_string()));

        let items = vec![
            JsonValue::String("nginx".to_string()),
            JsonValue::String("vim".to_string()),
            JsonValue::String("htop".to_string()),
        ];

        match processor.analyze_loop("apt", &args, &items) {
            BatchAnalysis::Batchable(BatchStrategy::PackageList) => {}
            other => panic!("Expected PackageList strategy, got {:?}", other),
        }
    }

    #[test]
    fn test_create_package_batch() {
        let processor = BatchProcessor::new(BatchConfig::default());

        let mut args = IndexMap::new();
        args.insert("state".to_string(), JsonValue::String("present".to_string()));

        let items = vec![
            JsonValue::String("nginx".to_string()),
            JsonValue::String("vim".to_string()),
        ];

        let batch = processor
            .create_package_batch("apt", &args, &items, "item")
            .unwrap();

        assert_eq!(batch.module, "apt");
        assert!(batch.args.contains_key("name"));

        if let JsonValue::Array(names) = batch.args.get("name").unwrap() {
            assert_eq!(names.len(), 2);
        } else {
            panic!("Expected array for name");
        }
    }

    #[test]
    fn test_command_pipeline() {
        let processor = BatchProcessor::new(BatchConfig::default());

        let args = IndexMap::new();
        let items = vec![
            JsonValue::String("echo hello".to_string()),
            JsonValue::String("echo world".to_string()),
        ];

        let batch = processor
            .create_command_pipeline("command", &args, &items, "item")
            .unwrap();

        let raw_params = batch.args.get("_raw_params").unwrap().as_str().unwrap();
        assert!(raw_params.contains("echo hello"));
        assert!(raw_params.contains("echo world"));
    }

    #[test]
    fn test_split_large_batch() {
        let config = BatchConfig {
            max_batch_size: 5,
            ..Default::default()
        };
        let processor = BatchProcessor::new(config);

        let batch = BatchedOperation {
            module: "apt".to_string(),
            args: IndexMap::new(),
            items: (0..12).map(|i| JsonValue::Number(i.into())).collect(),
            strategy: BatchStrategy::PackageList,
            estimated_speedup: 1.0,
        };

        let splits = processor.split_batch(batch);
        assert_eq!(splits.len(), 3); // 5 + 5 + 2
        assert_eq!(splits[0].items.len(), 5);
        assert_eq!(splits[1].items.len(), 5);
        assert_eq!(splits[2].items.len(), 2);
    }
}
