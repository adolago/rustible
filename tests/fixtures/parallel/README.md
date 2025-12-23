# Parallel Execution End-to-End Tests

This directory contains fixtures and tests for validating Rustible's parallel execution capabilities.

## Overview

The parallel execution tests validate:

1. **Parallel task execution** across multiple hosts
2. **Connection pooling and reuse** for efficient SSH connections
3. **Performance improvements** from parallelization
4. **Different execution strategies** (Linear vs Free)
5. **Fork limiting** behavior with semaphore-based concurrency control

## Test Structure

```
tests/fixtures/parallel/
├── playbook.yml      # Test playbook with multiple tasks
├── inventory.yml     # Sample inventory (can be overridden via env vars)
└── README.md         # This file
```

## Running the Tests

### Quick Start (Localhost Only)

Run tests using only localhost (no external hosts required):

```bash
cargo test --test parallel_e2e_tests test_parallel_execution_on_localhost -- --nocapture
```

### Full E2E Tests (with SSH Hosts)

To run the complete test suite with real SSH hosts:

1. **Set up test environment variables:**

```bash
export RUSTIBLE_E2E_ENABLED=1
export RUSTIBLE_E2E_SSH_USER=testuser
export RUSTIBLE_E2E_SSH_KEY=~/.ssh/id_ed25519
export RUSTIBLE_E2E_HOSTS=192.168.178.141,192.168.178.142,192.168.178.143,192.168.178.144
```

2. **Run all E2E tests:**

```bash
cargo test --test parallel_e2e_tests -- --nocapture --test-threads=1
```

### Individual Test Cases

Run specific tests:

```bash
# Test parallel execution on multiple hosts
cargo test --test parallel_e2e_tests test_parallel_execution_multiple_hosts -- --nocapture

# Compare Linear vs Free strategies
cargo test --test parallel_e2e_tests test_linear_vs_free_strategy_performance -- --nocapture

# Test connection pooling
cargo test --test parallel_e2e_tests test_connection_reuse_in_parallel_execution -- --nocapture

# Test fork limiting
cargo test --test parallel_e2e_tests test_fork_limiting_with_many_hosts -- --nocapture

# Measure performance improvement
cargo test --test parallel_e2e_tests test_parallel_performance_improvement -- --nocapture
```

## Environment Variables

| Variable | Description | Default | Example |
|----------|-------------|---------|---------|
| `RUSTIBLE_E2E_ENABLED` | Enable E2E tests | `false` | `1` or `true` |
| `RUSTIBLE_E2E_SSH_USER` | SSH username for test hosts | `testuser` | `ubuntu` |
| `RUSTIBLE_E2E_SSH_KEY` | Path to SSH private key | `~/.ssh/id_ed25519` | `~/.ssh/id_rsa` |
| `RUSTIBLE_E2E_HOSTS` | Comma-separated list of host IPs | Empty | `10.0.0.1,10.0.0.2` |

## Test Infrastructure Setup

### Using Docker Containers

You can set up test hosts using Docker containers with SSH:

```bash
# Create a test SSH container
docker run -d \
  --name rustible-test-1 \
  -p 2222:22 \
  -e SSH_USERS=testuser:1001:1001 \
  -e SSH_ENABLE_PASSWORD_AUTH=false \
  -v ~/.ssh/id_ed25519.pub:/home/testuser/.ssh/authorized_keys:ro \
  linuxserver/openssh-server

# Set environment to use the container
export RUSTIBLE_E2E_ENABLED=1
export RUSTIBLE_E2E_SSH_USER=testuser
export RUSTIBLE_E2E_HOSTS=localhost:2222
```

### Using VMs or Cloud Instances

For more realistic testing, use actual VMs or cloud instances:

```bash
# Example with AWS EC2 instances
export RUSTIBLE_E2E_ENABLED=1
export RUSTIBLE_E2E_SSH_USER=ec2-user
export RUSTIBLE_E2E_SSH_KEY=~/.ssh/aws-key.pem
export RUSTIBLE_E2E_HOSTS=\
ec2-1.amazonaws.com,\
ec2-2.amazonaws.com,\
ec2-3.amazonaws.com,\
ec2-4.amazonaws.com
```

## What Each Test Validates

### test_parallel_execution_on_localhost

- Validates basic playbook execution
- Uses only localhost (no external dependencies)
- Good for smoke testing

### test_parallel_execution_multiple_hosts

- Validates parallel execution across multiple hosts
- Tests connection establishment and task execution
- Verifies all hosts complete their tasks
- Reports detailed statistics per host

### test_linear_vs_free_strategy_performance

- Compares Linear and Free execution strategies
- Measures execution time for each strategy
- Calculates speedup from Free strategy
- Validates both strategies produce correct results

### test_connection_reuse_in_parallel_execution

- Creates a playbook with 5 sequential tasks
- Validates that connections are reused across tasks
- Ensures connection pooling works correctly
- Verifies all tasks complete on all hosts

### test_fork_limiting_with_many_hosts

- Tests semaphore-based fork limiting
- Validates that concurrency respects the fork limit
- Ensures tasks still complete with limited parallelism

### test_parallel_performance_improvement

- Compares serial (forks=1) vs parallel (forks=N) execution
- Uses a slow task (sleep 2) to amplify differences
- Calculates speedup and efficiency metrics
- Validates that parallel execution is significantly faster

## Expected Results

### Performance Expectations

With 4 hosts and a 2-second sleep task:

- **Serial execution (forks=1)**: ~8 seconds (2s × 4 hosts)
- **Parallel execution (forks=4)**: ~2 seconds (2s × 1)
- **Expected speedup**: ~4x
- **Expected efficiency**: ~100%

### Success Criteria

For tests to pass:

1. All hosts must complete their tasks
2. No unreachable hosts (unless expected)
3. Parallel execution must be faster than serial
4. Speedup must be at least 1.5x for 2+ hosts
5. Connection reuse must work across multiple tasks

## Benchmarking

To benchmark parallel execution performance:

```bash
# Run the performance test with timing output
cargo test --test parallel_e2e_tests test_parallel_performance_improvement \
  -- --nocapture --exact

# Run with verbose output for detailed timing
RUST_LOG=rustible=debug cargo test --test parallel_e2e_tests \
  test_parallel_performance_improvement -- --nocapture --exact
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Parallel E2E Tests

on: [push, pull_request]

jobs:
  e2e:
    runs-on: ubuntu-latest
    services:
      ssh-server-1:
        image: linuxserver/openssh-server
        ports:
          - 2221:22
        env:
          SSH_USERS: testuser:1001:1001

      ssh-server-2:
        image: linuxserver/openssh-server
        ports:
          - 2222:22
        env:
          SSH_USERS: testuser:1001:1001

    steps:
      - uses: actions/checkout@v3

      - name: Setup SSH keys
        run: |
          ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -N ""
          # Add keys to containers...

      - name: Run E2E tests
        env:
          RUSTIBLE_E2E_ENABLED: 1
          RUSTIBLE_E2E_SSH_USER: testuser
          RUSTIBLE_E2E_HOSTS: localhost:2221,localhost:2222
        run: |
          cargo test --test parallel_e2e_tests -- --nocapture
```

## Troubleshooting

### Tests Skip Automatically

If you see "Skipping E2E tests (RUSTIBLE_E2E_ENABLED not set)", make sure:

1. `RUSTIBLE_E2E_ENABLED=1` is set
2. For multi-host tests, `RUSTIBLE_E2E_HOSTS` is configured

### Connection Failures

If tests fail with SSH connection errors:

1. Verify hosts are reachable: `ping <host>`
2. Test SSH manually: `ssh -i <key> <user>@<host>`
3. Check firewall rules
4. Verify SSH key permissions: `chmod 600 ~/.ssh/id_ed25519`

### Slow Performance

If parallel execution is not faster than serial:

1. Check that `forks` setting is greater than 1
2. Verify network latency to test hosts
3. Ensure test hosts have sufficient resources
4. Check if tasks are actually parallelizable

## Related Tests

- `tests/parallel_stress_tests.rs` - Lower-level connection stress tests
- `tests/ssh_tests.rs` - SSH-specific connection tests
- `benches/execution_benchmark.rs` - Benchmark suite for performance metrics

## Contributing

When adding new parallel execution tests:

1. Follow the existing test structure
2. Use environment variables for configuration
3. Include skip conditions for environments without SSH hosts
4. Document expected behavior and performance characteristics
5. Add comprehensive error messages for test failures
