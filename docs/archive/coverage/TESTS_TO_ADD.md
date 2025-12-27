# Tests To Add - Prioritized Checklist

This file tracks the specific tests that need to be added to reach 80%+ coverage.

## Priority 1: Critical (Must Have)

### Connection Module

- [ ] **`src/connection/local.rs`** - Add inline test module
  - [ ] `test_local_connection_new()`
  - [ ] `test_local_connection_with_identifier()`
  - [ ] `test_build_command_simple()`
  - [ ] `test_build_command_with_sudo()`
  - [ ] `test_build_command_with_su()`
  - [ ] `test_build_command_with_doas()`
  - [ ] `test_build_command_with_cwd()`
  - [ ] `test_build_command_with_env()`
  - [ ] `test_build_command_with_timeout()`
  - [ ] `test_execute_simple()` (async)
  - [ ] `test_execute_with_env()` (async)
  - [ ] `test_execute_failure()` (async)
  - [ ] `test_execute_with_timeout()` (async)
  - [ ] `test_is_alive()` (async)
  - [ ] `test_path_exists_true()` (async)
  - [ ] `test_path_exists_false()` (async)
  - [ ] `test_is_directory_true()` (async)
  - [ ] `test_is_directory_false()` (async)
  - [ ] `test_upload_content()` (async)
  - [ ] `test_upload_file()` (async)
  - [ ] `test_download_content()` (async)
  - [ ] `test_download_file()` (async)
  - [ ] `test_stat_file()` (async)
  - [ ] `test_stat_directory()` (async)
  - [ ] `test_stat_nonexistent()` (async)
  - [ ] `test_close()` (async)

### Callback Module

- [ ] **`src/callback/manager.rs`** - Add inline test module
  - [ ] `test_callback_manager_new()`
  - [ ] `test_register_plugin()`
  - [ ] `test_unregister_plugin()`
  - [ ] `test_dispatch_playbook_start()`
  - [ ] `test_dispatch_playbook_end()`
  - [ ] `test_dispatch_play_start()`
  - [ ] `test_dispatch_task_start()`
  - [ ] `test_dispatch_task_result_ok()`
  - [ ] `test_dispatch_task_result_changed()`
  - [ ] `test_dispatch_task_result_failed()`
  - [ ] `test_dispatch_to_multiple_plugins()`
  - [ ] `test_plugin_error_handling()`
  - [ ] `test_concurrent_dispatch()`

### Executor Module

- [ ] **`src/executor/mod.rs`** - Expand existing tests
  - [ ] `test_executor_builder_defaults()`
  - [ ] `test_executor_builder_forks()`
  - [ ] `test_executor_builder_check_mode()`
  - [ ] `test_executor_builder_diff_mode()`
  - [ ] `test_executor_builder_strategy()`
  - [ ] `test_serial_spec_fixed()`
  - [ ] `test_serial_spec_percentage()`
  - [ ] `test_serial_spec_progressive()`
  - [ ] `test_task_result_success()`
  - [ ] `test_task_result_changed()`
  - [ ] `test_task_result_failed()`
  - [ ] `test_task_result_skipped()`
  - [ ] `test_play_result_aggregation()`
  - [ ] `test_play_result_failure_detection()`
  - [ ] `test_playbook_result_summary()`

- [ ] **`src/executor/parallelization.rs`** - Add more tests
  - [ ] `test_strategy_linear()`
  - [ ] `test_strategy_free()`
  - [ ] `test_strategy_serial()`
  - [ ] `test_batch_hosts_empty()`
  - [ ] `test_batch_hosts_single()`
  - [ ] `test_batch_hosts_exact_multiple()`
  - [ ] `test_batch_hosts_with_remainder()`
  - [ ] `test_load_balancing()`
  - [ ] `test_failure_handling_continue()`
  - [ ] `test_failure_handling_any_errors_fatal()`

## Priority 2: Important

### Module System

- [ ] **`src/modules/service.rs`** - Expand tests
  - [ ] `test_detect_systemd()`
  - [ ] `test_detect_sysvinit()`
  - [ ] `test_detect_upstart()`
  - [ ] `test_start_service()`
  - [ ] `test_stop_service()`
  - [ ] `test_restart_service()`
  - [ ] `test_reload_service()`
  - [ ] `test_enable_service()`
  - [ ] `test_disable_service()`
  - [ ] `test_daemon_reload()`
  - [ ] `test_service_status_check()`
  - [ ] `test_masked_service_handling()`

- [ ] **`src/modules/user.rs`** - Expand tests
  - [ ] `test_create_user_minimal()`
  - [ ] `test_create_user_full_options()`
  - [ ] `test_remove_user()`
  - [ ] `test_modify_user_groups()`
  - [ ] `test_change_password()`
  - [ ] `test_change_shell()`
  - [ ] `test_change_home()`
  - [ ] `test_user_idempotency()`

- [ ] **`src/modules/package.rs`** - Expand tests
  - [ ] `test_detect_package_manager()`
  - [ ] `test_install_single_package()`
  - [ ] `test_install_multiple_packages()`
  - [ ] `test_remove_package()`
  - [ ] `test_package_state_latest()`
  - [ ] `test_package_with_version()`

- [ ] **`src/modules/copy.rs`** - Expand tests
  - [ ] `test_copy_file_simple()`
  - [ ] `test_copy_directory_recursive()`
  - [ ] `test_copy_with_backup()`
  - [ ] `test_copy_preserve_ownership()`
  - [ ] `test_copy_preserve_permissions()`
  - [ ] `test_copy_content_param()`
  - [ ] `test_copy_idempotency()`

### Parser Module

- [ ] **`src/parser/mod.rs`** - Add edge case tests
  - [ ] `test_parse_empty_yaml()`
  - [ ] `test_parse_malformed_yaml()`
  - [ ] `test_parse_unicode_names()`
  - [ ] `test_parse_deep_nesting()`
  - [ ] `test_parse_yaml_anchors()`
  - [ ] `test_parse_yaml_aliases()`
  - [ ] `test_parse_multiline_strings()`
  - [ ] `test_parse_special_characters()`

### Connection Pool

- [ ] **`src/connection/russh_pool.rs`** - Expand tests
  - [ ] `test_pool_checkout()`
  - [ ] `test_pool_checkin()`
  - [ ] `test_pool_exhaustion()`
  - [ ] `test_pool_recycling()`
  - [ ] `test_pool_timeout()`
  - [ ] `test_stale_connection_eviction()`
  - [ ] `test_concurrent_checkout()`
  - [ ] `test_pool_stats()`

## Priority 3: Nice to Have

### CLI Module

- [ ] **`src/cli/commands/run.rs`** - Add tests
  - [ ] `test_parse_run_args()`
  - [ ] `test_limit_parsing()`
  - [ ] `test_tags_parsing()`
  - [ ] `test_extra_vars_parsing()`
  - [ ] `test_check_mode_flag()`

### Inventory Module

- [ ] **`src/inventory/plugin.rs`** - Expand tests
  - [ ] `test_script_inventory_execution()`
  - [ ] `test_script_inventory_caching()`
  - [ ] `test_script_inventory_error_handling()`

### Vars Module

- [ ] **`src/vars/mod.rs`** - Expand tests
  - [ ] `test_variable_interpolation()`
  - [ ] `test_nested_variable_resolution()`
  - [ ] `test_undefined_variable_handling()`
  - [ ] `test_variable_precedence()`

---

## Test Implementation Notes

### Async Test Setup

For async tests, use:
```rust
#[tokio::test]
async fn test_name() {
    // test code
}
```

### Temporary Files

For file operation tests, use:
```rust
use tempfile::tempdir;

#[test]
fn test_with_files() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    // use file_path
}
```

### Mocking Connections

For tests that need mock connections:
```rust
#[cfg(test)]
struct MockConnection {
    // fields
}

#[cfg(test)]
#[async_trait]
impl Connection for MockConnection {
    // implement required methods
}
```

---

## Progress Tracking

| Module | Tests Planned | Tests Added | Coverage Delta |
|--------|---------------|-------------|----------------|
| local.rs | 26 | 0 | - |
| manager.rs | 13 | 0 | - |
| executor/mod.rs | 15 | 0 | - |
| parallelization.rs | 10 | 0 | - |
| service.rs | 12 | 0 | - |
| user.rs | 8 | 0 | - |
| package.rs | 6 | 0 | - |
| copy.rs | 7 | 0 | - |
| parser/mod.rs | 8 | 0 | - |
| russh_pool.rs | 8 | 0 | - |

**Total Tests Planned:** ~113
**Total Tests Added:** 0
**Estimated Coverage Improvement:** +25%
