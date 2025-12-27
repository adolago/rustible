# Parallelization Hint Enforcement in Rustible

## Overview

Rustible's executor enforces module parallelization hints to ensure safe concurrent execution and prevent resource contention. This document explains how the enforcement mechanism works and how to integrate it.

## Architecture

### Components

1. **ParallelizationManager** (`src/executor/parallelization.rs`)
   - Central coordinator for all parallelization constraints
   - Manages per-host semaphores, global locks, and rate limiters
   - Thread-safe and async-compatible

2. **Module Trait** (`src/modules/mod.rs`)
   - Each module declares its parallelization requirements via `parallelization_hint()`
   - Four hint types: `FullyParallel`, `HostExclusive`, `RateLimited`, `GlobalExclusive`

3. **Executor Integration** (`src/executor/mod.rs`)
   - Modified to consult parallelization hints before task execution
   - Acquires appropriate locks/permits before running module code
   - Automatically releases constraints when task completes

## Parallelization Hint Types

### 1. FullyParallel (Default)
```rust
fn parallelization_hint(&self) -> ParallelizationHint {
    ParallelizationHint::FullyParallel
}
```
- **Use case**: Stateless modules with no resource contention
- **Examples**: `debug`, `set_fact`, `assert`
- **Enforcement**: No restrictions - executes immediately
- **Behavior**: All instances can run simultaneously across all hosts

### 2. HostExclusive
```rust
fn parallelization_hint(&self) -> ParallelizationHint {
    ParallelizationHint::HostExclusive
}
```
- **Use case**: Modules that acquire host-level locks
- **Examples**: `apt`, `yum`, `dnf`, `package`
- **Enforcement**: Per-host semaphore (capacity = 1)
- **Behavior**: Only one instance per host at a time
- **Rationale**: Package managers use system-wide locks that prevent concurrent operations

### 3. RateLimited
```rust
fn parallelization_hint(&self) -> ParallelizationHint {
    ParallelizationHint::RateLimited {
        requests_per_second: 10,
    }
}
```
- **Use case**: API-calling modules with rate limits
- **Examples**: Cloud provider modules (AWS, Azure, GCP)
- **Enforcement**: Token bucket algorithm
- **Behavior**: Enforces maximum requests per second across all hosts
- **Rationale**: Prevents API rate limit errors and associated costs

### 4. GlobalExclusive
```rust
fn parallelization_hint(&self) -> ParallelizationHint {
    ParallelizationHint::GlobalExclusive
}
```
- **Use case**: Cluster-wide configuration changes
- **Examples**: Cluster membership changes, global state modifications
- **Enforcement**: Global semaphore (capacity = 1)
- **Behavior**: Only one instance across entire inventory
- **Rationale**: Prevents race conditions in distributed systems

## Implementation Details

### Token Bucket Algorithm (Rate Limiting)

The rate limiter uses a token bucket algorithm:

```rust
struct TokenBucket {
    capacity: u32,           // Maximum tokens
    tokens: f64,             // Current tokens
    refill_rate: f64,        // Tokens per second
    last_refill: Instant,    // Last refill time
}
```

**Algorithm**:
1. Refill tokens based on elapsed time: `tokens += elapsed_secs * refill_rate`
2. Cap tokens at capacity
3. If tokens >= 1.0, consume one token and proceed
4. Otherwise, wait until next token is available

**Example**: With 10 requests/second:
- Each token represents 100ms
- If 5 tokens consumed, wait 500ms for refill
- Smooth out bursts while maintaining average rate

### Per-Host Semaphores (HostExclusive)

```rust
host_semaphores: HashMap<String, Arc<Semaphore>>
```

**Algorithm**:
1. On first access to a host, create `Semaphore::new(1)`
2. Each task acquires the semaphore before executing
3. Semaphore automatically released when guard drops
4. Other tasks on same host wait in queue

### Global Mutex (GlobalExclusive)

```rust
global_mutex: Arc<Semaphore>  // capacity = 1
```

**Algorithm**:
1. Single global semaphore shared across all tasks
2. First task to acquire blocks all others
3. Tasks wait regardless of which host they target
4. Released when guard drops

## Executor Integration

### Modified `run_task_on_hosts` Function

```rust
async fn run_task_on_hosts(
    &self,
    hosts: &[String],
    task: &Task,
) -> ExecutorResult<HashMap<String, TaskResult>> {
    // Get the parallelization hint for this module
    let hint = self.get_module_parallelization_hint(&task.module);

    let handles: Vec<_> = hosts
        .iter()
        .map(|host| {
            tokio::spawn(async move {
                // 1. Acquire fork limit (overall concurrency)
                let _fork_permit = semaphore.acquire().await.unwrap();

                // 2. Acquire parallelization constraints
                let _para_guard = parallelization
                    .acquire(hint, &host, &module_name)
                    .await;

                // 3. Execute module
                let result = task.execute(&ctx, &runtime, &handlers, &notified).await;

                // 4. Guards automatically released when dropped
            })
        })
        .collect();

    join_all(handles).await;
}
```

### Key Integration Points

1. **Module Registry**: Executor holds `Arc<ModuleRegistry>` to look up hints
2. **Guard Lifecycle**: `ParallelizationGuard` holds permits until dropped
3. **Error Handling**: Failures don't leave locks held indefinitely
4. **Logging**: Debug logs show when constraints are acquired/released

## Testing Strategy

### Unit Tests (`src/executor/parallelization.rs`)

- `test_fully_parallel_no_blocking`: Verify no delays
- `test_host_exclusive_blocks_per_host`: Verify serialization per host
- `test_host_exclusive_different_hosts_parallel`: Verify independence across hosts
- `test_global_exclusive_blocks_all`: Verify global serialization
- `test_rate_limited_enforces_limit`: Verify rate limit compliance
- `test_token_bucket_refill`: Verify token bucket algorithm

### Integration Tests (`tests/parallelization_enforcement_tests.rs`)

- Mixed workloads with different hint types
- Real-world scenarios (apt on multiple hosts, cloud API calls)
- Performance verification (timing assertions)
- Stats tracking validation

## Performance Impact

### Overhead Analysis

1. **FullyParallel**: Near-zero overhead (immediate return)
2. **HostExclusive**: Minimal overhead (single semaphore acquire)
3. **RateLimited**: Small overhead (token bucket calculation)
4. **GlobalExclusive**: Minimal overhead (single semaphore acquire)

### Memory Usage

- Per-host semaphores: ~100 bytes per host
- Token buckets: ~64 bytes per module
- Global semaphore: ~64 bytes total
- **Total**: O(hosts + modules) memory, typically < 10KB for most playbooks

### Throughput Impact

| Hint Type | Same Host | Different Hosts | Across Inventory |
|-----------|-----------|-----------------|------------------|
| FullyParallel | No impact | No impact | No impact |
| HostExclusive | Serialized | Parallel | Parallel |
| RateLimited | Limited | Limited | Limited |
| GlobalExclusive | Serialized | Serialized | Serialized |

## Migration Guide

### Step 1: Add Parallelization Module

```rust
// src/executor/mod.rs
pub mod parallelization;

use crate::executor::parallelization::ParallelizationManager;
use crate::modules::{ModuleRegistry, ParallelizationHint};
```

### Step 2: Update Executor Struct

```rust
pub struct Executor {
    // ... existing fields ...
    parallelization: Arc<ParallelizationManager>,
    module_registry: Arc<ModuleRegistry>,
}
```

### Step 3: Initialize in Constructors

```rust
impl Executor {
    pub fn new(config: ExecutorConfig) -> Self {
        Self {
            // ... existing initialization ...
            parallelization: Arc::new(ParallelizationManager::new()),
            module_registry: Arc::new(ModuleRegistry::with_builtins()),
        }
    }
}
```

### Step 4: Add Helper Method

```rust
fn get_module_parallelization_hint(&self, module_name: &str) -> ParallelizationHint {
    self.module_registry
        .get(module_name)
        .map(|m| m.parallelization_hint())
        .unwrap_or(ParallelizationHint::FullyParallel)
}
```

### Step 5: Update `run_task_on_hosts`

Add parallelization guard acquisition before task execution (see code example above).

## Debugging and Monitoring

### Enable Debug Logging

```rust
RUST_LOG=rustible::executor::parallelization=debug cargo run
```

### Check Parallelization Stats

```rust
let stats = executor.parallelization().stats();
println!("Host locks: {:?}", stats.host_locks);
println!("Global available: {}", stats.global_available);
println!("Rate limiters: {:?}", stats.rate_limiter_states);
```

### Common Issues

1. **Deadlock**: Ensure guards are dropped (not held across await points)
2. **Starvation**: Check rate limits aren't too restrictive
3. **Performance**: Verify hints match actual module behavior

## Future Enhancements

1. **Dynamic Rate Limiting**: Adjust rates based on API responses
2. **Priority Queues**: High-priority tasks jump queue
3. **Adaptive Hints**: Learn optimal hints from execution patterns
4. **Distributed Coordination**: Share state across executor instances
5. **Metrics Export**: Prometheus/OpenTelemetry integration

## References

- Module Trait: `src/modules/mod.rs:454-481`
- ParallelizationManager: `src/executor/parallelization.rs`
- Integration Tests: `tests/parallelization_enforcement_tests.rs`
- Example Modules: `src/modules/apt.rs:210`, `src/modules/package.rs:245`
