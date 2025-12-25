# Executor Integration Patch for Parallelization Enforcement

## Overview

This document provides the exact changes needed to integrate ParallelizationManager into the Executor.

## File Structure

```
src/executor/
├── mod.rs                      # Main executor - NEEDS MODIFICATION
├── parallelization.rs          # NEW FILE - Already created
├── playbook.rs                 # No changes needed
├── runtime.rs                  # No changes needed
└── task.rs                     # No changes needed

tests/
└── parallelization_enforcement_tests.rs  # NEW FILE - Already created

docs/
└── PARALLELIZATION_ENFORCEMENT.md        # NEW FILE - Already created
```

## Changes to `src/executor/mod.rs`

### Change 1: Add Module Declaration

**Location**: After line 9 (after existing module declarations)

```rust
pub mod parallelization;  // ADD THIS LINE
pub mod playbook;
pub mod runtime;
pub mod task;
```

### Change 2: Add Imports

**Location**: After line 24 (after existing use statements)

```rust
use crate::executor::parallelization::ParallelizationManager;  // ADD THIS
use crate::executor::playbook::{Play, Playbook};
use crate::executor::runtime::{ExecutionContext, RuntimeContext};
use crate::executor::task::{Handler, Task, TaskResult, TaskStatus};
use crate::modules::{ModuleRegistry, ParallelizationHint};      // ADD THIS
```

### Change 3: Update Executor Struct

**Location**: Lines 140-146 (Executor struct definition)

**Before**:
```rust
pub struct Executor {
    config: ExecutorConfig,
    runtime: Arc<RwLock<RuntimeContext>>,
    handlers: Arc<RwLock<HashMap<String, Handler>>>,
    notified_handlers: Arc<Mutex<HashSet<String>>>,
    semaphore: Arc<Semaphore>,
}
```

**After**:
```rust
pub struct Executor {
    config: ExecutorConfig,
    runtime: Arc<RwLock<RuntimeContext>>,
    handlers: Arc<RwLock<HashMap<String, Handler>>>,
    notified_handlers: Arc<Mutex<HashSet<String>>>,
    semaphore: Arc<Semaphore>,
    parallelization: Arc<ParallelizationManager>,     // ADD THIS
    module_registry: Arc<ModuleRegistry>,             // ADD THIS
}
```

### Change 4: Update `new()` Constructor

**Location**: Lines 149-159 (Executor::new implementation)

**Before**:
```rust
pub fn new(config: ExecutorConfig) -> Self {
    let forks = config.forks;
    Self {
        config,
        runtime: Arc::new(RwLock::new(RuntimeContext::new())),
        handlers: Arc::new(RwLock::new(HashMap::new())),
        notified_handlers: Arc::new(Mutex::new(HashSet::new())),
        semaphore: Arc::new(Semaphore::new(forks)),
    }
}
```

**After**:
```rust
pub fn new(config: ExecutorConfig) -> Self {
    let forks = config.forks;
    Self {
        config,
        runtime: Arc::new(RwLock::new(RuntimeContext::new())),
        handlers: Arc::new(RwLock::new(HashMap::new())),
        notified_handlers: Arc::new(Mutex::new(HashSet::new())),
        semaphore: Arc::new(Semaphore::new(forks)),
        parallelization: Arc::new(ParallelizationManager::new()),    // ADD THIS
        module_registry: Arc::new(ModuleRegistry::with_builtins()),  // ADD THIS
    }
}
```

### Change 5: Update `with_runtime()` Constructor

**Location**: Lines 162-170 (Executor::with_runtime implementation)

**Before**:
```rust
pub fn with_runtime(config: ExecutorConfig, runtime: RuntimeContext) -> Self {
    let forks = config.forks;
    Self {
        config,
        runtime: Arc::new(RwLock::new(runtime)),
        handlers: Arc::new(RwLock::new(HashMap::new())),
        notified_handlers: Arc::new(Mutex::new(HashSet::new())),
        semaphore: Arc::new(Semaphore::new(forks)),
    }
}
```

**After**:
```rust
pub fn with_runtime(config: ExecutorConfig, runtime: RuntimeContext) -> Self {
    let forks = config.forks;
    Self {
        config,
        runtime: Arc::new(RwLock::new(runtime)),
        handlers: Arc::new(RwLock::new(HashMap::new())),
        notified_handlers: Arc::new(Mutex::new(HashSet::new())),
        semaphore: Arc::new(Semaphore::new(forks)),
        parallelization: Arc::new(ParallelizationManager::new()),    // ADD THIS
        module_registry: Arc::new(ModuleRegistry::with_builtins()),  // ADD THIS
    }
}
```

### Change 6: Add Helper Method

**Location**: After line 171 (after `with_runtime()`, before `run_playbook()`)

```rust
/// Get the parallelization hint for a task's module
fn get_module_parallelization_hint(&self, module_name: &str) -> ParallelizationHint {
    self.module_registry
        .get(module_name)
        .map(|m| m.parallelization_hint())
        .unwrap_or(ParallelizationHint::FullyParallel)
}
```

### Change 7: Update `run_task_on_hosts()` Method

**Location**: Lines 506-565 (run_task_on_hosts implementation)

**Before** (line 510):
```rust
async fn run_task_on_hosts(
    &self,
    hosts: &[String],
    task: &Task,
) -> ExecutorResult<HashMap<String, TaskResult>> {
    debug!("Running task '{}' on {} hosts", task.name, hosts.len());

    let results = Arc::new(Mutex::new(HashMap::new()));
```

**After** (add after line 511):
```rust
async fn run_task_on_hosts(
    &self,
    hosts: &[String],
    task: &Task,
) -> ExecutorResult<HashMap<String, HostResult>> {
    debug!("Running task '{}' on {} hosts", task.name, hosts.len());

    // Get the parallelization hint for this module
    let hint = self.get_module_parallelization_hint(&task.module);
    debug!("Task '{}' parallelization hint: {:?}", task.name, hint);

    let results = Arc::new(Mutex::new(HashMap::new()));
```

**Before** (line 527):
```rust
tokio::spawn(async move {
    let _permit = semaphore.acquire().await.unwrap();

    let ctx = ExecutionContext::new(host.clone())
        .with_check_mode(config.check_mode)
        .with_diff_mode(config.diff_mode);
```

**After** (modify tokio::spawn section):
```rust
tokio::spawn(async move {
    // First acquire the general fork limit
    let _fork_permit = semaphore.acquire().await.unwrap();

    // Then acquire parallelization-specific constraints
    let _para_guard = parallelization
        .acquire(hint, &host, &module_name)
        .await;

    let ctx = ExecutionContext::new(host.clone())
        .with_check_mode(config.check_mode)
        .with_diff_mode(config.diff_mode);
```

**Required additions to tokio::spawn closure** (add these to the closure captures):
```rust
let parallelization = Arc::clone(&self.parallelization);  // ADD THIS before tokio::spawn
let module_name = task.module.clone();                     // ADD THIS before tokio::spawn
```

### Change 8: Add Accessor Method

**Location**: After line 692 (after `summarize_results()`, before tests module)

```rust
/// Get access to the parallelization manager for testing/debugging
pub fn parallelization(&self) -> &Arc<ParallelizationManager> {
    &self.parallelization
}
```

## Complete Modified `run_task_on_hosts()` Method

Here's the complete modified version for reference:

```rust
/// Run a single task on multiple hosts in parallel with parallelization enforcement
async fn run_task_on_hosts(
    &self,
    hosts: &[String],
    task: &Task,
) -> ExecutorResult<HashMap<String, TaskResult>> {
    debug!("Running task '{}' on {} hosts", task.name, hosts.len());

    // Get the parallelization hint for this module
    let hint = self.get_module_parallelization_hint(&task.module);
    debug!("Task '{}' parallelization hint: {:?}", task.name, hint);

    let results = Arc::new(Mutex::new(HashMap::new()));

    let handles: Vec<_> = hosts
        .iter()
        .map(|host| {
            let host = host.clone();
            let task = task.clone();
            let results = Arc::clone(&results);
            let semaphore = Arc::clone(&self.semaphore);
            let runtime = Arc::clone(&self.runtime);
            let config = self.config.clone();
            let handlers = Arc::clone(&self.handlers);
            let notified = Arc::clone(&self.notified_handlers);
            let parallelization = Arc::clone(&self.parallelization);  // NEW
            let module_name = task.module.clone();                     // NEW

            tokio::spawn(async move {
                // First acquire the general fork limit
                let _fork_permit = semaphore.acquire().await.unwrap();

                // Then acquire parallelization-specific constraints
                let _para_guard = parallelization
                    .acquire(hint, &host, &module_name)
                    .await;

                let ctx = ExecutionContext::new(host.clone())
                    .with_check_mode(config.check_mode)
                    .with_diff_mode(config.diff_mode);

                let result = task.execute(&ctx, &runtime, &handlers, &notified).await;

                match result {
                    Ok(task_result) => {
                        results.lock().await.insert(host, task_result);
                    }
                    Err(e) => {
                        error!("Task failed on host {}: {}", host, e);
                        results.lock().await.insert(
                            host,
                            TaskResult {
                                status: TaskStatus::Failed,
                                changed: false,
                                msg: Some(e.to_string()),
                                result: None,
                                diff: None,
                            },
                        );
                    }
                }
            })
        })
        .collect();

    join_all(handles).await;

    let results = Arc::try_unwrap(results)
        .map_err(|_| ExecutorError::RuntimeError("Failed to unwrap results".into()))?
        .into_inner();

    Ok(results)
}
```

## Testing the Integration

### Step 1: Run Unit Tests

```bash
cargo test --lib parallelization
```

Expected output: All ParallelizationManager tests pass.

### Step 2: Run Integration Tests

```bash
cargo test --test parallelization_enforcement_tests
```

Expected output: All enforcement tests pass with correct timing.

### Step 3: Build Project

```bash
cargo build --lib
```

Expected output: Clean build with no errors.

## Verification Checklist

- [ ] `src/executor/parallelization.rs` exists with 100% test coverage
- [ ] `tests/parallelization_enforcement_tests.rs` exists with comprehensive tests
- [ ] `src/executor/mod.rs` has all 8 changes applied
- [ ] Project compiles without errors
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Debug logging shows constraint acquisition/release

## Troubleshooting

### Issue: "cannot find module `parallelization`"
**Solution**: Ensure `pub mod parallelization;` is added to `src/executor/mod.rs`

### Issue: "cannot find type `ParallelizationManager`"
**Solution**: Ensure import statement is added: `use crate::executor::parallelization::ParallelizationManager;`

### Issue: Compilation errors in `run_task_on_hosts`
**Solution**: Ensure both `parallelization` and `module_name` are cloned before `tokio::spawn`

### Issue: Tests timing out
**Solution**: Check that guards are being properly dropped (not held across await points)

## Performance Validation

Run the benchmark to verify overhead is minimal:

```bash
cargo bench --bench execution_benchmark
```

Expected results:
- FullyParallel: < 1% overhead
- HostExclusive: < 5% overhead
- RateLimited: Depends on rate
- GlobalExclusive: < 5% overhead

## Next Steps

1. Apply all changes from this patch
2. Run full test suite: `cargo test`
3. Review debug logs: `RUST_LOG=debug cargo test`
4. Update ROADMAP.md to mark parallelization enforcement as complete
5. Consider additional enhancements (priority queues, adaptive hints, etc.)
