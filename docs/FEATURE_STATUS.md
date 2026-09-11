---
summary: Canonical feature status for Rustible. Update this document first whenever implementation status changes.
read_when: You need an accurate snapshot of what is implemented, partial, beta, or still planned.
---

# Rustible Feature Status

This document is the canonical status source for Rustible features. If code or docs
disagree, update this file first and then align secondary docs.

## Maintenance Rule

- Issue numbers in status docs must be live GitHub links or omitted.
- Any feature-status change must update this document before `README.md`, `docs/ROADMAP.md`,
  or release checklists.
- While beta-readiness work remains in progress, `README.md` stays alpha-facing and links to
  beta-readiness docs rather than claiming beta availability.

## Status Summary

The 4 September 2026 diligence found that previous "Complete" and "Beta"
labels overstated the available evidence. Rustible remains `0.1.1-alpha`.
Implementation presence, focused tests, and verification against real systems
are separate claims. See [Verification status](VERIFICATION_STATUS.md) for the
review baseline, draft repairs, and outstanding verification.

Rows marked "Partially verified" below were exercised against a disposable
sshd container on 10 September 2026 (`tests/remote_modules_ssh_tests.rs`, run
with `RUSTIBLE_TEST_SSH_DOCKER=1`). That covers a Debian target reached over
SSH; it says nothing about other distributions, Windows, or cloud services.

| Area | Status | Notes |
|------|--------|-------|
| Core playbook execution | Partially verified | The CLI now builds a transport from the inventory, so remote hosts execute; before this it reported every remote task unreachable. Local and SSH paths are exercised end to end in tests, including against a live sshd container. Ansible parity as a whole is still not established. |
| Remote module coverage | Partial (verified list) | 23 modules are exercised against a live SSH target (see `Task::REMOTE_VERIFIED_MODULES`): apt, authorized_key, blockinfile, command, copy, cron, fetch, file, gather_facts, git, group, lineinfile, package, ping, replace, script, setup, shell, slurp, stat, template, timezone, user. A second list holds 13 modules whose implementation is connection-only but which no environment here can exercise (systemd, firewalls, RPM distributions, mounts). Everything else is refused remotely rather than run against the control node. |
| Privilege escalation (`become`) | Partially verified | Escalation reaches the target for the modules that pass it into their commands, verified with passwordless sudo in a container. Transfer-based modules (copy, template, lineinfile) refuse escalation because SFTP cannot escalate, and escalation on the control node is still refused. |
| Drift detection | Partially verified | `drift detect` checks each host through its own connection; an unreachable host reports unknown rather than "in sync". Verified for file and group resources locally and over SSH. |
| Lock/checkpoint workflow | Partially verified | Lockfiles record the local files a playbook depends on and `lock verify` detects an edit. Roles and collections are not locked. |
| Rollback engine | Partially verified | `run --checkpoint` records pre-task state and `lock rollback` undoes file, package, service, user and group changes. Verified for a created directory end to end. Modules outside that set produce no rollback action. |
| Agent mode | Partially verified | `agent build/deploy/status/stop` and `run --agent-mode` work against a live container. The deployed agent is one-shot per command; a persistent listening agent is implemented in the library but not deployed by these commands. |
| Cross-run task cache | Implemented | `run --cache-state` skips tasks whose inputs are unchanged since a no-op run. Off by default; check mode never uses it. |
| Jinja2 filters | Implemented | The filter plugins are registered in the production engine and pinned by tests from the engine down. `json_query`, `vault`/`unvault` and the advanced `ipaddr` queries are absent; see `docs/compatibility/jinja2-filters.md`. |
| WinRM transport | Under validation | Feature-gated with `winrm`, no `experimental` gate required. Windows-target behavior has not been exercised here. |
| WinRM auth support | Partial | NTLM, Basic, and certificate auth are implemented. Kerberos and CredSSP fail fast with explicit unsupported errors. Windows Credential Manager remains unsupported. |
| Windows native modules | Under validation | Implementations and tests exist; passing real-Windows parity/integration evidence has not been established here. |
| AWS native modules | Under validation | AWS module implementations exist behind `aws`; live-cloud correctness and parity are not established by source presence or mock tests. |
| AWS provisioning resources | Experimental / Incomplete | Resource implementations exist; planning, locking, durable state, and failure recovery have open correctness findings. |
| Azure / GCP modules | Experimental | Still require `experimental` plus provider feature flags. |
| Terraform-like provisioning | Experimental / Incomplete | Not a Terraform replacement or a verified safe state-migration path. |
| Beta readiness docs and checklists | In Progress | Beta gate docs exist; use them with the live tracker, explicit CLI smoke coverage, and the high-risk sign-off workflow. |

## Beta-Readiness Tracker

- [#849](https://github.com/adolago/rustible/issues/849) Align roadmap and feature-status docs with the live implementation
- [#850](https://github.com/adolago/rustible/issues/850) Stabilize v0.2 baseline: get default CI and test suite fully green
- [#851](https://github.com/adolago/rustible/issues/851) Complete checkpoint rollback execution in the CLI lock workflow
- [#852](https://github.com/adolago/rustible/issues/852) Harden WinRM/Windows support and define exit criteria for non-experimental status
- [#853](https://github.com/adolago/rustible/issues/853) Implement `aws_security_group_rule` as a native playbook module
- [#854](https://github.com/adolago/rustible/issues/854) Implement `aws_ebs_volume` as a native playbook module
- [#855](https://github.com/adolago/rustible/issues/855) Execution sequence tracker for beta-readiness and AWS module parity

## Known Limits Worth Calling Out

- Only the modules on the executor's verified list run against a remote host. A module with a local-filesystem implementation and no connection path is refused rather than run on the control node.
- A play of tasks that never touch the target (debug, set_fact, assert, ...) opens no connection at all, so an unreachable inventory costs nothing until a task needs it.
- The CLI does not retry the initial connection; set `ansible_ssh_retries` per host to opt back in.
- `become` is refused for transfer-based modules and on the control node.
- Rollback needs `run --checkpoint`; a run without it records no prior state and cannot be undone.
- Lockfiles cover local file dependencies only; roles and collections need Galaxy resolution.
- The `winrm` feature's lack of an `experimental` gate is a build choice, not evidence of beta readiness. Real-Windows test coverage still depends on host availability.
- Kerberos and CredSSP authentication are parsed and tested for explicit failure behavior, but are not implemented.
- Rollback requires snapshot-backed checkpoints for live execution. Older checkpoint files remain readable but must be recreated for live rollback.
- EBS volume code selects resources by `volume_id` or `Name` tag lookup and rejects ambiguous matches. Live-cloud idempotency has not been independently verified in this diligence.
- Standalone security group rule management supports IPv4 CIDRs, IPv6 CIDRs, referenced security groups, and self-referencing rules, with description changes applied as revoke-plus-authorize when AWS requires replacement semantics.
