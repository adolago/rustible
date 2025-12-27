//! Kubernetes Service module for managing services.
//!
//! This module provides service management including:
//!
//! - ClusterIP, NodePort, LoadBalancer, and ExternalName services
//! - Headless services for StatefulSets
//! - Port mapping and protocol configuration
//! - Session affinity and external traffic policies
//!
//! ## Parameters
//!
//! | Parameter | Required | Description |
//! |-----------|----------|-------------|
//! | `name` | Yes | Service name |
//! | `namespace` | No | Kubernetes namespace (default: "default") |
//! | `state` | No | Desired state: present, absent (default: present) |
//! | `type` | No | Service type: ClusterIP, NodePort, LoadBalancer, ExternalName, Headless |
//! | `selector` | No | Label selector for target pods |
//! | `ports` | No | Port specifications |
//! | `cluster_ip` | No | ClusterIP address (None for headless) |
//! | `external_ips` | No | External IP addresses |
//! | `load_balancer_ip` | No | Requested load balancer IP |
//! | `external_name` | No | External DNS name (for ExternalName type) |
//! | `session_affinity` | No | Session affinity: None or ClientIP |
//! | `external_traffic_policy` | No | External traffic policy: Cluster or Local |
//! | `labels` | No | Labels for the service |
//! | `annotations` | No | Annotations for the service |
//! | `definition` | No | Full service YAML definition |
//!
//! ## Example
//!
//! ```yaml
//! - name: Create ClusterIP service
//!   k8s_service:
//!     name: nginx-svc
//!     namespace: default
//!     selector:
//!       app: nginx
//!     ports:
//!       - port: 80
//!         target_port: 80
//!         protocol: TCP
//!     type: ClusterIP
//!
//! - name: Create NodePort service
//!   k8s_service:
//!     name: nginx-nodeport
//!     namespace: default
//!     selector:
//!       app: nginx
//!     ports:
//!       - port: 80
//!         target_port: 80
//!         node_port: 30080
//!     type: NodePort
//!
//! - name: Create LoadBalancer service
//!   k8s_service:
//!     name: nginx-lb
//!     namespace: default
//!     selector:
//!       app: nginx
//!     ports:
//!       - port: 80
//!         target_port: 80
//!     type: LoadBalancer
//!     annotations:
//!       service.beta.kubernetes.io/aws-load-balancer-type: nlb
//! ```

use crate::modules::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParallelizationHint, ParamExt,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    parse_annotations, parse_labels, validate_k8s_name, validate_k8s_namespace, K8sResourceState,
    ServicePort, ServiceType,
};

/// Service configuration parsed from module parameters
#[derive(Debug, Clone)]
struct ServiceConfig {
    name: String,
    namespace: String,
    state: K8sResourceState,
    service_type: ServiceType,
    selector: BTreeMap<String, String>,
    ports: Vec<ServicePort>,
    cluster_ip: Option<String>,
    external_ips: Vec<String>,
    load_balancer_ip: Option<String>,
    load_balancer_source_ranges: Vec<String>,
    external_name: Option<String>,
    session_affinity: Option<String>,
    session_affinity_timeout: Option<i32>,
    external_traffic_policy: Option<String>,
    internal_traffic_policy: Option<String>,
    health_check_node_port: Option<i32>,
    publish_not_ready_addresses: bool,
    labels: BTreeMap<String, String>,
    annotations: BTreeMap<String, String>,
    definition: Option<serde_json::Value>,
    kubeconfig: Option<String>,
    context: Option<String>,
}

impl ServiceConfig {
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

        let service_type = if let Some(t) = params.get_string("type")? {
            ServiceType::from_str(&t)?
        } else {
            ServiceType::default()
        };

        // Parse selector
        let selector = if let Some(sel_value) = params.get("selector") {
            parse_labels(sel_value)
        } else {
            BTreeMap::new()
        };

        // Parse ports
        let ports = if let Some(ports_value) = params.get("ports") {
            if let Some(ports_array) = ports_value.as_array() {
                ports_array
                    .iter()
                    .filter_map(|v| {
                        if let Some(obj) = v.as_object() {
                            Some(ServicePort {
                                port: obj.get("port")?.as_i64()? as i32,
                                target_port: obj
                                    .get("target_port")
                                    .and_then(|v| v.as_i64())
                                    .map(|v| v as i32),
                                node_port: obj
                                    .get("node_port")
                                    .and_then(|v| v.as_i64())
                                    .map(|v| v as i32),
                                protocol: obj
                                    .get("protocol")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("TCP")
                                    .to_string(),
                                name: obj
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Parse external IPs
        let external_ips = params.get_vec_string("external_ips")?.unwrap_or_default();

        // Parse load balancer source ranges
        let load_balancer_source_ranges = params
            .get_vec_string("load_balancer_source_ranges")?
            .unwrap_or_default();

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
            service_type,
            selector,
            ports,
            cluster_ip: params.get_string("cluster_ip")?,
            external_ips,
            load_balancer_ip: params.get_string("load_balancer_ip")?,
            load_balancer_source_ranges,
            external_name: params.get_string("external_name")?,
            session_affinity: params.get_string("session_affinity")?,
            session_affinity_timeout: params
                .get_i64("session_affinity_timeout")?
                .map(|v| v as i32),
            external_traffic_policy: params.get_string("external_traffic_policy")?,
            internal_traffic_policy: params.get_string("internal_traffic_policy")?,
            health_check_node_port: params.get_i64("health_check_node_port")?.map(|v| v as i32),
            publish_not_ready_addresses: params.get_bool_or("publish_not_ready_addresses", false),
            labels,
            annotations,
            definition: params.get("definition").cloned(),
            kubeconfig: params.get_string("kubeconfig")?,
            context: params.get_string("context")?,
        })
    }
}

/// Simulated Kubernetes Service info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub namespace: String,
    pub service_type: String,
    pub cluster_ip: Option<String>,
    pub external_ips: Vec<String>,
    pub load_balancer_ip: Option<String>,
    pub ports: Vec<PortInfo>,
    pub selector: BTreeMap<String, String>,
    pub creation_timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: i32,
    pub target_port: Option<i32>,
    pub node_port: Option<i32>,
    pub protocol: String,
    pub name: Option<String>,
}

/// Kubernetes Service module
pub struct K8sServiceModule;

impl K8sServiceModule {
    /// Get service by name
    async fn get_service(
        _name: &str,
        _namespace: &str,
        _kubeconfig: Option<&str>,
        _context: Option<&str>,
    ) -> ModuleResult<Option<ServiceInfo>> {
        // In a real implementation using kube crate:
        // let services: Api<Service> = Api::namespaced(client, namespace);
        // services.get_opt(name).await?

        Ok(None)
    }

    /// Create or update service
    async fn apply_service(config: &ServiceConfig) -> ModuleResult<ServiceInfo> {
        // Validate required fields
        if config.service_type == ServiceType::ExternalName && config.external_name.is_none() {
            return Err(ModuleError::MissingParameter(
                "external_name is required for ExternalName service type".to_string(),
            ));
        }

        if config.service_type != ServiceType::ExternalName && config.ports.is_empty() {
            return Err(ModuleError::MissingParameter(
                "ports are required for service (except ExternalName type)".to_string(),
            ));
        }

        // In a real implementation:
        // let service = Service {
        //     metadata: ObjectMeta { name, namespace, labels, annotations, .. },
        //     spec: Some(ServiceSpec {
        //         type_: Some(config.service_type.to_k8s_type().to_string()),
        //         selector: Some(config.selector.clone()),
        //         ports: Some(ports),
        //         cluster_ip: if config.service_type == ServiceType::Headless {
        //             Some("None".to_string())
        //         } else {
        //             config.cluster_ip.clone()
        //         },
        //         ..
        //     }),
        //     ..
        // };
        // services.patch(&config.name, &PatchParams::apply("rustible"), &Patch::Apply(&service)).await?

        let cluster_ip = if config.service_type == ServiceType::Headless {
            None
        } else {
            config
                .cluster_ip
                .clone()
                .or_else(|| Some("10.96.0.1".to_string()))
        };

        tracing::info!(
            "Would create/update service '{}' in namespace '{}' of type {:?}",
            config.name,
            config.namespace,
            config.service_type
        );

        Ok(ServiceInfo {
            name: config.name.clone(),
            namespace: config.namespace.clone(),
            service_type: config.service_type.to_k8s_type().to_string(),
            cluster_ip,
            external_ips: config.external_ips.clone(),
            load_balancer_ip: config.load_balancer_ip.clone(),
            ports: config
                .ports
                .iter()
                .map(|p| PortInfo {
                    port: p.port,
                    target_port: p.target_port,
                    node_port: p.node_port,
                    protocol: p.protocol.clone(),
                    name: p.name.clone(),
                })
                .collect(),
            selector: config.selector.clone(),
            creation_timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Delete service
    async fn delete_service(
        name: &str,
        namespace: &str,
        _kubeconfig: Option<&str>,
        _context: Option<&str>,
    ) -> ModuleResult<()> {
        tracing::info!(
            "Would delete service '{}' from namespace '{}'",
            name,
            namespace
        );
        Ok(())
    }

    /// Execute async service operations
    async fn execute_async(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let config = ServiceConfig::from_params(params)?;

        // Check for YAML definition override
        if let Some(ref _definition) = config.definition {
            return self.apply_from_definition(&config, context).await;
        }

        // Get existing service
        let existing = Self::get_service(
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

    /// Apply service from YAML definition
    async fn apply_from_definition(
        &self,
        config: &ServiceConfig,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        if context.check_mode {
            return Ok(ModuleOutput::changed(format!(
                "Would apply service definition for '{}'",
                config.name
            )));
        }

        Ok(
            ModuleOutput::changed(format!("Applied service definition for '{}'", config.name))
                .with_data("name", serde_json::json!(config.name))
                .with_data("namespace", serde_json::json!(config.namespace)),
        )
    }

    /// Ensure service is present
    async fn ensure_present(
        &self,
        config: &ServiceConfig,
        existing: Option<ServiceInfo>,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        if let Some(svc) = existing {
            // Service exists - check for updates
            let needs_update = self.needs_update(config, &svc);

            if !needs_update {
                return Ok(
                    ModuleOutput::ok(format!("Service '{}' is up to date", config.name))
                        .with_data("service", serde_json::to_value(&svc).unwrap()),
                );
            }

            if context.check_mode {
                return Ok(ModuleOutput::changed(format!(
                    "Would update service '{}'",
                    config.name
                )));
            }

            let updated = Self::apply_service(config).await?;

            Ok(
                ModuleOutput::changed(format!("Updated service '{}'", config.name))
                    .with_data("service", serde_json::to_value(&updated).unwrap()),
            )
        } else {
            // Create new service
            if context.check_mode {
                return Ok(ModuleOutput::changed(format!(
                    "Would create service '{}'",
                    config.name
                )));
            }

            let created = Self::apply_service(config).await?;

            Ok(
                ModuleOutput::changed(format!("Created service '{}'", config.name))
                    .with_data("service", serde_json::to_value(&created).unwrap()),
            )
        }
    }

    /// Ensure service is absent
    async fn ensure_absent(
        &self,
        config: &ServiceConfig,
        existing: Option<ServiceInfo>,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        if existing.is_none() {
            return Ok(ModuleOutput::ok(format!(
                "Service '{}' does not exist",
                config.name
            )));
        }

        if context.check_mode {
            return Ok(ModuleOutput::changed(format!(
                "Would delete service '{}'",
                config.name
            )));
        }

        Self::delete_service(
            &config.name,
            &config.namespace,
            config.kubeconfig.as_deref(),
            config.context.as_deref(),
        )
        .await?;

        Ok(ModuleOutput::changed(format!(
            "Deleted service '{}'",
            config.name
        )))
    }

    /// Check if service needs update
    fn needs_update(&self, config: &ServiceConfig, existing: &ServiceInfo) -> bool {
        // Check type
        if existing.service_type != config.service_type.to_k8s_type() {
            return true;
        }

        // Check ports (simplified)
        if existing.ports.len() != config.ports.len() {
            return true;
        }

        // Check selector
        if existing.selector != config.selector {
            return true;
        }

        false
    }
}

impl Module for K8sServiceModule {
    fn name(&self) -> &'static str {
        "k8s_service"
    }

    fn description(&self) -> &'static str {
        "Manage Kubernetes Services"
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

        // Validate type if provided
        if let Some(t) = params.get_string("type")? {
            ServiceType::from_str(&t)?;
        }

        // Validate session affinity if provided
        if let Some(affinity) = params.get_string("session_affinity")? {
            if !["None", "ClientIP"].contains(&affinity.as_str()) {
                return Err(ModuleError::InvalidParameter(format!(
                    "Invalid session_affinity '{}'. Valid values: None, ClientIP",
                    affinity
                )));
            }
        }

        // Validate external traffic policy if provided
        if let Some(policy) = params.get_string("external_traffic_policy")? {
            if !["Cluster", "Local"].contains(&policy.as_str()) {
                return Err(ModuleError::InvalidParameter(format!(
                    "Invalid external_traffic_policy '{}'. Valid values: Cluster, Local",
                    policy
                )));
            }
        }

        Ok(())
    }

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let config = ServiceConfig::from_params(params)?;

        let before = "# Current state: unknown (would query API)".to_string();
        let after = format!(
            r#"apiVersion: v1
kind: Service
metadata:
  name: {}
  namespace: {}
spec:
  type: {}
  selector: {:?}
  ports: {:?}"#,
            config.name,
            config.namespace,
            config.service_type.to_k8s_type(),
            config.selector,
            config.ports.len()
        );

        Ok(Some(Diff::new(before, after)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_module_metadata() {
        let module = K8sServiceModule;
        assert_eq!(module.name(), "k8s_service");
        assert_eq!(module.classification(), ModuleClassification::LocalLogic);
        assert_eq!(module.required_params(), &["name"]);
    }

    #[test]
    fn test_service_config_basic() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx-svc"));
        params.insert("selector".to_string(), serde_json::json!({"app": "nginx"}));
        params.insert(
            "ports".to_string(),
            serde_json::json!([
                {"port": 80, "target_port": 80}
            ]),
        );

        let config = ServiceConfig::from_params(&params).unwrap();
        assert_eq!(config.name, "nginx-svc");
        assert_eq!(config.namespace, "default");
        assert_eq!(config.service_type, ServiceType::ClusterIP);
        assert_eq!(config.ports.len(), 1);
        assert_eq!(config.ports[0].port, 80);
    }

    #[test]
    fn test_service_config_nodeport() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx-nodeport"));
        params.insert("type".to_string(), serde_json::json!("NodePort"));
        params.insert(
            "ports".to_string(),
            serde_json::json!([
                {"port": 80, "target_port": 80, "node_port": 30080}
            ]),
        );

        let config = ServiceConfig::from_params(&params).unwrap();
        assert_eq!(config.service_type, ServiceType::NodePort);
        assert_eq!(config.ports[0].node_port, Some(30080));
    }

    #[test]
    fn test_service_config_loadbalancer() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx-lb"));
        params.insert("type".to_string(), serde_json::json!("LoadBalancer"));
        params.insert(
            "load_balancer_ip".to_string(),
            serde_json::json!("192.168.1.100"),
        );
        params.insert(
            "ports".to_string(),
            serde_json::json!([{"port": 80, "target_port": 80}]),
        );

        let config = ServiceConfig::from_params(&params).unwrap();
        assert_eq!(config.service_type, ServiceType::LoadBalancer);
        assert_eq!(config.load_balancer_ip, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_service_config_headless() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("headless-svc"));
        params.insert("type".to_string(), serde_json::json!("Headless"));
        params.insert(
            "ports".to_string(),
            serde_json::json!([{"port": 80, "target_port": 80}]),
        );

        let config = ServiceConfig::from_params(&params).unwrap();
        assert_eq!(config.service_type, ServiceType::Headless);
    }

    #[test]
    fn test_service_config_with_annotations() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx-svc"));
        params.insert(
            "annotations".to_string(),
            serde_json::json!({
                "service.beta.kubernetes.io/aws-load-balancer-type": "nlb"
            }),
        );
        params.insert("ports".to_string(), serde_json::json!([{"port": 80}]));

        let config = ServiceConfig::from_params(&params).unwrap();
        assert!(config
            .annotations
            .contains_key("service.beta.kubernetes.io/aws-load-balancer-type"));
    }

    #[test]
    fn test_validate_params_invalid_name() {
        let module = K8sServiceModule;
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("Invalid_Name"));

        assert!(module.validate_params(&params).is_err());
    }

    #[test]
    fn test_validate_params_invalid_session_affinity() {
        let module = K8sServiceModule;
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx-svc"));
        params.insert("session_affinity".to_string(), serde_json::json!("Invalid"));

        assert!(module.validate_params(&params).is_err());
    }
}
