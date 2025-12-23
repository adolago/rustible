# RecapStats and Result Aggregation Analysis

## Executive Summary

RecapStats is a non-thread-safe statistics collection mechanism in Rustible used to aggregate task execution results across all hosts. Currently, it stores task outcome tallies (ok, changed, failed, skipped, unreachable, rescued, ignored) per host and provides methods for recording individual task results and querying aggregated statistics.

**Current Status**: RecapStats is **NOT** designed for concurrent updates from multiple parallel tasks. It requires exclusive mutable access (`&mut self`), which makes it incompatible with parallel task execution without synchronization wrappers.

---

## 1. RecapStats Definition and Location

### File Location
- **Primary**: `/home/artur/Repositories/rustible/src/cli/output.rs` (lines 602-636)
- **Usage**: `/home/artur/Repositories/rustible/src/cli/commands/run.rs` (imported as `use crate::cli::output::{RecapStats, TaskStatus};`)

### Type Definition

```rust
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RecapStats {
    pub hosts: HashMap<String, HostStats>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct HostStats {
    pub ok: u32,
    pub changed: u32,
    pub unreachable: u32,
    pub failed: u32,
    pub skipped: u32,
    pub rescued: u32,
    pub ignored: u32,
}
```

### Key Characteristics
- **Serde Serialization**: Both structs derive `serde::Serialize`, enabling JSON output support
- **Cloneable**: Both derive `Clone`, indicating immutable data sharing via full copies
- **Mutable API**: The `record()` method requires exclusive mutable access
- **No Synchronization**: No built-in thread safety mechanisms (no `Mutex`, `RwLock`, or `Arc`)

---

## 2. How stats.record() Works

### Method Signature
**RecapStats::record()** (lines 614-620 in output.rs):
```rust
pub fn record(&mut self, host: &str, status: TaskStatus) {
    self.hosts
        .entry(host.to_string())
        .or_default()
        .record(status);
}
```

### Execution Flow

1. **Entry Creation**: Uses `HashMap::entry()` to get or create a `HostStats` for the given host
2. **Stat Increment**: Delegates to `HostStats::record()` to increment the appropriate counter
3. **Direct Mutation**: Updates happen in-place via `&mut self` reference

### HostStats::record() (lines 584-594 in output.rs):
```rust
pub fn record(&mut self, status: TaskStatus) {
    match status {
        TaskStatus::Ok => self.ok += 1,
        TaskStatus::Changed => self.changed += 1,
        TaskStatus::Skipped => self.skipped += 1,
        TaskStatus::Failed => self.failed += 1,
        TaskStatus::Unreachable => self.unreachable += 1,
        TaskStatus::Rescued => self.rescued += 1,
        TaskStatus::Ignored => self.ignored += 1,
    }
}
```

### Current Usage Pattern (in run.rs, lines 129-145)
```rust
let mut stats = RecapStats::new();

// Sequential execution: one mutable reference at a time
if let Some(plays) = playbook.as_sequence() {
    for play in plays {
        self.execute_play(ctx, play, &mut stats).await?;
    }
}

// At end of playbook
ctx.output.recap(&stats);
```

**Problem**: This pattern works for sequential execution but breaks with parallel task execution because multiple async tasks cannot hold exclusive mutable references simultaneously.

---

## 3. Safe Concurrent Updates from Parallel Tasks

### Current Executor Architecture

The executor in `/home/artur/Repositories/rustible/src/executor/mod.rs` already demonstrates proper concurrent patterns:

**Synchronization Primitives Used**:
```rust
pub struct Executor {
    runtime: Arc<RwLock<RuntimeContext>>,              // Shared read-write access
    handlers: Arc<RwLock<HashMap<String, Handler>>>,   // Shared handler registry
    notified_handlers: Arc<Mutex<HashSet<String>>>,    // Exclusive access for notifications
    connections: Arc<RwLock<HashMap<...>>>,            // Shared connection pool
}
```

### Issue with Current RecapStats

RecapStats is passed by mutable reference (`&mut RecapStats`), which is **incompatible with concurrent updates**:

```rust
// From run.rs, line 134
self.execute_play(ctx, play, &mut stats).await?;  // ❌ Cannot share &mut across async tasks
```

### Recommended Synchronization Approaches

#### Option 1: Arc<Mutex<RecapStats>> (Simplest)
**Pros**:
- Simple to implement
- Works with existing `record()` API
- Minimal performance impact for stat recording

**Cons**:
- Mutex contention under high concurrency
- Blocking operations in async code (requires `tokio::sync::Mutex`)

**Implementation Pattern**:
```rust
// In concurrent context
let stats = Arc::new(tokio::sync::Mutex::new(RecapStats::new()));

// Spawn parallel tasks
for host in hosts {
    let stats = Arc::clone(&stats);
    tokio::spawn(async move {
        // ... execute task ...
        stats.lock().await.record(&host, status);  // Safe concurrent access
    });
}

// Aggregate at end
let final_stats = Arc::try_unwrap(stats)
    .unwrap_or_else(|arc| (*arc).blocking_lock().clone())
    .into_inner();
```

#### Option 2: Arc<RwLock<RecapStats>> (Read-Heavy Optimized)
**Pros**:
- Better performance when mostly reading
- Allows multiple concurrent readers
- Consistent with other executor patterns

**Cons**:
- More overhead for write-heavy scenarios
- Still blocks on writes

**Implementation Pattern**:
```rust
let stats = Arc::new(RwLock::new(RecapStats::new()));

for host in hosts {
    let stats = Arc::clone(&stats);
    tokio::spawn(async move {
        // ... execute task ...
        stats.write().await.record(&host, status);  // Exclusive write access
    });
}
```

#### Option 3: Lock-Free AtomicU32 (Most Complex, Highest Performance)
**Pros**:
- Zero-contention for stat updates
- Best performance under high concurrency
- No allocations or context switches

**Cons**:
- Requires restructuring RecapStats entirely
- More complex implementation
- Requires careful ordering guarantees

**Pseudo-structure**:
```rust
pub struct HostAtomicStats {
    ok: AtomicU32,
    changed: AtomicU32,
    failed: AtomicU32,
    // ... etc
}

pub struct RecapStatsAtomic {
    hosts: Arc<DashMap<String, HostAtomicStats>>,  // Lock-free concurrent hashmap
}

// Usage: stats.record(&host, status)  // No locks needed
```

---

## 4. Current Synchronization in Rustible

### Executor Patterns (src/executor/mod.rs)

The codebase already uses synchronization extensively for parallel task execution:

#### 1. **Free Strategy** (lines 459-561)
```rust
// Pre-establish connections
let results = Arc::new(Mutex::new(HashMap::new()));

// Spawn parallel host tasks
let handles: Vec<_> = hosts.iter().map(|host| {
    let results = Arc::clone(&results);
    tokio::spawn(async move {
        // ... execute all tasks for this host ...
        results.lock().await.insert(host, host_result);  // ✓ Thread-safe
    })
}).collect();

join_all(handles).await;
```

#### 2. **Linear Strategy** (lines 406-456)
```rust
// Single-threaded accumulator (safe, no synchronization needed)
let mut results: HashMap<String, HostResult> = ...;

for task in tasks {
    let task_results = self.run_task_on_hosts(&active_hosts, task).await?;
    for (host, task_result) in task_results {
        if let Some(host_result) = results.get_mut(&host) {
            self.update_host_stats(host_result, &task_result);  // Safe sequential update
        }
    }
}
```

#### 3. **Task Execution** (lines 574-670)
```rust
// Results collected in Arc<Mutex<HashMap>>
let results = Arc::new(Mutex::new(HashMap::new()));

// Parallel task execution
for host in hosts {
    let results = Arc::clone(&results);
    tokio::spawn(async move {
        let result = task.execute(...).await;
        results.lock().await.insert(host, task_result);  // ✓ Thread-safe lock
    });
}
```

### Key Synchronization Patterns Used
1. **Arc<RwLock<T>>**: For shared mutable state with frequent reads
2. **Arc<Mutex<T>>**: For shared mutable state requiring exclusive access
3. **tokio::sync variants**: Async-aware synchronization (non-blocking)
4. **Semaphore**: For controlling concurrency (fork limit)

---

## 5. Recommended Solutions

### Short-term: Wrap RecapStats with Arc<tokio::sync::Mutex>

**Location to Update**: `/home/artur/Repositories/rustible/src/cli/commands/run.rs` (line 129)

```rust
// Before (sequential only)
let mut stats = RecapStats::new();

// After (supports parallel execution)
let stats = Arc::new(tokio::sync::Mutex::new(RecapStats::new()));
```

**Update recording calls**:
```rust
// Before
stats.record(host, TaskStatus::Ok);

// After
stats.lock().await.record(host, TaskStatus::Ok);
```

### Medium-term: Thread-safe API for RecapStats

Create a new wrapper type in `/home/artur/Repositories/rustible/src/cli/output.rs`:

```rust
/// Thread-safe wrapper for concurrent stat recording
pub struct ConcurrentRecapStats {
    inner: Arc<tokio::sync::Mutex<RecapStats>>,
}

impl ConcurrentRecapStats {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(RecapStats::new())),
        }
    }

    pub async fn record(&self, host: &str, status: TaskStatus) {
        self.inner.lock().await.record(host, status);
    }

    pub async fn into_inner(self) -> RecapStats {
        Arc::try_unwrap(self.inner)
            .map(|mutex| mutex.into_inner())
            .unwrap_or_else(|arc| {
                // Fallback for cloned instances
                (*arc).blocking_lock().clone()
            })
    }
}
```

### Long-term: Lock-free Stats with DashMap

For maximum performance in high-concurrency scenarios:

```rust
use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct HighPerformanceRecapStats {
    hosts: Arc<DashMap<String, HostAtomicStats>>,
}

pub struct HostAtomicStats {
    ok: AtomicU32,
    changed: AtomicU32,
    // ... etc
}

impl HighPerformanceRecapStats {
    pub fn record(&self, host: &str, status: TaskStatus) {
        // No awaits, no locks - fully non-blocking
        self.hosts
            .entry(host.to_string())
            .or_insert_with(HostAtomicStats::new)
            .record(status);
    }
}
```

---

## 6. Testing Concurrent Updates

### Current Test Coverage

Located in `/home/artur/Repositories/rustible/src/cli/output.rs` (lines 659-701):

```rust
#[test]
fn test_recap_stats() {
    let mut recap = RecapStats::new();
    recap.record("host1", TaskStatus::Ok);
    recap.record("host1", TaskStatus::Changed);
    recap.record("host2", TaskStatus::Failed);

    assert!(recap.has_failures());
    assert_eq!(recap.total_tasks(), 3);
}
```

### Required New Tests for Concurrency

```rust
#[tokio::test]
async fn test_concurrent_record() {
    let stats = Arc::new(tokio::sync::Mutex::new(RecapStats::new()));
    
    let mut handles = vec![];
    for i in 0..100 {
        let stats = Arc::clone(&stats);
        let handle = tokio::spawn(async move {
            stats.lock().await.record("host1", TaskStatus::Ok);
        });
        handles.push(handle);
    }
    
    for h in handles {
        h.await.unwrap();
    }
    
    let final_stats = stats.lock().await;
    assert_eq!(final_stats.hosts["host1"].ok, 100);
}
```

---

## 7. Impact Analysis

### Components Affected
1. **src/cli/commands/run.rs** - Primary API consumer
2. **src/cli/commands/check.rs** - May use stats (needs verification)
3. **src/executor/mod.rs** - Could integrate stats directly
4. **src/cli/output.rs** - RecapStats definition

### Migration Path
1. **Phase 1**: Wrap with `Arc<tokio::sync::Mutex>` at call sites
2. **Phase 2**: Create `ConcurrentRecapStats` wrapper for cleaner API
3. **Phase 3**: Migrate executor to use stats internally instead of `HostResult`
4. **Phase 4**: Benchmark and optimize to lock-free if needed

### Performance Considerations
- **Mutex overhead**: ~100-200ns per lock/unlock cycle
- **Expected impact**: <1% for typical playbooks (stats recording is not performance-critical)
- **Bottleneck**: Actually executing tasks (I/O), not recording results

---

## 8. Code References Summary

| Component | File | Lines | Purpose |
|-----------|------|-------|---------|
| RecapStats | `src/cli/output.rs` | 602-636 | Stats definition |
| HostStats | `src/cli/output.rs` | 565-600 | Per-host stats |
| TaskStatus enum | `src/cli/output.rs` | 12-57 | Status types |
| Run command usage | `src/cli/commands/run.rs` | 129-145, 261, 272, 293, etc. | Current usage |
| Executor patterns | `src/executor/mod.rs` | 459-561, 406-456, 574-670 | Sync patterns to follow |

---

## Conclusion

RecapStats requires explicit synchronization to support concurrent task updates. The most practical short-term solution is wrapping with `Arc<tokio::sync::Mutex>`, while a dedicated thread-safe wrapper type would provide better long-term maintainability. The executor already demonstrates best practices for concurrent patterns that should be applied to stats aggregation.

**Recommendation**: Implement Phase 1 (Arc<tokio::sync::Mutex> wrapper) immediately for compatibility with parallel execution strategies, then evaluate Phase 2-3 based on profiling results.
