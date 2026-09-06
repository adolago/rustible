//! Inventory management for Rustible.
//!
//! This module provides comprehensive inventory management including:
//! - Loading from YAML, INI, and JSON formats
//! - Dynamic inventory support (executable scripts)
//! - Host pattern matching
//! - Group hierarchy and variable inheritance
//! - Plugin-based inventory sources (AWS EC2, etc.)
//! - Inventory caching for improved performance
//!
//! # Architecture
//!
//! The inventory system consists of several key components:
//!
//! - [`Inventory`]: Main inventory structure holding hosts and groups
//! - [`Host`]: A managed host with connection parameters and variables
//! - [`Group`]: A logical grouping of hosts with shared variables
//! - [`InventoryPlugin`]: Trait for custom inventory sources
//! - [`InventoryCache`]: Caching layer for improved performance
//!
//! # Inventory Formats
//!
//! ## INI Format
//! ```ini
//! [webservers]
//! web1 ansible_host=10.0.0.1
//! web2 ansible_host=10.0.0.2
//!
//! [webservers:vars]
//! http_port=80
//!
//! [production:children]
//! webservers
//! databases
//! ```
//!
//! ## YAML Format
//! ```yaml
//! all:
//!   children:
//!     webservers:
//!       hosts:
//!         web1:
//!           ansible_host: 10.0.0.1
//!         web2:
//!           ansible_host: 10.0.0.2
//!       vars:
//!         http_port: 80
//! ```
//!
//! ## JSON Format (Dynamic Inventory)
//! ```json
//! {
//!   "webservers": {
//!     "hosts": ["web1", "web2"],
//!     "vars": {"http_port": 80}
//!   },
//!   "_meta": {
//!     "hostvars": {
//!       "web1": {"ansible_host": "10.0.0.1"}
//!     }
//!   }
//! }
//! ```
//!
//! # Plugin System
//!
//! The inventory plugin system allows extending inventory sources:
//!
//! ```rust,no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use rustible::inventory::plugin::{
//!     InventoryPlugin,
//!     InventoryPluginFactory,
//!     InventoryPluginConfig,
//!     InventoryCache,
//! };
//! use std::time::Duration;
//!
//! // Create an AWS EC2 plugin with caching
//! let config = InventoryPluginConfig::new()
//!     .with_option("region", "us-east-1")
//!     .with_cache_ttl(Duration::from_secs(300));
//!
//! let plugin = InventoryPluginFactory::create("aws_ec2", config)?;
//! let inventory = plugin.parse().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Pattern Matching
//!
//! The inventory supports powerful pattern matching:
//!
//! - `all` - All hosts
//! - `groupname` - All hosts in a group
//! - `host1:host2` - Multiple hosts/groups (union)
//! - `group1:&group2` - Intersection
//! - `group1:!group2` - Exclusion
//! - `~regex` - Regex match on hostname
//! - `web*` - Wildcard match

pub mod cache;
pub mod constructed;
pub mod group;
pub mod host;
pub mod plugin;
pub mod plugins;

pub use group::{Group, GroupBuilder, GroupHierarchy};
pub use host::{ConnectionParams, ConnectionType, Host, HostParseError, SshParams};
pub use plugin::{
    inventory_to_json, parse_json_inventory, parse_json_inventory_from_value,
    AwsEc2InventoryPlugin, CacheStats, CachedInventoryPlugin, FileInventoryPlugin, InventoryCache,
    InventoryPlugin, InventoryPluginConfig, InventoryPluginFactory, InventoryPluginRegistry,
    KeyedGroup, PluginError, PluginErrorKind, PluginInfo, PluginOptionInfo, PluginResult,
    PluginType, ScriptInventoryPlugin,
};

// Re-export enhanced cache types
pub use cache::{
    CacheEntryInfo, CacheStatsSnapshot, FileDependency, InventoryCache as EnhancedInventoryCache,
    InventoryCacheConfig, InventoryCacheEntry, InventoryCacheMetrics,
};

// Re-export constructed inventory plugin types
pub use constructed::{
    ConstructedConfig, ConstructedConfigBuilder, ConstructedError, ConstructedPlugin,
    ExpressionEvaluator,
};

// Re-export dynamic inventory plugin types
pub use plugins::{
    create_plugin_from_config, create_plugin_from_file, sanitize_group_name, AwsEc2Plugin,
    AzurePlugin, CacheConfig, ComposeConfig, DynamicInventoryPlugin, DynamicPluginRegistry,
    FilterConfig, FilterOperator, GcpPlugin, GroupByRule, HostnameConfig, KeyedGroupConfig,
    LocalBackend, PluginConfig, PluginConfigBuilder, PluginConfigError, PluginConfigResult,
    PluginOption, PluginOptionType, ResourceMapping, TerraformBackendType,
    TerraformInventoryPlugin, TerraformPlugin, TerraformPluginConfig, TerraformStateBackend,
};

use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;
use thiserror::Error;

/// Errors that can occur during inventory operations
#[derive(Debug, Error)]
pub enum InventoryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("host not found: {0}")]
    HostNotFound(String),

    #[error("group not found: {0}")]
    GroupNotFound(String),

    #[error("invalid host pattern: {0}")]
    InvalidPattern(String),

    #[error("duplicate host: {0}")]
    DuplicateHost(String),

    #[error("duplicate group: {0}")]
    DuplicateGroup(String),

    #[error("circular group dependency detected: {0}")]
    CircularDependency(String),

    #[error("dynamic inventory script failed: {0}")]
    DynamicInventoryFailed(String),

    #[error("invalid INI format: {0}")]
    InvalidIniFormat(String),

    #[error("host parse error: {0}")]
    HostParse(#[from] HostParseError),
}

/// Result type for inventory operations
pub type InventoryResult<T> = Result<T, InventoryError>;

/// The main inventory structure holding all hosts and groups
#[derive(Debug, Clone)]
pub struct Inventory {
    /// All hosts indexed by name
    hosts: HashMap<String, Host>,

    /// All groups indexed by name
    groups: HashMap<String, Group>,

    /// Source file/directory path
    source: Option<String>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

impl Inventory {
    /// Create a new empty inventory with default groups
    pub fn new() -> Self {
        let mut inventory = Self {
            hosts: HashMap::new(),
            groups: HashMap::new(),
            source: None,
        };

        // Create default groups
        inventory.groups.insert("all".to_string(), Group::all());
        inventory
            .groups
            .insert("ungrouped".to_string(), Group::ungrouped());

        inventory
    }

    /// Load inventory from a file or directory
    pub fn load<P: AsRef<Path>>(path: P) -> InventoryResult<Self> {
        let path = path.as_ref();
        let mut inventory = Self::new();
        inventory.source = Some(path.display().to_string());

        if path.is_file() {
            inventory.load_file(path)?;
        } else if path.is_dir() {
            inventory.load_directory(path)?;
        } else {
            return Err(InventoryError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Path not found: {}", path.display()),
            )));
        }

        // Finalize parent-child relationships
        inventory.validate_group_graph()?;
        inventory.compute_group_parents();

        Ok(inventory)
    }

    /// Load a single inventory file
    fn load_file(&mut self, path: &Path) -> InventoryResult<()> {
        // Check if it's an executable (dynamic inventory)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = path.metadata() {
                if metadata.permissions().mode() & 0o111 != 0 {
                    return self.load_dynamic(path);
                }
            }
        }

        let content = std::fs::read_to_string(path)?;
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match extension.to_lowercase().as_str() {
            "yml" | "yaml" => self.parse_yaml(&content)?,
            "json" => self.parse_json(&content)?,
            "ini" => {
                // Explicitly INI extension - parse as INI
                self.parse_ini(&content)?;
            }
            _ => {
                // Try to detect format from content
                let trimmed = content.trim();
                // Skip comment lines for detection
                let first_non_comment: &str = trimmed
                    .lines()
                    .find(|line| {
                        let t = line.trim();
                        !t.is_empty() && !t.starts_with('#')
                    })
                    .unwrap_or(trimmed);

                if trimmed.starts_with('{') {
                    // Starts with '{' - likely JSON
                    self.parse_json(&content)?;
                } else if first_non_comment.starts_with('[') {
                    // Check if it looks like INI section header [group] or JSON array
                    if first_non_comment.ends_with(']') && !first_non_comment.contains('{') {
                        // INI section like [webservers]
                        self.parse_ini(&content)?;
                    } else {
                        // JSON array
                        self.parse_json(&content)?;
                    }
                } else if first_non_comment.contains(':') && !first_non_comment.contains('=') {
                    // Looks like YAML (has colons but no INI-style equals)
                    self.parse_yaml(&content)?;
                } else if first_non_comment.contains('=') {
                    // INI-style key=value
                    self.parse_ini(&content)?;
                } else {
                    // Default to INI
                    self.parse_ini(&content)?;
                }
            }
        }

        Ok(())
    }

    /// Load inventory from a directory
    fn load_directory(&mut self, path: &Path) -> InventoryResult<()> {
        // Look for hosts file
        for name in ["hosts", "hosts.yml", "hosts.yaml", "hosts.ini"] {
            let hosts_file = path.join(name);
            if hosts_file.exists() {
                self.load_file(&hosts_file)?;
                break;
            }
        }

        // Load group_vars directory
        let group_vars = path.join("group_vars");
        if group_vars.is_dir() {
            self.load_group_vars(&group_vars)?;
        }

        // Load host_vars directory
        let host_vars = path.join("host_vars");
        if host_vars.is_dir() {
            self.load_host_vars(&host_vars)?;
        }

        Ok(())
    }

    /// Load group variables from group_vars directory
    fn load_group_vars(&mut self, path: &Path) -> InventoryResult<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.is_file() {
                let group_name = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                let content = std::fs::read_to_string(&file_path)?;
                let vars: IndexMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)?;

                if let Some(group) = self.groups.get_mut(&group_name) {
                    group.merge_vars(&vars);
                } else {
                    // Create the group if it doesn't exist
                    let mut group = Group::new(&group_name);
                    group.merge_vars(&vars);
                    self.groups.insert(group_name, group);
                }
            } else if file_path.is_dir() {
                // Handle directory-based group vars (group_name/vars.yml)
                let group_name = file_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                let vars = self.load_vars_from_directory(&file_path)?;
                if let Some(group) = self.groups.get_mut(&group_name) {
                    group.merge_vars(&vars);
                }
            }
        }

        Ok(())
    }

    /// Load host variables from host_vars directory
    fn load_host_vars(&mut self, path: &Path) -> InventoryResult<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.is_file() {
                let host_name = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                let content = std::fs::read_to_string(&file_path)?;
                let vars: IndexMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)?;

                if let Some(host) = self.hosts.get_mut(&host_name) {
                    host.merge_vars(&vars);
                }
            } else if file_path.is_dir() {
                let host_name = file_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                let vars = self.load_vars_from_directory(&file_path)?;
                if let Some(host) = self.hosts.get_mut(&host_name) {
                    host.merge_vars(&vars);
                }
            }
        }

        Ok(())
    }

    /// Load variables from a directory (multiple files merged)
    fn load_vars_from_directory(
        &self,
        path: &Path,
    ) -> InventoryResult<IndexMap<String, serde_yaml::Value>> {
        let mut merged_vars = IndexMap::new();

        let mut entries: Vec<_> = std::fs::read_dir(path)?.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let file_path = entry.path();
            if file_path.is_file() {
                let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "yml" || ext == "yaml" {
                    let content = std::fs::read_to_string(&file_path)?;
                    let vars: IndexMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)?;
                    merged_vars.extend(vars);
                }
            }
        }

        Ok(merged_vars)
    }

    /// Load dynamic inventory from an executable script
    fn load_dynamic(&mut self, path: &Path) -> InventoryResult<()> {
        let output = Command::new(path)
            .arg("--list")
            .output()
            .map_err(|e| InventoryError::DynamicInventoryFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(InventoryError::DynamicInventoryFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let json_output = String::from_utf8_lossy(&output.stdout);
        self.parse_json(&json_output)?;

        Ok(())
    }

    /// Parse YAML inventory format
    fn parse_yaml(&mut self, content: &str) -> InventoryResult<()> {
        let data: serde_yaml::Value = serde_yaml::from_str(content)?;

        let map = data
            .as_mapping()
            .ok_or_else(|| invalid_yaml_structure("inventory must be a mapping of groups"))?;
        // `all` is a group, not a wrapper that hides its top-level siblings.
        for (key, value) in map {
            let group_name = key
                .as_str()
                .ok_or_else(|| invalid_yaml_structure("group names must be strings"))?;
            self.parse_yaml_group(group_name, value)?;
        }

        Ok(())
    }

    /// Parse a YAML group definition
    fn parse_yaml_group(&mut self, name: &str, value: &serde_yaml::Value) -> InventoryResult<()> {
        validate_yaml_group_shape(value)?;
        let _group = self
            .groups
            .entry(name.to_string())
            .or_insert_with(|| Group::new(name));

        if let serde_yaml::Value::Mapping(map) = value {
            // Parse hosts
            if let Some(serde_yaml::Value::Mapping(hosts_map)) =
                map.get(serde_yaml::Value::String("hosts".to_string()))
            {
                for (host_key, host_value) in hosts_map {
                    if let serde_yaml::Value::String(host_name) = host_key {
                        // Check if host already exists
                        let host_exists = self.hosts.contains_key(host_name);

                        if host_exists {
                            // Host exists - just add it to this group and merge vars
                            if let Some(existing_host) = self.hosts.get_mut(host_name) {
                                existing_host.add_to_group(name.to_string());

                                // Parse and merge host variables
                                if let serde_yaml::Value::Mapping(host_vars) = host_value {
                                    for (var_key, var_value) in host_vars {
                                        if let serde_yaml::Value::String(key) = var_key {
                                            Self::apply_host_var_static(
                                                existing_host,
                                                key,
                                                var_value.clone(),
                                            )?;
                                        }
                                    }
                                }
                            }

                            // Add host to group
                            if let Some(g) = self.groups.get_mut(name) {
                                g.add_host(host_name.clone());
                            }
                        } else {
                            // New host - create it
                            let mut host = Host::new(host_name.clone());

                            // Parse host variables
                            if let serde_yaml::Value::Mapping(host_vars) = host_value {
                                for (var_key, var_value) in host_vars {
                                    if let serde_yaml::Value::String(key) = var_key {
                                        Self::apply_host_var_static(
                                            &mut host,
                                            key,
                                            var_value.clone(),
                                        )?;
                                    }
                                }
                            }

                            host.add_to_group(name.to_string());

                            // Get mutable reference to group and add host
                            if let Some(g) = self.groups.get_mut(name) {
                                g.add_host(host_name.clone());
                            }

                            // Add to all group
                            if name != "all" {
                                host.add_to_group("all".to_string());
                                if let Some(all_group) = self.groups.get_mut("all") {
                                    all_group.add_host(host_name.clone());
                                }
                            }

                            self.hosts.insert(host_name.clone(), host);
                        }
                    }
                }
            }

            // Parse children
            if let Some(serde_yaml::Value::Mapping(children_map)) =
                map.get(serde_yaml::Value::String("children".to_string()))
            {
                for (child_key, child_value) in children_map {
                    if let serde_yaml::Value::String(child_name) = child_key {
                        // Get mutable reference to group and add child
                        if let Some(g) = self.groups.get_mut(name) {
                            g.add_child(child_name.clone());
                        }
                        self.parse_yaml_group(child_name, child_value)?;
                    }
                }
            }

            // Parse vars
            if let Some(serde_yaml::Value::Mapping(vars_map)) =
                map.get(serde_yaml::Value::String("vars".to_string()))
            {
                for (var_key, var_value) in vars_map {
                    if let serde_yaml::Value::String(key) = var_key {
                        if let Some(g) = self.groups.get_mut(name) {
                            g.set_var(key.clone(), var_value.clone());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply a host variable from YAML.
    #[allow(dead_code)]
    fn apply_host_var(
        &self,
        host: &mut Host,
        key: &str,
        value: serde_yaml::Value,
    ) -> InventoryResult<()> {
        Self::apply_host_var_static(host, key, value)
    }

    /// Parse JSON inventory format (compatible with Ansible dynamic inventory)
    fn parse_json(&mut self, content: &str) -> InventoryResult<()> {
        let data: serde_json::Value = serde_json::from_str(content)?;

        if let serde_json::Value::Object(map) = data {
            // Collect hosts to add to "all" group
            let mut all_hosts: Vec<String> = Vec::new();

            // First pass: create groups
            for (key, _value) in &map {
                if key == "_meta" {
                    continue;
                }
                self.groups
                    .entry(key.clone())
                    .or_insert_with(|| Group::new(key));
            }

            // Second pass: populate groups and hosts
            for (key, value) in &map {
                if key == "_meta" {
                    continue;
                }

                // Collect data to add
                let mut hosts_to_add: Vec<String> = Vec::new();
                let mut children_to_add: Vec<String> = Vec::new();
                let mut vars_to_add: Vec<(String, serde_yaml::Value)> = Vec::new();

                if let serde_json::Value::Object(group_data) = value {
                    if let Some(serde_json::Value::Array(hosts)) = group_data.get("hosts") {
                        for host_value in hosts {
                            if let serde_json::Value::String(host_name) = host_value {
                                hosts_to_add.push(host_name.clone());
                            }
                        }
                    }

                    if let Some(serde_json::Value::Array(children)) = group_data.get("children") {
                        for child_value in children {
                            if let serde_json::Value::String(child_name) = child_value {
                                children_to_add.push(child_name.clone());
                            }
                        }
                    }

                    if let Some(serde_json::Value::Object(vars)) = group_data.get("vars") {
                        for (var_key, var_value) in vars {
                            let yaml_value = json_to_yaml(var_value)?;
                            if var_key == "ansible_port" {
                                parse_inventory_port(&yaml_value)?;
                            }
                            vars_to_add.push((var_key.clone(), yaml_value));
                        }
                    }
                } else if let serde_json::Value::Array(hosts) = value {
                    for host_value in hosts {
                        if let serde_json::Value::String(host_name) = host_value {
                            hosts_to_add.push(host_name.clone());
                        }
                    }
                }

                // Now apply changes to group
                if let Some(group) = self.groups.get_mut(key) {
                    for host_name in &hosts_to_add {
                        group.add_host(host_name.clone());
                    }
                    for child_name in children_to_add {
                        group.add_child(child_name);
                    }
                    for (var_key, var_value) in vars_to_add {
                        group.set_var(var_key, var_value);
                    }
                }

                // Add hosts to inventory
                for host_name in &hosts_to_add {
                    all_hosts.push(host_name.clone());
                    if !self.hosts.contains_key(host_name) {
                        let mut host = Host::new(host_name.clone());
                        host.add_to_group(key.clone());
                        host.add_to_group("all".to_string());
                        self.hosts.insert(host_name.clone(), host);
                    } else if let Some(h) = self.hosts.get_mut(host_name) {
                        h.add_to_group(key.clone());
                    }
                }
            }

            // Add all hosts to the "all" group
            if let Some(all_group) = self.groups.get_mut("all") {
                for host_name in all_hosts {
                    all_group.add_host(host_name);
                }
            }

            // Second pass: apply host variables from _meta
            if let Some(serde_json::Value::Object(meta)) = map.get("_meta") {
                if let Some(serde_json::Value::Object(hostvars)) = meta.get("hostvars") {
                    for (host_name, vars) in hostvars {
                        if let serde_json::Value::Object(vars_map) = vars {
                            // Collect the vars first
                            let yaml_vars: Vec<(String, serde_yaml::Value)> = vars_map
                                .iter()
                                .map(|(k, v)| Ok((k.clone(), json_to_yaml(v)?)))
                                .collect::<InventoryResult<_>>()?;

                            // Then apply them
                            if let Some(host) = self.hosts.get_mut(host_name) {
                                for (var_key, yaml_value) in yaml_vars {
                                    Self::apply_host_var_static(host, &var_key, yaml_value)?;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply a host variable from YAML (static version to avoid borrow issues)
    fn apply_host_var_static(
        host: &mut Host,
        key: &str,
        value: serde_yaml::Value,
    ) -> InventoryResult<()> {
        if key == "ansible_connection" {
            host.set_var(key, value.clone());
        }
        match key {
            "ansible_host" => {
                if let serde_yaml::Value::String(s) = value {
                    host.ansible_host = Some(s);
                }
            }
            "ansible_port" => {
                host.connection.ssh.port = parse_inventory_port(&value)?;
            }
            "ansible_user" => {
                if let serde_yaml::Value::String(s) = value {
                    host.connection.ssh.user = Some(s);
                }
            }
            "ansible_ssh_private_key_file" => {
                if let serde_yaml::Value::String(s) = value {
                    host.connection.ssh.private_key_file = Some(s);
                }
            }
            "ansible_connection" => {
                if let serde_yaml::Value::String(s) = value {
                    host.connection.connection = match s.as_str() {
                        "local" => ConnectionType::Local,
                        "docker" => ConnectionType::Docker,
                        "podman" => ConnectionType::Podman,
                        "winrm" => ConnectionType::Winrm,
                        _ => ConnectionType::Ssh,
                    };
                }
            }
            "ansible_become" => {
                host.connection.r#become = match value {
                    serde_yaml::Value::Bool(b) => b,
                    serde_yaml::Value::String(s) => s.to_lowercase() == "true" || s == "1",
                    _ => false,
                };
            }
            "ansible_become_method" => {
                if let serde_yaml::Value::String(s) = value {
                    host.connection.become_method = s;
                }
            }
            "ansible_become_user" => {
                if let serde_yaml::Value::String(s) = value {
                    host.connection.become_user = s;
                }
            }
            "ansible_python_interpreter" => {
                if let serde_yaml::Value::String(s) = value {
                    host.connection.python_interpreter = Some(s);
                }
            }
            _ => {
                host.set_var(key, value);
            }
        }
        Ok(())
    }

    /// Parse INI inventory format
    fn parse_ini(&mut self, content: &str) -> InventoryResult<()> {
        let mut current_group = "ungrouped".to_string();
        let mut is_vars_section = false;
        let mut is_children_section = false;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Check for section header
            if line.starts_with('[') && line.ends_with(']') {
                let section = &line[1..line.len() - 1];

                if let Some((group_name, suffix)) = section.rsplit_once(':') {
                    current_group = group_name.to_string();
                    is_vars_section = suffix == "vars";
                    is_children_section = suffix == "children";
                } else {
                    current_group = section.to_string();
                    is_vars_section = false;
                    is_children_section = false;
                }

                // Create group if it doesn't exist
                self.groups
                    .entry(current_group.clone())
                    .or_insert_with(|| Group::new(&current_group));

                continue;
            }

            if is_vars_section {
                // Parse group variable
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = parse_ini_value(value.trim())?;
                    if key == "ansible_port" {
                        parse_inventory_port(&value)?;
                    }

                    if let Some(group) = self.groups.get_mut(&current_group) {
                        group.set_var(key, value);
                    }
                }
            } else if is_children_section {
                // Add child group
                if let Some(group) = self.groups.get_mut(&current_group) {
                    group.add_child(line.to_string());
                }

                // Create child group if it doesn't exist
                self.groups
                    .entry(line.to_string())
                    .or_insert_with(|| Group::new(line));
            } else {
                // Parse host definition
                let host = Host::parse(line)?;
                let host_name = host.name.clone();

                // Add to current group
                if let Some(group) = self.groups.get_mut(&current_group) {
                    group.add_host(host_name.clone());
                }

                // Add to all group
                if current_group != "all" {
                    if let Some(all_group) = self.groups.get_mut("all") {
                        all_group.add_host(host_name.clone());
                    }
                }

                // Update or insert host
                if let Some(existing) = self.hosts.get_mut(&host_name) {
                    existing.add_to_group(current_group.clone());
                    existing.merge_vars(&host.vars);
                } else {
                    let mut new_host = host;
                    new_host.add_to_group(current_group.clone());
                    new_host.add_to_group("all".to_string());
                    self.hosts.insert(host_name, new_host);
                }
            }
        }

        Ok(())
    }

    /// Reject cycles without recursing, including deeply nested group graphs.
    fn validate_group_graph(&self) -> InventoryResult<()> {
        let mut complete = HashSet::new();
        let mut active = HashSet::new();

        for root in self.groups.keys() {
            let mut pending = vec![(root.as_str(), false)];
            while let Some((name, exiting)) = pending.pop() {
                if exiting {
                    active.remove(name);
                    complete.insert(name);
                    continue;
                }
                if complete.contains(name) {
                    continue;
                }
                if !active.insert(name) {
                    return Err(InventoryError::CircularDependency(name.to_string()));
                }
                pending.push((name, true));
                if let Some(group) = self.groups.get(name) {
                    for child in &group.children {
                        // Forward references remain permitted until the child is added.
                        if self.groups.contains_key(child) {
                            pending.push((child.as_str(), false));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Recompute parent group relationships from children.
    fn compute_group_parents(&mut self) {
        let children_map: HashMap<String, Vec<String>> = self
            .groups
            .iter()
            .map(|(name, group)| (name.clone(), group.children.iter().cloned().collect()))
            .collect();

        // Group replacement can remove edges; do not retain stale parents.
        for group in self.groups.values_mut() {
            group.parents.clear();
        }
        for (parent_name, children) in children_map {
            for child_name in children {
                if let Some(child) = self.groups.get_mut(&child_name) {
                    child.add_parent(parent_name.clone());
                }
            }
        }
    }

    /// Add a host to the inventory
    pub fn add_host(&mut self, host: Host) -> InventoryResult<()> {
        let name = host.name.clone();

        // Add to all group
        if let Some(all_group) = self.groups.get_mut("all") {
            all_group.add_host(name.clone());
        }

        // If host has no groups, add to ungrouped
        if host.groups.is_empty() || (host.groups.len() == 1 && host.in_group("all")) {
            if let Some(ungrouped) = self.groups.get_mut("ungrouped") {
                ungrouped.add_host(name.clone());
            }
        }

        self.hosts.insert(name, host);
        Ok(())
    }

    /// Add or replace a group, rejecting cycles without changing the inventory.
    pub fn add_group(&mut self, group: Group) -> InventoryResult<()> {
        let name = group.name.clone();
        let previous = self.groups.insert(name.clone(), group);
        if let Err(error) = self.validate_group_graph() {
            if let Some(previous) = previous {
                self.groups.insert(name, previous);
            } else {
                self.groups.remove(&name);
            }
            return Err(error);
        }
        self.compute_group_parents();
        Ok(())
    }

    /// Get a host by name
    pub fn get_host(&self, name: &str) -> Option<&Host> {
        self.hosts.get(name)
    }

    /// Get a mutable reference to a host by name
    pub fn get_host_mut(&mut self, name: &str) -> Option<&mut Host> {
        self.hosts.get_mut(name)
    }

    /// Get a group by name
    pub fn get_group(&self, name: &str) -> Option<&Group> {
        self.groups.get(name)
    }

    /// Get a mutable reference to a group by name
    pub fn get_group_mut(&mut self, name: &str) -> Option<&mut Group> {
        self.groups.get_mut(name)
    }

    /// Get all hosts
    pub fn hosts(&self) -> impl Iterator<Item = &Host> {
        self.hosts.values()
    }

    /// Get all hosts as a vector
    pub fn get_all_hosts(&self) -> Vec<&Host> {
        self.hosts.values().collect()
    }

    /// Get all groups
    pub fn groups(&self) -> impl Iterator<Item = &Group> {
        self.groups.values()
    }

    /// Get all host names
    pub fn host_names(&self) -> impl Iterator<Item = &String> {
        self.hosts.keys()
    }

    /// Get all group names
    pub fn group_names(&self) -> impl Iterator<Item = &String> {
        self.groups.keys()
    }

    /// Get hosts matching a pattern
    ///
    /// Supported patterns:
    /// - `all` - all hosts
    /// - `hostname` - specific host
    /// - `groupname` - all hosts in group
    /// - `host1:host2` - multiple hosts/groups (union)
    /// - `group1:&group2` - intersection
    /// - `group1:!group2` - exclusion
    /// - `~regex` - regex match on hostname
    /// - `*` - wildcard match
    pub fn get_hosts_for_pattern(&self, pattern: &str) -> InventoryResult<Vec<&Host>> {
        // Public mutable group access can bypass load/add_group validation.
        self.validate_group_graph()?;
        self.get_hosts_for_pattern_inner(pattern, 0)
    }

    /// Maximum recursion depth for pattern matching to prevent stack overflow
    /// with malformed or adversarial patterns containing many `:` characters.
    const MAX_PATTERN_DEPTH: usize = 20;

    fn get_hosts_for_pattern_inner(
        &self,
        pattern: &str,
        depth: usize,
    ) -> InventoryResult<Vec<&Host>> {
        if depth > Self::MAX_PATTERN_DEPTH {
            return Err(InventoryError::InvalidPattern(format!(
                "Pattern recursion depth exceeded (max {}): {}",
                Self::MAX_PATTERN_DEPTH,
                pattern
            )));
        }

        let pattern = pattern.trim();

        if pattern.is_empty() {
            return Ok(Vec::new());
        }

        // Handle "all"
        if pattern == "all" || pattern == "*" {
            return Ok(self.hosts.values().collect());
        }

        // Complex patterns: `:` outside brackets separates elements. A `:` inside
        // brackets belongs to a range such as `web[01:03]` and must not recurse here.
        if split_host_pattern(pattern).len() > 1 {
            return self.parse_complex_pattern(pattern, depth);
        }

        // `@path`: one host pattern per line, as in Ansible's `--limit @file`. Like
        // Ansible, this runs after splitting, so a path may not contain `:`.
        if let Some(path) = pattern.strip_prefix('@') {
            return self.get_hosts_from_limit_file(path, depth);
        }

        // Regexes keep their own bracket semantics: `~^node[1:3]$` is a character
        // class, so they dispatch before inventory-style ranges.
        if let Some(regex_str) = pattern.strip_prefix('~') {
            // OPTIMIZATION (Bolt): Use cached get_regex instead of Regex::new to prevent
            // expensive recompilation on every evaluation.
            let regex = crate::utils::get_regex(regex_str)
                .map_err(|_| InventoryError::InvalidPattern(pattern.to_string()))?;

            return Ok(self
                .hosts
                .values()
                .filter(|h| regex.is_match(&h.name))
                .collect());
        }

        // Inventory-style ranges come before globs: `web[01:03]` is not a character class.
        if let Some(hosts) = self.expand_range_pattern(pattern, depth)? {
            return Ok(hosts);
        }

        // Handle glob/wildcard pattern
        if pattern.contains('*') || pattern.contains('?') || pattern.contains('[') {
            let regex_pattern = glob_to_regex(pattern);
            // OPTIMIZATION (Bolt): Use cached get_regex instead of Regex::new to prevent
            // expensive recompilation of the converted glob pattern.
            let regex = crate::utils::get_regex(&regex_pattern)
                .map_err(|_| InventoryError::InvalidPattern(pattern.to_string()))?;

            return Ok(self
                .hosts
                .values()
                .filter(|h| regex.is_match(&h.name))
                .collect());
        }

        // Try as group name first
        if self.groups.contains_key(pattern) {
            return Ok(self.get_hosts_in_group(pattern));
        }

        // Try as host name
        if let Some(host) = self.hosts.get(pattern) {
            return Ok(vec![host]);
        }

        // Pattern didn't match anything
        Err(InventoryError::InvalidPattern(format!(
            "No hosts matched pattern: {}",
            pattern
        )))
    }

    /// Resolve `@path`: every non-empty line that does not start with `#` is a
    /// host pattern, and the result is their union.
    fn get_hosts_from_limit_file(&self, path: &str, depth: usize) -> InventoryResult<Vec<&Host>> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            InventoryError::InvalidPattern(format!("Cannot read limit file {}: {}", path, e))
        })?;
        let mut result: HashSet<&str> = HashSet::new();
        let mut patterns = 0usize;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            patterns += 1;
            for host in self.get_hosts_for_pattern_inner(line, depth + 1)? {
                result.insert(host.name.as_str());
            }
        }
        if patterns == 0 {
            return Err(InventoryError::InvalidPattern(format!(
                "Limit file contains no host patterns: {}",
                path
            )));
        }
        Ok(result
            .into_iter()
            .filter_map(|name| self.hosts.get(name))
            .collect())
    }

    /// Largest number of names a single range may expand to.
    const MAX_RANGE_EXPANSION: u64 = 10_000;

    /// Expand the first inventory-style range in `pattern` (`web[01:03]`,
    /// `node[a:c]`, `rack[1:9:2]`) into candidate names. Returns `None` when the
    /// pattern has no range. Zero padding follows the start bound. With
    /// `check_subscript`, text before the bracket that names a group is treated
    /// as an Ansible subscript and reported as unsupported rather than silently
    /// matching nothing; that check applies to the pattern as written, not to
    /// partially expanded candidates whose prefix may coincide with a group name.
    fn range_candidates(
        &self,
        pattern: &str,
        check_subscript: bool,
    ) -> InventoryResult<Option<Vec<String>>> {
        let regex = crate::utils::get_regex(
            r"^([^\[]*)\[([0-9]+|[A-Za-z]):([0-9]+|[A-Za-z])(?::([0-9]+))?\](.*)$",
        )
        .map_err(|_| InventoryError::InvalidPattern(pattern.to_string()))?;
        let Some(caps) = regex.captures(pattern) else {
            return Ok(None);
        };
        let (prefix, start, end, suffix) = (&caps[1], &caps[2], &caps[3], &caps[5]);
        if check_subscript && self.groups.contains_key(prefix) {
            return Err(InventoryError::InvalidPattern(format!(
                "Group subscripts are not supported: {}",
                pattern
            )));
        }
        let invalid = || InventoryError::InvalidPattern(format!("Invalid range: {}", pattern));
        let step: u64 = match caps.get(4) {
            Some(m) => m.as_str().parse().map_err(|_| invalid())?,
            None => 1,
        };
        if step == 0 {
            return Err(invalid());
        }
        let candidates = match (start.parse::<u64>(), end.parse::<u64>()) {
            (Ok(first), Ok(last)) => {
                if first > last || (last - first) / step >= Self::MAX_RANGE_EXPANSION {
                    return Err(invalid());
                }
                let width = if start.len() > 1 && start.starts_with('0') {
                    start.len()
                } else {
                    0
                };
                (first..=last)
                    .step_by(step as usize)
                    .map(|n| format!("{prefix}{n:0width$}{suffix}"))
                    .collect()
            }
            _ => {
                let (Some(first), Some(last)) = (start.chars().next(), end.chars().next()) else {
                    return Err(invalid());
                };
                if !(first.is_ascii_alphabetic() && last.is_ascii_alphabetic())
                    || first.is_ascii_lowercase() != last.is_ascii_lowercase()
                    || first > last
                {
                    return Err(invalid());
                }
                (first as u32..=last as u32)
                    .step_by(step as usize)
                    .filter_map(char::from_u32)
                    .map(|c| format!("{prefix}{c}{suffix}"))
                    .collect()
            }
        };
        Ok(Some(candidates))
    }

    /// Resolve one expanded candidate. Nested ranges are expanded in place,
    /// plain names that do not exist are skipped, and candidates that still
    /// carry a wildcard are collected into `globs` so the whole set is matched
    /// against the inventory in a single pass. `remaining` is the number of
    /// candidates the whole pattern may still produce, so nested ranges cannot
    /// multiply past `MAX_RANGE_EXPANSION`.
    fn collect_range_candidate<'a>(
        &'a self,
        candidate: &str,
        depth: usize,
        remaining: &mut u64,
        globs: &mut Vec<String>,
        result: &mut HashSet<&'a str>,
    ) -> InventoryResult<()> {
        if depth > Self::MAX_PATTERN_DEPTH {
            return Err(InventoryError::InvalidPattern(format!(
                "Pattern recursion depth exceeded (max {}): {}",
                Self::MAX_PATTERN_DEPTH,
                candidate
            )));
        }
        if *remaining == 0 {
            return Err(InventoryError::InvalidPattern(format!(
                "Invalid range: pattern expands to more than {} names: {}",
                Self::MAX_RANGE_EXPANSION,
                candidate
            )));
        }
        *remaining -= 1;
        if let Some(nested) = self.range_candidates(candidate, false)? {
            for name in &nested {
                self.collect_range_candidate(name, depth + 1, remaining, globs, result)?;
            }
        } else if candidate.contains(['*', '?', '[']) {
            globs.push(candidate.to_string());
        } else if let Some(host) = self.hosts.get(candidate) {
            result.insert(host.name.as_str());
        }
        Ok(())
    }

    /// Match every wildcard candidate left by range expansion in one pass:
    /// the globs become one alternation, compiled once and checked once per host.
    fn collect_glob_candidates<'a>(
        &'a self,
        pattern: &str,
        globs: &[String],
        result: &mut HashSet<&'a str>,
    ) -> InventoryResult<()> {
        if globs.is_empty() {
            return Ok(());
        }
        let alternation = globs
            .iter()
            .map(|glob| {
                let regex = glob_to_regex(glob);
                regex[1..regex.len() - 1].to_string()
            })
            .collect::<Vec<_>>()
            .join("|");
        let regex = regex::Regex::new(&format!("^(?:{alternation})$"))
            .map_err(|_| InventoryError::InvalidPattern(pattern.to_string()))?;
        for host in self.hosts.values() {
            if regex.is_match(&host.name) {
                result.insert(host.name.as_str());
            }
        }
        Ok(())
    }

    /// Expand and resolve an inventory-style range pattern. Returns `None` when
    /// the pattern has no range; members that do not exist are skipped, and a
    /// range matching nothing at all is an error.
    fn expand_range_pattern(
        &self,
        pattern: &str,
        depth: usize,
    ) -> InventoryResult<Option<Vec<&Host>>> {
        let Some(candidates) = self.range_candidates(pattern, true)? else {
            return Ok(None);
        };
        let mut result: HashSet<&str> = HashSet::new();
        let mut globs = Vec::new();
        let mut remaining = Self::MAX_RANGE_EXPANSION;
        for candidate in &candidates {
            self.collect_range_candidate(
                candidate,
                depth + 1,
                &mut remaining,
                &mut globs,
                &mut result,
            )?;
        }
        self.collect_glob_candidates(pattern, &globs, &mut result)?;
        if result.is_empty() {
            return Err(InventoryError::InvalidPattern(format!(
                "No hosts matched pattern: {}",
                pattern
            )));
        }
        Ok(Some(
            result
                .into_iter()
                .filter_map(|name| self.hosts.get(name))
                .collect(),
        ))
    }

    /// Parse a complex pattern with operators
    fn parse_complex_pattern(&self, pattern: &str, depth: usize) -> InventoryResult<Vec<&Host>> {
        let mut result: HashSet<&str> = HashSet::new();
        let mut first = true;

        // Split by : but not inside brackets
        let parts = split_host_pattern(pattern);

        for part in parts {
            let part = part.trim();

            if part.is_empty() {
                continue;
            }

            if let Some(sub_pattern) = part.strip_prefix('&') {
                // Intersection
                let sub_hosts = self.get_hosts_for_pattern_inner(sub_pattern, depth + 1)?;
                let sub_set: HashSet<&str> = sub_hosts.iter().map(|h| h.name.as_str()).collect();
                result = result.intersection(&sub_set).cloned().collect();
            } else if let Some(sub_pattern) = part.strip_prefix('!') {
                // Exclusion
                let sub_hosts = self.get_hosts_for_pattern_inner(sub_pattern, depth + 1)?;
                for host in sub_hosts {
                    result.remove(host.name.as_str());
                }
            } else {
                // Union
                let sub_hosts = self.get_hosts_for_pattern_inner(part, depth + 1)?;

                if first {
                    for host in sub_hosts {
                        result.insert(&host.name);
                    }
                    first = false;
                } else {
                    for host in sub_hosts {
                        result.insert(&host.name);
                    }
                }
            }
        }

        Ok(result
            .into_iter()
            .filter_map(|name| self.hosts.get(name))
            .collect())
    }

    /// Get all hosts in a group, including hosts from child groups
    fn get_hosts_in_group(&self, group_name: &str) -> Vec<&Host> {
        let mut hosts: HashSet<&str> = HashSet::new();
        let mut visited = HashSet::new();
        let mut pending = vec![group_name];
        while let Some(name) = pending.pop() {
            // Map keys define edges; public Group.name can be changed independently.
            if !visited.insert(name) {
                continue;
            }
            let Some(group) = self.groups.get(name) else {
                continue;
            };
            hosts.extend(group.hosts.iter().map(String::as_str));
            pending.extend(group.children.iter().map(String::as_str));
        }

        hosts
            .into_iter()
            .filter_map(|name| self.hosts.get(name))
            .collect()
    }

    /// Get the group hierarchy for a host (from most specific to least specific)
    pub fn get_host_group_hierarchy(&self, host: &Host) -> GroupHierarchy {
        let mut hierarchy = GroupHierarchy::new();
        let mut visited = HashSet::new();

        fn collect_parents(
            inventory: &Inventory,
            group_name: &str,
            hierarchy: &mut GroupHierarchy,
            visited: &mut HashSet<String>,
        ) {
            if visited.contains(group_name) {
                return;
            }
            visited.insert(group_name.to_string());
            hierarchy.push(group_name);

            if let Some(group) = inventory.groups.get(group_name) {
                for parent in &group.parents {
                    collect_parents(inventory, parent, hierarchy, visited);
                }
            }
        }

        // Helper to check if a group is an ancestor of another
        fn is_ancestor_of(
            inventory: &Inventory,
            potential_ancestor: &str,
            group: &str,
            visited: &mut HashSet<String>,
        ) -> bool {
            if visited.contains(group) {
                return false;
            }
            visited.insert(group.to_string());

            if let Some(g) = inventory.groups.get(group) {
                for parent in &g.parents {
                    if parent == potential_ancestor {
                        return true;
                    }
                    if is_ancestor_of(inventory, potential_ancestor, parent, visited) {
                        return true;
                    }
                }
            }
            false
        }

        // Filter host.groups to only include "leaf" groups (groups that are not
        // ancestors of any other group the host is in). This ensures we start
        // from the most specific groups and traverse up to parents.
        let host_groups: Vec<&String> = host.groups.iter().collect();
        let leaf_groups: Vec<&String> = host_groups
            .iter()
            .filter(|&group| {
                // A group is a "leaf" if no other group in host.groups has it as an ancestor
                !host_groups.iter().any(|other| {
                    if *other == *group {
                        return false;
                    }
                    let mut check_visited = HashSet::new();
                    is_ancestor_of(self, group, other, &mut check_visited)
                })
            })
            .copied()
            .collect();

        for group_name in leaf_groups {
            collect_parents(self, group_name, &mut hierarchy, &mut visited);
        }

        hierarchy
    }

    /// Get merged variables for a host (respecting group hierarchy)
    pub fn get_host_vars(&self, host: &Host) -> IndexMap<String, serde_yaml::Value> {
        let mut vars = IndexMap::new();

        // Get group hierarchy
        let hierarchy = self.get_host_group_hierarchy(host);

        // Apply variables from parent to child (so child overrides parent)
        for group_name in hierarchy.parent_to_child() {
            if let Some(group) = self.groups.get(group_name) {
                for (key, value) in &group.vars {
                    vars.insert(key.clone(), value.clone());
                }
            }
        }

        // Apply host-specific variables (highest precedence)
        for (key, value) in &host.vars {
            vars.insert(key.clone(), value.clone());
        }

        vars
    }

    /// Count total hosts
    pub fn host_count(&self) -> usize {
        self.hosts.len()
    }

    /// Count total groups
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
}

/// Split a host pattern on `:` outside brackets, so a range such as
/// `web[01:03]` or a regex class stays one element. Shared with the CLI so
/// its pre-validation sees the same elements the resolver does.
pub fn split_host_pattern(pattern: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut bracket_depth: usize = 0;

    for (i, ch) in pattern.char_indices() {
        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ':' if bracket_depth == 0 => {
                parts.push(&pattern[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    parts.push(&pattern[start..]);
    parts
}

/// Convert a glob pattern to regex
fn glob_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");

    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '[' | ']' | '(' | ')' | '{' | '}' | '.' | '+' | '^' | '$' | '|' | '\\' => {
                regex.push('\\');
                regex.push(ch);
            }
            _ => regex.push(ch),
        }
    }

    regex.push('$');
    regex
}

/// Report malformed YAML without reproducing potentially sensitive values.
fn invalid_yaml_structure(message: &str) -> InventoryError {
    InventoryError::Yaml(<serde_yaml::Error as serde::de::Error>::custom(message))
}

/// Validate group structure before interpreting it; null groups/sections are empty.
fn validate_yaml_group_shape(value: &serde_yaml::Value) -> InventoryResult<()> {
    if value.is_null() {
        return Ok(());
    }
    let group = value
        .as_mapping()
        .ok_or_else(|| invalid_yaml_structure("group definitions must be mappings or null"))?;
    for (key, section) in group {
        let name = key
            .as_str()
            .ok_or_else(|| invalid_yaml_structure("group section names must be strings"))?;
        if !matches!(name, "hosts" | "children" | "vars") {
            return Err(invalid_yaml_structure(
                "group sections must be hosts, children, or vars",
            ));
        }
        if section.is_null() {
            continue;
        }
        let entries = section
            .as_mapping()
            .ok_or_else(|| invalid_yaml_structure("group sections must be mappings or null"))?;
        for (entry_key, entry_value) in entries {
            let entry_name = entry_key
                .as_str()
                .ok_or_else(|| invalid_yaml_structure("inventory names must be strings"))?;
            if name == "hosts" && !entry_value.is_null() {
                let host_vars = entry_value.as_mapping().ok_or_else(|| {
                    invalid_yaml_structure("host variables must be mappings or null")
                })?;
                for var_key in host_vars.keys() {
                    if var_key.as_str().is_none() {
                        return Err(invalid_yaml_structure("variable names must be strings"));
                    }
                }
            }
            if name == "vars" && entry_name == "ansible_port" {
                parse_inventory_port(entry_value)?;
            }
        }
    }
    Ok(())
}

/// SSH destination ports are integer values in 1..=65535; numeric strings are accepted.
fn parse_inventory_port(value: &serde_yaml::Value) -> InventoryResult<u16> {
    let port = match value {
        serde_yaml::Value::Number(number) => {
            number.as_u64().and_then(|port| u16::try_from(port).ok())
        }
        serde_yaml::Value::String(port) => port.parse::<u16>().ok(),
        _ => None,
    };
    port.filter(|port| *port != 0).ok_or_else(|| {
        HostParseError::InvalidPort("expected an integer in 1..=65535".to_string()).into()
    })
}

/// Parse an INI group value, preserving numeric types and explicitly quoted strings.
fn parse_ini_value(value: &str) -> InventoryResult<serde_yaml::Value> {
    let value = value.trim();

    if value.starts_with('"') || value.starts_with('\'') {
        let words = shell_words::split(value).map_err(|_| {
            InventoryError::InvalidIniFormat("unmatched quote in group variable".to_string())
        })?;
        if words.len() != 1 {
            return Err(InventoryError::InvalidIniFormat(
                "quoted group variable must contain one value".to_string(),
            ));
        }
        return Ok(serde_yaml::Value::String(words[0].clone()));
    }

    match value.to_lowercase().as_str() {
        "true" | "yes" | "on" | "y" | "t" => return Ok(serde_yaml::Value::Bool(true)),
        "false" | "no" | "off" | "n" | "f" => return Ok(serde_yaml::Value::Bool(false)),
        _ => {}
    }

    if let Ok(number) = value.parse::<i64>() {
        return Ok(serde_yaml::Value::Number(number.into()));
    }
    if let Ok(number) = value.parse::<u64>() {
        return Ok(serde_yaml::Value::Number(number.into()));
    }
    if let Ok(number) = value.parse::<f64>() {
        if number.is_finite() {
            return Ok(serde_yaml::Value::Number(number.into()));
        }
    }
    Ok(serde_yaml::Value::String(value.to_string()))
}

/// Let serde preserve signed, unsigned, fractional, and nested JSON values.
fn json_to_yaml(value: &serde_json::Value) -> InventoryResult<serde_yaml::Value> {
    Ok(serde_yaml::to_value(value)?)
}

impl std::fmt::Display for Inventory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Inventory ({} hosts, {} groups)",
            self.hosts.len(),
            self.groups.len()
        )?;

        for group in self.groups.values() {
            if group.hosts.is_empty() && group.children.is_empty() {
                continue;
            }
            writeln!(f, "  [{}]", group.name)?;
            for host_name in &group.hosts {
                if let Some(host) = self.hosts.get(host_name) {
                    writeln!(f, "    {}", host)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_inventory() {
        let inv = Inventory::new();
        assert_eq!(inv.host_count(), 0);
        assert!(inv.groups.contains_key("all"));
        assert!(inv.groups.contains_key("ungrouped"));
    }

    #[test]
    fn test_add_host() {
        let mut inv = Inventory::new();
        let host = Host::new("webserver1");
        inv.add_host(host).unwrap();

        assert_eq!(inv.host_count(), 1);
        assert!(inv.get_host("webserver1").is_some());
    }

    #[test]
    fn test_parse_ini() {
        let mut inv = Inventory::new();
        inv.parse_ini(
            r#"
[webservers]
web1 ansible_host=10.0.0.1
web2 ansible_host=10.0.0.2

[databases]
db1 ansible_host=10.0.0.10

[webservers:vars]
http_port=80

[production:children]
webservers
databases
        "#,
        )
        .unwrap();

        assert_eq!(inv.host_count(), 3);
        assert!(inv.get_group("webservers").is_some());
        assert!(inv.get_group("databases").is_some());
        assert!(inv.get_group("production").is_some());

        let webservers = inv.get_group("webservers").unwrap();
        assert!(webservers.has_host("web1"));
        assert!(webservers.has_host("web2"));
        assert!(webservers.has_var("http_port"));
    }

    #[test]
    fn test_pattern_matching() {
        let mut inv = Inventory::new();
        inv.parse_ini(
            r#"
[webservers]
web1
web2

[databases]
db1
        "#,
        )
        .unwrap();

        let all = inv.get_hosts_for_pattern("all").unwrap();
        assert_eq!(all.len(), 3);

        let webs = inv.get_hosts_for_pattern("webservers").unwrap();
        assert_eq!(webs.len(), 2);

        let single = inv.get_hosts_for_pattern("web1").unwrap();
        assert_eq!(single.len(), 1);
    }

    #[test]
    fn test_glob_pattern() {
        let mut inv = Inventory::new();
        inv.add_host(Host::new("web1")).unwrap();
        inv.add_host(Host::new("web2")).unwrap();
        inv.add_host(Host::new("db1")).unwrap();

        let webs = inv.get_hosts_for_pattern("web*").unwrap();
        assert_eq!(webs.len(), 2);
    }

    fn range_inventory() -> Inventory {
        let mut inv = Inventory::new();
        inv.parse_ini(
            r#"
[webservers]
web01
web02
web03

[databases]
db01

[nodes]
nodea
nodeb
nodec
        "#,
        )
        .unwrap();
        inv
    }

    #[test]
    fn test_range_pattern_expands_inventory_style_ranges() {
        let inv = range_inventory();
        assert_eq!(inv.get_hosts_for_pattern("web[01:03]").unwrap().len(), 3);
        assert_eq!(inv.get_hosts_for_pattern("web[01:02]").unwrap().len(), 2);
        // Missing members are skipped; a range matching nothing is an error.
        assert_eq!(inv.get_hosts_for_pattern("web[01:05]").unwrap().len(), 3);
        assert!(inv.get_hosts_for_pattern("web[05:09]").is_err());
        // Zero padding follows the start bound; steps and letters work as in inventory files.
        assert!(inv.get_hosts_for_pattern("web[1:3]").is_err());
        assert_eq!(inv.get_hosts_for_pattern("web[01:03:2]").unwrap().len(), 2);
        assert_eq!(inv.get_hosts_for_pattern("node[a:c]").unwrap().len(), 3);
        // Ranges compose with the other operators and no longer recurse.
        assert_eq!(
            inv.get_hosts_for_pattern("web[01:03]:db01").unwrap().len(),
            4
        );
        assert_eq!(
            inv.get_hosts_for_pattern("web[01:03]:!web02")
                .unwrap()
                .len(),
            2
        );
        // Globs still work, brackets without a `:` stay literal as before, and
        // group subscripts are reported instead of matching nothing.
        assert_eq!(inv.get_hosts_for_pattern("web0*").unwrap().len(), 3);
        assert!(inv.get_hosts_for_pattern("web0[12]").unwrap().is_empty());
        assert!(inv.get_hosts_for_pattern("webservers[0:1]").is_err());
        assert!(inv.get_hosts_for_pattern("web[03:01]").is_err());
        assert!(inv.get_hosts_for_pattern("web[01:03:0]").is_err());
        // A regex keeps its own bracket semantics: `[1:3]` is a character class.
        assert_eq!(inv.get_hosts_for_pattern("~^web0[1:3]$").unwrap().len(), 2);
        // Wildcards left in expanded candidates are matched in one pass.
        assert_eq!(inv.get_hosts_for_pattern("web[01:02]*").unwrap().len(), 2);
        assert_eq!(inv.get_hosts_for_pattern("w[a:z]b0*").unwrap().len(), 3);
        assert!(inv.get_hosts_for_pattern("x[a:c]*").is_err());
    }

    #[test]
    fn test_nested_range_patterns_skip_empty_branches() {
        let mut inv = Inventory::new();
        // A group whose name coincides with a partially expanded prefix must not
        // turn the nested range into a "group subscript".
        inv.parse_ini("[rack1node]\nrack1node1\nrack1node2\n")
            .unwrap();
        assert_eq!(
            inv.get_hosts_for_pattern("rack[1:2]node[1:2]")
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            inv.get_hosts_for_pattern("rack[1:2]node[2:3]")
                .unwrap()
                .len(),
            1
        );
        assert!(inv.get_hosts_for_pattern("rack[3:4]node[1:2]").is_err());
        // The expansion budget is shared across nested ranges, and an
        // unparseable step is rejected instead of defaulting to one.
        let error = inv
            .get_hosts_for_pattern("rack[1:200]node[1:200]")
            .unwrap_err()
            .to_string();
        assert!(error.contains("more than"), "{error}");
        assert!(inv
            .get_hosts_for_pattern("rack[1:2]node[1:2:18446744073709551616]")
            .is_err());
    }

    #[test]
    fn test_limit_file_patterns() {
        use std::io::Write;
        let inv = range_inventory();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "# comment\n\nweb01\nweb[02:03]\n  db01  \n").unwrap();
        let pattern = format!("@{}", file.path().display());
        assert_eq!(inv.get_hosts_for_pattern(&pattern).unwrap().len(), 4);
        assert_eq!(
            inv.get_hosts_for_pattern(&format!("{pattern}:!db01"))
                .unwrap()
                .len(),
            3
        );
        let empty = tempfile::NamedTempFile::new().unwrap();
        let error = inv
            .get_hosts_for_pattern(&format!("@{}", empty.path().display()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("no host patterns"), "{error}");
        assert!(inv
            .get_hosts_for_pattern("@/nonexistent/limit-file")
            .is_err());
    }

    #[test]
    fn test_regex_pattern() {
        let mut inv = Inventory::new();
        inv.add_host(Host::new("web1")).unwrap();
        inv.add_host(Host::new("web2")).unwrap();
        inv.add_host(Host::new("db1")).unwrap();

        let webs = inv.get_hosts_for_pattern("~web\\d+").unwrap();
        assert_eq!(webs.len(), 2);
    }
}
