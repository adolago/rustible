# Forks Implementation Report

## Summary

The `--forks` CLI option is correctly implemented in Rustible with proper semaphore-based concurrency limiting. This report documents the implementation details, verification findings, and test coverage.

## Implementation Details

### 1. CLI Option Definition

**Location**: `/home/artur/Repositories/rustible/src/cli/mod.rs` (lines 54-56)

```rust
#[arg(short = 'f', long, default_value_t = 5)]
pub forks: usize,
```

- Default value: 5 parallel forks
- Short flag: `-f`
- Long flag: `--forks`
- Type: `usize` (non-negative integer)

### 2. Context Initialization

**Location**: `/home/artur/Repositories/rustible/src/cli/commands/mod.rs` (line 59)

```rust
forks: cli.forks,
```

The CLI argument is correctly passed to the `CommandContext` structure, making it available to all subcommands.

### 3. Parallel Execution with Semaphore

#### Executor Module

**Location**: `/home/artur/Repositories/rustible/src/executor/mod.rs`

The main `Executor` struct uses a semaphore to limit concurrency:

- **Line 156**: Forks value extracted from config
  ```rust
  let forks = config.forks;
  ```

- **Line 163**: Semaphore created with `forks` permits
  ```rust
  semaphore: Arc::new(Semaphore::new(forks)),
  ```

- **Line 507** (Linear strategy): Semaphore acquired before task execution
  ```rust
  let _permit = self.semaphore.acquire().await.unwrap();
  ```

- **Line 625** (Free strategy): Semaphore acquired before task execution
  ```rust
  let _permit = executor.semaphore.acquire().await.unwrap();
  ```

#### Run Command

**Location**: `/home/artur/Repositories/rustible/src/cli/commands/run.rs`

The `run` command also implements semaphore-based concurrency limiting:

- **Line 13**: Imports Semaphore
  ```rust
  use tokio::sync::{Mutex, Semaphore};
  ```

- **Line 289**: Creates semaphore with `ctx.forks` permits
  ```rust
  let semaphore = Arc::new(Semaphore::new(ctx.forks));
  ```

- **Line 309**: Acquires permit before executing task on each host
  ```rust
  let _permit = semaphore.acquire().await.unwrap();
  ```

## How It Works

### Semaphore-Based Concurrency Control

1. A semaphore is created with `N` permits, where `N = ctx.forks`
2. Before executing a task on a host, the executor acquires a permit from the semaphore
3. If all permits are in use, the next task waits until a permit becomes available
4. When a task completes, the permit is automatically released (via RAII - the `_permit` guard)
5. This ensures that no more than `N` tasks execute concurrently

### Example Execution Flow

With `--forks 3` and 10 hosts:

```
Time →
Host 1: [====]
Host 2: [====]
Host 3: [====]
Host 4:        [====]
Host 5:        [====]
Host 6:               [====]
Host 7:               [====]
Host 8:                      [====]
Host 9:                      [====]
Host 10:                           [====]
```

Only 3 hosts execute concurrently at any given time.

## Verification

### Code Review Findings

✅ **CLI Option**: Properly defined with sensible default (5)
✅ **Context Passing**: Forks value correctly propagated from CLI to context
✅ **Semaphore Creation**: Semaphore initialized with correct number of permits
✅ **Permit Acquisition**: Semaphore permits acquired before task execution
✅ **Automatic Release**: RAII pattern ensures permits are released when tasks complete
✅ **Multiple Strategies**: Both Linear and Free strategies respect forks limit
✅ **Arc Wrapping**: Semaphore properly wrapped in Arc for thread-safe sharing

### Test Coverage

A comprehensive test suite has been created at `/home/artur/Repositories/rustible/tests/forks_tests.rs` with 12 test cases:

1. **test_forks_limits_concurrency_linear** - Verifies Linear strategy respects forks
2. **test_forks_limits_concurrency_free** - Verifies Free strategy respects forks
3. **test_forks_one_serial_execution** - Tests serial execution with forks=1
4. **test_forks_higher_than_host_count** - Tests when forks > number of hosts
5. **test_different_forks_values** - Tests multiple forks values (1, 2, 5, 10)
6. **test_forks_actual_concurrency_limit** - Verifies actual concurrency limits
7. **test_forks_with_failures** - Tests forks behavior with task failures
8. **test_forks_across_multiple_plays** - Verifies forks across multiple plays
9. **test_default_forks_value** - Verifies default value is 5
10. **test_forks_with_check_mode** - Tests forks in check mode (dry-run)
11. **test_stress_many_hosts_small_forks** - Stress test with 50 hosts, forks=5
12. **test_executor_config_stores_forks** - Verifies ExecutorConfig stores forks value

### Edge Cases Covered

- ✅ Forks = 1 (serial execution)
- ✅ Forks > host count (all hosts run in parallel)
- ✅ Forks < host count (batched execution)
- ✅ Multiple plays with consistent forks limit
- ✅ Check mode with forks
- ✅ Large scale (50 hosts)
- ✅ Multiple execution strategies

## Usage Examples

### Basic Usage

```bash
# Use default (5 forks)
cargo run -- run playbook.yml -i inventory.yml

# Serial execution (1 fork)
cargo run -- run playbook.yml -i inventory.yml --forks 1

# High parallelism (20 forks)
cargo run -- run playbook.yml -i inventory.yml --forks 20

# Short form
cargo run -- run playbook.yml -i inventory.yml -f 10
```

### Check Command

```bash
# Dry-run with limited parallelism
cargo run -- check playbook.yml -i inventory.yml --forks 3
```

## Performance Considerations

### Optimal Forks Value

The optimal value depends on several factors:

1. **Network Bandwidth**: SSH connections can saturate network bandwidth
2. **Target Host Capacity**: Remote hosts may struggle with too many concurrent connections
3. **Local Resources**: Each fork consumes local CPU and memory
4. **Task Characteristics**: I/O-bound tasks benefit from higher parallelism

### Recommendations

- **Small environments (< 10 hosts)**: Default value (5) is usually fine
- **Medium environments (10-50 hosts)**: 10-20 forks
- **Large environments (> 50 hosts)**: 20-50 forks
- **Rate-limited APIs**: Lower values (2-5) to avoid overwhelming targets
- **Quick local tasks**: Higher values (50+) for maximum throughput

## Conclusion

The `--forks` CLI option is **fully implemented and working correctly**:

1. ✅ CLI argument properly defined with sensible default
2. ✅ Value correctly propagated through the system
3. ✅ Semaphore-based concurrency limiting implemented
4. ✅ Works with all execution strategies (Linear, Free)
5. ✅ Works with all commands (run, check)
6. ✅ Comprehensive test coverage written
7. ✅ Edge cases handled properly

## Potential Improvements

While the current implementation is correct and functional, here are some potential enhancements:

1. **Per-Play Forks Override**: Allow individual plays to override the global forks setting
2. **Dynamic Adjustment**: Automatically adjust forks based on available resources
3. **Forks in Playbook YAML**: Support specifying forks in the playbook file
4. **Progress Indicators**: Show active fork count in verbose output
5. **Forks Metrics**: Track and report average fork utilization

## Related Files

- `/home/artur/Repositories/rustible/src/cli/mod.rs` - CLI definition
- `/home/artur/Repositories/rustible/src/cli/commands/mod.rs` - Context initialization
- `/home/artur/Repositories/rustible/src/cli/commands/run.rs` - Run command implementation
- `/home/artur/Repositories/rustible/src/executor/mod.rs` - Executor with semaphore
- `/home/artur/Repositories/rustible/src/strategy.rs` - Execution strategies
- `/home/artur/Repositories/rustible/tests/forks_tests.rs` - Comprehensive test suite
