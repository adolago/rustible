# Rustible Performance Benchmarks

This directory contains comprehensive performance benchmarks for the Rustible automation engine.

## Overview

The benchmarks are designed to measure performance across all critical components of Rustible:

1. **Playbook Parsing** - YAML parsing and deserialization performance
2. **Inventory Management** - Host and group loading at various scales
3. **Template Rendering** - Jinja2-compatible template engine performance
4. **Variable Resolution** - Variable hierarchy and merging operations
5. **Connection Pooling** - Connection management and reuse efficiency
6. **Task Execution** - Task parsing and execution overhead
7. **Handler Notifications** - Handler lookup and notification tracking
8. **Parallel Execution** - Async task spawning and parallel scaling
9. **Memory Allocation** - Memory usage patterns and data structure cloning

## Running Benchmarks

### Run All Benchmarks

```bash
cargo bench --bench execution_benchmark
```

### Run Specific Benchmark Groups

```bash
# Playbook parsing benchmarks
cargo bench --bench execution_benchmark playbook

# Inventory benchmarks
cargo bench --bench execution_benchmark inventory

# Template rendering benchmarks
cargo bench --bench execution_benchmark template

# Variable resolution benchmarks
cargo bench --bench execution_benchmark variable

# Connection pool benchmarks
cargo bench --bench execution_benchmark connection

# Task execution benchmarks
cargo bench --bench execution_benchmark task

# Handler benchmarks
cargo bench --bench execution_benchmark handler

# Parallel execution benchmarks
cargo bench --bench execution_benchmark parallel

# Memory allocation benchmarks
cargo bench --bench execution_benchmark memory
```

### Run Individual Benchmarks

```bash
# Simple playbook parsing
cargo bench --bench execution_benchmark playbook_parse_simple

# Complex playbook parsing
cargo bench --bench execution_benchmark playbook_parse_complex

# YAML inventory parsing with different sizes
cargo bench --bench execution_benchmark inventory_parse_yaml

# Pattern matching
cargo bench --bench execution_benchmark inventory_pattern_matching

# Template rendering
cargo bench --bench execution_benchmark template_render

# Connection pool operations
cargo bench --bench execution_benchmark connection_pool
```

## Benchmark Details

### 1. Playbook Parsing Benchmarks

- **playbook_parse_simple**: Parses a simple 3-task playbook
- **playbook_parse_complex**: Parses a complex multi-play playbook with roles, handlers, loops, and conditionals
- **playbook_parse_complex_repeated**: Repeated parsing to test caching/optimization opportunities

### 2. Inventory Parsing Benchmarks

- **inventory_parse_yaml**: Parses YAML inventory files at 3 scales (10, 100, 1000 hosts)
- **inventory_parse_ini**: Parses INI format inventory files at 3 scales
- **inventory_pattern_matching**: Tests various host pattern matching strategies:
  - All hosts (`all`)
  - Single group (`webservers`)
  - Wildcard patterns (`web*`)
  - Regex patterns (`~web\d+`)
  - Union (`webservers:databases`)
  - Intersection (`webservers:&databases`)
  - Exclusion (`all:!databases`)

### 3. Template Rendering Benchmarks

- **template_render_simple**: Simple variable interpolation
- **template_render_complex**: Complex templates with loops, nested variables
- **template_variable_interpolation**: Variable interpolation scaling (1, 10, 50, 100 variables)

### 4. Variable Resolution Benchmarks

- **get_host_vars**: Measures variable resolution with group hierarchy
- **get_host_group_hierarchy**: Measures group hierarchy traversal
- **variable_merging**: Tests variable merging at different scales (10-500 variables)

### 5. Connection Pool Benchmarks

- **factory_create**: Connection factory creation overhead
- **get_local_connection**: Getting a local connection from the pool
- **connection_reuse**: Tests connection reuse from pool
- **connection_pool_scaling**: Tests pool behavior with different pool sizes (1, 5, 10)

### 6. Task Execution Benchmarks

- **task_parsing**: Task definition parsing overhead
- **task_clone**: Task structure cloning performance (important for parallel execution)

### 7. Handler Notification Benchmarks

- **handler_lookup**: Handler lookup by name or listener
- **notification_tracking**: Notification set management

### 8. Parallel Execution Benchmarks

- **parallel_execution_scaling**: Measures parallel task execution scaling (1-20 forks)
- **async_task_spawning**: Measures async task spawning overhead (10-500 tasks)

### 9. Memory Allocation Benchmarks

- **inventory_small_alloc**: Small inventory allocation (10 hosts)
- **inventory_large_alloc**: Large inventory allocation (1000 hosts)
- **playbook_alloc**: Playbook structure allocation
- **variables_alloc**: Variable storage allocation (100 variables)
- **playbook_clone**: Playbook structure cloning
- **inventory_clone**: Inventory structure cloning

## Interpreting Results

Criterion produces detailed statistical analysis of each benchmark, including:

- **Time**: Mean execution time with confidence intervals
- **Throughput**: Operations per second (where applicable)
- **Change**: Comparison with previous runs (if available)

Results are saved in `target/criterion/` with HTML reports viewable in a browser.

### Viewing HTML Reports

```bash
# Open the criterion report index
open target/criterion/report/index.html  # macOS
xdg-open target/criterion/report/index.html  # Linux
start target/criterion/report/index.html  # Windows
```

## Performance Targets

Expected performance characteristics (as of initial implementation):

| Benchmark | Target | Notes |
|-----------|--------|-------|
| Simple playbook parse | < 50µs | 3 tasks |
| Complex playbook parse | < 500µs | ~20 tasks, roles, handlers |
| Small inventory (10 hosts) | < 100µs | YAML format |
| Medium inventory (100 hosts) | < 1ms | YAML format |
| Large inventory (1000 hosts) | < 10ms | YAML format |
| Simple template render | < 10µs | 2 variables |
| Complex template render | < 100µs | Loops, nested vars |
| Variable resolution | < 5µs | 3-level hierarchy |
| Connection pool get | < 1µs | Pool hit |
| Task clone | < 1µs | Standard task |

## Continuous Performance Monitoring

These benchmarks should be run:

1. **Before major refactorings** - Establish baseline
2. **After optimization work** - Verify improvements
3. **During PR reviews** - Catch performance regressions
4. **Periodically** - Monitor long-term trends

## Adding New Benchmarks

To add a new benchmark:

1. Create a benchmark function following the pattern:
   ```rust
   fn bench_my_feature(c: &mut Criterion) {
       let mut group = c.benchmark_group("my_feature");

       group.bench_function("my_test", |b| {
           b.iter(|| {
               // Code to benchmark
               black_box(result)
           })
       });

       group.finish();
   }
   ```

2. Add it to the appropriate criterion_group! at the bottom of the file

3. Run and verify:
   ```bash
   cargo bench --bench execution_benchmark my_feature
   ```

## Profiling Integration

For deeper performance analysis, use these tools with the benchmarks:

### CPU Profiling (Linux)

```bash
# Using perf
cargo bench --bench execution_benchmark --no-run
perf record -g target/release/deps/execution_benchmark-* --bench
perf report

# Using flamegraph
cargo install flamegraph
cargo flamegraph --bench execution_benchmark
```

### Memory Profiling

```bash
# Using valgrind/massif
cargo bench --bench execution_benchmark --no-run
valgrind --tool=massif target/release/deps/execution_benchmark-* --bench
ms_print massif.out.*

# Using heaptrack (Linux)
heaptrack target/release/deps/execution_benchmark-* --bench
```

## CI Integration

Recommended CI integration:

```yaml
# .github/workflows/benchmarks.yml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run benchmarks
        run: cargo bench --bench execution_benchmark
      - name: Store results
        uses: actions/upload-artifact@v3
        with:
          name: criterion-results
          path: target/criterion
```

## Rustible vs Ansible Comparison Benchmarks

The `comparison/` directory contains benchmarks that directly compare Rustible against Ansible on real infrastructure.

### Setup

These benchmarks require:
- 5 LXC containers or VMs accessible via SSH
- Ansible installed on the host system
- SSH keys configured for passwordless authentication

The inventory is configured in `comparison/inventory.yml`.

### Available Comparison Benchmarks

#### Standard Feature Benchmarks

Run the standard comparison benchmarks:

```bash
cd benches/comparison
./run_benchmark.sh
```

This runs multiple playbooks testing different features:
- `bench_01_simple.yml`: Basic command execution
- `bench_02_file_ops.yml`: File operations
- `bench_03_multi_task.yml`: Multi-task workflows
- `bench_04_comprehensive.yml`: Comprehensive feature test

Results are saved to `comparison/results/` with timestamp-based filenames.

#### Parallel Host Execution Benchmark

Run the parallel execution benchmark to test how Rustible and Ansible scale with different parallelism levels:

```bash
cd benches/comparison
./run_parallel_benchmark.sh
```

This benchmark:
- Runs `bench_parallel_hosts.yml` (20 simple commands across 5 hosts)
- Tests three fork/parallelism levels:
  - `forks=1`: Sequential execution (one host at a time)
  - `forks=2`: Partial parallel (2 hosts at a time)
  - `forks=5`: Full parallel (all 5 hosts simultaneously)
- Compares execution time and efficiency for each configuration
- Calculates speedup vs sequential baseline
- Shows parallel scaling efficiency

The playbook is designed with many quick commands to demonstrate the performance benefits of parallel execution.

**Understanding the Results:**

- **Speedup**: How much faster compared to sequential (forks=1)
- **Efficiency**: How well parallelism is utilized (ideal is 100%)
  - Calculated as: (speedup / forks) * 100%
  - Example: 2x speedup with forks=5 = 40% efficiency
- **Ideal scaling**: forks=5 should be ~5x faster than forks=1

Results show:
1. Raw execution times for each fork level
2. Speedup compared to sequential baseline
3. Parallel efficiency metrics
4. Rustible vs Ansible comparison at each parallelism level

### Customizing Benchmarks

You can customize the number of runs:

```bash
# Run with 10 iterations instead of default 5
RUNS=10 ./run_benchmark.sh
RUNS=10 ./run_parallel_benchmark.sh
```

### Interpreting Comparison Results

The comparison benchmarks provide:

1. **Raw timing data**: CSV files with all run times
2. **Summary reports**: Text files with averages and speedup calculations
3. **Speedup metrics**: How much faster Rustible is compared to Ansible

Typical results show Rustible being 2-5x faster than Ansible for equivalent operations, with better scaling as workload complexity increases.

## License

Same as Rustible project (MIT).
