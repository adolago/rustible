# Rustible Test Failure Eradication Plan

## Executive Summary

**Current State:** ~3246 tests passing, ~30 failing (~99.1% pass rate)
**Goal:** 100% pass rate with README claims matching reality

---

## Phase 1: Ansible Boolean Compatibility (4 failures)

**Problem:** YAML parsing rejects Ansible-style booleans (`yes`/`no`/`True`/`False`/`on`/`off`)

**Failing Tests:**
- `test_async_with_become` - `become: "yes"` fails
- `test_load_async_edge_cases_fixture` - `ignore_errors: "yes"` fails
- 2 more async tests with similar issues

**Root Cause:** Custom deserializers for booleans exist in `playbook.rs` but aren't applied to all boolean fields consistently.

**Fix Strategy:**
1. Create a reusable `deserialize_ansible_bool` function
2. Apply `#[serde(deserialize_with = "deserialize_ansible_bool")]` to ALL boolean fields:
   - `become`, `ignore_errors`, `no_log`, `run_once`, `check_mode`, `diff`, `changed_when`, `failed_when`, `gather_facts`
3. Apply to both `Task` struct and `Play` struct

**Files to Modify:**
- `src/playbook.rs`
- `src/executor/task.rs`
- `src/executor/playbook.rs`

**Estimated Effort:** 1 agent, 30 minutes

---

## Phase 2: Block Parsing Enhancement (9 failures)

**Problem:** Block tasks don't properly inherit/support all task attributes

**Failing Tests:**
- `test_parse_block_with_when_condition`
- `test_parse_block_with_loop`
- `test_parse_block_with_loop_control`
- `test_parse_block_with_async`
- `test_parse_block_task_with_notify`
- `test_parse_rescue_task_with_notify`
- `test_parse_block_tasks_inherit_conditions`
- `test_parse_comprehensive_block_playbook`
- `test_load_block_handler_notify_fixture`

**Root Cause:** The block parsing in `parse_task_definition()` doesn't properly handle all attributes on block-level tasks (loop, when, async, notify).

**Fix Strategy:**
1. Update `parse_task_definition()` to properly handle block-level attributes
2. Ensure block tasks inherit parent `when` conditions
3. Support `loop` on block tasks (iterate the entire block)
4. Support `async` on block tasks
5. Support `notify` at block level

**Files to Modify:**
- `src/executor/playbook.rs` (block parsing)
- `src/executor/task.rs` (block execution with loops)

**Estimated Effort:** 1 agent, 1-2 hours

---

## Phase 3: Python Module / FQCN Support (10 failures)

**Problem:** FQCN parsing, collection discovery, and AnsiballZ bundling are broken

**Failing Tests:**
- `fqcn_support::test_fqcn_parsing_extracts_correct_parts`
- `fqcn_support::test_fqcn_ansible_builtin_format`
- `fqcn_support::test_fqcn_custom_collection_format`
- `fqcn_support::test_fqcn_with_nested_module_path`
- `collection_support::test_collection_structure_ansible_builtin`
- `collection_support::test_find_module_in_collection`
- `collection_support::test_installed_collections_discovery`
- `module_discovery::test_module_path_precedence`
- `integration::test_fqcn_discovery_to_bundle_flow`
- `ansiballz_bundling::test_bundle_includes_base64_encoded_args`

**Root Cause:** The `src/modules/python.rs` module has FQCN parsing and collection discovery implementations that don't match expected behavior.

**Fix Strategy:**
1. Review and fix FQCN parsing logic to correctly extract namespace/collection/module
2. Fix collection path discovery (`~/.ansible/collections`, `/usr/share/ansible/collections`)
3. Fix module resolution within collections
4. Verify AnsiballZ bundling includes all required components

**Files to Modify:**
- `src/modules/python.rs`
- Tests may need fixture path adjustments

**Estimated Effort:** 1-2 agents, 2-3 hours

---

## Phase 4: CLI Edge Cases (3 failures)

**Problem:** CLI edge cases not handled properly

**Failing Tests:**
- `edge_cases::test_invalid_limit_pattern`
- `limit_from_file::test_limit_file_not_found`
- `skip_tags::test_skip_tags_with_list_tasks`

**Fix Strategy:**
1. Review test expectations for invalid limit patterns
2. Ensure proper error handling for missing limit files
3. Fix skip-tags interaction with list-tasks command

**Files to Modify:**
- `src/cli/` modules
- Potentially test assertions if expectations are wrong

**Estimated Effort:** 1 agent, 30 minutes

---

## Phase 5: Handler Statistics (1 failure)

**Problem:** Handler execution statistics not counted correctly

**Failing Test:**
- `test_handler_execution_statistics`

**Fix Strategy:**
1. Review how handler executions are tracked in stats
2. Ensure each handler execution increments the correct counter

**Files to Modify:**
- `src/executor/mod.rs`

**Estimated Effort:** 1 agent, 15 minutes

---

## Phase 6: Module Required Params (1 failure)

**Problem:** Module parameter validation changed

**Failing Test:**
- `test_module_required_params`

**Root Cause:** The command module change (argv without cmd) may have broken expected behavior.

**Fix Strategy:**
1. Review test expectations
2. Either fix module or update test

**Files to Modify:**
- `src/modules/command.rs` or test file

**Estimated Effort:** 1 agent, 15 minutes

---

## Phase 7: Reliability/When Conditions (1 failure)

**Problem:** Conditional execution with `when` clause failing

**Failing Test:**
- `test_conditional_execution_with_when`

**Fix Strategy:**
1. Review when condition evaluation
2. Check if Ansible boolean parsing affects condition strings

**Files to Modify:**
- `src/executor/task.rs` (condition evaluation)

**Estimated Effort:** 1 agent, 30 minutes

---

## Phase 8: Stress Test Edge Cases (1 failure)

**Problem:** Retry delay overflow

**Failing Test:**
- `test_retry_delay_overflow_protection`

**Fix Strategy:**
1. Add overflow protection to retry delay calculations
2. Use saturating arithmetic for delay multipliers

**Files to Modify:**
- `src/executor/task.rs` or wherever retry logic lives

**Estimated Effort:** 1 agent, 15 minutes

---

## Phase 9: Missing Modules (Address README claims)

**Problem:** README claims 18 modules but only ~11 exist

**Missing Modules to Implement:**
1. `group` - Manage groups (similar to user)
2. `apt` - APT-specific package management
3. `yum` - YUM-specific package management
4. `git` - Clone git repositories
5. `debug` - Print debug messages
6. `set_fact` - Set host facts
7. `pause` - Pause execution
8. `wait_for` - Wait for conditions
9. `assert` - Assert conditions

**Fix Strategy:**
Option A: Implement missing modules
Option B: Update README to reflect reality
Option C: Rely on Python fallback for missing modules (but test this works)

**Estimated Effort:** 2-4 hours per module, or 30 min to update README

---

## Execution Order (Recommended)

| Phase | Priority | Reason |
|-------|----------|--------|
| 1 | Critical | Boolean parsing affects many tests |
| 2 | Critical | Block is a core feature |
| 3 | High | Python/FQCN is marketed as "Full" |
| 4-8 | Medium | Individual fixes |
| 9 | Low | Can rely on Python fallback |

---

## Swarm Configuration

### Wave 1 (Parallel - 3 agents)
- Agent 1: Phase 1 (Ansible booleans)
- Agent 2: Phase 2 (Block parsing)
- Agent 3: Phase 3 (Python/FQCN)

### Wave 2 (Parallel - 5 agents)
- Agent 4: Phase 4 (CLI edge cases)
- Agent 5: Phase 5 (Handler stats)
- Agent 6: Phase 6 (Module params)
- Agent 7: Phase 7 (When conditions)
- Agent 8: Phase 8 (Overflow protection)

### Wave 3 (If needed)
- Agent 9: Phase 9 (Missing modules or README update)

---

## Success Criteria

1. All 30 failing tests pass
2. `cargo test` exits with 0
3. `cargo clippy` has no errors
4. README claims match implementation reality

