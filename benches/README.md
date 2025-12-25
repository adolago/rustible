# Rustible Performance Benchmarks

Comprehensive performance benchmark suite for Rustible, the async-first configuration management tool.

## Quick Start

```bash
# Run unified benchmark suite (recommended for regression testing)
cargo bench --bench unified_benchmark

# Run all benchmarks
./benches/scripts/run_all.sh

# Run with baseline comparison
cargo bench -- --baseline baseline
```

## Benchmark Suites

### Unified Benchmark (`unified_benchmark`) - Recommended

Quick regression testing with key benchmarks across all subsystems:

```bash
cargo bench --bench unified_benchmark
```

**Categories:**
- Playbook parsing (5/20/100 tasks)
- Inventory parsing (10/100/1000 hosts)
- Module execution (lookup, output, params)
- Connection pool (factory, local, stats)
- Template rendering (simple, medium, detection)
- Full playbook runs (simulated execution)
- Task operations (create, clone)

### Performance Benchmark (`performance_benchmark`)

Comprehensive core benchmarks:

```bash
cargo bench --bench performance_benchmark
```

**Categories:**
- Execution engine: Task latency, parallel hosts, scheduling
- Connection pool: Establishment, hit/miss, concurrency
- Template: Variable substitution, nested, large contexts
- Inventory: Parsing at scale, pattern matching, variable merging
- Module: Dispatch, parameters, serialization
- Playbook: Parsing, play construction

### Callback Benchmark (`callback_benchmark`)

Callback plugin system:

```bash
cargo bench --bench callback_benchmark
```

**Categories:**
- Dispatch overhead: NoOp, counting, buffering, JSON
- Multiple plugins: 1-20 concurrent callbacks
- Large event data: Payload sizes, serialization
- Memory usage: State accumulation

### Sprint 2 Features (`sprint2_feature_benchmark`)

Advanced features:

```bash
cargo bench --bench sprint2_feature_benchmark
```

**Categories:**
- Include tasks: Loading, nested includes
- Delegation: `delegate_to` overhead
- Serial/Free strategy: Batch calculation
- Plan mode: Execution planning
- Parallelization: Semaphores, token buckets

### SSH/Russh Benchmark (`russh_benchmark`)

SSH connection performance:

```bash
cargo bench --bench russh_benchmark
```

### Ansible Comparison (`comparison/`)

Direct comparison with Ansible:

```bash
cd benches/comparison
./run_benchmark.sh
./run_parallel_benchmark.sh
```

## Performance Targets

### Parsing

| Benchmark | Target | Description |
|-----------|--------|-------------|
| Playbook (5 tasks) | < 25us | Small playbook |
| Playbook (20 tasks) | < 100us | Medium playbook |
| Playbook (100 tasks) | < 500us | Large playbook |
| Inventory (10 hosts) | < 100us | Small inventory |
| Inventory (100 hosts) | < 1ms | Medium inventory |
| Inventory (1000 hosts) | < 10ms | Large inventory |

### Execution

| Benchmark | Target | Description |
|-----------|--------|-------------|
| Task creation | < 1us | Simple task |
| Task clone | < 1us | Task struct clone |
| Module lookup | < 100ns | Registry lookup |
| Module output | < 200ns | Result creation |
| Template render | < 10us | Simple substitution |

### Connection

| Benchmark | Target | Description |
|-----------|--------|-------------|
| Factory create | < 1us | Connection factory |
| Local connection | < 10us | localhost get |
| Pool stats | < 100ns | Statistics query |

## Running Specific Categories

```bash
# By benchmark name filter
cargo bench --bench unified_benchmark -- playbook
cargo bench --bench unified_benchmark -- inventory
cargo bench --bench unified_benchmark -- module
cargo bench --bench unified_benchmark -- connection
cargo bench --bench unified_benchmark -- template
cargo bench --bench unified_benchmark -- full_run
cargo bench --bench unified_benchmark -- task
```

## Viewing Results

### HTML Reports

```bash
# Linux
xdg-open target/criterion/report/index.html

# macOS
open target/criterion/report/index.html

# Windows
start target/criterion/report/index.html
```

### Text Summary

```bash
./benches/scripts/generate_report.sh benchmark_report.md
```

## Baseline Management

### Create Baseline

```bash
cargo bench -- --save-baseline baseline
```

### Compare Against Baseline

```bash
cargo bench -- --baseline baseline
```

### CI Baseline Comparison

```bash
./benches/scripts/compare_baselines.sh baseline current 10
```

## CI Integration

The `.github/workflows/benchmarks.yml` workflow:

1. Runs on push/PR to main
2. Compares against cached baseline
3. Detects regressions > 10%
4. Posts results to PR comments
5. Stores results as artifacts

### Manual Trigger

```bash
gh workflow run benchmarks.yml
```

## Profiling

### CPU Profiling (Linux)

```bash
# Using perf
cargo bench --bench unified_benchmark --no-run
perf record -g target/release/deps/unified_benchmark-*
perf report

# Using flamegraph
cargo install flamegraph
cargo flamegraph --bench unified_benchmark
```

### Memory Profiling

```bash
# Using valgrind
valgrind --tool=massif target/release/deps/unified_benchmark-*
ms_print massif.out.*

# Using heaptrack
heaptrack target/release/deps/unified_benchmark-*
```

## Ansible Comparison

### Setup

```bash
pip install ansible
# Configure benches/comparison/inventory.yml with your hosts
```

### Run

```bash
cd benches/comparison
RUNS=10 ./run_benchmark.sh
RUNS=10 ./run_parallel_benchmark.sh
```

### Typical Results

| Scenario | Rustible | Ansible | Speedup |
|----------|----------|---------|---------|
| Simple command | 0.5s | 2.1s | 4.2x |
| File operations | 0.8s | 3.5s | 4.4x |
| Multi-task | 1.2s | 5.8s | 4.8x |
| Parallel (5 forks) | 0.3s | 1.2s | 4.0x |

## Adding Benchmarks

### 1. Create Function

```rust
fn bench_my_feature(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_feature");

    group.bench_function("test_name", |b| {
        b.iter(|| {
            black_box(my_function())
        })
    });

    group.finish();
}
```

### 2. Register

```rust
criterion_group!(my_benches, bench_my_feature);
criterion_main!(/* ... */, my_benches);
```

### 3. Add to Cargo.toml

```toml
[[bench]]
name = "my_benchmark"
harness = false
```

## File Structure

```
benches/
|-- README.md                      # This file
|-- unified_benchmark.rs           # Quick regression testing
|-- performance_benchmark.rs       # Core performance
|-- callback_benchmark.rs          # Callback system
|-- sprint2_feature_benchmark.rs   # Sprint 2 features
|-- russh_benchmark.rs             # SSH benchmarks
|-- comparison/                    # Ansible comparison
|   |-- run_benchmark.sh
|   |-- run_parallel_benchmark.sh
|   |-- inventory.yml
|   +-- bench_*.yml
+-- scripts/                       # Automation
    |-- run_all.sh                 # Run all
    |-- generate_report.sh         # Generate report
    +-- compare_baselines.sh       # Compare baselines
```

## Best Practices

### Do

- Run on quiet system
- Use release builds (default)
- Compare against baselines
- Use `black_box()` to prevent optimization

### Don't

- Benchmark in debug mode
- Run with heavy background processes
- Compare across different machines
- Ignore statistical warnings

## Troubleshooting

### High Variance

```bash
# More warm-up
cargo bench -- --warm-up-time 10

# Pin CPU
taskset -c 0 cargo bench
```

### Out of Memory

```bash
# Reduce sample size
cargo bench -- --sample-size 20
```

## License

MIT (same as Rustible)
