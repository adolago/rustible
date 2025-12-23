# SSH Library Comparison Benchmarks

Comprehensive performance comparison between `ssh2` (libssh2 wrapper) and `async-ssh2-tokio` (russh - pure Rust async).

## Overview

This benchmark suite compares two SSH libraries used in Rustible:

- **ssh2**: C library (libssh2) wrapper, synchronous, requires `spawn_blocking` for async contexts
- **async-ssh2-tokio (russh)**: Pure Rust, async-native SSH implementation

## Benchmarks Included

### 1. Connection Time
- Measures time to establish SSH connection and authenticate
- Tests both libraries with identical connection parameters

### 2. Command Execution (Reused Connection)
- Measures command execution time on an already-established connection
- Tests throughput when connection overhead is eliminated

### 3. Command Execution (New Connection)
- Measures total time for connect + authenticate + execute + disconnect
- Simulates one-off command scenarios

### 4. File Transfer
- **Upload**: Measures SFTP upload performance
- **Download**: Measures SFTP download performance
- Configurable file size (default 100 KB)

### 5. Parallel Execution
- **ssh2**: Uses `spawn_blocking` with thread pool
- **russh**: Native async with tokio tasks
- Compares scalability of both approaches

## Prerequisites

### SSH Server Setup

You need a reachable SSH server with key-based authentication. The default configuration expects:

- Host: `192.168.178.141`
- Port: `22`
- User: `testuser`
- Key: `~/.ssh/id_ed25519`

### Setting Up Test Environment

If you need to set up a test SSH server:

```bash
# Using Docker
docker run -d \
  -p 2222:22 \
  -e PUID=1000 \
  -e PGID=1000 \
  -e USER_NAME=testuser \
  -e PUBLIC_KEY="$(cat ~/.ssh/id_ed25519.pub)" \
  linuxserver/openssh-server

# Then use --host localhost --port 2222
```

Or use a VM/remote server with SSH enabled and your public key in `~/.ssh/authorized_keys`.

## Building

```bash
cd benches/ssh_comparison
cargo build --release
```

## Running Benchmarks

### Basic Usage

```bash
# Run with defaults (requires SSH server at 192.168.178.141)
cargo run --release

# Or directly
./target/release/ssh_bench
```

### Custom Configuration

```bash
# Specify custom host/port/user
cargo run --release -- \
  --host 192.168.1.100 \
  --port 22 \
  --user myuser \
  --key ~/.ssh/id_rsa

# Adjust iterations and file size
cargo run --release -- \
  --iterations 50 \
  --file-size-kb 500

# Skip file transfer benchmarks (faster)
cargo run --release -- --skip-file-transfer

# Verbose output with progress indicators
cargo run --release -- --verbose
```

### All Options

```
OPTIONS:
  -H, --host <HOST>              Host to connect to [default: 192.168.178.141]
  -p, --port <PORT>              SSH port [default: 22]
  -u, --user <USER>              SSH user [default: testuser]
  -k, --key <KEY>                SSH key file [default: ~/.ssh/id_ed25519]
  -i, --iterations <ITERATIONS>  Number of iterations [default: 100]
  -f, --file-size-kb <SIZE>      File size in KB [default: 100]
  --skip-file-transfer           Skip file transfer benchmarks
  -v, --verbose                  Output detailed statistics
  -h, --help                     Print help
```

## Output

The benchmark produces a comprehensive table with statistics:

- **Mean**: Average execution time
- **Median**: 50th percentile (P50)
- **P95**: 95th percentile latency
- **P99**: 99th percentile latency
- **Min/Max**: Best and worst case times

Example output:

```
╔═══════════════════════════════════════════════════════════════╗
║                     BENCHMARK RESULTS                         ║
╚═══════════════════════════════════════════════════════════════╝

┌──────────────────┬──────────────────────┬───────────┬────────────┬──────────┬──────────┬──────────┬──────────┐
│ Benchmark        │ Library              │ Mean (ms) │ Median (ms)│ P95 (ms) │ P99 (ms) │ Min (ms) │ Max (ms) │
├──────────────────┼──────────────────────┼───────────┼────────────┼──────────┼──────────┼──────────┼──────────┤
│ Connection       │ ssh2                 │ 45.23     │ 44.10      │ 52.30    │ 58.10    │ 42.00    │ 65.20    │
│ Connection       │ russh                │ 38.12     │ 37.50      │ 43.20    │ 47.80    │ 35.10    │ 52.30    │
│ Command (reused) │ ssh2                 │ 2.15      │ 2.10       │ 2.50     │ 3.10     │ 1.95     │ 4.20     │
│ Command (reused) │ russh                │ 1.85      │ 1.80       │ 2.20     │ 2.60     │ 1.70     │ 3.50     │
│ ...              │ ...                  │ ...       │ ...        │ ...      │ ...      │ ...      │ ...      │
└──────────────────┴──────────────────────┴───────────┴────────────┴──────────┴──────────┴──────────┴──────────┘
```

## Understanding Results

### Lower is Better
All timings represent latency - lower values indicate better performance.

### Percentiles (P95/P99)
- **P95**: 95% of operations complete within this time
- **P99**: 99% of operations complete within this time
- Higher percentiles reveal tail latency behavior

### Key Comparisons

1. **Connection Time**: How fast can we establish authenticated SSH sessions?
2. **Command Reuse**: Once connected, how efficient is command execution?
3. **Parallel Performance**: How do the libraries scale with concurrent operations?
4. **File Transfer**: Which implementation has better SFTP throughput?

## Expected Outcomes

Generally, you should observe:

- **russh** performs better in async contexts due to native async support
- **ssh2** requires `spawn_blocking`, adding thread pool overhead
- **Connection pooling** significantly improves performance for both
- **Parallel operations** heavily favor russh's native async model
- **File transfer** performance may be similar (depends on SFTP implementation)

## Troubleshooting

### Connection Refused

```
Error: connection refused
```

**Solution**: Ensure SSH server is running and reachable:
```bash
ssh testuser@192.168.178.141 "echo test"
```

### Authentication Failed

```
Error: authentication failed
```

**Solution**: Verify key-based auth:
```bash
# Ensure your public key is in authorized_keys on the server
ssh-copy-id -i ~/.ssh/id_ed25519.pub testuser@192.168.178.141
```

### Permission Denied (File Transfer)

```
Error: permission denied
```

**Solution**: Ensure `/tmp` is writable by test user, or change `remote_path` in code.

## Performance Tips

1. **Run on dedicated hardware**: Avoid running other heavy processes
2. **Use wired network**: WiFi can introduce latency variance
3. **Multiple runs**: Run several times and compare medians
4. **Adjust iterations**: More iterations = more accurate statistics
5. **Consider disk I/O**: File transfers are affected by disk performance

## Implementation Notes

- Uses `hdrhistogram` for accurate percentile calculations
- File transfer uses random data to avoid compression artifacts
- Each benchmark warms up connections before timing
- Results use high-resolution timers (`Instant::now()`)

## Integration with Rustible

These benchmarks inform decisions about which SSH library to use in Rustible:

- **russh** is preferred for async-first architecture
- Better parallelism for multi-host playbook execution
- Native async reduces executor overhead
- Pure Rust improves portability and reduces dependencies

## Contributing

To add new benchmarks:

1. Add benchmark function in `src/main.rs`
2. Create `BenchStats` instance
3. Run iterations and record durations
4. Push result to `results` vec

Example:
```rust
let mut stats = BenchStats::new("My Benchmark", "ssh2");
for _ in 0..iterations {
    let duration = my_benchmark_function();
    stats.record(duration);
}
results.push(stats.to_result());
```

## License

Same as Rustible (MIT)
