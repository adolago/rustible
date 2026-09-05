# Parser, schema and cache corrections (alpha)

These changes correct three bounded contracts. They do not establish full Ansible
compatibility or prove that every CLI path uses the same parser or validator.
The alternate `Parser` remains internal; `rustible::schema` and
`rustible::template::TemplateEngine` retain their public method signatures.

## Alternate parser strictness and ordering

Previously, `Parser::strict(true)` stored a flag but still rendered an undefined
value as an empty string. It now selects MiniJinja's strict undefined behavior.
`strict(false)` restores the previous lenient behavior. Explicit `default` filters
and `is defined` tests still work; nested `render_value` calls propagate errors.

The `sort`, `min` and `max` filters previously compared display strings, so
`[2, 10] | min` produced `10`. They now compare MiniJinja values: numeric values
use numeric ordering, strings retain case-sensitive lexical ordering, and mixed
types follow MiniJinja's type ordering. Numeric-looking strings remain strings.
This is an alpha behavior change for numeric and mixed-type collections.

## Nested schema validation and strict mode

The schema validator previously skipped task lists under `block`, `rescue` and
`always`. It now visits those lists under tasks, pre-tasks, post-tasks and handlers,
including nested blocks, and reports their complete paths. Invalid nested task
list shapes are errors. Traversal uses an explicit stack.

`ValidatorConfig::max_depth` now limits task-block nesting. Top-level tasks have
depth zero; a value of zero permits flat tasks, one permits one nested task level,
and the default remains 50. A non-empty task list beyond the limit produces an
error instead of silently skipping validation. Empty lists contain no deeper
tasks. This limit does not replace the YAML parser's own input limits.

`strict_mode: true` now promotes both built-in and custom-rule warnings into
errors, changes their severity to `Error`, and sets `ValidationResult::valid` to
false. Normal mode retains warnings without making them fatal. Informational
messages remain informational.

## Unsupported validator options

No undefined-variable analysis or custom-schema directory loading was previously
performed, despite the configuration fields. The default for
`check_undefined_vars` is now `false`, matching the implemented default behavior.
Explicitly requesting `check_undefined_vars: true` or supplying
`custom_schema_dir` returns an invalid validation result with an unsupported
option diagnostic. Construction still returns a validator; validation reports
the error. No custom schema files are loaded.

Callers that explicitly set either option must disable it or provide the required
analysis separately. This correction does not implement variable-context analysis
or a custom-schema loader. A valid schema result is not proof that all runtime
variables exist, that every module argument is supported, or that execution will
succeed.

## Template cache

Previously, the LRU index stayed bounded while compiled templates accumulated in
the environment. Capacity eviction now removes the corresponding compiled
template. Rendering, insertion, eviction and clearing use a consistent
environment-before-cache lock order. Rendering keeps the selected template alive
until completion, and concurrent cache misses recheck before insertion.

Cache capacity zero still disables caching. Invalid template compilation preserves
existing valid cache entries. Public cache statistics and rendering signatures
remain unchanged; this is a correctness and retention fix, not a new performance
benchmark claim.
