//! Azure cloud modules for infrastructure management.
//!
//! This module provides native Rust implementations for managing Azure resources.
//!
//! ## Available Modules
//!
//! - [`AzureVmModule`](vm::AzureVmModule): Virtual machine lifecycle management
//! - [`AzureResourceGroupModule`](vm::AzureResourceGroupModule): Resource group management
//! - [`AzureNetworkInterfaceModule`](vm::AzureNetworkInterfaceModule): Network interface management
//!
//! ## Authentication
//!
//! Azure credentials are loaded from the standard credential chain:
//!
//! 1. Environment variables (`AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`)
//! 2. Azure CLI credentials (`az login`)
//! 3. Managed Identity (when running on Azure infrastructure)
//! 4. Azure PowerShell credentials
//!
//! The subscription can be specified via:
//! - Module parameter (`subscription_id`)
//! - Environment variable (`AZURE_SUBSCRIPTION_ID`)
//! - Azure CLI default subscription
//!
//! ## Example
//!
//! ```yaml
//! - name: Create a resource group
//!   azure_resource_group:
//!     name: my-rg
//!     location: eastus
//!     state: present
//!     tags:
//!       Environment: production
//!
//! - name: Create an Azure VM
//!   azure_vm:
//!     name: web-server-01
//!     resource_group: my-rg
//!     location: eastus
//!     vm_size: Standard_B2s
//!     image:
//!       publisher: Canonical
//!       offer: 0001-com-ubuntu-server-jammy
//!       sku: 22_04-lts-gen2
//!       version: latest
//!     admin_username: azureuser
//!     ssh_public_keys:
//!       - path: /home/azureuser/.ssh/authorized_keys
//!         key_data: ssh-rsa AAAAB3...
//!     state: present
//! ```

pub mod vm;

pub use vm::{AzureNetworkInterfaceModule, AzureResourceGroupModule, AzureVmModule};
