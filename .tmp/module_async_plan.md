# Module Async Migration Plan for russh Integration

## Executive Summary

This document analyzes the current module system in Rustible and proposes a migration plan to properly support async connection methods. The goal is to enable modules to execute remote operations via the async `Connection` trait without blocking the Tokio runtime.

## Current Architecture Analysis

### ModuleContext (src/modules/mod.rs)

The `ModuleContext` struct currently includes:

```rust
pub struct ModuleContext {
    pub check_mode: bool,
    pub diff_mode: bool,
    pub vars: HashMap<String, serde_json::Value>,
    pub facts: HashMap<String, serde_json::Value>,
    pub work_dir: Option<String>,
    pub r#become: bool,
    pub become_method: Option<String>,
    pub become_user: Option<String>,
    pub connection: Option<Arc<dyn Connection + Send + Sync>>,  // <-- async connection
}
```

**Key observation**: The `connection` field already provides an async-capable `Connection` trait object.

### Module Trait (src/modules/mod.rs lines 398-464)

The current `Module` trait is **synchronous**:

```rust
pub trait Module: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn classification(&self) -> ModuleClassification { ... }
    fn parallelization_hint(&self) -> ParallelizationHint { ... }
    fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput>;
    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput>;
    fn diff(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<Option<Diff>>;
    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()>;
    fn required_params(&self) -> &[&'static str];
    fn optional_params(&self) -> HashMap<&'static str, serde_json::Value>;
}
```

### Connection Trait (src/connection/mod.rs)

The `Connection` trait is **async** (uses `#[async_trait]`):

```rust
#[async_trait]
pub trait Connection: Send + Sync {
    fn identifier(&self) -> &str;
    async fn is_alive(&self) -> bool;
    async fn execute(&self, command: &str, options: Option<ExecuteOptions>) -> ConnectionResult<CommandResult>;
    async fn upload(&self, local_path: &Path, remote_path: &Path, options: Option<TransferOptions>) -> ConnectionResult<()>;
    async fn upload_content(&self, content: &[u8], remote_path: &Path, options: Option<TransferOptions>) -> ConnectionResult<()>;
    async fn download(&self, remote_path: &Path, local_path: &Path) -> ConnectionResult<()>;
    async fn download_content(&self, remote_path: &Path) -> ConnectionResult<Vec<u8>>;
    async fn path_exists(&self, path: &Path) -> ConnectionResult<bool>;
    async fn is_directory(&self, path: &Path) -> ConnectionResult<bool>;
    async fn stat(&self, path: &Path) -> ConnectionResult<FileStat>;
    async fn close(&self) -> ConnectionResult<()>;
}
```

### Dual Trait System

Interestingly, there are **two Module traits** in the codebase:

1. **`src/modules/mod.rs`**: Synchronous `Module` trait
2. **`src/traits.rs`**: Async `Module` trait with `#[async_trait]`

The async version in `traits.rs` is:
```rust
#[async_trait]
pub trait Module: Send + Sync + Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str { "No description available" }
    fn validate_args(&self, args: &dyn ModuleArgs) -> Result<()> { ... }
    async fn execute(&self, args: &dyn ModuleArgs, ctx: &ExecutionContext) -> Result<ModuleResult>;
    async fn check(&self, args: &dyn ModuleArgs, ctx: &ExecutionContext) -> Result<ModuleResult>;
    async fn diff(&self, args: &dyn ModuleArgs, ctx: &ExecutionContext) -> Result<Option<ModuleDiff>>;
}
```

---

## Module Classification and Current State

### Modules By Classification

#### 1. LocalLogic (No Connection Needed)
These modules run entirely on the control node:
- `debug.rs` - Debug output
- `set_fact.rs` - Set variables
- `assert.rs` - Assertions

**Status**: No changes needed. These should remain synchronous.

#### 2. NativeTransport (Need Async Connection)
These modules use SSH/SFTP for file operations:
- `copy.rs` - File copying
- `template.rs` - Template rendering
- `lineinfile.rs` - Line-in-file editing
- `blockinfile.rs` - Block-in-file editing
- `stat.rs` - File statistics
- `file.rs` - File/directory management

**Status**: These need async execution to use `connection.upload()`, `connection.download_content()`, etc.

#### 3. RemoteCommand (Need Async Connection)
These modules execute commands remotely:
- `command.rs` - Execute commands
- `shell.rs` - Shell execution
- `service.rs` - Service management
- `user.rs` - User management
- `group.rs` - Group management
- `apt.rs` - APT package management
- `dnf.rs` - DNF package management
- `yum.rs` - YUM package management
- `pip.rs` - PIP package management
- `package.rs` - Generic package management
- `git.rs` - Git operations

**Status**: These need async execution to use `connection.execute()`.

#### 4. PythonFallback
- `python.rs` - Python module executor

**Status**: Already uses async internally via `execute()` method.

---

## Current Blocking Patterns Identified

### Pattern 1: tokio::task::block_in_place (yum.rs)

```rust
// Found in yum.rs lines 170, 364
let result = tokio::task::block_in_place(|| {
    tokio::runtime::Handle::current().block_on(async {
        // async code here
    })
});
```

**Problem**: This pattern creates a new runtime or uses `block_in_place` to bridge sync->async, which is inefficient and can cause deadlocks with russh.

### Pattern 2: Sync runtime creation (copy.rs, lineinfile.rs)

```rust
// Found in copy.rs line 111, lineinfile.rs line 277
let rt = tokio::runtime::Runtime::new().map_err(|e| {
    ModuleError::ExecutionFailed(format!("Failed to create runtime: {}", e))
})?;

rt.block_on(async {
    // async code
})
```

**Problem**: Creating new runtimes is expensive and problematic when already inside a Tokio runtime.

### Pattern 3: Local-only execution with TODO comments

```rust
// Found in: copy.rs line 406, lineinfile.rs line 497, stat.rs line 50
// TODO: Remote execution removed temporarily - ModuleContext no longer has connection field
```

**Observation**: The connection field is now present in ModuleContext, but modules haven't been updated to use it.

### Pattern 4: Direct std::process::Command (user.rs, service.rs)

```rust
// user.rs, service.rs, group.rs use std::process::Command directly
Command::new("useradd")
    .args(...)
    .output()
```

**Problem**: These execute locally but should use `connection.execute()` for remote hosts.

---

## Modules Needing Updates

### Priority 1: High-Usage Modules

| Module | File | Issues | Blocking Calls |
|--------|------|--------|----------------|
| copy | copy.rs | Has execute_remote() with Runtime::new() | Yes |
| template | template.rs | Local only | No connection usage |
| command | command.rs | Uses std::process::Command | No connection usage |
| shell | shell.rs | Uses std::process::Command | No connection usage |
| lineinfile | lineinfile.rs | Has execute_remote() with Runtime::new() | Yes |
| service | service.rs | Uses std::process::Command | Local only |
| user | user.rs | Uses std::process::Command | Local only |

### Priority 2: Package Managers

| Module | File | Issues | Blocking Calls |
|--------|------|--------|----------------|
| yum | yum.rs | Uses block_in_place, returns Unsupported error | Yes (commented) |
| apt | apt.rs | Local std::process::Command | No |
| dnf | dnf.rs | Local std::process::Command | No |

### Priority 3: Lower Usage

| Module | File | Issues |
|--------|------|--------|
| stat | stat.rs | Local only, needs remote support |
| file | file.rs | Local only |
| group | group.rs | Local only |
| blockinfile | blockinfile.rs | Local only |
| git | git.rs | Local only |
| pip | pip.rs | Local only |

---

## Proposed Migration Strategy

### Option A: Make Module Trait Async (Recommended)

Convert the sync `Module` trait to async:

```rust
#[async_trait]
pub trait Module: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn classification(&self) -> ModuleClassification { ModuleClassification::RemoteCommand }
    fn parallelization_hint(&self) -> ParallelizationHint { ParallelizationHint::FullyParallel }

    // Make these async
    async fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput>;
    async fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput>;
    async fn diff(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<Option<Diff>>;

    // These stay sync (no I/O)
    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()>;
    fn required_params(&self) -> &[&'static str];
    fn optional_params(&self) -> HashMap<&'static str, serde_json::Value>;
}
```

**Pros**:
- Clean async/await throughout
- No blocking workarounds
- Proper integration with russh async SSH
- Matches the async Module trait in `traits.rs`

**Cons**:
- Breaking change for all modules
- Requires updating ~20 module implementations

### Option B: Hybrid Approach with AsyncModule Trait

Add a new async trait alongside the existing sync one:

```rust
#[async_trait]
pub trait AsyncModule: Module {
    async fn execute_async(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput>;
}

// Default implementation bridges to sync
impl<T: Module> AsyncModule for T {
    async fn execute_async(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        // Use tokio::task::spawn_blocking for sync modules
        let params = params.clone();
        let context = context.clone();
        tokio::task::spawn_blocking(move || self.execute(&params, &context)).await??
    }
}
```

**Pros**:
- Backward compatible
- Incremental migration possible

**Cons**:
- More complex
- Still requires spawn_blocking for sync modules
- May have issues with russh requiring async-only operation

### Option C: Unify with traits.rs Module Trait

The async `Module` trait in `traits.rs` already exists. We could:
1. Remove the sync Module trait from `modules/mod.rs`
2. Migrate all modules to use the async trait from `traits.rs`
3. Update `ModuleRegistry` to use async execution

**Pros**:
- Eliminates duplicate trait definitions
- Single source of truth for Module interface

**Cons**:
- Requires significant refactoring
- Different argument types (ModuleArgs vs ModuleParams)

---

## Recommended Migration Plan

### Phase 1: Prepare (No Breaking Changes)

1. Add `#[async_trait]` attribute to Module trait
2. Create async wrapper methods that call sync versions
3. Update executor to call async methods

### Phase 2: Migrate High-Priority Modules

1. **copy.rs**: Convert execute_remote() to async method
2. **lineinfile.rs**: Convert execute_remote() to async method
3. **command.rs/shell.rs**: Add connection.execute() support
4. **template.rs**: Add remote upload support

For each module:
```rust
async fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
    if let Some(ref connection) = context.connection {
        // Remote execution
        self.execute_remote(connection.clone(), params, context).await
    } else {
        // Local execution
        self.execute_local(params, context)
    }
}
```

### Phase 3: Migrate Package Managers

1. **yum.rs**: Remove block_in_place, use async directly
2. **apt.rs**: Add connection.execute() for remote
3. **dnf.rs**: Add connection.execute() for remote

### Phase 4: Migrate Remaining Modules

1. **service.rs**: Add connection.execute() for remote
2. **user.rs/group.rs**: Add connection.execute() for remote
3. **stat.rs**: Add connection.stat() for remote
4. **file.rs**: Add connection methods for remote

### Phase 5: Cleanup

1. Remove duplicate Module trait from traits.rs (or merge)
2. Remove all block_in_place and Runtime::new() patterns
3. Update documentation

---

## Detailed Changes Per Module

### copy.rs

```rust
// Current (blocking):
fn execute_remote(...) -> ModuleResult<ModuleOutput> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { ... })
}

// Proposed (async):
async fn execute_remote(
    &self,
    connection: Arc<dyn Connection + Send + Sync>,
    params: &ModuleParams,
    context: &ModuleContext,
) -> ModuleResult<ModuleOutput> {
    // Direct async calls
    let exists = connection.path_exists(remote_path).await?;
    connection.upload(local_path, remote_path, options).await?;
    ...
}
```

### lineinfile.rs

```rust
// Current (blocking):
fn execute_remote(...) -> ModuleResult<ModuleOutput> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { ... })
}

// Proposed (async):
async fn execute_remote(
    &self,
    connection: Arc<dyn Connection + Send + Sync>,
    ...
) -> ModuleResult<ModuleOutput> {
    let content = connection.download_content(remote_path).await?;
    // Process content
    connection.upload_content(&new_content, remote_path, options).await?;
}
```

### command.rs / shell.rs

```rust
// Current (local only):
fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
    let output = Command::new(&cmd).output()?;
    ...
}

// Proposed (local + remote):
async fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
    if let Some(ref connection) = context.connection {
        // Remote execution
        let result = connection.execute(&cmd, options).await?;
        ...
    } else {
        // Local execution (spawn_blocking for sync Command)
        let output = tokio::task::spawn_blocking(move || {
            Command::new(&cmd).output()
        }).await??;
        ...
    }
}
```

### yum.rs (and other package managers)

```rust
// Current (commented out due to API change):
return Err(ModuleError::Unsupported("Yum module needs rework..."));

// Proposed (fully async):
async fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
    let connection = context.connection.as_ref()
        .ok_or(ModuleError::ExecutionFailed("Connection required for yum module"))?;

    let options = Self::build_exec_options(context);

    // Check package status
    let result = connection.execute(&format!("rpm -q {}", package), Some(options.clone())).await?;

    // Install/remove as needed
    if !result.success {
        connection.execute(&format!("yum install -y {}", package), Some(options)).await?;
    }
}
```

---

## Module Trait Changes Summary

```rust
// Before (sync)
pub trait Module: Send + Sync {
    fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput>;
}

// After (async)
#[async_trait]
pub trait Module: Send + Sync {
    async fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput>;
}
```

### ModuleRegistry Changes

```rust
impl ModuleRegistry {
    // Before (sync)
    pub fn execute(&self, name: &str, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        ...
        module.execute(params, context)
    }

    // After (async)
    pub async fn execute(&self, name: &str, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        ...
        module.execute(params, context).await
    }
}
```

---

## Incremental Migration Path

The migration can be done incrementally:

1. **Week 1**: Convert Module trait to async, update executor
2. **Week 2**: Migrate copy.rs, lineinfile.rs, template.rs
3. **Week 3**: Migrate command.rs, shell.rs
4. **Week 4**: Migrate package managers (yum, apt, dnf)
5. **Week 5**: Migrate system modules (service, user, group)
6. **Week 6**: Migrate remaining modules, cleanup

Each module can be migrated independently as long as the trait is async.

---

## Testing Strategy

For each migrated module:

1. **Unit tests**: Verify local execution still works
2. **Integration tests**: Test with mock Connection implementation
3. **SSH tests**: Test against real SSH connections
4. **Check mode tests**: Verify check mode behavior

Create a test helper:
```rust
struct MockConnection {
    expected_commands: Vec<(String, CommandResult)>,
    expected_uploads: Vec<(PathBuf, Vec<u8>)>,
}

#[async_trait]
impl Connection for MockConnection {
    async fn execute(&self, command: &str, _options: Option<ExecuteOptions>) -> ConnectionResult<CommandResult> {
        // Return expected result
    }
}
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking existing playbooks | Low | High | Maintain backward compat for params |
| Performance regression | Medium | Medium | Benchmark before/after |
| russh compatibility issues | Medium | High | Test with russh early |
| Deadlocks with async | Medium | High | Avoid nested runtimes |

---

## Conclusion

The recommended approach is **Option A: Make Module Trait Async**. This provides:

1. Clean integration with russh async SSH
2. No blocking workarounds needed
3. Better performance through async I/O
4. Alignment with Rust async ecosystem

The migration can be done incrementally over 4-6 weeks, prioritizing high-usage modules first. The key changes are:

1. Add `#[async_trait]` to Module trait
2. Make `execute()`, `check()`, and `diff()` methods async
3. Update ModuleRegistry to call async methods
4. Convert each module to use `connection.method().await` instead of blocking patterns

The ModuleContext already has the connection field needed; modules just need to use it with async/await instead of blocking bridges.
