//! Error types for Rustible.
//!
//! This module defines the error types used throughout Rustible, providing
//! rich error information for debugging and user feedback.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for Rustible operations.
pub type Result<T> = std::result::Result<T, Error>;

/// The main error type for Rustible.
#[derive(Error, Debug)]
pub enum Error {
    // ========================================================================
    // Playbook Errors
    // ========================================================================
    /// Error parsing a playbook file.
    #[error("Failed to parse playbook '{path}': {message}")]
    PlaybookParse {
        /// Path to the playbook file
        path: PathBuf,
        /// Error message
        message: String,
        /// Source error
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Error validating playbook structure.
    #[error("Playbook validation failed: {0}")]
    PlaybookValidation(String),

    /// Play not found.
    #[error("Play '{0}' not found in playbook")]
    PlayNotFound(String),

    // ========================================================================
    // Task Errors
    // ========================================================================
    /// Task execution failed.
    #[error("Task '{task}' failed on host '{host}': {message}")]
    TaskFailed {
        /// Task name
        task: String,
        /// Target host
        host: String,
        /// Error message
        message: String,
    },

    /// Task timeout.
    #[error("Task '{task}' timed out on host '{host}' after {timeout_secs} seconds")]
    TaskTimeout {
        /// Task name
        task: String,
        /// Target host
        host: String,
        /// Timeout in seconds
        timeout_secs: u64,
    },

    /// Task skipped due to condition.
    #[error("Task '{0}' skipped")]
    TaskSkipped(String),

    // ========================================================================
    // Module Errors
    // ========================================================================
    /// Module not found.
    #[error("Module '{0}' not found")]
    ModuleNotFound(String),

    /// Invalid module arguments.
    #[error("Invalid arguments for module '{module}': {message}")]
    ModuleArgs {
        /// Module name
        module: String,
        /// Error message
        message: String,
    },

    /// Module execution failed.
    #[error("Module '{module}' execution failed: {message}")]
    ModuleExecution {
        /// Module name
        module: String,
        /// Error message
        message: String,
    },

    // ========================================================================
    // Inventory Errors
    // ========================================================================
    /// Error loading inventory.
    #[error("Failed to load inventory from '{path}': {message}")]
    InventoryLoad {
        /// Path to inventory
        path: PathBuf,
        /// Error message
        message: String,
    },

    /// Host not found in inventory.
    #[error("Host '{0}' not found in inventory")]
    HostNotFound(String),

    /// Group not found in inventory.
    #[error("Group '{0}' not found in inventory")]
    GroupNotFound(String),

    /// Invalid host pattern.
    #[error("Invalid host pattern: '{0}'")]
    InvalidHostPattern(String),

    // ========================================================================
    // Connection Errors
    // ========================================================================
    /// Failed to connect to host.
    #[error("Failed to connect to '{host}': {message}")]
    ConnectionFailed {
        /// Target host
        host: String,
        /// Error message
        message: String,
    },

    /// Connection timeout.
    #[error("Connection to '{host}' timed out after {timeout_secs} seconds")]
    ConnectionTimeout {
        /// Target host
        host: String,
        /// Timeout in seconds
        timeout_secs: u64,
    },

    /// Authentication failed.
    #[error("Authentication failed for '{user}@{host}': {message}")]
    AuthenticationFailed {
        /// Username
        user: String,
        /// Target host
        host: String,
        /// Error message
        message: String,
    },

    /// Command execution failed on remote.
    #[error("Command failed on '{host}' with exit code {exit_code}: {message}")]
    RemoteCommandFailed {
        /// Target host
        host: String,
        /// Exit code
        exit_code: i32,
        /// Error message
        message: String,
    },

    /// File transfer failed.
    #[error("File transfer failed: {0}")]
    FileTransfer(String),

    // ========================================================================
    // Variable Errors
    // ========================================================================
    /// Undefined variable.
    #[error("Undefined variable: '{0}'")]
    UndefinedVariable(String),

    /// Invalid variable value.
    #[error("Invalid value for variable '{name}': {message}")]
    InvalidVariableValue {
        /// Variable name
        name: String,
        /// Error message
        message: String,
    },

    /// Variable file not found.
    #[error("Variables file not found: {0}")]
    VariablesFileNotFound(PathBuf),

    // ========================================================================
    // Template Errors
    // ========================================================================
    /// Template syntax error.
    #[error("Template syntax error in '{template}': {message}")]
    TemplateSyntax {
        /// Template name or path
        template: String,
        /// Error message
        message: String,
    },

    /// Template rendering error.
    #[error("Template rendering failed for '{template}': {message}")]
    TemplateRender {
        /// Template name or path
        template: String,
        /// Error message
        message: String,
    },

    // ========================================================================
    // Role Errors
    // ========================================================================
    /// Role not found.
    #[error("Role '{0}' not found")]
    RoleNotFound(String),

    /// Role dependency error.
    #[error("Role dependency error: {0}")]
    RoleDependency(String),

    /// Invalid role structure.
    #[error("Invalid role structure in '{role}': {message}")]
    InvalidRole {
        /// Role name
        role: String,
        /// Error message
        message: String,
    },

    // ========================================================================
    // Vault Errors
    // ========================================================================
    /// Vault decryption failed.
    #[error("Failed to decrypt vault: {0}")]
    VaultDecryption(String),

    /// Vault encryption failed.
    #[error("Failed to encrypt vault: {0}")]
    VaultEncryption(String),

    /// Invalid vault password.
    #[error("Invalid vault password")]
    InvalidVaultPassword,

    /// Vault file not found.
    #[error("Vault file not found: {0}")]
    VaultFileNotFound(PathBuf),

    // ========================================================================
    // Handler Errors
    // ========================================================================
    /// Handler not found.
    #[error("Handler '{0}' not found")]
    HandlerNotFound(String),

    /// Handler execution failed.
    #[error("Handler '{handler}' failed on host '{host}': {message}")]
    HandlerFailed {
        /// Handler name
        handler: String,
        /// Target host
        host: String,
        /// Error message
        message: String,
    },

    // ========================================================================
    // Configuration Errors
    // ========================================================================
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid configuration value.
    #[error("Invalid configuration value for '{key}': {message}")]
    InvalidConfig {
        /// Configuration key
        key: String,
        /// Error message
        message: String,
    },

    // ========================================================================
    // IO Errors
    // ========================================================================
    /// File not found.
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // ========================================================================
    // Serialization Errors
    // ========================================================================
    /// YAML parsing error.
    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    /// JSON parsing error.
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// TOML parsing error.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Template error.
    #[error("Template error: {0}")]
    Template(#[from] minijinja::Error),

    /// Generic vault error.
    #[error("Vault error: {0}")]
    Vault(String),

    // ========================================================================
    // Other Errors
    // ========================================================================
    /// Strategy error.
    #[error("Execution strategy error: {0}")]
    Strategy(String),

    /// Privilege escalation failed.
    #[error("Privilege escalation failed on '{host}': {message}")]
    BecomeError {
        /// Target host
        host: String,
        /// Error message
        message: String,
    },

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Generic error with source.
    #[error("{message}")]
    Other {
        /// Error message
        message: String,
        /// Source error
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl Error {
    /// Creates a new playbook parse error.
    pub fn playbook_parse(
        path: impl Into<PathBuf>,
        message: impl Into<String>,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self::PlaybookParse {
            path: path.into(),
            message: message.into(),
            source,
        }
    }

    /// Creates a new task failed error.
    pub fn task_failed(
        task: impl Into<String>,
        host: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::TaskFailed {
            task: task.into(),
            host: host.into(),
            message: message.into(),
        }
    }

    /// Creates a new connection failed error.
    pub fn connection_failed(host: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ConnectionFailed {
            host: host.into(),
            message: message.into(),
        }
    }

    /// Creates a new module args error.
    pub fn module_args(module: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ModuleArgs {
            module: module.into(),
            message: message.into(),
        }
    }

    /// Creates a new template render error.
    pub fn template_render(template: impl Into<String>, message: impl Into<String>) -> Self {
        Self::TemplateRender {
            template: template.into(),
            message: message.into(),
        }
    }

    /// Returns true if this error is recoverable.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::TaskSkipped(_)
                | Error::ConnectionTimeout { .. }
                | Error::TaskTimeout { .. }
        )
    }

    /// Returns the error code for CLI exit status.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::TaskFailed { .. } | Error::ModuleExecution { .. } => 2,
            Error::ConnectionFailed { .. } | Error::AuthenticationFailed { .. } => 3,
            Error::PlaybookParse { .. } | Error::PlaybookValidation(_) => 4,
            Error::InventoryLoad { .. } | Error::HostNotFound(_) => 5,
            Error::VaultDecryption(_) | Error::InvalidVaultPassword => 6,
            _ => 1,
        }
    }
}

/// Extension trait for adding context to errors.
pub trait ErrorContext<T> {
    /// Adds context to an error.
    fn context(self, message: impl Into<String>) -> Result<T>;

    /// Adds context with a closure that is only evaluated on error.
    fn with_context<F, S>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<String>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context(self, message: impl Into<String>) -> Result<T> {
        self.map_err(|e| Error::Other {
            message: message.into(),
            source: Some(Box::new(e)),
        })
    }

    fn with_context<F, S>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<String>,
    {
        self.map_err(|e| Error::Other {
            message: f().into(),
            source: Some(Box::new(e)),
        })
    }
}
