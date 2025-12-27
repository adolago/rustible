# Serial Execution Integration - Complete Implementation

## Overview

Serial execution has been successfully integrated into Rustible's executor, allowing for controlled batch-based execution of tasks across hosts. This feature is essential for rolling deployments and scenarios where you need to limit the number of hosts executing tasks simultaneously.

## Architecture

### Core Components

1. **SerialSpec Type** (`/home/artur/Repositories/rustible/src/playbook.rs:586-669`)
   - Defines three execution modes:
     - `Fixed(usize)`: Fixed batch size (e.g., serial: 2)
     - `Percentage(String)`: Percentage-based batches (e.g., serial: "50%")
     - `Progressive(Vec<SerialSpec>)`: Progressive batches (e.g., serial: [1, 5, 10])

2. **Play Configuration** (`/home/artur/Repositories/rustible/src/executor/playbook.rs:478-480`)
   - `serial: Option<SerialSpec>` - Serial specification for the play
   - `max_fail_percentage: Option<u8>` - Maximum allowed failure percentage
   - Integration with play parsing at lines 535-537

3. **Executor Integration** (`/home/artur/Repositories/rustible/src/executor/mod.rs`)
   - **Main integration point** (lines 252-262): Checks if play has serial spec
   - **run_serial method** (lines 406-501): Implements batch execution logic
   - Strategy-aware execution within batches

## Execution Flow

### 1. Play Execution Entry Point
```rust
// src/executor/mod.rs:252-262
let results = if let Some(ref serial_spec) = play.serial {
    self.run_serial(serial_spec, &hosts, &play.tasks, play.max_fail_percentage)
        .await?
} else {
    // Execute based on strategy without serial batching
    match self.config.strategy {
        ExecutionStrategy::Linear => self.run_linear(&hosts, &play.tasks).await?,
        ExecutionStrategy::Free => self.run_free(&hosts, &play.tasks).await?,
        ExecutionStrategy::HostPinned => self.run_host_pinned(&hosts, &play.tasks).await?,
    }
};
```

### 2. Serial Execution Logic
```rust
// src/executor/mod.rs:406-501
async fn run_serial(
    &self,
    serial_spec: &crate::playbook::SerialSpec,
    hosts: &[String],
    tasks: &[Task],
    max_fail_percentage: Option<u8>,
) -> ExecutorResult<HashMap<String, HostResult>>
```

**Key Features:**
- Splits hosts into batches using `serial_spec.batch_hosts(hosts)`
- Executes each batch sequentially
- Respects the configured execution strategy within each batch
- Tracks failure percentage across all batches
- Aborts execution if `max_fail_percentage` is exceeded
- Marks remaining hosts as skipped when aborting

### 3. Batch Calculation
```rust
// src/playbook.rs:640-668
pub fn batch_hosts<'a>(&self, hosts: &'a [String]) -> Vec<&'a [String]>
```

**Algorithm:**
- For `Fixed(n)`: Creates batches of size n
- For `Percentage(pct)`: Calculates batch size as percentage of total hosts
- For `Progressive([s1, s2, ...])`: Cycles through specs for each batch
- Handles edge cases (empty hosts, zero batch size, etc.)

## Configuration Examples

### YAML Playbook Configuration

```yaml
# Fixed batch size - 2 hosts at a time
- name: Rolling Update
  hosts: webservers
  serial: 2
  max_fail_percentage: 25
  tasks:
    - name: Update application
      command: /usr/local/bin/deploy.sh

# Percentage-based - 50% of hosts at a time
- name: Database Migration
  hosts: databases
  serial: "50%"
  tasks:
    - name: Run migration
      command: migrate.sh

# Progressive deployment - start small, scale up
- name: Canary Deployment
  hosts: production
  serial: [1, 5, 10]
  max_fail_percentage: 10
  tasks:
    - name: Deploy new version
      command: deploy.sh
```

### Programmatic Configuration

```rust
let mut play = Play::new("Rolling Update", "all");
play.serial = Some(SerialSpec::Fixed(2));
play.max_fail_percentage = Some(25);
```

## Failure Handling

### max_fail_percentage

When `max_fail_percentage` is set, the executor monitors the failure rate across all batches:

1. After each batch completes, calculate: `(total_failed / total_hosts) * 100`
2. If failure percentage exceeds threshold:
   - Log error message
   - Mark all remaining hosts as skipped
   - Abort further batch execution
   - Return accumulated results

**Example:**
```rust
// src/executor/mod.rs:464-492
if let Some(max_fail_pct) = max_fail_percentage {
    let current_fail_pct = (total_failed as f64 / total_hosts as f64 * 100.0) as u8;

    if current_fail_pct > max_fail_pct {
        error!(
            "Failure percentage ({:.1}%) exceeded max_fail_percentage ({}%), aborting remaining batches",
            current_fail_pct, max_fail_pct
        );

        // Mark remaining hosts as skipped
        for remaining_batch in batches.iter().skip(batch_idx + 1) {
            for host in remaining_batch.iter() {
                all_results.insert(
                    host.to_string(),
                    HostResult {
                        host: host.to_string(),
                        stats: ExecutionStats {
                            skipped: tasks.len(),
                            ..Default::default()
                        },
                        failed: false,
                        unreachable: false,
                    },
                );
            }
        }

        break;
    }
}
```

## Strategy Interaction

Serial execution respects the configured execution strategy **within each batch**:

- **Linear Strategy**: All hosts in batch execute task 1, then all execute task 2, etc.
- **Free Strategy**: Each host in batch runs all tasks independently
- **HostPinned Strategy**: Currently implemented as free (future enhancement)

```rust
// src/executor/mod.rs:444-448
let batch_results = match self.config.strategy {
    ExecutionStrategy::Linear => self.run_linear(&batch_hosts_owned, tasks).await?,
    ExecutionStrategy::Free => self.run_free(&batch_hosts_owned, tasks).await?,
    ExecutionStrategy::HostPinned => self.run_host_pinned(&batch_hosts_owned, tasks).await?,
};
```

## Testing Coverage

### Comprehensive Test Suite
Location: `/home/artur/Repositories/rustible/tests/serial_execution_tests.rs`

**29 passing tests covering:**

1. **Fixed Batch Sizes** (tests 34-128)
   - serial: 1 (one host at a time)
   - serial: 2 (batches of 2)
   - Batch size larger than total hosts

2. **Percentage-Based Batches** (tests 134-256)
   - serial: "50%" (half the hosts)
   - serial: "25%" (quarter of hosts)
   - serial: "100%" (all hosts)
   - Rounding behavior (30% of 5 hosts = 2)

3. **Progressive Batches** (tests 263-355)
   - serial: [1, 5, 10] (canary deployment)
   - serial: ["10%", "50%", "100%"] (percentage progression)

4. **Strategy Combinations** (tests 362-448)
   - Serial + Linear
   - Serial + Free
   - Serial + HostPinned

5. **Failure Handling** (tests 455-576)
   - max_fail_percentage not exceeded (continues)
   - max_fail_percentage exceeded (aborts)
   - max_fail_percentage: 0 (abort on first failure)

6. **Edge Cases** (tests 583-691)
   - Zero hosts
   - Single host
   - Batch size of 0
   - Percentage of 0%

7. **Complex Scenarios** (tests 698-818)
   - Rolling updates with handlers
   - Conditional tasks
   - Multiple plays with different serial specs

### Test Results
```bash
$ cargo test --test serial_execution_tests
running 29 tests
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Implementation Files

### Modified Files

1. **`/home/artur/Repositories/rustible/src/playbook.rs`**
   - Added `SerialSpec` enum (lines 586-669)
   - Implemented `calculate_batches()` method
   - Implemented `batch_hosts()` method

2. **`/home/artur/Repositories/rustible/src/executor/playbook.rs`**
   - Added `serial` field to `Play` struct (line 478)
   - Added `max_fail_percentage` field (line 480)
   - Integrated serial parsing in `Play::from_definition()` (lines 535-537)

3. **`/home/artur/Repositories/rustible/src/executor/mod.rs`**
   - Integrated serial check in `run_play()` (lines 252-262)
   - Implemented `run_serial()` method (lines 406-501)

### Test Files

1. **`/home/artur/Repositories/rustible/tests/serial_execution_tests.rs`**
   - Comprehensive test suite (897 lines)
   - 29 test cases covering all scenarios

## Usage Patterns

### Rolling Deployment Pattern
```yaml
- name: Rolling Application Update
  hosts: webservers
  serial: 2  # Update 2 servers at a time
  max_fail_percentage: 25  # Abort if more than 25% fail
  tasks:
    - name: Stop service
      service:
        name: myapp
        state: stopped

    - name: Update code
      git:
        repo: https://github.com/example/app.git
        dest: /var/www/myapp

    - name: Start service
      service:
        name: myapp
        state: started
      notify: verify deployment

  handlers:
    - name: verify deployment
      uri:
        url: http://localhost:8080/health
        status_code: 200
```

### Canary Deployment Pattern
```yaml
- name: Canary Deployment
  hosts: production
  serial: [1, 5, "10%"]  # 1 host, then 5, then 10% of remaining
  max_fail_percentage: 0  # Abort on any failure
  tasks:
    - name: Deploy canary
      command: /usr/local/bin/deploy-canary.sh
```

### Database Migration Pattern
```yaml
- name: Database Schema Update
  hosts: databases
  serial: 1  # One database at a time
  tasks:
    - name: Backup database
      command: pg_dump -Fc mydb > /backup/mydb.dump

    - name: Run migration
      command: /usr/local/bin/migrate.sh
```

## Performance Characteristics

### Execution Time
For N hosts with batch size B:
- Number of batches: ceil(N / B)
- Total time: sum(time_per_batch) + overhead
- Batch time depends on configured strategy

### Example: 100 hosts, serial: 10
- 10 batches of 10 hosts each
- If each batch takes 30 seconds: ~5 minutes total
- Linear strategy: all 10 hosts complete each task before next
- Free strategy: hosts in batch run independently

## Future Enhancements

### Potential Improvements

1. **Batch Timing Control**
   - Add `serial_delay` parameter for pause between batches
   - Useful for allowing systems to stabilize

2. **Dynamic Batch Sizing**
   - Adjust batch size based on success/failure rate
   - Slow down if failures increase

3. **Parallel Batch Execution**
   - Allow multiple batches to run in parallel with limits
   - E.g., serial: { size: 5, parallel: 2 }

4. **Enhanced HostPinned Strategy**
   - Implement true worker pinning for serial batches
   - Maintain host affinity across batches

5. **Batch-Level Hooks**
   - Pre-batch and post-batch hooks
   - Allow custom validation between batches

6. **Progress Reporting**
   - Real-time batch progress updates
   - Estimated time remaining

## Conclusion

Serial execution is fully integrated and production-ready. The implementation:

✅ **Complete**: All core functionality implemented
✅ **Tested**: 29 comprehensive tests passing
✅ **Documented**: Full API and usage documentation
✅ **Robust**: Handles edge cases and failures gracefully
✅ **Flexible**: Supports multiple batching strategies
✅ **Compatible**: Works with all execution strategies

The serial execution feature enables safe, controlled deployments and is ready for production use in Rustible.
