# SSH Library Comparison Benchmark Suite - Summary

## Overview

This directory contains a comprehensive benchmark suite comparing two SSH libraries for use in Rustible:

1. **ssh2** - C library (libssh2) wrapper, synchronous API
2. **async-ssh2-tokio (russh)** - Pure Rust, async-native implementation

## What's Included

### Main Benchmark (`src/main.rs`)

Comprehensive benchmark covering:

- **Connection Time**: SSH handshake and authentication performance
- **Command Execution (Reused)**: Performance on existing connections
- **Command Execution (New)**: Full connect-execute-disconnect cycle
- **File Transfer**: Upload and download performance
- **Parallel Execution**: Comparison of spawn_blocking (ssh2) vs native async (russh)

### Documentation

- **README.md**: Full documentation on running benchmarks
- **QUICKSTART.md**: Get started in 5 minutes
- **ANALYSIS.md**: Framework for analyzing and interpreting results
- **SUMMARY.md**: This file

### Helper Scripts

- **run_benchmarks.sh**: Convenient script for running various benchmark scenarios

## Quick Start

```bash
# Navigate to benchmark directory
cd benches/ssh_comparison

# Quick test (1 minute)
./run_benchmarks.sh -h your-server -u your-user quick

# Full benchmark (5-10 minutes)
./run_benchmarks.sh -h your-server -u your-user standard

# View results
ls -la results/
```

## Benchmark Categories

| Category | What It Measures | Why It Matters |
|----------|-----------------|----------------|
| Connection | Time to establish SSH connection | Initial playbook startup |
| Command (reused) | Execute on existing connection | Task throughput |
| Connect + Command | One-shot command execution | Ad-hoc operations |
| File Transfer | SFTP upload/download | Template deployment |
| Parallel 10x | 10 concurrent connections | Multi-host execution |

## Key Metrics

Each benchmark reports:

- **Mean**: Average time
- **Median (P50)**: Middle value
- **P95**: 95th percentile (tail latency)
- **P99**: 99th percentile (worst case)
- **Min/Max**: Best and worst times

## Expected Results

Based on architectural differences:

### russh Should Excel At:

- ✓ Parallel execution (native async)
- ✓ Integration with async runtime
- ✓ Command execution (no spawn_blocking overhead)
- ✓ Memory efficiency under load

### ssh2 Advantages:

- ✓ Maturity and battle-testing
- ✓ Known security track record
- ✓ Comprehensive SSH2 feature coverage

### Similar Performance:

- ~ Connection time (network-bound)
- ~ File transfer (I/O bound)

## Typical Use Cases

### Scenario 1: Multi-Host Playbook

**Importance**: HIGH for Rustible

A playbook targeting 100 hosts, each executing 10 tasks.

**Expected winner**: russh (native async scales better)

### Scenario 2: Single Host, Many Tasks

**Importance**: MEDIUM

One host, complex playbook with 100 tasks.

**Expected winner**: russh (slightly, due to connection reuse)

### Scenario 3: One-Off Commands

**Importance**: LOW

Ad-hoc command execution, new connection each time.

**Expected winner**: russh (async connection establishment)

### Scenario 4: Large File Transfers

**Importance**: MEDIUM

Deploying large artifacts or templates.

**Expected winner**: Similar (network/disk bound)

## Decision Criteria

For Rustible, the most important factors are:

1. **Parallel Execution Performance** (Weight: 40%)
   - Multi-host execution is core value proposition
   - russh should significantly outperform here

2. **Async Integration** (Weight: 25%)
   - Rustible is async-first
   - russh eliminates spawn_blocking overhead

3. **Reliability** (Weight: 20%)
   - Both must work correctly
   - ssh2 has longer track record

4. **Feature Coverage** (Weight: 10%)
   - Must support required SSH operations
   - Both support core features

5. **Maintainability** (Weight: 5%)
   - Pure Rust is easier to maintain
   - russh has advantage

## Running the Benchmarks

### Prerequisites

- SSH server with key-based auth
- Rust 1.75+
- ~10 minutes for full benchmark

### Basic Commands

```bash
# Build
cargo build --release

# Run with defaults
./target/release/ssh_bench

# Custom configuration
./target/release/ssh_bench \
  --host 192.168.1.100 \
  --user testuser \
  --iterations 100 \
  --verbose

# Skip slow file transfers
./target/release/ssh_bench --skip-file-transfer

# Or use the convenience script
./run_benchmarks.sh standard
```

### Benchmark Presets

```bash
# Quick (20 iterations, no file transfer) - 1 min
./run_benchmarks.sh quick

# Standard (100 iterations) - 5 min
./run_benchmarks.sh standard

# Thorough (500 iterations, large files) - 15 min
./run_benchmarks.sh thorough
```

## Interpreting Results

### Example Output

```
┌──────────────────┬──────────────────────┬───────────┬────────────┐
│ Benchmark        │ Library              │ Mean (ms) │ Median (ms)│
├──────────────────┼──────────────────────┼───────────┼────────────┤
│ Parallel 10x     │ ssh2 (spawn_blocking)│ 280.45    │ 275.30     │
│ Parallel 10x     │ russh (async)        │ 155.12    │ 152.80     │
└──────────────────┴──────────────────────┴───────────┴────────────┘
```

**Interpretation**: russh is 1.8x faster for parallel execution - this is significant!

### What to Look For

1. **Parallel execution**: russh should be 1.5-3x faster
2. **Command execution**: russh should be slightly faster or similar
3. **Connection time**: Should be similar (network-bound)
4. **Consistency**: Check P99 - should be close to median
5. **Outliers**: Large max values indicate performance issues

## Common Issues

### Connection Refused

```bash
# Test SSH connection manually
ssh -p 22 testuser@your-server "echo test"
```

### Authentication Failed

```bash
# Ensure key-based auth is set up
ssh-copy-id -i ~/.ssh/id_ed25519 testuser@your-server
```

### Slow Performance

- Check network latency: `ping your-server`
- Ensure no other heavy processes running
- Use local VM for consistent results

## File Structure

```
benches/ssh_comparison/
├── src/
│   └── main.rs              # Main benchmark implementation
├── results/                 # Benchmark results (gitignored)
│   └── benchmark_*.txt      # Timestamped result files
├── Cargo.toml               # Dependencies
├── README.md                # Full documentation
├── QUICKSTART.md            # 5-minute getting started
├── ANALYSIS.md              # Analysis framework
├── SUMMARY.md               # This file
└── run_benchmarks.sh        # Convenience script
```

## Implementation Notes

### File Transfer Benchmarks

Currently implemented using base64-encoded data over SSH commands rather than SFTP/SCP directly. This is:

- Simple and reliable
- Works with both libraries
- Focuses on SSH protocol performance
- Good enough for comparison purposes

For production use, native SFTP would be used.

### Parallel Execution

- **ssh2**: Uses `tokio::task::spawn_blocking` with thread pool
- **russh**: Uses `tokio::spawn` with native async

This is the most important architectural difference.

### Statistics

Uses `hdrhistogram` crate for accurate percentile calculations, which is industry-standard for latency measurement.

## Next Steps

After running benchmarks:

1. Review results table
2. Compare parallel execution performance (most important)
3. Check for any concerning outliers or variance
4. Read ANALYSIS.md for decision framework
5. Make recommendation: russh vs ssh2

## Contributing

To add new benchmarks:

1. Add benchmark function to `src/main.rs`
2. Create `BenchStats` instance
3. Run iterations and record durations
4. Push result to results vec

Example:
```rust
let mut stats = BenchStats::new("My Test", "ssh2");
for _ in 0..iterations {
    let duration = my_benchmark();
    stats.record(duration);
}
results.push(stats.to_result());
```

## License

Same as Rustible (MIT)

## Questions?

See:
- **README.md** for detailed usage
- **QUICKSTART.md** for quick setup
- **ANALYSIS.md** for result interpretation
- **Main Rustible docs** for project context
