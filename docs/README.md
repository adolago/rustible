# Rustible Documentation

**Comprehensive documentation for the Rustible automation engine**

---

## Documentation Structure

```
docs/
  architecture/     - System design and architecture decisions
  guides/           - User guides and tutorials
  reference/        - API and module reference
  development/      - Developer documentation
  archive/          - Historical reports and implementation notes
```

---

## Quick Navigation

### Getting Started

- **[guides/quick-start.md](./guides/quick-start.md)** - Get up and running
- **[guides/cli-reference.md](./guides/cli-reference.md)** - Command-line interface
- **[guides/migration-from-ansible.md](./guides/migration-from-ansible.md)** - Migration guide

### User Guides

- **[guides/](./guides/)** - Complete user documentation
  - [01-introduction.md](./guides/01-introduction.md) - Introduction to Rustible
  - [02-playbooks.md](./guides/02-playbooks.md) - Working with playbooks
  - [best-practices.md](./guides/best-practices.md) - Best practices
  - [performance-tuning.md](./guides/performance-tuning.md) - Performance tuning
  - [troubleshooting.md](./guides/troubleshooting.md) - Troubleshooting guide
  - [container-guide.md](./guides/container-guide.md) - Docker/container usage

### Reference

- **[reference/](./reference/)** - Technical reference
  - [variables.md](./reference/variables.md) - Variable system
  - [inventory.md](./reference/inventory.md) - Inventory management
  - [performance.md](./reference/performance.md) - Performance benchmarks
  - [modules/](./reference/modules/) - Module documentation
  - [callbacks.md](./reference/callbacks.md) - Callback plugins
  - [modules.md](./reference/modules.md) - Module API reference

### Architecture

- **[architecture/](./architecture/)** - System design
  - [ARCHITECTURE.md](./architecture/ARCHITECTURE.md) - Main architecture overview
  - [0001-architecture-overview.md](./architecture/0001-architecture-overview.md) - ADR: Architecture
  - [0002-module-system-design.md](./architecture/0002-module-system-design.md) - ADR: Modules
  - [0003-callback-plugin-architecture.md](./architecture/0003-callback-plugin-architecture.md) - ADR: Callbacks
  - [REGISTRY_ARCHITECTURE.md](./architecture/REGISTRY_ARCHITECTURE.md) - Registry design
  - [distributed-execution.md](./architecture/distributed-execution.md) - Distributed execution
  - [terraform-integration.md](./architecture/terraform-integration.md) - Terraform integration
  - [web-ui.md](./architecture/web-ui.md) - Web UI design

### Development

- **[development/](./development/)** - Developer resources
  - [CONTRIBUTING.md](./development/CONTRIBUTING.md) - Contribution guide
  - [custom-modules.md](./development/custom-modules.md) - Writing custom modules
  - [callback-plugins.md](./development/callback-plugins.md) - Callback plugin development
  - [connection-plugins.md](./development/connection-plugins.md) - Connection plugin development

### Planning

- **[ROADMAP.md](./ROADMAP.md)** - Development roadmap and milestones

---

## Performance Highlights

| Metric | Value | Details |
|--------|-------|---------|
| Connection pooling speedup | **11x faster** | [reference/performance.md](./reference/performance.md) |
| Overall execution speedup | **5.3x faster** | vs Ansible |
| Parallel scaling | **2x better** | Multi-host operations |
| Memory efficiency | **3.7x less** | Memory usage |

```
Rustible vs Ansible (5 hosts, 10 tasks each)

Execution Time:
Ansible:  ========================== 47.3s
Rustible: ===== 8.9s (5.3x faster)

Memory Usage:
Ansible:  ================ 156 MB
Rustible: ==== 42 MB (3.7x less)
```

---

## Architecture Overview

```
                      Rustible Architecture

  Playbook Parser  -->  Inventory Parser  -->  Variables Context
         |                    |                       |
         +--------------------+-----------------------+
                              |
                         Executor (async/await)
                              |
         +--------------------+-----------------------+
         |                    |                       |
    Linear Strategy    Free Strategy     HostPinned Strategy
         |                    |                       |
         +--------------------+-----------------------+
                              |
                       Connection Pool (11x speedup)
                              |
         +--------------------+-----------------------+
         |                    |                       |
    SSH (russh)       Docker (bollard)         Local (tokio)
                              |
                       Module Registry
              (command, copy, template, package, ...)
```

See [architecture/ARCHITECTURE.md](./architecture/ARCHITECTURE.md) for detailed documentation.

---

## Getting Started

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

See [guides/quick-start.md](./guides/quick-start.md) for complete instructions.

---

## Archive

Historical documentation, sprint reports, and implementation notes are preserved in:

- **[archive/](./archive/)** - Old reports, security audits, coverage analysis

---

## Contributing

See [development/CONTRIBUTING.md](./development/CONTRIBUTING.md) for guidelines.

---

**Rustible** - Ansible compatibility with Rust performance

*Last updated: 2025-12-27*
