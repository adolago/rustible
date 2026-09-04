# Build and release policy changes

This alpha repair changes dependency and release contracts. It does not certify
production readiness, complete Ansible compatibility, or cloud behavior.

## Rust dependency compatibility

Before: Rustible used russh 0.61; `rustible::connection::RusshError` exposed that
version's error type in its public tuple field.

After: Rustible requires russh 0.62.5 or later within the 0.62 series (the checked-in
lock resolves 0.62.7). A direct downstream dependency on russh 0.61 produces a
different Rust type and cannot be passed to that field. Downstream Rust consumers
that construct or match this wrapper must align their direct russh dependency
with Rustible and recompile. The upstream agent-forward callback also requires
an explicit channel-open reply; Rustible accepts it only when forwarding is
enabled. No CLI or YAML syntax change is intended by this dependency update.

The independent SSH-comparison benchmark now requires async-ssh2-tokio 0.13.
Its dependency changes do not validate the benchmark's equivalence or results.

## Release inputs and package contents

Release tags must have a `v` prefix. Manual release input omits that prefix.
Both must exactly match `package.version`, including prerelease and build
metadata; mismatches now fail instead of warning. Output is emitted only after
validation, and dispatch input is not interpolated into shell source.

Generated `tarpaulin-report.html` coverage exports remain in the repository but
are omitted from the crate archive. They are not build inputs. The release job
checks that the compressed archive is within crates.io's default 10 MB limit.
Source, tests, fixtures, and substantive documentation are retained.

## Security and publication gates

History secret scanning runs offline, redacts findings, and fails on new
candidates or scanner errors. `.gitleaksignore` contains only individually
reviewed historical commit/path/rule/line fingerprints. A new commit at the same
path and line is not exempt. Do not add broad exclusions to make the gate pass.

The primary Docker image's high/critical vulnerability scan now fails its job.
The separate multi-architecture publication path still needs independent image
scanning and an exact scanned-digest publication guarantee. The Compose check is
explicitly a startup smoke test, not authenticated remote execution evidence.

Manual `push_image=false` no longer publishes merely because the selected branch
is main. Manual benchmark baseline-saving inputs are now honored.

## Known verification limits

The root audit still has an explicit unresolved RSA exception and an AWS S3
transitive lru warning. A passing audit under that policy is not a warning-free
or vulnerability-free result. Native Windows release targets and their ZIP
publication declarations have been removed because the control-node code has
unconditional Unix dependencies. See [Platform support](PLATFORM_SUPPORT.md) for
the declared Linux/macOS targets and verification limits. Real cloud/HPC
behavior and broad performance claims still require separate evidence.

Focused regression tests cover test-runner control flow, release input/output
validation, and archive-size boundaries. Full release packaging, optional feature
builds, remote CI and container verification must also pass before promotion.
