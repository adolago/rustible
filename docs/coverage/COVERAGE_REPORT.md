# Rustible Test Coverage Analysis Report

**Generated:** 2025-12-25
**Analysis Type:** Static Analysis (automated coverage tools blocked by build issues)

## Executive Summary

Based on static analysis of source files and test files, this report identifies coverage gaps and provides a prioritized action plan to achieve 80%+ overall coverage.

## Current Coverage Assessment

### Source Code Statistics

| Category | Files | Total Lines | Files with Inline Tests | Estimated Coverage |
|----------|-------|-------------|------------------------|-------------------|
| **Connection** | 8 | ~10,000 | 7/8 (87.5%) | ~60% |
| **Executor** | 6 | ~8,500 | 6/6 (100%) | ~65% |
| **Modules** | 21 | ~18,000 | 21/21 (100%) | ~55% |
| **Parser** | 2 | ~2,500 | 2/2 (100%) | ~70% |
| **Callback** | 25 | ~25,000 | 24/25 (96%) | ~50% |
| **Inventory** | 4 | ~4,800 | 4/4 (100%) | ~65% |
| **CLI** | 5 | ~3,000 | 5/5 (100%) | ~60% |
| **Other** | 6 | ~4,500 | 5/6 (83%) | ~55% |

**Estimated Overall Coverage: ~58%**

### Test Infrastructure Summary

| Test Category | Test Files | Total Tests | Lines |
|---------------|------------|-------------|-------|
| CLI Tests | 1 | 223 | 3,821 |
| Module Tests | 1 | 190 | 4,078 |
| Variable Precedence | 1 | 139 | 2,673 |
| Inventory Tests | 1 | 141 | 2,973 |
| Ansible Compat | 1 | 129 | 3,998 |
| Error Tests | 1 | 123 | 2,434 |
| Config Tests | 1 | 123 | 3,012 |
| Become Tests | 1 | 104 | 1,692 |
| Vault Tests | 1 | 97 | 1,520 |
| Connection Tests | 1 | 91 | 2,541 |
| Other (55+ files) | 55+ | ~2,000+ | ~100k+ |

**Total External Tests: ~3,500+**

---

## Critical Coverage Gaps (Priority 1: HIGH)

### 1. Connection: `local.rs` - **0 Inline Tests**
**File:** `/src/connection/local.rs` (557 lines)
**Risk:** High - Local connection is used for all localhost operations

**Missing Test Coverage:**
- `build_command()` - escalation methods (sudo, su, doas)
- `execute()` - timeout handling, error conditions
- `upload()` / `upload_content()` - file operations
- `download()` / `download_content()` - file retrieval
- `stat()` - file metadata
- Privilege escalation password handling

**Action Items:**
```rust
#[cfg(test)]
mod tests {
    // Test privilege escalation command building
    // Test environment variable handling
    // Test timeout behavior
    // Test file upload/download operations
    // Test error handling for missing files
}
```

### 2. Callback: `manager.rs` - **0 Inline Tests**
**File:** `/src/callback/manager.rs` (1,183 lines)
**Risk:** High - Central callback orchestration

**Missing Test Coverage:**
- Plugin registration and lifecycle
- Event dispatching to multiple plugins
- Error handling in callbacks
- Concurrent callback execution

### 3. Callback: `mod.rs` - **0 Inline Tests**
**File:** `/src/callback/mod.rs` (334 lines)
**Risk:** Medium - Core callback types and exports

---

## Executor Module Gaps (Priority 1: HIGH)

### 4. Executor: `mod.rs` - Only 3 Tests
**File:** `/src/executor/mod.rs` (1,194 lines)
**Coverage Gap:** ~85% of code untested

**Critical Untested Functions:**
- `execute_play()` - core play execution logic
- Error recovery and block/rescue handling
- Handler notification system
- Serial execution modes

### 5. Executor: `parallelization.rs` - Only 1 Test
**File:** `/src/executor/parallelization.rs` (529 lines)
**Risk:** High - Parallel execution is a core feature

**Missing Tests:**
- `ParallelizationStrategy` variants
- Batch size calculations
- Load balancing across hosts
- Failure handling in parallel mode

---

## Module Coverage Gaps (Priority 2: MEDIUM)

### 6. Modules with Fewer Tests Than Expected

| Module | Lines | Tests | Test Density | Target |
|--------|-------|-------|--------------|--------|
| `package.rs` | 532 | 3 | 0.6% | 10+ |
| `yum.rs` | 631 | 6 | 1.0% | 12+ |
| `python.rs` | 633 | 6 | 1.0% | 10+ |
| `user.rs` | 834 | 5 | 0.6% | 15+ |
| `group.rs` | 457 | 5 | 1.1% | 10+ |
| `service.rs` | 1,406 | 7 | 0.5% | 20+ |

### 7. Specific Module Test Gaps

**`service.rs` (1,406 lines, 7 tests):**
- systemd vs sysvinit detection
- Service state transitions
- Enable/disable operations
- Daemon reload

**`user.rs` (834 lines, 5 tests):**
- User creation with all options
- Password handling (encrypted, cleartext)
- Group membership modifications
- Shell/home directory changes

**`copy.rs` (944 lines, 6 tests):**
- Remote-to-remote copy
- Recursive directory copy
- Backup creation
- Ownership preservation

---

## Parser Gaps (Priority 2: MEDIUM)

### 8. Parser: Complex YAML Parsing
**Files:** `/src/parser/mod.rs` (1,296 lines), `/src/parser/playbook.rs` (1,180 lines)

**Untested Edge Cases:**
- Malformed YAML error handling
- Unicode in task names/variables
- Very deep nesting (10+ levels)
- Large playbooks (1000+ tasks)
- YAML anchor/alias handling

---

## Connection Module Gaps (Priority 2: MEDIUM)

### 9. SSH Connection Pools
**File:** `/src/connection/russh_pool.rs` (1,703 lines, 6 tests)

**Missing Tests:**
- Pool exhaustion behavior
- Connection recycling
- Timeout handling
- Concurrent checkout/checkin
- Stale connection detection

### 10. Docker Connection
**File:** `/src/connection/docker.rs` (639 lines, 6 tests)

**Missing Tests:**
- Container not found errors
- Permission denied scenarios
- Large file transfers
- Stream handling

---

## Test Infrastructure Issues (Priority 3: LOW)

### 11. Integration Tests Without `#[test]` Annotations

Several test files have 0 tests detected, likely using `#[tokio::test]` or integration patterns:

- `strategy_tests.rs` (2,452 lines, 0 detected)
- `scenario_tests.rs` (3,610 lines, 0 detected)
- `chaos_tests.rs` (970 lines, 0 detected)

**Recommendation:** Verify these are running in CI, not just being skipped.

---

## Prioritized Action Plan

### Phase 1: Critical Path (Target: +15% coverage)

1. **Add inline tests to `local.rs`**
   - Estimate: 15-20 tests
   - Focus: Command building, file operations

2. **Add inline tests to `callback/manager.rs`**
   - Estimate: 10-15 tests
   - Focus: Plugin lifecycle, event dispatch

3. **Expand `executor/mod.rs` tests**
   - Estimate: 20-25 tests
   - Focus: Play execution, error handling

4. **Add `parallelization.rs` tests**
   - Estimate: 10-15 tests
   - Focus: Strategy selection, batch processing

### Phase 2: Module Hardening (Target: +10% coverage)

5. **Service module deep testing**
   - All init systems (systemd, sysvinit, upstart)
   - State transitions

6. **User/Group module edge cases**
   - Password scenarios
   - Membership changes

7. **Copy module comprehensive tests**
   - Recursive operations
   - Permission preservation

### Phase 3: Edge Cases (Target: +5% coverage)

8. **Parser fuzzing**
   - Malformed input handling
   - Unicode edge cases

9. **Connection pool stress tests**
   - High concurrency
   - Failure recovery

10. **Callback plugin isolation tests**
    - Each plugin independently verified

---

## Coverage Targets by Module

| Module | Current Est. | Phase 1 Target | Final Target |
|--------|-------------|----------------|--------------|
| Connection | 60% | 75% | 85% |
| Executor | 65% | 80% | 90% |
| Modules | 55% | 70% | 80% |
| Parser | 70% | 80% | 85% |
| Callback | 50% | 65% | 80% |
| Inventory | 65% | 75% | 85% |
| CLI | 60% | 75% | 85% |
| **Overall** | **58%** | **73%** | **83%** |

---

## Recommended Test Additions (Detailed)

### 1. LocalConnection Tests (Priority: Critical)

```rust
// Tests to add to src/connection/local.rs

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_local_connection_new() {
        let conn = LocalConnection::new();
        assert!(!conn.identifier.is_empty());
    }

    #[test]
    fn test_local_connection_with_identifier() {
        let conn = LocalConnection::with_identifier("test-host");
        assert_eq!(conn.identifier, "test-host");
    }

    #[test]
    fn test_build_command_simple() {
        let conn = LocalConnection::new();
        let options = ExecuteOptions::default();
        let cmd = conn.build_command("echo test", &options);
        // Verify command structure
    }

    #[test]
    fn test_build_command_with_sudo() {
        let conn = LocalConnection::new();
        let options = ExecuteOptions::new()
            .with_escalation(Some("root".to_string()));
        let cmd = conn.build_command("whoami", &options);
        // Verify sudo is used
    }

    #[test]
    fn test_build_command_with_su() {
        let conn = LocalConnection::new();
        let mut options = ExecuteOptions::new()
            .with_escalation(Some("root".to_string()));
        options.escalate_method = Some("su".to_string());
        let cmd = conn.build_command("whoami", &options);
        // Verify su is used
    }

    #[test]
    fn test_build_command_with_doas() {
        let conn = LocalConnection::new();
        let mut options = ExecuteOptions::new()
            .with_escalation(Some("root".to_string()));
        options.escalate_method = Some("doas".to_string());
        let cmd = conn.build_command("whoami", &options);
        // Verify doas is used
    }

    #[test]
    fn test_build_command_with_cwd() {
        let conn = LocalConnection::new();
        let options = ExecuteOptions::new().with_cwd("/tmp");
        let cmd = conn.build_command("pwd", &options);
        // Verify cwd is set
    }

    #[test]
    fn test_build_command_with_env() {
        let conn = LocalConnection::new();
        let options = ExecuteOptions::new()
            .with_env("FOO", "bar");
        let cmd = conn.build_command("echo $FOO", &options);
        // Verify env is set
    }

    #[tokio::test]
    async fn test_execute_simple() {
        let conn = LocalConnection::new();
        let result = conn.execute("echo hello", None).await.unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_failure() {
        let conn = LocalConnection::new();
        let result = conn.execute("exit 1", None).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
    }

    #[tokio::test]
    async fn test_is_alive() {
        let conn = LocalConnection::new();
        assert!(conn.is_alive().await);
    }

    #[tokio::test]
    async fn test_path_exists() {
        let conn = LocalConnection::new();
        assert!(conn.path_exists(Path::new("/")).await.unwrap());
        assert!(!conn.path_exists(Path::new("/nonexistent_path_12345")).await.unwrap());
    }

    #[tokio::test]
    async fn test_is_directory() {
        let conn = LocalConnection::new();
        assert!(conn.is_directory(Path::new("/tmp")).await.unwrap());
        assert!(!conn.is_directory(Path::new("/etc/passwd")).await.unwrap());
    }

    #[tokio::test]
    async fn test_upload_content() {
        let conn = LocalConnection::new();
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");

        conn.upload_content(b"test content", &path, None).await.unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test content");
    }

    #[tokio::test]
    async fn test_download_content() {
        let conn = LocalConnection::new();
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "download test").unwrap();

        let content = conn.download_content(&path).await.unwrap();
        assert_eq!(content, b"download test");
    }

    #[tokio::test]
    async fn test_stat_file() {
        let conn = LocalConnection::new();
        let stat = conn.stat(Path::new("/etc/passwd")).await.unwrap();
        assert!(stat.is_file);
        assert!(!stat.is_dir);
    }

    #[tokio::test]
    async fn test_stat_directory() {
        let conn = LocalConnection::new();
        let stat = conn.stat(Path::new("/tmp")).await.unwrap();
        assert!(!stat.is_file);
        assert!(stat.is_dir);
    }
}
```

### 2. Executor Tests to Add

```rust
// Tests to add to src/executor/mod.rs

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_executor_builder_defaults() {
        let executor = Executor::builder().build().unwrap();
        assert_eq!(executor.forks, 5); // default
        assert!(!executor.check_mode);
    }

    #[test]
    fn test_executor_builder_custom_forks() {
        let executor = Executor::builder()
            .forks(10)
            .build()
            .unwrap();
        assert_eq!(executor.forks, 10);
    }

    #[test]
    fn test_executor_check_mode() {
        let executor = Executor::builder()
            .check_mode(true)
            .build()
            .unwrap();
        assert!(executor.check_mode);
    }

    #[test]
    fn test_serial_spec_fixed() {
        let spec = SerialSpec::Fixed(5);
        assert_eq!(spec.batch_size(100), 5);
    }

    #[test]
    fn test_serial_spec_percentage() {
        let spec = SerialSpec::Percentage("50%".to_string());
        assert_eq!(spec.batch_size(100), 50);
    }

    #[test]
    fn test_serial_spec_progressive() {
        let spec = SerialSpec::Progressive(vec![
            SerialSpec::Fixed(1),
            SerialSpec::Fixed(5),
            SerialSpec::Fixed(10),
        ]);
        // Test progressive batch sizing
    }

    #[test]
    fn test_task_result_success() {
        let result = TaskResult::success("test".to_string());
        assert!(result.is_success());
        assert!(!result.changed);
    }

    #[test]
    fn test_task_result_changed() {
        let result = TaskResult::changed("test".to_string());
        assert!(result.is_success());
        assert!(result.changed);
    }

    #[test]
    fn test_task_result_failed() {
        let result = TaskResult::failed("error".to_string());
        assert!(!result.is_success());
    }

    #[test]
    fn test_play_result_aggregation() {
        let mut play_result = PlayResult::new("test play");
        play_result.add_host_result("host1", TaskResult::success("ok"));
        play_result.add_host_result("host2", TaskResult::changed("changed"));

        assert_eq!(play_result.host_count(), 2);
        assert_eq!(play_result.changed_count(), 1);
    }
}
```

### 3. Parallelization Tests

```rust
// Tests to add to src/executor/parallelization.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_linear() {
        let strategy = ParallelizationStrategy::Linear;
        assert_eq!(strategy.batch_size(100, 5), 1);
    }

    #[test]
    fn test_strategy_free() {
        let strategy = ParallelizationStrategy::Free;
        assert_eq!(strategy.batch_size(100, 5), 5);
    }

    #[test]
    fn test_strategy_serial_fixed() {
        let strategy = ParallelizationStrategy::Serial(SerialSpec::Fixed(10));
        assert_eq!(strategy.batch_size(100, 5), 10);
    }

    #[test]
    fn test_batch_hosts_empty() {
        let hosts: Vec<String> = vec![];
        let batches = batch_hosts(&hosts, 5);
        assert!(batches.is_empty());
    }

    #[test]
    fn test_batch_hosts_smaller_than_batch() {
        let hosts = vec!["h1", "h2", "h3"];
        let batches = batch_hosts(&hosts, 10);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 3);
    }

    #[test]
    fn test_batch_hosts_exact_multiple() {
        let hosts: Vec<_> = (0..10).map(|i| format!("h{}", i)).collect();
        let batches = batch_hosts(&hosts, 5);
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn test_batch_hosts_with_remainder() {
        let hosts: Vec<_> = (0..13).map(|i| format!("h{}", i)).collect();
        let batches = batch_hosts(&hosts, 5);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[2].len(), 3);
    }
}
```

---

## Verification Commands

After implementing the test additions, run:

```bash
# Run all tests
cargo test --all-features

# Run with coverage (when tarpaulin works)
cargo tarpaulin --out Html --output-dir docs/coverage/

# Run specific module tests
cargo test --lib connection::local::tests
cargo test --lib executor::tests
cargo test --lib modules::tests

# Run integration tests
cargo test --test '*'
```

---

## Conclusion

The Rustible project has a solid test foundation with ~3,500+ external tests and inline tests in most modules. However, several critical paths lack adequate coverage:

1. **LocalConnection** has no inline tests despite being fundamental
2. **CallbackManager** orchestrates all output but has no unit tests
3. **Parallelization** is under-tested for a core feature
4. **Several modules** have low test density relative to complexity

Following the prioritized action plan above should bring coverage from an estimated 58% to 83%+, with particular focus on the critical connection and executor paths.
