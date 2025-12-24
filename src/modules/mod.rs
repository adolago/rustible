//! Module system for Rustible
//!
//! This module provides the core traits, types, and registry for the Rustible module system.
//! Modules are the building blocks that perform actual work on target systems.

pub mod apt;
pub mod assert;
pub mod blockinfile;
pub mod command;
pub mod copy;
pub mod debug;
pub mod dnf;
// TODO: facts module needs to be converted to sync Module trait
// pub mod facts;
pub mod file;
pub mod git;
pub mod group;
pub mod lineinfile;
pub mod package;
pub mod pip;
pub mod python;
pub mod service;
pub mod set_fact;
pub mod shell;
pub mod stat;
pub mod template;
pub mod user;
pub mod yum;

pub use python::PythonModuleExecutor;

use crate::connection::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during module execution
#[derive(Error, Debug)]
pub enum ModuleError {
    #[error("Module not found: {0}")]
    NotFound(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Command failed with exit code {code}: {message}")]
    CommandFailed { code: i32, message: String },

    #[error("Template error: {0}")]
    TemplateError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    #[error("Ansible module not found: {0}")]
    ModuleNotFound(String),
}

/// Result type for module operations
pub type ModuleResult<T> = Result<T, ModuleError>;

/// Status of a module execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleStatus {
    /// Module executed successfully and made changes
    Changed,
    /// Module executed successfully but no changes were needed
    Ok,
    /// Module execution failed
    Failed,
    /// Module was skipped (e.g., condition not met)
    Skipped,
}

impl fmt::Display for ModuleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleStatus::Changed => write!(f, "changed"),
            ModuleStatus::Ok => write!(f, "ok"),
            ModuleStatus::Failed => write!(f, "failed"),
            ModuleStatus::Skipped => write!(f, "skipped"),
        }
    }
}

/// Classification of modules based on their execution characteristics.
///
/// This enables intelligent parallelization and backwards compatibility with
/// Ansible modules by categorizing how each module executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModuleClassification {
    /// Tier 1: Logic modules that run entirely on the control node.
    /// Examples: debug, set_fact, assert, fail, meta, include_tasks
    /// These never touch the remote host and execute in nanoseconds.
    LocalLogic,

    /// Tier 2: File/transport modules implemented natively in Rust.
    /// Examples: copy, template, file, lineinfile, fetch
    /// These use direct SSH/SFTP operations without remote Python.
    NativeTransport,

    /// Tier 3: Remote command execution modules.
    /// Examples: command, shell, service, package, user
    /// These execute commands on the remote host via SSH.
    #[default]
    RemoteCommand,

    /// Tier 4: Python fallback for Ansible module compatibility.
    /// Used for any module without a native Rust implementation.
    /// Executes via AnsiballZ-compatible Python wrapper.
    PythonFallback,
}

impl fmt::Display for ModuleClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleClassification::LocalLogic => write!(f, "local_logic"),
            ModuleClassification::NativeTransport => write!(f, "native_transport"),
            ModuleClassification::RemoteCommand => write!(f, "remote_command"),
            ModuleClassification::PythonFallback => write!(f, "python_fallback"),
        }
    }
}

/// Hints for how a module can be parallelized across hosts.
///
/// The executor uses these hints to determine safe concurrency levels
/// and prevent race conditions or resource contention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ParallelizationHint {
    /// Safe to run simultaneously across all hosts.
    /// No shared state, no resource contention expected.
    #[default]
    FullyParallel,

    /// Requires exclusive access per host.
    /// Example: apt/yum operations that acquire package manager locks.
    HostExclusive,

    /// Network rate-limited operations.
    /// Example: API calls to cloud providers with rate limits.
    RateLimited {
        /// Maximum requests per second across all hosts
        requests_per_second: u32,
    },

    /// Requires global exclusive access.
    /// Only one instance can run across the entire inventory.
    /// Example: Cluster-wide configuration changes.
    GlobalExclusive,
}

/// Represents a difference between current and desired state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diff {
    /// Description of what will change
    pub before: String,
    /// Description of what it will change to
    pub after: String,
    /// Optional detailed diff (e.g., unified diff for files)
    pub details: Option<String>,
}

impl Diff {
    pub fn new(before: impl Into<String>, after: impl Into<String>) -> Self {
        Self {
            before: before.into(),
            after: after.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// Result of a module execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleOutput {
    /// Whether the module changed anything
    pub changed: bool,
    /// Human-readable message about what happened
    pub msg: String,
    /// Status of the execution
    pub status: ModuleStatus,
    /// Optional diff showing what changed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<Diff>,
    /// Additional data returned by the module
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub data: HashMap<String, serde_json::Value>,
    /// Standard output (for command modules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    /// Standard error (for command modules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// Return code (for command modules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rc: Option<i32>,
}

impl ModuleOutput {
    /// Create a new successful output with no changes
    pub fn ok(msg: impl Into<String>) -> Self {
        Self {
            changed: false,
            msg: msg.into(),
            status: ModuleStatus::Ok,
            diff: None,
            data: HashMap::new(),
            stdout: None,
            stderr: None,
            rc: None,
        }
    }

    /// Create a new successful output with changes
    pub fn changed(msg: impl Into<String>) -> Self {
        Self {
            changed: true,
            msg: msg.into(),
            status: ModuleStatus::Changed,
            diff: None,
            data: HashMap::new(),
            stdout: None,
            stderr: None,
            rc: None,
        }
    }

    /// Create a failed output
    pub fn failed(msg: impl Into<String>) -> Self {
        Self {
            changed: false,
            msg: msg.into(),
            status: ModuleStatus::Failed,
            diff: None,
            data: HashMap::new(),
            stdout: None,
            stderr: None,
            rc: None,
        }
    }

    /// Create a skipped output
    pub fn skipped(msg: impl Into<String>) -> Self {
        Self {
            changed: false,
            msg: msg.into(),
            status: ModuleStatus::Skipped,
            diff: None,
            data: HashMap::new(),
            stdout: None,
            stderr: None,
            rc: None,
        }
    }

    /// Add a diff to the output
    pub fn with_diff(mut self, diff: Diff) -> Self {
        self.diff = Some(diff);
        self
    }

    /// Add data to the output
    pub fn with_data(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.data.insert(key.into(), value);
        self
    }

    /// Add stdout/stderr/rc for command outputs
    pub fn with_command_output(
        mut self,
        stdout: Option<String>,
        stderr: Option<String>,
        rc: Option<i32>,
    ) -> Self {
        self.stdout = stdout;
        self.stderr = stderr;
        self.rc = rc;
        self
    }
}

/// Parameters passed to a module
pub type ModuleParams = HashMap<String, serde_json::Value>;

/// Context for module execution
#[derive(Clone)]
pub struct ModuleContext {
    /// Whether to run in check mode (dry run)
    pub check_mode: bool,
    /// Whether to show diffs
    pub diff_mode: bool,
    /// Variables available to the module
    pub vars: HashMap<String, serde_json::Value>,
    /// Facts about the target system
    pub facts: HashMap<String, serde_json::Value>,
    /// Working directory for the module
    pub work_dir: Option<String>,
    /// Whether running with elevated privileges
    pub r#become: bool,
    /// Method for privilege escalation
    pub become_method: Option<String>,
    /// User to become
    pub become_user: Option<String>,
    /// Connection to use for remote operations
    pub connection: Option<Arc<dyn Connection + Send + Sync>>,
}

impl std::fmt::Debug for ModuleContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleContext")
            .field("check_mode", &self.check_mode)
            .field("diff_mode", &self.diff_mode)
            .field("vars", &self.vars)
            .field("facts", &self.facts)
            .field("work_dir", &self.work_dir)
            .field("become", &self.r#become)
            .field("become_method", &self.become_method)
            .field("become_user", &self.become_user)
            .field(
                "connection",
                &self.connection.as_ref().map(|c| c.identifier()),
            )
            .finish()
    }
}

impl Default for ModuleContext {
    fn default() -> Self {
        Self {
            check_mode: false,
            diff_mode: false,
            vars: HashMap::new(),
            facts: HashMap::new(),
            work_dir: None,
            r#become: false,
            become_method: None,
            become_user: None,
            connection: None,
        }
    }
}

impl ModuleContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_check_mode(mut self, check_mode: bool) -> Self {
        self.check_mode = check_mode;
        self
    }

    pub fn with_diff_mode(mut self, diff_mode: bool) -> Self {
        self.diff_mode = diff_mode;
        self
    }

    pub fn with_vars(mut self, vars: HashMap<String, serde_json::Value>) -> Self {
        self.vars = vars;
        self
    }

    pub fn with_facts(mut self, facts: HashMap<String, serde_json::Value>) -> Self {
        self.facts = facts;
        self
    }

    pub fn with_connection(mut self, connection: Arc<dyn Connection + Send + Sync>) -> Self {
        self.connection = Some(connection);
        self
    }
}

/// Trait that all modules must implement
pub trait Module: Send + Sync {
    /// Returns the name of the module
    fn name(&self) -> &'static str;

    /// Returns a description of what the module does
    fn description(&self) -> &'static str;

    /// Returns the classification of this module for execution optimization.
    ///
    /// The classification determines how the executor handles this module:
    /// - `LocalLogic`: Runs on control node only, no remote execution
    /// - `NativeTransport`: Uses native Rust SSH/SFTP operations
    /// - `RemoteCommand`: Executes commands on remote host (default)
    /// - `PythonFallback`: Falls back to Ansible Python module execution
    fn classification(&self) -> ModuleClassification {
        ModuleClassification::RemoteCommand
    }

    /// Returns parallelization hints for the executor.
    ///
    /// This helps the executor determine safe concurrency levels:
    /// - `FullyParallel`: Can run on all hosts simultaneously (default)
    /// - `HostExclusive`: Only one task per host (e.g., package managers)
    /// - `RateLimited`: Network rate-limited operations
    /// - `GlobalExclusive`: Only one instance across entire inventory
    fn parallelization_hint(&self) -> ParallelizationHint {
        ParallelizationHint::FullyParallel
    }

    /// Execute the module with the given parameters
    fn execute(&self, params: &ModuleParams, context: &ModuleContext)
        -> ModuleResult<ModuleOutput>;

    /// Check what would change without making changes (for check mode)
    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        // Default implementation just calls execute with check_mode=true
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    /// Generate a diff of what would change
    fn diff(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        // Default implementation returns None
        let _ = (params, context);
        Ok(None)
    }

    /// Validate the parameters before execution
    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        // Default implementation does nothing
        let _ = params;
        Ok(())
    }

    /// Returns the list of required parameters
    fn required_params(&self) -> &[&'static str] {
        &[]
    }

    /// Returns the list of optional parameters with their default values
    fn optional_params(&self) -> HashMap<&'static str, serde_json::Value> {
        HashMap::new()
    }
}

/// Helper trait for extracting parameters
pub trait ParamExt {
    fn get_string(&self, key: &str) -> ModuleResult<Option<String>>;
    fn get_string_required(&self, key: &str) -> ModuleResult<String>;
    fn get_bool(&self, key: &str) -> ModuleResult<Option<bool>>;
    fn get_bool_or(&self, key: &str, default: bool) -> bool;
    fn get_i64(&self, key: &str) -> ModuleResult<Option<i64>>;
    fn get_u32(&self, key: &str) -> ModuleResult<Option<u32>>;
    fn get_vec_string(&self, key: &str) -> ModuleResult<Option<Vec<String>>>;
}

impl ParamExt for ModuleParams {
    fn get_string(&self, key: &str) -> ModuleResult<Option<String>> {
        match self.get(key) {
            Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
            Some(v) => Ok(Some(v.to_string().trim_matches('"').to_string())),
            None => Ok(None),
        }
    }

    fn get_string_required(&self, key: &str) -> ModuleResult<String> {
        self.get_string(key)?
            .ok_or_else(|| ModuleError::MissingParameter(key.to_string()))
    }

    fn get_bool(&self, key: &str) -> ModuleResult<Option<bool>> {
        match self.get(key) {
            Some(serde_json::Value::Bool(b)) => Ok(Some(*b)),
            Some(serde_json::Value::String(s)) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" | "on" => Ok(Some(true)),
                "false" | "no" | "0" | "off" => Ok(Some(false)),
                _ => Err(ModuleError::InvalidParameter(format!(
                    "{} must be a boolean",
                    key
                ))),
            },
            Some(_) => Err(ModuleError::InvalidParameter(format!(
                "{} must be a boolean",
                key
            ))),
            None => Ok(None),
        }
    }

    fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).ok().flatten().unwrap_or(default)
    }

    fn get_i64(&self, key: &str) -> ModuleResult<Option<i64>> {
        match self.get(key) {
            Some(serde_json::Value::Number(n)) => n.as_i64().map(Some).ok_or_else(|| {
                ModuleError::InvalidParameter(format!("{} must be an integer", key))
            }),
            Some(serde_json::Value::String(s)) => s
                .parse()
                .map(Some)
                .map_err(|_| ModuleError::InvalidParameter(format!("{} must be an integer", key))),
            Some(_) => Err(ModuleError::InvalidParameter(format!(
                "{} must be an integer",
                key
            ))),
            None => Ok(None),
        }
    }

    fn get_u32(&self, key: &str) -> ModuleResult<Option<u32>> {
        match self.get(key) {
            Some(serde_json::Value::Number(n)) => n
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .map(Some)
                .ok_or_else(|| {
                    ModuleError::InvalidParameter(format!("{} must be a positive integer", key))
                }),
            Some(serde_json::Value::String(s)) => s.parse().map(Some).map_err(|_| {
                ModuleError::InvalidParameter(format!("{} must be a positive integer", key))
            }),
            Some(_) => Err(ModuleError::InvalidParameter(format!(
                "{} must be a positive integer",
                key
            ))),
            None => Ok(None),
        }
    }

    fn get_vec_string(&self, key: &str) -> ModuleResult<Option<Vec<String>>> {
        match self.get(key) {
            Some(serde_json::Value::Array(arr)) => {
                let mut result = Vec::new();
                for item in arr {
                    match item {
                        serde_json::Value::String(s) => result.push(s.clone()),
                        v => result.push(v.to_string().trim_matches('"').to_string()),
                    }
                }
                Ok(Some(result))
            }
            Some(serde_json::Value::String(s)) => {
                // Handle comma-separated string
                Ok(Some(s.split(',').map(|s| s.trim().to_string()).collect()))
            }
            Some(_) => Err(ModuleError::InvalidParameter(format!(
                "{} must be an array",
                key
            ))),
            None => Ok(None),
        }
    }
}

/// Registry for looking up modules by name
pub struct ModuleRegistry {
    modules: HashMap<String, Arc<dyn Module>>,
}

impl ModuleRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    /// Create a registry with all built-in modules
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        // Package management modules
        registry.register(Arc::new(apt::AptModule));
        registry.register(Arc::new(dnf::DnfModule));
        registry.register(Arc::new(package::PackageModule));
        registry.register(Arc::new(pip::PipModule));
        registry.register(Arc::new(yum::YumModule));

        // Core command modules
        registry.register(Arc::new(command::CommandModule));
        registry.register(Arc::new(shell::ShellModule));

        // File/transport modules
        registry.register(Arc::new(blockinfile::BlockinfileModule));
        registry.register(Arc::new(copy::CopyModule));
        registry.register(Arc::new(file::FileModule));
        registry.register(Arc::new(lineinfile::LineinfileModule));
        registry.register(Arc::new(template::TemplateModule));

        // System management modules
        registry.register(Arc::new(group::GroupModule));
        registry.register(Arc::new(service::ServiceModule));
        registry.register(Arc::new(user::UserModule));

        // Source control modules
        registry.register(Arc::new(git::GitModule));

        // Logic/utility modules
        registry.register(Arc::new(assert::AssertModule));
        registry.register(Arc::new(debug::DebugModule));
        registry.register(Arc::new(set_fact::SetFactModule));
        registry.register(Arc::new(stat::StatModule));

        // TODO: facts module needs to be converted to sync Module trait
        // registry.register(Arc::new(facts::FactsModule));
        registry
    }

    /// Register a module
    pub fn register(&mut self, module: Arc<dyn Module>) {
        self.modules.insert(module.name().to_string(), module);
    }

    /// Get a module by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Module>> {
        self.modules.get(name).cloned()
    }

    /// Check if a module exists
    pub fn contains(&self, name: &str) -> bool {
        self.modules.contains_key(name)
    }

    /// Get all module names
    pub fn names(&self) -> Vec<&str> {
        self.modules.keys().map(|s| s.as_str()).collect()
    }

    /// Execute a module by name
    pub fn execute(
        &self,
        name: &str,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let module = self
            .get(name)
            .ok_or_else(|| ModuleError::NotFound(name.to_string()))?;

        // Validate parameters first
        module.validate_params(params)?;

        // Check required parameters
        for param in module.required_params() {
            if !params.contains_key(*param) {
                return Err(ModuleError::MissingParameter((*param).to_string()));
            }
        }

        // Execute based on mode
        if context.check_mode {
            module.check(params, context)
        } else {
            module.execute(params, context)
        }
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModule;

    impl Module for TestModule {
        fn name(&self) -> &'static str {
            "test"
        }

        fn description(&self) -> &'static str {
            "A test module"
        }

        fn execute(
            &self,
            params: &ModuleParams,
            context: &ModuleContext,
        ) -> ModuleResult<ModuleOutput> {
            if context.check_mode {
                return Ok(ModuleOutput::ok("Would do something"));
            }

            let msg = params
                .get_string("msg")?
                .unwrap_or_else(|| "Hello".to_string());
            Ok(ModuleOutput::changed(msg))
        }

        fn required_params(&self) -> &[&'static str] {
            &[]
        }
    }

    #[test]
    fn test_module_registry() {
        let mut registry = ModuleRegistry::new();
        registry.register(Arc::new(TestModule));

        assert!(registry.contains("test"));
        assert!(!registry.contains("nonexistent"));

        let module = registry.get("test").unwrap();
        assert_eq!(module.name(), "test");
    }

    #[test]
    fn test_module_output() {
        let output = ModuleOutput::changed("Something changed")
            .with_data("key", serde_json::json!("value"))
            .with_diff(Diff::new("old", "new"));

        assert!(output.changed);
        assert_eq!(output.status, ModuleStatus::Changed);
        assert!(output.diff.is_some());
        assert!(output.data.contains_key("key"));
    }

    #[test]
    fn test_param_ext() {
        let mut params: ModuleParams = HashMap::new();
        params.insert("string".to_string(), serde_json::json!("hello"));
        params.insert("bool_true".to_string(), serde_json::json!(true));
        params.insert("bool_str".to_string(), serde_json::json!("yes"));
        params.insert("number".to_string(), serde_json::json!(42));
        params.insert(
            "array".to_string(),
            serde_json::json!(["one", "two", "three"]),
        );

        assert_eq!(
            params.get_string("string").unwrap(),
            Some("hello".to_string())
        );
        assert_eq!(params.get_bool("bool_true").unwrap(), Some(true));
        assert_eq!(params.get_bool("bool_str").unwrap(), Some(true));
        assert_eq!(params.get_i64("number").unwrap(), Some(42));
        assert_eq!(
            params.get_vec_string("array").unwrap(),
            Some(vec![
                "one".to_string(),
                "two".to_string(),
                "three".to_string()
            ])
        );
    }
}
