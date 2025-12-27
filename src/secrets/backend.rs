//! Secret backend trait and types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::error::SecretResult;
use super::types::Secret;

/// Supported secret backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecretBackendType {
    /// HashiCorp Vault
    #[default]
    Vault,
    /// AWS Secrets Manager
    AwsSecretsManager,
}

impl fmt::Display for SecretBackendType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecretBackendType::Vault => write!(f, "vault"),
            SecretBackendType::AwsSecretsManager => write!(f, "aws_secrets_manager"),
        }
    }
}

impl std::str::FromStr for SecretBackendType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "vault" | "hashicorp_vault" | "hashicorp-vault" => Ok(SecretBackendType::Vault),
            "aws" | "aws_secrets_manager" | "aws-secrets-manager" | "secretsmanager" => {
                Ok(SecretBackendType::AwsSecretsManager)
            }
            _ => Err(format!("Unknown secret backend type: {}", s)),
        }
    }
}

/// Trait for secret backend implementations.
///
/// This trait defines the interface that all secret backends must implement.
/// Implementations include HashiCorp Vault and AWS Secrets Manager.
#[async_trait]
pub trait SecretBackend: Send + Sync {
    /// Get the backend type.
    fn backend_type(&self) -> SecretBackendType;

    /// Get a secret by path.
    ///
    /// # Arguments
    ///
    /// * `path` - The secret path (format depends on backend)
    ///
    /// # Returns
    ///
    /// The secret with all its key-value pairs.
    async fn get_secret(&self, path: &str) -> SecretResult<Secret>;

    /// Get a specific version of a secret.
    ///
    /// # Arguments
    ///
    /// * `path` - The secret path
    /// * `version` - The version to retrieve
    ///
    /// # Returns
    ///
    /// The secret at the specified version.
    async fn get_secret_version(&self, path: &str, version: &str) -> SecretResult<Secret>;

    /// List secrets at a path.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to list secrets under
    ///
    /// # Returns
    ///
    /// A list of secret names/paths.
    async fn list_secrets(&self, path: &str) -> SecretResult<Vec<String>>;

    /// Write a secret.
    ///
    /// # Arguments
    ///
    /// * `path` - The secret path
    /// * `secret` - The secret data to write
    async fn put_secret(&self, path: &str, secret: &Secret) -> SecretResult<()>;

    /// Delete a secret.
    ///
    /// # Arguments
    ///
    /// * `path` - The secret path to delete
    async fn delete_secret(&self, path: &str) -> SecretResult<()>;

    /// Check if the backend is healthy and authenticated.
    ///
    /// # Returns
    ///
    /// `true` if the backend is reachable and authenticated.
    async fn health_check(&self) -> SecretResult<bool>;

    /// Get the backend name for logging/debugging.
    fn name(&self) -> &str {
        match self.backend_type() {
            SecretBackendType::Vault => "HashiCorp Vault",
            SecretBackendType::AwsSecretsManager => "AWS Secrets Manager",
        }
    }
}

/// Capabilities that a secret backend may support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendCapability {
    /// Can list secrets at a path
    List,
    /// Supports versioning
    Versioning,
    /// Supports automatic rotation
    Rotation,
    /// Supports soft delete (recovery possible)
    SoftDelete,
    /// Supports metadata on secrets
    Metadata,
    /// Supports binary data
    BinaryData,
    /// Supports generating secrets
    Generation,
}

/// Extension trait for checking backend capabilities.
pub trait BackendCapabilities {
    /// Get all supported capabilities.
    fn capabilities(&self) -> Vec<BackendCapability>;

    /// Check if a specific capability is supported.
    fn supports(&self, capability: BackendCapability) -> bool {
        self.capabilities().contains(&capability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_display() {
        assert_eq!(format!("{}", SecretBackendType::Vault), "vault");
        assert_eq!(
            format!("{}", SecretBackendType::AwsSecretsManager),
            "aws_secrets_manager"
        );
    }

    #[test]
    fn test_backend_type_from_str() {
        assert_eq!(
            "vault".parse::<SecretBackendType>().unwrap(),
            SecretBackendType::Vault
        );
        assert_eq!(
            "aws".parse::<SecretBackendType>().unwrap(),
            SecretBackendType::AwsSecretsManager
        );
        assert_eq!(
            "aws_secrets_manager".parse::<SecretBackendType>().unwrap(),
            SecretBackendType::AwsSecretsManager
        );
        assert!("unknown".parse::<SecretBackendType>().is_err());
    }
}
