# Rustible Test Suite

This directory contains the comprehensive test suite for Rustible, including unit tests,
integration tests, and test infrastructure.

## Table of Contents

- [Running Tests](#running-tests)
- [Test Organization](#test-organization)
- [Test Files Overview](#test-files-overview)
- [Test Fixtures](#test-fixtures)
- [Common Test Utilities](#common-test-utilities)
- [Mock Implementations](#mock-implementations)
- [Adding New Tests](#adding-new-tests)
- [Integration Test Requirements](#integration-test-requirements)
- [Benchmarking](#benchmarking)

## Running Tests

### Basic Commands

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test file
cargo test --test executor_tests

# Run specific test
cargo test test_executor_config_default

# Run tests matching a pattern
cargo test runtime_context

# Run tests with verbose output
cargo test -- --show-output
```

### Test Categories

```bash
# Run only unit tests (lib tests)
cargo test --lib

# Run only integration tests
cargo test --tests

# Run only documentation tests
cargo test --doc

# Run benchmarks
cargo bench
```

### Parallel Execution

```bash
# Run tests in parallel (default)
cargo test

# Run tests sequentially
cargo test -- --test-threads=1

# Run with specific thread count
cargo test -- --test-threads=4
```

### Filtering Tests

```bash
# Run tests containing "executor"
cargo test executor

# Run tests containing "connection" in a specific file
cargo test --test connection_tests connection

# Skip expensive tests
cargo test -- --skip performance

# Run ignored tests
cargo test -- --ignored
```

## Test Organization

The test suite is organized into several categories:

```
tests/
├── common/              # Shared test utilities and mocks
│   └── mod.rs          # MockConnection, MockModule, builders, helpers
├── fixtures/           # Test data files
│   ├── playbooks/      # Sample playbook YAML files
│   ├── inventories/    # Sample inventory YAML files
│   ├── roles/          # Sample role structures
│   ├── templates/      # Sample Jinja2 templates
│   └── files/          # Sample files for copy/file modules
├── executor_tests.rs   # Executor and runtime tests
├── connection_tests.rs # Connection layer tests
├── module_tests.rs     # Module system tests
├── inventory_tests.rs  # Inventory parsing tests
├── template_tests.rs   # Template engine tests
├── vault_tests.rs      # Vault encryption tests
├── parser_tests.rs     # YAML parsing tests
├── handler_tests.rs    # Handler notification tests
├── role_tests.rs       # Role loading tests
├── strategy_tests.rs   # Execution strategy tests
├── facts_tests.rs      # Fact gathering tests
├── config_tests.rs     # Configuration tests
├── cli_tests.rs        # CLI integration tests
├── error_tests.rs      # Error handling tests
├── integration_tests.rs # Full integration tests
├── ansible_compat_tests.rs # Ansible compatibility tests
└── README.md           # This file
```

## Test Files Overview

### executor_tests.rs

Tests for the core execution engine:
- `ExecutorConfig` - configuration options
- `ExecutionStrategy` - linear, free, host-pinned strategies
- `ExecutionStats` - statistics tracking
- `RuntimeContext` - variable scoping, host management
- `Task` - task definition and builder pattern
- `TaskResult` - result states and data
- `Handler` - handler definitions
- `Playbook` - playbook parsing and execution
- `DependencyGraph` - task dependency resolution

### connection_tests.rs

Tests for the connection layer:
- `LocalConnection` - local command execution
- `CommandResult` - command output handling
- `ExecuteOptions` - execution configuration
- `TransferOptions` - file transfer options
- `ConnectionType` - connection type resolution
- `ConnectionConfig` - SSH configuration
- `HostConfig` - per-host settings
- `ConnectionFactory` - connection pooling
- `ConnectionBuilder` - fluent connection setup
- Error handling and edge cases

### module_tests.rs

Tests for the module system:
- `ModuleRegistry` - registration and lookup
- `ModuleOutput` - result factory methods
- `ModuleContext` - execution context
- `ParamExt` trait - parameter extraction
- Built-in modules: command, shell, copy, template, file, package, service, user
- Check mode behavior
- Diff generation
- Error handling

### Other Test Files

| File | Purpose |
|------|---------|
| `inventory_tests.rs` | Inventory parsing, host/group resolution |
| `template_tests.rs` | Jinja2 template rendering |
| `vault_tests.rs` | Encryption/decryption |
| `parser_tests.rs` | YAML playbook parsing |
| `handler_tests.rs` | Handler notification chain |
| `role_tests.rs` | Role structure loading |
| `strategy_tests.rs` | Parallel execution strategies |
| `facts_tests.rs` | System fact gathering |
| `config_tests.rs` | Configuration loading |
| `cli_tests.rs` | Command-line interface |
| `error_tests.rs` | Error types and propagation |
| `integration_tests.rs` | End-to-end workflows |
| `ansible_compat_tests.rs` | Ansible playbook compatibility |

## Test Fixtures

The `fixtures/` directory contains sample data for testing:

### Playbooks (`fixtures/playbooks/`)

| File | Description |
|------|-------------|
| `minimal_playbook.yml` | Simplest valid playbook - one task |
| `complex_playbook.yml` | All major features demonstrated |
| `error_playbook.yml` | Known failures for error testing |
| `performance_playbook.yml` | Many tasks for benchmarking |

### Inventories (`fixtures/inventories/`)

| File | Description |
|------|-------------|
| `simple.yml` | Basic hosts and groups |
| `complex.yml` | Nested groups, variables |
| `localhost_only.yml` | Local testing only |

### Roles (`fixtures/roles/`)

The `webserver` role demonstrates a complete role structure:
- `tasks/main.yml` - Main task list
- `handlers/main.yml` - Handler definitions
- `defaults/main.yml` - Default variables
- `vars/main.yml` - Role variables
- `templates/` - Jinja2 templates
- `files/` - Static files
- `meta/main.yml` - Role metadata

### Templates (`fixtures/templates/`)

- `simple.j2` - Basic variable substitution
- `complex.j2` - Loops, conditionals, filters

### Files (`fixtures/files/`)

- `sample_config.conf` - Configuration file
- `sample_script.sh` - Executable script

## Common Test Utilities

The `common/mod.rs` module provides shared testing utilities.

### MockConnection

A configurable mock for the `Connection` trait:

```rust
use common::MockConnection;

// Create mock
let mock = MockConnection::new("test-host");

// Configure command results
mock.set_command_result(
    "echo hello",
    CommandResult::success("hello".into(), "".into())
);

// Configure failure
mock.set_should_fail(true);

// Fail after N operations
mock.fail_after(5);

// Check execution history
assert_eq!(mock.command_count(), 1);
let commands = mock.get_commands();
```

### MockModule

A configurable mock for the `Module` trait:

```rust
use common::MockModule;

let mock = MockModule::new("test_module")
    .with_result(ModuleOutput::changed("Changed"))
    .with_check_result(ModuleOutput::ok("Would change"))
    .with_required_params(vec!["name", "state"]);

// Execute and verify
let result = mock.execute(&params, &context).unwrap();
assert_eq!(mock.execution_count(), 1);
```

### Fluent Builders

Build test data with a fluent API:

```rust
use common::{PlaybookBuilder, PlayBuilder, TaskBuilder, InventoryBuilder};

// Build a playbook
let playbook = PlaybookBuilder::new("Test")
    .add_play(
        PlayBuilder::new("Web Setup", "webservers")
            .gather_facts(false)
            .become(true)
            .add_task(
                TaskBuilder::new("Install nginx", "package")
                    .arg("name", "nginx")
                    .arg("state", "present")
                    .notify("restart nginx")
                    .build()
            )
            .build()
    )
    .build();

// Build an inventory
let inventory = InventoryBuilder::new()
    .add_host("web1", Some("webservers"))
    .add_host("db1", Some("databases"))
    .host_var("web1", "priority", 1)
    .group_var("webservers", "http_port", 80)
    .build();
```

### Assertion Helpers

Convenient assertions for common checks:

```rust
use common::*;

// Module assertions
assert_module_success(&result);
assert_module_changed(&result);
assert_module_unchanged(&result);
assert_module_failed(&result);
assert_module_skipped(&result);

// Task assertions
assert_task_success(&task_result);
assert_task_failed(&task_result);

// Host assertions
assert_host_success(&host_result);

// Stats assertions
assert_stats(&stats, ok: 5, changed: 2, failed: 0, skipped: 1, unreachable: 0);
```

### Test Context

Manage temporary directories and test setup:

```rust
use common::TestContext;

let ctx = TestContext::new().unwrap();

// Create files
let path = ctx.create_file("config.yml", "key: value")?;

// Check files
assert!(ctx.file_exists("config.yml"));
let content = ctx.read_file("config.yml")?;

// With mock connection
let ctx = TestContext::with_mock_connection("test-host")?;
let mock = ctx.mock_connection().unwrap();
```

### Fixture Loading

Load test fixtures:

```rust
use common::*;

// Get fixture paths
let path = fixture_path("playbooks/minimal_playbook.yml");

// Load as string
let content = load_fixture("playbooks/minimal_playbook.yml")?;

// Load and parse playbook
let playbook = load_playbook_fixture("minimal_playbook")?;

// Load and parse inventory
let inventory = load_inventory_fixture("simple")?;
```

### Module Params Macro

Quickly create module parameters:

```rust
use common::module_params;

let params = module_params! {
    "name" => "nginx",
    "state" => "present",
    "enabled" => true,
};
```

### Async Helpers

Utilities for async tests:

```rust
use common::*;

#[tokio::test]
async fn test_with_timeout() {
    let result = run_with_timeout(Duration::from_secs(5), async {
        // Your async test code
    }).await.expect("Test timed out");
}

// Pre-configured executor configs
let config = test_executor_config();
let check_config = test_check_mode_config();
```

## Mock Implementations

### When to Use Mocks

Use mocks when:
- Testing code that depends on external systems (SSH, network)
- Testing error handling scenarios
- Testing specific sequences of operations
- Verifying correct interactions

### MockConnection Features

- Track all executed commands
- Configure specific command results
- Simulate failures after N operations
- Virtual filesystem for file operations
- Reset state between tests

### MockModule Features

- Configure execution results
- Separate check mode results
- Track execution count
- Configure required parameters
- Set classification and parallelization hints

## Adding New Tests

### 1. Unit Tests

Add to the appropriate test file:

```rust
#[test]
fn test_new_feature() {
    // Arrange
    let config = MyConfig::default();

    // Act
    let result = config.process();

    // Assert
    assert_eq!(result, expected);
}
```

### 2. Async Tests

Use `#[tokio::test]`:

```rust
#[tokio::test]
async fn test_async_operation() {
    let conn = LocalConnection::new();
    let result = conn.execute("echo test", None).await.unwrap();
    assert!(result.success);
}
```

### 3. Integration Tests

Create a new test file or add to `integration_tests.rs`:

```rust
mod common;
use common::*;

#[tokio::test]
async fn test_full_workflow() {
    let ctx = TestContext::new().unwrap();

    // Setup
    ctx.create_file("playbook.yml", &simple_playbook_yaml())?;

    // Execute
    let playbook = load_playbook_fixture("minimal_playbook").unwrap();
    let executor = Executor::new(test_executor_config());
    let results = executor.run_playbook(&playbook).await.unwrap();

    // Verify
    assert_host_success(&results["localhost"]);
}
```

### 4. Using Fixtures

Add new fixtures to appropriate directories:

```bash
tests/fixtures/playbooks/my_feature_playbook.yml
tests/fixtures/inventories/my_feature_inventory.yml
```

Then use in tests:

```rust
let playbook = load_playbook_fixture("my_feature_playbook").unwrap();
```

## Integration Test Requirements

Some tests require specific environments:

### Local Tests (Always Run)

- Use `LocalConnection` or `MockConnection`
- No external dependencies
- Fast execution

### SSH Tests (Optional)

- Require SSH access to test hosts
- Set `RUSTIBLE_TEST_SSH_HOST` environment variable
- Skip with `#[ignore]` by default

### Docker Tests (Optional)

- Require Docker daemon running
- Enable with `--features docker`
- Skip with `#[ignore]` by default

## Benchmarking

Run performance benchmarks:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench -- execution

# Generate benchmark report
cargo bench -- --save-baseline main
```

Benchmarks are located in `benches/execution_benchmark.rs`.

## Best Practices

1. **Test One Thing** - Each test should verify one specific behavior
2. **Use Descriptive Names** - `test_executor_handles_task_failure`
3. **Follow AAA Pattern** - Arrange, Act, Assert
4. **Clean Up Resources** - Use `TempDir` for files
5. **Test Edge Cases** - Empty inputs, large data, special characters
6. **Mock External Systems** - Don't depend on network in unit tests
7. **Document Test Purpose** - Add comments for complex scenarios
8. **Use Helpers** - Leverage common module for consistency

## Troubleshooting

### Tests Hang

```bash
# Run with timeout
cargo test -- --test-threads=1

# Check for blocking operations
RUST_BACKTRACE=1 cargo test test_name
```

### Flaky Tests

```bash
# Run test multiple times
for i in {1..10}; do cargo test test_name; done

# Check for race conditions
cargo test -- --test-threads=1
```

### Debug Output

```bash
# Show all output
cargo test -- --nocapture

# With logging
RUST_LOG=debug cargo test -- --nocapture
```
