# Terraform Integration Design Document

## Document Information

| Field | Value |
|-------|-------|
| Document ID | INTEGRATION-01 |
| Version | 1.0 |
| Status | Draft |
| Created | 2025-12-26 |
| Author | System Architecture Designer |

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Goals and Non-Goals](#2-goals-and-non-goals)
3. [Architecture Overview](#3-architecture-overview)
4. [Dynamic Inventory from Terraform State](#4-dynamic-inventory-from-terraform-state)
5. [Terraform Provisioner](#5-terraform-provisioner)
6. [Variable Sharing](#6-variable-sharing)
7. [Example Workflows](#7-example-workflows)
8. [Implementation Roadmap](#8-implementation-roadmap)
9. [Architecture Decision Records](#9-architecture-decision-records)

---

## 1. Executive Summary

This document outlines the design for integrating Rustible with Terraform, enabling seamless infrastructure provisioning and configuration management workflows. The integration provides:

- **Dynamic Inventory**: Automatically discover hosts from Terraform state files
- **Terraform Provisioner**: Execute Rustible playbooks as Terraform provisioners
- **Variable Sharing**: Bidirectional variable exchange between Terraform and Rustible
- **Unified Workflows**: Combined infrastructure-as-code and configuration management

### Value Proposition

```
Infrastructure as Code (Terraform)
         |
         v
   +-----------+
   | Terraform |  <-- Provisions infrastructure (VMs, networks, storage)
   +-----------+
         |
         | terraform.tfstate
         v
   +-----------+
   | Rustible  |  <-- Configures provisioned infrastructure
   +-----------+
         |
         v
   Running System
```

---

## 2. Goals and Non-Goals

### Goals

1. **Zero-friction inventory discovery** - Read Terraform state without manual inventory maintenance
2. **Native Rust implementation** - No Python dependencies or external scripts required
3. **Multi-backend support** - Local, S3, GCS, Azure Blob, Consul, HTTP backends
4. **Resource-to-host mapping** - Intelligent mapping of cloud resources to Rustible hosts
5. **Secure state access** - Support for encrypted state and remote backend authentication
6. **Variable flow** - Share Terraform outputs as Rustible variables and vice versa
7. **Provisioner integration** - Execute Rustible as a Terraform provisioner

### Non-Goals

1. Replacing Terraform for infrastructure provisioning
2. Managing Terraform state lifecycle
3. Supporting Terraform Cloud/Enterprise-specific features (initial version)
4. Providing a Terraform provider for Rustible resources

---

## 3. Architecture Overview

### Component Diagram (C4 Level 2)

```
+-------------------------------------------------------------------+
|                     Rustible Terraform Integration                  |
+-------------------------------------------------------------------+
|                                                                     |
|  +------------------------+     +-----------------------------+     |
|  |  Terraform State       |     |  Terraform Provisioner      |     |
|  |  Inventory Plugin      |     |  (local-exec bridge)        |     |
|  +------------------------+     +-----------------------------+     |
|  | - State file parsing   |     | - Provisioner protocol      |     |
|  | - Backend adapters     |     | - Connection handling       |     |
|  | - Resource mapping     |     | - Variable injection        |     |
|  | - Dynamic grouping     |     | - Error propagation         |     |
|  +------------------------+     +-----------------------------+     |
|            |                              |                         |
|            v                              v                         |
|  +------------------------+     +-----------------------------+     |
|  |  TerraformStateBackend |     |  ProvisionerContext         |     |
|  |  (trait)               |     |  (struct)                   |     |
|  +------------------------+     +-----------------------------+     |
|  | + LocalBackend         |     | - resource_type             |     |
|  | + S3Backend            |     | - resource_name             |     |
|  | + GcsBackend           |     | - connection_info           |     |
|  | + AzureBackend         |     | - terraform_vars            |     |
|  | + ConsulBackend        |     | - triggers                  |     |
|  | + HttpBackend          |     +-----------------------------+     |
|  +------------------------+                                         |
|            |                                                        |
|            v                                                        |
|  +-----------------------------------------------------------+     |
|  |              Variable Sharing Layer                        |     |
|  +-----------------------------------------------------------+     |
|  | - Terraform outputs -> Rustible vars                       |     |
|  | - Rustible facts -> Terraform data sources                |     |
|  | - Type conversion and validation                          |     |
|  | - Sensitive value handling                                 |     |
|  +-----------------------------------------------------------+     |
|                                                                     |
+-------------------------------------------------------------------+
```

### Data Flow

```
                    Read                      Parse                    Map
Terraform State ---------> State Backend ---------> State Parser ---------> Inventory
     |                          |                       |                      |
     |                          |                       |                      |
     v                          v                       v                      v
  backends:               adapters:               resources:            hosts + groups:
  - local file            - LocalFile             - aws_instance        - web-server-1
  - s3://bucket           - S3Adapter             - azurerm_vm          - db-server-1
  - consul://kv           - GcsAdapter            - google_compute      - [aws_instances]
  - http(s)://            - AzureAdapter                                - [region_us_east]
                          - ConsulAdapter
                          - HttpAdapter
```

---

## 4. Dynamic Inventory from Terraform State

### 4.1 Terraform Inventory Plugin

The plugin reads Terraform state and generates Rustible inventory dynamically.

#### Configuration File Format

Create `terraform.rustible.yml`:

```yaml
plugin: terraform_state

# State source (one of: local, s3, gcs, azure, consul, http)
backend: local
path: ./terraform.tfstate

# OR for remote backends:
# backend: s3
# bucket: my-terraform-state
# key: production/terraform.tfstate
# region: us-east-1

# Resource mapping rules
resource_mappings:
  # Map AWS EC2 instances to hosts
  aws_instance:
    hostname_attribute: tags.Name
    address_attribute: public_ip
    fallback_address: private_ip
    group_by:
      - attribute: tags.Environment
        prefix: env
      - attribute: tags.Role
        prefix: role
      - attribute: availability_zone
        prefix: az
    host_vars:
      ansible_user: "{{ tags.ssh_user | default('ec2-user') }}"
      instance_type: "{{ instance_type }}"
      instance_id: "{{ id }}"

  # Map Azure VMs
  azurerm_linux_virtual_machine:
    hostname_attribute: name
    address_attribute: public_ip_address
    fallback_address: private_ip_address
    group_by:
      - attribute: location
        prefix: region
    host_vars:
      ansible_user: admin_username

  # Map GCP instances
  google_compute_instance:
    hostname_attribute: name
    address_attribute: network_interface.0.access_config.0.nat_ip
    fallback_address: network_interface.0.network_ip

# Keyed groups for dynamic grouping
keyed_groups:
  - key: "{{ resource_type }}"
    prefix: tf
    separator: "_"

# Host filters (Jinja2 expressions)
filters:
  - "{{ state == 'running' }}"
  - "{{ tags.managed_by == 'rustible' }}"

# Compose additional groups
compose:
  ansible_host: "{{ public_ip | default(private_ip) }}"

# Caching
cache:
  enabled: true
  ttl: 300  # seconds
```

#### Rust Implementation Structure

```rust
// src/inventory/plugins/terraform.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Terraform state inventory plugin
#[derive(Debug)]
pub struct TerraformInventoryPlugin {
    config: TerraformPluginConfig,
    backend: Box<dyn TerraformStateBackend>,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformPluginConfig {
    /// State backend type
    pub backend: TerraformBackendType,

    /// Resource to host mapping rules
    #[serde(default)]
    pub resource_mappings: HashMap<String, ResourceMapping>,

    /// Keyed groups configuration
    #[serde(default)]
    pub keyed_groups: Vec<KeyedGroup>,

    /// Host filters
    #[serde(default)]
    pub filters: Vec<String>,

    /// Variable composition
    #[serde(default)]
    pub compose: HashMap<String, String>,

    /// Cache configuration
    #[serde(default)]
    pub cache: CacheConfig,
}

/// Backend type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerraformBackendType {
    Local { path: PathBuf },
    S3 { bucket: String, key: String, region: String },
    Gcs { bucket: String, prefix: String },
    Azure { storage_account: String, container: String, key: String },
    Consul { address: String, path: String },
    Http { address: String, lock_address: Option<String> },
}

/// Resource to host mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMapping {
    /// Attribute to use for hostname
    pub hostname_attribute: String,

    /// Primary address attribute
    pub address_attribute: String,

    /// Fallback address if primary is unavailable
    pub fallback_address: Option<String>,

    /// Group-by rules
    #[serde(default)]
    pub group_by: Vec<GroupByRule>,

    /// Host variables to set
    #[serde(default)]
    pub host_vars: HashMap<String, String>,

    /// Connection type (ssh, winrm, docker)
    #[serde(default = "default_connection")]
    pub connection: String,
}

/// Trait for Terraform state backends
#[async_trait]
pub trait TerraformStateBackend: Send + Sync + std::fmt::Debug {
    /// Get the backend name
    fn name(&self) -> &str;

    /// Read the state file
    async fn read_state(&self) -> Result<TerraformState, TerraformError>;

    /// Check if state is available
    async fn is_available(&self) -> bool;

    /// Get state metadata (serial, lineage, version)
    async fn get_metadata(&self) -> Result<StateMetadata, TerraformError>;
}
```

#### State Parsing

```rust
// src/inventory/plugins/terraform/state.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Terraform state file structure (v4 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformState {
    /// State format version
    pub version: u32,

    /// Terraform version that created this state
    pub terraform_version: String,

    /// State serial number (incremented on changes)
    pub serial: u64,

    /// Unique identifier for this state lineage
    pub lineage: String,

    /// Terraform outputs
    #[serde(default)]
    pub outputs: HashMap<String, OutputValue>,

    /// Resources in state
    #[serde(default)]
    pub resources: Vec<StateResource>,
}

/// State resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResource {
    /// Resource mode (managed or data)
    pub mode: String,

    /// Resource type (e.g., "aws_instance")
    #[serde(rename = "type")]
    pub resource_type: String,

    /// Resource name from configuration
    pub name: String,

    /// Provider configuration
    pub provider: String,

    /// Resource instances (for count/for_each)
    pub instances: Vec<ResourceInstance>,
}

/// Resource instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInstance {
    /// Instance index key (for count/for_each)
    pub index_key: Option<serde_json::Value>,

    /// Resource schema version
    pub schema_version: u32,

    /// Resource attributes
    pub attributes: serde_json::Value,

    /// Sensitive attributes
    #[serde(default)]
    pub sensitive_attributes: Vec<String>,

    /// Resource dependencies
    #[serde(default)]
    pub dependencies: Vec<String>,
}

impl TerraformState {
    /// Extract hosts from state based on resource mappings
    pub fn to_inventory(
        &self,
        mappings: &HashMap<String, ResourceMapping>,
    ) -> Result<Inventory, TerraformError> {
        let mut inventory = Inventory::new();

        for resource in &self.resources {
            // Skip data sources
            if resource.mode != "managed" {
                continue;
            }

            // Check if we have a mapping for this resource type
            if let Some(mapping) = mappings.get(&resource.resource_type) {
                for instance in &resource.instances {
                    let host = self.resource_to_host(
                        resource,
                        instance,
                        mapping,
                    )?;

                    inventory.add_host(host)?;
                }
            }
        }

        Ok(inventory)
    }

    /// Convert a resource instance to a host
    fn resource_to_host(
        &self,
        resource: &StateResource,
        instance: &ResourceInstance,
        mapping: &ResourceMapping,
    ) -> Result<Host, TerraformError> {
        // Extract hostname
        let hostname = self.extract_attribute(
            &instance.attributes,
            &mapping.hostname_attribute,
        )?;

        // Extract address
        let address = self.extract_attribute(
            &instance.attributes,
            &mapping.address_attribute,
        ).or_else(|_| {
            mapping.fallback_address.as_ref()
                .ok_or_else(|| TerraformError::MissingAttribute(
                    mapping.address_attribute.clone()
                ))
                .and_then(|fallback| {
                    self.extract_attribute(&instance.attributes, fallback)
                })
        })?;

        let mut host = Host::with_address(&hostname, &address);

        // Apply host variables
        for (key, template) in &mapping.host_vars {
            let value = self.render_template(
                template,
                &instance.attributes,
            )?;
            host.set_var(key, serde_yaml::Value::String(value));
        }

        // Add to resource type group
        host.add_to_group(format!("tf_{}", resource.resource_type));

        // Apply group_by rules
        for rule in &mapping.group_by {
            if let Ok(value) = self.extract_attribute(
                &instance.attributes,
                &rule.attribute,
            ) {
                let group_name = if rule.prefix.is_empty() {
                    sanitize_group_name(&value)
                } else {
                    format!("{}_{}", rule.prefix, sanitize_group_name(&value))
                };
                host.add_to_group(group_name);
            }
        }

        Ok(host)
    }

    /// Extract an attribute using dot notation
    fn extract_attribute(
        &self,
        attributes: &serde_json::Value,
        path: &str,
    ) -> Result<String, TerraformError> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = attributes;

        for part in parts {
            // Handle array indexing
            if let Ok(index) = part.parse::<usize>() {
                current = current.get(index)
                    .ok_or_else(|| TerraformError::MissingAttribute(path.to_string()))?;
            } else {
                current = current.get(part)
                    .ok_or_else(|| TerraformError::MissingAttribute(path.to_string()))?;
            }
        }

        match current {
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            _ => Err(TerraformError::InvalidAttributeType(path.to_string())),
        }
    }
}
```

### 4.2 Backend Implementations

#### Local Backend

```rust
// src/inventory/plugins/terraform/backends/local.rs

use super::{TerraformStateBackend, TerraformState, TerraformError, StateMetadata};
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug)]
pub struct LocalBackend {
    path: PathBuf,
}

impl LocalBackend {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl TerraformStateBackend for LocalBackend {
    fn name(&self) -> &str {
        "local"
    }

    async fn read_state(&self) -> Result<TerraformState, TerraformError> {
        let content = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| TerraformError::IoError(e.to_string()))?;

        serde_json::from_str(&content)
            .map_err(|e| TerraformError::ParseError(e.to_string()))
    }

    async fn is_available(&self) -> bool {
        self.path.exists()
    }

    async fn get_metadata(&self) -> Result<StateMetadata, TerraformError> {
        let state = self.read_state().await?;
        Ok(StateMetadata {
            serial: state.serial,
            lineage: state.lineage,
            version: state.version,
        })
    }
}
```

#### S3 Backend

```rust
// src/inventory/plugins/terraform/backends/s3.rs

use super::{TerraformStateBackend, TerraformState, TerraformError, StateMetadata};
use async_trait::async_trait;

#[derive(Debug)]
pub struct S3Backend {
    bucket: String,
    key: String,
    region: String,
    // Use reqwest for S3 operations to avoid heavy SDK dependency
}

impl S3Backend {
    pub fn new(bucket: String, key: String, region: String) -> Self {
        Self { bucket, key, region }
    }

    /// Build S3 endpoint URL
    fn endpoint_url(&self) -> String {
        format!(
            "https://{}.s3.{}.amazonaws.com/{}",
            self.bucket, self.region, self.key
        )
    }
}

#[async_trait]
impl TerraformStateBackend for S3Backend {
    fn name(&self) -> &str {
        "s3"
    }

    async fn read_state(&self) -> Result<TerraformState, TerraformError> {
        // Use AWS credential chain: env vars, config file, instance profile
        let client = reqwest::Client::new();

        // Note: Real implementation needs AWS4 signature
        // This is simplified for design document
        let response = client
            .get(&self.endpoint_url())
            .send()
            .await
            .map_err(|e| TerraformError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(TerraformError::BackendError(
                format!("S3 returned status: {}", response.status())
            ));
        }

        let content = response.text().await
            .map_err(|e| TerraformError::NetworkError(e.to_string()))?;

        serde_json::from_str(&content)
            .map_err(|e| TerraformError::ParseError(e.to_string()))
    }

    async fn is_available(&self) -> bool {
        // HEAD request to check if object exists
        true // Simplified
    }

    async fn get_metadata(&self) -> Result<StateMetadata, TerraformError> {
        let state = self.read_state().await?;
        Ok(StateMetadata {
            serial: state.serial,
            lineage: state.lineage,
            version: state.version,
        })
    }
}
```

### 4.3 Plugin Registration

```rust
// src/inventory/plugin.rs (additions)

impl InventoryPluginFactory {
    pub fn create(
        name: &str,
        config: InventoryPluginConfig,
    ) -> PluginResult<Arc<dyn InventoryPlugin>> {
        match name.to_lowercase().as_str() {
            // Existing plugins...
            "file" | "ini" | "yaml" | "json" => Self::create_file_plugin(config),
            "script" | "dynamic" => Self::create_script_plugin(config),
            "aws_ec2" | "ec2" => Self::create_aws_ec2_plugin(config),

            // New Terraform plugin
            "terraform" | "terraform_state" | "tf" => {
                Self::create_terraform_plugin(config)
            },

            _ => Err(PluginError {
                kind: PluginErrorKind::NotFound,
                message: format!("Unknown plugin: {}", name),
            }),
        }
    }

    fn create_terraform_plugin(
        config: InventoryPluginConfig,
    ) -> PluginResult<Arc<dyn InventoryPlugin>> {
        let tf_config: TerraformPluginConfig = config.try_into()
            .map_err(|e| PluginError {
                kind: PluginErrorKind::InvalidConfig,
                message: format!("Invalid Terraform config: {}", e),
            })?;

        let backend = create_backend(&tf_config.backend)?;

        Ok(Arc::new(TerraformInventoryPlugin::new(tf_config, backend)))
    }
}
```

---

## 5. Terraform Provisioner

### 5.1 Design Overview

Rustible can be invoked as a Terraform provisioner using `local-exec` with a structured interface. A dedicated CLI subcommand handles provisioner mode.

```hcl
# Terraform configuration
resource "aws_instance" "web" {
  ami           = "ami-0c55b159cbfafe1f0"
  instance_type = "t2.micro"

  provisioner "local-exec" {
    command = <<-EOT
      rustible provisioner \
        --resource-type aws_instance \
        --resource-name web \
        --playbook configure.yml \
        --host ${self.public_ip} \
        --user ec2-user \
        --private-key ~/.ssh/id_rsa \
        --extra-vars '${jsonencode({
          instance_id    = self.id,
          instance_type  = self.instance_type,
          availability_zone = self.availability_zone
        })}'
    EOT

    environment = {
      TF_RESOURCE_TYPE = "aws_instance"
      TF_RESOURCE_NAME = "web"
    }
  }
}
```

### 5.2 Provisioner CLI Interface

```rust
// src/cli/commands/provisioner.rs

use clap::Args;
use std::collections::HashMap;
use std::path::PathBuf;

/// Terraform provisioner mode for Rustible
#[derive(Args, Debug)]
pub struct ProvisionerArgs {
    /// Playbook to execute
    #[arg(short, long)]
    playbook: PathBuf,

    /// Target host IP or hostname
    #[arg(long)]
    host: String,

    /// SSH user
    #[arg(long, default_value = "root")]
    user: String,

    /// SSH private key file
    #[arg(long)]
    private_key: Option<PathBuf>,

    /// SSH port
    #[arg(long, default_value = "22")]
    port: u16,

    /// Terraform resource type
    #[arg(long, env = "TF_RESOURCE_TYPE")]
    resource_type: String,

    /// Terraform resource name
    #[arg(long, env = "TF_RESOURCE_NAME")]
    resource_name: String,

    /// Extra variables as JSON
    #[arg(short = 'e', long, value_parser = parse_json_vars)]
    extra_vars: Option<serde_json::Value>,

    /// Connection timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Maximum retries for connection
    #[arg(long, default_value = "3")]
    retries: u32,

    /// Delay between retries in seconds
    #[arg(long, default_value = "10")]
    retry_delay: u64,

    /// Check mode (dry run)
    #[arg(long)]
    check: bool,
}

impl ProvisionerArgs {
    pub async fn execute(&self) -> Result<(), ProvisionerError> {
        // Build single-host inventory
        let mut inventory = Inventory::new();
        let mut host = Host::with_address(&self.resource_name, &self.host);

        host.connection.ssh.port = self.port;
        host.connection.ssh.user = Some(self.user.clone());
        if let Some(key) = &self.private_key {
            host.connection.ssh.private_key_file = Some(
                key.to_string_lossy().to_string()
            );
        }

        // Add Terraform metadata as host vars
        host.set_var("terraform_resource_type",
            serde_yaml::Value::String(self.resource_type.clone()));
        host.set_var("terraform_resource_name",
            serde_yaml::Value::String(self.resource_name.clone()));

        // Add extra vars
        if let Some(vars) = &self.extra_vars {
            if let serde_json::Value::Object(map) = vars {
                for (key, value) in map {
                    host.set_var(key, json_to_yaml(value));
                }
            }
        }

        // Add to groups
        host.add_to_group(format!("tf_{}", self.resource_type));

        inventory.add_host(host)?;

        // Wait for host to become available
        self.wait_for_connection(&self.host).await?;

        // Execute playbook
        let playbook = Playbook::load(&self.playbook).await?;

        let executor = PlaybookExecutor::builder()
            .inventory(inventory)
            .check_mode(self.check)
            .build()?;

        let result = executor.run(&playbook).await?;

        // Report results
        self.report_results(&result)?;

        Ok(())
    }

    async fn wait_for_connection(&self, host: &str) -> Result<(), ProvisionerError> {
        use tokio::time::{sleep, Duration, timeout};

        for attempt in 0..self.retries {
            if attempt > 0 {
                tracing::info!(
                    "Retrying connection to {} (attempt {}/{})",
                    host, attempt + 1, self.retries
                );
                sleep(Duration::from_secs(self.retry_delay)).await;
            }

            match timeout(
                Duration::from_secs(self.timeout),
                self.check_connectivity(host),
            ).await {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(e)) => {
                    tracing::warn!("Connection check failed: {}", e);
                }
                Err(_) => {
                    tracing::warn!("Connection timed out after {}s", self.timeout);
                }
            }
        }

        Err(ProvisionerError::ConnectionFailed {
            host: host.to_string(),
            retries: self.retries,
        })
    }
}
```

### 5.3 Terraform Module for Rustible

Create a reusable Terraform module:

```hcl
# modules/rustible-provisioner/main.tf

variable "host" {
  description = "Target host IP or hostname"
  type        = string
}

variable "playbook" {
  description = "Path to Rustible playbook"
  type        = string
}

variable "user" {
  description = "SSH user"
  type        = string
  default     = "root"
}

variable "private_key_path" {
  description = "Path to SSH private key"
  type        = string
  default     = "~/.ssh/id_rsa"
}

variable "extra_vars" {
  description = "Extra variables to pass to Rustible"
  type        = map(any)
  default     = {}
}

variable "resource_type" {
  description = "Terraform resource type"
  type        = string
}

variable "resource_name" {
  description = "Terraform resource name"
  type        = string
}

variable "timeout" {
  description = "Connection timeout in seconds"
  type        = number
  default     = 30
}

variable "retries" {
  description = "Maximum connection retries"
  type        = number
  default     = 5
}

variable "retry_delay" {
  description = "Delay between retries in seconds"
  type        = number
  default     = 15
}

resource "null_resource" "rustible_provisioner" {
  triggers = {
    playbook_hash = filemd5(var.playbook)
    host          = var.host
    extra_vars    = jsonencode(var.extra_vars)
  }

  provisioner "local-exec" {
    command = <<-EOT
      rustible provisioner \
        --playbook ${var.playbook} \
        --host ${var.host} \
        --user ${var.user} \
        --private-key ${var.private_key_path} \
        --resource-type ${var.resource_type} \
        --resource-name ${var.resource_name} \
        --timeout ${var.timeout} \
        --retries ${var.retries} \
        --retry-delay ${var.retry_delay} \
        --extra-vars '${jsonencode(var.extra_vars)}'
    EOT

    environment = {
      RUSTIBLE_LOG = "info"
    }
  }
}
```

Usage:

```hcl
module "configure_web_server" {
  source = "./modules/rustible-provisioner"

  host          = aws_instance.web.public_ip
  playbook      = "${path.module}/playbooks/web-server.yml"
  user          = "ubuntu"
  private_key_path = "~/.ssh/aws-key.pem"
  resource_type = "aws_instance"
  resource_name = "web"

  extra_vars = {
    instance_id       = aws_instance.web.id
    instance_type     = aws_instance.web.instance_type
    availability_zone = aws_instance.web.availability_zone
    vpc_id            = aws_instance.web.vpc_id
  }

  depends_on = [aws_instance.web]
}
```

---

## 6. Variable Sharing

### 6.1 Terraform Outputs to Rustible Variables

#### Automatic Variable Import

```yaml
# playbook.yml
- name: Configure infrastructure
  hosts: all

  vars_files:
    # Import all Terraform outputs
    - terraform: ./terraform.tfstate
      # Or from remote backend
      # terraform: s3://bucket/key

    # Import specific outputs
    - terraform:
        path: ./terraform.tfstate
        outputs:
          - vpc_id
          - subnet_ids
          - security_group_id
```

#### Implementation

```rust
// src/vars/terraform.rs

use super::{VarStore, VarPrecedence, VarsResult};
use std::path::Path;

/// Import Terraform outputs as variables
pub struct TerraformVarImporter;

impl TerraformVarImporter {
    /// Import all outputs from a state file
    pub async fn import_outputs<P: AsRef<Path>>(
        path: P,
        store: &mut VarStore,
    ) -> VarsResult<()> {
        let state = Self::read_state(path).await?;

        for (name, output) in &state.outputs {
            let value = Self::convert_output_value(&output.value)?;
            store.set(
                format!("terraform_{}", name),
                value,
                VarPrecedence::PlayVarsFiles,
            );
        }

        Ok(())
    }

    /// Import specific outputs
    pub async fn import_specific_outputs<P: AsRef<Path>>(
        path: P,
        output_names: &[String],
        store: &mut VarStore,
    ) -> VarsResult<()> {
        let state = Self::read_state(path).await?;

        for name in output_names {
            if let Some(output) = state.outputs.get(name) {
                let value = Self::convert_output_value(&output.value)?;
                store.set(
                    format!("terraform_{}", name),
                    value,
                    VarPrecedence::PlayVarsFiles,
                );
            } else {
                tracing::warn!(
                    "Terraform output '{}' not found in state",
                    name
                );
            }
        }

        Ok(())
    }

    /// Convert Terraform output value to YAML value
    fn convert_output_value(
        value: &serde_json::Value,
    ) -> VarsResult<serde_yaml::Value> {
        // Handle sensitive values
        // Note: Sensitive values in state are still visible, just marked
        json_to_yaml(value)
    }
}
```

### 6.2 Rustible Facts to Terraform

#### Export Module

```yaml
# Export facts to a file for Terraform to consume
- name: Export facts for Terraform
  hosts: all
  tasks:
    - name: Gather facts
      setup:
        gather_subset:
          - hardware
          - network

    - name: Export to JSON
      rustible.terraform.export_facts:
        dest: ./terraform_data/{{ inventory_hostname }}.json
        facts:
          - ansible_hostname
          - ansible_default_ipv4
          - ansible_processor_count
          - ansible_memtotal_mb
```

#### Terraform Data Source

```hcl
# Read Rustible-exported facts
data "local_file" "rustible_facts" {
  for_each = toset(var.managed_hosts)
  filename = "${path.module}/terraform_data/${each.key}.json"
}

locals {
  host_facts = {
    for host in var.managed_hosts :
    host => jsondecode(data.local_file.rustible_facts[host].content)
  }
}

# Use facts in Terraform
resource "aws_instance" "scaled" {
  count = local.host_facts["controller"]["ansible_processor_count"] * 2
  # ...
}
```

### 6.3 Sensitive Value Handling

```rust
// src/vars/terraform.rs (additions)

/// Handle sensitive values from Terraform
impl TerraformVarImporter {
    /// Import with sensitivity awareness
    pub async fn import_with_sensitivity<P: AsRef<Path>>(
        path: P,
        store: &mut VarStore,
        mask_sensitive: bool,
    ) -> VarsResult<()> {
        let state = Self::read_state(path).await?;

        for (name, output) in &state.outputs {
            let mut variable = Variable::with_source(
                Self::convert_output_value(&output.value)?,
                VarPrecedence::PlayVarsFiles,
                path.as_ref(),
            );

            // Mark as encrypted/sensitive
            if output.sensitive {
                if mask_sensitive {
                    // Replace with placeholder
                    variable = Variable::new(
                        serde_yaml::Value::String("<<SENSITIVE>>".to_string()),
                        VarPrecedence::PlayVarsFiles,
                    );
                } else {
                    variable = variable.encrypted();
                }
            }

            store.set_variable(format!("terraform_{}", name), variable);
        }

        Ok(())
    }
}
```

---

## 7. Example Workflows

### 7.1 Complete Infrastructure Deployment

```hcl
# main.tf - Terraform infrastructure

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }

  backend "s3" {
    bucket = "my-terraform-state"
    key    = "production/terraform.tfstate"
    region = "us-east-1"
  }
}

# VPC and networking
module "vpc" {
  source = "terraform-aws-modules/vpc/aws"
  # ...
}

# Web servers
resource "aws_instance" "web" {
  count         = 3
  ami           = data.aws_ami.ubuntu.id
  instance_type = "t3.medium"
  subnet_id     = module.vpc.private_subnets[count.index % length(module.vpc.private_subnets)]

  tags = {
    Name        = "web-${count.index + 1}"
    Role        = "web"
    Environment = "production"
    managed_by  = "rustible"
  }
}

# Database servers
resource "aws_instance" "db" {
  count         = 2
  ami           = data.aws_ami.ubuntu.id
  instance_type = "r5.large"
  subnet_id     = module.vpc.database_subnets[count.index]

  tags = {
    Name        = "db-${count.index + 1}"
    Role        = "database"
    Environment = "production"
    managed_by  = "rustible"
  }
}

# Outputs for Rustible
output "vpc_id" {
  value = module.vpc.vpc_id
}

output "web_ips" {
  value = aws_instance.web[*].private_ip
}

output "db_ips" {
  value     = aws_instance.db[*].private_ip
  sensitive = true
}
```

```yaml
# terraform.rustible.yml - Dynamic inventory

plugin: terraform_state
backend: s3
bucket: my-terraform-state
key: production/terraform.tfstate
region: us-east-1

resource_mappings:
  aws_instance:
    hostname_attribute: tags.Name
    address_attribute: private_ip
    group_by:
      - attribute: tags.Role
        prefix: role
      - attribute: tags.Environment
        prefix: env
    host_vars:
      instance_id: "{{ id }}"
      instance_type: "{{ instance_type }}"
      availability_zone: "{{ availability_zone }}"

keyed_groups:
  - key: tags.Role
    prefix: ""

filters:
  - "{{ tags.managed_by == 'rustible' }}"

cache:
  enabled: true
  ttl: 60
```

```yaml
# site.yml - Rustible playbook

- name: Import Terraform outputs
  hosts: localhost
  vars_files:
    - terraform:
        backend: s3
        bucket: my-terraform-state
        key: production/terraform.tfstate
        region: us-east-1

- name: Configure web servers
  hosts: role_web
  become: true

  roles:
    - common
    - nginx
    - app_deploy

  vars:
    vpc_id: "{{ terraform_vpc_id }}"

- name: Configure database servers
  hosts: role_database
  become: true

  roles:
    - common
    - postgresql

  vars:
    db_ips: "{{ terraform_db_ips }}"
```

```bash
#!/bin/bash
# deploy.sh - Complete deployment workflow

set -e

echo "=== Step 1: Terraform Apply ==="
cd infrastructure
terraform init
terraform plan -out=tfplan
terraform apply tfplan

echo "=== Step 2: Rustible Configuration ==="
cd ../configuration
rustible site.yml -i terraform.rustible.yml -v

echo "=== Deployment Complete ==="
```

### 7.2 Blue-Green Deployment

```hcl
# blue-green.tf

variable "active_environment" {
  default = "blue"
}

resource "aws_instance" "blue" {
  count = var.active_environment == "blue" ? var.instance_count : 0
  # ...
  tags = {
    Environment = "blue"
    Active      = var.active_environment == "blue" ? "true" : "false"
  }
}

resource "aws_instance" "green" {
  count = var.active_environment == "green" ? var.instance_count : 0
  # ...
  tags = {
    Environment = "green"
    Active      = var.active_environment == "green" ? "true" : "false"
  }
}
```

```yaml
# deploy-blue-green.yml

- name: Configure new environment
  hosts: "env_{{ target_environment }}"
  serial: 1  # Rolling deployment
  become: true

  pre_tasks:
    - name: Remove from load balancer
      rustible.aws.elb_target:
        target_group_arn: "{{ terraform_target_group_arn }}"
        target_id: "{{ instance_id }}"
        state: absent
      delegate_to: localhost

  roles:
    - app_deploy

  post_tasks:
    - name: Health check
      uri:
        url: "http://localhost:{{ app_port }}/health"
        status_code: 200
      retries: 10
      delay: 5

    - name: Add to load balancer
      rustible.aws.elb_target:
        target_group_arn: "{{ terraform_target_group_arn }}"
        target_id: "{{ instance_id }}"
        state: present
      delegate_to: localhost
```

### 7.3 Multi-Cloud Deployment

```yaml
# multi-cloud.rustible.yml

plugin: terraform_state

# Multiple backends for multi-cloud
backends:
  - name: aws
    type: s3
    bucket: terraform-state-aws
    key: production/terraform.tfstate
    region: us-east-1

  - name: azure
    type: azure
    storage_account: terraformstateazure
    container: tfstate
    key: production.tfstate

  - name: gcp
    type: gcs
    bucket: terraform-state-gcp
    prefix: production

# Unified resource mappings
resource_mappings:
  # AWS
  aws_instance:
    hostname_attribute: tags.Name
    address_attribute: public_ip
    fallback_address: private_ip
    host_vars:
      cloud_provider: aws

  # Azure
  azurerm_linux_virtual_machine:
    hostname_attribute: name
    address_attribute: public_ip_address
    host_vars:
      cloud_provider: azure

  # GCP
  google_compute_instance:
    hostname_attribute: name
    address_attribute: network_interface.0.access_config.0.nat_ip
    fallback_address: network_interface.0.network_ip
    host_vars:
      cloud_provider: gcp

# Cross-cloud groups
keyed_groups:
  - key: cloud_provider
    prefix: cloud
  - key: tags.Application
    prefix: app
```

---

## 8. Implementation Roadmap

### Phase 1: Core Infrastructure (Weeks 1-3)

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| State file parser (v4 format) | Critical | 3d | None |
| Local backend implementation | Critical | 2d | Parser |
| Resource mapping engine | Critical | 4d | Parser |
| Plugin registration | High | 1d | Mapping |
| Unit tests | High | 3d | All above |

### Phase 2: Remote Backends (Weeks 4-6)

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| S3 backend | High | 3d | Phase 1 |
| GCS backend | Medium | 2d | Phase 1 |
| Azure Blob backend | Medium | 2d | Phase 1 |
| HTTP backend | Low | 2d | Phase 1 |
| Consul backend | Low | 2d | Phase 1 |
| Backend authentication | High | 3d | All backends |

### Phase 3: Provisioner Mode (Weeks 7-8)

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| CLI subcommand | High | 2d | None |
| Connection retry logic | High | 2d | CLI |
| Variable injection | High | 2d | CLI |
| Terraform module | Medium | 1d | CLI |
| Documentation | Medium | 2d | All above |

### Phase 4: Variable Sharing (Weeks 9-10)

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| Output import | High | 3d | Phase 1 |
| Sensitive value handling | High | 2d | Import |
| Export facts module | Medium | 2d | None |
| Integration tests | High | 3d | All above |

### Phase 5: Polish and Documentation (Week 11-12)

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| Error handling improvement | High | 2d | All phases |
| Performance optimization | Medium | 3d | All phases |
| User documentation | High | 3d | All phases |
| Example workflows | High | 2d | All phases |
| Release preparation | High | 2d | All above |

---

## 9. Architecture Decision Records

### ADR-001: State File Format Support

**Status**: Accepted

**Context**: Terraform has had multiple state file format versions (v1-v4). Current versions use v4.

**Decision**: Support only v4 state format initially. Add v3 support if needed based on user feedback.

**Consequences**:
- Simpler implementation
- May exclude users with very old Terraform versions
- Can add backward compatibility later

### ADR-002: Backend Authentication

**Status**: Accepted

**Context**: Remote backends (S3, GCS, Azure) require authentication.

**Decision**: Use standard credential chain patterns:
- Environment variables
- Configuration files (~/.aws/credentials, etc.)
- Instance profiles/managed identities
- Explicit credentials in plugin config (discouraged)

**Consequences**:
- Familiar patterns for users
- Secure by default
- Some complexity in implementation

### ADR-003: Caching Strategy

**Status**: Accepted

**Context**: Remote state fetching can be slow and rate-limited.

**Decision**: Implement two-level caching:
1. In-memory cache with TTL
2. Optional local file cache for persistence

**Consequences**:
- Better performance
- Reduced API calls
- Potential for stale data (mitigated by TTL)

### ADR-004: Resource Mapping Approach

**Status**: Accepted

**Context**: Different cloud resources have different attribute structures.

**Decision**: Use declarative mapping configuration with:
- Dot-notation attribute access
- Template expressions for complex mapping
- Fallback values for optional attributes

**Consequences**:
- Flexible and extensible
- Users must configure mappings
- Can provide sensible defaults for common resources

### ADR-005: Provisioner Mode vs. Native Provider

**Status**: Accepted

**Context**: Could implement either:
1. Terraform provisioner (local-exec wrapper)
2. Native Terraform provider for Rustible

**Decision**: Implement provisioner mode first.

**Rationale**:
- Lower implementation effort
- No CGO or Go dependencies
- Simpler maintenance
- Sufficient for most use cases

**Consequences**:
- Less tight integration with Terraform
- Relies on local-exec
- Can implement provider later if needed

---

## Appendix A: Error Codes

| Code | Description |
|------|-------------|
| TF001 | State file not found |
| TF002 | Invalid state file format |
| TF003 | Backend authentication failed |
| TF004 | Resource mapping failed |
| TF005 | Required attribute missing |
| TF006 | Network error accessing remote backend |
| TF007 | Cache read/write error |
| TF008 | Unsupported state version |

## Appendix B: Configuration Reference

See full configuration schema in `/docs/reference/terraform-plugin-config.md`.

## Appendix C: Terraform Resource Type Reference

Common resource types and their default mappings:

| Provider | Resource Type | Default Hostname | Default Address |
|----------|--------------|------------------|-----------------|
| AWS | aws_instance | tags.Name | public_ip |
| AWS | aws_spot_instance_request | tags.Name | public_ip |
| Azure | azurerm_linux_virtual_machine | name | public_ip_address |
| Azure | azurerm_windows_virtual_machine | name | public_ip_address |
| GCP | google_compute_instance | name | network_interface.0.access_config.0.nat_ip |
| DigitalOcean | digitalocean_droplet | name | ipv4_address |
| Linode | linode_instance | label | ip_address |
| Vultr | vultr_instance | label | main_ip |
