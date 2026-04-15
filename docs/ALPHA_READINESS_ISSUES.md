# Alpha Readiness Issues

This tracker captures alpha risks and their current disposition. Every open item
must keep ownership, next action, and evidence current.

- Last reviewed: `2026-04-15`
- Release lead: `TBD`

For canonical shipped status, see [FEATURE_STATUS.md](FEATURE_STATUS.md). For the live
beta-readiness execution queue, see [GITHUB_ISSUES_SUMMARY.md](GITHUB_ISSUES_SUMMARY.md).

## Blockers (must fix before alpha)

- _No current blockers. Add one immediately when discovered._

## High (fix before alpha or explicitly waive)

- _No current high-severity items. Keep this section empty only if triaged._

## Medium (fix soon or keep out of alpha scope)

- _No current medium-severity items. Keep this section empty only if triaged._

## Low (track for later or batch for beta)

- [ ] Password material is still stored as `String` in parts of the vault path and is not uniformly zeroized.
  - Owner: `Security`
  - Next action: Reduce plaintext password lifetime in CLI/vault flows and expand zeroization regression coverage.
  - Evidence: `src/cli/commands/vault.rs`, `src/vault.rs`, `tests/secrets_zeroization_tests.rs`.
- [ ] Security audit artifacts need a fresh run against the current CI baseline.
  - Owner: `Release lead`
  - Next action: Re-run `.github/workflows/security.yml` and reconcile the results with `docs/security/SECURITY_AUDIT_REPORT.md`.
  - Evidence: Updated security report link + workflow run URL.

## Resolved (post-triage)

- [x] `--ask-become-pass` CLI support is available and covered by CLI tests.
  - Owner: `Completed`
  - Evidence: `src/cli/commands/run.rs`, `tests/cli_tests.rs`.
- [x] Keyboard-interactive SSH authentication is implemented in both SSH backends.
  - Owner: `Completed`
  - Evidence: `src/connection/ssh.rs`, `src/connection/russh_auth.rs`, `src/connection/russh.rs`.
- [x] Resource graph state comparison coverage exists for provisioning plan behavior.
  - Owner: `Completed`
  - Evidence: `tests/resource_graph_state_comparison_tests.rs`.
- [x] The legacy `russh_auth` API-drift TODO is no longer an active alpha risk.
  - Owner: `Completed`
  - Evidence: `src/connection/russh_auth.rs`, `src/connection/mod.rs`.
- [x] Privilege escalation username injection risk in become command builders.
  - Owner: `Completed`
  - Evidence: `docs/security/BECOME_AUDIT.md`, `src/connection/russh.rs`, `src/connection/ssh.rs`, `src/connection/local.rs`.
- [x] Path injection risk in ownership changes during local execution.
  - Owner: `Completed`
  - Evidence: `docs/security/BECOME_AUDIT.md`, `src/connection/local.rs`.
- [x] Deprecated `serde_yaml` dependency flagged in security audit.
  - Owner: `Completed`
  - Evidence: `docs/security/SECURITY_AUDIT_REPORT.md`, `Cargo.toml`.
- [x] DynamoDB state lock operations implemented for provisioning backend.
  - Owner: `Completed`
  - Evidence: `src/provisioning/state_lock.rs`.
- [x] Stubbed feature flags now require explicit experimental opt-in.
  - Owner: `Completed`
  - Evidence: `Cargo.toml`, `README.md`.
- [x] Python module local execution path implemented for executor fallback.
  - Owner: `Completed`
  - Evidence: `src/executor/task.rs`.
- [x] Coverage improvements completed for executor task execution paths.
  - Owner: `Completed`
  - Evidence: `src/executor/task.rs`.
