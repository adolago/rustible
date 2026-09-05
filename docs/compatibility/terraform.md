# Terraform Integration Compatibility

> **Last Updated:** 2026-09-04
> **Rustible Version:** 0.1.x
> **Status:** Experimental (Feature-gated)

This document tracks Rustible's Terraform-like provisioning capabilities and integration scope.

---

## Overview

Rustible includes experimental Terraform-like provisioning capabilities, enabling infrastructure-as-code workflows alongside configuration management. This feature is enabled via the `provisioning` feature flag.

This is not a production-ready Terraform replacement. Source implementations and unit tests do not establish working cloud workflows. The built-in AWS provider/resource registration is incomplete, and plan/apply still has unresolved state freshness, replacement, dependency-failure and recovery behavior. Do not use it to take over existing infrastructure state without a separately reviewed migration and disposable acceptance tests.

```bash
# Build with provisioning support
cargo build --release --features provisioning

# Or with full AWS support
cargo build --release --features full-provisioning
```

Rustible can also import Terraform outputs via `vars_files` entries (local, HTTP, or S3) and use Terraform state for dynamic inventory with resource mappings and caching.

---

## Feature Status

| Capability | Terraform | Rustible | Status |
|------------|-----------|----------|--------|
| Plan mode preview | Yes | Partial | Experimental; not an enforced saved-plan approval workflow |
| State management | Yes | Partial | Experimental; stale-writer and recovery limitations remain |
| Drift detection | Yes | Partial | Observed cloud deletion/change is not reliably reflected in plans |
| Remote state backends | Yes | Implementations present | Live interoperability and lease safety not established |
| State locking | Yes | Partial | Unix local exclusion tested; renewal and cancellation cleanup incomplete |
| Terraform state import | Yes | Restricted | Root managed, single unindexed instances only; see migration limits |
| Lockfiles | Yes | Helpers present | Provisioning frozen-mode enforcement is not implemented |
| Checkpoints/rollback | Varies by workflow | Helpers present | Not integrated into ordinary provisioning apply |

---

## Plan Mode

Configuration-management plan mode provides execution previews. This is separate from `rustible provision plan`; it does not establish Terraform planning parity. The following output is illustrative:

```bash
rustible plan playbook.yml -i inventory.yml
```

Output format:
```
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

| Symbol | Meaning |
|--------|---------|
| `+` | Resource will be created |
| `~` | Resource will be modified |
| `-` | Resource will be deleted |
| `.` | Resource unchanged (no action) |

---

## AWS Resource Support

The provisioning CLI requires `--features provisioning`, which also enables `aws`. The `aws` feature alone enables cloud modules but not the provisioning CLI implementation.

### Resource Source Implementations (18 total)

The following catalog counts implementation files, not verified working resource types. Built-in provider/resource registration is incomplete; no entry below should be read as an end-to-end cloud acceptance result. The table's "Implemented" label refers only to source implementation.

| Resource Type | Terraform Equivalent | Status | Notes |
|---------------|---------------------|--------|-------|
| `aws_autoscaling_group` | `aws_autoscaling_group` | Implemented | Auto Scaling Groups with launch templates |
| `aws_db_subnet_group` | `aws_db_subnet_group` | Implemented | RDS DB Subnet Groups |
| `aws_ebs_volume` | `aws_ebs_volume` | Implemented | EBS volumes with encryption support |
| `aws_eip` | `aws_eip` | Implemented | Elastic IPs with VPC association |
| `aws_iam_policy` | `aws_iam_policy` | Implemented | IAM policies with JSON documents |
| `aws_iam_role` | `aws_iam_role` | Implemented | IAM roles with assume role policies |
| `aws_instance` | `aws_instance` | Implemented | EC2 instances with full config |
| `aws_internet_gateway` | `aws_internet_gateway` | Implemented | Internet Gateways |
| `aws_launch_template` | `aws_launch_template` | Implemented | EC2 Launch Templates |
| `aws_lb` | `aws_lb` | Implemented | ALB/NLB/GWLB load balancers |
| `aws_nat_gateway` | `aws_nat_gateway` | Implemented | NAT Gateways |
| `aws_rds_instance` | `aws_db_instance` | Implemented | RDS instances (MySQL, PostgreSQL, etc.) |
| `aws_route_table` | `aws_route_table` | Implemented | Route tables with associations |
| `aws_s3_bucket` | `aws_s3_bucket` | Implemented | S3 buckets with versioning, encryption |
| `aws_security_group` | `aws_security_group` | Implemented | Security groups with inline rules |
| `aws_security_group_rule` | `aws_security_group_rule` | Implemented | Standalone security group rules |
| `aws_subnet` | `aws_subnet` | Implemented | VPC subnets |
| `aws_vpc` | `aws_vpc` | Implemented | Virtual Private Clouds |

### Planned Resources

| Resource Type | Priority | Notes |
|---------------|----------|-------|
| `aws_lambda_function` | High | Lambda functions |
| `aws_sqs_queue` | Medium | SQS queues |
| `aws_sns_topic` | Medium | SNS topics |
| `aws_dynamodb_table` | Medium | DynamoDB tables |
| `aws_ecs_cluster` | Medium | ECS clusters |
| `aws_eks_cluster` | Low | EKS clusters |

---

## State Management

### State Commands

```bash
# Initialize state with backend configuration
rustible state init --backend s3 --bucket my-bucket --key state.json --region us-east-1

# Migrate state between backends
rustible state migrate --from local --to s3 --from-path ./state.json --to-path s3://bucket/key

# Import Terraform state
rustible state import-terraform --tfstate terraform.tfstate --output .rustible/state.json

# List states
rustible state list

# Show state details
rustible state show <name>

# Manage locks
rustible state lock list
rustible state lock release <lock-id>
```

### Remote Backends

| Backend | Status | Locking | Notes |
|---------|--------|---------|-------|
| Local | Experimental | File-based | Cooperating Unix processes; upgrade all writers together |
| S3 | Experimental | DynamoDB | Requires `aws`; live integration not established |
| GCS | Experimental | None | Requires `gcp`; no backend locking |
| Azure Blob | Experimental | Lease-based | Requires `azure`; automatic lease renewal is incomplete |
| Consul | Experimental | Session-based | Session renewal and ownership fencing are incomplete |
| HTTP | Experimental | HTTP Lock/Unlock | Generic HTTP protocol; Terraform Cloud interoperability unverified |

### State File Location

```
./.rustible/provisioning.state.json
./.rustible/provisioning.backend.json
```

---

## Drift Detection

The separate configuration-management drift command inspects supported host resources. It does not establish complete cloud provisioning drift detection: provisioning refresh currently misses some deletion/drift cases. The following output is illustrative:

```bash
rustible drift --playbook site.yml --inventory production.yml
```

Output:
```
╭─────────────────────────────────────────────────────────────────────────╮
│                            DRIFT DETECTION                              │
╰─────────────────────────────────────────────────────────────────────────╯

Host: web1.example.com
  ~ /etc/nginx/nginx.conf
      worker_connections: 1024 → 2048

  + /etc/nginx/conf.d/site.conf (missing)

  - /etc/nginx/conf.d/old.conf (extra file)

Summary: 1 modified, 1 missing, 1 extra
```

---

## Provider Ecosystem

### Current Status

| Provider | Resources | Status | Notes |
|----------|-----------|--------|-------|
| AWS | 18 source implementations | Experimental | Built-in provider/resource registration incomplete |
| Azure | 0 | Stub | Provisioning not yet implemented |
| GCP | 0 | Stub | Provisioning not yet implemented |
| Kubernetes | N/A | Module implementations present | Not provisioning; live interoperability unverified |
| Docker | N/A | Module implementations present | Not provisioning; live interoperability unverified |

### Provider SDK (Planned)

The provider ecosystem architecture includes:

1. **Provider SDK** - Rust SDK for writing providers
2. **Provider CLI** - Packaging and publishing tools
3. **Provider Registry** - Discovery and versioning

See [architecture/provider-ecosystem.md](../architecture/provider-ecosystem.md) for details.

---

## Comparison with Terraform

| Aspect | Terraform | Rustible |
|--------|-----------|----------|
| Primary use case | Infrastructure provisioning | Config management + provisioning |
| Language | HCL | YAML; compatibility must be checked per workflow |
| State tracking | Central to design | Optional feature |
| Configuration drift | Supported workflows | Partial; provisioning deletion/drift handling incomplete |
| Provider ecosystem | Broad provider ecosystem | Experimental; 18 AWS resource source implementations |
| Execution model | Graph-based | Task-based with DAG support |
| Secret management | Depends on configuration/backend | Built-in vault; provisioning local state is not encrypted |
| Learning curve | Depends on workflow | Unvalidated; YAML similarity does not guarantee compatibility |

---

## Migration from Terraform

### Importing Terraform State

```bash
# Import existing Terraform state into Rustible
rustible state import-terraform --tfstate terraform.tfstate

# The import preserves:
# - Resource attributes
# - Dependencies
# - Outputs
# - Lineage and serial numbers
```

Import requires a build with `--features provisioning`. Default builds now reject `state import-terraform` before writing instead of using the lossy fallback converter. Ordinary state listing and other unrelated state commands remain available.

Import is deliberately restricted to Terraform state version 4 with a serial, nonempty lineage, and root managed resources having exactly one unindexed instance. Module-qualified addresses, data resources, count/for_each instances, deposed instances and duplicate addresses are rejected before import returns state. Previous behavior silently collapsed distinct resources; it is not supported compatibility to preserve.

Provider aliases/URLs, private provider metadata, complete dependency addressing and Terraform type roundtrips are not guaranteed. Preserve the original Terraform state and use Terraform/OpenTofu to manage unsupported state. Do not flatten addresses or omit resources just to make import succeed.

### Safety-change migration notes

- For every command, output **format** is a root option and must precede the subcommand: use `rustible --output json state list` or `rustible --output json run playbook.yml`. Previously `--output json` was advertised as global and could follow a subcommand; that placement now returns an argument error where no local file-output option exists. Local `--output PATH` options keep their names and positions. For example, `rustible --output json state import-terraform --tfstate terraform.tfstate --output imported.json` separates display format from destination path. The previous shared internal option name could panic instead of parsing an import destination.
- Back up state before upgrading and upgrade every writer together. New saves use deterministic canonical checksums. The reader verifies original legacy checksums, but old binaries may reject newly saved files; do not alternate versions against the same state.
- Local lock metadata uses persistent `.guard` files. Do not remove these while a process may use the state, or mix writers that ignore them. Corrupt lock metadata is no longer automatically removed. Non-Unix local locking fails explicitly pending a safe portable implementation.
- Saved-plan apply, resume, frozen provider enforcement, state encryption, canary, blast-radius limits and admission-policy options are not implemented in the apply workflow. They must not be used as safety guarantees; unsupported requests are rejected rather than applied without protection.
- Dependency lockfile integrity verifies regular local-file checksums in bounded chunks. Special files, roles, collections and remote resource types fail explicitly when their integrity cannot be verified; a previous unconditional success was not proof of integrity. Symlinks to regular files remain supported; this is not a file snapshot or protection against concurrent content changes.

### When to Use Rustible vs Terraform

**Use Rustible when:**
- You have existing Ansible playbooks
- Configuration management is primary need
- Want unified tool for provisioning + config
- Need SSH-based management

**Use Terraform when:**
- Infrastructure provisioning is primary need
- Need extensive cloud provider coverage
- Complex multi-cloud deployments
- Large team already using HCL

### Hybrid Approach

Rustible can complement Terraform:

```yaml
# Use Terraform for infrastructure
# Use Rustible for configuration

- name: Configure instances provisioned by Terraform
  hosts: "{{ lookup('file', 'terraform.tfstate') | from_json | json_query('resources[?type==`aws_instance`].instances[*].attributes.public_ip') | flatten }}"
  tasks:
    - name: Install application
      package:
        name: myapp
        state: present
```

---

## Limitations

1. **Provider Coverage**: Fewer providers than Terraform (AWS only for provisioning)
2. **HCL Support**: No HCL parsing (YAML only)
3. **Modules**: No Terraform module compatibility
4. **Azure/GCP Provisioning**: Not yet implemented (planned)

---

## Roadmap

| Version | Features |
|---------|----------|
| Current alpha | Partial provisioning and state helpers; known correctness gaps above |
| Proposed next milestone | Verified provider wiring, state safety, drift and recovery in disposable workflows |
| v0.3 | Azure/GCP provisioning baseline |
| v1.0 | Lockfiles, checkpoints, provider registry |

---

*For detailed architecture, see [architecture/terraform-integration.md](../architecture/terraform-integration.md)*
