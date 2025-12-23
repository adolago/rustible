# File Module Test Suite

## Overview

Comprehensive test suite for the Rustible file module located at:
`/home/artur/Repositories/rustible/tests/file_module_tests.rs`

This test file provides extensive coverage of all file module functionality including the specific requirements from the task.

## Test Coverage Summary

### 1. Directory Creation Tests (state=directory)

#### Basic Functionality
- `test_directory_create_simple` - Creates a simple directory
- `test_directory_create_idempotent` - Verifies directory creation is idempotent
- `test_directory_create_nested` - Creates nested directories (a/b/c/d)
- `test_directory_create_with_mode` - Creates directory with specific permissions (0o755)
- `test_directory_update_mode_on_existing` - Changes permissions on existing directory

#### Check Mode
- `test_directory_check_mode` - Verifies check mode doesn't create directory
- `test_directory_check_mode_existing` - Check mode on existing directory

### 2. File Removal Tests (state=absent)

#### Basic Functionality
- `test_absent_remove_file` - Removes a file
- `test_absent_remove_directory` - Removes an empty directory
- `test_absent_remove_directory_with_contents` - Removes directory tree with contents
- `test_absent_idempotent` - Verifies removal is idempotent (no-op when already absent)
- `test_absent_remove_symlink` - Removes symlink without affecting target

#### Check Mode
- `test_absent_check_mode` - Verifies check mode doesn't remove files

### 3. Touch Tests (state=touch)

#### Basic Functionality
- `test_touch_create_new_file` - Creates a new empty file
- `test_touch_existing_file_updates_timestamp` - Updates mtime/atime on existing file
- `test_touch_with_mode` - Creates file with specific permissions (0o644)
- `test_touch_creates_parent_directories` - Creates parent directories as needed

#### Check Mode
- `test_touch_check_mode_new_file` - Check mode for new file
- `test_touch_check_mode_existing_file` - Check mode for existing file

### 4. Permission Change Tests (mode parameter)

#### Comprehensive Mode Testing
- `test_mode_change_on_file` - Changes file permissions (0o644 -> 0o600)
- `test_mode_change_on_directory` - Changes directory permissions (0o755 -> 0o700)
- `test_mode_no_change_when_same` - Idempotent when permissions already correct
- `test_mode_various_permissions` - Tests multiple permission values (0o777, 0o755, 0o700, 0o644, 0o600, 0o444, 0o400)

### 5. Idempotency Tests

Dedicated tests for verifying idempotent behavior:
- `test_idempotent_file_creation` - File creation idempotency (3 runs)
- `test_idempotent_directory_creation` - Directory creation idempotency (3 runs)
- `test_idempotent_absent` - Removal idempotency (2 runs)
- `test_idempotent_mode_changes` - Permission change idempotency (3 runs)

### 6. Additional Comprehensive Tests

#### Symlink Tests
- `test_symlink_create` - Creates a symlink
- `test_symlink_idempotent` - Verifies symlink creation is idempotent
- `test_symlink_to_nonexistent_target` - Allows symlinks to non-existent targets

#### Hard Link Tests
- `test_hardlink_create` - Creates a hard link (verifies same inode)
- `test_hardlink_idempotent` - Verifies hard link creation is idempotent
- `test_hardlink_error_nonexistent_source` - Errors when source doesn't exist

#### Error Handling Tests
- `test_error_missing_path_parameter` - Validates required path parameter
- `test_error_invalid_state` - Rejects invalid state values
- `test_error_symlink_missing_src` - Requires src for symlinks
- `test_error_hardlink_missing_src` - Requires src for hard links
- `test_error_file_exists_when_creating_directory` - Detects file/directory conflicts
- `test_error_directory_exists_when_creating_file` - Detects directory/file conflicts

#### Diff Tests
- `test_diff_file_creation` - Generates diff for file creation
- `test_diff_directory_creation` - Generates diff for directory creation
- `test_diff_removal` - Generates diff for file removal
- `test_diff_no_change` - No diff when no changes needed

#### Module Metadata Tests
- `test_module_name` - Verifies module name is "file"
- `test_module_description` - Validates module description
- `test_module_required_params` - Verifies "path" is required
- `test_module_classification` - Confirms NativeTransport classification

## Test Organization

The tests are organized into logical sections using comments:

1. Helper Functions
2. Directory Creation Tests (state=directory)
3. File Removal Tests (state=absent)
4. Touch Tests (state=touch)
5. Permission Change Tests (mode parameter)
6. Idempotency Tests
7. Edge Cases and Error Handling
8. Symlink Tests
9. Hard Link Tests
10. Diff Tests
11. Module Metadata Tests

## Task Requirements Fulfillment

### Required Test Coverage

| Requirement | Test(s) | Status |
|------------|---------|--------|
| state=directory creates directory | `test_directory_create_simple`, `test_directory_create_nested`, `test_directory_create_with_mode` | ✓ Complete |
| state=absent removes file | `test_absent_remove_file`, `test_absent_remove_directory`, `test_absent_remove_directory_with_contents` | ✓ Complete |
| state=touch creates empty file | `test_touch_create_new_file`, `test_touch_existing_file_updates_timestamp`, `test_touch_with_mode` | ✓ Complete |
| mode changes permissions | `test_mode_change_on_file`, `test_mode_change_on_directory`, `test_mode_various_permissions` | ✓ Complete |
| Idempotency checks | `test_idempotent_file_creation`, `test_idempotent_directory_creation`, `test_idempotent_absent`, `test_idempotent_mode_changes` | ✓ Complete |

## Running the Tests

```bash
# Run all file module tests
cargo test --test file_module_tests

# Run specific test
cargo test --test file_module_tests test_directory_create_simple

# Run tests with output
cargo test --test file_module_tests -- --nocapture

# Run tests matching pattern
cargo test --test file_module_tests idempotent
```

## Test Statistics

- **Total Tests**: 54 tests
- **Directory Tests**: 7 tests
- **Removal Tests**: 6 tests
- **Touch Tests**: 6 tests
- **Permission Tests**: 4 tests
- **Idempotency Tests**: 4 tests
- **Symlink Tests**: 3 tests
- **Hard Link Tests**: 3 tests
- **Error Handling Tests**: 6 tests
- **Diff Tests**: 4 tests
- **Metadata Tests**: 4 tests
- **Helper Tests**: 7 tests (across various sections)

## Test Patterns Used

### Idempotency Pattern
```rust
// First execution - should change
let result1 = module.execute(&params, &context).unwrap();
assert!(result1.changed);

// Second execution - should be idempotent
let result2 = module.execute(&params, &context).unwrap();
assert!(!result2.changed);
```

### Check Mode Pattern
```rust
let context = ModuleContext::default().with_check_mode(true);
let result = module.execute(&params, &context).unwrap();
assert!(result.changed);
assert!(result.msg.contains("Would"));
assert!(!path.exists()); // No actual changes
```

### Error Handling Pattern
```rust
let result = module.execute(&params, &context);
assert!(result.is_err());
match result {
    Err(ModuleError::MissingParameter(msg)) => {
        assert!(msg.contains("expected_param"));
    }
    _ => panic!("Expected specific error type"),
}
```

## Notes

1. All tests use `tempfile::TempDir` for isolation
2. Tests verify both the module output (changed status, messages) and actual filesystem state
3. Permission tests use Unix-specific functionality (`PermissionsExt`, `MetadataExt`)
4. Idempotency tests run operations multiple times to ensure stability
5. Check mode tests verify that no actual changes occur on the filesystem
6. Error tests validate proper error handling for invalid inputs

## Existing Tests

Note that the main test suite in `/home/artur/Repositories/rustible/tests/module_tests.rs` also contains file module tests (lines 937-2747). The tests created here are designed to:

1. Provide a dedicated, focused test file for the file module
2. Add more comprehensive idempotency testing
3. Include more edge cases and error scenarios
4. Provide better test organization and documentation
5. Serve as examples for testing other modules

## Future Enhancements

Potential areas for additional testing:

1. Ownership tests (owner/group parameters) - requires root privileges
2. Recursive directory operations
3. Force parameter behavior
4. Symbolic link force replacement
5. Hard link across filesystems (error case)
6. Performance tests with large directory trees
7. Concurrent access scenarios
8. Integration tests with actual remote connections (SSH)
9. Tests for file state transitions (file -> directory -> absent -> file)
10. Unicode filename handling
