# Rustible Feature Roadmap

A comprehensive roadmap for Rustible development, outlining current features, planned enhancements, and community-driven priorities.

## Table of Contents

- [Vision Statement](#vision-statement)
- [v0.1 - MVP (Current)](#v01---mvp-current)
- [v0.2 - Planned Features](#v02---planned-features)
- [v1.0 - Production Ready](#v10---production-ready)
- [Community Feature Requests](#community-feature-requests)
- [Contributor Guidelines](#contributor-guidelines)
- [Performance Benchmarks](#performance-benchmarks)

---

## Vision Statement

**Rustible** aims to combine Ansible's accessibility with Nix-like guarantees and Rust's performance. Our goal is to provide a modern, async-first configuration management tool that offers:

- **Drop-in Ansible compatibility** - Existing playbooks work with minimal changes
- **Superior performance** - 5-11x faster than Ansible through native Rust and connection pooling
- **Type safety** - Catch configuration errors at parse time, not runtime
- **Reproducibility** - State tracking, drift detection, and lockfiles for guaranteed outcomes

---

## v0.1 - MVP (Current)

**Status**: Released (December 2025)
**Focus**: Core functionality with Ansible compatibility

### Core Execution Engine

| Feature | Status | Description |
|---------|--------|-------------|
| Playbook parsing | Complete | YAML playbooks with plays, tasks, handlers |
| Inventory management | Complete | YAML, INI, JSON formats; dynamic scripts |
| Task execution | Complete | Sequential and parallel task execution |
| Variable resolution | Complete | Full variable precedence chain |
| Template engine | Complete | Jinja2-compatible via MiniJinja |
| Handlers | Complete | Including `listen` for multiple triggers |
| Blocks | Complete | `block`, `rescue`, `always` error handling |
| Roles | Complete | Full role structure support |
| Tags | Complete | Task filtering with `--tags`/`--skip-tags` |

### Connection Types

| Connection | Status | Description |
|------------|--------|-------------|
| SSH (russh) | Complete | Pure Rust SSH with connection pooling (11x faster) |
| SSH (ssh2) | Complete | libssh2 wrapper (legacy option) |
| Local | Complete | Direct localhost execution |
| Docker | Complete | Container execution via Bollard |
| Kubernetes | Planned | Pod execution via kube-rs |

### Native Modules (31 total)

**File Operations:**
- `file` - File/directory management
- `copy` - Copy files to remote
- `template` - Jinja2 template rendering
- `lineinfile` - Line manipulation in files
- `blockinfile` - Block manipulation in files
- `stat` - File statistics
- `archive` - Archive creation/extraction

**Command Execution:**
- `command` - Execute commands (no shell)
- `shell` - Execute shell commands

**Package Management:**
- `package` - Generic package manager abstraction
- `apt` - Debian/Ubuntu package management
- `yum` - RHEL/CentOS package management
- `dnf` - Fedora/RHEL 8+ package management
- `pip` - Python package management

**System Management:**
- `service` - Service control (systemd/init)
- `systemd_unit` - Systemd unit file management
- `user` - User account management
- `group` - Group management
- `hostname` - Hostname configuration
- `sysctl` - Kernel parameter management
- `mount` - Filesystem mounting
- `cron` - Cron job management

**Utility Modules:**
- `debug` - Debug message output
- `set_fact` - Set host facts
- `assert` - Condition assertions
- `include_vars` - Include variable files
- `facts` - Fact gathering
- `uri` - HTTP/HTTPS requests
- `git` - Git repository management

**Fallback:**
- `python` - Ansible Python module fallback (FQCN support)

### Execution Strategies

| Strategy | Status | Description |
|----------|--------|-------------|
| Linear | Complete | Task-by-task across all hosts (Ansible default) |
| Free | Complete | Maximum parallelism, hosts run independently |
| HostPinned | Complete | Dedicated worker per host (connection affinity) |

### Security Features

| Feature | Status | Description |
|---------|--------|-------------|
| Vault encryption | Complete | AES-256-GCM with Argon2id key derivation |
| Privilege escalation | Complete | `become`, `become_user`, `become_method` |
| SSH key authentication | Complete | RSA, Ed25519, ECDSA keys |
| Host key checking | Complete | Configurable strict/accept modes |

### Performance Metrics (v0.1)

- **SSH Connection Pooling**: 11x faster than Ansible
- **Simple playbook (10 hosts)**: 5.8x improvement
- **File copy (100 files)**: 5.6x improvement
- **Template rendering**: 5.3x improvement
- **Test coverage**: ~3,246 tests (99.1% pass rate)

---

## v0.2 - Planned Features

**Target**: Q1 2026
**Focus**: Stability, execution preview, parallelization intelligence

### Critical Path (Stabilization)

| Task | Priority | Description |
|------|----------|-------------|
| Fix remaining tests | Critical | Achieve 100% test pass rate |
| Ansible boolean compat | High | Fix 4 failing boolean tests |
| Block parsing | High | Fix 9 block parsing edge cases |
| Python/FQCN edge cases | High | Fix 10 FQCN resolution issues |
| CLI edge cases | Medium | Fix 3 CLI handling issues |

### ParallelizationHint Enforcement

Intelligent per-module concurrency control:

```rust
pub enum ParallelizationHint {
    FullyParallel,                           // Default - maximum parallelism
    HostExclusive,                           // apt/yum - one per host at a time
    RateLimited { requests_per_second: u32 }, // API calls - throttled
    GlobalExclusive,                          // Cluster-wide operations - serialized
}
```

| Module | Hint | Rationale |
|--------|------|-----------|
| file, copy, template | FullyParallel | No conflicts |
| apt, yum, dnf | HostExclusive | Package lock contention |
| uri (API calls) | RateLimited | API rate limits |
| cluster operations | GlobalExclusive | Data consistency |

### Execution Plan Preview

```bash
rustible plan playbook.yml -i inventory.yml

# Output
Execution Plan:
  web1.example.com:
    + [package] Install nginx (will install)
    ~ [template] Configure nginx.conf (will modify)
    - [file] Remove old config (will delete)

  web2.example.com:
    . [package] Install nginx (already installed)
    ~ [template] Configure nginx.conf (will modify)

Apply this plan? [y/N]
```

### Schema Validation

Parse-time validation of module arguments:

```rust
fn schema(&self) -> JsonSchema;

// Validate before execution
module.schema().validate(&task.args)?;
```

### State Manifest Foundation

```
~/.rustible/state/
├── web1.example.com.json
├── web2.example.com.json
└── db1.example.com.json
```

### New Modules Planned

| Module | Priority | Description |
|--------|----------|-------------|
| `wait_for` | High | Wait for conditions (port, file, regex) |
| `pause` | High | Pause execution with prompt |
| `fail` | Medium | Fail with custom message |
| `meta` | Medium | Meta actions (flush handlers, etc.) |
| `raw` | Medium | Raw command execution (no Python) |
| `script` | Medium | Transfer and execute script |
| `unarchive` | Medium | Extract archives on remote |
| `synchronize` | Low | rsync wrapper |

---

## v1.0 - Production Ready

**Target**: Q4 2026
**Focus**: Enterprise features, Nix-like guarantees, full Ansible compatibility

### State Management

**Drift Detection:**

```bash
rustible drift-check -i inventory.yml

# Output
web1.example.com: OK (no drift detected)
web2.example.com: DRIFTED
  - /etc/nginx/nginx.conf: modified (expected: abc, actual: xyz)
  - package nginx: version mismatch (expected: 1.18, actual: 1.20)
db1.example.com: OK
```

**State Caching:**

```rust
pub struct StateCache {
    entries: HashMap<StateKey, CachedResult>,
}

pub struct StateKey {
    module: String,
    params_hash: u64,      // Hash of module parameters
    host_facts_hash: u64,  // Relevant facts only
}
```

- Hash `(module_name, params, relevant_host_facts)` before execution
- Store result with timestamp
- On re-run: compare hash, skip if unchanged
- **Target**: "Instant" re-runs for unchanged configurations

**Lockfile Support:**

```yaml
# rustible.lock
version: 1
generated: 2026-01-15T10:30:00Z
modules:
  package: { version: "1.0.0", hash: "abc123" }
templates:
  nginx.conf.j2: { hash: "789ghi" }
variables:
  nginx_port: 80
```

### Advanced Execution

**Dependency Graph Execution (DAG):**

```yaml
- name: Install database
  package: name=postgresql
  provides: database

- name: Configure database
  template: src=pg.conf.j2
  requires: database
  provides: db_config

- name: Install app  # Runs in parallel with db tasks
  package: name=myapp
  provides: app

- name: Configure app
  template: src=app.conf.j2
  requires: [db_config, app]
```

**Transactional Rollback:**

```bash
rustible run playbook.yml --checkpoint

# If something goes wrong
rustible rollback web1.example.com --to checkpoint-20260115

# List checkpoints
rustible checkpoints web1.example.com
```

### Performance Enhancements

**Pipelined SSH:**

```rust
// Current: 1 command = 1 round-trip
ssh.execute("apt update").await?;
ssh.execute("apt install nginx").await?;

// Target: Pipeline multiple commands
ssh.pipeline(&[
    "apt update",
    "apt install -y nginx",
    "systemctl enable nginx",
]).await?;
```

Expected: 2-3x improvement on top of existing 11x (total: 20-30x)

**Native Module Bindings:**

| Module | Current | Target | Expected Gain |
|--------|---------|--------|---------------|
| apt | Shell out | libapt-pkg bindings | 1.5-2x |
| systemctl | Shell out | D-Bus bindings | 2x |
| user/group | Shell out | Native /etc/passwd | 1.5x |

**Binary Agent Mode:**

```bash
# Compile small Rust binary for target
rustible agent-build --target x86_64-unknown-linux-musl

# Deploy and use persistent agent
rustible run --agent-mode playbook.yml
```

### Full Ansible Compatibility

| Feature | Status | Target |
|---------|--------|--------|
| Playbook syntax | Complete | 100% |
| Module compatibility | ~85% | 95%+ |
| Ansible Galaxy | Planned | Full support |
| Callback plugins | Planned | Python plugin support |
| Dynamic inventory | Partial | Full plugin system |
| Lookup plugins | Partial | Full support |
| Filter plugins | Partial | Full Jinja2 filters |

### Connection Enhancements

| Connection | Status | Target |
|------------|--------|--------|
| WinRM | Planned | Windows remote management |
| Kubernetes | Planned | Pod execution |
| Podman | Planned | Rootless containers |
| AWS SSM | Planned | EC2 Session Manager |

---

## Community Feature Requests

We track community requests and prioritize based on demand and alignment with project goals.

### How to Submit a Feature Request

1. **GitHub Issues**: Open an issue with the `feature-request` label
2. **Discussions**: Start a discussion in GitHub Discussions
3. **Pull Requests**: Submit a PR with proposed implementation

### Current Requests (Vote with reactions!)

| Request | Votes | Status | Priority |
|---------|-------|--------|----------|
| Ansible Galaxy collection support | - | Planned (v1.0) | High |
| WinRM connection support | - | Planned (v1.0) | Medium |
| Web UI for playbook management | - | Under consideration | Low |
| Terraform integration | - | Under consideration | Medium |
| HashiCorp Vault integration | - | Under consideration | Medium |
| AWX/Tower API compatibility | - | Under consideration | Low |
| YAML anchor/alias support | - | Investigating | Medium |
| Custom callback plugin API | - | Planned (v1.0) | Medium |
| Parallel role execution | - | Investigating | Medium |
| Incremental fact gathering | - | Planned (v0.2) | High |

### Feature Request Template

```markdown
## Feature Request

**Is your feature request related to a problem?**
A clear description of the problem.

**Describe the solution you'd like**
A clear description of what you want to happen.

**Describe alternatives you've considered**
Other solutions or features you've considered.

**Use Case**
How would you use this feature?

**Additional context**
Any other context, mockups, or examples.
```

---

## Contributor Guidelines

We welcome contributions from the community! Here's how to get started.

### Getting Started

1. **Fork the repository**
   ```bash
   git clone https://github.com/YOUR_USERNAME/rustible.git
   cd rustible
   ```

2. **Set up development environment**
   ```bash
   # Install Rust (1.75+)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Build the project
   cargo build

   # Run tests
   cargo test
   ```

3. **Create a feature branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

### Development Workflow

**Code Standards:**

```bash
# Format code
cargo fmt

# Run lints
cargo clippy --all-features

# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run benchmarks
cargo bench
```

**Test Requirements:**
- All new features must have tests
- Maintain >95% code coverage for new code
- Integration tests for module changes
- Property-based tests where applicable

**Documentation:**
- Update relevant docs for API changes
- Add examples for new features
- Include rustdoc comments for public APIs

### Contribution Areas

**Good First Issues:**
- Bug fixes with clear reproduction steps
- Documentation improvements
- Test coverage improvements
- Error message enhancements

**Intermediate:**
- New module implementations
- Performance optimizations
- CLI enhancements
- Inventory plugin development

**Advanced:**
- Connection type implementations
- Execution strategy improvements
- Parser enhancements
- Security features

### Pull Request Process

1. **Create PR** with clear description
2. **Link related issues** using `Fixes #123`
3. **Ensure CI passes** - tests, lints, format
4. **Request review** from maintainers
5. **Address feedback** promptly
6. **Squash commits** before merge

### PR Template

```markdown
## Description
Brief description of changes.

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing performed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] No breaking changes (or documented)
```

### Code of Conduct

- Be respectful and inclusive
- Provide constructive feedback
- Focus on the code, not the person
- Help newcomers get started
- Credit others' contributions

### Communication Channels

- **GitHub Issues**: Bug reports, feature requests
- **GitHub Discussions**: Questions, ideas, community
- **Pull Requests**: Code contributions

---

## Performance Benchmarks

### Connection Pooling Results

| Metric | Ansible | Rustible | Improvement |
|--------|---------|----------|-------------|
| 100 tasks / 5 hosts | ~45s | ~4s | **11.25x** |
| Connection setup | Per-task | Pooled | N/A |
| Memory usage | ~200MB | ~50MB | 4x |

### Module Execution Performance

| Module | Ansible | Rustible | Improvement |
|--------|---------|----------|-------------|
| file (stat) | ~80ms | ~8ms | 10x |
| copy (small) | ~120ms | ~15ms | 8x |
| command | ~100ms | ~10ms | 10x |
| template | ~150ms | ~20ms | 7.5x |

### Scalability Testing

- **Tested**: Up to 50 concurrent hosts (Free strategy)
- **Result**: Linear scaling with forks limit
- **Connection Pool**: <10 active connections via reuse
- **Memory**: ~50MB base + ~2MB per active host

### Future Performance Targets

| Enhancement | Current | Target | Expected Gain |
|-------------|---------|--------|---------------|
| SSH pipelining | 11x | 20-30x | 2-3x |
| Native apt | 8x | 12-16x | 1.5-2x |
| State caching | N/A | "Instant" | 10-100x (unchanged) |

---

## Comparison Matrix

| Feature | Ansible | NixOS | Rustible v0.1 | Rustible v1.0 |
|---------|---------|-------|---------------|---------------|
| Speed | Slow | Fast | **11x faster** | 20-30x faster |
| Idempotency | Honor system | Guaranteed | Trait-enforced | + State tracking |
| Reproducibility | Best effort | Perfect | Basic | Lockfile-based |
| Rollback | Manual | Built-in | Not yet | Checkpoints |
| Drift detection | None | Implicit | Not yet | Explicit |
| Learning curve | Low | High | **Low** | Low |
| Existing infra | Works | Needs NixOS | **Works** | Works |

---

## Release Schedule

| Version | Target Date | Theme |
|---------|-------------|-------|
| v0.1.0 | Dec 2025 | MVP - Core functionality |
| v0.1.x | Jan 2026 | Bug fixes, stability |
| v0.2.0 | Q1 2026 | Execution preview, parallelization |
| v0.3.0 | Q2 2026 | State management, drift detection |
| v0.4.0 | Q3 2026 | Native bindings, performance |
| v1.0.0 | Q4 2026 | Production ready, full compatibility |

---

*Last updated: December 2025*

*For the latest updates, see [GitHub Releases](https://github.com/rustible/rustible/releases)*
