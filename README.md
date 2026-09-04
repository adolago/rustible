# Rustible

Experimental automation engine written in Rust.

**Acknowledgment**: Rustible is inspired by Ansible and Terraform. It explores
Ansible-style playbooks and infrastructure automation in Rust. Compatibility,
correctness, and performance require workflow-specific verification.

## What is being built?

- A compiled command-line tool with asynchronous execution code.
- Runtime parsing of Ansible-style YAML; full Ansible compatibility is not established.
- Native module and connection implementations, with an incomplete execution contract.
- Reusable tests and fixtures for contributors to verify individual workflows.

## Alpha Status

Rustible is currently in alpha. Expect breaking changes, incomplete features, and evolving
performance/security characteristics.

The ongoing diligence has found execution, state, input-handling, and verification
defects. Focused repairs are being reviewed in draft PRs; they are not yet a
combined, passing release. Read [Verification status](docs/VERIFICATION_STATUS.md)
before choosing a workflow. No production-readiness or comparative speed claim
is made for this alpha.

- Terraform-like provisioning is experimental and limited in scope; Terraform integration
  focuses on state inventory and workflow bridging, not full replacement.
- Current shipped status is tracked in `docs/FEATURE_STATUS.md`.
- Beta-readiness execution is tracked in `docs/GITHUB_ISSUES_SUMMARY.md`.
- Some feature flags remain experimental and still require explicit
  `experimental` opt-in, but `winrm` no longer does.
- Alpha readiness risks and active ownership are tracked in `docs/ALPHA_READINESS_ISSUES.md`.
- Alpha release execution tasks are tracked in `docs/ALPHA_LAUNCH_CHECKLIST.md`.
- Beta promotion criteria and sign-off requirements are defined in `docs/BETA_ENTRY_CRITERIA.md`.
- Begin with disposable test environments and synthetic data while the known
  execution and state defects are resolved.

## Quick Start

Build from source and inspect the CLI (Rust 1.88 or newer):

```bash
# Clone and install
git clone https://github.com/adolago/rustible.git
cd rustible && cargo install --path .

# Inspect commands; this does not execute a playbook
rustible --help
```

### Sample Playbook

```yaml
- name: Inspect a local example
  hosts: localhost
  connection: local
  gather_facts: false

  tasks:
    - name: Display a message
      debug:
        msg: "Rustible example"
```

Save this as `playbook.yml`. `rustible check playbook.yml` runs this local debug
example in requested check mode. It calls the executor; it is not parser-only
syntax validation. Each module is responsible for honoring check mode, and a
successful check does not establish Ansible compatibility or execution correctness.

## CLI Usage

The CLI exposes these playbook options; accepted syntax is not proof of complete
Ansible semantics. The verification status lists known limits and draft repairs.

```bash
rustible run <PLAYBOOK> [OPTIONS]

Options:
  -i, --inventory <FILE>   Inventory file
  -l, --limit <PATTERN>    Limit to specific hosts
  -e, --extra-vars <VARS>  Extra variables
      --check             Request check mode
  -v, --verbose            Increase verbosity
  -f, --forks <N>          Requested parallelism [default: 5]
      --step               Step through tasks interactively
```

### Additional Commands

```bash
rustible check <PLAYBOOK>     # Execute in requested check mode
rustible lock checkpoint NAME # Create a rollback checkpoint
rustible lock rollback NAME   # Dry-run or execute rollback from a checkpoint
rustible vault encrypt <FILE> # AES-256-GCM encryption
rustible vault decrypt <FILE> # Decrypt files
rustible galaxy install <PKG> # Install collections/roles
rustible init <PATH>          # Initialize new project
```

## Features

| Feature | Status |
|---------|--------|
| Playbook syntax | Ansible-style subset; full compatibility not established |
| Inventory formats | YAML, INI, JSON, dynamic scripts |
| Templating | Jinja2 via minijinja |
| Vault encryption | AES-256-GCM |
| Roles | Partial; loading, inheritance, and dependency behavior need further verification |
| Handlers | Partial; scheduling and failure-propagation repairs are in draft |
| Python modules | Fallback implementation exists; successful CLI fallback is not established |

### Connection Implementations

These implementations exist in the source tree. Their presence does not certify
CLI routing, transfer correctness, authentication parity, or real-host behavior.

- **SSH** (default): Via russh
- **Local**: Direct local execution
- **Docker**: Container-based execution
- **Kubernetes**: Pod execution (feature flag)
- **Podman**: Rootless container execution
- **AWS SSM**: EC2 Session Manager connection
- **WinRM**: Windows remote management (feature flag, no `experimental` gate; real-target validation pending)

### Module Catalogue

The following lists describe implementation families, not a verified support
matrix. Some require feature flags, external programs, or unfinished wiring.

**Core modules**: command, shell, raw, script, debug, set_fact, assert, fail, meta, pause, wait_for, stat

**File operations**: copy, template, file, lineinfile, blockinfile, archive, unarchive, synchronize, get_url

**Package management**: package, apt, yum, dnf, pip

**System administration**: service, systemd_unit, user, group, cron, hostname, sysctl

**Security**: authorized_key, known_hosts, ufw, firewalld

**Cloud modules** (feature flags): aws_ec2_instance, aws_s3, aws_iam_role, aws_iam_policy, aws_security_group_rule, aws_ebs_volume, azure_vm, gcp_compute_instance

**Docker**: docker_container, docker_image, docker_network, docker_volume, docker_compose

**Kubernetes** (feature flag): k8s_namespace, k8s_deployment, k8s_service, k8s_configmap, k8s_secret

**Database** (feature flag): postgresql_db, postgresql_user, mysql_db, mysql_user, and more

**Network devices** (feature flag): ios_config, eos_config, junos_config, nxos_config

**HPC** (feature flags): Cluster-related implementations including:
  - *Scheduler*: slurm_config, slurm_ops, slurm_node, slurm_partition, slurm_account, slurm_qos, slurm_job, slurm_queue, slurm_info, slurmrestd, pbs_job, pbs_queue, pbs_server, scheduler_orchestration, partition_policy, lsf_queue, lsf_host, lsf_policy
  - *GPU*: nvidia_gpu, nvidia_driver, cuda
  - *InfiniBand/OFED*: rdma_stack, opensm, ib_partition, ib_diagnostics, ipoib
  - *Parallel Filesystems*: lustre_client, lustre_mount, lustre_ost, beegfs_client
  - *Identity*: kerberos, sssd_config, sssd_domain
  - *Bare-Metal Provisioning*: pxe_profile, pxe_host, warewulf_node, warewulf_image
  - *BMC/IPMI*: redfish_power, redfish_info, ipmi_power, ipmi_boot
  - *Infrastructure*: hpc_baseline, lmod, mpi, munge, hpc_nfs, hpc_facts, hpc_healthcheck, hpc_toolchain, hpc_discovery, hpc_power, boot_profile, image_pipeline

**Windows** (feature flag): win_copy, win_feature, win_service, win_package, win_user

Do not rely on an unknown module automatically executing through Ansible's
Python engine. The audited CLI fallback is incomplete; draft execution repairs
reject unsupported modules explicitly.

## Configuration

Configuration files: `rustible.toml`, `~/.config/rustible/config.toml`, or `/etc/rustible/rustible.toml`

```toml
[defaults]
inventory = "inventory.yml"
forks = 10
timeout = 30

[ssh]
host_key_checking = true
pipelining = true
```

## Feature Flags

Build with additional features:

```bash
cargo build --features docker,kubernetes,aws
```

| Flag | Description |
|------|-------------|
| `russh` | Pure Rust SSH (default) |
| `docker` | Docker container support |
| `kubernetes` | Kubernetes pod execution |
| `aws` | AWS cloud modules |
| `hpc` | Slurm and GPU modules; OFED requires `ofed` separately |
| `slurm` | Slurm workload manager modules |
| `gpu` | GPU management modules (NVIDIA) |
| `ofed` | InfiniBand/RDMA/OFED support |
| `parallel_fs` | Parallel filesystem clients (Lustre, BeeGFS) |
| `pbs` | PBS Pro workload manager modules |
| `lsf` | IBM Spectrum LSF workload manager modules |
| `identity` | Kerberos and SSSD identity management |
| `bare_metal` | PXE boot and Warewulf bare-metal provisioning |
| `distributed` | Distributed execution support |
| `api` | REST API server |
| `provisioning` | Infrastructure provisioning (requires AWS) |
| `full` | russh, local, ssh2-backend, docker, kubernetes, hpc; does not mean every feature |
| `full-cloud` | full, aws, azure, gcp; add experimental for gated providers |
| `full-aws` | full plus aws |
| `full-provisioning` | full-aws plus provisioning |
| `full-hpc` | full plus hpc, pbs, ofed, parallel_fs, redfish, vsphere, identity, bare_metal, lsf |
| `pure-rust` | russh and local; does not guarantee a binary without native C dependencies |
| `ssh2-backend` | Legacy SSH via libssh2 (C dependency) |
| `startup-warmup` | Background warmup of lazy components |
| `openstack` | OpenStack cloud provider (experimental) |
| `redfish` | Bare-metal BMC management via Redfish/IPMI |
| `database` | Database modules (PostgreSQL, MySQL) |
| `winrm` | Windows Remote Management implementation; no `experimental` opt-in required |
| `azure` | Azure cloud modules (experimental) |
| `gcp` | GCP cloud modules (experimental) |
| `reqwest` | HTTP client backend (experimental) |
| `experimental` | Required opt-in for experimental features (azure, gcp, reqwest, openstack) |

## Performance

The previous 5.3–5.9x comparison figures are withdrawn. The comparison runner
counts failed Rustible commands as measurements and does not establish equivalent
successful effects. Existing benchmark files are experimental material, not
evidence of a product speedup.

A publishable comparison needs pinned versions, equivalent successful outputs,
warmups, repeated runs, median and spread, and the execution environment. No such
end-to-end comparison has been completed in the current diligence.

## Documentation

- [Feature Status](docs/FEATURE_STATUS.md) - Canonical implementation status
- [User Guide](docs/guides/README.md) - Comprehensive usage guide
- [API Reference](docs/reference/README.md) - Module documentation
- [Architecture](docs/architecture/ARCHITECTURE.md) - Technical design

## Testing

Run real-host integration tests only against disposable targets. The separate
execution draft replaces simulated results with real module effects, so a test
can invoke package managers, services, accounts, and file writes. Keep controller
and target environments isolated; do not point the examples below at production
hosts while the recorded execution and state findings remain unresolved.

Run the default CLI smoke path:

```bash
bash scripts/smoke_tests.sh
```

That smoke suite covers `rustible run`, `rustible check`, and a vault encrypt/decrypt
round trip. High-risk beta sign-off coverage is defined in
`.github/workflows/high-risk-suites.yml` and documented in
`docs/development/BETA_SIGNOFF_REQUIREMENTS.md`.

### SSH Integration Tests (Ignored)

Russh integration tests are ignored by default and require real SSH hosts.
You can export the variables manually or source the helper script:

```bash
source scripts/ssh-test-env.sh
cargo test test_russh_ -- --ignored
```

Environment variables:

- `RUSTIBLE_SSH_TEST_HOST` / `RUSTIBLE_SSH_TEST_PORT` / `RUSTIBLE_SSH_TEST_USER` / `RUSTIBLE_SSH_TEST_KEY`
- `RUSTIBLE_SSH_TEST_JUMP_HOST` / `RUSTIBLE_SSH_TEST_JUMP_PORT` / `RUSTIBLE_SSH_TEST_JUMP_USER` / `RUSTIBLE_SSH_TEST_JUMP_KEY`
- `RUSTIBLE_SSH_TEST_JUMP2_HOST` / `RUSTIBLE_SSH_TEST_JUMP2_PORT` / `RUSTIBLE_SSH_TEST_JUMP2_USER` / `RUSTIBLE_SSH_TEST_JUMP2_KEY` (multi-hop test)

### Homelab Playbook Tests (Ignored)

Run the homelab smoke playbook against real hosts:

```bash
export RUSTIBLE_HOMELAB_TESTS=1
export RUSTIBLE_HOMELAB_INVENTORY=tests/fixtures/homelab_inventory.yml
cargo test --test homelab_playbook_tests -- --ignored
```

## Contributing

Rustible is a community project maintained by Artur, with substantial AI
assistance in its implementation and review. The maintainer remains responsible
for released code and claims; agent review is not external human review.

Small reproducible examples, independent verification, and ownership of bounded
test areas are especially useful contributions.

See `CONTRIBUTING.md` for guidelines and `CODE_OF_CONDUCT.md` for community expectations.
For security issues, see `SECURITY.md`.

## License

MIT
