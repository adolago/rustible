# Rustible Architecture

This document describes the internal architecture of Rustible, a modern configuration management tool written in Rust.

## Design Principles

1. **Async-First**: All I/O operations are asynchronous using Tokio
2. **Type Safety**: Strong typing prevents runtime errors
3. **Parallel by Default**: Tasks execute concurrently across hosts
4. **Ansible Compatibility**: Familiar YAML syntax for easy migration
5. **Extensibility**: Plugin architecture for modules and connections
6. **Performance**: Zero-cost abstractions and efficient resource usage

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CLI Layer                                       │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │  Argument       │  │  Configuration  │  │  Output                     │  │
│  │  Parser (clap)  │  │  Loader         │  │  Formatter                  │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Execution Engine                                   │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │  Playbook       │  │  Task           │  │  Strategy                   │  │
│  │  Executor       │  │  Executor       │  │  Manager                    │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
│                                                                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │  Handler        │  │  Variable       │  │  Callback                   │  │
│  │  Manager        │  │  Resolver       │  │  System                     │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
          ┌─────────────────────────┼─────────────────────────┐
          ▼                         ▼                         ▼
┌───────────────────┐   ┌───────────────────┐   ┌───────────────────────────┐
│    Inventory      │   │     Modules       │   │    Template Engine        │
│    ┌───────────┐  │   │   ┌───────────┐   │   │   ┌───────────────────┐   │
│    │  Parser   │  │   │   │ Registry  │   │   │   │  MiniJinja        │   │
│    ├───────────┤  │   │   ├───────────┤   │   │   ├───────────────────┤   │
│    │  Groups   │  │   │   │ Built-in  │   │   │   │  Filters          │   │
│    ├───────────┤  │   │   ├───────────┤   │   │   ├───────────────────┤   │
│    │  Hosts    │  │   │   │ Custom    │   │   │   │  Tests            │   │
│    └───────────┘  │   │   └───────────┘   │   │   └───────────────────┘   │
└───────────────────┘   └───────────────────┘   └───────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Connection Layer                                    │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │  Connection     │  │  Connection     │  │  Privilege                  │  │
│  │  Factory        │  │  Pool           │  │  Escalation                 │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  │
│                                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │  SSH        │  │  Local      │  │  Docker     │  │  Kubernetes         │ │
│  │  Connection │  │  Connection │  │  Connection │  │  Connection         │ │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Target Hosts                                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Playbook Parser (`src/playbook.rs`, `src/parser/`)

Responsible for parsing YAML playbooks into strongly-typed Rust structures.

```rust
pub struct Playbook {
    pub name: Option<String>,
    pub plays: Vec<Play>,
    pub source_path: Option<PathBuf>,
}

pub struct Play {
    pub name: String,
    pub hosts: String,
    pub tasks: Vec<Task>,
    pub handlers: Vec<Handler>,
    pub vars: Variables,
    // ...
}

pub struct Task {
    pub name: String,
    pub module: TaskModule,
    pub when: Option<When>,
    pub notify: Vec<String>,
    // ...
}
```

**Key Features:**
- Serde-based deserialization with custom error handling
- Validation of playbook structure before execution
- Support for includes, imports, and role references

### 2. Inventory System (`src/inventory/`)

Manages host and group information from multiple sources.

```rust
pub struct Inventory {
    hosts: HashMap<String, Host>,
    groups: HashMap<String, Group>,
    vars: Variables,
}

pub struct Host {
    pub name: String,
    pub vars: Variables,
}

pub struct Group {
    pub name: String,
    pub hosts: Vec<String>,
    pub children: Vec<String>,
    pub vars: Variables,
}
```

**Supported Formats:**
- YAML inventory (Ansible-compatible)
- INI inventory (Ansible-compatible)
- Dynamic inventory scripts (executable returning JSON)

**Host Pattern Matching:**
- `all` - All hosts
- `groupname` - Hosts in group
- `host1:host2` - Union
- `group1:&group2` - Intersection
- `group1:!group2` - Difference
- `~regex` - Regex matching
- `host[1:5]` - Range expansion

### 3. Connection Layer (`src/connection/`)

Provides transport abstraction for executing commands on targets.

```rust
#[async_trait]
pub trait Connection: Send + Sync {
    fn identifier(&self) -> &str;
    async fn is_alive(&self) -> bool;
    async fn execute(&self, command: &str, options: Option<ExecuteOptions>)
        -> ConnectionResult<CommandResult>;
    async fn upload(&self, local_path: &Path, remote_path: &Path,
        options: Option<TransferOptions>) -> ConnectionResult<()>;
    async fn download(&self, remote_path: &Path, local_path: &Path)
        -> ConnectionResult<()>;
    async fn stat(&self, path: &Path) -> ConnectionResult<FileStat>;
    async fn close(&self) -> ConnectionResult<()>;
}
```

**Connection Types:**

| Type | Implementation | Use Case |
|------|---------------|----------|
| SSH | `ssh2` crate | Remote Linux/Unix hosts |
| Local | Direct execution | Localhost |
| Docker | Docker API | Containers |
| Kubernetes | Kubernetes API | Pods (planned) |

**Connection Pooling:**
- Connections are pooled and reused
- Configurable pool size per host
- Automatic cleanup of stale connections
- SSH ControlMaster support for multiplexing

### 4. Module System (`src/modules/`)

Modules are the units of work that perform actions on targets.

```rust
#[async_trait]
pub trait Module: Send + Sync + Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str;

    async fn execute(&self, args: &ModuleArgs, ctx: &ExecutionContext)
        -> Result<ModuleResult>;

    async fn check(&self, args: &ModuleArgs, ctx: &ExecutionContext)
        -> Result<ModuleResult>;
}

pub struct ModuleResult {
    pub success: bool,
    pub changed: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}
```

**Built-in Modules:**
- `command` / `shell` - Command execution
- `copy` / `template` - File operations
- `file` - File management
- `package` - Package management
- `service` - Service control
- `user` / `group` - User management
- `git` - Git operations
- `debug` / `assert` - Debugging

**Module Registry:**
```rust
pub struct ModuleRegistry {
    modules: HashMap<String, Arc<dyn Module>>,
}

impl ModuleRegistry {
    pub fn register(&mut self, module: impl Module + 'static);
    pub fn get(&self, name: &str) -> Option<Arc<dyn Module>>;
}
```

### 5. Execution Engine (`src/executor/`)

Orchestrates playbook execution across hosts.

```rust
pub struct PlaybookExecutor {
    inventory: Inventory,
    module_registry: ModuleRegistry,
    connection_factory: ConnectionFactory,
    config: ExecutorConfig,
}

impl PlaybookExecutor {
    pub async fn run(&self, playbook: &Playbook) -> Result<ExecutionReport>;
}
```

**Execution Flow:**
1. Parse and validate playbook
2. Resolve target hosts from inventory
3. For each play:
   a. Gather facts (if enabled)
   b. Execute pre_tasks
   c. Apply roles
   d. Execute tasks
   e. Execute post_tasks
   f. Run triggered handlers
4. Generate execution report

### 6. Execution Strategies (`src/strategy.rs`)

Control how tasks are distributed across hosts.

```rust
#[async_trait]
pub trait ExecutionStrategy: Send + Sync {
    fn name(&self) -> &str;

    async fn execute<F>(&self, tasks: &[Task], hosts: &[String],
        executor: F) -> Result<Vec<TaskResult>>
    where
        F: Fn(&str, &Task) -> BoxFuture<Result<TaskResult>> + Send + Sync;
}
```

**Available Strategies:**

| Strategy | Behavior |
|----------|----------|
| Linear | Execute task on all hosts before next task |
| Free | Execute independently, no synchronization |
| Parallel | Execute with configurable concurrency |

### 7. Variable System (`src/vars/`)

Manages variables with proper precedence and scoping.

```rust
pub struct Variables {
    inner: IndexMap<String, serde_json::Value>,
}

impl Variables {
    pub fn get(&self, key: &str) -> Option<&serde_json::Value>;
    pub fn set(&mut self, key: &str, value: serde_json::Value);
    pub fn merge(&mut self, other: &Variables);
}
```

**Variable Precedence (lowest to highest):**
1. Role defaults
2. Inventory group_vars
3. Inventory host_vars
4. Playbook group_vars
5. Playbook host_vars
6. Host facts
7. Play vars
8. Role vars
9. Task vars
10. Extra vars (-e)

### 8. Template Engine (`src/template.rs`)

Jinja2-compatible templating using MiniJinja.

```rust
pub struct TemplateEngine {
    env: minijinja::Environment<'static>,
}

impl TemplateEngine {
    pub fn render(&self, template: &str, vars: &Variables) -> Result<String>;
    pub fn render_file(&self, path: &Path, vars: &Variables) -> Result<String>;
}
```

**Supported Features:**
- Variable interpolation: `{{ variable }}`
- Filters: `{{ name | upper }}`
- Conditionals: `{% if condition %}...{% endif %}`
- Loops: `{% for item in list %}...{% endfor %}`
- Includes: `{% include "file.j2" %}`
- Macros: `{% macro name() %}...{% endmacro %}`

### 9. Vault (`src/vault.rs`)

Encryption for sensitive data.

```rust
pub struct Vault {
    cipher: Aes256Gcm,
}

impl Vault {
    pub fn new(password: &str) -> Result<Self>;
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>>;
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>>;
}
```

**Encryption Details:**
- Algorithm: AES-256-GCM
- Key derivation: Argon2id
- Format: Compatible with Ansible Vault 1.2

### 10. Fact Gathering (`src/facts.rs`)

Collects system information from hosts.

```rust
pub struct Facts {
    data: serde_json::Value,
}

impl Facts {
    pub async fn gather(connection: &dyn Connection) -> Result<Self>;
    pub fn get(&self, key: &str) -> Option<&serde_json::Value>;
}
```

**Gathered Facts:**
- `ansible_hostname` - System hostname
- `ansible_fqdn` - Fully qualified domain name
- `ansible_os_family` - OS family (Debian, RedHat, etc.)
- `ansible_distribution` - Distribution name
- `ansible_distribution_version` - Distribution version
- `ansible_architecture` - CPU architecture
- `ansible_memtotal_mb` - Total memory
- `ansible_processor_count` - CPU count
- `ansible_default_ipv4` - Default IPv4 address
- And many more...

## Data Flow

### Playbook Execution Flow

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Parse      │────▶│   Validate   │────▶│   Resolve    │
│   Playbook   │     │   Structure  │     │   Hosts      │
└──────────────┘     └──────────────┘     └──────────────┘
                                                 │
                                                 ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Report     │◀────│   Execute    │◀────│   Prepare    │
│   Results    │     │   Tasks      │     │   Context    │
└──────────────┘     └──────────────┘     └──────────────┘
```

### Task Execution Flow

```
┌───────────────────────────────────────────────────────────────────┐
│                         Task Executor                              │
├───────────────────────────────────────────────────────────────────┤
│  1. Check 'when' condition                                        │
│  2. Resolve variables and template arguments                      │
│  3. Check if loop required                                        │
│  4. For each iteration:                                           │
│     a. Get module from registry                                   │
│     b. Execute module with connection context                     │
│     c. Check changed_when / failed_when                          │
│     d. Register result if requested                              │
│  5. Queue handlers if changed                                     │
│  6. Return aggregated result                                      │
└───────────────────────────────────────────────────────────────────┘
```

## Async Architecture

Rustible uses Tokio for async execution:

```rust
// Parallel host execution
let results: Vec<TaskResult> = futures::future::join_all(
    hosts.iter().map(|host| {
        let executor = executor.clone();
        let task = task.clone();
        async move {
            executor.execute_task(&task, host).await
        }
    })
).await;

// Connection pooling with semaphore
let permit = semaphore.acquire().await?;
let connection = pool.get_or_create(&host).await?;
let result = connection.execute(command, options).await?;
drop(permit);
```

**Concurrency Control:**
- `forks` setting limits parallel host connections
- Semaphores prevent connection exhaustion
- Backpressure handling for slow hosts

## Error Handling

```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("Task '{task}' failed on host '{host}': {message}")]
    TaskFailed { task: String, host: String, message: String },

    #[error("Connection failed to '{host}': {message}")]
    ConnectionFailed { host: String, message: String },

    #[error("Module '{module}' not found")]
    ModuleNotFound(String),

    // ...
}
```

**Error Recovery:**
- `ignore_errors: true` - Continue on failure
- `rescue` blocks - Handle failures
- `always` blocks - Cleanup regardless of success
- Retry logic with `retries` and `delay`

## Extension Points

### Custom Modules

```rust
#[derive(Debug)]
struct MyModule;

#[async_trait]
impl Module for MyModule {
    fn name(&self) -> &str { "my_module" }

    async fn execute(&self, args: &ModuleArgs, ctx: &ExecutionContext)
        -> Result<ModuleResult> {
        // Implementation
    }
}

// Register
registry.register(MyModule);
```

### Custom Connections

```rust
#[derive(Debug)]
struct CustomConnection { /* ... */ }

#[async_trait]
impl Connection for CustomConnection {
    fn identifier(&self) -> &str { "custom://target" }

    async fn execute(&self, command: &str, options: Option<ExecuteOptions>)
        -> ConnectionResult<CommandResult> {
        // Implementation
    }

    // ... other trait methods
}
```

### Custom Inventory Sources

```rust
#[async_trait]
impl InventorySource for CloudInventory {
    fn name(&self) -> &str { "cloud" }

    async fn load(&self) -> Result<InventoryData> {
        // Query cloud API
    }
}
```

## Performance Optimizations

1. **Connection Pooling**: Reuse SSH connections across tasks
2. **Parallel Execution**: Execute across hosts concurrently
3. **Lazy Evaluation**: Only render templates when needed
4. **Fact Caching**: Cache gathered facts per execution
5. **Compiled Modules**: Native Rust performance
6. **Zero-Copy Parsing**: Efficient YAML deserialization

## Security Considerations

- **Vault Encryption**: AES-256-GCM with Argon2id key derivation
- **No Secrets in Logs**: Sensitive data masked in output
- **SSH Key Handling**: Keys read directly, never logged
- **Privilege Escalation**: Configurable become methods
- **Host Key Checking**: Enabled by default

## Future Enhancements

1. **Kubernetes Connection**: Execute in pods
2. **WinRM Support**: Windows remote management
3. **Distributed Execution**: Multiple control nodes
4. **Web Interface**: REST API and web UI
5. **Plugin System**: Dynamic loading of extensions
6. **Ansible Galaxy**: Role and collection support
