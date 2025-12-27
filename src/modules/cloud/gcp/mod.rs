//! GCP (Google Cloud Platform) modules for cloud infrastructure management.
//!
//! This module provides native Rust implementations for managing GCP resources
//! using the official Google Cloud client libraries for Rust.
//!
//! ## Available Modules
//!
//! - [`GcpComputeInstanceModule`](compute::GcpComputeInstanceModule): Compute Engine instance management
//! - [`GcpComputeFirewallModule`](compute::GcpComputeFirewallModule): Firewall rule management
//! - [`GcpComputeNetworkModule`](compute::GcpComputeNetworkModule): VPC network management
//! - [`GcpServiceAccountModule`](compute::GcpServiceAccountModule): Service account management
//!
//! ## Authentication
//!
//! GCP credentials are loaded from the standard credential chain:
//!
//! 1. Environment variable (`GOOGLE_APPLICATION_CREDENTIALS` pointing to a service account key file)
//! 2. Application Default Credentials (ADC) via `gcloud auth application-default login`
//! 3. Compute Engine default service account (when running on GCE)
//! 4. Cloud Run/Cloud Functions service account (when running in serverless environments)
//!
//! The project can be specified via:
//! - Module parameter (`project`)
//! - Environment variable (`GOOGLE_CLOUD_PROJECT` or `GCLOUD_PROJECT`)
//! - Metadata server (when running on GCP infrastructure)
//!
//! ## Example
//!
//! ```yaml
//! - name: Create a Compute Engine instance
//!   gcp_compute_instance:
//!     name: web-server-01
//!     zone: us-central1-a
//!     machine_type: e2-medium
//!     image_family: debian-11
//!     image_project: debian-cloud
//!     state: running
//!     network: default
//!     tags:
//!       - http-server
//!       - https-server
//!
//! - name: Create a firewall rule
//!   gcp_compute_firewall:
//!     name: allow-http
//!     network: default
//!     allowed:
//!       - protocol: tcp
//!         ports:
//!           - "80"
//!           - "443"
//!     source_ranges:
//!       - 0.0.0.0/0
//!     target_tags:
//!       - http-server
//! ```

pub mod compute;

pub use compute::{
    GcpComputeFirewallModule, GcpComputeInstanceModule, GcpComputeNetworkModule,
    GcpServiceAccountModule,
};
