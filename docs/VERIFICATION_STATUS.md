# Verification status

Snapshot: 4 September 2026. Diligence baseline:
`c9f9b44db291f543be6580339795b5eedc86fb89`. The review is incomplete.
This page describes measured limits and submitted repairs, not release approval.

## What the evidence supports

The baseline default binary built on Debian 13 x86_64. Focused regressions have
reproduced defects in execution results, inventory inputs, file operations, and
state handling. Source review also leaves many paths awaiting runtime validation.
Test-file presence, generated coverage exports, or a feature flag alone is not
passing behavior evidence.

Full Ansible compatibility, production readiness, complete rollback, live-cloud
correctness, physical-HPC support, and Windows-controller support are not
established. Rustible parses playbooks at runtime; Rust's compile-time type checks
do not validate arbitrary YAML configurations. The `pure-rust` feature selects
backends but does not remove every dependency that uses native C code.

## Draft repairs

All of the following remain draft changes; none were merged at this snapshot.
Their tests apply to their own source revisions, not the unchanged baseline or
an untested combination of branches.

- [Connection and vault input boundaries (#938)](https://github.com/adolago/rustible/pull/938).
- [Execution selection and real-result propagation (#939)](https://github.com/adolago/rustible/pull/939).
- [Build, dependency, packaging, and scan policies (#940)](https://github.com/adolago/rustible/pull/940).
- [Native file and archive operations (#941)](https://github.com/adolago/rustible/pull/941).
- [Inventory inputs and graph traversal (#942)](https://github.com/adolago/rustible/pull/942), and [state/provisioning guards (#943)](https://github.com/adolago/rustible/pull/943).

The execution draft replaces simulated module results with real dispatch and
rejects unsupported remote, privacy, and privilege paths. It does not implement
full remote execution or Ansible parity. The state draft contains bounded guards
and integrity fixes; planning, concurrent updates, provider coverage, and recovery
still have open findings. Read each PR's migration note before using its branch.

## Verification still required

- Required CI on the final combined source and its dependency graph, including
  applicable feature and platform builds.
- Real transport, cloud, Windows-target, and physical-cluster behavior in
  disposable environments with explicit workflow expectations.
- Full package verification, distribution support, and exact-image scanning.
- Coverage of remaining source, tests, claims, and historical changes; all
  confirmed defects require dispositions and verified repairs.

The current container repair applies available stable package fixes, but its
high/critical scanner gate still fails on remaining advisories. Cargo audit's
existing RSA exception is unresolved. A passing command under an exception
policy is not a statement that the dependency graph has no advisories.

## Performance and testing claims

The README's previous speedup figures are withdrawn. The comparison script
currently accepts failed Rustible runs and assumes host counts without verifying
equivalent outcomes. It must be repaired and rerun before publishing comparative
numbers. No benchmark run in this diligence establishes an end-to-end speedup.

The original four callback fuzz targets and callback property-test file exercise
stand-in implementations. Their historical success does not establish production
callback coverage. Replacement harness work and final campaigns are incomplete.

For a contributor trial, first establish a small supported workflow with expected
first-run, repeat-run, failure, and unsupported-input results. Keep experiments
disposable while the recorded execution and state defects remain unresolved.
