//! Docker modules for container, image, network, volume, and compose management
//!
//! This module provides Ansible-compatible Docker management modules using the bollard crate.
//!
//! ## Available Modules
//!
//! - `docker_container` - Manage Docker containers (create, start, stop, remove)
//! - `docker_image` - Manage Docker images (pull, build, tag, push, remove)
//! - `docker_network` - Manage Docker networks (create, connect, remove)
//! - `docker_volume` - Manage Docker volumes (create, remove)
//! - `docker_compose` - Manage Docker Compose projects (up, down, restart)
//!
//! ## Feature Flag
//!
//! These modules require the `docker` feature to be enabled:
//!
//! ```toml
//! [dependencies]
//! rustible = { version = "0.1", features = ["docker"] }
//! ```
//!
//! ## Example Usage
//!
//! ```yaml
//! # Start a container
//! - docker_container:
//!     name: my-nginx
//!     image: nginx:latest
//!     state: started
//!     ports:
//!       - "8080:80"
//!
//! # Pull an image
//! - docker_image:
//!     name: redis
//!     tag: alpine
//!     source: pull
//!
//! # Create a network
//! - docker_network:
//!     name: my-network
//!     driver: bridge
//!
//! # Create a volume
//! - docker_volume:
//!     name: my-data
//!     driver: local
//!
//! # Deploy with Docker Compose
//! - docker_compose:
//!     project_src: /app
//!     state: present
//! ```

pub mod docker_compose;
pub mod docker_container;
pub mod docker_image;
pub mod docker_network;
pub mod docker_volume;

pub use docker_compose::DockerComposeModule;
pub use docker_container::DockerContainerModule;
pub use docker_image::DockerImageModule;
pub use docker_network::DockerNetworkModule;
pub use docker_volume::DockerVolumeModule;

// Re-export common types
pub use docker_container::{ContainerConfig, ContainerState, PullPolicy};
pub use docker_image::{BuildConfig, ImageConfig, ImageSource, ImageState};
pub use docker_network::{IpamConfiguration, NetworkConfig, NetworkDriver, NetworkState, SubnetConfig};
pub use docker_volume::{VolumeConfig, VolumeState};
pub use docker_compose::{ComposeConfig, ComposeState, ComposePullPolicy, RecreatePolicy};
