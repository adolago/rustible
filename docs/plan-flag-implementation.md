# --plan CLI Flag Implementation Summary

## Overview
The `--plan` flag has been successfully implemented in Rustible to provide a dry-run planning mode that shows what would be executed without actually running any tasks. This is similar to Ansible's `--check` flag but with more detailed output inspired by Terraform's plan feature.

## Implementation Details

### 1. CLI Structure
- **File**: `/home/artur/Repositories/rustible/src/cli/commands/run.rs`
- **Flag Added**: Line 76-77
```rust
/// Plan mode - show what would be executed without running
#[arg(long)]
pub plan: bool,
```

### 2. Core Functionality

#### Plan Detection (Lines 131-134)
```rust
if self.plan {
    ctx.output
        .warning("Running in PLAN MODE - showing execution plan only");
}
```

#### Plan Execution (Lines 147-150)
```rust
if self.plan {
    // In plan mode, show what would be executed
    self.show_plan(ctx, plays, &extra_vars).await?;
} else {
    // Normal execution
    for play in plays {
        self.execute_play(ctx, play, &stats).await?;
    }
}
```

### 3. Plan Output Format (Lines 183-347)

The `show_plan` method provides:

#### Play Information
- Play name and index (e.g., `[Play 1/2] ⚡ Test Play`)
- Target hosts pattern and count (e.g., `Hosts: localhost (1 host)`)
- Task count (e.g., `Tasks: 2 tasks`)

#### Task Details
- Task name with index (e.g., `▸ Task 1/2: Install nginx`)
- Module being used (e.g., `Module: package`)
- Action description per host (e.g., `[localhost] will install package: nginx`)
- Conditional execution (e.g., `When: ansible_os_family == "Debian"`)
- Handler notifications (e.g., `Notify: restart app, reload config`)

#### Plan Summary
- Total task count across all plays
- Total host count
- Instruction to execute (e.g., `To execute this plan, run the same command without --plan`)

### 4. Module-Specific Action Descriptions (Lines 349-501)

The implementation includes detailed descriptions for:
- **command/shell**: Shows the command to be executed
- **package/apt/yum/dnf/pip**: Shows install/remove action with package name
- **service**: Shows service state change (started/stopped/restarted)
- **copy**: Shows source and destination paths
- **file**: Shows path and state (file/directory/link)
- **template**: Shows template source and destination
- **user**: Shows create/update/remove action
- **group**: Shows create/update/remove action
- **git**: Shows repository URL and destination
- **debug**: Shows message to be displayed
- **lineinfile/blockinfile**: Shows file modification target

### 5. Variable Templating (Lines 1043-1091)

The plan output supports Jinja2-style variable templating:
- Resolves `{{ variable }}` patterns using play and extra vars
- Shows the actual values that would be used during execution
- Example: `{{ package_name }}` → `nginx`

## Example Output

```
===========================
  PLAYBOOK: test_plan.yml
===========================

WARNING: Running in PLAN MODE - showing execution plan only

EXECUTION PLAN
--------------
Rustible will perform the following actions:

[Play 1/1] ⚡ Test Play
  Hosts: localhost (1 host)
  Tasks: 2 tasks

  ▸ Task 1/2: Install nginx
    Module: package
      [localhost] will install package: nginx

  ▸ Task 2/2: Start nginx service
    Module: service
      [localhost] will started service: nginx
    Notify: Restart nginx

PLAN SUMMARY
-------------
Plan: 2 tasks across 1 host

To execute this plan, run the same command without --plan
```

## Testing

### Test File
- **Location**: `/home/artur/Repositories/rustible/tests/plan_tests.rs`
- **Test Count**: 16 comprehensive tests

### Test Coverage
1. ✅ Basic execution plan display
2. ✅ Task counting and summary
3. ✅ Module-specific action descriptions
4. ✅ Conditional task display
5. ✅ Handler notification display
6. ✅ Variable substitution in plan
7. ✅ Multiple play handling
8. ✅ Tag filtering support
9. ✅ Package module variants (apt, yum, pip)
10. ✅ User and group management modules
11. ✅ Git module support
12. ✅ Template module support
13. ✅ Host information display
14. ✅ Warning message display
15. ✅ Exit code verification
16. ✅ Extra vars integration

## Key Features

### 1. No Execution
- Plan mode does NOT connect to remote hosts
- No actual changes are made
- Safe to run against production systems

### 2. Detailed Output
- Shows exactly what would happen
- Includes variable resolution
- Displays conditional logic
- Shows handler dependencies

### 3. Tag Support
- Respects `--tags` and `--skip-tags` filters
- Only shows tasks that would actually run
- Accurate task counting with filters

### 4. Variable Resolution
- Supports play-level variables
- Supports extra variables (`-e` flag)
- Shows templated values in output

## Usage

```bash
# Basic plan
rustible run playbook.yml --plan

# Plan with extra variables
rustible run playbook.yml --plan -e package_name=nginx

# Plan with tag filters
rustible run playbook.yml --plan --tags install

# Plan with inventory
rustible run playbook.yml --plan -i inventory/hosts.yml

# Plan with verbose output
rustible run playbook.yml --plan -vvv
```

## Integration with Existing Features

### Check Mode vs Plan Mode
- **Check Mode** (`--check`): Connects to hosts but doesn't make changes
- **Plan Mode** (`--plan`): Shows plan without connecting to hosts
- Both can be used together for maximum safety

### Diff Mode
- Plan mode shows what WOULD change
- Diff mode shows HOW it would change
- Complementary features

### Verbosity
- `-v`: Shows INFO level output
- `-vv`: Shows DEBUG level output
- `-vvv`: Shows TRACE level output with detailed execution info

## Performance

- **Zero Network Overhead**: No SSH connections in plan mode
- **Fast Execution**: Only YAML parsing and template resolution
- **Scalable**: Performance independent of host count

## Future Enhancements

Potential improvements for future versions:
1. Cost estimation for cloud resources
2. Dependency graph visualization
3. Risk assessment scoring
4. Change impact analysis
5. Export plan to JSON/YAML format

## Files Modified

1. `/home/artur/Repositories/rustible/src/cli/commands/run.rs`
   - Added `plan: bool` field to `RunArgs` struct
   - Implemented `show_plan()` method
   - Added module-specific action description logic
   - Integrated plan mode into execution flow

## Related Documentation

- CLI Module: `/home/artur/Repositories/rustible/src/cli/mod.rs`
- Output Formatter: `/home/artur/Repositories/rustible/src/cli/output.rs`
- Command Context: `/home/artur/Repositories/rustible/src/cli/commands/mod.rs`
- Main Entry Point: `/home/artur/Repositories/rustible/src/main.rs`

## Conclusion

The `--plan` flag implementation is complete and fully functional. It provides a comprehensive dry-run capability that shows exactly what would be executed without making any actual changes or connections. The feature includes extensive test coverage and detailed output formatting for a great user experience.
