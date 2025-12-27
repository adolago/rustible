# Rustible API Reference

Rustible is an async-first, type-safe configuration management and automation tool written in Rust. This document provides a comprehensive overview of the Rustible API.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Core Modules](#core-modules)
3. [Inventory Management](#inventory-management)
4. [Playbook System](#playbook-system)
5. [Module System](#module-system)
6. [Connection Layer](#connection-layer)
7. [Callback Plugins](#callback-plugins)
8. [Error Handling](#error-handling)
9. [Template Engine](#template-engine)
10. [Vault (Encryption)](#vault-encryption)

---

## Quick Start

Add Rustible to your `Cargo.toml`:

```toml
[dependencies]
rustible = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Basic Usage

```rust
use rustible::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load inventory from YAML file
    let inventory = Inventory::from_file("inventory.yml").await?;

    // Load and parse playbook
    let playbook = Playbook::from_file("playbook.yml").await?;

    // Create executor with parallel execution
    let executor = PlaybookExecutor::new()
        .with_inventory(inventory)
        .with_parallelism(10)
        .build()?;

    // Execute playbook
    let result = executor.run(&playbook).await?;

    // Report results
    println!("{}", result.summary());
    Ok(())
}
```

---

## Core Modules

### Prelude (`rustible::prelude`)

The prelude provides convenient re-exports of commonly used types:

```rust
use rustible::prelude::*;

// Includes:
// - Connection types: SshConnection, LocalConnection, DockerConnection
// - Execution: PlaybookExecutor, TaskExecutor
// - Inventory: Inventory, Host, Group
// - Modules: Module, ModuleRegistry, ModuleResult
// - Callbacks: DefaultCallback, MinimalCallback, ProgressCallback
// - Errors: Error, Result
```

### Version Information

```rust
use rustible::{version, version_info, VersionInfo};

// Get version string
let ver: &str = version();

// Get detailed version info
let info: VersionInfo = version_info();
println!("{}", info);  // "rustible 0.1.0 (x86_64, release)"
```

---

## Inventory Management

The inventory system manages hosts and groups for automation targets.

### Loading Inventory

```rust
use rustible::inventory::{Inventory, Host, Group};

// From YAML file
let inventory = Inventory::from_file("inventory.yml").await?;

// From INI file (Ansible-style)
let inventory = Inventory::from_file("inventory.ini").await?;

// Programmatically
let mut inventory = Inventory::new();
inventory.add_host(Host::new("webserver1").with_var("http_port", 8080));
inventory.add_group(Group::new("webservers").with_host("webserver1"));
```

### Host Pattern Matching

```rust
// Get all hosts
let all_hosts = inventory.get_hosts("all")?;

// Get hosts by group
let web_hosts = inventory.get_hosts("webservers")?;

// Pattern matching
let matched = inventory.get_hosts("web*")?;      // Glob pattern
let excluded = inventory.get_hosts("all:!db")?;  // Exclude pattern
let intersect = inventory.get_hosts("web:&prod")?;  // Intersection
```

### Host Variables

```rust
let host = inventory.get_host("webserver1")?;

// Access variables
let port: u16 = host.get_var("http_port").unwrap_or(80);
let ansible_host: &str = host.get_var("ansible_host").unwrap_or(&host.name);

// Set variables
host.set_var("custom_var", serde_json::json!({"nested": "value"}));
```

### Inventory File Formats

**YAML Format:**
```yaml
all:
  hosts:
    webserver1:
      ansible_host: 192.168.1.10
      http_port: 8080
  children:
    webservers:
      hosts:
        webserver1:
    databases:
      hosts:
        db1:
          ansible_host: 192.168.1.20
```

**INI Format:**
```ini
[webservers]
webserver1 ansible_host=192.168.1.10 http_port=8080

[databases]
db1 ansible_host=192.168.1.20

[all:vars]
ansible_user=admin
```

---

## Playbook System

Playbooks define automation workflows using YAML syntax.

### Loading Playbooks

```rust
use rustible::playbook::{Playbook, Play, Task};

// Load from file
let playbook = Playbook::from_file("playbook.yml").await?;

// Access plays
for play in &playbook.plays {
    println!("Play: {}", play.name);
    for task in &play.tasks {
        println!("  Task: {}", task.name);
    }
}
```

### Playbook Structure

```yaml
---
- name: Configure web servers
  hosts: webservers
  become: true
  vars:
    http_port: 8080

  tasks:
    - name: Install nginx
      apt:
        name: nginx
        state: present

    - name: Start nginx service
      service:
        name: nginx
        state: started
        enabled: true
      notify: Reload nginx

  handlers:
    - name: Reload nginx
      service:
        name: nginx
        state: reloaded
```

### Task Execution

```rust
use rustible::executor::{PlaybookExecutor, ExecutorConfig};

let config = ExecutorConfig {
    forks: 10,           // Parallel execution
    check_mode: false,   // Dry-run mode
    diff_mode: true,     // Show changes
    ..Default::default()
};

let executor = PlaybookExecutor::new()
    .with_config(config)
    .with_inventory(inventory)
    .build()?;

let results = executor.run(&playbook).await?;

// Check results
for result in &results {
    match result.status {
        TaskStatus::Ok => println!("OK: {}", result.task_name),
        TaskStatus::Changed => println!("CHANGED: {}", result.task_name),
        TaskStatus::Failed => eprintln!("FAILED: {}", result.task_name),
        TaskStatus::Skipped => println!("SKIPPED: {}", result.task_name),
    }
}
```

---

## Module System

Modules perform the actual work on target systems.

### Built-in Modules

| Module | Description |
|--------|-------------|
| `apt` | Debian/Ubuntu package management |
| `yum`/`dnf` | RHEL/Fedora package management |
| `pip` | Python package management |
| `copy` | Copy files to remote hosts |
| `file` | Manage files and directories |
| `template` | Deploy Jinja2 templates |
| `lineinfile` | Manage lines in files |
| `command` | Execute commands |
| `shell` | Execute shell commands |
| `service` | Manage system services |
| `user` | Manage user accounts |
| `group` | Manage groups |
| `git` | Git repository operations |

### Using the Module Registry

```rust
use rustible::modules::{ModuleRegistry, ModuleContext, ModuleParams};

// Create registry with all built-in modules
let registry = ModuleRegistry::with_builtins();

// Execute a module
let params: ModuleParams = serde_json::from_value(serde_json::json!({
    "name": "nginx",
    "state": "present"
}))?;

let result = registry.execute("apt", &params, &context).await?;

if result.changed {
    println!("Package was installed");
}
```

### Implementing Custom Modules

```rust
use rustible::traits::{Module, ModuleResult};
use async_trait::async_trait;

#[derive(Debug)]
pub struct MyCustomModule;

#[async_trait]
impl Module for MyCustomModule {
    fn name(&self) -> &'static str {
        "my_custom"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ExecutionContext,
    ) -> Result<ModuleResult> {
        // Validate arguments
        let target = args.get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::module_args("my_custom", "missing 'target' argument"))?;

        // Perform work...

        Ok(ModuleResult {
            changed: true,
            msg: Some("Custom action completed".to_string()),
            ..Default::default()
        })
    }
}

// Register the module
registry.register(Box::new(MyCustomModule));
```

---

## Connection Layer

The connection layer handles communication with target hosts.

### SSH Connections (Russh Backend)

```rust
use rustible::connection::russh::{RusshConnection, RusshConnectionBuilder};

let connection = RusshConnectionBuilder::new()
    .host("192.168.1.10")
    .port(22)
    .username("admin")
    .private_key_path("/home/user/.ssh/id_rsa")
    .timeout(Duration::from_secs(30))
    .build()
    .await?;

// Execute command
let result = connection.execute("uname -a").await?;
println!("Output: {}", result.stdout);

// Transfer file
connection.upload("/local/file.txt", "/remote/file.txt").await?;
```

### Local Connections

```rust
use rustible::connection::local::LocalConnection;

let connection = LocalConnection::new();
let result = connection.execute("hostname").await?;
```

### Docker Connections

```rust
use rustible::connection::docker::DockerConnection;

let connection = DockerConnection::new("container_name");
let result = connection.execute("cat /etc/os-release").await?;
```

### Connection Trait

```rust
use rustible::connection::{Connection, CommandResult};

// The Connection trait defines the interface
#[async_trait]
pub trait Connection: Send + Sync {
    async fn execute(&self, command: &str) -> Result<CommandResult>;
    async fn execute_with_options(&self, command: &str, options: &ExecuteOptions) -> Result<CommandResult>;
    async fn upload(&self, local_path: &Path, remote_path: &Path) -> Result<()>;
    async fn download(&self, remote_path: &Path, local_path: &Path) -> Result<()>;
    async fn stat(&self, path: &Path) -> Result<FileStat>;
}
```

---

## Callback Plugins

Callbacks receive execution events for logging, metrics, or custom integrations.

### Built-in Callbacks

```rust
use rustible::callback::prelude::*;

// Default Ansible-like output
let default = DefaultCallback::new();

// Minimal output (only failures and recap)
let minimal = MinimalCallback::new();

// Progress bars for interactive use
let progress = ProgressCallback::new();

// JSON-formatted output
let json = JsonCallback::new();

// Silent with summary at end
let summary = SummaryCallback::new();

// Combine multiple callbacks
let composite = CompositeCallback::new()
    .with_callback(Box::new(ProgressCallback::new()))
    .with_callback(Box::new(JsonCallback::to_file("output.json")));
```

### Callback Categories

| Category | Callbacks |
|----------|-----------|
| **Core Output** | `DefaultCallback`, `MinimalCallback`, `SummaryCallback`, `NullCallback` |
| **Visual** | `ProgressCallback`, `DiffCallback`, `DenseCallback`, `OnelineCallback`, `TreeCallback` |
| **Timing** | `TimerCallback`, `ContextCallback`, `StatsCallback`, `CounterCallback` |
| **Filtering** | `SelectiveCallback`, `SkippyCallback`, `ActionableCallback`, `FullSkipCallback` |
| **Logging** | `JsonCallback`, `YamlCallback`, `LogFileCallback`, `SyslogCallback`, `DebugCallback` |
| **Integration** | `JUnitCallback`, `MailCallback`, `ForkedCallback` |

### Implementing Custom Callbacks

```rust
use rustible::callback::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
struct MetricsCallback {
    task_count: AtomicUsize,
    change_count: AtomicUsize,
}

#[async_trait]
impl ExecutionCallback for MetricsCallback {
    async fn on_task_start(&self, task: &str, host: &str) {
        println!("Starting: {} on {}", task, host);
    }

    async fn on_task_complete(&self, result: &ExecutionResult) {
        self.task_count.fetch_add(1, Ordering::SeqCst);
        if result.changed {
            self.change_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    async fn on_playbook_complete(&self) {
        let tasks = self.task_count.load(Ordering::SeqCst);
        let changes = self.change_count.load(Ordering::SeqCst);
        println!("Completed {} tasks with {} changes", tasks, changes);
    }
}
```

---

## Error Handling

Rustible uses a comprehensive error type with rich context.

### Error Types

```rust
use rustible::error::{Error, Result, ErrorContext, EnrichedError};

// Common error patterns
fn example() -> Result<()> {
    // Task failures
    Err(Error::task_failed("Install nginx", "webserver1", "Package not found"))?;

    // Connection errors
    Err(Error::connection_failed("192.168.1.10", "Connection refused"))?;

    // Module errors
    Err(Error::module_args("apt", "missing 'name' argument"))?;

    Ok(())
}
```

### Error Context

```rust
use rustible::error::{Error, ErrorContext, EnrichedError};

// Create enriched errors with context
let context = ErrorContext::new()
    .with_file("playbook.yml")
    .with_line(42)
    .with_task("Install nginx")
    .with_host("webserver1");

let enriched = Error::task_failed_enriched(
    "Install nginx",
    "webserver1",
    "Package not found",
    Some(context),
);

// Prints detailed error with hints and suggestions
println!("{}", enriched.format());
```

### Error Categories

| Category | Error Types |
|----------|-------------|
| **Playbook** | `PlaybookParse`, `PlaybookValidation`, `PlayNotFound` |
| **Task** | `TaskFailed`, `TaskTimeout`, `TaskSkipped` |
| **Module** | `ModuleNotFound`, `ModuleArgs`, `ModuleExecution` |
| **Inventory** | `InventoryLoad`, `HostNotFound`, `GroupNotFound`, `InvalidHostPattern` |
| **Connection** | `ConnectionFailed`, `ConnectionTimeout`, `AuthenticationFailed`, `RemoteCommandFailed` |
| **Variables** | `UndefinedVariable`, `InvalidVariableValue`, `VariablesFileNotFound` |
| **Template** | `TemplateSyntax`, `TemplateRender` |
| **Role** | `RoleNotFound`, `RoleDependency`, `InvalidRole` |
| **Vault** | `VaultDecryption`, `VaultEncryption`, `InvalidVaultPassword` |
| **Handler** | `HandlerNotFound`, `HandlerFailed` |

---

## Template Engine

Rustible uses minijinja for Jinja2-compatible templating.

### Basic Usage

```rust
use rustible::template::TemplateEngine;
use std::collections::HashMap;

let engine = TemplateEngine::new();

let mut vars = HashMap::new();
vars.insert("name".to_string(), serde_json::json!("World"));
vars.insert("port".to_string(), serde_json::json!(8080));

let result = engine.render("Hello, {{ name }}! Port: {{ port }}", &vars)?;
assert_eq!(result, "Hello, World! Port: 8080");
```

### Template Syntax Detection

```rust
use rustible::template::TemplateEngine;

// Check if a string contains template syntax
assert!(TemplateEngine::is_template("{{ var }}"));
assert!(TemplateEngine::is_template("{% if condition %}"));
assert!(!TemplateEngine::is_template("plain text"));
```

### Template Features

- **Variables**: `{{ variable }}`
- **Filters**: `{{ name | upper }}`, `{{ list | join(",") }}`
- **Conditionals**: `{% if condition %}...{% endif %}`
- **Loops**: `{% for item in list %}...{% endfor %}`
- **Comments**: `{# This is a comment #}`
- **Include**: `{% include "other.j2" %}`

---

## Vault (Encryption)

Rustible supports Ansible Vault-compatible encryption.

### Encrypting Data

```rust
use rustible::vault::Vault;

let vault = Vault::new("my_secret_password");

// Encrypt a string
let encrypted = vault.encrypt("sensitive data")?;
println!("{}", encrypted);
// Output: $RUSTIBLE_VAULT;1.0;AES256
// [base64-encoded encrypted content]

// Encrypt a file
vault.encrypt_file("secrets.yml", "secrets.yml.vault")?;
```

### Decrypting Data

```rust
use rustible::vault::Vault;

let vault = Vault::new("my_secret_password");

// Decrypt vault content
let decrypted = vault.decrypt(&encrypted_content)?;

// Decrypt a file
let content = vault.decrypt_file("secrets.yml.vault")?;
```

### Vault Format

Rustible vault files use the format:
```
$RUSTIBLE_VAULT;1.0;AES256
[base64-encoded content]
```

The encryption uses:
- **Algorithm**: AES-256-GCM
- **Key Derivation**: Argon2id
- **Encoding**: Base64

---

## See Also

- [Module Reference](modules.md) - Detailed documentation for all built-in modules
- [Callback Reference](callbacks.md) - Complete callback plugin documentation
- [CLI Reference](../cli.md) - Command-line interface documentation
- [Migration Guide](../migration.md) - Migrating from Ansible to Rustible
