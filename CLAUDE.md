# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rustible is an async-first configuration management and automation tool written in Rust, designed as a high-performance alternative to Ansible. It provides Ansible-compatible YAML playbook syntax while offering improved speed, type safety, and parallel execution.

## Build & Development Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build

# Run tests
cargo test                     # Run all tests
cargo test <test_name>         # Run specific test
cargo test -- --nocapture      # Show test output

# Linting and formatting
cargo clippy --all-features    # Run clippy lints
cargo fmt                      # Format code

# Run the CLI
cargo run -- run playbook.yml -i inventory.yml
cargo run -- check playbook.yml -i inventory.yml     # Dry-run
cargo run -- validate playbook.yml                   # Validate syntax
cargo run -- list-hosts -i inventory.yml             # List inventory hosts

# Feature flags
cargo build --features docker      # Enable Docker connection support
cargo build --features kubernetes  # Enable Kubernetes support
cargo build --features full        # Enable all features
```

## Architecture

### Core Execution Flow

1. **CLI Layer** (`src/cli/`) - clap-based argument parsing, subcommand routing
2. **Playbook Parsing** (`src/playbook.rs`, `src/parser/`) - YAML deserialization into typed Rust structs
3. **Inventory Resolution** (`src/inventory/`) - Host/group loading from YAML/INI files
4. **Execution Engine** (`src/executor/`) - Orchestrates task execution across hosts
5. **Connection Layer** (`src/connection/`) - Transport abstraction (SSH, local, Docker)
6. **Module System** (`src/modules/`) - Individual task implementations

### Key Abstractions (src/traits.rs)

- **`Module` trait** - Units of work (copy, package, service, etc.). Modules must be idempotent.
- **`Connection` trait** - Transport for command execution and file transfer to targets
- **`ExecutionStrategy` trait** - How tasks distribute across hosts (linear, free, parallel)
- **`InventorySource` trait** - Loading hosts/groups from various sources
- **`ExecutionCallback` trait** - Hooks for execution events (for custom output)

### Module Result Pattern

All modules return `ModuleResult` with: `success`, `changed`, `skipped`, `message`, and optional `data`. Use factory methods: `ModuleResult::ok()`, `ModuleResult::changed()`, `ModuleResult::failed()`, `ModuleResult::skipped()`.

### Template Engine

Uses MiniJinja for Jinja2-compatible templating (`src/template.rs`). Supports variable interpolation, filters, conditionals, loops, includes, and macros.

### Variable Precedence (lowest to highest)

Role defaults → inventory group_vars → inventory host_vars → playbook vars → play vars → role vars → task vars → extra vars (-e)

## Code Conventions

- Async-first: All I/O uses Tokio async/await
- Warn on `missing_docs` and `clippy::pedantic` - maintain documentation
- Use `thiserror` for error types, `anyhow` for application errors
- Connection pooling is handled automatically; prefer reusing connections

## CLI Subcommands

- `run` - Execute a playbook
- `check` - Dry-run execution
- `validate` - Validate playbook syntax
- `list-hosts` - List inventory hosts
- `list-tasks` - List playbook tasks
- `vault` - Encrypt/decrypt secrets
- `init` - Initialize new project structure
