# SSH Library Comparison Analysis

## Executive Summary

This document provides an analysis framework for comparing `ssh2` (libssh2 wrapper) versus `async-ssh2-tokio` (russh) for use in Rustible's async-first architecture.

## Libraries Compared

### ssh2 (libssh2 wrapper)

**Characteristics:**
- Wraps the C library libssh2
- Synchronous API
- Mature, battle-tested (libssh2 has 15+ years of production use)
- Requires `spawn_blocking` when used in async contexts
- System dependency on libssh2

**Pros:**
- Extremely stable and well-tested
- Wide compatibility
- Comprehensive SSH2 protocol support
- Known security track record

**Cons:**
- Synchronous blocking I/O
- Thread pool overhead when used with `spawn_blocking`
- External C dependency complicates cross-compilation
- Not idiomatic in async Rust codebases

### async-ssh2-tokio (russh)

**Characteristics:**
- Pure Rust implementation
- Native async/await with Tokio
- Modern, actively developed
- No external C dependencies
- Built from the ground up for async

**Pros:**
- Native async - no `spawn_blocking` needed
- Better integration with async runtime
- Pure Rust - easier cross-compilation
- Potentially better performance in concurrent scenarios
- Idiomatic async Rust code

**Cons:**
- Younger library (less battle-tested than libssh2)
- Smaller ecosystem
- Potentially less extensive SSH2 feature coverage
- May have undiscovered edge cases

## Benchmark Categories

### 1. Connection Establishment Time

**What it measures:**
- Time to establish TCP connection
- SSH handshake
- Key-based authentication
- Connection ready for use

**Why it matters:**
- Rustible often connects to many hosts
- Connection pooling can amortize this cost
- Initial playbook startup time

**Expected results:**
- Similar performance (network-bound)
- russh may have slight edge due to async I/O
- ssh2 has `spawn_blocking` overhead

### 2. Command Execution (Reused Connection)

**What it measures:**
- Time to execute a simple command on existing connection
- Channel creation, execution, result retrieval

**Why it matters:**
- Most common operation in Rustible
- Many tasks execute multiple commands
- Playbook throughput heavily depends on this

**Expected results:**
- russh should perform better (native async)
- ssh2 has thread synchronization overhead
- Both should be quite fast (milliseconds)

### 3. Command Execution (New Connection)

**What it measures:**
- Total time: connect + auth + exec + cleanup
- Simulates "one-shot" operations

**Why it matters:**
- Ad-hoc task execution
- One-off commands
- Connection pool misses

**Expected results:**
- Dominated by connection time
- russh likely faster due to async I/O
- ssh2 penalized by `spawn_blocking` overhead

### 4. File Transfer (SFTP)

**What it measures:**
- Upload performance (various file sizes)
- Download performance
- SFTP subsystem initialization

**Why it matters:**
- Template deployment
- File copy operations
- Artifact distribution

**Expected results:**
- Network and disk I/O bound
- May be similar between libraries
- Large transfers should saturate network
- SFTP implementation quality matters

### 5. Parallel Execution

**What it measures:**
- Multiple concurrent SSH operations
- Thread pool (ssh2) vs async tasks (russh)
- Scalability under load

**Why it matters:**
- Rustible's core value proposition
- Multi-host playbook execution
- Free strategy and parallel execution

**Expected results:**
- **russh should significantly outperform**
- ssh2 limited by blocking thread pool
- russh can scale to thousands of concurrent connections
- This is the most important benchmark for Rustible

## Metrics Explained

### Mean
Average time across all iterations. Good for overall performance indication but can be skewed by outliers.

### Median (P50)
Middle value - half of operations complete faster, half slower. More robust to outliers than mean.

### P95 (95th Percentile)
95% of operations complete within this time. Important for understanding typical worst-case.

### P99 (99th Percentile)
99% of operations complete within this time. Critical for identifying tail latency.

### Min/Max
Best and worst case. Large gap indicates high variance.

## Analysis Framework

### Performance Evaluation

When analyzing results, consider:

1. **Absolute Performance**
   - Are times acceptable for production use?
   - Do any operations have unacceptable latency?

2. **Relative Performance**
   - How much faster/slower is russh vs ssh2?
   - Are differences significant (>10%)?
   - Where are the biggest gaps?

3. **Consistency**
   - How large is the P50 to P99 gap?
   - Are there outliers (check Max)?
   - Is performance predictable?

4. **Scalability**
   - How do parallel benchmarks compare?
   - Does performance degrade with concurrency?

### Decision Matrix

| Factor | Weight | ssh2 | russh | Winner |
|--------|--------|------|-------|--------|
| **Performance** |
| Single connection speed | Medium | ? | ? | TBD |
| Parallel execution | **High** | ? | ? | TBD |
| File transfer | Medium | ? | ? | TBD |
| Connection pooling | Medium | ? | ? | TBD |
| **Architecture** |
| Async integration | **High** | ✗ | ✓ | russh |
| Code complexity | High | ✗ | ✓ | russh |
| Thread pool overhead | Medium | ✗ | ✓ | russh |
| **Reliability** |
| Maturity | **High** | ✓ | ✗ | ssh2 |
| Battle-tested | High | ✓ | ✗ | ssh2 |
| Security track record | High | ✓ | ? | ssh2 |
| **Ecosystem** |
| Dependencies | Medium | ✗ | ✓ | russh |
| Cross-compilation | Medium | ✗ | ✓ | russh |
| Pure Rust | Low | ✗ | ✓ | russh |

**Weights:**
- High: Critical for Rustible's success
- Medium: Important but not critical
- Low: Nice to have

### Expected Outcome

Based on architectural considerations, **russh is likely the better choice** if:

1. ✓ Parallel execution shows significant improvement (30%+ faster)
2. ✓ Single command performance is comparable or better
3. ✓ No major reliability concerns emerge from testing
4. ✓ SFTP functionality meets requirements

**ssh2 might be preferred** if:

1. russh shows reliability issues in production testing
2. Specific SSH2 features are missing in russh
3. Performance differences are negligible
4. Risk tolerance is very low

## Interpreting Results

### Good Results for russh

```
Parallel 10x | russh (async)     | 150.00 ms
Parallel 10x | ssh2 (spawn_blocking) | 280.00 ms
```
**Interpretation:** russh is 1.87x faster for parallel operations - strong advantage for multi-host execution.

### Acceptable Results

```
Command (reused) | russh | 2.10 ms
Command (reused) | ssh2  | 2.30 ms
```
**Interpretation:** Similar performance - no significant advantage either way.

### Concerning Results

```
Connection | russh | 450.00 ms (P99: 1200.00 ms)
Connection | ssh2  | 45.00 ms (P99: 52.00 ms)
```
**Interpretation:** russh showing 10x slower connection time with high variance - investigate implementation or network issues.

## Recommendations

### For Rustible Production Use

1. **Primary metric: Parallel execution performance**
   - This is where Rustible adds value
   - russh should excel here

2. **Secondary: Command execution on reused connection**
   - Most common operation
   - Must be fast (< 10ms for simple commands)

3. **Tertiary: Connection establishment**
   - Can be amortized with connection pooling
   - Still important for initial playbook startup

4. **File transfer: Baseline requirement**
   - Must work correctly
   - Performance should be "good enough" (network-bound anyway)

### Testing Beyond Benchmarks

Before production deployment, also test:

1. **Error handling**
   - Connection timeouts
   - Authentication failures
   - Network interruptions

2. **Edge cases**
   - Large file transfers (GB+)
   - Long-running commands
   - Connection stability over time

3. **Real-world scenarios**
   - Actual playbooks from test suite
   - Various host types (Ubuntu, RHEL, etc.)
   - Different network conditions

4. **Resource usage**
   - Memory consumption under load
   - CPU utilization
   - File descriptor limits

## Conclusion Template

After running benchmarks, fill this out:

**Performance Summary:**
- Connection time: russh is [X]x [faster/slower]
- Command execution: russh is [X]x [faster/slower]
- File transfer: russh is [X]x [faster/slower]
- Parallel (10x): russh is [X]x [faster/slower]

**Recommendation:** [Choose russh / Choose ssh2 / Need more testing]

**Rationale:**
[Explain decision based on benchmark results and architectural considerations]

**Next Steps:**
1. [Action item 1]
2. [Action item 2]
3. [Action item 3]

**Risks:**
- [Risk 1 and mitigation]
- [Risk 2 and mitigation]

## References

- [ssh2 crate](https://crates.io/crates/ssh2)
- [libssh2 library](https://www.libssh2.org/)
- [async-ssh2-tokio crate](https://crates.io/crates/async-ssh2-tokio)
- [russh repository](https://github.com/warp-tech/russh)
- [Tokio spawn_blocking docs](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
