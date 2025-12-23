# Output Buffering Implementation for Parallel Execution

## Problem
When tasks run in parallel across multiple hosts, output from different hosts can interleave, making it difficult to read. For example:
```
ok: [host1]
changed: [host3]
ok: [host2]
```

## Solution
Implement output buffering with ordered printing:

1. **Buffer results per host** - Collect all task results before printing
2. **Print in sorted order** - Display results sorted by hostname after all hosts complete
3. **Protect task_result() with Mutex** - Prevent interleaving even when buffering isn't used

## Implementation

### 1. Add BufferedTaskResult struct (src/cli/output.rs)

After the TaskStatus enum, add:

```rust
/// Buffered task result for deferred output
#[derive(Debug, Clone)]
pub struct BufferedTaskResult {
    pub host: String,
    pub status: TaskStatus,
    pub message: Option<String>,
}
```

### 2. Add output_lock to OutputFormatter (src/cli/output.rs)

Update imports to include Mutex:
```rust
use std::sync::{Arc, Mutex};
```

Add field to OutputFormatter struct:
```rust
pub struct OutputFormatter {
    // ... existing fields ...
    /// Mutex for synchronized output to prevent interleaving
    output_lock: Arc<Mutex<()>>,
}
```

Update constructor:
```rust
pub fn new(use_color: bool, json_mode: bool, verbosity: u8) -> Self {
    // ...
    Self {
        // ... existing fields ...
        output_lock: Arc::new(Mutex::new(())),
    }
}
```

### 3. Add synchronized output methods (src/cli/output.rs)

Update task_result() to use lock:
```rust
pub fn task_result(&self, host: &str, status: TaskStatus, message: Option<&str>) {
    // Acquire lock to ensure atomic output
    let _lock = self.output_lock.lock().unwrap();

    // ... existing implementation ...
}
```

Add new batch printing method:
```rust
/// Print multiple task results in a consistent order
/// This ensures that when tasks run in parallel, output appears sorted by host
pub fn task_results_batch(&self, results: &[BufferedTaskResult]) {
    // Acquire lock once for the entire batch
    let _lock = self.output_lock.lock().unwrap();

    // Sort results by host name for consistent output
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| a.host.cmp(&b.host));

    for result in sorted_results {
        // Print without acquiring the lock again (we already hold it for the batch)
        self.task_result_internal(&result.host, result.status, result.message.as_deref());
    }
}

/// Internal task result printing (assumes lock is already held)
fn task_result_internal(&self, host: &str, status: TaskStatus, message: Option<&str>) {
    // Same logic as task_result() but without the lock acquisition
    if self.json_mode {
        let result = serde_json::json!({
            "host": host,
            "status": status.as_str(),
            "message": message
        });
        println!("{}", serde_json::to_string(&result).unwrap());
        return;
    }

    let status_str = if self.use_color {
        status.colored_string()
    } else {
        status.as_str().to_string()
    };

    let host_str = if self.use_color {
        host.bright_white().bold().to_string()
    } else {
        host.to_string()
    };

    match status {
        TaskStatus::Ok
        | TaskStatus::Changed
        | TaskStatus::Skipped
        | TaskStatus::Rescued
        | TaskStatus::Ignored => {
            print!("{}: [{}]", status_str, host_str);
        }
        TaskStatus::Failed | TaskStatus::Unreachable => {
            print!("{}: [{}]", status_str, host_str);
        }
    }

    if let Some(msg) = message {
        print!(" => {}", msg);
    }

    println!();
}
```

### 4. Update execute_task in run.rs (src/cli/commands/run.rs)

The current code already collects results in parallel and displays them. Update the display section to use batch printing:

Find this section (around line 380-390):
```rust
// Collect results and display them
let results: Vec<_> = join_all(handles)
    .await
    .into_iter()
    .filter_map(|r| r.ok())
    .collect();

// Display results
for (host, status, message) in results {
    ctx.output.task_result(&host, status, message.as_deref());
}
```

Replace with:
```rust
// Collect results and display them
let results: Vec<_> = join_all(handles)
    .await
    .into_iter()
    .filter_map(|r| r.ok())
    .collect();

// Convert to BufferedTaskResult and display in sorted order
use crate::cli::output::BufferedTaskResult;
let buffered_results: Vec<BufferedTaskResult> = results
    .into_iter()
    .map(|(host, status, message)| BufferedTaskResult {
        host,
        status,
        message,
    })
    .collect();

// Display all results in a consistent order (sorted by host)
ctx.output.task_results_batch(&buffered_results);
```

## Benefits

1. **Ordered Output**: Results always appear sorted by hostname
2. **No Interleaving**: The Mutex ensures even individual task_result() calls don't interleave
3. **Parallel Execution**: Hosts still execute in parallel, only output is serialized
4. **Backward Compatible**: Existing code using task_result() directly still works

## Testing

Test with multiple hosts in parallel:
```yaml
- name: Test parallel output
  hosts: host1,host2,host3
  tasks:
    - name: Run command
      command: echo "hello from {{ inventory_hostname }}"
```

Expected output (sorted):
```
TASK [Run command] ****************************************************
changed: [host1]
changed: [host2]
changed: [host3]
```

Instead of potentially interleaved output.
