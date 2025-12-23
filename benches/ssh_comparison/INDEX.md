# SSH Library Comparison Benchmarks - Documentation Index

Welcome to the SSH library comparison benchmark suite for Rustible. This directory contains comprehensive benchmarks comparing `ssh2` (libssh2) and `async-ssh2-tokio` (russh).

## 📚 Documentation Guide

Start here based on your needs:

### 🚀 I want to run benchmarks quickly
→ **[QUICKSTART.md](QUICKSTART.md)** - Get results in 5 minutes

### 📖 I want to understand what's being tested
→ **[README.md](README.md)** - Full documentation on benchmarks and setup

### 📊 I want to see example results
→ **[EXAMPLE_OUTPUT.md](EXAMPLE_OUTPUT.md)** - Sample output with analysis

### 🔬 I want to interpret my results
→ **[ANALYSIS.md](ANALYSIS.md)** - Framework for analyzing results

### 📝 I want a high-level overview
→ **[SUMMARY.md](SUMMARY.md)** - Executive summary and key points

### ⚙️ I want to run different scenarios
→ **[run_benchmarks.sh](run_benchmarks.sh)** - Convenience script with presets

## 📂 File Structure

```
benches/ssh_comparison/
│
├── 📖 Documentation
│   ├── INDEX.md              ← You are here
│   ├── QUICKSTART.md         ← 5-minute getting started
│   ├── README.md             ← Complete documentation
│   ├── SUMMARY.md            ← Executive summary
│   ├── ANALYSIS.md           ← Result interpretation
│   └── EXAMPLE_OUTPUT.md     ← Sample output
│
├── 💻 Code
│   ├── src/main.rs           ← Benchmark implementation
│   ├── Cargo.toml            ← Dependencies
│   └── Cargo.lock            ← Locked dependencies
│
├── 🔧 Tools
│   └── run_benchmarks.sh     ← Convenience script
│
└── 📊 Results (created at runtime)
    └── results/
        └── benchmark_*.txt   ← Timestamped results
```

## 🎯 Quick Reference

### What This Benchmarks

| Category | Description | Importance |
|----------|-------------|------------|
| **Connection Time** | SSH handshake + auth | Medium |
| **Command (reused)** | Execute on existing connection | High |
| **Connect + Command** | Full cycle (connect/exec/close) | Medium |
| **File Transfer** | Upload/download performance | Medium |
| **Parallel 10x** | 10 concurrent connections | **CRITICAL** |

### Why This Matters

Rustible is an async-first, multi-host automation tool. The choice between:
- **ssh2**: C library (libssh2), synchronous, needs `spawn_blocking`
- **russh**: Pure Rust, async-native

...significantly impacts performance, especially for parallel multi-host execution.

### Expected Outcome

**russh should win decisively on parallel execution** (1.5-2x faster), which is Rustible's core value proposition.

## 🏃 Quick Commands

```bash
# Navigate to directory
cd benches/ssh_comparison

# Quick test (1 min)
./run_benchmarks.sh quick

# Standard benchmark (5 min)
./run_benchmarks.sh standard

# Full benchmark (15 min)
./run_benchmarks.sh thorough

# View latest results
cat results/benchmark_*.txt | tail -50

# Custom run
./target/release/ssh_bench \
  --host your-server \
  --user your-user \
  --iterations 100 \
  --verbose
```

## 📋 Prerequisites Checklist

- [ ] SSH server accessible
- [ ] Key-based authentication configured
- [ ] Rust 1.75+ installed
- [ ] 5-15 minutes available for benchmarks
- [ ] Wired network connection (recommended)

## 🔍 Reading Path

### For Developers

1. **QUICKSTART.md** - Set up and run
2. **README.md** - Understand implementation
3. Run benchmarks
4. **ANALYSIS.md** - Interpret results
5. Make decision

### For Decision Makers

1. **SUMMARY.md** - High-level overview
2. **EXAMPLE_OUTPUT.md** - See what results look like
3. Review actual results from engineering team
4. **ANALYSIS.md** - Decision criteria
5. Approve library choice

### For Contributors

1. **README.md** - Full documentation
2. Review `src/main.rs` - Implementation
3. **ANALYSIS.md** - Testing methodology
4. Add new benchmarks following existing patterns

## 🎓 Learning Path

### Level 1: Just Run It
```bash
./run_benchmarks.sh quick
```
Read: QUICKSTART.md (5 min)

### Level 2: Understand Results
```bash
./run_benchmarks.sh standard
```
Read: EXAMPLE_OUTPUT.md (10 min)

### Level 3: Deep Analysis
```bash
./run_benchmarks.sh thorough
```
Read: ANALYSIS.md (20 min)

### Level 4: Contribute
```bash
# Modify src/main.rs
cargo build --release
./target/release/ssh_bench
```
Read: README.md + src/main.rs (30 min)

## 🔑 Key Insights

From running these benchmarks, you'll learn:

1. **Parallel performance difference** between sync and async SSH
2. **Impact of spawn_blocking** on throughput
3. **Connection pooling benefits** for both libraries
4. **File transfer performance** characteristics
5. **Latency distribution** (P50, P95, P99) for reliability

## ⚡ TL;DR

**Want the fastest path to a decision?**

1. Run: `./run_benchmarks.sh -h your-server quick` (1 min)
2. Look at "Parallel 10x" result
3. If russh is 1.5x+ faster → Use russh ✅
4. Read ANALYSIS.md for full justification

## 📞 Need Help?

- **Setup issues**: See README.md "Troubleshooting" section
- **Result interpretation**: See ANALYSIS.md
- **Example results**: See EXAMPLE_OUTPUT.md
- **Quick start**: See QUICKSTART.md

## 🎯 Success Criteria

After running benchmarks, you should be able to answer:

- [ ] Is russh faster for parallel execution?
- [ ] How much faster? (Target: 1.5-2x)
- [ ] Are there any reliability concerns?
- [ ] Do both libraries support required features?
- [ ] What is the recommendation for Rustible?

## 📊 Benchmark Coverage

| Aspect | Tested | Documentation |
|--------|--------|---------------|
| Connection time | ✅ Yes | README.md §2.1 |
| Command execution | ✅ Yes | README.md §2.2 |
| File transfer | ✅ Yes | README.md §2.4 |
| Parallel execution | ✅ Yes | README.md §2.5 |
| Async vs blocking | ✅ Yes | README.md §2.5 |
| Latency distribution | ✅ Yes | ANALYSIS.md |
| Error handling | ⚠️ Manual | README.md "Beyond Benchmarks" |
| Memory usage | ⚠️ Manual | README.md "Beyond Benchmarks" |

## 🏗️ Architecture

```
┌─────────────────────────────────────────┐
│         Benchmark Runner (main)         │
└────────────┬────────────────────────────┘
             │
   ┌─────────┴──────────┐
   │                    │
   ▼                    ▼
┌──────────┐      ┌──────────┐
│   ssh2   │      │  russh   │
│ (libssh2)│      │ (async)  │
└────┬─────┘      └────┬─────┘
     │                 │
     ▼                 ▼
┌─────────────────────────────┐
│      SSH Server (test)      │
└─────────────────────────────┘
```

## 🔄 Workflow

```
Setup SSH Server
       ↓
Build Benchmarks (cargo build --release)
       ↓
Run Quick Test (./run_benchmarks.sh quick)
       ↓
   Results OK?
       ↓ Yes
Run Full Benchmark (./run_benchmarks.sh standard)
       ↓
Analyze Results (compare Parallel 10x)
       ↓
Read ANALYSIS.md
       ↓
Make Decision (russh vs ssh2)
```

## 📈 Typical Results

**Expected performance improvements with russh:**

- Connection: ~15-20% faster
- Command execution: ~10-15% faster
- File transfer: ~5-10% faster
- **Parallel execution: ~50-100% faster** ⭐

The parallel execution difference is the **most important metric**.

## 🎓 Additional Resources

- [ssh2 crate](https://crates.io/crates/ssh2)
- [async-ssh2-tokio crate](https://crates.io/crates/async-ssh2-tokio)
- [russh repository](https://github.com/warp-tech/russh)
- [Tokio documentation](https://tokio.rs)
- [libssh2 documentation](https://www.libssh2.org/)

## ✨ Features

- ✅ Comprehensive coverage (5 benchmark categories)
- ✅ Statistical analysis (mean, median, P95, P99)
- ✅ Easy to run (convenience scripts)
- ✅ Well documented (6 documentation files)
- ✅ Configurable (iterations, file size, host, etc.)
- ✅ Results saved (timestamped output files)
- ✅ Progress indicators (verbose mode)
- ✅ Multiple presets (quick, standard, thorough)

## 🚦 Status

- [x] Benchmark implementation complete
- [x] Documentation complete
- [x] Builds successfully
- [x] Help output verified
- [ ] Results from actual SSH server (requires setup)
- [ ] Production recommendation (pending results)

## 📝 License

Same as Rustible (MIT)

---

**Ready to start?** → [QUICKSTART.md](QUICKSTART.md)

**Want details?** → [README.md](README.md)

**Need help?** → All docs have troubleshooting sections
