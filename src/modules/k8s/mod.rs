//! Kubernetes modules for managing K8s resources
//!
//! This module provides Kubernetes resource management capabilities using the kube-rs crate.
//! These modules allow managing Deployments, Services, ConfigMaps, Secrets, and Namespaces.
//!
//! ## Available Modules
//!
//! - `k8s_deployment` - Manage Kubernetes Deployments
//! - `k8s_service` - Manage Kubernetes Services
//! - `k8s_configmap` - Manage Kubernetes ConfigMaps
//! - `k8s_secret` - Manage Kubernetes Secrets
//! - `k8s_namespace` - Manage Kubernetes Namespaces
//!
//! ## Requirements
//!
//! These modules require the `kubernetes` feature to be enabled:
//!
//! ```toml
//! [dependencies]
//! rustible = { version = "0.1", features = ["kubernetes"] }
//! ```

pub mod k8s_configmap;
pub mod k8s_deployment;
pub mod k8s_namespace;
pub mod k8s_secret;
pub mod k8s_service;

pub use k8s_configmap::K8sConfigMapModule;
pub use k8s_deployment::K8sDeploymentModule;
pub use k8s_namespace::K8sNamespaceModule;
pub use k8s_secret::K8sSecretModule;
pub use k8s_service::K8sServiceModule;
