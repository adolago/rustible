# ADR-0003: Callback Plugin Architecture

## Status

Accepted

## Context

Callbacks are plugins that receive notifications about execution events. They enable:

1. Custom output formatting (JSON, YAML, progress bars, etc.)
2. Logging and audit trails
3. Metrics collection and monitoring
4. External integrations (webhooks, email, CI/CD)
5. Debugging and development aids

The callback system must be:
- Non-blocking for high performance
- Composable (multiple callbacks can run simultaneously)
- Thread-safe for parallel execution
- Easy to implement custom callbacks

## Decision

### ExecutionCallback Trait

```rust
#[async_trait]
pub trait ExecutionCallback: Send + Sync + Debug {
    // ========================================
    // Playbook Lifecycle
    // ========================================

    /// Called when playbook execution starts
    async fn on_playbook_start(&self, _playbook: &str) {}

    /// Called when playbook execution completes
    async fn on_playbook_complete(&self) {}

    // ========================================
    // Play Lifecycle
    // ========================================

    /// Called when a play starts
    async fn on_play_start(&self, _play: &str, _hosts: &[String]) {}

    /// Called when a play completes
    async fn on_play_complete(&self) {}

    // ========================================
    // Task Lifecycle
    // ========================================

    /// Called when a task starts on a host
    async fn on_task_start(&self, _task: &str, _host: &str) {}

    /// Called when a task completes
    async fn on_task_complete(&self, _result: &ExecutionResult) {}

    /// Called when a task is skipped
    async fn on_task_skipped(&self, _task: &str, _host: &str, _reason: &str) {}

    // ========================================
    // Host Events
    // ========================================

    /// Called when a host becomes unreachable
    async fn on_host_unreachable(&self, _host: &str, _error: &str) {}

    /// Called when a task succeeds on a host
    async fn on_host_ok(&self, _host: &str, _changed: bool) {}

    /// Called when a task fails on a host
    async fn on_host_failed(&self, _host: &str, _error: &str) {}

    // ========================================
    // Handler Events
    // ========================================

    /// Called when a handler is triggered
    async fn on_handler_triggered(&self, _handler: &str, _by_task: &str) {}

    /// Called when a handler completes
    async fn on_handler_complete(&self, _handler: &str, _result: &ExecutionResult) {}

    // ========================================
    // Statistics
    // ========================================

    /// Called with final statistics
    async fn on_stats(&self, _stats: &PlayStats) {}
}
```

### Callback Categories

#### 1. Core Output Callbacks

| Callback | Description |
|----------|-------------|
| `DefaultCallback` | Standard Ansible-like colored output |
| `MinimalCallback` | Only failures and recap |
| `SummaryCallback` | Silent execution, comprehensive summary at end |
| `NullCallback` | No output (for testing) |

#### 2. Visual Callbacks

| Callback | Description |
|----------|-------------|
| `ProgressCallback` | Progress bars and spinners |
| `DiffCallback` | Before/after diffs for changed files |
| `DenseCallback` | Compact single-line output |
| `OnelineCallback` | One line per task |
| `TreeCallback` | Tree-structured hierarchical output |

#### 3. Timing and Analysis

| Callback | Description |
|----------|-------------|
| `TimerCallback` | Execution timing with summary |
| `StatsCallback` | Detailed statistics collection |
| `ContextCallback` | Task context with variables/conditions |
| `CounterCallback` | Task counting and tracking |

#### 4. Filtering Callbacks

| Callback | Description |
|----------|-------------|
| `SelectiveCallback` | Filter by status, host, or patterns |
| `SkippyCallback` | Hide skipped tasks |
| `ActionableCallback` | Only changed/failed tasks |
| `FullSkipCallback` | Detailed skip analysis |

#### 5. Logging Callbacks

| Callback | Description |
|----------|-------------|
| `JsonCallback` | JSON-formatted output |
| `YamlCallback` | YAML-formatted output |
| `LogFileCallback` | File-based logging |
| `SyslogCallback` | System syslog integration |
| `DebugCallback` | Debug output for development |

#### 6. Integration Callbacks

| Callback | Description |
|----------|-------------|
| `JUnitCallback` | JUnit XML reports for CI/CD |
| `MailCallback` | Email notifications |
| `ForkedCallback` | Parallel execution output |

### CompositeCallback

Multiple callbacks can be combined:

```rust
pub struct CompositeCallback {
    callbacks: Vec<Box<dyn ExecutionCallback>>,
}

impl CompositeCallback {
    pub fn new() -> Self {
        Self { callbacks: Vec::new() }
    }

    pub fn with_callback(mut self, callback: Box<dyn ExecutionCallback>) -> Self {
        self.callbacks.push(callback);
        self
    }
}

#[async_trait]
impl ExecutionCallback for CompositeCallback {
    async fn on_task_complete(&self, result: &ExecutionResult) {
        // Fan out to all callbacks concurrently
        let futures: Vec<_> = self.callbacks
            .iter()
            .map(|cb| cb.on_task_complete(result))
            .collect();
        futures::future::join_all(futures).await;
    }
    // ... other methods
}
```

### Type Aliases

```rust
/// A boxed callback for dynamic dispatch
pub type BoxedCallback = Box<dyn ExecutionCallback>;

/// A shared callback wrapped in Arc for thread-safe shared ownership
pub type SharedCallback = Arc<dyn ExecutionCallback>;
```

### Builder Pattern

Callbacks use the builder pattern for configuration:

```rust
let callback = DefaultCallback::builder()
    .verbosity(Verbosity::Verbose)
    .show_task_path(true)
    .show_skipped(true)
    .use_color(true)
    .build();

let timer = TimerCallback::builder()
    .show_per_task(true)
    .show_per_host(true)
    .top_n_slowest(10)
    .build();
```

### Thread Safety

All callbacks must be `Send + Sync`:

```rust
#[derive(Debug)]
pub struct ThreadSafeCallback {
    // Use atomic types for counters
    task_count: AtomicUsize,
    // Use RwLock for mutable state
    results: RwLock<Vec<ExecutionResult>>,
    // Use Mutex for exclusive access
    file_handle: Mutex<File>,
}
```

### Error Handling

Callback errors are logged but don't stop execution:

```rust
async fn dispatch_event(&self, callback: &dyn ExecutionCallback, result: &ExecutionResult) {
    if let Err(e) = callback.on_task_complete(result).await {
        // Log error but continue execution
        log::warn!("Callback error: {}", e);
    }
}
```

### Prelude Module

Convenient imports for callback development:

```rust
pub mod prelude {
    // Core traits
    pub use crate::traits::{ExecutionCallback, ExecutionResult, ModuleResult};

    // All callback plugins
    pub use super::{
        DefaultCallback, MinimalCallback, SummaryCallback, NullCallback,
        ProgressCallback, DiffCallback, DenseCallback, OnelineCallback, TreeCallback,
        TimerCallback, StatsCallback, ContextCallback, CounterCallback,
        SelectiveCallback, SkippyCallback, ActionableCallback, FullSkipCallback,
        JsonCallback, YamlCallback, LogFileCallback, SyslogCallback, DebugCallback,
        JUnitCallback, MailCallback, ForkedCallback,
        CompositeCallback, BoxedCallback, SharedCallback,
    };

    // Common dependencies
    pub use async_trait::async_trait;
    pub use std::sync::Arc;
}
```

## Consequences

### Positive

1. **Flexibility**: Easy to add new callbacks without modifying core
2. **Composability**: Multiple callbacks can run simultaneously
3. **Performance**: Async callbacks don't block execution
4. **Testability**: NullCallback enables testing without output
5. **Integration**: Callbacks enable CI/CD, monitoring, alerting

### Negative

1. **Complexity**: Many callback options can be overwhelming
2. **Memory**: Each callback maintains its own state
3. **Ordering**: No guaranteed order of callback execution

### Mitigations

- Clear documentation with use-case recommendations
- Sensible defaults (DefaultCallback works out of the box)
- CompositeCallback for combining multiple plugins

## References

- Ansible Callback Plugins: https://docs.ansible.com/ansible/latest/plugins/callback.html
- Observer Pattern: https://refactoring.guru/design-patterns/observer
- Rust Async Trait: https://docs.rs/async-trait/
