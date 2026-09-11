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

Some rows marked "Partially verified" below were exercised against a disposable
sshd container on 10-11 September 2026 (`tests/remote_modules_ssh_tests.rs`, run
with `RUSTIBLE_TEST_SSH_DOCKER=1`). That covers a Debian target reached over
SSH; it says nothing about other distributions, Windows, or cloud services.
Other rows — lock/checkpoint, rollback and state manifests — have local
evidence only, and say so in their own notes. Read each row for what backs it;
the label alone does not mean a live target was involved.

| Area | Status | Notes |
|------|--------|-------|
| Core playbook execution | Partially verified | The CLI now builds a transport from the inventory, so remote hosts execute; before this it reported every remote task unreachable. Local and SSH paths are exercised end to end in tests, including against a live sshd container. Ansible parity as a whole is still not established. |
| Remote module coverage | Partial (verified list) | The executor permits 27 list entries remotely, covering 26 registered modules (`setup` aliases `gather_facts`) — see `Task::REMOTE_VERIFIED_MODULES`. Of those, **21 are exercised against a live SSH target** in `tests/remote_modules_ssh_tests.rs`: archive, authorized_key, blockinfile, command, copy, cron, fetch, file, gather_facts, git, group, lineinfile, raw, replace, script, slurp, stat, timezone, unarchive, user, wait_for. The remaining six (apt, package, ping, setup, shell, template) are on the list on the strength of a source review of their transport path only, and are not covered by any live-target test. A second list holds 20 connection-only modules no environment here can exercise; a third names the 19 that run on the control node by design. |
| Privilege escalation (`become`) | Partially verified | Escalation reaches the target for the modules that pass it into their commands, verified with passwordless sudo in a container. Transfer-based modules (copy, template, lineinfile) refuse escalation because SFTP cannot escalate, and escalation on the control node is still refused. |
| Drift detection | Partially verified | `drift detect` checks each host through its own connection; an unreachable host reports unknown rather than "in sync". Evidence is `tests/drift_target_tests.rs`, which covers a file resource against a local host and an unreachable host; there is no over-SSH drift test and no group-resource drift test. |
| Lock/checkpoint workflow | Partially verified (local only) | Lockfiles record the local files a playbook hands to `copy`, `template` and `script`, plus the roles the playbook **names** (`roles:`, `include_role`, `import_role`) and every collection under `collections/ansible_collections`, each by a checksum over its file tree; `lock verify` detects an edit to any of them. Known limits: a role pulled in only through another role's `meta` dependencies is not hashed; `include_tasks`/`import_tasks` targets and `vars_files` are not locked; stale entries are never pruned on re-lock; and recorded paths are resolved against the process working directory, so a lockfile verifies reliably only from the directory that produced it. |
| Rollback engine | Partially verified | `run --checkpoint` records pre-task state and `lock rollback` undoes file, package, service, user and group changes. Verified for a created directory end to end. Modules outside that set produce no rollback action. |
| Agent mode | Partially verified | `agent build/deploy/status/stop` and `run --agent-mode` work against a live container. The deployed agent is one-shot per command; a persistent listening agent is implemented in the library but not deployed by these commands. |
| State manifests | Experimental / Incomplete | `run --manifest` records one manifest per host covering every resource the run applied, changed or not, and `drift manifest list/show/check` reads them back; `check` replays each resource through a connection to its host, so an unreachable host reports unknown rather than in sync. A pre-PR review found real limits that make this unsuitable for relying on yet: the recorded desired state is the task's **untemplated** arguments, so any tracked task using a variable cannot be replayed; a task naming N resources replays the whole task N times; the replay drops the task's `become`, vars and facts; a module whose check mode always reports changed is recorded as permanently drifted; and `run --manifest` and `drift manifest --dir` resolve their default directory differently. |
| Cross-run task cache | Implemented | `run --cache-state` skips tasks whose inputs are unchanged since a no-op run. Off by default; check mode never uses it. |
| Jinja2 filters | Implemented | The filter plugins are registered in the production engine and pinned by tests from the engine down. `json_query` (JMESPath), `vault`/`unvault` and the `ipaddr` membership, `range_usable`, `peer` and `revdns` queries are now present; the IPv6 transition queries (`6to4`, `teredo`) still raise rather than guess. `vault` writes Rustible's format, not `$ANSIBLE_VAULT`. See `docs/compatibility/jinja2-filters.md`. |
| WinRM transport | Under validation | Feature-gated with `winrm`, no `experimental` gate required. Windows-target behavior has not been exercised here. |
| WinRM auth support | Partial | NTLM, Basic, and certificate auth are implemented. Kerberos and CredSSP fail fast with explicit unsupported errors. Windows Credential Manager remains unsupported. |
| Windows native modules | Under validation | Implementations and tests exist; passing real-Windows parity/integration evidence has not been established here. |
| AWS native modules | Under validation | AWS module implementations exist behind `aws`; live-cloud correctness and parity are not established by source presence or mock tests. |
| AWS provisioning resources | Experimental / Incomplete | Resource implementations exist; planning, locking, durable state, and failure recovery have open correctness findings. |
| Azure / GCP modules | Experimental | Still require `experimental` plus provider feature flags. |
| Terraform-like provisioning | Experimental / Incomplete | Not a Terraform replacement or a verified safe state-migration path. |
| Beta readiness docs and checklists | In Progress | Beta gate docs exist; use them with the live tracker, explicit CLI smoke coverage, and the high-risk sign-off workflow. |
| Default test suite | Green | `cargo test --no-fail-fast -- --test-threads=1` at `39316b4a` on 11 September 2026: 177 result groups, 12132 passed, 0 failed, 16 ignored. Docker-gated remote tests are separate and run with `RUSTIBLE_TEST_SSH_DOCKER=1`. |

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
- Manifests need `run --manifest`; a plain run records none. Modules that manage nothing durable (`command`, `shell`, `debug`, `set_fact`, ...) are never recorded.
- Lockfiles cover what is on disk: local file dependencies, installed roles and installed collections. A Galaxy reference with nothing installed is not locked.
- The `winrm` feature's lack of an `experimental` gate is a build choice, not evidence of beta readiness. Real-Windows test coverage still depends on host availability.
- Kerberos and CredSSP authentication are parsed and tested for explicit failure behavior, but are not implemented.
- Rollback requires snapshot-backed checkpoints for live execution. Older checkpoint files remain readable but must be recreated for live rollback.
- EBS volume code selects resources by `volume_id` or `Name` tag lookup and rejects ambiguous matches. Live-cloud idempotency has not been independently verified in this diligence.
- Standalone security group rule management supports IPv4 CIDRs, IPv6 CIDRs, referenced security groups, and self-referencing rules, with description changes applied as revoke-plus-authorize when AWS requires replacement semantics.
