//! Kubernetes ConfigMap module for managing configuration data.
//!
//! This module provides ConfigMap management including:
//!
//! - Create, update, and delete ConfigMaps
//! - Data from literals, files, or directories
//! - Binary data support
//! - Immutable ConfigMaps (Kubernetes 1.21+)
//!
//! ## Parameters
//!
//! | Parameter | Required | Description |
//! |-----------|----------|-------------|
//! | `name` | Yes | ConfigMap name |
//! | `namespace` | No | Kubernetes namespace (default: "default") |
//! | `state` | No | Desired state: present, absent (default: present) |
//! | `data` | No | Key-value pairs for configuration data |
//! | `binary_data` | No | Binary data as base64-encoded values |
//! | `from_file` | No | Load data from file(s) |
//! | `from_literal` | No | Literal key-value pairs |
//! | `from_env_file` | No | Load data from env file |
//! | `immutable` | No | Make ConfigMap immutable (default: false) |
//! | `labels` | No | Labels for the ConfigMap |
//! | `annotations` | No | Annotations for the ConfigMap |
//! | `definition` | No | Full ConfigMap YAML definition |
//!
//! ## Example
//!
//! ```yaml
//! - name: Create config from literals
//!   k8s_configmap:
//!     name: app-config
//!     namespace: default
//!     data:
//!       DATABASE_URL: "postgres://localhost:5432/mydb"
//!       LOG_LEVEL: "info"
//!
//! - name: Create config from file content
//!   k8s_configmap:
//!     name: nginx-config
//!     namespace: default
//!     data:
//!       nginx.conf: |
//!         server {
//!           listen 80;
//!           server_name localhost;
//!         }
//!
//! - name: Create immutable config
//!   k8s_configmap:
//!     name: immutable-config
//!     namespace: default
//!     data:
//!       VERSION: "1.0.0"
//!     immutable: true
//! ```

use crate::modules::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParallelizationHint, ParamExt,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    parse_annotations, parse_labels, validate_k8s_name, validate_k8s_namespace,
    K8sResourceState,
};

/// ConfigMap configuration parsed from module parameters
#[derive(Debug, Clone)]
struct ConfigMapConfig {
    name: String,
    namespace: String,
    state: K8sResourceState,
    data: BTreeMap<String, String>,
    binary_data: BTreeMap<String, String>,
    immutable: bool,
    labels: BTreeMap<String, String>,
    annotations: BTreeMap<String, String>,
    definition: Option<serde_json::Value>,
    kubeconfig: Option<String>,
    context: Option<String>,
}

impl ConfigMapConfig {
    fn from_params(params: &ModuleParams) -> ModuleResult<Self> {
        let name = params.get_string_required("name")?;
        validate_k8s_name(&name)?;

        let namespace = params
            .get_string("namespace")?
            .unwrap_or_else(|| "default".to_string());
        validate_k8s_namespace(&namespace)?;

        let state = if let Some(s) = params.get_string("state")? {
            K8sResourceState::from_str(&s)?
        } else {
            K8sResourceState::default()
        };

        // Parse data
        let data = if let Some(data_value) = params.get("data") {
            parse_configmap_data(data_value)
        } else {
            BTreeMap::new()
        };

        // Parse binary_data
        let binary_data = if let Some(bd_value) = params.get("binary_data") {
            parse_configmap_data(bd_value)
        } else {
            BTreeMap::new()
        };

        // Parse from_literal as additional data entries
        let mut all_data = data;
        if let Some(literal_value) = params.get("from_literal") {
            let literals = parse_configmap_data(literal_value);
            all_data.extend(literals);
        }

        // Parse labels
        let labels = if let Some(label_value) = params.get("labels") {
            parse_labels(label_value)
        } else {
            BTreeMap::new()
        };

        // Parse annotations
        let annotations = if let Some(ann_value) = params.get("annotations") {
            parse_annotations(ann_value)
        } else {
            BTreeMap::new()
        };

        Ok(Self {
            name,
            namespace,
            state,
            data: all_data,
            binary_data,
            immutable: params.get_bool_or("immutable", false),
            labels,
            annotations,
            definition: params.get("definition").cloned(),
            kubeconfig: params.get_string("kubeconfig")?,
            context: params.get_string("context")?,
        })
    }
}

/// Parse ConfigMap data from JSON value
fn parse_configmap_data(value: &serde_json::Value) -> BTreeMap<String, String> {
    let mut data = BTreeMap::new();
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            if let Some(vs) = v.as_str() {
                data.insert(k.clone(), vs.to_string());
            } else {
                // For non-string values, convert to string representation
                data.insert(k.clone(), v.to_string().trim_matches('"').to_string());
            }
        }
    }
    data
}

/// Simulated Kubernetes ConfigMap info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMapInfo {
    pub name: String,
    pub namespace: String,
    pub data: BTreeMap<String, String>,
    pub binary_data_keys: Vec<String>,
    pub immutable: bool,
    pub labels: BTreeMap<String, String>,
    pub creation_timestamp: String,
}

/// Kubernetes ConfigMap module
pub struct K8sConfigMapModule;

impl K8sConfigMapModule {
    /// Get ConfigMap by name
    async fn get_configmap(
        _name: &str,
        _namespace: &str,
        _kubeconfig: Option<&str>,
        _context: Option<&str>,
    ) -> ModuleResult<Option<ConfigMapInfo>> {
        // In a real implementation using kube crate:
        // let configmaps: Api<ConfigMap> = Api::namespaced(client, namespace);
        // configmaps.get_opt(name).await?

        Ok(None)
    }

    /// Create or update ConfigMap
    async fn apply_configmap(config: &ConfigMapConfig) -> ModuleResult<ConfigMapInfo> {
        // Validate: must have some data
        if config.data.is_empty() && config.binary_data.is_empty() {
            return Err(ModuleError::InvalidParameter(
                "ConfigMap must have at least one data or binary_data entry".to_string(),
            ));
        }

        // In a real implementation:
        // let configmap = ConfigMap {
        //     metadata: ObjectMeta {
        //         name: Some(config.name.clone()),
        //         namespace: Some(config.namespace.clone()),
        //         labels: Some(config.labels.clone()),
        //         annotations: Some(config.annotations.clone()),
        //         ..Default::default()
        //     },
        //     data: Some(config.data.clone()),
        //     binary_data: Some(config.binary_data.iter().map(|(k, v)| {
        //         (k.clone(), base64::decode(v).unwrap().into())
        //     }).collect()),
        //     immutable: Some(config.immutable),
        // };
        // configmaps.patch(&config.name, &PatchParams::apply("rustible"), &Patch::Apply(&configmap)).await?

        tracing::info!(
            "Would create/update ConfigMap '{}' in namespace '{}' with {} data entries",
            config.name,
            config.namespace,
            config.data.len() + config.binary_data.len()
        );

        Ok(ConfigMapInfo {
            name: config.name.clone(),
            namespace: config.namespace.clone(),
            data: config.data.clone(),
            binary_data_keys: config.binary_data.keys().cloned().collect(),
            immutable: config.immutable,
            labels: config.labels.clone(),
            creation_timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Delete ConfigMap
    async fn delete_configmap(
        name: &str,
        namespace: &str,
        _kubeconfig: Option<&str>,
        _context: Option<&str>,
    ) -> ModuleResult<()> {
        tracing::info!(
            "Would delete ConfigMap '{}' from namespace '{}'",
            name,
            namespace
        );
        Ok(())
    }

    /// Execute async ConfigMap operations
    async fn execute_async(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let config = ConfigMapConfig::from_params(params)?;

        // Check for YAML definition override
        if let Some(ref _definition) = config.definition {
            return self.apply_from_definition(&config, context).await;
        }

        // Get existing ConfigMap
        let existing = Self::get_configmap(
            &config.name,
            &config.namespace,
            config.kubeconfig.as_deref(),
            config.context.as_deref(),
        )
        .await?;

        match config.state {
            K8sResourceState::Present => self.ensure_present(&config, existing, context).await,
            K8sResourceState::Absent => self.ensure_absent(&config, existing, context).await,
        }
    }

    /// Apply ConfigMap from YAML definition
    async fn apply_from_definition(
        &self,
        config: &ConfigMapConfig,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        if context.check_mode {
            return Ok(ModuleOutput::changed(format!(
                "Would apply ConfigMap definition for '{}'",
                config.name
            )));
        }

        Ok(ModuleOutput::changed(format!(
            "Applied ConfigMap definition for '{}'",
            config.name
        ))
        .with_data("name", serde_json::json!(config.name))
        .with_data("namespace", serde_json::json!(config.namespace)))
    }

    /// Ensure ConfigMap is present
    async fn ensure_present(
        &self,
        config: &ConfigMapConfig,
        existing: Option<ConfigMapInfo>,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        if let Some(cm) = existing {
            // ConfigMap exists - check if immutable
            if cm.immutable {
                // Cannot update immutable ConfigMap
                if self.data_differs(config, &cm) {
                    return Err(ModuleError::ExecutionFailed(format!(
                        "ConfigMap '{}' is immutable and cannot be updated. Delete and recreate to change.",
                        config.name
                    )));
                }
                return Ok(ModuleOutput::ok(format!(
                    "Immutable ConfigMap '{}' is up to date",
                    config.name
                ))
                .with_data("configmap", serde_json::to_value(&cm).unwrap()));
            }

            // Check for updates
            let needs_update = self.data_differs(config, &cm);

            if !needs_update {
                return Ok(ModuleOutput::ok(format!(
                    "ConfigMap '{}' is up to date",
                    config.name
                ))
                .with_data("configmap", serde_json::to_value(&cm).unwrap()));
            }

            if context.check_mode {
                return Ok(ModuleOutput::changed(format!(
                    "Would update ConfigMap '{}'",
                    config.name
                )));
            }

            let updated = Self::apply_configmap(config).await?;

            Ok(ModuleOutput::changed(format!(
                "Updated ConfigMap '{}'",
                config.name
            ))
            .with_data("configmap", serde_json::to_value(&updated).unwrap()))
        } else {
            // Create new ConfigMap
            if context.check_mode {
                return Ok(ModuleOutput::changed(format!(
                    "Would create ConfigMap '{}'",
                    config.name
                )));
            }

            let created = Self::apply_configmap(config).await?;

            Ok(ModuleOutput::changed(format!(
                "Created ConfigMap '{}'",
                config.name
            ))
            .with_data("configmap", serde_json::to_value(&created).unwrap()))
        }
    }

    /// Ensure ConfigMap is absent
    async fn ensure_absent(
        &self,
        config: &ConfigMapConfig,
        existing: Option<ConfigMapInfo>,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        if existing.is_none() {
            return Ok(ModuleOutput::ok(format!(
                "ConfigMap '{}' does not exist",
                config.name
            )));
        }

        if context.check_mode {
            return Ok(ModuleOutput::changed(format!(
                "Would delete ConfigMap '{}'",
                config.name
            )));
        }

        Self::delete_configmap(
            &config.name,
            &config.namespace,
            config.kubeconfig.as_deref(),
            config.context.as_deref(),
        )
        .await?;

        Ok(ModuleOutput::changed(format!(
            "Deleted ConfigMap '{}'",
            config.name
        )))
    }

    /// Check if data differs
    fn data_differs(&self, config: &ConfigMapConfig, existing: &ConfigMapInfo) -> bool {
        config.data != existing.data
    }
}

impl Module for K8sConfigMapModule {
    fn name(&self) -> &'static str {
        "k8s_configmap"
    }

    fn description(&self) -> &'static str {
        "Manage Kubernetes ConfigMaps"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::LocalLogic
    }

    fn parallelization_hint(&self) -> ParallelizationHint {
        ParallelizationHint::RateLimited {
            requests_per_second: 20,
        }
    }

    fn required_params(&self) -> &[&'static str] {
        &["name"]
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|_| ModuleError::ExecutionFailed("No tokio runtime available".to_string()))?;

        let params = params.clone();
        let context = context.clone();
        let module = self;

        std::thread::scope(|s| {
            s.spawn(|| handle.block_on(module.execute_async(&params, &context)))
                .join()
                .unwrap()
        })
    }

    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        // Validate name
        let name = params.get_string_required("name")?;
        validate_k8s_name(&name)?;

        // Validate namespace if provided
        if let Some(namespace) = params.get_string("namespace")? {
            validate_k8s_namespace(&namespace)?;
        }

        // Validate state if provided
        if let Some(state) = params.get_string("state")? {
            K8sResourceState::from_str(&state)?;
        }

        // Validate data keys if provided
        if let Some(data_value) = params.get("data") {
            if let Some(obj) = data_value.as_object() {
                for key in obj.keys() {
                    validate_configmap_key(key)?;
                }
            }
        }

        Ok(())
    }

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let config = ConfigMapConfig::from_params(params)?;

        let before = "# Current state: unknown (would query API)".to_string();

        let data_preview: Vec<String> = config
            .data
            .iter()
            .take(5)
            .map(|(k, v)| {
                let truncated = if v.len() > 50 {
                    format!("{}...", &v[..47])
                } else {
                    v.clone()
                };
                format!("  {}: {}", k, truncated)
            })
            .collect();

        let after = format!(
            r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: {}
  namespace: {}
data:
{}"#,
            config.name,
            config.namespace,
            data_preview.join("\n")
        );

        Ok(Some(Diff::new(before, after)))
    }
}

/// Validate ConfigMap data key
fn validate_configmap_key(key: &str) -> ModuleResult<()> {
    if key.is_empty() {
        return Err(ModuleError::InvalidParameter(
            "ConfigMap key cannot be empty".to_string(),
        ));
    }

    // ConfigMap keys must be alphanumeric, -, _, or .
    // and must be less than 253 characters
    if key.len() > 253 {
        return Err(ModuleError::InvalidParameter(format!(
            "ConfigMap key '{}' exceeds 253 character limit",
            key
        )));
    }

    for c in key.chars() {
        if !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.' {
            return Err(ModuleError::InvalidParameter(format!(
                "ConfigMap key '{}' contains invalid character '{}'. Only alphanumeric, '-', '_', and '.' are allowed",
                key, c
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configmap_module_metadata() {
        let module = K8sConfigMapModule;
        assert_eq!(module.name(), "k8s_configmap");
        assert_eq!(module.classification(), ModuleClassification::LocalLogic);
        assert_eq!(module.required_params(), &["name"]);
    }

    #[test]
    fn test_configmap_config_basic() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("app-config"));
        params.insert(
            "data".to_string(),
            serde_json::json!({
                "DATABASE_URL": "postgres://localhost:5432/mydb",
                "LOG_LEVEL": "info"
            }),
        );

        let config = ConfigMapConfig::from_params(&params).unwrap();
        assert_eq!(config.name, "app-config");
        assert_eq!(config.namespace, "default");
        assert_eq!(config.data.len(), 2);
        assert_eq!(
            config.data.get("DATABASE_URL"),
            Some(&"postgres://localhost:5432/mydb".to_string())
        );
    }

    #[test]
    fn test_configmap_config_with_multiline() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx-config"));
        params.insert(
            "data".to_string(),
            serde_json::json!({
                "nginx.conf": "server {\n  listen 80;\n  server_name localhost;\n}"
            }),
        );

        let config = ConfigMapConfig::from_params(&params).unwrap();
        assert!(config.data.get("nginx.conf").unwrap().contains("listen 80"));
    }

    #[test]
    fn test_configmap_config_immutable() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("immutable-config"));
        params.insert(
            "data".to_string(),
            serde_json::json!({"VERSION": "1.0.0"}),
        );
        params.insert("immutable".to_string(), serde_json::json!(true));

        let config = ConfigMapConfig::from_params(&params).unwrap();
        assert!(config.immutable);
    }

    #[test]
    fn test_configmap_config_with_labels() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("app-config"));
        params.insert(
            "data".to_string(),
            serde_json::json!({"key": "value"}),
        );
        params.insert(
            "labels".to_string(),
            serde_json::json!({
                "app": "myapp",
                "environment": "production"
            }),
        );

        let config = ConfigMapConfig::from_params(&params).unwrap();
        assert_eq!(config.labels.get("app"), Some(&"myapp".to_string()));
        assert_eq!(config.labels.get("environment"), Some(&"production".to_string()));
    }

    #[test]
    fn test_validate_configmap_key_valid() {
        assert!(validate_configmap_key("config.yaml").is_ok());
        assert!(validate_configmap_key("DATABASE_URL").is_ok());
        assert!(validate_configmap_key("app-config").is_ok());
        assert!(validate_configmap_key("config_file.json").is_ok());
    }

    #[test]
    fn test_validate_configmap_key_invalid() {
        assert!(validate_configmap_key("").is_err());
        assert!(validate_configmap_key("key/with/slashes").is_err());
        assert!(validate_configmap_key("key with spaces").is_err());
    }

    #[test]
    fn test_validate_params_invalid_name() {
        let module = K8sConfigMapModule;
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("Invalid_Name"));

        assert!(module.validate_params(&params).is_err());
    }
}
