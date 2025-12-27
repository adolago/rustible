//! Unit tests for newly integrated modules (archive, uri, pause, wait_for)
//!
//! This file includes comprehensive tests for the following modules:
//! - archive: Creating compressed archives (tar, tar.gz, zip)
//! - uri: HTTP request handling with authentication
//! - pause: Playbook execution pausing
//! - wait_for: Waiting for conditions (port, path, regex)

mod modules;

// Re-export the tests
pub use modules::archive_tests;
pub use modules::pause_tests;
pub use modules::uri_tests;
pub use modules::wait_for_tests;
