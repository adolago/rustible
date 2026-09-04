# Policy evaluation contract corrections

Unknown or unimplemented pack checks now report an explicit unsupported error.
They no longer appear as passed, informational or warning-only rules. This
includes `require-become-explicit`, `max-forks`, `require-limit` and
`deny-localhost-in-prod`, as well as misspelled or arbitrary custom names.
Lower severity metadata cannot convert a missing evaluator into a successful
check. Existing implemented rules retain their current evaluation behavior.

This is an alpha behavior change. The operations baseline currently has three
unsupported checks; the security baseline has one. The `policy check` CLI
evaluates all built-in packs and now exits with failure while those unavailable
checks are selected. It cannot certify that a playbook satisfies those policies.
`policy list` and `policy inspect` describe the unavailable checks explicitly.

Library users may construct a manifest containing only implemented rules and
load it with `PackRegistry::load`. That choice excludes the unavailable policies;
it is not a replacement for their enforcement. Custom checks need a future
evaluator implementation and an explicit input contract. No production
classification, variable context, fork configuration or target limit is inferred
from missing data by this patch.

This correction does not establish complete module-policy coverage: traversal
of all executable containers and module aliases remains a separate repair. The
existing maximum-task rule uses a fixed limit of twenty; declared parameter
metadata is not consumed by that evaluator. A passing selected rule group is
not a general safety or compliance certification.

The `policy init` hint now directs custom-pack users to the Rust loading API.
The CLI has no custom-manifest argument. Its generated parameter description
also states that `max-tasks` uses the fixed limit, so editing that metadata does
not imply a changed limit.
