# Inventory and template input corrections

These alpha changes correct input handling. They do not add remote execution or
claim complete Ansible inventory/template compatibility.

## Inventory values and structure

Before, INI group variables and JSON inventory numbers could lose fractions or
truncate unsigned integers. These values now retain their numeric types and
values. Explicitly quoted INI group values remain strings; host-line custom
variables continue to be strings. Host lines now understand balanced shell-style
quotes, escapes, and inline `#` comments. Unmatched quotes and stray tokens without
`key=value` are errors rather than silently incomplete values. Group variables
with unmatched leading quotes are errors rather than panics.

YAML inventory now loads every top-level group, including siblings of `all`.
Top-level inventory must be a mapping. Group definitions and their `hosts`,
`children`, and `vars` sections must be mappings or null; host variable sets must
be mappings or null. Group, host, and variable names must be strings. Unknown
group sections fail, so correct misspellings such as `host` to `hosts`. Empty
groups and null host definitions remain valid.

Host and inline group `ansible_port` values in YAML, JSON, and INI must be integers
in 1 through 65535. Numeric strings are accepted; zero, negative, fractional,
out-of-range, and wrongly typed values are rejected instead of wrapping or falling
back to port 22. This parser validation does not change the Rust `Host::set_port`
API or certify every inventory plugin/external variables-file path.

Inventory files/directories and `Inventory::add_group` now reject cyclic child
group relationships with the existing `CircularDependency` error. A rejected
addition/replacement leaves the previous inventory unchanged. Replacements also
recompute parent edges, removing old inherited relationships. Remove self-links
or back-edges from invalid inventories before loading them. Forward references
to absent groups remain accepted. Cycles introduced through `get_group_mut`
are rejected on the next host-pattern query. Group-host traversal uses a visited
set and an explicit stack; valid diamonds still de-duplicate hosts without
recursion through a deep group chain.

`RuntimeContext` can also receive groups directly, bypassing `Inventory`.
Its existing `get_group_hosts` API now visits each reachable group once,
returning a de-duplicated host list even if that direct graph is cyclic. Its
`Option` return type and depth-first child order are unchanged. This does not
validate every external inventory plugin or repair unrelated pattern matching.

Rust consumers exhaustively matching `HostParseError` must handle the new
`InvalidSyntax` variant. Valid existing inventory retains its API and file format.

## Password and random template operations

The internal alternate `Parser` previously returned a fixed placeholder for `password(path)`, and its
`lookup`/`query` password forms silently returned empty results. These forms,
including `ansible.builtin.password`, now return an explicit unsupported-operation
error. No password is generated, read, or written. Supply a real secret through
your established secure input process; do not treat the former placeholder or
an empty result as a credential. Secure persistent password lookup remains
unimplemented, not simulated.

`random` now rejects an empty sequence or nonpositive integer bound with a
template error rather than returning an undefined result or panicking. Positive
bounds still produce integers in `0..bound`; nonempty sequences still select an
existing element. This filter is not a password-generation API.

## Child variable scopes

`VarScope::get` and `VarScope::all` now inherit the parent's resolved hash-merge
behavior consistently, whether its cache was already populated or not. Local
scope values still replace the corresponding parent value completely, and do
not mutate the parent. No public method signature changes.

The safe `input_semantics_tests` integration target covers inventory and scope
contracts; `parser::input_semantics_regressions` unit tests cover the private
alternate parser. These use temporary files and pure template/variable
operations. They do not run commands, connect to hosts, or access cloud
infrastructure. The parser repair does not prove that any main CLI password
lookup entry point uses this alternate component.

## Generated privileged starters

New `init --template webserver` and `init --template docker` playbooks write
`become: true`, replacing the invalid YAML key `r#become: true` that silently
left escalation disabled. Existing files are not overwritten: manually replace
that key in previously generated starters. The corrected files now actually
request privilege escalation when run; confirm the intended user and commands
before execution. Generation tests only create and parse files in temporary
directories, never run the package, service, or user tasks. This typo repair
does not certify those templates or their modules for production use.
