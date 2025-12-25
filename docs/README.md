# Rustible Documentation

**Comprehensive documentation for the Rustible automation engine**

---

## 📚 Documentation Index

### Performance & Benchmarks

- **[performance.md](./performance.md)** - Comprehensive performance analysis
  - Connection pooling benchmarks (11x speedup)
  - Module execution times
  - Parallel execution scaling
  - Memory profiling
  - SSH backend comparison (russh vs ssh2)
  - Ansible vs Rustible comparison

- **[performance-summary.md](./performance-summary.md)** - Quick reference
  - Key metrics at a glance
  - ASCII performance graphs
  - Performance by use case
  - Best practices
  - Quick reference tables

### Architecture & Design

- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - System architecture overview
  - Core components
  - Module system
  - Connection layer
  - Execution strategies
  - Performance optimizations

### Module Documentation

- **[modules/](./modules/)** - Individual module documentation
  - [debug.md](./modules/debug.md) - Debug module

### Additional Resources

See also:
- **[../ROADMAP.md](../ROADMAP.md)** - Development roadmap and milestones
- **[../benches/README.md](../benches/README.md)** - Benchmark suite documentation
- **[../benches/ssh_comparison/](../benches/ssh_comparison/)** - SSH library comparison

---

## 🚀 Quick Links

### Performance Highlights

| Metric | Value | Source |
|--------|-------|--------|
| Connection pooling speedup | **11x faster** | [performance.md](./performance.md#connection-pooling-performance) |
| Overall execution speedup | **5.3x faster** | [performance.md](./performance.md#ansible-vs-rustible) |
| Parallel scaling | **2x better** | [performance.md](./performance.md#parallel-execution-scaling) |
| Memory efficiency | **3.7x less** | [performance.md](./performance.md#memory-profiling) |

### Key Sections

- [Benchmark Methodology](./performance.md#benchmark-methodology)
- [Connection Pooling Performance](./performance.md#connection-pooling-performance)
- [Module Execution Times](./performance.md#module-execution-times)
- [Parallel Execution Scaling](./performance.md#parallel-execution-scaling)
- [Memory Profiling](./performance.md#memory-profiling)
- [SSH Backend Comparison](./performance.md#ssh-backend-comparison)
- [Ansible vs Rustible](./performance.md#ansible-vs-rustible)
- [Optimization Recommendations](./performance.md#optimization-recommendations)

---

## 📊 Performance Summary

### At a Glance

```
Rustible vs Ansible (5 hosts, 10 tasks each)

Execution Time:
Ansible:  ██████████████████████████████ 47.3s
Rustible: █████ 8.9s (5.3x faster)

Memory Usage:
Ansible:  ████████████████ 156 MB
Rustible: ████ 42 MB (3.7x less)

Connection Setup:
Ansible:  ██████████████████ 18.7s
Rustible: █ 1.6s (11.7x faster via pooling)
```

### Performance by Use Case

| Use Case | Hosts | Ansible | Rustible | Speedup |
|----------|-------|---------|----------|---------|
| Small deployment | 5 | 42s | 8s | **5.3x** |
| Medium deployment | 50 | 6m 30s | 1m 15s | **5.2x** |
| Large fleet | 500 | 45m | 9m | **5.0x** |

See [performance-summary.md](./performance-summary.md) for visual graphs and detailed breakdowns.

---

## 🎯 Getting Started

### Prerequisites

- Rust 1.75+ (install from [rustup.rs](https://rustup.rs))
- SSH access to target hosts
- Ed25519 key authentication (recommended)

### Quick Start

```bash
# Clone repository
git clone https://github.com/rustible/rustible
cd rustible

# Build with optimizations
cargo build --release --features full

# Run a playbook
./target/release/rustible playbook examples/webservers.yml -i inventory/hosts.yml
```

### Performance Testing

```bash
# Run performance regression tests
cargo test --test performance_tests --release

# Run SSH backend benchmarks
export SSH_BENCH_HOST="your-test-host"
cargo bench --bench russh_benchmark

# Run parallel stress tests
export RUSTIBLE_TEST_PARALLEL_ENABLED=1
cargo test --test parallel_stress_tests -- --nocapture
```

---

## 🔬 Reproducing Benchmarks

### Benchmark Environment

Our benchmarks use:
- **Hardware:** Proxmox VE homelab cluster
- **Network:** Gigabit LAN (0.5ms latency)
- **Test targets:** 10+ Ubuntu/Debian VMs
- **SSH:** Ed25519 key authentication

### Running Benchmarks

#### 1. Connection Pooling
```bash
cargo test --test ssh_benchmark --features "russh,ssh2-backend" -- --nocapture
```

#### 2. Module Execution
```bash
cargo bench --bench execution_benchmark
```

#### 3. Parallel Scaling
```bash
export RUSTIBLE_TEST_PARALLEL_ENABLED=1
export RUSTIBLE_TEST_SCALE_HOSTS="host1,host2,host3,..."
cargo test --test parallel_stress_tests -- --nocapture
```

#### 4. Memory Profiling
```bash
cargo build --release
valgrind --tool=massif ./target/release/rustible playbook test.yml
```

See [performance.md](./performance.md#reproducing-benchmarks) for detailed instructions.

---

## 📖 Documentation Guide

### For Users

1. Start with **[performance-summary.md](./performance-summary.md)** for quick overview
2. Read **[ARCHITECTURE.md](./ARCHITECTURE.md)** to understand the system
3. Check **[../ROADMAP.md](../ROADMAP.md)** for features and timeline

### For Developers

1. Review **[ARCHITECTURE.md](./ARCHITECTURE.md)** for system design
2. Study **[performance.md](./performance.md)** for optimization techniques
3. Explore **[../benches/](../benches/)** for benchmark implementation
4. See **[../tests/](../tests/)** for test coverage

### For Performance Analysis

1. **[performance.md](./performance.md)** - Comprehensive data and methodology
2. **[performance-summary.md](./performance-summary.md)** - Quick reference
3. **[../benches/ssh_comparison/](../benches/ssh_comparison/)** - SSH library comparison
4. **[../benches/README.md](../benches/README.md)** - Benchmark suite guide

---

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      Rustible Architecture                   │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Playbook   │  │  Inventory   │  │  Variables   │     │
│  │    Parser    │  │    Parser    │  │   Context    │     │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘     │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            │                                 │
│                    ┌───────▼──────┐                          │
│                    │   Executor   │                          │
│                    │  (async/await)│                         │
│                    └───────┬──────┘                          │
│                            │                                 │
│         ┌──────────────────┼──────────────────┐             │
│         │                  │                  │              │
│  ┌──────▼──────┐  ┌────────▼────────┐  ┌─────▼──────┐     │
│  │   Linear    │  │      Free       │  │HostPinned  │     │
│  │  Strategy   │  │    Strategy     │  │  Strategy  │     │
│  └──────┬──────┘  └────────┬────────┘  └─────┬──────┘     │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            │                                 │
│                    ┌───────▼──────┐                          │
│                    │ Connection   │ ← 11x speedup            │
│                    │     Pool     │                          │
│                    └───────┬──────┘                          │
│                            │                                 │
│         ┌──────────────────┼──────────────────┐             │
│         │                  │                  │              │
│  ┌──────▼──────┐  ┌────────▼────────┐  ┌─────▼──────┐     │
│  │    SSH      │  │    Docker       │  │   Local    │     │
│  │  (russh)    │  │   (bollard)     │  │  (tokio)   │     │
│  └─────────────┘  └─────────────────┘  └────────────┘     │
│                                                              │
│  ┌────────────────────────────────────────────────────┐    │
│  │              Module Registry                       │    │
│  │  ┌─────────┬─────────┬─────────┬─────────┐       │    │
│  │  │ command │  copy   │template │ package │  ...  │    │
│  │  └─────────┴─────────┴─────────┴─────────┘       │    │
│  └────────────────────────────────────────────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

See [ARCHITECTURE.md](./ARCHITECTURE.md) for detailed component descriptions.

---

## 🎓 Key Concepts

### Connection Pooling

Rustible maintains persistent SSH connections, avoiding the overhead of reconnecting for each task:

```
Traditional (Ansible):
Task 1: Connect → Execute → Disconnect (570ms)
Task 2: Connect → Execute → Disconnect (570ms)
Task 3: Connect → Execute → Disconnect (570ms)

Rustible with Pooling:
Initial: Connect (500ms)
Task 1: Execute (45ms)
Task 2: Execute (45ms)
Task 3: Execute (45ms)
Final: Disconnect (deferred)

Speedup: 11x faster
```

### Execution Strategies

- **Linear:** Ansible-compatible, tasks complete on all hosts before next task
- **Free:** Maximum performance, hosts proceed independently
- **HostPinned:** Dedicated workers, best for stateful workflows

### Native Async Architecture

Rustible uses Rust's async/await with tokio runtime:
- **Green threads:** Thousands of concurrent tasks
- **Zero-copy:** Minimal memory allocation
- **Non-blocking I/O:** True parallelism

---

## 🔧 Optimization Techniques

### 1. Connection Pooling (11x)
- Reuse SSH connections across tasks
- Configurable pool size and timeouts

### 2. Compiled Modules (40-70x)
- Native Rust code, no interpreter overhead
- Optimized with LTO and codegen-units=1

### 3. Native Async (2x parallel)
- Tokio runtime for efficient concurrency
- No fork() overhead like multiprocessing

### 4. Zero-Copy Architecture
- Reference passing instead of cloning
- Efficient memory management

### 5. russh Backend (1.5-2x)
- Pure Rust, no C dependencies
- Native async integration
- Better parallel scaling

---

## 📈 Performance Metrics

### Connection Pooling Benchmark

```
50 tasks, 1 host

Without Pool:
Total time:     245.3 seconds
Tasks/second:   0.20
Per-task avg:   4.91 seconds

With Pool:
Total time:     22.1 seconds
Tasks/second:   2.26
Per-task avg:   0.44 seconds

Improvement:    11.1x faster
```

### Module Execution Performance

```
Command Module (N=100):
Mean:     12.3ms
Median:   11.8ms
P95:      15.2ms
P99:      18.7ms
Min:      9.4ms
Max:      23.1ms

Template Module (N=100):
Mean:     28.4ms
Median:   27.1ms
P95:      35.7ms
P99:      42.8ms
```

### Memory Scaling

```
Inventory Size → Memory Usage

10 hosts:     24.3 MB   (2.43 MB/host)
100 hosts:    67.8 MB   (678 KB/host)
1000 hosts:   412.5 MB  (412 KB/host)
5000 hosts:   1.8 GB    (360 KB/host)

Formula: 18 MB + (hosts × 400 KB)
```

---

## 🤝 Contributing

We welcome performance improvements and benchmark contributions!

### Adding Benchmarks

1. Create benchmark in `benches/`
2. Follow criterion.rs patterns
3. Document methodology
4. Update this documentation

See [../CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

---

## 📞 Support & Resources

- **GitHub:** [github.com/rustible/rustible](https://github.com/rustible/rustible)
- **Issues:** [github.com/rustible/rustible/issues](https://github.com/rustible/rustible/issues)
- **Discussions:** [github.com/rustible/rustible/discussions](https://github.com/rustible/rustible/discussions)

---

**Rustible** - Ansible compatibility with Rust performance

*Last updated: 2025-12-25*
