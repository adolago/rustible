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

| Area | Status | Notes |
|------|--------|-------|
| Core playbook execution | Incomplete | Parsing and execution code exist, but the audited baseline includes simulated success and incomplete selection, role, handler, and remote-execution behavior. Draft repairs do not make unchanged main complete. |
| Lock/checkpoint workflow | Under validation | Snapshot-backed checkpoint and rollback code exists. Recovery guarantees and failure handling require workflow-specific verification. |
| Rollback engine | Incomplete | Module-dependent restoration paths exist; generic rollback can report success without restoring state. The CLI and generic library paths require separate evidence. |
| WinRM transport | Under validation | Feature-gated with `winrm`, no `experimental` gate required. Windows-target behavior has not been independently exercised in this diligence. |
| WinRM auth support | Partial | NTLM, Basic, and certificate auth are implemented. Kerberos and CredSSP fail fast with explicit unsupported errors. Windows Credential Manager remains unsupported. |
| Windows native modules | Under validation | Implementations and tests exist; passing real-Windows parity/integration evidence has not been established here. |
| AWS native modules | Under validation | AWS module implementations exist behind `aws`; live-cloud correctness and parity are not established by source presence or mock tests. |
| AWS provisioning resources | Experimental / Incomplete | Resource implementations exist; planning, locking, durable state, and failure recovery have open correctness findings. |
| Azure / GCP modules | Experimental | Still require `experimental` plus provider feature flags. |
| Terraform-like provisioning | Experimental / Incomplete | Not a Terraform replacement or a verified safe state-migration path. Import and state repairs are draft changes pending broader checks. |
| Beta readiness docs and checklists | In Progress | Beta gate docs exist; use them with the live tracker, explicit CLI smoke coverage, and the high-risk sign-off workflow rather than the archived gap-analysis issue list. |

## Beta-Readiness Tracker

- [#849](https://github.com/adolago/rustible/issues/849) Align roadmap and feature-status docs with the live implementation
- [#850](https://github.com/adolago/rustible/issues/850) Stabilize v0.2 baseline: get default CI and test suite fully green
- [#851](https://github.com/adolago/rustible/issues/851) Complete checkpoint rollback execution in the CLI lock workflow
- [#852](https://github.com/adolago/rustible/issues/852) Harden WinRM/Windows support and define exit criteria for non-experimental status
- [#853](https://github.com/adolago/rustible/issues/853) Implement `aws_security_group_rule` as a native playbook module
- [#854](https://github.com/adolago/rustible/issues/854) Implement `aws_ebs_volume` as a native playbook module
- [#855](https://github.com/adolago/rustible/issues/855) Execution sequence tracker for beta-readiness and AWS module parity

## Known Limits Worth Calling Out

- The `winrm` feature's lack of an `experimental` gate is a build choice, not evidence of beta readiness. Real-Windows test coverage still depends on host availability.
- Kerberos and CredSSP authentication are parsed and tested for explicit failure behavior, but are not implemented.
- Rollback requires snapshot-backed checkpoints for live execution. Older checkpoint files remain readable but must be recreated for live rollback.
- EBS volume code selects resources by `volume_id` or `Name` tag lookup and rejects ambiguous matches. Live-cloud idempotency has not been independently verified in this diligence.
- Standalone security group rule management supports IPv4 CIDRs, IPv6 CIDRs, referenced security groups, and self-referencing rules, with description changes applied as revoke-plus-authorize when AWS requires replacement semantics.
