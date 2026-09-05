# Callback production fuzz targets

The four libFuzzer targets call Rustible production APIs through the same
bounded entrypoints used by `tests/callback_fuzz_tests.rs`.
They previously exercised independent stand-ins, so historical runs of those
targets and the old callback property suite are not evidence of product coverage.

## Exact coverage

- `fuzz_callback_config`: real `CallbackConfig` JSON parsing/serialization,
  the verbosity setter (0–4), and in-memory plugin enable/disable methods.
- `fuzz_callback_event`: real `CallbackEvent` JSON parsing, classification,
  host/handler/failure accessors and serialization round trips.
- `fuzz_plugin_resolution`: the real `PluginFactory` name lookup and construction.
  No callback is executed. Lookup and construction must agree for every input.
  Unsupported names must remain ordinary errors; invented prefix/suffix,
  namespace, version and alias support is not simulated.
- `fuzz_large_event_data`: real `ResultInfo::with_output` UTF-8 truncation,
  bounded result payloads and JSON round trips.

Every entrypoint caps raw input at 65,536 bytes. Invalid UTF-8 is represented
lossily where a string is required; the real JSON parsers also receive the raw
bytes. This is a bounded robustness campaign, not a load/concurrency benchmark,
memory-safety proof, full callback-manager audit or whole-project coverage.

The harness does not read configuration files, invoke the environment
configuration loader, execute tasks, emit callbacks, write user-selected files
or contact a network. Plugin construction can probe stdout's terminal status
and read the NO_COLOR display preference; these probes are not fully hermetic.
The fuzz package selects the local-only feature configuration. Default, optional
and all-feature compatibility require separate checks.

## Deterministic tests

Run from the repository root:

```bash
PROPTEST_CASES=128 cargo test --locked --test callback_fuzz_tests -- --test-threads=1
```

The actual production functions live in `tests/common/callback_fuzz.rs`;
both the normal test and fuzz package compile that same file. Deterministic
controls prove known configuration/event/plugin behavior and Unicode boundaries.
The suite respects `PROPTEST_CASES` rather than overriding it per group.
Other suites, including `proptest_tests`, have separate coverage dispositions.

## Bounded libFuzzer campaign

Requires nightly Rust and cargo-fuzz. Install the versions chosen for the
campaign, record them, then build once from the repository root:

```bash
cargo +nightly fuzz build
mkdir -p fuzz/corpus
cp -R fuzz/seeds/. fuzz/corpus/
cargo +nightly fuzz run fuzz_callback_event -- -max_total_time=30 -timeout=2 -rss_limit_mb=512 -max_len=65536
cargo +nightly fuzz run fuzz_callback_config -- -max_total_time=30 -timeout=2 -rss_limit_mb=512 -max_len=65536
cargo +nightly fuzz run fuzz_plugin_resolution -- -max_total_time=30 -timeout=2 -rss_limit_mb=512 -max_len=65536
cargo +nightly fuzz run fuzz_large_event_data -- -max_total_time=30 -timeout=2 -rss_limit_mb=512 -max_len=65536
```

Also run each campaign under an outer process timeout and disposable resource
limits. The default address sanitizer reserves a large virtual address range:
do not impose a small virtual-memory limit that prevents sanitizer startup.
Record source commit/dirty hashes, tool versions, lockfile, seed corpus, duration,
executions, crashes and coverage output. Seed valid event/configuration JSON and
known plugin names to reach successful branches. Preserve minimized synthetic
crash inputs, never user output or credentials. A passing short run cannot prove
absence of defects.

## Recorded campaign

One bounded run per target on 2026-09-05 from the exact source in this branch:
`cargo +nightly fuzz build --dev` (AddressSanitizer, unoptimized) with
rustc 1.100.0-nightly (a69a63265 2026-09-03) and cargo-fuzz 0.13.2. Each target
ran once in a disposable container with no network, a read-only root, all
capabilities dropped and a 2 GB memory cap, starting from the committed seeds
with `-seed=20260905 -max_total_time=30 -timeout=2 -rss_limit_mb=512
-max_len=65536` under an outer 45-second process timeout.

| Target | Runs | Exec/s | Edges (start → end) | New units | Peak RSS | Crashes/timeouts |
|---|---|---|---|---|---|---|
| `fuzz_callback_config` | 32,000 | 1,032 | 2,497 → 4,109 | 828 | 277 MB | 0 |
| `fuzz_callback_event` | 90,241 | 2,911 | 2,437 → 3,680 | 1,432 | 275 MB | 0 |
| `fuzz_plugin_resolution` | 253,838 | 8,188 | 914 → 1,018 | 17 | 279 MB | 0 |
| `fuzz_large_event_data` | 6,604 | 213 | 2,464 → 2,809 | 337 | 258 MB | 0 |

"Edges" are libFuzzer's inline 8-bit counter coverage (`cov:`), not line
coverage. Limits: 30 seconds per target, one fixed seed, an unoptimized build,
and generated corpora that are not committed. `fuzz_plugin_resolution`
saturates quickly because the accepted plugin-name space is small. A clean
short run shows only that these inputs produced no crash, timeout or sanitizer
report; it says nothing about other callback code or the rest of the project.

## Compatibility

No public callback API or plugin aliases are added by this coverage repair.
Output truncation retains the 10,000-byte prefix budget and existing suffix;
a multibyte character crossing the cutoff is omitted whole instead of panicking.
Short strings and ASCII output keep their previous successful behavior.
Duration JSON keeps its seconds/nanoseconds representation and normalizes
nanoseconds above one second. Values exceeding `Duration::MAX` now return a
deserialization error instead of panicking.
