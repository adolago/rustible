# SSH Benchmark Quick Start

Get started with SSH library comparison benchmarks in 5 minutes.

## Prerequisites

1. **SSH Server Access**
   - A reachable SSH server (local VM, remote server, or Docker container)
   - Key-based authentication configured
   - User with permissions to write to `/tmp`

2. **Rust Toolchain**
   - Rust 1.75+ installed
   - `cargo` available

## Quick Setup

### Option 1: Docker SSH Server (Easiest)

```bash
# Start SSH server in Docker
docker run -d \
  --name ssh-bench-server \
  -p 2222:22 \
  -e PUID=1000 \
  -e PGID=1000 \
  -e USER_NAME=testuser \
  -e PUBLIC_KEY="$(cat ~/.ssh/id_ed25519.pub)" \
  linuxserver/openssh-server

# Wait a few seconds for server to start
sleep 5

# Test connection
ssh -p 2222 -i ~/.ssh/id_ed25519 testuser@localhost "echo 'Connection works!'"
```

### Option 2: Use Existing Server

If you have an existing SSH server:

```bash
# Test connection
ssh -i ~/.ssh/id_ed25519 testuser@your-server "echo 'Connection works!'"
```

## Running Benchmarks

### Quick Test (1 minute)

```bash
cd benches/ssh_comparison

# For Docker server
./run_benchmarks.sh -h localhost -p 2222 -u testuser quick

# For existing server
./run_benchmarks.sh -h your-server -u testuser quick
```

### Standard Benchmark (5 minutes)

```bash
# For Docker server
./run_benchmarks.sh -h localhost -p 2222 -u testuser standard

# For existing server
./run_benchmarks.sh -h your-server -u testuser standard
```

### Full Benchmark (15 minutes)

```bash
./run_benchmarks.sh -h your-server -u testuser thorough
```

## Reading Results

Look for the results table:

```
┌──────────────────┬──────────────────────┬───────────┬────────────┐
│ Benchmark        │ Library              │ Mean (ms) │ Median (ms)│
├──────────────────┼──────────────────────┼───────────┼────────────┤
│ Connection       │ ssh2                 │ 45.23     │ 44.10      │
│ Connection       │ russh                │ 38.12     │ 37.50      │  ← russh is faster
│ Parallel 10x     │ ssh2 (spawn_blocking)│ 285.40    │ 280.10     │
│ Parallel 10x     │ russh (async)        │ 155.30    │ 152.20     │  ← russh is 1.8x faster!
└──────────────────┴──────────────────────┴───────────┴────────────┘
```

**Key Takeaway:** Lower numbers = better performance

## What Each Benchmark Tests

| Benchmark | What It Tests | Why It Matters |
|-----------|---------------|----------------|
| Connection | SSH connect + auth time | Playbook startup speed |
| Command (reused) | Execute on existing connection | Task execution speed |
| Connect + Command | One-shot operations | Ad-hoc commands |
| Upload/Download | File transfer (SFTP) | Template deployment, file copy |
| Parallel 10x | Concurrent operations | Multi-host playbook performance |

## Common Issues

### Connection Refused

```bash
# Check if SSH server is running
docker ps | grep ssh-bench-server

# Or test direct SSH
ssh -p 2222 testuser@localhost
```

### Authentication Failed

```bash
# Verify key exists
ls -la ~/.ssh/id_ed25519

# Re-add public key to Docker container
docker restart ssh-bench-server
```

### Permission Denied (during file transfer)

```bash
# Check /tmp is writable
ssh testuser@localhost "touch /tmp/test && rm /tmp/test"
```

## Customization

### Different SSH Key

```bash
./run_benchmarks.sh -k ~/.ssh/id_rsa standard
```

### Different Iterations

```bash
# Fewer iterations (faster)
cargo run --release -- --iterations 20 --skip-file-transfer

# More iterations (more accurate)
cargo run --release -- --iterations 500
```

### Larger File Transfers

```bash
cargo run --release -- --file-size-kb 1024  # 1 MB files
```

### Verbose Output

```bash
./run_benchmarks.sh -v standard
```

## Environment Variables

Set these to avoid typing repeatedly:

```bash
export SSH_BENCH_HOST=192.168.1.100
export SSH_BENCH_PORT=22
export SSH_BENCH_USER=myuser
export SSH_BENCH_KEY=~/.ssh/id_rsa

# Now just run
./run_benchmarks.sh standard
```

## Next Steps

1. ✅ Run quick benchmark to verify setup
2. ✅ Run standard benchmark for initial data
3. 📊 Review results in `results/` directory
4. 📖 Read `ANALYSIS.md` for interpretation guidance
5. 🔬 Run thorough benchmark for final decision-making

## Cleanup

### Remove Docker SSH Server

```bash
docker stop ssh-bench-server
docker rm ssh-bench-server
```

### Clean Build Artifacts

```bash
cd benches/ssh_comparison
cargo clean
```

## Results Location

Benchmark results are saved to:
```
benches/ssh_comparison/results/benchmark_TIMESTAMP.txt
```

Example:
```
benches/ssh_comparison/results/benchmark_20251222_143052.txt
```

## Help

```bash
# Show all options
./run_benchmarks.sh --help

# Or
cargo run --release -- --help
```

## Pro Tips

1. **Run multiple times**: Results can vary due to network conditions
2. **Use wired network**: WiFi adds latency variance
3. **Close other applications**: Reduce CPU contention
4. **Local server**: Use Docker for most consistent results
5. **Compare medians**: More reliable than means for performance comparison

## Example Workflow

```bash
# 1. Start Docker SSH server
docker run -d --name ssh-bench -p 2222:22 \
  -e USER_NAME=testuser \
  -e PUBLIC_KEY="$(cat ~/.ssh/id_ed25519.pub)" \
  linuxserver/openssh-server

# 2. Wait for startup
sleep 5

# 3. Quick test
cd benches/ssh_comparison
./run_benchmarks.sh -h localhost -p 2222 quick

# 4. Full benchmark
./run_benchmarks.sh -h localhost -p 2222 standard

# 5. Review results
cat results/benchmark_*.txt | tail -50

# 6. Cleanup
docker stop ssh-bench && docker rm ssh-bench
```

Done! You now have comprehensive SSH library performance data for Rustible.
