//! Network Device Modules
//!
//! This module provides comprehensive network device automation that solves
//! Ansible's broken network automation. Unlike Ansible's network modules which
//! suffer from:
//! - Broken device_config module that doesn't properly detect changes
//! - Poor templating support for network configurations
//! - Inconsistent diff generation
//! - Lack of proper backup functionality
//!
//! Rustible's network modules provide:
//! - Proper configuration templating with Jinja2 support
//! - Accurate configuration diff generation using structural analysis
//! - Automatic configuration backup before changes
//! - Support for multiple transports (SSH, NETCONF, gRPC, REST)
//! - Idempotent configuration application
//! - Rollback support with configuration checkpoints
//!
//! # Supported Platforms
//!
//! - **Cisco IOS/IOS-XE**: Full support via `ios_config` module
//! - **Cisco IOS-XR**: Planned
//! - **Cisco NX-OS**: Planned
//! - **Arista EOS**: Planned
//! - **Juniper Junos**: Planned
//!
//! # Example Usage
//!
//! ```yaml
//! # Apply configuration with backup and diff
//! - name: Configure interface
//!   ios_config:
//!     lines:
//!       - ip address {{ interface_ip }} 255.255.255.0
//!       - no shutdown
//!     parents:
//!       - interface GigabitEthernet0/0
//!     backup: true
//!     save_when: modified
//!
//! # Apply configuration from template
//! - name: Apply router configuration
//!   ios_config:
//!     src: templates/router.j2
//!     backup: true
//!     diff_against: running
//! ```
//!
//! # Architecture
//!
//! ```text
//! +------------------+     +-------------------+
//! |   ios_config     |---->|  NetworkDevice    |
//! |   nxos_config    |     |   Connection      |
//! |   junos_config   |     +-------------------+
//! +------------------+              |
//!         |                         v
//!         |              +-------------------+
//!         +------------->|   Transport       |
//!                        |   - SSH/CLI       |
//!                        |   - NETCONF       |
//!                        |   - gRPC/gNMI     |
//!                        +-------------------+
//! ```

pub mod common;
pub mod eos_config;
pub mod ios_config;
pub mod junos_config;
pub mod nxos_config;

// Re-export main types for convenience
pub use common::{
    calculate_config_checksum, generate_backup_filename, generate_config_diff, parse_config_input,
    validate_config_lines, ConfigBackup, ConfigCommandGenerator, ConfigSection, ConfigSource,
    IosCommandGenerator, NetworkConfig, NetworkDeviceConnection, NetworkPlatform, NetworkTransport,
    SectionChange, SectionChangeType,
};
pub use eos_config::EosConfigModule;
pub use ios_config::{
    escape_config_text, extract_config_sections, generate_ios_diff_commands, parse_ios_config,
    IosConfigModule, IosConfigParams,
};
pub use junos_config::JunosConfigModule;
pub use nxos_config::NxosConfigModule;

use crate::modules::ModuleRegistry;
use std::sync::Arc;

/// Register all network modules with the registry
pub fn register_network_modules(registry: &mut ModuleRegistry) {
    registry.register(Arc::new(IosConfigModule));
    registry.register(Arc::new(JunosConfigModule));
    registry.register(Arc::new(NxosConfigModule));
    registry.register(Arc::new(EosConfigModule));
}

/// Get a list of all available network module names
pub fn network_module_names() -> Vec<&'static str> {
    vec!["ios_config", "junos_config", "nxos_config", "eos_config"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_names() {
        let names = network_module_names();
        assert!(names.contains(&"ios_config"));
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(format!("{}", NetworkPlatform::CiscoIos), "cisco_ios");
        assert_eq!(
            format!("{}", NetworkPlatform::JuniperJunos),
            "juniper_junos"
        );
    }

    #[test]
    fn test_transport_display() {
        assert_eq!(format!("{}", NetworkTransport::Ssh), "ssh");
        assert_eq!(format!("{}", NetworkTransport::Netconf), "netconf");
    }

    #[test]
    fn test_config_source_display() {
        assert_eq!(format!("{}", ConfigSource::Running), "running");
        assert_eq!(format!("{}", ConfigSource::Startup), "startup");
    }
}
