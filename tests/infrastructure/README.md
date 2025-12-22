# Rustible Heavy-Duty Test Infrastructure

This directory contains infrastructure-as-code for deploying a comprehensive test cluster on Proxmox VE (svr-host) to validate Rustible at scale.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         svr-host (Proxmox VE)                       │
│                     Intel i9-14900KF, 94GB RAM                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │
│  │  test-ctrl-301  │  │  docker-310     │  │  heterogeneous  │     │
│  │  (Orchestrator) │  │  (Docker Host)  │  │  VMs (320-322)  │     │
│  │  Ubuntu 24.04   │  │  Ubuntu 24.04   │  │  Multi-distro   │     │
│  │  4 cores, 8GB   │  │  4 cores, 8GB   │  │  2c/2GB each    │     │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘     │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    SSH Target Fleet (LXC)                    │   │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐│   │
│  │  │ssh-t-401│ │ssh-t-402│ │ssh-t-403│ │ssh-t-404│ │ssh-t-405││   │
│  │  │ 2c/2GB  │ │ 2c/2GB  │ │ 2c/2GB  │ │ 2c/2GB  │ │ 2c/2GB  ││   │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘│   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                   Scale Test Fleet (LXC)                     │   │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ... ┌────────┐ │   │
│  │  │scl-501 │ │scl-502 │ │scl-503 │ │scl-504 │     │scl-510 │ │   │
│  │  │ 1c/1GB │ │ 1c/1GB │ │ 1c/1GB │ │ 1c/1GB │     │ 1c/1GB │ │   │
│  │  └────────┘ └────────┘ └────────┘ └────────┘     └────────┘ │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Resource Allocation

| Component | VMID | Type | vCPUs | RAM | Disk | Purpose |
|-----------|------|------|-------|-----|------|---------|
| test-ctrl | 301 | LXC | 4 | 8GB | 32GB | Test orchestration, Rust toolchain |
| docker-host | 310 | VM | 4 | 8GB | 64GB | Docker connection tests |
| ubuntu-target | 320 | VM | 2 | 2GB | 16GB | Ubuntu SSH target |
| debian-target | 321 | VM | 2 | 2GB | 16GB | Debian SSH target |
| rocky-target | 322 | VM | 2 | 2GB | 16GB | Rocky Linux target |
| ssh-target-N | 401-405 | LXC | 2 | 2GB | 8GB | SSH integration tests |
| scale-N | 501-510 | LXC | 1 | 1GB | 4GB | Parallel execution stress |

**Total: ~38GB RAM, ~50 vCPUs, ~220GB storage**

## Quick Start

```bash
# From wrk-main or any machine with SSH access to svr-host

# 1. Deploy test infrastructure
./provision.sh deploy

# 2. Run all tests
./run-tests.sh all

# 3. Run specific test suite
./run-tests.sh ssh-integration
./run-tests.sh parallel-stress
./run-tests.sh chaos

# 4. Tear down (preserves test-ctrl)
./provision.sh teardown

# 5. Full cleanup
./provision.sh destroy
```

## Test Suites

### 1. SSH Integration Tests (`tests/real_ssh_tests.rs`)
- Real SSH connections to LXC containers
- Key and password authentication
- SFTP file transfers
- Connection pooling validation
- Privilege escalation (sudo)
- Long-running command handling

### 2. Multi-Host Parallel Tests (`tests/parallel_stress_tests.rs`)
- 10-50 concurrent hosts
- Linear vs Free vs HostPinned strategies
- Fork limiting under load
- Throttle interaction tests
- Serial batching validation

### 3. Docker Integration Tests (`tests/real_docker_tests.rs`)
- Docker exec operations
- Docker cp file transfers
- Container lifecycle tests
- Docker Compose support

### 4. Chaos Engineering Tests (`tests/chaos_tests.rs`)
- Network partition simulation
- Random connection drops
- Slow network conditions
- Memory pressure scenarios
- Disk full conditions

### 5. Ansible Compatibility Tests (`tests/ansible_compat_e2e.rs`)
- Real playbook execution
- Module compatibility
- Variable precedence validation
- Handler execution verification

## Environment Variables

```bash
# Required for real SSH tests
export RUSTIBLE_TEST_SSH_ENABLED=1
export RUSTIBLE_TEST_SSH_USER=testuser
export RUSTIBLE_TEST_SSH_KEY=/path/to/test_key

# SSH target hosts (auto-discovered from inventory)
export RUSTIBLE_TEST_INVENTORY=/path/to/test_inventory.yml

# Docker tests
export RUSTIBLE_TEST_DOCKER_ENABLED=1
export RUSTIBLE_TEST_DOCKER_HOST=tcp://192.168.178.X:2375

# Chaos tests (requires root or privileged container)
export RUSTIBLE_TEST_CHAOS_ENABLED=1
```

## Network Layout

All test VMs/containers connect to `vmbr0` (main LAN: 192.168.178.0/24).

| Host | IP Address | SSH Port |
|------|------------|----------|
| test-ctrl-301 | 192.168.178.201 | 22 |
| docker-310 | 192.168.178.210 | 22 |
| ubuntu-320 | 192.168.178.220 | 22 |
| debian-321 | 192.168.178.221 | 22 |
| rocky-322 | 192.168.178.222 | 22 |
| ssh-target-401 | 192.168.178.141 | 22 |
| ssh-target-402 | 192.168.178.142 | 22 |
| ssh-target-403 | 192.168.178.143 | 22 |
| ssh-target-404 | 192.168.178.144 | 22 |
| ssh-target-405 | 192.168.178.145 | 22 |
| scale-501..510 | 192.168.178.151-160 | 22 |

## Maintenance

```bash
# Start all test VMs
./provision.sh start

# Stop all test VMs (preserves state)
./provision.sh stop

# Snapshot current state
./provision.sh snapshot "pre-test-run"

# Rollback to snapshot
./provision.sh rollback "pre-test-run"

# View logs
./provision.sh logs test-ctrl-301
```
