# Alpha platform declarations and limits

Rustible currently requires a Unix control node: the machine running its CLI or
library. Native Windows control-node binaries are unsupported. The default
module graph includes unconditional `std::os::unix` imports in local connection,
file and copy modules, and a Unix datagram socket type in audit logging. These
boundaries also apply to the `pure-rust` release feature selection.

The release workflow no longer attempts the two native Windows MSVC targets,
packages Windows ZIP archives or reports those archives as published. CI already
excluded Windows. This corrects unavailable artifact declarations; it does not
implement Windows portability or withdraw a verified Windows binary interface.

Windows target management is a separate feature: optional WinRM and Windows
modules describe operations directed at a remote Windows machine from a Unix
control node. Their presence does not establish that Rustible builds or runs
natively on Windows. This workflow correction does not validate their runtime
behavior or change those feature flags.

## Declared release targets

CI and release workflows declare the same five build targets:

- Linux x86_64 with glibc: `x86_64-unknown-linux-gnu`.
- Linux x86_64 with musl: `x86_64-unknown-linux-musl`.
- Linux ARM64 with glibc: `aarch64-unknown-linux-gnu`.
- macOS Intel: `x86_64-apple-darwin`.
- macOS Apple Silicon: `aarch64-apple-darwin`.

These declarations express intended build coverage. They are not evidence that
every target has passed the current release's build, dependency, feature and
runtime checks. Use successful jobs and matching artifacts from the exact
release commit when assessing available binaries.

## Verification for this correction

Selected Linux x86_64 builds and focused tests were verified during this
diligence batch. This platform-declaration change was checked with parsed YAML,
a synthetic release-summary invocation and workflow linting; no release was
dispatched or artifact published. It did not cross-compile or run the musl,
Linux ARM64, macOS or native Windows targets. Complete platform/feature coverage
and broad portability remain unverified by this change.
