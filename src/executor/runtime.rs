//! Runtime context for Rustible execution
//!
//! This module provides:
//! - Variable scoping (global, play, task, host)
//! - Fact storage
//! - Register system for task results

use std::sync::Arc;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// Scope levels for variable resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VarScope {
    /// Built-in variables (lowest precedence)
    Builtin,
    /// Inventory group variables
    GroupVars,
    /// Inventory host variables
    HostVars,
    /// Playbook variables
    PlaybookVars,
    /// Play-level variables
    PlayVars,
    /// Block variables
    BlockVars,
    /// Task variables
    TaskVars,
    /// Registered variables
    Registered,
    /// Set_fact / include_vars
    SetFact,
    /// Extra vars from command line (highest precedence)
    ExtraVars,
}

/// Container for host-specific variables
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostVars {
    /// Variables specific to this host
    vars: IndexMap<String, JsonValue>,
    /// Facts gathered from this host
    facts: IndexMap<String, JsonValue>,
    /// Registered task results
    registered: IndexMap<String, RegisteredResult>,
}

impl HostVars {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a variable for this host
    pub fn set_var(&mut self, name: String, value: JsonValue) {
        self.vars.insert(name, value);
    }

    /// Get a variable for this host
    pub fn get_var(&self, name: &str) -> Option<&JsonValue> {
        self.vars.get(name)
    }

    /// Set a fact for this host
    pub fn set_fact(&mut self, name: String, value: JsonValue) {
        self.facts.insert(name, value);
    }

    /// Get a fact for this host
    pub fn get_fact(&self, name: &str) -> Option<&JsonValue> {
        self.facts.get(name)
    }

    /// Get all facts for this host
    pub fn get_all_facts(&self) -> &IndexMap<String, JsonValue> {
        &self.facts
    }

    /// Register a task result
    pub fn register(&mut self, name: String, result: RegisteredResult) {
        self.registered.insert(name, result);
    }

    /// Get a registered result
    pub fn get_registered(&self, name: &str) -> Option<&RegisteredResult> {
        self.registered.get(name)
    }

    /// Merge another HostVars into this one
    pub fn merge(&mut self, other: &HostVars) {
        for (k, v) in &other.vars {
            self.vars.insert(k.clone(), v.clone());
        }
        for (k, v) in &other.facts {
            self.facts.insert(k.clone(), v.clone());
        }
        for (k, v) in &other.registered {
            self.registered.insert(k.clone(), v.clone());
        }
    }
}

/// Result of a task that can be registered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredResult {
    /// Whether the task changed something
    pub changed: bool,
    /// Whether the task failed
    pub failed: bool,
    /// Whether the task was skipped
    pub skipped: bool,
    /// Return code (for command/shell modules)
    pub rc: Option<i32>,
    /// Standard output
    pub stdout: Option<String>,
    /// Standard output as lines
    pub stdout_lines: Option<Vec<String>>,
    /// Standard error
    pub stderr: Option<String>,
    /// Standard error as lines
    pub stderr_lines: Option<Vec<String>>,
    /// Message from the task
    pub msg: Option<String>,
    /// Results for loop tasks
    pub results: Option<Vec<RegisteredResult>>,
    /// Module-specific data
    #[serde(flatten)]
    pub data: IndexMap<String, JsonValue>,
}

impl Default for RegisteredResult {
    fn default() -> Self {
        Self {
            changed: false,
            failed: false,
            skipped: false,
            rc: None,
            stdout: None,
            stdout_lines: None,
            stderr: None,
            stderr_lines: None,
            msg: None,
            results: None,
            data: IndexMap::new(),
        }
    }
}

impl RegisteredResult {
    /// Create a new registered result
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a successful result
    pub fn ok(changed: bool) -> Self {
        Self {
            changed,
            ..Default::default()
        }
    }

    /// Create a failed result
    pub fn failed(msg: impl Into<String>) -> Self {
        Self {
            failed: true,
            msg: Some(msg.into()),
            ..Default::default()
        }
    }

    /// Create a skipped result
    pub fn skipped(msg: impl Into<String>) -> Self {
        Self {
            skipped: true,
            msg: Some(msg.into()),
            ..Default::default()
        }
    }

    /// Convert to JSON value
    pub fn to_json(&self) -> JsonValue {
        serde_json::to_value(self).unwrap_or(JsonValue::Null)
    }
}

/// Group definition in inventory
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventoryGroup {
    /// Hosts in this group
    pub hosts: Vec<String>,
    /// Variables for this group
    pub vars: IndexMap<String, JsonValue>,
    /// Child groups
    pub children: Vec<String>,
}

/// Execution context passed to tasks
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Current host being executed on
    pub host: String,
    /// Whether we're in check mode (dry-run)
    pub check_mode: bool,
    /// Whether to show diffs
    pub diff_mode: bool,
}

impl ExecutionContext {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            check_mode: false,
            diff_mode: false,
        }
    }

    pub fn with_check_mode(mut self, check: bool) -> Self {
        self.check_mode = check;
        self
    }

    pub fn with_diff_mode(mut self, diff: bool) -> Self {
        self.diff_mode = diff;
        self
    }
}

/// The main runtime context holding all state during execution
#[derive(Debug, Default)]
pub struct RuntimeContext {
    /// Global variables (from inventory, playbook vars_files, etc.)
    global_vars: IndexMap<String, JsonValue>,

    /// Play-level variables
    play_vars: IndexMap<String, JsonValue>,

    /// Task-level variables
    task_vars: IndexMap<String, JsonValue>,

    /// Extra variables (highest precedence)
    extra_vars: IndexMap<String, JsonValue>,

    /// Per-host variables and facts
    host_data: IndexMap<String, HostVars>,

    /// Inventory groups
    groups: IndexMap<String, InventoryGroup>,

    /// Special "all" group containing all hosts
    all_hosts: Vec<String>,

    /// Magic variables
    magic_vars: IndexMap<String, JsonValue>,
}

impl RuntimeContext {
    /// Create a new runtime context
    pub fn new() -> Self {
        let mut ctx = Self::default();
        ctx.init_magic_vars();
        ctx
    }

    /// Initialize magic variables
    fn init_magic_vars(&mut self) {
        self.magic_vars.insert(
            "ansible_version".to_string(),
            serde_json::json!({
                "full": env!("CARGO_PKG_VERSION"),
                "major": 2,
                "minor": 16,
                "revision": 0,
                "string": format!("rustible {}", env!("CARGO_PKG_VERSION"))
            }),
        );

        self.magic_vars.insert(
            "rustible_version".to_string(),
            serde_json::json!(env!("CARGO_PKG_VERSION")),
        );

        // Playbook directory will be set when playbook is loaded
        self.magic_vars
            .insert("playbook_dir".to_string(), JsonValue::Null);

        self.magic_vars
            .insert("inventory_dir".to_string(), JsonValue::Null);
    }

    /// Set a global variable
    pub fn set_global_var(&mut self, name: String, value: JsonValue) {
        trace!("Setting global var: {} = {:?}", name, value);
        self.global_vars.insert(name, value);
    }

    /// Set a play-level variable
    pub fn set_play_var(&mut self, name: String, value: JsonValue) {
        trace!("Setting play var: {} = {:?}", name, value);
        self.play_vars.insert(name, value);
    }

    /// Set a task-level variable
    pub fn set_task_var(&mut self, name: String, value: JsonValue) {
        trace!("Setting task var: {} = {:?}", name, value);
        self.task_vars.insert(name, value);
    }

    /// Set an extra variable (highest precedence)
    pub fn set_extra_var(&mut self, name: String, value: JsonValue) {
        trace!("Setting extra var: {} = {:?}", name, value);
        self.extra_vars.insert(name, value);
    }

    /// Clear task-level variables (called between tasks)
    pub fn clear_task_vars(&mut self) {
        self.task_vars.clear();
    }

    /// Clear play-level variables (called between plays)
    pub fn clear_play_vars(&mut self) {
        self.play_vars.clear();
        self.task_vars.clear();
    }

    /// Get a variable by name, respecting precedence
    pub fn get_var(&self, name: &str, host: Option<&str>) -> Option<JsonValue> {
        // Check in order of precedence (highest first)

        // Extra vars (highest)
        if let Some(v) = self.extra_vars.get(name) {
            return Some(v.clone());
        }

        // Registered variables and set_fact (check host data)
        if let Some(host_name) = host {
            if let Some(host_data) = self.host_data.get(host_name) {
                if let Some(reg) = host_data.get_registered(name) {
                    return Some(reg.to_json());
                }
            }
        }

        // Task variables
        if let Some(v) = self.task_vars.get(name) {
            return Some(v.clone());
        }

        // Play variables
        if let Some(v) = self.play_vars.get(name) {
            return Some(v.clone());
        }

        // Global variables
        if let Some(v) = self.global_vars.get(name) {
            return Some(v.clone());
        }

        // Host variables
        if let Some(host_name) = host {
            if let Some(host_data) = self.host_data.get(host_name) {
                if let Some(v) = host_data.get_var(name) {
                    return Some(v.clone());
                }
            }
        }

        // Magic variables
        if let Some(v) = self.magic_vars.get(name) {
            return Some(v.clone());
        }

        None
    }

    /// Get all variables merged for a specific host
    pub fn get_merged_vars(&self, host: &str) -> IndexMap<String, JsonValue> {
        let mut merged = IndexMap::new();

        // Start with magic vars (lowest)
        for (k, v) in &self.magic_vars {
            merged.insert(k.clone(), v.clone());
        }

        // Global vars
        for (k, v) in &self.global_vars {
            merged.insert(k.clone(), v.clone());
        }

        // Group vars for groups this host is in
        for (_group_name, group) in &self.groups {
            if group.hosts.contains(&host.to_string()) {
                for (k, v) in &group.vars {
                    merged.insert(k.clone(), v.clone());
                }
            }
        }

        // Host-specific vars
        if let Some(host_data) = self.host_data.get(host) {
            for (k, v) in &host_data.vars {
                merged.insert(k.clone(), v.clone());
            }
        }

        // Play vars
        for (k, v) in &self.play_vars {
            merged.insert(k.clone(), v.clone());
        }

        // Task vars
        for (k, v) in &self.task_vars {
            merged.insert(k.clone(), v.clone());
        }

        // Host facts (under 'ansible_facts' namespace)
        if let Some(host_data) = self.host_data.get(host) {
            if !host_data.facts.is_empty() {
                merged.insert(
                    "ansible_facts".to_string(),
                    serde_json::to_value(host_data.get_all_facts()).unwrap_or(JsonValue::Null),
                );
            }

            // Registered vars
            for (k, v) in &host_data.registered {
                merged.insert(k.clone(), v.to_json());
            }
        }

        // Extra vars (highest)
        for (k, v) in &self.extra_vars {
            merged.insert(k.clone(), v.clone());
        }

        // Add special vars
        merged.insert("inventory_hostname".to_string(), JsonValue::String(host.to_string()));
        merged.insert(
            "inventory_hostname_short".to_string(),
            JsonValue::String(host.split('.').next().unwrap_or(host).to_string()),
        );

        // Add group names this host belongs to
        let group_names: Vec<String> = self
            .groups
            .iter()
            .filter(|(_, g)| g.hosts.contains(&host.to_string()))
            .map(|(name, _)| name.clone())
            .collect();
        merged.insert(
            "group_names".to_string(),
            serde_json::to_value(&group_names).unwrap_or(JsonValue::Array(vec![])),
        );

        merged
    }

    /// Add a host to the inventory
    pub fn add_host(&mut self, host: String, group: Option<&str>) {
        debug!("Adding host: {} to group: {:?}", host, group);

        if !self.all_hosts.contains(&host) {
            self.all_hosts.push(host.clone());
        }

        self.host_data
            .entry(host.clone())
            .or_insert_with(HostVars::new);

        if let Some(group_name) = group {
            let group = self
                .groups
                .entry(group_name.to_string())
                .or_insert_with(InventoryGroup::default);

            if !group.hosts.contains(&host) {
                group.hosts.push(host);
            }
        }
    }

    /// Add a group to the inventory
    pub fn add_group(&mut self, name: String, group: InventoryGroup) {
        debug!("Adding group: {}", name);
        self.groups.insert(name, group);
    }

    /// Get all hosts
    pub fn get_all_hosts(&self) -> Vec<String> {
        self.all_hosts.clone()
    }

    /// Get hosts in a group
    pub fn get_group_hosts(&self, group: &str) -> Option<Vec<String>> {
        self.groups.get(group).map(|g| {
            let mut hosts = g.hosts.clone();

            // Include hosts from child groups
            for child in &g.children {
                if let Some(child_hosts) = self.get_group_hosts(child) {
                    for h in child_hosts {
                        if !hosts.contains(&h) {
                            hosts.push(h);
                        }
                    }
                }
            }

            hosts
        })
    }

    /// Set a fact for a host
    pub fn set_host_fact(&mut self, host: &str, name: String, value: JsonValue) {
        let host_data = self
            .host_data
            .entry(host.to_string())
            .or_insert_with(HostVars::new);
        host_data.set_fact(name, value);
    }

    /// Get a fact for a host
    pub fn get_host_fact(&self, host: &str, name: &str) -> Option<JsonValue> {
        self.host_data
            .get(host)
            .and_then(|hd| hd.get_fact(name).cloned())
    }

    /// Set all facts for a host
    pub fn set_host_facts(&mut self, host: &str, facts: IndexMap<String, JsonValue>) {
        let host_data = self
            .host_data
            .entry(host.to_string())
            .or_insert_with(HostVars::new);
        for (k, v) in facts {
            host_data.set_fact(k, v);
        }
    }

    /// Register a task result for a host
    pub fn register_result(&mut self, host: &str, name: String, result: RegisteredResult) {
        debug!("Registering result '{}' for host '{}'", name, host);
        let host_data = self
            .host_data
            .entry(host.to_string())
            .or_insert_with(HostVars::new);
        host_data.register(name, result);
    }

    /// Get a registered result for a host
    pub fn get_registered(&self, host: &str, name: &str) -> Option<&RegisteredResult> {
        self.host_data
            .get(host)
            .and_then(|hd| hd.get_registered(name))
    }

    /// Set a host variable
    pub fn set_host_var(&mut self, host: &str, name: String, value: JsonValue) {
        let host_data = self
            .host_data
            .entry(host.to_string())
            .or_insert_with(HostVars::new);
        host_data.set_var(name, value);
    }

    /// Get a host variable
    pub fn get_host_var(&self, host: &str, name: &str) -> Option<JsonValue> {
        self.host_data
            .get(host)
            .and_then(|hd| hd.get_var(name).cloned())
    }

    /// Set a magic variable
    pub fn set_magic_var(&mut self, name: String, value: JsonValue) {
        self.magic_vars.insert(name, value);
    }

    /// Check if a host exists in the inventory
    pub fn has_host(&self, host: &str) -> bool {
        self.all_hosts.contains(&host.to_string())
    }

    /// Check if a group exists
    pub fn has_group(&self, group: &str) -> bool {
        self.groups.contains_key(group)
    }

    /// Get all group names
    pub fn get_all_groups(&self) -> Vec<String> {
        self.groups.keys().cloned().collect()
    }
}

/// Thread-safe wrapper for RuntimeContext
pub struct SharedRuntime {
    inner: Arc<RwLock<RuntimeContext>>,
}

impl SharedRuntime {
    pub fn new(ctx: RuntimeContext) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ctx)),
        }
    }

    pub fn inner(&self) -> Arc<RwLock<RuntimeContext>> {
        Arc::clone(&self.inner)
    }

    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, RuntimeContext> {
        self.inner.read().await
    }

    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, RuntimeContext> {
        self.inner.write().await
    }
}

impl Clone for SharedRuntime {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_precedence() {
        let mut ctx = RuntimeContext::new();

        // Set variables at different levels
        ctx.set_global_var("var1".to_string(), serde_json::json!("global"));
        ctx.set_play_var("var1".to_string(), serde_json::json!("play"));

        // Play should override global
        assert_eq!(
            ctx.get_var("var1", None),
            Some(serde_json::json!("play"))
        );

        // Task should override play
        ctx.set_task_var("var1".to_string(), serde_json::json!("task"));
        assert_eq!(
            ctx.get_var("var1", None),
            Some(serde_json::json!("task"))
        );

        // Extra should override all
        ctx.set_extra_var("var1".to_string(), serde_json::json!("extra"));
        assert_eq!(
            ctx.get_var("var1", None),
            Some(serde_json::json!("extra"))
        );
    }

    #[test]
    fn test_host_vars() {
        let mut ctx = RuntimeContext::new();
        ctx.add_host("server1".to_string(), Some("webservers"));

        ctx.set_host_var("server1", "http_port".to_string(), serde_json::json!(80));

        assert_eq!(
            ctx.get_host_var("server1", "http_port"),
            Some(serde_json::json!(80))
        );
    }

    #[test]
    fn test_host_facts() {
        let mut ctx = RuntimeContext::new();
        ctx.add_host("server1".to_string(), None);

        ctx.set_host_fact(
            "server1",
            "os_family".to_string(),
            serde_json::json!("Debian"),
        );

        assert_eq!(
            ctx.get_host_fact("server1", "os_family"),
            Some(serde_json::json!("Debian"))
        );
    }

    #[test]
    fn test_registered_result() {
        let mut ctx = RuntimeContext::new();
        ctx.add_host("server1".to_string(), None);

        let result = RegisteredResult {
            changed: true,
            stdout: Some("hello world".to_string()),
            stdout_lines: Some(vec!["hello world".to_string()]),
            ..Default::default()
        };

        ctx.register_result("server1", "my_result".to_string(), result);

        let registered = ctx.get_registered("server1", "my_result").unwrap();
        assert!(registered.changed);
        assert_eq!(registered.stdout, Some("hello world".to_string()));
    }

    #[test]
    fn test_group_hosts() {
        let mut ctx = RuntimeContext::new();

        ctx.add_host("web1".to_string(), Some("webservers"));
        ctx.add_host("web2".to_string(), Some("webservers"));
        ctx.add_host("db1".to_string(), Some("databases"));

        let web_hosts = ctx.get_group_hosts("webservers").unwrap();
        assert_eq!(web_hosts.len(), 2);
        assert!(web_hosts.contains(&"web1".to_string()));
        assert!(web_hosts.contains(&"web2".to_string()));
    }

    #[test]
    fn test_merged_vars() {
        let mut ctx = RuntimeContext::new();
        ctx.add_host("server1".to_string(), Some("webservers"));

        ctx.set_global_var("env".to_string(), serde_json::json!("production"));
        ctx.set_host_var("server1", "port".to_string(), serde_json::json!(8080));

        let merged = ctx.get_merged_vars("server1");

        assert_eq!(merged.get("env"), Some(&serde_json::json!("production")));
        assert_eq!(merged.get("port"), Some(&serde_json::json!(8080)));
        assert_eq!(
            merged.get("inventory_hostname"),
            Some(&serde_json::json!("server1"))
        );
    }
}
