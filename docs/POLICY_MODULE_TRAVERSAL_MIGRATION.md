# Module-denial policy input changes

This alpha change affects `RuleCondition::DenyModule` and
`RuleCheck::ModuleBlacklist`. Both now inspect `pre_tasks`, `tasks`, `post_tasks`
and `handlers`, including nested `block`, `rescue` and `always` lists. Conditions
and tags do not hide a supplied task from module inspection. Module argument and
variable data are not treated as executable task containers.

Short names and `ansible.builtin.` / `ansible.legacy.` names use the same
normalization as the executor parser and module registry. In those two
namespaces, the final name component is compared. Other collection names retain
their complete name. Policy selectors are normalized the same way as task keys.

Accepted input remains an array of plays, an object containing a `plays` array,
or one play object. Empty and null task lists are accepted, matching the parser.
Do not mix wrapper-level `plays` with separate play task/role fields.

Raw task module keys and serialized executor tasks with a nonempty `module`
string are supported. Handler wrappers with an explicit task are inspected too;
only name, listen and when metadata may accompany the wrapped task. Other outer
task fields are rejected instead of being silently skipped.
The separate public `playbook::Task` serializer currently omits its module name;
its `module: {args: ...}` output cannot prove which module will run and is
rejected. Supply raw playbook data or a representation retaining module identity.
This change does not repair that serializer. Metadata-only tasks are treated as
implicit `debug` tasks, following the executor parser; a block is a container.
Ambiguous tasks containing multiple module fields, or both a module and a block,
are rejected.

Malformed containers and missing module identity no longer produce a passing
module-denial result. Traversal is limited to 10,000 combined play/task nodes and
64 nested task levels below the outer task. Inputs exceeding either limit fail
explicitly. Nonempty roles and external task/role/playbook includes or imports
also fail as unsupported: their contents are unavailable in the supplied JSON.
No referenced file is loaded or executed. For static evaluation, supply the
complete expanded task data without unresolved external references; this check
does not implement expansion or certify how external content was expanded.

`PolicySet` returns `PolicyError::InvalidInput` for these inspection failures.
`PackRule::evaluate` retains its public nonempty violation-list error reporting.
The pack registry now uses a typed internal result and counts inspection errors
as failures, even for a rule configured with warning or informational severity.
Ordinary policy violations retain their configured severity. Callers must keep
an evaluation error distinct from a successful decision.

Other rule types, OPA execution/response handling, policy discovery and runtime
enforcement wiring are outside this change. Passing a module-denial check means
only that the supported supplied task data does not use the configured modules;
it does not prove that the entire playbook is valid or safe to execute.
