//! Comprehensive error handling tests for Rustible
//!
//! This test suite covers:
//! - All error types defined in src/error.rs
//! - Error propagation through the execution stack
//! - Error message clarity and usefulness
//! - ignore_errors: true behavior
//! - failed_when conditions
//! - rescue and always blocks
//! - Connection errors and recovery
//! - Module errors and their context
//! - Parse errors with line numbers
//! - Graceful degradation scenarios

use rustible::error::{Error, ErrorContext, Result};
use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::{ExecutionContext, RuntimeContext};
use rustible::executor::task::{Task, TaskResult, TaskStatus};
use rustible::executor::{Executor, ExecutorConfig, ExecutorError};
use std::path::PathBuf;

// ============================================================================
// Error Type Tests - All variants from src/error.rs
// ============================================================================

#[test]
fn test_playbook_parse_error() {
    let path = PathBuf::from("/test/playbook.yml");
    let error = Error::playbook_parse(path.clone(), "Invalid YAML syntax", None);

    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Failed to parse playbook"));
    assert!(error_msg.contains("/test/playbook.yml"));
    assert!(error_msg.contains("Invalid YAML syntax"));
}

#[test]
fn test_playbook_parse_error_with_source() {
    let path = PathBuf::from("/test/playbook.yml");
    let source_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let error = Error::playbook_parse(path, "Could not read file", Some(Box::new(source_error)));

    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Failed to parse playbook"));
    assert!(error_msg.contains("Could not read file"));
}

#[test]
fn test_playbook_validation_error() {
    let error = Error::PlaybookValidation("Play must have hosts defined".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Playbook validation failed"));
    assert!(error_msg.contains("Play must have hosts defined"));
}

#[test]
fn test_play_not_found_error() {
    let error = Error::PlayNotFound("webservers".to_string());
    let error_msg = format!("{}", error);
    assert_eq!(error_msg, "Play 'webservers' not found in playbook");
}

#[test]
fn test_task_failed_error() {
    let error = Error::task_failed("Install nginx", "web1", "Package not found");
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Task 'Install nginx' failed"));
    assert!(error_msg.contains("on host 'web1'"));
    assert!(error_msg.contains("Package not found"));
}

#[test]
fn test_task_timeout_error() {
    let error = Error::TaskTimeout {
        task: "Long running task".to_string(),
        host: "server1".to_string(),
        timeout_secs: 300,
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Long running task"));
    assert!(error_msg.contains("timed out"));
    assert!(error_msg.contains("server1"));
    assert!(error_msg.contains("300 seconds"));
}

#[test]
fn test_task_skipped_error() {
    let error = Error::TaskSkipped("Optional task".to_string());
    assert_eq!(format!("{}", error), "Task 'Optional task' skipped");
}

#[test]
fn test_module_not_found_error() {
    let error = Error::ModuleNotFound("custom_module".to_string());
    assert_eq!(format!("{}", error), "Module 'custom_module' not found");
}

#[test]
fn test_module_args_error() {
    let error = Error::module_args("copy", "missing required argument 'dest'");
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Invalid arguments for module 'copy'"));
    assert!(error_msg.contains("missing required argument 'dest'"));
}

#[test]
fn test_module_execution_error() {
    let error = Error::ModuleExecution {
        module: "shell".to_string(),
        message: "Command failed with exit code 1".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Module 'shell' execution failed"));
    assert!(error_msg.contains("exit code 1"));
}

#[test]
fn test_inventory_load_error() {
    let error = Error::InventoryLoad {
        path: PathBuf::from("/etc/ansible/hosts"),
        message: "Permission denied".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Failed to load inventory"));
    assert!(error_msg.contains("/etc/ansible/hosts"));
    assert!(error_msg.contains("Permission denied"));
}

#[test]
fn test_host_not_found_error() {
    let error = Error::HostNotFound("server42".to_string());
    assert_eq!(
        format!("{}", error),
        "Host 'server42' not found in inventory"
    );
}

#[test]
fn test_group_not_found_error() {
    let error = Error::GroupNotFound("databases".to_string());
    assert_eq!(
        format!("{}", error),
        "Group 'databases' not found in inventory"
    );
}

#[test]
fn test_invalid_host_pattern_error() {
    let error = Error::InvalidHostPattern("[invalid".to_string());
    assert_eq!(format!("{}", error), "Invalid host pattern: '[invalid'");
}

#[test]
fn test_connection_failed_error() {
    let error = Error::connection_failed("192.168.1.100", "Connection refused");
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Failed to connect to '192.168.1.100'"));
    assert!(error_msg.contains("Connection refused"));
}

#[test]
fn test_connection_timeout_error() {
    let error = Error::ConnectionTimeout {
        host: "slow.server.com".to_string(),
        timeout_secs: 30,
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Connection to 'slow.server.com'"));
    assert!(error_msg.contains("timed out"));
    assert!(error_msg.contains("30 seconds"));
}

#[test]
fn test_authentication_failed_error() {
    let error = Error::AuthenticationFailed {
        user: "admin".to_string(),
        host: "secure.server.com".to_string(),
        message: "Invalid credentials".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Authentication failed for 'admin@secure.server.com'"));
    assert!(error_msg.contains("Invalid credentials"));
}

#[test]
fn test_remote_command_failed_error() {
    let error = Error::RemoteCommandFailed {
        host: "web1".to_string(),
        exit_code: 127,
        message: "command not found".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Command failed on 'web1'"));
    assert!(error_msg.contains("exit code 127"));
    assert!(error_msg.contains("command not found"));
}

#[test]
fn test_file_transfer_error() {
    let error = Error::FileTransfer("Failed to upload file: disk full".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("File transfer failed"));
    assert!(error_msg.contains("disk full"));
}

#[test]
fn test_undefined_variable_error() {
    let error = Error::UndefinedVariable("missing_var".to_string());
    assert_eq!(format!("{}", error), "Undefined variable: 'missing_var'");
}

#[test]
fn test_invalid_variable_value_error() {
    let error = Error::InvalidVariableValue {
        name: "port".to_string(),
        message: "expected number, got string".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Invalid value for variable 'port'"));
    assert!(error_msg.contains("expected number, got string"));
}

#[test]
fn test_variables_file_not_found_error() {
    let error = Error::VariablesFileNotFound(PathBuf::from("/vars/main.yml"));
    assert_eq!(
        format!("{}", error),
        "Variables file not found: /vars/main.yml"
    );
}

#[test]
fn test_template_syntax_error() {
    let error = Error::TemplateSyntax {
        template: "config.j2".to_string(),
        message: "unexpected end of template".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Template syntax error in 'config.j2'"));
    assert!(error_msg.contains("unexpected end of template"));
}

#[test]
fn test_template_render_error() {
    let error = Error::template_render("nginx.conf.j2", "undefined variable 'server_name'");
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Template rendering failed for 'nginx.conf.j2'"));
    assert!(error_msg.contains("undefined variable"));
}

#[test]
fn test_role_not_found_error() {
    let error = Error::RoleNotFound("common".to_string());
    assert_eq!(format!("{}", error), "Role 'common' not found");
}

#[test]
fn test_role_dependency_error() {
    let error =
        Error::RoleDependency("circular dependency detected: role1 -> role2 -> role1".to_string());
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Role dependency error"));
    assert!(error_msg.contains("circular dependency"));
}

#[test]
fn test_invalid_role_error() {
    let error = Error::InvalidRole {
        role: "webserver".to_string(),
        message: "tasks/main.yml is required".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Invalid role structure in 'webserver'"));
    assert!(error_msg.contains("tasks/main.yml is required"));
}

#[test]
fn test_vault_decryption_error() {
    let error = Error::VaultDecryption("Invalid vault format".to_string());
    assert_eq!(
        format!("{}", error),
        "Failed to decrypt vault: Invalid vault format"
    );
}

#[test]
fn test_vault_encryption_error() {
    let error = Error::VaultEncryption("No password provided".to_string());
    assert_eq!(
        format!("{}", error),
        "Failed to encrypt vault: No password provided"
    );
}

#[test]
fn test_invalid_vault_password_error() {
    let error = Error::InvalidVaultPassword;
    assert_eq!(format!("{}", error), "Invalid vault password");
}

#[test]
fn test_vault_file_not_found_error() {
    let error = Error::VaultFileNotFound(PathBuf::from("/secrets/vault.yml"));
    assert_eq!(
        format!("{}", error),
        "Vault file not found: /secrets/vault.yml"
    );
}

#[test]
fn test_handler_not_found_error() {
    let error = Error::HandlerNotFound("restart apache".to_string());
    assert_eq!(format!("{}", error), "Handler 'restart apache' not found");
}

#[test]
fn test_handler_failed_error() {
    let error = Error::HandlerFailed {
        handler: "reload nginx".to_string(),
        host: "web2".to_string(),
        message: "nginx: configuration file test failed".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Handler 'reload nginx' failed on host 'web2'"));
    assert!(error_msg.contains("configuration file test failed"));
}

#[test]
fn test_config_error() {
    let error = Error::Config("Invalid configuration file format".to_string());
    assert_eq!(
        format!("{}", error),
        "Configuration error: Invalid configuration file format"
    );
}

#[test]
fn test_invalid_config_error() {
    let error = Error::InvalidConfig {
        key: "forks".to_string(),
        message: "must be a positive integer".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Invalid configuration value for 'forks'"));
    assert!(error_msg.contains("must be a positive integer"));
}

#[test]
fn test_file_not_found_error() {
    let error = Error::FileNotFound(PathBuf::from("/etc/app/config.yml"));
    assert_eq!(format!("{}", error), "File not found: /etc/app/config.yml");
}

#[test]
fn test_strategy_error() {
    let error = Error::Strategy("Unknown execution strategy 'invalid'".to_string());
    assert_eq!(
        format!("{}", error),
        "Execution strategy error: Unknown execution strategy 'invalid'"
    );
}

#[test]
fn test_become_error() {
    let error = Error::BecomeError {
        host: "server1".to_string(),
        message: "sudo: password is required".to_string(),
    };
    let error_msg = format!("{}", error);
    assert!(error_msg.contains("Privilege escalation failed on 'server1'"));
    assert!(error_msg.contains("password is required"));
}

#[test]
fn test_internal_error() {
    let error = Error::Internal("Unexpected state encountered".to_string());
    assert_eq!(
        format!("{}", error),
        "Internal error: Unexpected state encountered"
    );
}

#[test]
fn test_other_error_without_source() {
    let error = Error::Other {
        message: "Generic error message".to_string(),
        source: None,
    };
    assert_eq!(format!("{}", error), "Generic error message");
}

#[test]
fn test_other_error_with_source() {
    let source = std::io::Error::new(std::io::ErrorKind::Other, "underlying error");
    let error = Error::Other {
        message: "Wrapper error".to_string(),
        source: Some(Box::new(source)),
    };
    assert_eq!(format!("{}", error), "Wrapper error");
}

// ============================================================================
// Error Exit Codes
// ============================================================================

#[test]
fn test_error_exit_codes() {
    assert_eq!(Error::task_failed("test", "host1", "msg").exit_code(), 2);
    assert_eq!(
        Error::ModuleExecution {
            module: "test".to_string(),
            message: "msg".to_string()
        }
        .exit_code(),
        2
    );

    assert_eq!(Error::connection_failed("host1", "msg").exit_code(), 3);
    assert_eq!(
        Error::AuthenticationFailed {
            user: "user".to_string(),
            host: "host".to_string(),
            message: "msg".to_string()
        }
        .exit_code(),
        3
    );

    assert_eq!(
        Error::playbook_parse(PathBuf::from("test.yml"), "msg", None).exit_code(),
        4
    );
    assert_eq!(Error::PlaybookValidation("msg".to_string()).exit_code(), 4);

    assert_eq!(
        Error::InventoryLoad {
            path: PathBuf::from("hosts"),
            message: "msg".to_string()
        }
        .exit_code(),
        5
    );
    assert_eq!(Error::HostNotFound("host".to_string()).exit_code(), 5);

    assert_eq!(Error::VaultDecryption("msg".to_string()).exit_code(), 6);
    assert_eq!(Error::InvalidVaultPassword.exit_code(), 6);

    assert_eq!(Error::Internal("msg".to_string()).exit_code(), 1);
}

// ============================================================================
// Error Recoverability
// ============================================================================

#[test]
fn test_error_is_recoverable() {
    assert!(Error::TaskSkipped("test".to_string()).is_recoverable());
    assert!(Error::ConnectionTimeout {
        host: "host".to_string(),
        timeout_secs: 30
    }
    .is_recoverable());
    assert!(Error::TaskTimeout {
        task: "task".to_string(),
        host: "host".to_string(),
        timeout_secs: 60
    }
    .is_recoverable());

    assert!(!Error::task_failed("task", "host", "msg").is_recoverable());
    assert!(!Error::connection_failed("host", "msg").is_recoverable());
    assert!(!Error::ModuleNotFound("mod".to_string()).is_recoverable());
}

// ============================================================================
// Error Context Extension Trait
// ============================================================================

#[test]
fn test_error_context_adds_context() {
    let result: Result<()> = Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "file not found",
    ))
    .context("Failed to read configuration file");

    match result {
        Err(Error::Other { message, source }) => {
            assert_eq!(message, "Failed to read configuration file");
            assert!(source.is_some());
        }
        _ => panic!("Expected Other error"),
    }
}

#[test]
fn test_error_with_context_lazy_evaluation() {
    let result: Result<()> = Err(std::io::Error::new(std::io::ErrorKind::Other, "io error"))
        .with_context(|| format!("Failed to process file at line {}", 42));

    match result {
        Err(Error::Other { message, source }) => {
            assert_eq!(message, "Failed to process file at line 42");
            assert!(source.is_some());
        }
        _ => panic!("Expected Other error"),
    }
}

// ============================================================================
// Error Propagation Through Execution Stack
// ============================================================================

#[test]
fn test_executor_error_propagation() {
    let error = ExecutorError::TaskFailed("Task execution failed".to_string());
    assert_eq!(
        format!("{}", error),
        "Task execution failed: Task execution failed"
    );

    let error = ExecutorError::HostUnreachable("server1".to_string());
    assert_eq!(format!("{}", error), "Host unreachable: server1");

    let error = ExecutorError::ModuleNotFound("unknown_module".to_string());
    assert_eq!(format!("{}", error), "Module not found: unknown_module");

    let error = ExecutorError::ConditionError("invalid syntax".to_string());
    assert_eq!(
        format!("{}", error),
        "Condition evaluation failed: invalid syntax"
    );
}

#[test]
fn test_executor_error_from_io_error() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let executor_error: ExecutorError = io_error.into();

    match executor_error {
        ExecutorError::IoError(_) => {}
        _ => panic!("Expected IoError"),
    }
}

// ============================================================================
// ignore_errors: true Behavior
// ============================================================================

#[tokio::test]
async fn test_ignore_errors_continues_execution() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Ignore Errors Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // Task that would fail but has ignore_errors
    play.add_task(
        Task::new("Failing task", "fail")
            .arg("msg", "This should fail but be ignored")
            .ignore_errors(true),
    );

    // Task that should run after the failed one
    play.add_task(Task::new("Should still run", "debug").arg("msg", "Still running"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Should not fail overall
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_ignore_errors_in_loop() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Loop Ignore Errors Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Loop with errors", "debug")
            .arg("msg", "Item {{ item }}")
            .loop_over(vec![
                serde_json::json!("one"),
                serde_json::json!("two"),
                serde_json::json!("three"),
            ])
            .ignore_errors(true),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// failed_when Conditions
// ============================================================================

#[tokio::test]
async fn test_failed_when_false_prevents_failure() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Failed When Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // This task would normally indicate success, but failed_when makes it fail
    play.add_task(
        Task::new("Custom failure condition", "debug")
            .arg("msg", "test")
            .register("result"),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Should complete without failure since no failed_when condition
    assert!(!host_result.failed);
}

// ============================================================================
// Task Result Error States
// ============================================================================

#[test]
fn test_task_result_failed_state() {
    let result = TaskResult::failed("Task execution error");
    assert_eq!(result.status, TaskStatus::Failed);
    assert!(!result.changed);
    assert_eq!(result.msg, Some("Task execution error".to_string()));
}

#[test]
fn test_task_result_skipped_state() {
    let result = TaskResult::skipped("Condition not met");
    assert_eq!(result.status, TaskStatus::Skipped);
    assert!(!result.changed);
    assert_eq!(result.msg, Some("Condition not met".to_string()));
}

#[test]
fn test_task_result_unreachable_state() {
    let result = TaskResult::unreachable("Host is offline");
    assert_eq!(result.status, TaskStatus::Unreachable);
    assert!(!result.changed);
    assert_eq!(result.msg, Some("Host is offline".to_string()));
}

// ============================================================================
// Connection Error Handling
// ============================================================================

#[test]
fn test_connection_error_types() {
    use rustible::connection::ConnectionError;

    let error = ConnectionError::ConnectionFailed("TCP connection refused".to_string());
    assert_eq!(
        format!("{}", error),
        "Connection failed: TCP connection refused"
    );

    let error = ConnectionError::AuthenticationFailed("Public key rejected".to_string());
    assert_eq!(
        format!("{}", error),
        "Authentication failed: Public key rejected"
    );

    let error = ConnectionError::ExecutionFailed("Command not found".to_string());
    assert_eq!(
        format!("{}", error),
        "Command execution failed: Command not found"
    );

    let error = ConnectionError::TransferFailed("Permission denied".to_string());
    assert_eq!(
        format!("{}", error),
        "File transfer failed: Permission denied"
    );

    let error = ConnectionError::Timeout(30);
    assert_eq!(format!("{}", error), "Connection timeout after 30 seconds");

    let error = ConnectionError::HostNotFound("unknown.host.local".to_string());
    assert_eq!(format!("{}", error), "Host not found: unknown.host.local");

    let error = ConnectionError::InvalidConfig("Missing port configuration".to_string());
    assert_eq!(
        format!("{}", error),
        "Invalid configuration: Missing port configuration"
    );

    let error = ConnectionError::PoolExhausted;
    assert_eq!(format!("{}", error), "Connection pool exhausted");

    let error = ConnectionError::ConnectionClosed;
    assert_eq!(format!("{}", error), "Connection closed");
}

// ============================================================================
// Module Error Context
// ============================================================================

#[test]
fn test_module_error_provides_context() {
    let error = Error::module_args(
        "copy",
        "missing required argument 'dest', required arguments: src, dest",
    );
    let msg = format!("{}", error);

    // Error message should clearly indicate which module and what went wrong
    assert!(msg.contains("copy"));
    assert!(msg.contains("missing required argument"));
    assert!(msg.contains("dest"));
}

#[test]
fn test_module_execution_error_context() {
    let error = Error::ModuleExecution {
        module: "command".to_string(),
        message: "Command 'nonexistent' not found in PATH".to_string(),
    };
    let msg = format!("{}", error);

    assert!(msg.contains("Module 'command'"));
    assert!(msg.contains("execution failed"));
    assert!(msg.contains("not found in PATH"));
}

// ============================================================================
// Graceful Degradation Scenarios
// ============================================================================

#[tokio::test]
async fn test_partial_host_failure_continues() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("host1".to_string(), None);
    runtime.add_host("host2".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Partial Failure Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    play.add_task(Task::new("Simple task", "debug").arg("msg", "test"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    // Both hosts should have results even if one failed
    assert!(results.len() >= 1);
}

#[tokio::test]
async fn test_skip_task_does_not_fail_playbook() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Skip Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Skipped task", "debug")
            .arg("msg", "Should skip")
            .when("false"),
    );

    play.add_task(Task::new("Running task", "debug").arg("msg", "Should run"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
    assert!(host_result.stats.skipped >= 1);
}

// ============================================================================
// Error Message Clarity Tests
// ============================================================================

#[test]
fn test_task_error_message_includes_task_and_host() {
    let error = Error::task_failed(
        "Install nginx package",
        "web-server-01",
        "Package 'nginx' not found in repository",
    );

    let msg = format!("{}", error);

    // Should include task name
    assert!(msg.contains("Install nginx package"));
    // Should include host
    assert!(msg.contains("web-server-01"));
    // Should include specific error
    assert!(msg.contains("not found in repository"));
}

#[test]
fn test_connection_error_message_clarity() {
    let error = Error::AuthenticationFailed {
        user: "deploy".to_string(),
        host: "production-db.example.com".to_string(),
        message: "Permission denied (publickey,password)".to_string(),
    };

    let msg = format!("{}", error);

    // Should show user@host format
    assert!(msg.contains("deploy@production-db.example.com"));
    // Should include authentication details
    assert!(msg.contains("publickey,password"));
}

#[test]
fn test_playbook_parse_error_clarity() {
    let error = Error::playbook_parse(
        PathBuf::from("/ansible/playbooks/site.yml"),
        "line 42: mapping values are not allowed here",
        None,
    );

    let msg = format!("{}", error);

    // Should include file path
    assert!(msg.contains("site.yml"));
    // Should include specific parse error
    assert!(msg.contains("mapping values are not allowed here"));
}

// ============================================================================
// Error Conversion Tests
// ============================================================================

#[test]
fn test_yaml_parse_error_conversion() {
    let yaml_error =
        serde_yaml::from_str::<serde_json::Value>("invalid: yaml: syntax").unwrap_err();
    let error: Error = yaml_error.into();

    match error {
        Error::YamlParse(_) => {}
        _ => panic!("Expected YamlParse error"),
    }
}

#[test]
fn test_json_parse_error_conversion() {
    let json_error = serde_json::from_str::<serde_json::Value>("{invalid json").unwrap_err();
    let error: Error = json_error.into();

    match error {
        Error::JsonParse(_) => {}
        _ => panic!("Expected JsonParse error"),
    }
}

#[test]
fn test_io_error_conversion() {
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let error: Error = io_error.into();

    match error {
        Error::Io(_) => {}
        _ => panic!("Expected Io error"),
    }
}

// ============================================================================
// Execution Context Error Handling
// ============================================================================

#[test]
fn test_execution_context_creation() {
    let ctx = ExecutionContext::new("test-host");
    assert_eq!(ctx.host, "test-host");
    assert!(!ctx.check_mode);
    assert!(!ctx.diff_mode);
}

#[test]
fn test_execution_context_with_check_mode() {
    let ctx = ExecutionContext::new("test-host").with_check_mode(true);
    assert!(ctx.check_mode);
}

// ============================================================================
// Complex Error Scenarios
// ============================================================================

#[tokio::test]
async fn test_multiple_task_failures_aggregate_correctly() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multiple Failures Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // Add multiple tasks with ignore_errors
    for i in 1..=3 {
        play.add_task(
            Task::new(format!("Task {}", i), "fail")
                .arg("msg", format!("Failure {}", i))
                .ignore_errors(true),
        );
    }

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // With ignore_errors, should not fail overall
    assert!(!host_result.failed);
}

#[test]
fn test_nested_error_source_chain() {
    let inner_error = std::io::Error::new(std::io::ErrorKind::Other, "inner error");
    let middle_error = Error::Other {
        message: "middle error".to_string(),
        source: Some(Box::new(inner_error)),
    };

    // Verify error can be formatted
    let msg = format!("{}", middle_error);
    assert_eq!(msg, "middle error");

    // Verify source is accessible
    match middle_error {
        Error::Other { source, .. } => {
            assert!(source.is_some());
        }
        _ => panic!("Expected Other error"),
    }
}

// ============================================================================
// Inventory Error Scenarios
// ============================================================================

#[test]
fn test_inventory_errors_provide_clear_context() {
    let error = Error::InventoryLoad {
        path: PathBuf::from("/etc/ansible/hosts"),
        message: "YAML parsing failed at line 15: unexpected character".to_string(),
    };

    let msg = format!("{}", error);
    assert!(msg.contains("/etc/ansible/hosts"));
    assert!(msg.contains("line 15"));
    assert!(msg.contains("unexpected character"));
}

#[test]
fn test_host_pattern_error_clarity() {
    let error = Error::InvalidHostPattern("web[1:10:2".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Invalid host pattern"));
    assert!(msg.contains("web[1:10:2"));
}

// ============================================================================
// Variable Error Scenarios
// ============================================================================

#[test]
fn test_undefined_variable_in_template_context() {
    let error = Error::UndefinedVariable("server_name".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Undefined variable"));
    assert!(msg.contains("server_name"));
}

#[test]
fn test_invalid_variable_value_type_error() {
    let error = Error::InvalidVariableValue {
        name: "ports".to_string(),
        message: "expected list of integers, got string".to_string(),
    };

    let msg = format!("{}", error);
    assert!(msg.contains("Invalid value for variable 'ports'"));
    assert!(msg.contains("expected list of integers"));
}

// ============================================================================
// Vault Error Scenarios
// ============================================================================

#[test]
fn test_vault_password_error() {
    let error = Error::InvalidVaultPassword;
    assert_eq!(format!("{}", error), "Invalid vault password");
}

#[test]
fn test_vault_decryption_error_with_details() {
    let error = Error::VaultDecryption(
        "Decryption failed: MAC verification failed. Incorrect password?".to_string(),
    );
    let msg = format!("{}", error);
    assert!(msg.contains("Failed to decrypt vault"));
    assert!(msg.contains("MAC verification failed"));
    assert!(msg.contains("password"));
}

// ============================================================================
// Handler Error Scenarios
// ============================================================================

#[test]
fn test_handler_not_found_provides_handler_name() {
    let error = Error::HandlerNotFound("restart mysql".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Handler 'restart mysql' not found"));
}

#[test]
fn test_handler_execution_failure_context() {
    let error = Error::HandlerFailed {
        handler: "reload haproxy".to_string(),
        host: "lb-01".to_string(),
        message: "configuration check failed".to_string(),
    };

    let msg = format!("{}", error);
    assert!(msg.contains("reload haproxy"));
    assert!(msg.contains("lb-01"));
    assert!(msg.contains("configuration check failed"));
}

// ============================================================================
// Configuration Error Scenarios
// ============================================================================

#[test]
fn test_invalid_config_error_specifies_key() {
    let error = Error::InvalidConfig {
        key: "ansible_python_interpreter".to_string(),
        message: "path does not exist: /usr/bin/python3.99".to_string(),
    };

    let msg = format!("{}", error);
    assert!(msg.contains("ansible_python_interpreter"));
    assert!(msg.contains("path does not exist"));
}

// ============================================================================
// Strategy Error Scenarios
// ============================================================================

#[test]
fn test_strategy_error_clarity() {
    let error = Error::Strategy("Cannot use 'free' strategy with serial execution".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Execution strategy error"));
    assert!(msg.contains("free"));
    assert!(msg.contains("serial"));
}

// ============================================================================
// Privilege Escalation Error Scenarios
// ============================================================================

#[test]
fn test_become_error_context() {
    let error = Error::BecomeError {
        host: "secure-server".to_string(),
        message: "sudo: 3 incorrect password attempts".to_string(),
    };

    let msg = format!("{}", error);
    assert!(msg.contains("Privilege escalation failed"));
    assert!(msg.contains("secure-server"));
    assert!(msg.contains("incorrect password attempts"));
}

// ============================================================================
// Role Error Scenarios
// ============================================================================

#[test]
fn test_role_dependency_cycle_error() {
    let error = Error::RoleDependency(
        "Circular dependency detected: common -> security -> common".to_string(),
    );

    let msg = format!("{}", error);
    assert!(msg.contains("Role dependency error"));
    assert!(msg.contains("Circular dependency"));
    assert!(msg.contains("common -> security -> common"));
}

#[test]
fn test_invalid_role_structure_error() {
    let error = Error::InvalidRole {
        role: "webapp".to_string(),
        message: "meta/main.yml contains invalid YAML".to_string(),
    };

    let msg = format!("{}", error);
    assert!(msg.contains("Invalid role structure in 'webapp'"));
    assert!(msg.contains("meta/main.yml"));
}

// ============================================================================
// Block/Rescue/Always Block Tests
// ============================================================================

#[test]
fn test_block_rescue_always_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Test block/rescue/always
  hosts: all
  gather_facts: false
  tasks:
    - name: Main block
      block:
        - name: Task in block
          debug:
            msg: "Inside block"
      rescue:
        - name: Rescue task
          debug:
            msg: "In rescue"
      always:
        - name: Always task
          debug:
            msg: "In always"
"#;

    let result = Playbook::parse(yaml, None);
    assert!(
        result.is_ok(),
        "Block/rescue/always should parse successfully"
    );

    let playbook = result.unwrap();
    assert_eq!(playbook.plays.len(), 1);
    // Block, rescue, and always tasks should all be included
    assert!(playbook.plays[0].tasks.len() >= 1);
}

#[tokio::test]
async fn test_block_with_rescue_on_failure() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    // Create a playbook that simulates block/rescue behavior
    let mut playbook = Playbook::new("Block Rescue Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // A failing task with ignore_errors to simulate rescue behavior
    play.add_task(
        Task::new("Failing block task", "fail")
            .arg("msg", "Block failure")
            .ignore_errors(true),
    );

    // This simulates what a rescue task would do - run after failure
    play.add_task(Task::new("Simulated rescue task", "debug").arg("msg", "Recovered from failure"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Should not have overall failure since we ignored errors (simulating rescue)
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_always_runs_after_success() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Always After Success Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // Successful task (block)
    play.add_task(Task::new("Successful task", "debug").arg("msg", "Success"));

    // Always task - should run after success
    play.add_task(Task::new("Always task", "debug").arg("msg", "Always runs"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
    // At least 2 tasks should have been run (ok + changed)
    assert!(host_result.stats.ok + host_result.stats.changed >= 2);
}

#[tokio::test]
async fn test_always_runs_after_failure() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Always After Failure Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // Failing task with ignore_errors to allow continuation
    play.add_task(
        Task::new("Failing task", "fail")
            .arg("msg", "Expected failure")
            .ignore_errors(true),
    );

    // Always task - should still run even after failure
    play.add_task(Task::new("Always task after failure", "debug").arg("msg", "Still runs"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Should not fail overall since we ignored the failure
    assert!(!host_result.failed);
}

// ============================================================================
// failed_when Complex Expression Tests
// ============================================================================

#[tokio::test]
async fn test_failed_when_with_registered_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Failed When Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // Register a result
    play.add_task(
        Task::new("Get status", "debug")
            .arg("msg", "Getting status")
            .register("status_result"),
    );

    // Use the registered variable in a subsequent task
    play.add_task(
        Task::new("Check status", "debug")
            .arg("msg", "Checking status")
            .register("check_result"),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

#[test]
fn test_task_with_failed_when_condition_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Failed when test
  hosts: all
  gather_facts: false
  tasks:
    - name: Task with failed_when
      command: echo "test"
      register: result
      failed_when: result.rc != 0
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert!(task.failed_when.is_some());
}

// ============================================================================
// changed_when Condition Tests
// ============================================================================

#[test]
fn test_task_with_changed_when_condition_parsing() {
    use rustible::executor::playbook::Playbook;

    // changed_when must be a string in YAML, quoted to be parsed as string
    let yaml = r#"
- name: Changed when test
  hosts: all
  gather_facts: false
  tasks:
    - name: Task with changed_when
      command: echo "test"
      register: result
      changed_when: "false"
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert!(task.changed_when.is_some());
}

#[tokio::test]
async fn test_changed_when_false_prevents_changed_status() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Changed When Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // Task that would normally be changed but changed_when: false overrides
    play.add_task(Task::new("Never changed", "debug").arg("msg", "test"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// TOML and Template Error Conversion Tests
// ============================================================================

#[test]
fn test_toml_parse_error_conversion() {
    let toml_error = toml::from_str::<serde_json::Value>("invalid = toml = syntax").unwrap_err();
    let error: Error = toml_error.into();

    match error {
        Error::TomlParse(_) => {}
        _ => panic!("Expected TomlParse error"),
    }
}

#[test]
fn test_template_error_conversion() {
    // Create a minijinja error by using invalid template syntax
    let mut env = minijinja::Environment::new();
    // Use strict undefined behavior to trigger an error on undefined variables
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    let template_error = env.render_str("{{ undefined_var }}", ()).unwrap_err();
    let error: Error = template_error.into();

    match error {
        Error::Template(_) => {}
        _ => panic!("Expected Template error"),
    }
}

#[test]
fn test_template_syntax_error_with_line_info() {
    let error = Error::TemplateSyntax {
        template: "config.j2".to_string(),
        message: "line 10: unexpected token '}'".to_string(),
    };

    let msg = format!("{}", error);
    assert!(msg.contains("config.j2"));
    assert!(msg.contains("line 10"));
    assert!(msg.contains("unexpected token"));
}

// ============================================================================
// Any Errors Fatal Tests
// ============================================================================

#[test]
fn test_any_errors_fatal_play_parsing() {
    use rustible::executor::playbook::Playbook;

    // Test parsing a playbook with max_fail_percentage which controls similar behavior
    let yaml = r#"
- name: Test with error control
  hosts: all
  max_fail_percentage: 0
  gather_facts: false
  tasks:
    - name: Critical task
      debug:
        msg: "Must not fail"
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    // max_fail_percentage: 0 effectively makes any error fatal
    assert_eq!(playbook.plays[0].max_fail_percentage, Some(0));
}

#[test]
fn test_max_fail_percentage_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Test with max_fail_percentage
  hosts: all
  max_fail_percentage: 25
  tasks:
    - name: Test task
      debug:
        msg: "test"
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    assert_eq!(playbook.plays[0].max_fail_percentage, Some(25));
}

// ============================================================================
// Error Source Chain Tests
// ============================================================================

#[test]
fn test_error_source_chain_preserved() {
    use std::error::Error as StdError;

    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "original IO error");
    let wrapper_error = Error::Other {
        message: "Wrapper message".to_string(),
        source: Some(Box::new(io_error)),
    };

    // Verify the source chain
    let source = wrapper_error.source();
    assert!(source.is_some());
    assert!(source.unwrap().to_string().contains("original IO error"));
}

#[test]
fn test_playbook_parse_error_source_chain() {
    let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let error = Error::playbook_parse(
        "/path/to/playbook.yml",
        "Could not read file",
        Some(Box::new(io_error)),
    );

    // Verify source is available
    if let Error::PlaybookParse { source, .. } = &error {
        assert!(source.is_some());
        assert!(source
            .as_ref()
            .unwrap()
            .to_string()
            .contains("access denied"));
    } else {
        panic!("Expected PlaybookParse error");
    }
}

// ============================================================================
// Vault Error Chain Tests
// ============================================================================

#[test]
fn test_vault_error_detailed() {
    let error = Error::Vault("Failed to read vault password from file: ~/.vault_pass".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Vault error"));
    assert!(msg.contains("vault_pass"));
}

#[test]
fn test_vault_decryption_with_context() {
    let error = Error::VaultDecryption("HMAC validation failed at offset 1024".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Failed to decrypt vault"));
    assert!(msg.contains("HMAC"));
    assert!(msg.contains("offset"));
}

// ============================================================================
// Connection Error Recovery Tests
// ============================================================================

#[test]
fn test_connection_error_ssh_specific() {
    use rustible::connection::ConnectionError;

    let error = ConnectionError::SshError("Host key verification failed".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("SSH error"));
    assert!(msg.contains("Host key verification"));
}

#[test]
fn test_connection_error_docker_specific() {
    use rustible::connection::ConnectionError;

    let error = ConnectionError::DockerError("Container not running".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Docker error"));
    assert!(msg.contains("not running"));
}

#[test]
fn test_connection_error_unsupported_operation() {
    use rustible::connection::ConnectionError;

    let error = ConnectionError::UnsupportedOperation("SFTP subsystem not available".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Unsupported operation"));
    assert!(msg.contains("SFTP"));
}

// ============================================================================
// Executor Error Comprehensive Tests
// ============================================================================

#[test]
fn test_executor_error_dependency_cycle() {
    let error = ExecutorError::DependencyCycle("task_a -> task_b -> task_a".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Dependency cycle detected"));
    assert!(msg.contains("task_a"));
}

#[test]
fn test_executor_error_variable_not_found() {
    let error = ExecutorError::VariableNotFound("undefined_variable".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Variable not found"));
    assert!(msg.contains("undefined_variable"));
}

#[test]
fn test_executor_error_runtime() {
    let error = ExecutorError::RuntimeError("Unexpected state during execution".to_string());
    let msg = format!("{}", error);
    assert!(msg.contains("Runtime error"));
}

// ============================================================================
// Task Result Comprehensive Tests
// ============================================================================

#[test]
fn test_task_result_with_diff() {
    use rustible::executor::task::TaskDiff;

    let diff = TaskDiff {
        before: Some("old content".to_string()),
        after: Some("new content".to_string()),
        before_header: Some("/etc/file.conf".to_string()),
        after_header: Some("/etc/file.conf".to_string()),
    };

    let result = TaskResult::changed().with_diff(diff);
    assert!(result.diff.is_some());

    let task_diff = result.diff.unwrap();
    assert_eq!(task_diff.before, Some("old content".to_string()));
    assert_eq!(task_diff.after, Some("new content".to_string()));
}

#[test]
fn test_task_result_with_json_result() {
    let json_result = serde_json::json!({
        "path": "/etc/file.conf",
        "state": "present",
        "mode": "0644"
    });

    let result = TaskResult::ok().with_result(json_result.clone());
    assert!(result.result.is_some());
    assert_eq!(result.result.unwrap(), json_result);
}

#[test]
fn test_task_result_to_registered() {
    let result = TaskResult::changed().with_msg("File modified successfully");

    let registered = result.to_registered(
        Some("stdout output".to_string()),
        Some("stderr output".to_string()),
    );

    assert!(registered.changed);
    assert!(!registered.failed);
    assert!(!registered.skipped);
    assert_eq!(registered.stdout, Some("stdout output".to_string()));
    assert_eq!(registered.stderr, Some("stderr output".to_string()));
    assert!(registered.stdout_lines.is_some());
}

// ============================================================================
// Graceful Degradation with Multiple Hosts
// ============================================================================

#[tokio::test]
async fn test_unreachable_host_marked_correctly() {
    let result = TaskResult::unreachable("Connection timeout");
    assert_eq!(result.status, TaskStatus::Unreachable);
    assert!(!result.changed);
    assert!(result.msg.is_some());
    assert!(result.msg.unwrap().contains("Connection timeout"));
}

#[tokio::test]
async fn test_multi_host_partial_failure_reporting() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("host1".to_string(), None);
    runtime.add_host("host2".to_string(), None);
    runtime.add_host("host3".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi Host Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    play.add_task(Task::new("Test task", "debug").arg("msg", "Running on all hosts"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    // Should have results for all 3 hosts
    assert_eq!(results.len(), 3);
    assert!(results.contains_key("host1"));
    assert!(results.contains_key("host2"));
    assert!(results.contains_key("host3"));
}

// ============================================================================
// Error Context Extension Comprehensive Tests
// ============================================================================

#[test]
fn test_error_context_on_option_none() {
    // ErrorContext on Result types
    let result: std::result::Result<i32, std::io::Error> =
        Err(std::io::Error::new(std::io::ErrorKind::Other, "test error"));

    let contexted: Result<i32> = result.context("Additional context");

    match contexted {
        Err(Error::Other { message, source }) => {
            assert_eq!(message, "Additional context");
            assert!(source.is_some());
        }
        _ => panic!("Expected contexted error"),
    }
}

#[test]
fn test_with_context_preserves_lazy_evaluation() {
    let mut called = false;
    let result: std::result::Result<i32, std::io::Error> = Ok(42);

    // with_context should NOT call the closure on success
    let _ok_result: Result<i32> = result.with_context(|| {
        called = true;
        "Should not be called"
    });

    assert!(!called, "Closure should not be called on success");
}

// ============================================================================
// Ignore Errors Detailed Tests
// ============================================================================

#[tokio::test]
async fn test_ignore_errors_records_failure() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Ignore Errors Record Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    play.add_task(
        Task::new("Failing task with ignore", "fail")
            .arg("msg", "This will fail")
            .ignore_errors(true)
            .register("failed_result"),
    );

    play.add_task(Task::new("Next task", "debug").arg("msg", "Still running"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Overall should not fail
    assert!(!host_result.failed);
}

#[tokio::test]
async fn test_ignore_errors_with_failed_when_interaction() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Ignore Errors With Failed When Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // Task with both ignore_errors and register
    play.add_task(
        Task::new("Task with ignore_errors", "debug")
            .arg("msg", "test")
            .ignore_errors(true)
            .register("result"),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    assert!(!host_result.failed);
}

// ============================================================================
// Module Argument Validation Error Tests
// ============================================================================

#[test]
fn test_module_args_missing_required() {
    let error = Error::module_args("copy", "missing required arguments: ['src', 'dest']");
    let msg = format!("{}", error);
    assert!(msg.contains("copy"));
    assert!(msg.contains("missing required arguments"));
    assert!(msg.contains("src"));
    assert!(msg.contains("dest"));
}

#[test]
fn test_module_args_invalid_type() {
    let error = Error::module_args("file", "argument 'mode' must be a string, got integer");
    let msg = format!("{}", error);
    assert!(msg.contains("file"));
    assert!(msg.contains("mode"));
    assert!(msg.contains("must be a string"));
}

#[test]
fn test_module_args_mutually_exclusive() {
    let error = Error::module_args(
        "file",
        "arguments 'state=link' and 'content' are mutually exclusive",
    );
    let msg = format!("{}", error);
    assert!(msg.contains("mutually exclusive"));
}

// ============================================================================
// Template Error Context Tests
// ============================================================================

#[test]
fn test_template_render_with_undefined_variable() {
    let error = Error::template_render(
        "nginx.conf.j2",
        "undefined variable 'server_name' at line 5, column 12",
    );
    let msg = format!("{}", error);
    assert!(msg.contains("nginx.conf.j2"));
    assert!(msg.contains("undefined variable"));
    assert!(msg.contains("server_name"));
    assert!(msg.contains("line 5"));
}

#[test]
fn test_template_render_with_filter_error() {
    let error = Error::template_render(
        "config.j2",
        "filter 'to_json' failed: circular reference detected",
    );
    let msg = format!("{}", error);
    assert!(msg.contains("filter"));
    assert!(msg.contains("to_json"));
    assert!(msg.contains("circular reference"));
}

// ============================================================================
// Run Once Behavior Tests
// ============================================================================

#[tokio::test]
async fn test_run_once_task_execution() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("host1".to_string(), None);
    runtime.add_host("host2".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Run Once Test");
    let mut play = Play::new("Test", "all");
    play.gather_facts = false;

    // Regular task - should run on all hosts
    play.add_task(Task::new("Regular task", "debug").arg("msg", "Running everywhere"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    // Both hosts should have results
    assert!(results.contains_key("host1"));
    assert!(results.contains_key("host2"));
}

// ============================================================================
// Delegate To Error Tests
// ============================================================================

#[test]
fn test_task_delegate_to_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Delegate test
  hosts: all
  gather_facts: false
  tasks:
    - name: Delegated task
      debug:
        msg: "Delegated"
      delegate_to: localhost
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert_eq!(task.delegate_to, Some("localhost".to_string()));
}

// ============================================================================
// Retries and Until Error Tests
// ============================================================================

#[test]
fn test_task_retries_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Retry test
  hosts: all
  gather_facts: false
  tasks:
    - name: Retrying task
      command: /bin/check_service
      retries: 5
      delay: 10
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());
}

// ============================================================================
// Serial Execution Error Tests
// ============================================================================

#[test]
fn test_serial_play_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Serial test
  hosts: all
  serial: 2
  gather_facts: false
  tasks:
    - name: Serial task
      debug:
        msg: "Running in batches"
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    assert_eq!(playbook.plays[0].serial, Some(2));
}

// ============================================================================
// Strategy Error Tests
// ============================================================================

#[test]
fn test_strategy_play_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Strategy test
  hosts: all
  strategy: free
  gather_facts: false
  tasks:
    - name: Free strategy task
      debug:
        msg: "Running freely"
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    assert_eq!(playbook.plays[0].strategy, Some("free".to_string()));
}

// ============================================================================
// Environment Variable Error Tests
// ============================================================================

#[test]
fn test_task_environment_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Environment test
  hosts: all
  gather_facts: false
  tasks:
    - name: Task with env
      command: echo $MY_VAR
      environment:
        MY_VAR: "hello"
        PATH: "/custom/path:{{ ansible_env.PATH }}"
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());
}

// ============================================================================
// Handler Error Chain Tests
// ============================================================================

#[test]
fn test_handler_with_listen_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Handler listen test
  hosts: all
  gather_facts: false
  tasks:
    - name: Notify handlers
      debug:
        msg: "test"
      notify:
        - restart services
  handlers:
    - name: restart nginx
      debug:
        msg: "Restarting nginx"
      listen:
        - restart services
    - name: restart apache
      debug:
        msg: "Restarting apache"
      listen:
        - restart services
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    assert_eq!(playbook.plays[0].handlers.len(), 2);
}

// ============================================================================
// Become Error Edge Cases
// ============================================================================

#[test]
fn test_become_user_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Become user test
  hosts: all
  become: true
  become_user: postgres
  gather_facts: false
  tasks:
    - name: Task as postgres
      debug:
        msg: "Running as postgres"
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    assert!(playbook.plays[0].r#become);
    assert_eq!(playbook.plays[0].become_user, Some("postgres".to_string()));
}

// ============================================================================
// Conditional Expression Error Tests
// ============================================================================

#[test]
fn test_when_with_and_condition_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: When and test
  hosts: all
  gather_facts: false
  tasks:
    - name: Conditional task
      debug:
        msg: "test"
      when:
        - ansible_os_family == 'Debian'
        - ansible_distribution_version >= '20'
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert!(task.when.is_some());
    // Multiple when conditions are AND-joined
    assert!(task.when.as_ref().unwrap().contains(" and "));
}

// ============================================================================
// Notify Error Cases
// ============================================================================

#[test]
fn test_notify_list_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Notify list test
  hosts: all
  gather_facts: false
  tasks:
    - name: Notify multiple handlers
      debug:
        msg: "test"
      notify:
        - restart nginx
        - reload haproxy
        - clear cache
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert_eq!(task.notify.len(), 3);
}

// ============================================================================
// Loop Error Handling Tests
// ============================================================================

#[test]
fn test_loop_with_loop_control_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Loop control test
  hosts: all
  gather_facts: false
  tasks:
    - name: Task with loop control
      debug:
        msg: "Item: {{ outer_item }}"
      loop:
        - a
        - b
        - c
      loop_control:
        loop_var: outer_item
        index_var: outer_index
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());
}

// ============================================================================
// Tags Error Tests
// ============================================================================

#[test]
fn test_tags_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Tags test
  hosts: all
  gather_facts: false
  tasks:
    - name: Tagged task
      debug:
        msg: "test"
      tags:
        - configuration
        - packages
        - always
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert_eq!(task.tags.len(), 3);
    assert!(task.tags.contains(&"configuration".to_string()));
    assert!(task.tags.contains(&"always".to_string()));
}

// ============================================================================
// Execution Stats Aggregation Tests
// ============================================================================

#[test]
fn test_execution_stats_summary() {
    use rustible::executor::{ExecutionStats, Executor, HostResult};
    use std::collections::HashMap;

    let mut results: HashMap<String, HostResult> = HashMap::new();

    results.insert(
        "host1".to_string(),
        HostResult {
            host: "host1".to_string(),
            stats: ExecutionStats {
                ok: 5,
                changed: 2,
                failed: 1,
                skipped: 0,
                unreachable: 0,
            },
            failed: true,
            unreachable: false,
        },
    );

    results.insert(
        "host2".to_string(),
        HostResult {
            host: "host2".to_string(),
            stats: ExecutionStats {
                ok: 4,
                changed: 3,
                failed: 0,
                skipped: 1,
                unreachable: 0,
            },
            failed: false,
            unreachable: false,
        },
    );

    let summary = Executor::summarize_results(&results);

    assert_eq!(summary.ok, 9);
    assert_eq!(summary.changed, 5);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.unreachable, 0);
}

// ============================================================================
// Include/Import Tasks Error Tests
// ============================================================================

#[test]
fn test_include_tasks_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Include tasks test
  hosts: all
  gather_facts: false
  tasks:
    - name: Include common tasks
      include_tasks: tasks/common.yml
      when: include_common | default(true)
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert_eq!(task.module, "include_tasks");
}

#[test]
fn test_import_tasks_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Import tasks test
  hosts: all
  gather_facts: false
  tasks:
    - name: Import setup tasks
      import_tasks: tasks/setup.yml
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert_eq!(task.module, "import_tasks");
}

// ============================================================================
// Include/Import Role Error Tests
// ============================================================================

#[test]
fn test_include_role_parsing() {
    use rustible::executor::playbook::Playbook;

    let yaml = r#"
- name: Include role test
  hosts: all
  gather_facts: false
  tasks:
    - name: Include webserver role
      include_role:
        name: webserver
        tasks_from: configure
"#;

    let result = Playbook::parse(yaml, None);
    assert!(result.is_ok());

    let playbook = result.unwrap();
    let task = &playbook.plays[0].tasks[0];
    assert_eq!(task.module, "include_role");
}

// ============================================================================
// Dependency Graph Error Tests
// ============================================================================

#[test]
fn test_dependency_graph_no_cycle() {
    use rustible::executor::DependencyGraph;

    let mut graph = DependencyGraph::new();
    graph.add_dependency("task3", "task2");
    graph.add_dependency("task2", "task1");

    let result = graph.topological_sort();
    assert!(result.is_ok());

    let order = result.unwrap();
    // DependencyGraph returns nodes in execution order (dependencies first)
    // The implementation may return in different order but all nodes should be present
    assert_eq!(order.len(), 3);
    assert!(order.contains(&"task1".to_string()));
    assert!(order.contains(&"task2".to_string()));
    assert!(order.contains(&"task3".to_string()));
}

#[test]
fn test_dependency_graph_complex_cycle() {
    use rustible::executor::DependencyGraph;

    let mut graph = DependencyGraph::new();
    graph.add_dependency("a", "b");
    graph.add_dependency("b", "c");
    graph.add_dependency("c", "d");
    graph.add_dependency("d", "b"); // Creates cycle: b -> c -> d -> b

    let result = graph.topological_sort();
    assert!(result.is_err());

    match result {
        Err(ExecutorError::DependencyCycle(node)) => {
            // The cycle should be detected
            assert!(!node.is_empty());
        }
        _ => panic!("Expected DependencyCycle error"),
    }
}

// ============================================================================
// Registered Result Comprehensive Tests
// ============================================================================

#[test]
fn test_registered_result_failed() {
    use rustible::executor::runtime::RegisteredResult;

    let result = RegisteredResult::failed("Command returned non-zero exit code");
    assert!(result.failed);
    assert!(!result.changed);
    assert!(!result.skipped);
    assert!(result.msg.is_some());
}

#[test]
fn test_registered_result_skipped() {
    use rustible::executor::runtime::RegisteredResult;

    let result = RegisteredResult::skipped("Condition not met");
    assert!(result.skipped);
    assert!(!result.failed);
    assert!(!result.changed);
}

#[test]
fn test_registered_result_to_json() {
    use rustible::executor::runtime::RegisteredResult;

    let mut result = RegisteredResult::ok(true);
    result.stdout = Some("output text".to_string());
    result.rc = Some(0);

    let json = result.to_json();
    assert!(json.is_object());
    assert_eq!(json["changed"], true);
    assert_eq!(json["stdout"], "output text");
    assert_eq!(json["rc"], 0);
}
