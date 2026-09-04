# Drift observation and rollback safety changes

These alpha behavior changes affect the library `DriftDetector` and
`RollbackManager`. They do not establish end-to-end CLI drift or cloud recovery
support.

## Drift observations

Failed file/package inspections and missing requested file fields now produce
`DriftType::Unknown` findings instead of inferring absence or reporting no drift.
File modes compare as octal values, so `0644` and `644` are equivalent. Package
inspection requires a successful, nonempty, recognizable response; a failed query
does not prove that a package is absent.

Service inspection recognizes explicit active/inactive and enabled/disabled or
masked states. Missing, extra or unsupported status output produces an unknown
finding. A disabled service can legitimately return a nonzero command status, so
valid observed status values are evaluated directly.

Callers must retain unknown findings as incomplete observations. They are not
evidence that a host is in sync. User checks, ignore patterns and broader
permission-check configuration remain incomplete. Command argument quoting is
also a separate unresolved issue; this patch does not make arbitrary untrusted
drift inputs safe to execute.

## Rollback results

A missing backup now returns an error while preserving the destination. Service,
package and user deletion actions require a module registry and a module result
whose status is `Ok` or `Changed`, with no nonzero return code. Failed or skipped
results are errors. Only the supported module operation is checked; this does not
guarantee complete restoration of all prior system attributes.

Restoring captured user attributes is unsupported and now fails before invoking
the user module. A custom change without an undo operation also fails explicitly.
Handle these errors and retain backups and original state for manual recovery;
do not report the containing operation as rolled back successfully. Durable cloud
rollback, cancellation cleanup and atomic file restoration
remain outside this bounded change.

The recovery manager now executes the rollback plan in its declared order.
Previously it reversed the already ordered plan again: two modifications of one
file could leave the intermediate version while reporting successful rollback.
The corrected order preserves descending priority and reverses equal-priority
changes as prepared by the planner. General dependency-aware undo remains a
separate concern.
