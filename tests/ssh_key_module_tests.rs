//! Unit tests for SSH key management modules (authorized_key, known_hosts)
//!
//! This file includes comprehensive tests for SSH key parsing, manipulation,
//! state management, and module execution.

mod modules;

// Re-export the tests
pub use modules::authorized_key_tests;
pub use modules::known_hosts_tests;
