# Ansible Compatibility Matrix

> **Last Updated:** 2026-01-26
> **Rustible Version:** 0.1.x

This document tracks Rustible's compatibility with Ansible features, modules, and behaviors.

---

## Feature Flags

Enable features in `Cargo.toml` or via build flags:

```bash
# Default build (pure Rust SSH)
cargo build --release

# With Docker support
cargo build --release --features docker

# With Kubernetes support
cargo build --release --features kubernetes

# With AWS cloud modules
cargo build --release --features aws

# Full feature set
cargo build --release --features full

# Full with all cloud providers (includes experimental)
cargo build --release --features full-cloud
```

| Feature Flag | Status | Description |
|--------------|--------|-------------|
| `russh` (default) | Stable | Pure Rust SSH backend |
| `ssh2-backend` | Stable | Legacy libssh2 wrapper |
| `local` (default) | Stable | Local connection |
| `docker` | Stable | Docker container execution |
| `kubernetes` | Stable | Kubernetes pod execution |
| `aws` | Stable | AWS cloud modules (EC2, S3, IAM) |
| `azure` | Experimental | Azure cloud modules (stub) |
| `gcp` | Experimental | GCP cloud modules (stub) |
| `database` | Experimental | Database modules (disabled) |
| `winrm` | Experimental | Windows Remote Management |
| `provisioning` | Experimental | Terraform-like provisioning |

---

## Core Execution Features

| Feature | Ansible | Rustible | Notes |
|---------|---------|----------|-------|
| Playbook parsing | Yes | Yes | Full YAML support |
| Inventory (YAML/INI/JSON) | Yes | Yes | All formats supported |
| Dynamic inventory scripts | Yes | Yes | JSON output expected |
| Variable precedence | Yes | Yes | Full 22-level chain |
| Jinja2 templates | Yes | Yes | Via MiniJinja |
| Handlers | Yes | Yes | Including `listen` |
| Blocks (block/rescue/always) | Yes | Yes | Full support |
| Roles | Yes | Yes | Full structure |
| Tags | Yes | Yes | `--tags`/`--skip-tags` |
| Fact gathering | Yes | Yes | `gather_facts`/`setup` |
| Privilege escalation | Yes | Beta | `become` reaches remote targets for modules that pass it into their commands; transfer-based modules and control-node escalation are refused |
| Vault encryption | Yes | Yes | Different format (AES-256-GCM) |
| Check mode | Yes | Yes | `--check` flag |
| Diff mode | Yes | Yes | `--diff` flag |
| Async tasks (`async_tasks`) | Yes | Partial | Beta async execution |
| Delegation (`delegate_to`) | Yes | Yes | Targeted host delegation |
| Run once (`run_once`) | Yes | Yes | Single host execution |
| SSH pipelining (`ssh_pipelining`) | Yes | Beta | Fact gathering is batched into one round trip; general task pipelining is not wired |
| Declared task order (`provides`/`requires`) | No | Yes | Rustible extension: stable topological ordering within a play |

---

## Execution Strategies

| Strategy | Ansible | Rustible | Notes |
|----------|---------|----------|-------|
| `linear` | Yes | Yes | Default, task-by-task |
| `free` | Yes | Yes | Maximum parallelism |
| `host_pinned` | Yes | Yes | Connection affinity |
| `serial` | Yes | Yes | Batch execution (serial_execution) |
| `debug` | Yes | No | Planned |

---

## Connection Types

| Connection | Ansible | Rustible | Feature Flag | Notes |
|------------|---------|----------|--------------|-------|
| SSH | Yes | Yes | `russh` (default) | 11x faster with pooling |
| SSH (libssh2) | Yes | Yes | `ssh2-backend` | Legacy option |
| Local | Yes | Yes | `local` (default) | Direct execution |
| Docker | Yes | Yes | `docker` | Via Bollard |
| Kubernetes | Yes | Yes | `kubernetes` | Via kube-rs |
| WinRM | Yes | Partial | `winrm` | Experimental |
| Podman | Yes | Yes | none (always built) | Rootless containers, via the `podman` CLI |
| AWS SSM | Yes | Yes | `aws` | EC2 Session Manager |

---

## Provisioning and Agent Features

| Feature | Ansible | Rustible | Notes |
|---------|---------|----------|-------|
| Resource graph (`resource_graph`) | No | Partial | Terraform-like dependencies |
| State management (`state_management`) | No | Partial | Terraform-style state tracking |
| Drift detection (`drift_detection`) | No | Yes | `drift detect` checks each host through its own connection |
| Agent mode (`agent_mode`) | No | Partial | `agent build/deploy/status/stop` and `run --agent-mode`; the deployed agent is one-shot per command |
| Native bindings (`native_bindings`) | No | Partial | Local user, group and package checks read the system databases directly |
| Checkpoints/rollback (`checkpoints`) | No | Yes | Checkpoint and rollback support, 18 tests |

---

## Module Compatibility

Module availability is one question; running a module against a *remote* host
is another. Modules that reach the executor's dispatch carry one of three
classifications, and `tests/remote_transport_coverage_tests.rs` fails if one of
them carries none — including under a feature build, where the gated HPC,
cloud and database modules register and are classified control-node-only. One
group sits outside that check: the seven modules the executor handles before
dispatch without any connection (`assert`, `debug`, `fail`, `include_vars`,
`meta`, `pause`, `set_fact`), which the test exempts by name.

- **Verified** — routes all of its work through the connection and has been
  exercised against a live SSH target. See `Task::REMOTE_VERIFIED_MODULES`.
- **Connection-only** — routes all of its work through the connection, but no
  environment here can exercise it end to end (systemd, firewalls, RPM
  distributions, switches, PostgreSQL).
- **Control-node-only** — does its work on the control node by design, whether
  through `std::fs` or its own protocol. A remote task is refused with a
  pointer to `delegate_to: localhost` rather than run against the wrong
  machine.

`docs/FEATURE_STATUS.md` carries the current lists, including which of the verified entries are actually exercised against a live target and which rest on a source review of their transport path.

`replace`, `fetch`, `slurp`, `unarchive`, `wait_for`, `archive` and `raw` were
added to the verified set alongside the existing modules, each with a live-target test.

### Stable Modules (No Feature Flag Required)

#### Package Management
| Module | Ansible | Rustible | Test Coverage |
|--------|---------|----------|---------------|
| `apt` | Yes | Yes | 30 tests |
| `yum` | Yes | Yes | 30 tests |
| `dnf` | Yes | Yes | 27 tests |
| `pip` | Yes | Yes | 34 tests |
| `package` | Yes | Yes | 36 tests |

#### File Operations
| Module | Ansible | Rustible | Test Coverage |
|--------|---------|----------|---------------|
| `file` | Yes | Yes | Needs tests |
| `copy` | Yes | Yes | Needs tests |
| `template` | Yes | Yes | Needs tests |
| `lineinfile` | Yes | Yes | Needs tests |
| `blockinfile` | Yes | Yes | Needs tests |
| `stat` | Yes | Yes | 19 tests |
| `archive` | Yes | Yes | 17 tests plus live remote packing |
| `unarchive` | Yes | Yes | Unit tests plus live remote extraction |

#### Command Execution
| Module | Ansible | Rustible | Test Coverage |
|--------|---------|----------|---------------|
| `command` | Yes | Yes | 31 tests |
| `shell` | Yes | Yes | 22 tests |
| `raw` | Yes | No | Planned for v0.2 |
| `script` | Yes | No | Planned for v0.2 |

#### System Administration
| Module | Ansible | Rustible | Test Coverage |
|--------|---------|----------|---------------|
| `service` | Yes | Yes | 27 tests |
| `systemd` / `systemd_unit` | Yes | Yes | 41 tests |
| `user` | Yes | Yes | 35 tests |
| `group` | Yes | Yes | 28 tests |
| `hostname` | Yes | Yes | Needs tests |
| `sysctl` | Yes | Yes | Needs tests |
| `mount` | Yes | Yes | Needs tests |
| `cron` | Yes | Yes | Needs tests |
| `timezone` | Yes | Yes | 23 tests |

#### Security & Firewall
| Module | Ansible | Rustible | Test Coverage |
|--------|---------|----------|---------------|
| `authorized_key` | Yes | Yes | 87 tests |
| `known_hosts` | Yes | Yes | 78 tests |
| `ufw` | Yes | Yes | 75 tests |
| `firewalld` | Yes | Yes | 60 tests |
| `selinux` | Yes | Yes | 27 tests |

#### Network & HTTP
| Module | Ansible | Rustible | Test Coverage |
|--------|---------|----------|---------------|
| `uri` | Yes | Yes | 25 tests |
| `wait_for` | Yes | Yes | 37 tests plus live remote port/path checks |
| `get_url` | Yes | No | Use `uri` |

#### Utility & Logic
| Module | Ansible | Rustible | Test Coverage |
|--------|---------|----------|---------------|
| `debug` | Yes | Yes | Needs tests |
| `set_fact` | Yes | Yes | Needs tests |
| `assert` | Yes | Yes | Needs tests |
| `fail` | Yes | No | Planned for v0.2 |
| `meta` | Yes | No | Planned for v0.2 |
| `include_vars` | Yes | Yes | Needs tests |
| `pause` | Yes | Yes | 31 tests |
| `git` | Yes | Yes | 23 tests |

### Feature-Gated Modules

#### Docker Modules (`--features docker`)
| Module | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `docker_container` | Yes | Yes | Via Bollard |
| `docker_image` | Yes | Yes | Via Bollard |
| `docker_network` | Yes | Yes | Via Bollard |
| `docker_volume` | Yes | Yes | Via Bollard |
| `docker_compose` | Yes | Yes | Via Bollard |

#### Kubernetes Modules (`--features kubernetes`)
| Module | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `k8s` | Yes | Partial | Via kube-rs |
| `k8s_deployment` | Yes | Yes | - |
| `k8s_service` | Yes | Yes | - |
| `k8s_configmap` | Yes | Yes | - |
| `k8s_secret` | Yes | Yes | - |
| `k8s_namespace` | Yes | Yes | - |

#### AWS Cloud Modules (`--features aws`)
| Module | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `ec2` / `aws_ec2` | Yes | Yes | Via AWS SDK |
| `s3` / `aws_s3` | Yes | Yes | Via AWS SDK |
| `iam_role` | Yes | Partial | Via AWS SDK |
| `iam_policy` | Yes | Partial | Via AWS SDK |

#### Azure Cloud Modules (`--features azure`) - Experimental
| Module | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `azure_rm_virtualmachine` | Yes | Stub | Experimental |

#### GCP Cloud Modules (`--features gcp`) - Experimental
| Module | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `gcp_compute_instance` | Yes | Stub | Experimental |

#### Network Device Modules (Always Available)
| Module | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `ios_config` | Yes | Yes | Cisco IOS |
| `eos_config` | Yes | Yes | Arista EOS |
| `junos_config` | Yes | Yes | Juniper Junos |
| `nxos_config` | Yes | Yes | Cisco NX-OS |

#### Windows Modules (`--features winrm`) - Experimental
| Module | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `win_copy` | Yes | Partial | Requires WinRM |
| `win_feature` | Yes | Partial | Requires WinRM |
| `win_service` | Yes | Partial | Requires WinRM |
| `win_package` | Yes | Partial | Requires WinRM |
| `win_user` | Yes | Partial | Requires WinRM |

#### Database Modules (`--features database`) - Disabled
| Module | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `postgresql_db` | Yes | Disabled | Pending sqlx integration |
| `postgresql_user` | Yes | Disabled | Pending sqlx integration |
| `mysql_db` | Yes | Disabled | Pending sqlx integration |
| `mysql_user` | Yes | Disabled | Pending sqlx integration |

---

## Jinja2 Filter Compatibility

See [jinja2-filters.md](jinja2-filters.md) for the comprehensive filter gap list.

### Fully Supported Filters (40+)

**String:** `default`/`d`, `lower`, `upper`, `capitalize`, `title`, `trim`, `replace`, `regex_replace`, `regex_search`, `split`, `join`, `quote`

**List:** `first`, `last`, `length`/`count`, `unique`, `sort`, `reverse`, `flatten`, `list`, `selectattr`, `rejectattr`, `map`

**Dict:** `combine`, `dict2items`, `items2dict`

**Type:** `int`, `float`, `string`, `bool`, `list`

**Path:** `basename`, `dirname`, `expanduser`, `realpath`

**Encoding:** `b64encode`, `b64decode`, `to_json`, `to_nice_json`, `from_json`, `to_yaml`, `to_nice_yaml`, `from_yaml`, `from_yaml_all`

**Ansible-Specific:** `mandatory`, `ternary`

### Planned Filters (High Priority)

- `min` / `max` / `sum` - Expose MiniJinja builtins
- `regex_findall` - Find all regex matches
- `password_hash` - Password hashing
- `ipaddr` - IP address manipulation

### Not Yet Supported

- `ansible_vault` - Different vault format
- `json_query` - JMESPath queries
- `subelements` - Nested loop helper

---

## Lookup Plugins

| Plugin | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `file` | Yes | Yes | Read file contents |
| `env` | Yes | Yes | Environment variables |
| `password` | Yes | Yes | Random passwords |
| `pipe` | Yes | Yes | Command output |
| `url` | Yes | Yes | HTTP/HTTPS fetch |
| `template` | Yes | No | Planned |
| `items` | Yes | Yes | List iteration |

---

## Dynamic Inventory Plugins

| Plugin | Ansible | Rustible | Notes |
|--------|---------|----------|-------|
| `aws_ec2` | Yes | Yes | AWS EC2 instances |
| `azure_rm` | Yes | Yes | Azure VMs |
| `gcp_compute` | Yes | Yes | GCP instances |
| `constructed` | Yes | Yes | Dynamic groups |
| Script-based | Yes | Yes | JSON output |

---

## Callback Plugins

Rustible includes 30+ callback plugins matching Ansible functionality:

| Category | Plugins |
|----------|---------|
| Output | `default`, `minimal`, `oneline`, `dense`, `summary` |
| Visual | `progress`, `diff`, `tree` |
| Timing | `timer`, `profile_tasks`, `stats` |
| Logging | `json`, `yaml`, `logfile`, `syslog` |
| Integration | `junit`, `slack`, `mail`, `splunk`, `logstash` |

---

## Edge-Case Conformance

Rustible includes a conformance test suite (`tests/conformance_tests.rs`) verifying Ansible edge-case behaviors:

### Boolean Coercion

Rustible accepts Ansible's truthy/falsey string values:

| Value | Boolean |
|-------|---------|
| `yes`, `Yes`, `YES` | `true` |
| `no`, `No`, `NO` | `false` |
| `true`, `True`, `TRUE` | `true` |
| `false`, `False`, `FALSE` | `false` |
| `on`, `On`, `ON` | `true` |
| `off`, `Off`, `OFF` | `false` |
| `1` | `true` |
| `0` | `false` |
| `""` (empty) | `false` |
| `[]` (empty list) | `false` |

### Block/Rescue/Always

- Sections execute in order: `block` -> `rescue` (on error) -> `always`
- Null sections treated as empty lists
- Nested blocks supported

### FQCN Normalization

Both short names and FQCN work identically:

```yaml
# These are equivalent:
- debug: msg="Hello"
- ansible.builtin.debug: msg="Hello"
- ansible.legacy.debug: msg="Hello"
```

### CLI Defaults

| Flag | Default | Notes |
|------|---------|-------|
| `--check` | `false` | Dry-run mode |
| `--diff` | `false` | Show diffs |
| `--tags` | All | Run all tags |
| `--skip-tags` | None | Skip nothing |
| `--forks` / `-f` | 5 | Parallel hosts |
| `--become` / `-b` | `false` | Privilege escalation |

---

## Known Incompatibilities

1. **Vault Format**: Rustible uses AES-256-GCM with Argon2id (vs Ansible's AES-256-CTR with PBKDF2). Re-encryption required when migrating.

2. **Python Module Fallback**: The `python` module provides FQCN support for running Ansible Python modules, but requires Python on the target.

3. **Some Jinja2 Filters**: A few Ansible-specific filters not yet implemented. See [jinja2-filters.md](jinja2-filters.md).

4. **WinRM**: Experimental support, not production-ready.

5. **Database Modules**: Currently disabled pending sqlx integration.

---

## Version Compatibility Targets

| Rustible Version | Ansible Compatibility Target |
|------------------|------------------------------|
| v0.1.x | ~90% core module parity |
| v0.2.x | 95% module parity, full filter support |
| v1.0.x | 98%+ compatibility, production ready |

---

*For the latest updates, see the [ROADMAP](../ROADMAP.md)*
