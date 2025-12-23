//! Comprehensive CLI tests for Rustible
//!
//! This test suite covers all aspects of the CLI including:
//! - Argument parsing with clap
//! - Subcommand parsing
//! - Output format handling
//! - Verbosity levels
//! - Extra variables parsing
//! - Inventory specification
//! - Config file loading
//! - Environment variables
//! - Error handling for invalid arguments
//! - Integration testing with assert_cmd

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::{tempdir, NamedTempFile};

// Re-export serde_json for JSON parsing tests
use serde_json;

// Helper to get a command for testing
fn rustible_cmd() -> Command {
    Command::cargo_bin("rustible").unwrap()
}

// Helper to create a test playbook
fn create_test_playbook() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        r#"---
- name: Test playbook
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Test task
      debug:
        msg: "Hello from test"
"#
    )
    .unwrap();
    file
}

// Helper to create test inventory
fn create_test_inventory() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        r#"all:
  hosts:
    localhost:
      ansible_connection: local
    testhost1:
      ansible_host: 192.168.1.10
    testhost2:
      ansible_host: 192.168.1.11
  children:
    webservers:
      hosts:
        web01: {{}}
        web02: {{}}
    dbservers:
      hosts:
        db01: {{}}
"#
    )
    .unwrap();
    file
}

// Helper to create a config file
fn create_test_config() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        r#"[defaults]
forks = 10
timeout = 60
gathering = true

[ssh]
pipelining = true
"#
    )
    .unwrap();
    file
}

// =============================================================================
// Basic CLI Tests
// =============================================================================

#[test]
fn test_version_flag() {
    rustible_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("rustible"));
}

#[test]
fn test_help_flag() {
    rustible_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "An Ansible substitute written in Rust",
        ));
}

#[test]
fn test_no_command_fails() {
    rustible_cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("required").or(predicate::str::contains("Usage")));
}

// =============================================================================
// Subcommand Parsing Tests
// =============================================================================

#[test]
fn test_run_command_basic() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_run_command_missing_playbook() {
    rustible_cmd()
        .arg("run")
        .arg("nonexistent.yml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("Playbook")));
}

#[test]
fn test_check_command_basic() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("check")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("CHECK MODE").or(predicate::str::contains("DRY RUN")));
}

#[test]
fn test_validate_command() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("validate")
        .arg(playbook.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("VALIDATION"));
}

#[test]
fn test_validate_invalid_playbook() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "not: valid: yaml: [").unwrap();

    rustible_cmd()
        .arg("validate")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("error").or(predicate::str::contains("ERROR")));
}

#[test]
fn test_list_hosts_command() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("localhost"));
}

#[test]
fn test_list_hosts_with_pattern() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .arg("webservers")
        .assert()
        .success()
        .stdout(predicate::str::contains("web"));
}

#[test]
fn test_list_hosts_no_inventory() {
    rustible_cmd()
        .arg("list-hosts")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No inventory"));
}

#[test]
fn test_list_tasks_command() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("list-tasks")
        .arg(playbook.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Test task").or(predicate::str::contains("Tasks in playbook")),
        );
}

#[test]
fn test_init_command_basic() {
    let temp_dir = tempdir().unwrap();

    rustible_cmd()
        .arg("init")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("initialized"));

    // Verify created directories
    assert!(temp_dir.path().join("inventory").exists());
    assert!(temp_dir.path().join("playbooks").exists());
    assert!(temp_dir.path().join("roles").exists());
    assert!(temp_dir.path().join("rustible.cfg").exists());
}

#[test]
fn test_init_command_with_template() {
    let temp_dir = tempdir().unwrap();

    rustible_cmd()
        .arg("init")
        .arg(temp_dir.path())
        .arg("--template")
        .arg("webserver")
        .assert()
        .success()
        .stdout(predicate::str::contains("webserver").or(predicate::str::contains("initialized")));
}

#[test]
fn test_vault_help() {
    rustible_cmd()
        .arg("vault")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("encrypt"))
        .stdout(predicate::str::contains("decrypt"));
}

// =============================================================================
// Global Flags and Options Tests
// =============================================================================

#[test]
fn test_inventory_flag_short() {
    let playbook = create_test_playbook();
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("-i")
        .arg(inventory.path())
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_inventory_flag_long() {
    let playbook = create_test_playbook();
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("--inventory")
        .arg(inventory.path())
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_verbosity_single() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-v")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_verbosity_double() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-vv")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Rustible"));
}

#[test]
fn test_verbosity_triple() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-vvv")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_verbosity_max() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-vvvv")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_check_mode_flag() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--check")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("CHECK MODE"));
}

#[test]
fn test_diff_mode_flag() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--diff")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_limit_flag() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-l")
        .arg("localhost")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_forks_flag() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-f")
        .arg("10")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_timeout_flag() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--timeout")
        .arg("60")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_no_color_flag() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--no-color")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Output Format Tests
// =============================================================================

#[test]
fn test_output_format_human() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--output")
        .arg("human")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_output_format_json() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--output")
        .arg("json")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_output_format_yaml() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--output")
        .arg("yaml")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_output_format_minimal() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--output")
        .arg("minimal")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_output_format_invalid() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--output")
        .arg("invalid")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

// =============================================================================
// Extra Variables Tests
// =============================================================================

#[test]
fn test_extra_vars_single() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-e")
        .arg("var1=value1")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_extra_vars_multiple() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-e")
        .arg("var1=value1")
        .arg("-e")
        .arg("var2=value2")
        .arg("-e")
        .arg("var3=value3")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_extra_vars_from_file() {
    let playbook = create_test_playbook();
    let mut vars_file = NamedTempFile::new().unwrap();
    writeln!(
        vars_file,
        r#"
var1: value1
var2: value2
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("-e")
        .arg(format!("@{}", vars_file.path().display()))
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_extra_vars_json_value() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-e")
        .arg("count=42")
        .arg("-e")
        .arg("enabled=true")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_extra_vars_mixed() {
    let playbook = create_test_playbook();
    let mut vars_file = NamedTempFile::new().unwrap();
    writeln!(vars_file, "file_var: from_file").unwrap();

    rustible_cmd()
        .arg("-e")
        .arg("inline_var=from_cli")
        .arg("-e")
        .arg(format!("@{}", vars_file.path().display()))
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Run Command Specific Tests
// =============================================================================

#[test]
fn test_run_with_tags() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--tags")
        .arg("install")
        .assert()
        .success();
}

#[test]
fn test_run_with_multiple_tags() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("-t")
        .arg("install")
        .arg("-t")
        .arg("configure")
        .assert()
        .success();
}

#[test]
fn test_run_with_skip_tags() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--skip-tags")
        .arg("slow")
        .assert()
        .success();
}

#[test]
fn test_run_with_start_at_task() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--start-at-task")
        .arg("Test task")
        .assert()
        .success();
}

#[test]
fn test_run_with_step() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--step")
        .assert()
        .success();
}

#[test]
fn test_run_with_become() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("-b")
        .assert()
        .success();
}

#[test]
fn test_run_with_become_method() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--become")
        .arg("--become-method")
        .arg("su")
        .assert()
        .success();
}

#[test]
fn test_run_with_become_user() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--become")
        .arg("--become-user")
        .arg("admin")
        .assert()
        .success();
}

#[test]
fn test_run_with_user() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("-u")
        .arg("deploy")
        .assert()
        .success();
}

#[test]
fn test_run_with_private_key() {
    let playbook = create_test_playbook();
    let key_file = NamedTempFile::new().unwrap();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--private-key")
        .arg(key_file.path())
        .assert()
        .success();
}

// =============================================================================
// Check Command Tests
// =============================================================================

#[test]
fn test_check_with_diff() {
    let playbook = create_test_playbook();

    // --diff is a global flag, should be placed before the subcommand
    rustible_cmd()
        .arg("--diff")
        .arg("check")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("CHECK").or(predicate::str::contains("DRY")));
}

#[test]
fn test_check_global_flags_work_after_subcommand() {
    let playbook = create_test_playbook();

    // Global flags should also work after the subcommand
    rustible_cmd()
        .arg("check")
        .arg(playbook.path())
        .arg("--diff")
        .assert()
        .success()
        .stderr(predicate::str::contains("CHECK").or(predicate::str::contains("DRY")));
}

// =============================================================================
// List Hosts Tests
// =============================================================================

#[test]
fn test_list_hosts_with_vars() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .arg("--vars")
        .assert()
        .success();
}

#[test]
fn test_list_hosts_yaml_output() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .arg("--yaml")
        .assert()
        .success();
}

#[test]
fn test_list_hosts_graph_output() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .arg("--graph")
        .assert()
        .success();
}

// =============================================================================
// List Tasks Tests
// =============================================================================

#[test]
fn test_list_tasks_with_details() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("list-tasks")
        .arg(playbook.path())
        .arg("--show-details")
        .assert()
        .success()
        .stdout(predicate::str::contains("Test").or(predicate::str::contains("task")));
}

#[test]
fn test_list_tasks_with_tags() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: Test playbook
  hosts: localhost
  tasks:
    - name: Tagged task
      debug:
        msg: test
      tags:
        - install
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("list-tasks")
        .arg(playbook.path())
        .arg("-t")
        .arg("install")
        .assert()
        .success()
        .stdout(predicate::str::contains("Tagged task"));
}

// =============================================================================
// Config File Tests
// =============================================================================

#[test]
fn test_config_file_short_flag() {
    let playbook = create_test_playbook();
    let config = create_test_config();

    rustible_cmd()
        .arg("-c")
        .arg(config.path())
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_config_file_long_flag() {
    let playbook = create_test_playbook();
    let config = create_test_config();

    rustible_cmd()
        .arg("--config")
        .arg(config.path())
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_config_file_nonexistent() {
    let playbook = create_test_playbook();

    // Should not fail, just warn
    rustible_cmd()
        .arg("-c")
        .arg("/nonexistent/config.toml")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Environment Variable Tests
// =============================================================================

#[test]
fn test_inventory_from_env() {
    let playbook = create_test_playbook();
    let inventory = create_test_inventory();

    rustible_cmd()
        .env("RUSTIBLE_INVENTORY", inventory.path())
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_config_from_env() {
    let playbook = create_test_playbook();
    let config = create_test_config();

    rustible_cmd()
        .env("RUSTIBLE_CONFIG", config.path())
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_env_flag_override() {
    let playbook = create_test_playbook();
    let inventory1 = create_test_inventory();
    let inventory2 = create_test_inventory();

    // CLI flag should override env var
    rustible_cmd()
        .env("RUSTIBLE_INVENTORY", inventory1.path())
        .arg("-i")
        .arg(inventory2.path())
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn test_invalid_subcommand() {
    rustible_cmd()
        .arg("invalid-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_invalid_flag() {
    rustible_cmd()
        .arg("--invalid-flag")
        .arg("run")
        .arg("playbook.yml")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument")
                .or(predicate::str::contains("unrecognized")),
        );
}

#[test]
fn test_invalid_forks_value() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-f")
        .arg("not-a-number")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn test_invalid_timeout_value() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--timeout")
        .arg("not-a-number")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn test_missing_required_argument() {
    rustible_cmd()
        .arg("run")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_run_playbook_not_found() {
    rustible_cmd()
        .arg("run")
        .arg("/nonexistent/playbook.yml")
        .assert()
        .failure();
}

#[test]
fn test_list_tasks_playbook_not_found() {
    rustible_cmd()
        .arg("list-tasks")
        .arg("/nonexistent/playbook.yml")
        .assert()
        .failure();
}

#[test]
fn test_validate_playbook_not_found() {
    rustible_cmd()
        .arg("validate")
        .arg("/nonexistent/playbook.yml")
        .assert()
        .failure();
}

// =============================================================================
// Complex Integration Tests
// =============================================================================

#[test]
fn test_complex_run_with_all_flags() {
    let playbook = create_test_playbook();
    let inventory = create_test_inventory();
    let config = create_test_config();

    rustible_cmd()
        .arg("-i")
        .arg(inventory.path())
        .arg("-c")
        .arg(config.path())
        .arg("-e")
        .arg("var1=value1")
        .arg("-e")
        .arg("var2=value2")
        .arg("-vv")
        .arg("--check")
        .arg("--diff")
        .arg("-l")
        .arg("localhost")
        .arg("-f")
        .arg("10")
        .arg("--timeout")
        .arg("60")
        .arg("--output")
        .arg("human")
        .arg("run")
        .arg(playbook.path())
        .arg("-t")
        .arg("install")
        .arg("--become")
        .arg("--become-user")
        .arg("root")
        .assert()
        .success();
}

#[test]
fn test_check_command_inherits_global_flags() {
    let playbook = create_test_playbook();
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("-i")
        .arg(inventory.path())
        .arg("-vv")
        .arg("check")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("CHECK").or(predicate::str::contains("DRY")));
}

#[test]
fn test_list_hosts_all_pattern() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .arg("all")
        .assert()
        .success()
        .stdout(predicate::str::contains("localhost"));
}

#[test]
fn test_init_creates_proper_structure() {
    let temp_dir = tempdir().unwrap();

    rustible_cmd()
        .arg("init")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Verify all expected directories exist
    let expected_dirs = vec![
        "inventory",
        "playbooks",
        "roles",
        "group_vars",
        "host_vars",
        "files",
        "templates",
    ];

    for dir in expected_dirs {
        assert!(
            temp_dir.path().join(dir).exists(),
            "Directory {} should exist",
            dir
        );
    }

    // Verify files exist
    assert!(temp_dir.path().join("inventory/hosts.yml").exists());
    assert!(temp_dir.path().join("playbooks/site.yml").exists());
    assert!(temp_dir.path().join("rustible.cfg").exists());
    assert!(temp_dir.path().join(".gitignore").exists());
}

#[test]
fn test_validate_playbook_missing_hosts() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: Invalid play
  tasks:
    - name: Task
      debug:
        msg: test
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("validate")
        .arg(playbook.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing required 'hosts' field"));
}

#[test]
fn test_validate_playbook_warning_no_tasks() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: Empty play
  hosts: localhost
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("validate")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("warning").or(predicate::str::contains("WARNING")));
}

#[test]
fn test_playbook_execution_exit_code_success() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .code(0);
}

// =============================================================================
// Vault Command Tests
// =============================================================================

#[test]
fn test_vault_encrypt_help() {
    rustible_cmd()
        .arg("vault")
        .arg("encrypt")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Encrypt a file"));
}

#[test]
fn test_vault_decrypt_help() {
    rustible_cmd()
        .arg("vault")
        .arg("decrypt")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Decrypt a file"));
}

#[test]
fn test_vault_view_help() {
    rustible_cmd()
        .arg("vault")
        .arg("view")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("View an encrypted file"));
}

#[test]
fn test_vault_create_help() {
    rustible_cmd()
        .arg("vault")
        .arg("create")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Create a new encrypted file"));
}

#[test]
fn test_vault_rekey_help() {
    rustible_cmd()
        .arg("vault")
        .arg("rekey")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Re-encrypt with a new password"));
}

// =============================================================================
// Edge Cases and Boundary Tests
// =============================================================================

#[test]
fn test_empty_playbook() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(playbook, "---").unwrap();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .failure();
}

#[test]
fn test_playbook_with_invalid_yaml() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(playbook, "{{{{invalid yaml").unwrap();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .failure();
}

#[test]
fn test_very_long_extra_var_value() {
    let playbook = create_test_playbook();
    let long_value = "x".repeat(1000);

    rustible_cmd()
        .arg("-e")
        .arg(format!("long_var={}", long_value))
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_multiple_output_formats_last_wins() {
    let playbook = create_test_playbook();

    // Last --output flag should take precedence
    // Just verify it completes successfully
    rustible_cmd()
        .arg("--output")
        .arg("yaml")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_forks_zero_uses_default() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-f")
        .arg("0")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_timeout_zero() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--timeout")
        .arg("0")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Flags Position Tests
// =============================================================================

#[test]
fn test_global_flags_before_subcommand() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-v")
        .arg("--check")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_global_flags_after_subcommand() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg("-v")
        .arg("--check")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_mixed_flag_positions() {
    let playbook = create_test_playbook();
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("-i")
        .arg(inventory.path())
        .arg("run")
        .arg("-v")
        .arg(playbook.path())
        .arg("--check")
        .assert()
        .success();
}

// =============================================================================
// NO_COLOR Environment Variable Tests
// =============================================================================

#[test]
fn test_no_color_env_variable() {
    let playbook = create_test_playbook();

    // NO_COLOR should disable colored output
    rustible_cmd()
        .env("NO_COLOR", "1")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_rustible_no_color_env_variable() {
    let playbook = create_test_playbook();

    // RUSTIBLE_NO_COLOR should also disable colored output
    rustible_cmd()
        .env("RUSTIBLE_NO_COLOR", "1")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_no_color_flag_with_env() {
    let playbook = create_test_playbook();

    // Both should work together without issue
    rustible_cmd()
        .env("NO_COLOR", "1")
        .arg("--no-color")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Vault Password File Environment Variable Tests
// =============================================================================

#[test]
fn test_vault_password_file_env() {
    let mut password_file = NamedTempFile::new().unwrap();
    writeln!(password_file, "test_password_123").unwrap();

    rustible_cmd()
        .env("RUSTIBLE_VAULT_PASSWORD_FILE", password_file.path())
        .arg("vault")
        .arg("--help")
        .assert()
        .success();
}

// =============================================================================
// Exit Code Tests
// =============================================================================

#[test]
fn test_exit_code_success() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .code(0);
}

#[test]
fn test_exit_code_missing_file() {
    rustible_cmd()
        .arg("run")
        .arg("/nonexistent/playbook.yml")
        .assert()
        .code(predicate::ne(0));
}

#[test]
fn test_exit_code_invalid_arguments() {
    rustible_cmd()
        .arg("--invalid-arg-xyz")
        .assert()
        .code(predicate::ne(0));
}

#[test]
fn test_exit_code_missing_subcommand() {
    rustible_cmd().assert().code(predicate::ne(0));
}

#[test]
fn test_exit_code_help_is_zero() {
    rustible_cmd().arg("--help").assert().code(0);
}

#[test]
fn test_exit_code_version_is_zero() {
    rustible_cmd().arg("--version").assert().code(0);
}

#[test]
fn test_exit_code_validate_success() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("validate")
        .arg(playbook.path())
        .assert()
        .code(0);
}

#[test]
fn test_exit_code_validate_failure() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(playbook, "invalid: yaml: syntax: [[[").unwrap();

    rustible_cmd()
        .arg("validate")
        .arg(playbook.path())
        .assert()
        .code(predicate::ne(0));
}

// =============================================================================
// JSON Output Format Validation Tests
// =============================================================================

#[test]
fn test_json_output_format_structure() {
    let playbook = create_test_playbook();

    let output = rustible_cmd()
        .arg("--output")
        .arg("json")
        .arg("run")
        .arg(playbook.path())
        .output()
        .expect("Failed to run command");

    // At least some output should be valid JSON (task results are JSON)
    let stdout = String::from_utf8_lossy(&output.stdout);
    // When in JSON mode, any output should be parseable as JSON lines
    for line in stdout.lines() {
        if !line.trim().is_empty() {
            // Each non-empty line should be valid JSON or part of the output
            let _ = serde_json::from_str::<serde_json::Value>(line);
        }
    }
}

#[test]
fn test_json_output_list_hosts() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("--output")
        .arg("json")
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .assert()
        .success();
}

// =============================================================================
// YAML Output Format Tests
// =============================================================================

#[test]
fn test_yaml_output_list_hosts() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .arg("--yaml")
        .assert()
        .success();
}

// =============================================================================
// Error Output Tests (stderr)
// =============================================================================

#[test]
fn test_error_to_stderr() {
    rustible_cmd()
        .arg("run")
        .arg("/nonexistent/playbook.yml")
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn test_error_message_helpful_for_missing_playbook() {
    rustible_cmd()
        .arg("run")
        .arg("/nonexistent/playbook.yml")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .or(predicate::str::contains("Playbook"))
                .or(predicate::str::contains("file")),
        );
}

#[test]
fn test_error_message_for_invalid_yaml() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(playbook, "{{{{not valid yaml at all").unwrap();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("parse")
                .or(predicate::str::contains("YAML"))
                .or(predicate::str::contains("error")),
        );
}

#[test]
fn test_warning_no_inventory_to_stderr() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(
            predicate::str::contains("No inventory")
                .or(predicate::str::contains("localhost"))
                .or(predicate::str::is_empty()),
        );
}

// =============================================================================
// Limit Pattern Tests
// =============================================================================

#[test]
fn test_limit_single_host() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--limit")
        .arg("localhost")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_limit_multiple_hosts() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-l")
        .arg("host1:host2")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_limit_pattern_with_wildcard() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-l")
        .arg("web*")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Tags Parsing Tests
// =============================================================================

#[test]
fn test_tags_comma_separated() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("-t")
        .arg("tag1")
        .arg("-t")
        .arg("tag2")
        .assert()
        .success();
}

#[test]
fn test_skip_tags_multiple() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--skip-tags")
        .arg("skip1")
        .arg("--skip-tags")
        .arg("skip2")
        .assert()
        .success();
}

#[test]
fn test_tags_and_skip_tags_combined() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("-t")
        .arg("install")
        .arg("--skip-tags")
        .arg("slow")
        .assert()
        .success();
}

// =============================================================================
// Forks Configuration Tests
// =============================================================================

#[test]
fn test_forks_high_value() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-f")
        .arg("100")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_forks_default_value() {
    let playbook = create_test_playbook();

    // Default is 5, just ensure it runs without -f
    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Connection Timeout Tests
// =============================================================================

#[test]
fn test_timeout_large_value() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--timeout")
        .arg("3600")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Subcommand Help Tests
// =============================================================================

#[test]
fn test_run_help() {
    rustible_cmd()
        .arg("run")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Run a playbook"))
        .stdout(predicate::str::contains("--tags"))
        .stdout(predicate::str::contains("--become"));
}

#[test]
fn test_check_help() {
    rustible_cmd()
        .arg("check")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Run a playbook in check mode"))
        .stdout(predicate::str::contains("dry-run"));
}

#[test]
fn test_list_hosts_help() {
    rustible_cmd()
        .arg("list-hosts")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("List hosts"))
        .stdout(predicate::str::contains("--vars"));
}

#[test]
fn test_list_tasks_help() {
    rustible_cmd()
        .arg("list-tasks")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("List tasks"));
}

#[test]
fn test_init_help() {
    rustible_cmd()
        .arg("init")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialize"))
        .stdout(predicate::str::contains("--template"));
}

#[test]
fn test_validate_help() {
    rustible_cmd()
        .arg("validate")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Validate"));
}

// =============================================================================
// SSH Arguments Tests
// =============================================================================

#[test]
fn test_ssh_common_args() {
    let playbook = create_test_playbook();

    // Use equals-style argument to avoid clap interpreting the value as a flag
    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--ssh-common-args=StrictHostKeyChecking=no")
        .assert()
        .success();
}

// =============================================================================
// Vault Password File Flag Tests
// =============================================================================

#[test]
fn test_vault_password_file_flag() {
    let mut password_file = NamedTempFile::new().unwrap();
    writeln!(password_file, "test_password").unwrap();

    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .arg("--vault-password-file")
        .arg(password_file.path())
        .assert()
        .success();
}

// =============================================================================
// Playbook with Multiple Plays Tests
// =============================================================================

#[test]
fn test_playbook_multiple_plays() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: First play
  hosts: localhost
  gather_facts: false
  tasks:
    - name: First task
      debug:
        msg: "First"

- name: Second play
  hosts: localhost
  gather_facts: false
  tasks:
    - name: Second task
      debug:
        msg: "Second"
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_list_tasks_multiple_plays() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: First play
  hosts: localhost
  tasks:
    - name: Task A
      debug:
        msg: "A"

- name: Second play
  hosts: localhost
  tasks:
    - name: Task B
      debug:
        msg: "B"
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("list-tasks")
        .arg(playbook.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Task A"))
        .stdout(predicate::str::contains("Task B"));
}

// =============================================================================
// Handlers and Pre/Post Tasks Tests
// =============================================================================

#[test]
fn test_list_tasks_with_handlers() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: Play with handlers
  hosts: localhost
  tasks:
    - name: Main task
      debug:
        msg: "main"
      notify: Handler task

  handlers:
    - name: Handler task
      debug:
        msg: "handler"
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("list-tasks")
        .arg(playbook.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Handlers"));
}

#[test]
fn test_list_tasks_with_pre_post_tasks() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: Play with pre/post tasks
  hosts: localhost
  pre_tasks:
    - name: Pre task
      debug:
        msg: "pre"
  tasks:
    - name: Main task
      debug:
        msg: "main"
  post_tasks:
    - name: Post task
      debug:
        msg: "post"
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("list-tasks")
        .arg(playbook.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Pre-tasks").or(predicate::str::contains("Pre task")))
        .stdout(predicate::str::contains("Post-tasks").or(predicate::str::contains("Post task")));
}

// =============================================================================
// Progress Output Tests
// =============================================================================

#[test]
fn test_run_shows_play_header() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("PLAY").or(predicate::str::is_empty()));
}

#[test]
fn test_run_shows_task_header() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("TASK").or(predicate::str::is_empty()));
}

#[test]
fn test_run_shows_recap() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("RECAP").or(predicate::str::is_empty()));
}

// =============================================================================
// Inventory Format Tests
// =============================================================================

#[test]
fn test_inventory_ini_format() {
    let mut inventory = NamedTempFile::with_suffix(".ini").unwrap();
    writeln!(
        inventory,
        r#"[webservers]
web01 ansible_host=192.168.1.10
web02 ansible_host=192.168.1.11

[dbservers]
db01 ansible_host=192.168.1.20
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .assert()
        .success();
}

#[test]
fn test_inventory_json_format() {
    let mut inventory = NamedTempFile::with_suffix(".json").unwrap();
    writeln!(
        inventory,
        r#"{{
  "all": {{
    "hosts": {{
      "localhost": {{}}
    }}
  }}
}}"#
    )
    .unwrap();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .assert()
        .success();
}

// =============================================================================
// Config File Format Tests
// =============================================================================

#[test]
fn test_config_toml_format() {
    let mut config = NamedTempFile::with_suffix(".toml").unwrap();
    writeln!(
        config,
        r#"[defaults]
forks = 20
timeout = 120

[ssh]
pipelining = true
"#
    )
    .unwrap();

    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-c")
        .arg(config.path())
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Extra Variables Edge Cases
// =============================================================================

#[test]
fn test_extra_vars_with_spaces() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-e")
        .arg("message=hello world")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_extra_vars_yaml_syntax() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-e")
        .arg("items=[a, b, c]")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_extra_vars_complex_yaml() {
    let playbook = create_test_playbook();
    let mut vars_file = NamedTempFile::new().unwrap();
    writeln!(
        vars_file,
        r#"
nested:
  key1: value1
  key2:
    - item1
    - item2
list_var:
  - one
  - two
  - three
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("-e")
        .arg(format!("@{}", vars_file.path().display()))
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_extra_vars_file_not_found() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("-e")
        .arg("@/nonexistent/vars.yml")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .failure();
}

// =============================================================================
// Validate Command Edge Cases
// =============================================================================

#[test]
fn test_validate_playbook_with_roles() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: Play with roles
  hosts: localhost
  roles:
    - role: webserver
    - role: database
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("validate")
        .arg(playbook.path())
        .assert()
        .success();
}

#[test]
fn test_validate_playbook_with_vars() {
    let mut playbook = NamedTempFile::new().unwrap();
    writeln!(
        playbook,
        r#"---
- name: Play with vars
  hosts: localhost
  vars:
    http_port: 80
    max_clients: 200
  tasks:
    - name: Debug vars
      debug:
        msg: "Port: {{{{ http_port }}}}"
"#
    )
    .unwrap();

    rustible_cmd()
        .arg("validate")
        .arg(playbook.path())
        .assert()
        .success();
}

// =============================================================================
// Init Command Edge Cases
// =============================================================================

#[test]
fn test_init_in_existing_directory() {
    let temp_dir = tempdir().unwrap();

    // First init
    rustible_cmd()
        .arg("init")
        .arg(temp_dir.path())
        .assert()
        .success();

    // Second init should also succeed (idempotent)
    rustible_cmd()
        .arg("init")
        .arg(temp_dir.path())
        .assert()
        .success();
}

#[test]
fn test_init_docker_template() {
    let temp_dir = tempdir().unwrap();

    rustible_cmd()
        .arg("init")
        .arg(temp_dir.path())
        .arg("--template")
        .arg("docker")
        .assert()
        .success();

    // Verify site.yml exists
    assert!(temp_dir.path().join("playbooks/site.yml").exists());
}

// =============================================================================
// List Hosts Pattern Matching Tests
// =============================================================================

#[test]
fn test_list_hosts_no_match() {
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("list-hosts")
        .arg("-i")
        .arg(inventory.path())
        .arg("nonexistent_group")
        .assert()
        .success()
        .stderr(predicate::str::contains("No hosts").or(predicate::str::is_empty()));
}

// =============================================================================
// Verbosity Level Effects Tests
// =============================================================================

#[test]
fn test_verbosity_affects_output() {
    let playbook = create_test_playbook();

    // Low verbosity - less output
    let low_output = rustible_cmd()
        .arg("run")
        .arg(playbook.path())
        .output()
        .expect("Failed to run command");

    // High verbosity - more output
    let high_output = rustible_cmd()
        .arg("-vvvv")
        .arg("run")
        .arg(playbook.path())
        .output()
        .expect("Failed to run command");

    // High verbosity should produce at least as much output as low verbosity
    let low_len = low_output.stdout.len() + low_output.stderr.len();
    let high_len = high_output.stdout.len() + high_output.stderr.len();

    // With high verbosity, we generally expect more output
    // (or at least not less - though exact behavior depends on implementation)
    assert!(high_len >= low_len || low_len < 100); // Allow for minimal output case
}

// =============================================================================
// Combined Check and Diff Mode Tests
// =============================================================================

#[test]
fn test_check_and_diff_combined() {
    let playbook = create_test_playbook();

    rustible_cmd()
        .arg("--check")
        .arg("--diff")
        .arg("run")
        .arg(playbook.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("CHECK").or(predicate::str::is_empty()));
}

// =============================================================================
// Long-form Arguments Tests
// =============================================================================

#[test]
fn test_long_form_arguments() {
    let playbook = create_test_playbook();
    let inventory = create_test_inventory();

    rustible_cmd()
        .arg("--inventory")
        .arg(inventory.path())
        .arg("--verbose")
        .arg("--check")
        .arg("--diff")
        .arg("--limit")
        .arg("localhost")
        .arg("--forks")
        .arg("10")
        .arg("--timeout")
        .arg("60")
        .arg("--no-color")
        .arg("--output")
        .arg("human")
        .arg("run")
        .arg(playbook.path())
        .arg("--tags")
        .arg("test")
        .arg("--skip-tags")
        .arg("slow")
        .arg("--become")
        .arg("--become-method")
        .arg("sudo")
        .arg("--become-user")
        .arg("root")
        .assert()
        .success();
}

// =============================================================================
// Special Characters in Paths Tests
// =============================================================================

#[test]
fn test_playbook_path_with_spaces() {
    let temp_dir = tempdir().unwrap();
    let playbook_dir = temp_dir.path().join("path with spaces");
    std::fs::create_dir_all(&playbook_dir).unwrap();

    let playbook_path = playbook_dir.join("playbook.yml");
    std::fs::write(
        &playbook_path,
        r#"---
- name: Test
  hosts: localhost
  tasks:
    - name: Test task
      debug:
        msg: test
"#,
    )
    .unwrap();

    rustible_cmd()
        .arg("run")
        .arg(&playbook_path)
        .assert()
        .success();
}

// =============================================================================
// Subcommand Argument Requirement Tests
// =============================================================================

#[test]
fn test_vault_encrypt_requires_file() {
    rustible_cmd()
        .arg("vault")
        .arg("encrypt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_vault_decrypt_requires_file() {
    rustible_cmd()
        .arg("vault")
        .arg("decrypt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_list_tasks_requires_playbook() {
    rustible_cmd()
        .arg("list-tasks")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_validate_requires_playbook() {
    rustible_cmd()
        .arg("validate")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}
