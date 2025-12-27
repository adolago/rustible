//! Error types for the notification system.

use std::fmt;

/// Result type for notification operations.
pub type NotificationResult<T> = Result<T, NotificationError>;

/// Errors that can occur during notification operations.
#[derive(Debug)]
pub enum NotificationError {
    /// Configuration error.
    Config(String),

    /// Network/connection error.
    Network(String),

    /// HTTP request error.
    Http {
        /// HTTP status code
        status: Option<u16>,
        /// Error message
        message: String,
    },

    /// SMTP/email error.
    Smtp(String),

    /// Template rendering error.
    Template(String),

    /// Serialization error.
    Serialization(String),

    /// Timeout error.
    Timeout(String),

    /// Filter rejected the notification.
    Filtered(String),

    /// Backend not configured.
    NotConfigured(String),

    /// Generic internal error.
    Internal(String),
}

impl NotificationError {
    /// Creates a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    /// Creates a network error.
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network(message.into())
    }

    /// Creates an HTTP error.
    pub fn http(status: Option<u16>, message: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: message.into(),
        }
    }

    /// Creates an SMTP error.
    pub fn smtp(message: impl Into<String>) -> Self {
        Self::Smtp(message.into())
    }

    /// Creates a template error.
    pub fn template(message: impl Into<String>) -> Self {
        Self::Template(message.into())
    }

    /// Creates a serialization error.
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::Serialization(message.into())
    }

    /// Creates a timeout error.
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout(message.into())
    }

    /// Creates a filtered error.
    pub fn filtered(message: impl Into<String>) -> Self {
        Self::Filtered(message.into())
    }

    /// Creates a not configured error.
    pub fn not_configured(backend: impl Into<String>) -> Self {
        Self::NotConfigured(format!("{} backend not configured", backend.into()))
    }

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Returns true if this is a recoverable error.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Timeout(_) | Self::Http { .. }
        )
    }

    /// Returns true if this is a configuration error.
    pub fn is_config_error(&self) -> bool {
        matches!(self, Self::Config(_) | Self::NotConfigured(_))
    }
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "Configuration error: {}", msg),
            Self::Network(msg) => write!(f, "Network error: {}", msg),
            Self::Http { status, message } => {
                if let Some(code) = status {
                    write!(f, "HTTP error ({}): {}", code, message)
                } else {
                    write!(f, "HTTP error: {}", message)
                }
            }
            Self::Smtp(msg) => write!(f, "SMTP error: {}", msg),
            Self::Template(msg) => write!(f, "Template error: {}", msg),
            Self::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            Self::Timeout(msg) => write!(f, "Timeout: {}", msg),
            Self::Filtered(msg) => write!(f, "Notification filtered: {}", msg),
            Self::NotConfigured(msg) => write!(f, "{}", msg),
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for NotificationError {}

impl From<reqwest::Error> for NotificationError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout(err.to_string())
        } else if err.is_connect() {
            Self::Network(err.to_string())
        } else if err.is_status() {
            Self::Http {
                status: err.status().map(|s| s.as_u16()),
                message: err.to_string(),
            }
        } else {
            Self::Network(err.to_string())
        }
    }
}

impl From<serde_json::Error> for NotificationError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for NotificationError {
    fn from(err: std::io::Error) -> Self {
        Self::Network(err.to_string())
    }
}

impl From<minijinja::Error> for NotificationError {
    fn from(err: minijinja::Error) -> Self {
        Self::Template(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = NotificationError::config("missing webhook URL");
        assert_eq!(err.to_string(), "Configuration error: missing webhook URL");

        let err = NotificationError::http(Some(404), "Not found");
        assert_eq!(err.to_string(), "HTTP error (404): Not found");

        let err = NotificationError::http(None, "Connection failed");
        assert_eq!(err.to_string(), "HTTP error: Connection failed");
    }

    #[test]
    fn test_is_recoverable() {
        assert!(NotificationError::network("connection refused").is_recoverable());
        assert!(NotificationError::timeout("request timed out").is_recoverable());
        assert!(NotificationError::http(Some(503), "Service unavailable").is_recoverable());
        assert!(!NotificationError::config("missing field").is_recoverable());
        assert!(!NotificationError::template("syntax error").is_recoverable());
    }

    #[test]
    fn test_is_config_error() {
        assert!(NotificationError::config("missing field").is_config_error());
        assert!(NotificationError::not_configured("Slack").is_config_error());
        assert!(!NotificationError::network("failed").is_config_error());
    }
}
