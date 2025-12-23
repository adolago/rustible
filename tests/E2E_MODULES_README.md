# End-to-End Module Integration Tests

This directory contains comprehensive end-to-end tests for Rustible's core modules.

## Overview

The E2E module tests validate the complete workflow of:

1. **file module** - Directory and file management (create, touch, permissions)
2. **copy module** - File copying and content deployment
3. **template module** - Template rendering with Jinja2 variables
4. **command module** - Command execution and output handling
5. **service module** - Service status checking

## Test Playbook

The main test playbook is located at:
```
tests/fixtures/integration/playbooks/modules_e2e.yml
```

This playbook contains:
- **40+ tasks** covering all major module features
- **Idempotency checks** to ensure modules don't make unnecessary changes
- **Integration scenarios** combining multiple modules
- **Verification tasks** to validate all operations succeeded

## Running the Tests

### 1. Local Execution (Default)

Run tests against localhost using local connection:

```bash
cargo test --test modules_e2e_tests
```

This will execute all E2E tests including:
- Basic module functionality
- Idempotency tests
- Check mode validation
- Performance benchmarks
- Variable substitution

### 2. With Verbose Output

```bash
export RUSTIBLE_TEST_VERBOSE=1
cargo test --test modules_e2e_tests -- --nocapture
```

### 3. Individual Tests

Run specific test scenarios:

```bash
# Test local execution only
cargo test --test modules_e2e_tests test_e2e_modules_local

# Test idempotency
cargo test --test modules_e2e_tests test_e2e_modules_idempotency

# Test check mode
cargo test --test modules_e2e_tests test_e2e_modules_check_mode

# Test individual modules
cargo test --test modules_e2e_tests test_e2e_individual_modules

# Performance testing
cargo test --test modules_e2e_tests test_e2e_modules_performance
```

### 4. Against Docker Containers

If you have Docker available and want to test against containers:

```bash
export RUSTIBLE_TEST_DOCKER_ENABLED=1
cargo test --test modules_e2e_tests
```

### 5. Against Real SSH VMs

Test against actual VM infrastructure:

```bash
# Configure SSH access
export RUSTIBLE_TEST_SSH_ENABLED=1
export RUSTIBLE_TEST_SSH_USER=testuser
export RUSTIBLE_TEST_SSH_HOSTS="192.168.178.141,192.168.178.142,192.168.178.143"
export RUSTIBLE_TEST_SSH_KEY=$HOME/.ssh/id_ed25519

# Run tests
cargo test --test modules_e2e_tests test_e2e_modules_ssh
```

### 6. Using the Test Infrastructure

If you have the VM test infrastructure set up:

```bash
cd tests/infrastructure
./provision.sh deploy
./run-tests.sh e2e
```

Or use the inventory file:

```bash
export RUSTIBLE_TEST_INVENTORY=tests/infrastructure/test_inventory.yml
export RUSTIBLE_TEST_SSH_ENABLED=1
cargo test --test modules_e2e_tests test_e2e_modules_ssh
```

## Test Scenarios

### Basic Module Tests (`test_e2e_modules_local`)

Executes the full playbook against localhost, verifying:
- Directory creation with correct permissions
- File copying and content deployment
- Template rendering with variable substitution
- Command execution and output capture
- Service status checking
- Integration between modules

**Expected outcome**: All tasks pass, artifacts are created in temp directory

### Idempotency Tests (`test_e2e_modules_idempotency`)

Runs the same playbook twice to verify idempotent behavior:
- First run: Creates resources (should have changes)
- Second run: Resources exist (should have fewer/no changes)

**Expected outcome**: Second run has significantly fewer changes than first

### Check Mode Tests (`test_e2e_modules_check_mode`)

Runs playbook in dry-run mode:
- Tasks execute but don't make actual changes
- Shows what would change without changing it

**Expected outcome**: Tasks report what would change, but no files created

### Individual Module Tests (`test_e2e_individual_modules`)

Tests each module in isolation:
- file: Create directory
- copy: Deploy content
- template: Render template
- command: Execute command

**Expected outcome**: Each module works independently

### Performance Tests (`test_e2e_modules_performance`)

Measures execution speed:
- Records total execution time
- Calculates tasks per second

**Expected outcome**: Reasonable performance (varies by system)

### Variable Tests (`test_e2e_modules_with_variables`)

Tests variable substitution and override:
- Sets custom variables
- Verifies they're used in templates and files

**Expected outcome**: Custom variables appear in generated files

### SSH Tests (`test_e2e_modules_ssh`)

Runs playbook against remote VMs:
- Tests real SSH connections
- Validates remote execution
- Verifies SFTP file transfers

**Expected outcome**: All hosts complete successfully

## Test Artifacts

After running local tests, you can inspect the created files:

```bash
# The test creates files in a temporary directory
# Location is printed in test output, e.g., /tmp/.tmpXXXXXX/e2e_test/

# Typical structure:
/tmp/.tmpXXXXXX/e2e_test/
├── config/
│   └── app.conf               # Configuration file
├── data/
│   ├── test_file.txt         # Basic copy test
│   ├── multiline.txt         # Multiline content
│   └── marker.txt            # Command-created file
├── deployment/
│   ├── bin/
│   │   └── app.sh            # Deployment script
│   ├── conf/
│   ├── var/
│   ├── tmp/
│   └── metadata.json         # JSON metadata
├── logs/
│   └── application.log       # Touched file
└── templates/
    ├── simple.txt            # Simple template
    └── config.ini            # Complex template
```

## Troubleshooting

### Test Fails on Localhost

**Issue**: Tests fail with permission errors

**Solution**: Ensure the test can create files in `/tmp`:
```bash
mkdir -p /tmp/rustible_e2e_test
chmod 755 /tmp/rustible_e2e_test
```

### SSH Tests Skip

**Issue**: `test_e2e_modules_ssh` is skipped

**Solution**: Ensure environment variables are set:
```bash
echo $RUSTIBLE_TEST_SSH_ENABLED  # Should be "1"
echo $RUSTIBLE_TEST_SSH_HOSTS    # Should list hosts
```

### Service Module Tests Fail

**Issue**: Service status checks fail

**Solution**: The service tests are designed to be safe and read-only. They should gracefully handle systems without systemd. Check if the error is critical or just informational.

### Template Rendering Issues

**Issue**: Templates don't render correctly

**Solution**: Verify the MiniJinja template engine is working:
```bash
# Look for template-related errors in output
cargo test --test modules_e2e_tests -- --nocapture | grep -i template
```

### Idempotency Test Fails

**Issue**: Second run still shows changes

**Solution**: This may indicate a module isn't fully idempotent yet. Check which specific tasks are changing:
```bash
cargo test test_e2e_modules_idempotency -- --nocapture
```

## Expected Results

### Successful Run Output

```
Running E2E Module Tests (Local)
========================================

Executing playbook with 45 tasks...
✓ Host localhost completed successfully: 12 ok, 28 changed, 5 skipped
✓ All test artifacts verified successfully

✓ Local E2E module tests passed!
```

### Performance Benchmarks

Typical performance on modern hardware:
- **Local execution**: 40-50 tasks in 2-5 seconds (10-20 tasks/sec)
- **SSH execution**: 40-50 tasks in 10-30 seconds (2-5 tasks/sec per host)
- **Check mode**: 40-50 tasks in 1-3 seconds (15-30 tasks/sec)

## Integration with CI/CD

### GitHub Actions Example

```yaml
- name: Run E2E Module Tests
  run: |
    cargo test --test modules_e2e_tests --verbose
```

### GitLab CI Example

```yaml
test:e2e:modules:
  script:
    - cargo test --test modules_e2e_tests
```

### With SSH VM Infrastructure

```yaml
test:e2e:ssh:
  before_script:
    - cd tests/infrastructure && ./provision.sh deploy
  script:
    - export RUSTIBLE_TEST_SSH_ENABLED=1
    - export RUSTIBLE_TEST_INVENTORY=tests/infrastructure/test_inventory.yml
    - cargo test --test modules_e2e_tests test_e2e_modules_ssh
  after_script:
    - cd tests/infrastructure && ./provision.sh cleanup
```

## Module Coverage

| Module   | Create | Update | Delete | Idempotent | Check Mode | Variables |
|----------|--------|--------|--------|------------|------------|-----------|
| file     | ✓      | ✓      | ✗      | ✓          | ✓          | ✓         |
| copy     | ✓      | ✓      | ✗      | ✓          | ✓          | ✓         |
| template | ✓      | ✓      | ✗      | ✓          | ✓          | ✓         |
| command  | ✓      | N/A    | N/A    | N/A        | ✓          | ✓         |
| service  | ✗      | ✗      | ✗      | ✓          | ✓          | ✓         |

**Legend**:
- ✓ = Tested and working
- ✗ = Not tested in E2E suite
- N/A = Not applicable for this module

## Contributing

To add new test scenarios:

1. Edit `tests/fixtures/integration/playbooks/modules_e2e.yml`
2. Add your test tasks under appropriate sections
3. Update `tests/modules_e2e_tests.rs` if new verification is needed
4. Run tests to verify
5. Update this README with new scenarios

## Related Documentation

- [Integration Tests README](tests/README.md)
- [SSH Tests](real_ssh_tests.rs)
- [Module Tests](module_tests.rs)
- [Performance Tests](performance_tests.rs)
