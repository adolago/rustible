# Test Coverage Quick Reference

## Run Coverage Report

```bash
# Full coverage report with HTML output
cargo tarpaulin --out Html --output-dir docs/coverage

# Coverage for specific module
cargo tarpaulin --lib --packages rustible -- --test-threads=1

# Coverage with line-by-line details
cargo tarpaulin --out Html --out Stdout --output-dir docs/coverage
```

## Current Status (2025-12-25)

**Overall:** 38.53% (6,536/16,965 lines)  
**Tests:** 565 passing, 5 ignored  
**Grade:** B- (Good infrastructure, needs implementation coverage)

## Critical Gaps

| Component | Coverage | Lines Untested | Priority |
|-----------|----------|----------------|----------|
| russh.rs | 2.6% | 1,214 | 🔴 CRITICAL |
| executor/mod.rs | 12.9% | 216 | 🔴 CRITICAL |
| executor/task.rs | 17.7% | 438 | 🔴 CRITICAL |
| apt.rs | 6.6% | 214 | 🔴 HIGH |
| dnf.rs | 7.4% | 187 | 🔴 HIGH |
| yum.rs | 7.0% | 198 | 🔴 HIGH |
| user.rs | 4.2% | 293 | 🔴 HIGH |
| service.rs | 3.8% | 256 | 🔴 HIGH |

## Top Priorities

### 1. russh Connection (1,880 lines)
```bash
# Run russh tests
cargo test --test russh_tests
cargo test --test russh_connection_tests

# Add comprehensive tests in:
tests/russh_comprehensive_tests.rs
```

### 2. Executor Core (780 lines)
```bash
# Run executor tests
cargo test --test executor_tests
cargo test --test strategy_tests

# Add deep tests in:
tests/executor_deep_tests.rs
```

### 3. Package Modules (1,100 lines)
```bash
# Run module tests
cargo test --test module_tests
cargo test --lib --package rustible modules::

# Add package tests in:
tests/package_modules_tests.rs
```

## Well-Tested Components ✓

- `vault.rs`: 98.0% (49/50) ✓✓✓
- `template.rs`: 81.8% (9/11) ✓✓
- `junit.rs`: 96.9% (190/196) ✓✓
- `timer.rs`: 84.5% (207/245) ✓✓
- `tree.rs`: 85.0% (182/214) ✓✓

## Coverage Reports

- **HTML Report**: `/home/artur/Repositories/rustible/docs/coverage/tarpaulin-report.html`
- **Full Analysis**: `/home/artur/Repositories/rustible/docs/test_coverage_analysis.md`
- **Action Plan**: `/home/artur/Repositories/rustible/docs/coverage_summary.md`
- **Executive Summary**: `/home/artur/Repositories/rustible/docs/COVERAGE_REPORT.md`

## Quick Commands

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run all tests
cargo test

# Run specific test file
cargo test --test ssh_tests

# Run tests with output
cargo test -- --nocapture

# Run integration tests only
cargo test --tests

# Run unit tests only
cargo test --lib

# Run benchmarks
cargo bench

# Check test count
cargo test 2>&1 | grep "test result"
```

## CI/CD Integration

```yaml
# .github/workflows/coverage.yml
- name: Run coverage
  run: cargo tarpaulin --out Xml --output-dir coverage
  
- name: Upload to Codecov
  uses: codecov/codecov-action@v3
  with:
    files: coverage/cobertura.xml
```

## Coverage Goals

| Sprint | Target | Focus Areas |
|--------|--------|-------------|
| Sprint 1 | 52% | russh, executor, packages |
| Sprint 2 | 61% | parser, system modules, errors |
| Sprint 3 | 66% | callbacks, variables, inventory |
| Sprint 4 | 71% | docker, CLI, remaining modules |
| Sprint 5 | 80%+ | edge cases, polish |

## Testing Best Practices

1. ✅ Write tests before implementation (TDD)
2. ✅ Test both happy path and error cases
3. ✅ Use property-based testing for complex logic
4. ✅ Test against real infrastructure when possible
5. ✅ Keep tests fast and isolated
6. ✅ Use descriptive test names
7. ✅ Run coverage after each PR

## Need Help?

See detailed reports in `/docs/`:
- `test_coverage_analysis.md` - Complete breakdown
- `coverage_summary.md` - Action plan with examples
- `COVERAGE_REPORT.md` - Executive summary
