# Robust Error Handling for Parallel Host Execution

## Implementation Summary

This document describes the robust error handling improvements implemented for parallel host execution in Rustible.

## Key Features

### 1. Parallel Execution with Fault Tolerance

When executing tasks across multiple hosts, the system now:
- **Continues execution on other hosts when one fails** - Individual host failures don't stop the entire task
- **Collects all errors** - All errors are aggregated and reported together at the end
- **Respects ignore_errors flag** - Tasks marked with `ignore_errors: true` will record the failure but continue execution

### 2. Error Collection Architecture

```rust
// Shared error collection using Arc<Mutex<>>
let results = Arc::new(Mutex::new(Vec::new()));
let errors = Arc::new(Mutex::new(Vec::new()));

// Each host execution records its result independently
match result {
    Ok(changed) => {
        results_guard.push((host, status, None));
    }
    Err(e) => {
        // Collect error but don't stop other hosts
        errors_guard.push((host.clone(), e.to_string()));

        let status = if ignore_errors {
            TaskStatus::Ignored
        } else {
            TaskStatus::Failed
        };

        results_guard.push((host, status, Some(e.to_string())));
    }
}
```

### 3. Parallelism Control

Execution is controlled via a semaphore to limit concurrent host operations:

```rust
// Create semaphore to limit concurrency
let semaphore = Arc::new(Semaphore::new(ctx.forks));

// Each task acquires a permit before executing
let _permit = semaphore.acquire().await.unwrap();
```

This ensures we don't overwhelm the system with too many simultaneous connections.

### 4. Error Reporting

After all hosts complete, errors are reported in a summary format:

```rust
if !errors.is_empty() && !ignore_errors {
    ctx.output.warning(&format!(
        "Task '{}' failed on {} host(s):",
        task_name,
        errors.len()
    ));
    for (host, error) in errors.iter() {
        ctx.output.debug(&format!("  [{}] {}", host, error));
    }
}
```

## Implementation Details

### File: `/home/artur/Repositories/rustible/src/cli/commands/run.rs`

#### Modified Method: `execute_task`

The `execute_task` method now implements robust parallel error handling:

```rust
async fn execute_task(
    &self,
    ctx: &mut CommandContext,
    task: &serde_yaml::Value,
    hosts: &[String],
    stats: &mut RecapStats,
) -> Result<()>
```

**Key improvements:**

1. **Parallel execution with error isolation**
   - Each host runs in its own `tokio::spawn` task
   - Failures on one host don't affect others
   - All results are collected after all hosts complete

2. **Error collection without interruption**
   - Errors are added to a shared `Arc<Mutex<Vec<>>>`
   - Execution continues even when errors occur
   - All errors are reported together at the end

3. **Support for ignore_errors**
   - Tasks can set `ignore_errors: true` in YAML
   - Failed tasks are marked as `Ignored` instead of `Failed`
   - Execution continues normally

#### New Method: `execute_module_parallel`

A new Send-safe version of module execution that can be used in `tokio::spawn`:

```rust
async fn execute_module_parallel(
    host: &str,
    task: &serde_yaml::Value,
    _inventory: Option<&std::path::Path>,
    _user: Option<&str>,
    _private_key: Option<&std::path::Path>,
) -> Result<bool>
```

This method doesn't require `&mut CommandContext` so it can be used in parallel execution contexts.

## Benefits

### 1. Improved Reliability
- No single host failure causes the entire task to fail
- Better visibility into which hosts succeeded and which failed
- Easier to debug issues across multiple hosts

### 2. Better Performance
- Hosts execute in parallel (limited by forks setting)
- Failures don't block other hosts from completing
- Faster overall playbook execution

### 3. Ansible Compatibility
- Matches Ansible's behavior for parallel execution
- Supports `ignore_errors` flag
- Provides similar error reporting

## Example Playbook

```yaml
---
- name: Test parallel execution with errors
  hosts: all
  tasks:
    - name: Task that might fail on some hosts
      command: /usr/bin/some-command
      ignore_errors: true  # Continue even if it fails

    - name: Critical task
      command: /usr/bin/critical-command
      # No ignore_errors - will mark host as failed but others continue
```

## Testing Recommendations

1. **Test with multiple hosts** - Verify that failure on one host doesn't stop others
2. **Test ignore_errors flag** - Verify that tasks marked with ignore_errors continue execution
3. **Test error reporting** - Verify that all errors are collected and displayed
4. **Test parallelism limits** - Verify that forks setting is respected
5. **Test mixed success/failure** - Verify that some hosts can succeed while others fail

## Future Enhancements

1. **Retry logic** - Add support for retrying failed tasks
2. **Failure thresholds** - Add `max_fail_percentage` like Ansible
3. **Host-level error handling** - Add per-host error handling strategies
4. **Detailed error categorization** - Distinguish between connection errors, execution errors, etc.
5. **Error recovery** - Add hooks for custom error recovery logic

## Migration Notes

### Breaking Changes
None - this is a transparent improvement to existing functionality.

### API Changes
- Added `execute_module_parallel` static method
- Added `detect_module_static` static method

These are internal implementation details and don't affect the public API.

## Performance Considerations

1. **Memory usage** - Error collection uses heap-allocated vectors
2. **Lock contention** - Results and errors use Mutex guards (minimal contention in practice)
3. **Semaphore overhead** - Minimal, controlled by forks setting

## Conclusion

The robust error handling implementation provides a solid foundation for parallel host execution that matches Ansible's behavior while leveraging Rust's safety guarantees and async capabilities. The system now gracefully handles failures across multiple hosts while maintaining execution speed and reliability.
