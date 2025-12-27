# Rustible

An async-first automation engine for infrastructure deployment and configuration management. Rustible is designed to optimize for safety and performance, offering type safety and speed with parallel execution by default and backwards compatibility.

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
- **Custom Python Plugins**: Custom Python callbacks not yet supported (native Rust callbacks available)

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

- **SSH**: Secure remote execution with connection pooling (pure Rust via `russh` or libssh2)
- **Local**: Direct local system execution without network overhead
- **Docker**: Execute tasks inside Docker containers
- **Kubernetes**: Execute tasks in Kubernetes pods (requires `kubernetes` feature)
- **WinRM**: Windows Remote Management support (requires `winrm` feature)

### Connection Resilience Features

- **Circuit Breaker**: Automatic failure detection and recovery
- **Retry Logic**: Exponential backoff with configurable policies
- **Jump Host/Bastion**: Multi-hop SSH connections
- **SSH Agent Forwarding**: Key forwarding support
- **Health Monitoring**: Connection health checks and diagnostics

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

### Feature Flags

Rustible uses Cargo feature flags to enable optional functionality:

```toml
[dependencies]
rustible = { version = "0.1", features = ["docker", "kubernetes"] }
```

| Feature | Description | Default |
|---------|-------------|---------|
| `russh` | Pure Rust SSH implementation (recommended) | Yes |
| `ssh2-backend` | libssh2-based SSH backend | No |
| `docker` | Docker container and compose modules | No |
| `kubernetes` | Kubernetes resource modules and connection | No |
| `winrm` | Windows Remote Management connection | No |
| `aws` | AWS cloud modules (EC2, S3) | No |
| `azure` | Azure cloud modules (VMs, Resource Groups) | No |
| `gcp` | Google Cloud modules (Compute Engine) | No |

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

Rustible includes a comprehensive set of native modules organized by category:

### Core Modules

| Module | Description |
|--------|-------------|
| `command` | Execute commands |
| `shell` | Execute shell commands |
| `debug` | Print debug messages |
| `set_fact` | Set host facts |
| `assert` | Assert conditions |
| `pause` | Pause execution |
| `wait_for` | Wait for conditions |
| `include_vars` | Include variables from files |
| `stat` | Get file statistics |

### File Management

| Module | Description |
|--------|-------------|
| `copy` | Copy files to remote |
| `template` | Template and copy files (Jinja2) |
| `file` | Manage file properties |
| `lineinfile` | Manage lines in files |
| `blockinfile` | Manage blocks in files |
| `archive` | Create compressed archives |
| `unarchive` | Extract compressed archives |

### Package Management

| Module | Description |
|--------|-------------|
| `package` | Universal package manager (auto-detects apt/yum/dnf) |
| `apt` | Debian/Ubuntu package management |
| `yum` | RHEL/CentOS package management |
| `dnf` | Fedora/RHEL 8+ package management |
| `pip` | Python package management |

### System Administration

| Module | Description |
|--------|-------------|
| `service` | Manage services |
| `systemd_unit` | Manage systemd units |
| `user` | Manage users |
| `group` | Manage groups |
| `cron` | Manage cron jobs |
| `mount` | Manage filesystem mounts |
| `hostname` | Manage system hostname |
| `sysctl` | Manage kernel parameters |
| `timezone` | Manage system timezone |

### Security Modules

| Module | Description |
|--------|-------------|
| `authorized_key` | Manage SSH authorized keys |
| `known_hosts` | Manage SSH known hosts |
| `ufw` | Ubuntu firewall management |
| `firewalld` | FirewallD management |
| `selinux` | SELinux configuration |

### Network Modules

| Module | Description |
|--------|-------------|
| `uri` | HTTP/HTTPS requests |
| `git` | Clone git repositories |

### Network Device Configuration

| Module | Description |
|--------|-------------|
| `ios_config` | Cisco IOS/IOS-XE configuration |
| `nxos_config` | Cisco NX-OS configuration |
| `junos_config` | Juniper Junos configuration |
| `eos_config` | Arista EOS configuration |

### Docker Modules (requires `docker` feature)

| Module | Description |
|--------|-------------|
| `docker_container` | Manage containers |
| `docker_image` | Manage images |
| `docker_network` | Manage networks |
| `docker_volume` | Manage volumes |
| `docker_compose` | Docker Compose projects |

### Kubernetes Modules (requires `kubernetes` feature)

| Module | Description |
|--------|-------------|
| `k8s_deployment` | Manage Deployments |
| `k8s_service` | Manage Services |
| `k8s_configmap` | Manage ConfigMaps |
| `k8s_secret` | Manage Secrets |
| `k8s_namespace` | Manage Namespaces |

### Windows Modules

| Module | Description |
|--------|-------------|
| `win_copy` | Copy files to Windows |
| `win_service` | Manage Windows services |
| `win_package` | Chocolatey/MSI packages |
| `win_user` | Manage Windows users |
| `win_feature` | Windows Features |

### Cloud Modules (require respective feature flags)

| Module | Feature | Description |
|--------|---------|-------------|
| `aws_ec2_instance` | `aws` | AWS EC2 instances |
| `aws_s3` | `aws` | AWS S3 buckets |
| `azure_vm` | `azure` | Azure Virtual Machines |
| `azure_resource_group` | `azure` | Azure Resource Groups |
| `gcp_compute_instance` | `gcp` | GCP Compute Engine |
| `gcp_compute_network` | `gcp` | GCP Networks |

**Python Fallback**: Any module not listed above automatically falls back to Ansible's Python execution via AnsiballZ-style bundling.

## Callback Plugins

Rustible includes a rich set of native callback plugins for customizing output:

### Core Output
| Plugin | Description |
|--------|-------------|
| `default` | Standard Ansible-like output with colors |
| `minimal` | Shows only failures and final recap (ideal for CI/CD) |
| `null` | Silent callback - no output |
| `oneline` | Compact single-line output for log files |
| `summary` | Summary-only output at playbook end |

### Visual
| Plugin | Description |
|--------|-------------|
| `progress` | Visual progress bars |
| `diff` | Shows before/after diffs for changed files |
| `dense` | Compact output for large inventories |
| `tree` | Hierarchical directory structure |

### Timing & Analysis
| Plugin | Description |
|--------|-------------|
| `timer` | Execution timing with summary |
| `profile_tasks` | Detailed task profiling with recommendations |
| `stats` | Comprehensive statistics collection |
| `context` | Task context with variables/conditions |
| `counter` | Task counting and tracking |

### Filtering
| Plugin | Description |
|--------|-------------|
| `skippy` | Minimizes skipped task output |
| `selective` | Filters by status, host, or patterns |
| `actionable` | Only shows changed/failed tasks |
| `full_skip` | Detailed skip analysis |

### Logging & Integration
| Plugin | Description |
|--------|-------------|
| `json` | JSON-formatted output |
| `yaml` | YAML-formatted output |
| `junit` | JUnit XML for CI/CD integration |
| `logfile` | File-based logging |
| `syslog` | System syslog integration |
| `slack` | Slack notifications |
| `splunk` | Splunk integration |
| `logstash` | Logstash integration |
| `mail` | Email notifications |

## Lookup Plugins

Built-in lookup plugins for retrieving data from external sources:

| Plugin | Description |
|--------|-------------|
| `file` | Read file contents |
| `env` | Read environment variables |
| `password` | Generate random passwords |
| `pipe` | Execute commands and capture output |
| `url` | Fetch content from HTTP/HTTPS URLs |

## Dynamic Inventory Plugins

Support for dynamic inventory from cloud providers:

| Plugin | Description |
|--------|-------------|
| `aws_ec2` | AWS EC2 instances inventory |
| `azure` | Azure Virtual Machines inventory |
| `gcp` | Google Cloud Compute Engine inventory |

Static inventory formats supported:
- YAML format
- INI format
- JSON format
- Dynamic scripts (executable inventory)

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
| GPU cluster bootstrap (8 nodes) | 4m 12s | 47s | 5.4x |

*Benchmarks performed on Ubuntu 22.04, 8-core CPU, 16GB RAM, SSH over LAN*

### Performance on GPU/HPC Infrastructure

Provisioning time matters when renting expensive infrastructure by the hour.

**Cost calculation based on benchmark data (8-node cluster bootstrap):**

| Tool | Time | Cluster Cost (@$296/hr) |
|------|------|-------------------------|
| Ansible | 4m 12s | $20.72 |
| Rustible | 47s | $3.87 |
| **Difference** | 3m 25s | **$16.85** |

*Assumes 8× DGX H100 nodes at ~$37/hour each. Actual rates vary by provider.*

For teams deploying clusters multiple times per day (development, testing, scaling), these savings accumulate.

#### Why Rustible is Faster

Both tools support parallel execution across hosts. The performance difference comes from:

1. **No interpreter overhead**: Rustible runs as a compiled binary. Ansible requires Python startup on both control node and targets.

2. **Lightweight concurrency**: Rustible uses async tasks on a thread pool. Ansible spawns separate Python processes per fork.

3. **Native modules**: Built-in modules execute as Rust code. Ansible transfers and interprets Python scripts on each host.

4. **Connection reuse**: SSH connections are pooled across tasks, reducing handshake overhead.

#### Example: GPU Node Bootstrap

```yaml
- name: Bootstrap GPU compute nodes
  hosts: gpu_cluster
  become: true

  tasks:
    - name: Install NVIDIA drivers and CUDA
      ansible.builtin.package:
        name:
          - nvidia-driver-550
          - cuda-toolkit-12-4
        state: present

    - name: Enable nvidia-persistenced
      ansible.builtin.service:
        name: nvidia-persistenced
        state: started
        enabled: true

    - name: Configure Docker for GPU support
      ansible.builtin.template:
        src: daemon.json.j2
        dest: /etc/docker/daemon.json
      notify: Restart Docker

  handlers:
    - name: Restart Docker
      ansible.builtin.service:
        name: docker
        state: restarted
```

```bash
rustible gpu-bootstrap.yml -i inventory.yml -f 50
```

## Project Structure

```
rustible/
├── src/
│   ├── lib.rs              # Library root
│   ├── main.rs             # CLI entry point
│   ├── cli/                # CLI implementation
│   ├── config.rs           # Configuration handling
│   ├── connection/         # Connection implementations
│   │   ├── russh.rs        # Pure Rust SSH (default)
│   │   ├── ssh.rs          # libssh2 backend
│   │   ├── local.rs        # Local execution
│   │   ├── docker.rs       # Docker containers
│   │   ├── kubernetes.rs   # Kubernetes pods
│   │   ├── winrm.rs        # Windows Remote Management
│   │   ├── circuit_breaker.rs  # Connection resilience
│   │   ├── jump_host.rs    # Bastion/jump host support
│   │   └── health.rs       # Health monitoring
│   ├── callback/           # Callback plugin system
│   │   ├── plugins/        # 25+ output plugins
│   │   └── manager.rs      # Callback orchestration
│   ├── error.rs            # Error types
│   ├── executor/           # Playbook execution
│   ├── facts.rs            # Fact gathering
│   ├── handlers.rs         # Handler management
│   ├── inventory/          # Inventory handling
│   │   ├── plugins/        # Dynamic inventory plugins
│   │   └── ...
│   ├── lookup/             # Lookup plugins
│   ├── modules/            # Built-in modules
│   │   ├── cloud/          # AWS, Azure, GCP modules
│   │   ├── docker/         # Docker modules
│   │   ├── k8s/            # Kubernetes modules
│   │   ├── network/        # Network device modules
│   │   ├── windows/        # Windows modules
│   │   └── ...             # Core modules
│   ├── parser/             # YAML parsing
│   ├── playbook.rs         # Playbook structures
│   ├── roles.rs            # Role handling
│   ├── strategy.rs         # Execution strategies
│   ├── tasks.rs            # Task definitions
│   ├── template.rs         # Template engine (minijinja)
│   ├── traits.rs           # Core traits
│   ├── vars/               # Variable handling
│   └── vault.rs            # Vault encryption (AES-256-GCM)
├── docs/                   # Documentation
├── tests/                  # Integration tests
├── benches/                # Benchmarks
├── examples/               # Example playbooks
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

### Completed Features
- [x] Core playbook execution
- [x] SSH connections with pooling (pure Rust via russh)
- [x] Local execution
- [x] Docker connections
- [x] Kubernetes connections (feature-gated)
- [x] WinRM connections (feature-gated)
- [x] Template engine (Jinja2-compatible via minijinja)
- [x] Vault encryption (AES-256-GCM)
- [x] Role support
- [x] Handler notifications
- [x] Dynamic inventory plugins (AWS EC2, Azure, GCP)
- [x] Callback plugins (25+ native plugins)
- [x] Lookup plugins (file, env, password, pipe, url)
- [x] Network device modules (Cisco IOS/NX-OS, Juniper, Arista)
- [x] Docker modules (container, image, network, volume, compose)
- [x] Kubernetes modules (deployment, service, configmap, secret, namespace)
- [x] Windows modules (copy, service, package, user, feature)
- [x] Cloud modules (AWS, Azure, GCP - feature-gated)
- [x] Circuit breaker and retry patterns
- [x] Jump host/bastion support
- [x] SSH agent forwarding

### Planned Features
- [ ] Ansible Galaxy support (CLI integration)
- [ ] Web UI
- [ ] Custom Python plugin support
- [ ] Database modules (PostgreSQL, MySQL - pending sqlx integration)

## License

Rustible is licensed under:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
