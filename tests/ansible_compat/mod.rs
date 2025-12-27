//! Ansible Compatibility Test Suite
//!
//! This module provides comprehensive tests to verify that Rustible behaves
//! consistently with Ansible in key areas:
//!
//! - **Variable Precedence**: Tests the 20-level variable precedence hierarchy
//! - **Loop Behavior**: Tests loop constructs and loop control variables
//! - **Conditional Evaluation**: Tests when conditions and Jinja2 tests
//! - **Jinja2 Filters**: Tests common filters for string, list, and data manipulation
//!
//! ## Test Organization
//!
//! ```text
//! tests/ansible_compat/
//! +-- mod.rs                     # This file - module organization
//! +-- variable_precedence.rs     # Variable precedence tests
//! +-- loop_behavior.rs           # Loop construct tests
//! +-- conditionals.rs            # Conditional evaluation tests
//! +-- jinja2_filters.rs          # Jinja2 filter compatibility tests
//! +-- fixtures/
//!     +-- playbooks/             # Test playbook YAML files
//!     +-- vars/                  # Variable files for testing
//!     +-- inventory/             # Test inventory files
//!     +-- templates/             # Jinja2 template files
//! ```
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all Ansible compatibility tests
//! cargo test ansible_compat
//!
//! # Run specific test category
//! cargo test ansible_compat::variable_precedence
//! cargo test ansible_compat::loop_behavior
//! cargo test ansible_compat::conditionals
//! cargo test ansible_compat::jinja2_filters
//! ```

pub mod conditionals;
pub mod jinja2_filters;
pub mod loop_behavior;
pub mod variable_precedence;

use std::path::PathBuf;

/// Get the path to the ansible_compat fixtures directory
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ansible_compat")
        .join("fixtures")
}

/// Get the path to a specific fixture file
pub fn fixture_path(relative: &str) -> PathBuf {
    fixtures_dir().join(relative)
}

/// Common test utilities
pub mod test_utils {
    use indexmap::IndexMap;
    use serde_json::json;

    use rustible::executor::playbook::{Play, Playbook};
    use rustible::executor::runtime::RuntimeContext;
    use rustible::executor::task::Task;
    use rustible::executor::{Executor, ExecutorConfig};
    use rustible::vars::VarPrecedence;

    /// Create a test executor with specified hosts
    pub fn create_executor(hosts: Vec<&str>) -> Executor {
        let mut runtime = RuntimeContext::new();
        for host in hosts {
            runtime.add_host(host.to_string(), None);
        }

        let config = ExecutorConfig {
            gather_facts: false,
            ..Default::default()
        };

        Executor::with_runtime(config, runtime)
    }

    /// Create a test executor with hosts in groups
    pub fn create_executor_with_groups(host_groups: Vec<(&str, Option<&str>)>) -> Executor {
        let mut runtime = RuntimeContext::new();
        for (host, group) in host_groups {
            runtime.add_host(host.to_string(), group.map(|g| g.to_string()));
        }

        let config = ExecutorConfig {
            gather_facts: false,
            ..Default::default()
        };

        Executor::with_runtime(config, runtime)
    }

    /// Create a simple playbook with tasks
    pub fn create_playbook(name: &str, hosts: &str, tasks: Vec<Task>) -> Playbook {
        let mut playbook = Playbook::new(name);
        let mut play = Play::new(name, hosts);
        play.gather_facts = false;

        for task in tasks {
            play.add_task(task);
        }

        playbook.add_play(play);
        playbook
    }

    /// Create a playbook with play-level variables
    pub fn create_playbook_with_vars(
        name: &str,
        hosts: &str,
        vars: IndexMap<String, serde_json::Value>,
        tasks: Vec<Task>,
    ) -> Playbook {
        let mut playbook = Playbook::new(name);
        let mut play = Play::new(name, hosts);
        play.gather_facts = false;

        // Set play vars
        for (k, v) in vars {
            play.vars.set(k, v);
        }

        for task in tasks {
            play.add_task(task);
        }

        playbook.add_play(play);
        playbook
    }

    /// Create a runtime context with variables at different precedence levels
    pub fn create_runtime_with_precedence_vars() -> RuntimeContext {
        let mut runtime = RuntimeContext::new();
        runtime.add_host("localhost".to_string(), None);

        // Set variables at different precedence levels
        // These would normally be set from different sources

        runtime
    }

    /// Assert that a host result has expected stats
    pub fn assert_host_stats(
        results: &std::collections::HashMap<String, rustible::executor::HostResult>,
        host: &str,
        ok: usize,
        changed: usize,
        failed: usize,
        skipped: usize,
    ) {
        let result = results.get(host).expect(&format!("Host {} not found", host));
        assert_eq!(
            result.stats.ok, ok,
            "Expected {} ok tasks, got {}",
            ok, result.stats.ok
        );
        assert_eq!(
            result.stats.changed, changed,
            "Expected {} changed tasks, got {}",
            changed, result.stats.changed
        );
        assert_eq!(
            result.stats.failed, failed,
            "Expected {} failed tasks, got {}",
            failed, result.stats.failed
        );
        assert_eq!(
            result.stats.skipped, skipped,
            "Expected {} skipped tasks, got {}",
            skipped, result.stats.skipped
        );
    }
}
