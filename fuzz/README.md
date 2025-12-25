# Rustible Callback System Fuzz Tests

This directory contains fuzz testing infrastructure for the Rustible callback system.

## Test Categories

### 1. Event Type Parsing (`fuzz_callback_event`)
Tests robustness of event type creation and processing:
- Task status parsing (ok, changed, failed, skipped, unreachable)
- Event type validation
- Play stats calculation with arbitrary values
- Duration handling

### 2. Configuration Parsing (`fuzz_callback_config`)
Tests configuration parsing with random values:
- Plugin name validation
- Output destination parsing (stdout, stderr, file paths)
- Verbosity level handling (0-5)
- Config option parsing (bool, int, float, string, array)
- Priority ordering

### 3. Plugin Name Resolution (`fuzz_plugin_resolution`)
Tests plugin name matching and resolution:
- Exact matching
- Case-insensitive matching
- Prefix/suffix matching
- Alias resolution (e.g., "min" -> "minimal")
- Namespace handling (e.g., "rustible.callback.json")
- Version handling (e.g., "json@2.0")

### 4. Large Event Data (`fuzz_large_event_data`)
Tests handling of large payloads:
- Large host names (up to 1KB)
- Large task names (up to 4KB)
- Large stdout/stderr output (up to 1000 lines)
- Large warning lists (up to 100 items)
- Large notify handler lists (up to 50 items)
- Large fact maps (up to 256 entries)
- Concurrent event simulation

## Running Fuzz Tests

### Using proptest (no special setup required)

```bash
# Run all property-based fuzz tests
cargo test --test callback_fuzz_tests

# Run with more iterations
PROPTEST_CASES=10000 cargo test --test callback_fuzz_tests

# Run specific test
cargo test --test callback_fuzz_tests test_large_event_data
```

### Using cargo-fuzz (requires nightly)

First, install cargo-fuzz:
```bash
cargo install cargo-fuzz
```

Then run fuzzing:
```bash
# Fuzz event type parsing
cargo +nightly fuzz run fuzz_callback_event

# Fuzz configuration parsing
cargo +nightly fuzz run fuzz_callback_config

# Fuzz plugin name resolution
cargo +nightly fuzz run fuzz_plugin_resolution

# Fuzz large event data handling
cargo +nightly fuzz run fuzz_large_event_data
```

## Fuzz Targets

| Target | Description | Key Areas |
|--------|-------------|-----------|
| `fuzz_callback_event` | Event creation/processing | Status parsing, stats calculation |
| `fuzz_callback_config` | Configuration handling | Plugin names, verbosity, options |
| `fuzz_plugin_resolution` | Plugin name resolution | Matching, aliases, namespacing |
| `fuzz_large_event_data` | Large payload handling | Memory safety, performance |

## Known Plugins

The fuzzer tests against these known callback plugins:
- `default` - Standard Ansible-like colored output
- `minimal` - Minimal output showing only changes/failures
- `oneline` - Compact single-line output
- `json` - JSON-formatted machine-readable output
- `yaml` - YAML-formatted output
- `timer` - Task timing information
- `tree` - Hierarchical directory output
- `diff` - Diff-focused output
- `junit` - JUnit XML test reports
- `notification` - External notifications (Slack, email, webhook)
- `dense` - Compact dense output
- `forked` - Parallel execution output
- `selective` - Filtered output
- `counter` - Statistics counter
- `null` - No output (for testing)
- `profile_tasks` - Task profiling

## Contributing

When adding new callback features:
1. Add corresponding fuzz test coverage
2. Update the proptest tests in `tests/callback_fuzz_tests.rs`
3. Add cargo-fuzz targets if needed
4. Run extended fuzzing: `PROPTEST_CASES=100000 cargo test --test callback_fuzz_tests`
