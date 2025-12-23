//! Host definition for Rustible inventory system.
//!
//! This module provides the `Host` structure representing a managed node
//! with connection parameters, variables, and group membership.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Connection type for a host
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    /// SSH connection (default)
    #[default]
    Ssh,
    /// Local connection (no SSH)
    Local,
    /// Docker container connection
    Docker,
    /// Podman container connection
    Podman,
    /// WinRM connection for Windows hosts
    Winrm,
}

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionType::Ssh => write!(f, "ssh"),
            ConnectionType::Local => write!(f, "local"),
            ConnectionType::Docker => write!(f, "docker"),
            ConnectionType::Podman => write!(f, "podman"),
            ConnectionType::Winrm => write!(f, "winrm"),
        }
    }
}

/// SSH connection parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SshParams {
    /// SSH port (default: 22)
    #[serde(default = "default_ssh_port")]
    pub port: u16,

    /// SSH user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// SSH private key file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_file: Option<String>,

    /// SSH password (discouraged, use keys)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// SSH common args
    #[serde(skip_serializing_if = "Option::is_none")]
    pub common_args: Option<String>,

    /// SSH extra args
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_args: Option<String>,

    /// SSH executable path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,

    /// SSH pipelining enabled
    #[serde(default)]
    pub pipelining: bool,

    /// SSH host key checking
    #[serde(default = "default_host_key_checking")]
    pub host_key_checking: bool,

    /// SSH connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u32,
}

fn default_ssh_port() -> u16 {
    22
}

fn default_host_key_checking() -> bool {
    true
}

fn default_timeout() -> u32 {
    10
}

impl Default for SshParams {
    fn default() -> Self {
        Self {
            port: default_ssh_port(),
            user: None,
            private_key_file: None,
            password: None,
            common_args: None,
            extra_args: None,
            executable: None,
            pipelining: false,
            host_key_checking: default_host_key_checking(),
            timeout: default_timeout(),
        }
    }
}

/// Connection parameters for a host
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionParams {
    /// Connection type
    #[serde(default)]
    pub connection: ConnectionType,

    /// SSH-specific parameters
    #[serde(flatten)]
    pub ssh: SshParams,

    /// Become (privilege escalation) enabled
    #[serde(default)]
    pub r#become: bool,

    /// Become method (sudo, su, etc.)
    #[serde(default = "default_become_method")]
    pub become_method: String,

    /// Become user
    #[serde(default = "default_become_user")]
    pub become_user: String,

    /// Become password
    #[serde(skip_serializing_if = "Option::is_none")]
    pub become_password: Option<String>,

    /// Python interpreter path on remote host
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python_interpreter: Option<String>,

    /// Shell executable on remote host
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_executable: Option<String>,
}

fn default_become_method() -> String {
    "sudo".to_string()
}

fn default_become_user() -> String {
    "root".to_string()
}

impl Default for ConnectionParams {
    fn default() -> Self {
        Self {
            connection: ConnectionType::default(),
            ssh: SshParams::default(),
            r#become: false,
            become_method: default_become_method(),
            become_user: default_become_user(),
            become_password: None,
            python_interpreter: None,
            shell_executable: None,
        }
    }
}

/// A managed host in the inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    /// Host name (can be hostname, IP, or alias)
    pub name: String,

    /// Actual hostname or IP to connect to (if different from name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ansible_host: Option<String>,

    /// Connection parameters
    #[serde(flatten)]
    pub connection: ConnectionParams,

    /// Host-specific variables
    #[serde(default)]
    pub vars: IndexMap<String, serde_yaml::Value>,

    /// Groups this host belongs to
    #[serde(skip)]
    pub groups: HashSet<String>,

    /// Whether the host is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Host {
    /// Create a new host with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ansible_host: None,
            connection: ConnectionParams::default(),
            vars: IndexMap::new(),
            groups: HashSet::new(),
            enabled: true,
        }
    }

    /// Create a new host with hostname/IP
    pub fn with_address(name: impl Into<String>, address: impl Into<String>) -> Self {
        let mut host = Self::new(name);
        host.ansible_host = Some(address.into());
        host
    }

    /// Get the actual address to connect to
    pub fn address(&self) -> &str {
        self.ansible_host.as_deref().unwrap_or(&self.name)
    }

    /// Set a variable on this host
    pub fn set_var(&mut self, key: impl Into<String>, value: serde_yaml::Value) {
        self.vars.insert(key.into(), value);
    }

    /// Get a variable from this host
    pub fn get_var(&self, key: &str) -> Option<&serde_yaml::Value> {
        self.vars.get(key)
    }

    /// Check if host has a specific variable
    pub fn has_var(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    /// Add this host to a group
    pub fn add_to_group(&mut self, group: impl Into<String>) {
        self.groups.insert(group.into());
    }

    /// Remove this host from a group
    pub fn remove_from_group(&mut self, group: &str) {
        self.groups.remove(group);
    }

    /// Check if host belongs to a specific group
    pub fn in_group(&self, group: &str) -> bool {
        self.groups.contains(group)
    }

    /// Set SSH port
    pub fn set_port(&mut self, port: u16) {
        self.connection.ssh.port = port;
    }

    /// Set SSH user
    pub fn set_user(&mut self, user: impl Into<String>) {
        self.connection.ssh.user = Some(user.into());
    }

    /// Set SSH private key file
    pub fn set_private_key(&mut self, key_file: impl Into<String>) {
        self.connection.ssh.private_key_file = Some(key_file.into());
    }

    /// Enable privilege escalation (become)
    pub fn enable_become(&mut self) {
        self.connection.r#become = true;
    }

    /// Set become method
    pub fn set_become_method(&mut self, method: impl Into<String>) {
        self.connection.become_method = method.into();
    }

    /// Set become user
    pub fn set_become_user(&mut self, user: impl Into<String>) {
        self.connection.become_user = user.into();
    }

    /// Set connection type
    pub fn set_connection(&mut self, conn: ConnectionType) {
        self.connection.connection = conn;
    }

    /// Merge variables from another source (other takes precedence)
    pub fn merge_vars(&mut self, other: &IndexMap<String, serde_yaml::Value>) {
        for (key, value) in other {
            self.vars.insert(key.clone(), value.clone());
        }
    }

    /// Parse host definition from string (e.g., "host1 ansible_host=192.168.1.1 ansible_port=22")
    pub fn parse(input: &str) -> Result<Self, HostParseError> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Err(HostParseError::EmptyInput);
        }

        let name = parts[0].to_string();
        let mut host = Host::new(name);

        for part in &parts[1..] {
            if let Some((key, value)) = part.split_once('=') {
                match key {
                    "ansible_host" => host.ansible_host = Some(value.to_string()),
                    "ansible_port" => {
                        host.connection.ssh.port = value
                            .parse()
                            .map_err(|_| HostParseError::InvalidPort(value.to_string()))?;
                    }
                    "ansible_user" => host.connection.ssh.user = Some(value.to_string()),
                    "ansible_ssh_private_key_file" => {
                        host.connection.ssh.private_key_file = Some(value.to_string())
                    }
                    "ansible_ssh_pass" => host.connection.ssh.password = Some(value.to_string()),
                    "ansible_connection" => {
                        host.connection.connection = match value.to_lowercase().as_str() {
                            "ssh" => ConnectionType::Ssh,
                            "local" => ConnectionType::Local,
                            "docker" => ConnectionType::Docker,
                            "podman" => ConnectionType::Podman,
                            "winrm" => ConnectionType::Winrm,
                            _ => {
                                return Err(HostParseError::InvalidConnectionType(
                                    value.to_string(),
                                ))
                            }
                        };
                    }
                    "ansible_become" => {
                        host.connection.r#become = value.to_lowercase() == "true" || value == "1"
                    }
                    "ansible_become_method" => host.connection.become_method = value.to_string(),
                    "ansible_become_user" => host.connection.become_user = value.to_string(),
                    "ansible_python_interpreter" => {
                        host.connection.python_interpreter = Some(value.to_string())
                    }
                    _ => {
                        // Store as generic variable
                        host.vars.insert(
                            key.to_string(),
                            serde_yaml::Value::String(value.to_string()),
                        );
                    }
                }
            }
        }

        Ok(host)
    }
}

impl PartialEq for Host {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Host {}

impl std::hash::Hash for Host {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl std::fmt::Display for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(addr) = &self.ansible_host {
            write!(f, " ({})", addr)?;
        }
        Ok(())
    }
}

/// Errors that can occur when parsing a host definition
#[derive(Debug, thiserror::Error)]
pub enum HostParseError {
    #[error("empty input")]
    EmptyInput,
    #[error("invalid port: {0}")]
    InvalidPort(String),
    #[error("invalid connection type: {0}")]
    InvalidConnectionType(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_new() {
        let host = Host::new("webserver1");
        assert_eq!(host.name, "webserver1");
        assert_eq!(host.address(), "webserver1");
        assert!(host.enabled);
    }

    #[test]
    fn test_host_with_address() {
        let host = Host::with_address("webserver1", "192.168.1.10");
        assert_eq!(host.name, "webserver1");
        assert_eq!(host.address(), "192.168.1.10");
    }

    #[test]
    fn test_host_parse() {
        let host =
            Host::parse("web1 ansible_host=10.0.0.1 ansible_port=2222 ansible_user=admin").unwrap();
        assert_eq!(host.name, "web1");
        assert_eq!(host.address(), "10.0.0.1");
        assert_eq!(host.connection.ssh.port, 2222);
        assert_eq!(host.connection.ssh.user, Some("admin".to_string()));
    }

    #[test]
    fn test_host_groups() {
        let mut host = Host::new("test");
        host.add_to_group("webservers");
        host.add_to_group("production");
        assert!(host.in_group("webservers"));
        assert!(host.in_group("production"));
        assert!(!host.in_group("databases"));
        host.remove_from_group("webservers");
        assert!(!host.in_group("webservers"));
    }

    #[test]
    fn test_host_vars() {
        let mut host = Host::new("test");
        host.set_var("http_port", serde_yaml::Value::Number(80.into()));
        assert!(host.has_var("http_port"));
        assert_eq!(
            host.get_var("http_port"),
            Some(&serde_yaml::Value::Number(80.into()))
        );
    }
}
