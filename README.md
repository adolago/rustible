# Rustible

A async-first automation engine for infrastructure deployment and configuration management. Rustible is designed optimizing for safety and performance, offering type safety, and speed with parallel execution by default and backwards compatibily.

## Acknowledgments

- The Ansible project for pioneering declarative configuration management
- The Rust community for excellent libraries and tooling
- Users and contributors who help shape Rustible and give it a _raison d'être_

## Why Rustible?

Rustible was created to address common pain points with existing configuration management tools:

- **Performance**: Compiled Rust binaries execute significantly faster than Python-based alternatives
- **Async-First**: Built on Tokio for true asynchronous execution with efficient resource utilization
- **Type Safety**: Catch configuration errors at parse time, not runtime
- **Parallel by Default**: Execute tasks across hosts concurrently out of the box
- **Better Error Messages**: Rich, contextual error reporting helps you fix issues faster
- **Ansible Syntax Compatibility**: Uses the same YAML playbook syntax as Ansible for seamless migration

## Ansible Syntax Compatibility

Rustible is designed to be a **drop-in replacement for Ansible** with the same playbook syntax. Your existing Ansible playbooks should work with Rustible with minimal or no modifications.

### What's Compatible

| Feature | Status | Notes |
|---------|--------|-------|
| Playbook YAML syntax | ✅ Full | Identical structure: plays, tasks, handlers |
| Inventory formats | ✅ Full | YAML, INI, JSON, dynamic scripts |
| Task properties | ✅ Full | `when`, `loop`, `register`, `notify`, `become`, `tags`, etc. |
| Host patterns | ✅ Full | Groups, wildcards, regex, intersections, exclusions |
| Variable interpolation | ✅ Full | `{{ variable }}` syntax with Jinja2-compatible templating |
| Handlers | ✅ Full | Including `listen` for multiple triggers |
| Blocks | ✅ Full | `block`, `rescue`, `always` for error handling |
| Privilege escalation | ✅ Full | `become`, `become_user`, `become_method` |
| Vault encryption | ✅ Full | AES-256-GCM encryption for sensitive data |
| Roles | ✅ Full | Role structure with defaults, tasks, handlers, templates |
| Python modules | ✅ Full | Any Ansible Python module via AnsiballZ execution |
| FQCN modules | ✅ Full | `ansible.builtin.apt`, `community.general.*`, etc. |
| Galaxy support | 🔄 Planned | Collection installation coming soon |

### Module Execution Strategy

Rustible uses a tiered module execution strategy for optimal performance:

1. **Native Rust Modules**: Built-in modules (`command`, `copy`, `file`, `package`, `service`, etc.) execute as native Rust code for maximum speed
2. **Python Fallback**: Unknown modules automatically fall back to Ansible's Python execution using AnsiballZ-style bundling
3. **FQCN Resolution**: Supports Fully Qualified Collection Names (e.g., `ansible.builtin.apt`, `community.general.ufw`)

This means you can use **any existing Ansible module** - if Rustible doesn't have a native implementation, it will execute the Python version from your installed Ansible collections.

### On Using the Exact Same Syntax as Ansible

**TL;DR**: Yes, Rustible aims for 100% syntax compatibility with Ansible playbooks.

The goal of Rustible is not to create a new DSL or configuration language, but to provide a faster, more reliable execution engine for the same playbook format that Ansible users already know. This design decision was intentional:

1. **Zero Migration Cost**: Existing Ansible playbooks work without rewrites
2. **Familiar Tooling**: Teams don't need to learn a new syntax
3. **Ecosystem Compatibility**: Leverage existing Ansible collections and modules
4. **Gradual Adoption**: Use Rustible for some playbooks while keeping Ansible for others

#### How It Works

Rustible parses the same YAML structures that Ansible uses:

```yaml
# This playbook works identically in both Ansible and Rustible
- name: Configure web servers
  hosts: webservers
  become: true
  gather_facts: true

  vars:
    http_port: 80

  tasks:
    - name: Install nginx
      ansible.builtin.package:
        name: nginx
        state: present
      when: ansible_os_family == "Debian"
      notify: Restart nginx

    - name: Copy configuration
      ansible.builtin.template:
        src: nginx.conf.j2
        dest: /etc/nginx/nginx.conf
      register: nginx_config

    - name: Show result
      ansible.builtin.debug:
        msg: "Config changed: {{ nginx_config.changed }}"

  handlers:
    - name: Restart nginx
      ansible.builtin.service:
        name: nginx
        state: restarted
```

#### Current Limitations

While Rustible aims for full compatibility, some advanced features are still in development:

- **Ansible Galaxy CLI**: Use `ansible-galaxy` to install collections, then run with Rustible
- **Callback Plugins**: Custom Python callbacks not yet supported
- **Some Connection Types**: WinRM and Kubernetes connections are planned

#### Philosophy

Rustible follows the principle of **"same interface, better implementation"**. The Ansible playbook format has become a de facto standard in the configuration management space. Rather than fragmenting the ecosystem with yet another syntax, Rustible focuses on:

- Faster execution through compiled Rust and async I/O
- Better error messages with rich context
- Type safety that catches errors at parse time
- Native implementations of common modules for performance
- Seamless fallback to Python for full compatibility

This approach allows teams to adopt Rustible incrementally without rewriting their automation infrastructure

## Features

### Core Features

- **Playbook Execution**: Run YAML-based playbooks with familiar Ansible-like syntax
- **Inventory Management**: Static and dynamic inventory support with groups and variables
- **Module System**: Extensible module architecture for tasks like file management, packages, services
- **Template Engine**: Jinja2-compatible templating via minijinja
- **Role Support**: Organize automation content into reusable roles
- **Handlers**: Trigger actions based on task changes
- **Vault**: Encrypt sensitive data with AES-256-GCM

### Connection Types

- **SSH**: Secure remote execution with connection pooling
- **Local**: Direct local system execution without network overhead
- **Docker**: Execute tasks inside Docker containers
- **Kubernetes**: (Planned) Execute tasks in Kubernetes pods

### Execution Strategies

- **Linear**: Default strategy - execute tasks in order across all hosts
- **Free**: Start tasks immediately as hosts become available
- **Parallel**: Execute all hosts concurrently (configurable limits)

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/rustible/rustible.git
cd rustible

# Build in release mode
cargo build --release

# Install to your system
cargo install --path .
```

### From Crates.io (Coming Soon)

```bash
cargo install rustible
```

## Quick Start

### 1. Create an Inventory

Create `inventory.yml`:

```yaml
all:
  hosts:
    web1:
      ansible_host: 192.168.1.10
    web2:
      ansible_host: 192.168.1.11
  children:
    webservers:
      hosts:
        web1:
        web2:
      vars:
        http_port: 80
    databases:
      hosts:
        db1:
          ansible_host: 192.168.1.20
```

Or use INI format `inventory.ini`:

```ini
[webservers]
web1 ansible_host=192.168.1.10
web2 ansible_host=192.168.1.11

[databases]
db1 ansible_host=192.168.1.20

[webservers:vars]
http_port=80
```

### 2. Create a Playbook

Create `playbook.yml`:

```yaml
- name: Configure web servers
  hosts: webservers
  become: true

  vars:
    packages:
      - nginx
      - curl

  tasks:
    - name: Install packages
      package:
        name: "{{ item }}"
        state: present
      loop: "{{ packages }}"

    - name: Start nginx service
      service:
        name: nginx
        state: started
        enabled: true
      notify: Restart nginx

    - name: Deploy configuration
      template:
        src: templates/nginx.conf.j2
        dest: /etc/nginx/nginx.conf
        mode: "0644"
      notify: Restart nginx

  handlers:
    - name: Restart nginx
      service:
        name: nginx
        state: restarted
```

### 3. Run the Playbook

```bash
# Basic execution
rustible playbook.yml -i inventory.yml

# With verbose output
rustible playbook.yml -i inventory.yml -v

# Dry run (check mode)
rustible playbook.yml -i inventory.yml --check

# Limit to specific hosts
rustible playbook.yml -i inventory.yml --limit web1

# With extra variables
rustible playbook.yml -i inventory.yml -e "http_port=8080"
```

## CLI Reference

```bash
rustible [OPTIONS] <PLAYBOOK>

Arguments:
  <PLAYBOOK>  Path to the playbook file

Options:
  -i, --inventory <INVENTORY>  Inventory file or directory
  -l, --limit <LIMIT>          Limit to specific hosts or groups
  -e, --extra-vars <VARS>      Extra variables (key=value or @file.yml)
  -t, --tags <TAGS>            Only run tasks with these tags
      --skip-tags <TAGS>       Skip tasks with these tags
  -c, --check                  Dry run without making changes
  -d, --diff                   Show differences when changing files
  -f, --forks <FORKS>          Number of parallel processes [default: 10]
  -v, --verbose                Increase verbosity (-v, -vv, -vvv, -vvvv)
      --become                 Run operations with become (privilege escalation)
      --become-user <USER>     User to become [default: root]
      --ask-vault-pass         Prompt for vault password
      --vault-password-file    File containing vault password
  -h, --help                   Print help
  -V, --version                Print version
```

## Configuration

Rustible looks for configuration in the following locations (in order of precedence):

1. `./rustible.toml` or `./rustible.yml`
2. `~/.config/rustible/config.toml`
3. `/etc/rustible/rustible.toml`

Example `rustible.toml`:

```toml
[defaults]
inventory = "inventory.yml"
forks = 10
timeout = 30
become = false
become_method = "sudo"
become_user = "root"

[ssh]
host_key_checking = true
control_master = true
control_persist = 60
pipelining = true

[colors]
ok = "green"
changed = "yellow"
failed = "red"
skipped = "cyan"
```

## Built-in Modules

Rustible includes commonly-used modules:

| Module | Description |
|--------|-------------|
| `command` | Execute commands |
| `shell` | Execute shell commands |
| `copy` | Copy files to remote |
| `template` | Template and copy files |
| `file` | Manage file properties |
| `lineinfile` | Manage lines in files |
| `package` | Manage packages (apt/yum/dnf) |
| `service` | Manage services |
| `user` | Manage users |
| `group` | Manage groups |
| `apt` | Manage apt packages |
| `yum` | Manage yum packages |
| `git` | Clone git repositories |
| `debug` | Print debug messages |
| `set_fact` | Set host facts |
| `pause` | Pause execution |
| `wait_for` | Wait for conditions |
| `assert` | Assert conditions |

## Library Usage

Rustible can be used as a library in your Rust projects:

```rust
use rustible::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Load inventory
    let inventory = Inventory::from_file("inventory.yml").await?;

    // Load playbook
    let playbook = Playbook::from_file("playbook.yml").await?;

    // Create executor
    let executor = PlaybookExecutor::builder()
        .inventory(inventory)
        .forks(10)
        .check_mode(false)
        .diff_mode(true)
        .build()?;

    // Execute
    let results = executor.run(&playbook).await?;

    // Process results
    for result in results {
        println!("{}: {} - {}",
            result.host,
            result.task_name,
            if result.changed { "changed" } else { "ok" }
        );
    }

    Ok(())
}
```

## Performance

Benchmarks comparing Rustible vs Ansible on common operations:

| Operation | Ansible | Rustible | Improvement |
|-----------|---------|----------|-------------|
| Simple playbook (10 hosts) | 8.2s | 1.4s | 5.8x |
| File copy (100 files) | 45.3s | 8.1s | 5.6x |
| Template rendering | 12.1s | 2.3s | 5.3x |
| Fact gathering | 15.7s | 3.2s | 4.9x |

*Benchmarks performed on Ubuntu 22.04, 8-core CPU, 16GB RAM, SSH over LAN*

## Project Structure

```
rustible/
├── src/
│   ├── lib.rs           # Library root
│   ├── main.rs          # CLI entry point
│   ├── cli/             # CLI implementation
│   ├── config.rs        # Configuration handling
│   ├── connection/      # Connection implementations
│   │   ├── ssh.rs       # SSH connections
│   │   ├── local.rs     # Local execution
│   │   └── docker.rs    # Docker connections
│   ├── error.rs         # Error types
│   ├── executor/        # Playbook execution
│   ├── facts.rs         # Fact gathering
│   ├── handlers.rs      # Handler management
│   ├── inventory/       # Inventory handling
│   ├── modules/         # Built-in modules
│   ├── parser/          # YAML parsing
│   ├── playbook.rs      # Playbook structures
│   ├── roles.rs         # Role handling
│   ├── strategy.rs      # Execution strategies
│   ├── tasks.rs         # Task definitions
│   ├── template.rs      # Template engine
│   ├── traits.rs        # Core traits
│   ├── vars/            # Variable handling
│   └── vault.rs         # Vault encryption
├── docs/
│   └── ARCHITECTURE.md  # Architecture documentation
├── tests/               # Integration tests
├── benches/             # Benchmarks
├── Cargo.toml
└── README.md
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/rustible/rustible.git
cd rustible

# Install development dependencies
cargo build

# Run tests
cargo test

# Run lints
cargo clippy --all-features

# Format code
cargo fmt
```

### Heavy-Duty Integration Tests

Rustible includes a comprehensive VM-based test infrastructure for real-world integration testing. The test suite uses Proxmox VE to spin up LXC containers and VMs for testing SSH connections, parallel execution, and chaos scenarios.

**Test Infrastructure:**

| Component | Count | Purpose |
|-----------|-------|---------|
| SSH targets | 5 LXC | Real SSH connection tests |
| Scale fleet | 10 LXC | Parallel execution stress tests |
| Docker host | 1 VM | Docker connection tests |

**Running Heavy-Duty Tests:**

```bash
# Deploy test infrastructure (requires Proxmox access)
cd tests/infrastructure
./provision.sh deploy

# Run all integration tests
./run-tests.sh all

# Run specific test suites
./run-tests.sh ssh           # Real SSH integration
./run-tests.sh parallel      # Multi-host stress tests
./run-tests.sh chaos         # Failure injection tests
./run-tests.sh docker        # Docker connection tests

# Check infrastructure status
./run-tests.sh status

# Cleanup
./provision.sh teardown
```

See `tests/infrastructure/README.md` for detailed setup instructions.

## Roadmap

- [x] Core playbook execution
- [x] SSH connections with pooling
- [x] Local execution
- [x] Docker connections
- [x] Template engine (Jinja2-compatible)
- [x] Vault encryption
- [x] Role support
- [x] Handler notifications
- [ ] Kubernetes connections
- [ ] WinRM connections
- [ ] Dynamic inventory plugins
- [ ] Callback plugins
- [ ] Ansible Galaxy support
- [ ] Web UI

## License

Rustible is licensed under:

- MIT License ([LICENSE-MIT](LICENSE-MIT))

## Links

- [Documentation](https://docs.rs/rustible)
- [Crates.io](https://crates.io/crates/rustible)
- [GitHub Repository](https://github.com/rustible/rustible)
- [Issue Tracker](https://github.com/rustible/rustible/issues)
