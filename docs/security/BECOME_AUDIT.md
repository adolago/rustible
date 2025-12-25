# Security Audit: Privilege Escalation (Become) System

**Audit Date:** 2025-12-25
**Auditor:** Security Hardening Agent
**Scope:** All privilege escalation (become) functionality in Rustible
**Status:** COMPLETED - Issues Identified

---

## Executive Summary

This audit reviewed the privilege escalation ("become") system in Rustible, which enables tasks to execute with elevated privileges using sudo, su, doas, and other methods. The implementation is generally sound but has **several security concerns** that should be addressed.

### Risk Summary

| Severity | Count | Status |
|----------|-------|--------|
| Critical | 1     | Needs Fix |
| High     | 2     | Needs Fix |
| Medium   | 3     | Recommend Fix |
| Low      | 2     | Documented |

---

## 1. Architecture Overview

### 1.1 Components Involved

The privilege escalation system spans multiple modules:

- **Configuration Layer:**
  - `/src/config.rs` - `PrivilegeEscalation` struct (lines 199-216)
  - `/src/inventory/host.rs` - Host-level become settings (lines 128-164)

- **Execution Layer:**
  - `/src/connection/local.rs` - Local `build_command()` (lines 45-86)
  - `/src/connection/ssh.rs` - SSH `build_command()` (lines 386-421)
  - `/src/connection/russh.rs` - Russh `build_command()` (lines 569-600)
  - `/src/connection/mod.rs` - `ExecuteOptions` struct (lines 182-198)

- **Module Layer:**
  - `/src/modules/command.rs` - Command module escalation (lines 143-148)
  - `/src/modules/shell.rs` - Shell module escalation (lines 92-97)
  - `/src/modules/mod.rs` - `ModuleContext` with become fields (lines 489-525)

### 1.2 Data Flow

```
Playbook/Inventory -> Config -> ModuleContext -> ExecuteOptions -> Connection.build_command()
                                                                           |
                                                                           v
                                                            sudo/su/doas command string
```

---

## 2. Critical Findings

### 2.1 [CRITICAL] Command Injection via `escalate_user`

**Location:**
- `/src/connection/ssh.rs` lines 585-594
- `/src/connection/russh.rs` lines 585-598
- `/src/connection/local.rs` lines 52-78

**Issue:**
The `escalate_user` value is interpolated directly into command strings without validation or escaping. A malicious username could inject arbitrary commands.

**Vulnerable Code (russh.rs:585-598):**
```rust
match escalate_method {
    "sudo" => {
        if options.escalate_password.is_some() {
            parts.push(format!("sudo -S -u {} -- ", escalate_user));
        } else {
            parts.push(format!("sudo -u {} -- ", escalate_user));
        }
    }
    "su" => {
        parts.push(format!("su - {} -c ", escalate_user));
    }
    "doas" => {
        parts.push(format!("doas -u {} ", escalate_user));
    }
    // ...
}
```

**Attack Vector:**
```yaml
# Malicious playbook
become_user: "root; rm -rf /"
# Results in: sudo -u root; rm -rf / -- echo hello
```

**Impact:** Remote code execution with arbitrary privileges.

**Recommendation:**
1. Validate `escalate_user` against a strict regex pattern: `^[a-z_][a-z0-9_-]*[$]?$`
2. Reject usernames containing shell metacharacters
3. Use allowlist validation for known escalation methods

**Proposed Fix:**
```rust
fn validate_username(username: &str) -> Result<(), SecurityError> {
    // POSIX username regex: starts with letter/underscore,
    // contains only alphanumeric, underscore, hyphen
    let valid_pattern = regex::Regex::new(r"^[a-z_][a-z0-9_-]{0,31}$").unwrap();
    if !valid_pattern.is_match(username) {
        return Err(SecurityError::InvalidUsername(username.to_string()));
    }
    Ok(())
}
```

---

### 2.2 [HIGH] Unvalidated `escalate_method`

**Location:**
- `/src/connection/local.rs` lines 50-81
- `/src/connection/ssh.rs` lines 386-421
- `/src/connection/russh.rs` lines 578-599

**Issue:**
Unknown escalation methods silently fall back to `sudo` without warning. This could mask configuration errors or allow unexpected behavior.

**Vulnerable Code (local.rs:74-80):**
```rust
_ => {
    // Default to sudo
    let mut c = Command::new("sudo");
    c.arg("-u").arg(escalate_user);
    c.arg("--").arg("sh").arg("-c").arg(command);
    c
}
```

**Impact:**
- Configuration errors go unnoticed
- Attacker could introduce non-standard method names that default to sudo
- Inconsistent behavior across different connection types

**Recommendation:**
1. Maintain an explicit allowlist of supported methods
2. Return an error for unknown methods
3. Log warnings for deprecated or unusual methods

**Proposed Fix:**
```rust
const SUPPORTED_ESCALATION_METHODS: &[&str] = &["sudo", "su", "doas", "pbrun", "pfexec", "runas", "dzdo", "ksu"];

fn validate_escalation_method(method: &str) -> Result<(), SecurityError> {
    if !SUPPORTED_ESCALATION_METHODS.contains(&method) {
        return Err(SecurityError::UnsupportedEscalationMethod(method.to_string()));
    }
    Ok(())
}
```

---

### 2.3 [HIGH] Path Injection in `chown` Command

**Location:** `/src/connection/local.rs` lines 391-412

**Issue:**
The `set_ownership()` function constructs a `chown` command using unescaped path interpolation.

**Vulnerable Code:**
```rust
let command = format!("chown {} {}", ownership, path.display());
```

**Attack Vector:**
```rust
// Path containing shell metacharacters
let path = Path::new("/tmp/file; rm -rf /");
// Results in: chown root:root /tmp/file; rm -rf /
```

**Impact:** Command injection via crafted file paths.

**Recommendation:**
1. Use shell escaping for the path argument
2. Consider using `std::process::Command` with separate arguments instead of shell command string

**Proposed Fix:**
```rust
fn set_ownership(&self, path: &Path, owner: Option<&str>, group: Option<&str>) -> ConnectionResult<()> {
    // ... ownership string building ...

    // Escape the path for shell safety
    let escaped_path = shell_escape::escape(Cow::Borrowed(path.to_string_lossy().as_ref()));
    let command = format!("chown {} {}", ownership, escaped_path);
    // ...
}
```

---

## 3. Medium Severity Findings

### 3.1 [MEDIUM] Password Exposure in Debug Output

**Location:** `/src/connection/mod.rs` - `ExecuteOptions` struct

**Issue:**
The `ExecuteOptions` struct derives `Debug`, which means `escalate_password` could be printed in logs or debug output.

**Current Code:**
```rust
#[derive(Debug, Clone, Default)]
pub struct ExecuteOptions {
    // ...
    pub escalate_password: Option<String>,
}
```

**Recommendation:**
Implement custom `Debug` that redacts the password:

```rust
impl std::fmt::Debug for ExecuteOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecuteOptions")
            .field("escalate", &self.escalate)
            .field("escalate_user", &self.escalate_user)
            .field("escalate_method", &self.escalate_method)
            .field("escalate_password", &self.escalate_password.as_ref().map(|_| "[REDACTED]"))
            // ... other fields
            .finish()
    }
}
```

---

### 3.2 [MEDIUM] Password Not Zeroed from Memory

**Location:** Throughout connection modules

**Issue:**
Passwords stored in `String` types are not securely zeroed after use. They may persist in memory until garbage collected.

**Affected Locations:**
- `ExecuteOptions.escalate_password`
- `BecomeConfig.password`
- `HostConnection.become_password`

**Recommendation:**
1. Use the `secrecy` crate for password storage
2. Implement `Zeroize` trait for structs containing passwords
3. Use `SecretString` instead of `String` for passwords

**Proposed Dependency:**
```toml
[dependencies]
secrecy = { version = "0.8", features = ["serde"] }
zeroize = { version = "1.6", features = ["derive"] }
```

---

### 3.3 [MEDIUM] Missing Working Directory Validation

**Location:**
- `/src/connection/russh.rs` line 574
- `/src/connection/ssh.rs` - similar location

**Issue:**
The `cwd` option is interpolated into command string without path validation.

**Vulnerable Code:**
```rust
if let Some(cwd) = &options.cwd {
    parts.push(format!("cd {} && ", cwd));
}
```

**Attack Vector:**
```rust
options.cwd = Some("/tmp; malicious_command; cd /".to_string());
```

**Recommendation:**
1. Validate that `cwd` contains only valid path characters
2. Use shell escaping for the path

---

## 4. Low Severity Findings

### 4.1 [LOW] Missing Rate Limiting for Password Attempts

**Location:** Connection layer password handling

**Issue:**
There is no rate limiting or lockout mechanism for failed privilege escalation attempts. An attacker with partial access could potentially brute-force passwords.

**Current State:**
The tests in `/tests/become_tests.rs` show comprehensive password handling tests, but no rate limiting is implemented.

**Recommendation:**
1. Consider implementing exponential backoff after failed attempts
2. Log failed escalation attempts for audit purposes
3. Consider integration with system-level fail2ban or similar

---

### 4.2 [LOW] Inconsistent `su` Command Syntax

**Location:** Multiple connection modules

**Issue:**
The `su` command is constructed differently across modules:
- `su - {user} -c {command}` (russh.rs, ssh.rs)
- `su - {user} -c '{command}'` (some paths)

**Impact:**
Commands with special characters may behave differently across connection types.

**Recommendation:**
Standardize `su` command construction with proper quoting:
```rust
parts.push(format!("su - {} -c '{}'", escalate_user, command.replace("'", "'\"'\"'")));
```

---

## 5. Positive Security Findings

The following security practices were noted as positive:

### 5.1 Password via STDIN
The implementation correctly uses `-S` flag for sudo to read passwords from stdin rather than command line arguments:
```rust
if options.escalate_password.is_some() {
    c.arg("-S"); // Read password from stdin
}
```
This prevents password exposure in process listings.

### 5.2 Use of `--` Separator for sudo
The `--` separator is correctly used to prevent command argument injection:
```rust
c.arg("--").arg("sh").arg("-c").arg(command);
```

### 5.3 Comprehensive Test Coverage
The `/tests/become_tests.rs` file (1693 lines) provides extensive testing including:
- Security tests (lines 1090-1246)
- Edge cases with special characters (lines 1488-1626)
- Password handling tests (lines 267-365)

### 5.4 No Command-Line Password Exposure
Passwords are never included in command-line arguments, only passed via stdin.

---

## 6. Test Coverage Analysis

### Existing Security Tests (`/tests/become_tests.rs`)

| Test Category | Coverage | Assessment |
|---------------|----------|------------|
| Basic become functionality | Excellent | Lines 31-145 |
| Multiple escalation methods | Excellent | Lines 151-261 |
| Password handling | Good | Lines 267-365 |
| Edge cases (special chars) | Good | Lines 1488-1626 |
| Security (password masking) | Partial | Lines 1090-1246 |
| Command injection | **Missing** | Not tested |

### Missing Security Tests

The following test cases should be added:

```rust
#[test]
fn test_username_command_injection_rejected() {
    let malicious_users = vec![
        "root; rm -rf /",
        "root$(whoami)",
        "root`id`",
        "root|cat /etc/passwd",
        "root\nmalicious",
        "root\x00null",
    ];

    for user in malicious_users {
        let result = validate_username(user);
        assert!(result.is_err(), "Should reject malicious username: {}", user);
    }
}

#[test]
fn test_unknown_method_rejected() {
    let unknown_methods = vec!["unknown", "SUDO", "sudo2", "my_escalator"];

    for method in unknown_methods {
        let result = validate_escalation_method(method);
        assert!(result.is_err(), "Should reject unknown method: {}", method);
    }
}

#[test]
fn test_path_injection_in_chown() {
    let malicious_paths = vec![
        "/tmp/file; rm -rf /",
        "/tmp/$(whoami)",
        "/tmp/`id`",
    ];

    for path in malicious_paths {
        // Verify paths are properly escaped
        let escaped = shell_escape_path(path);
        assert!(!escaped.contains(';'));
        assert!(!escaped.contains('$'));
        assert!(!escaped.contains('`'));
    }
}
```

---

## 7. Recommendations Summary

### Immediate Actions (Critical/High)

1. **Add username validation** in all `build_command()` functions
   - Files: `local.rs`, `ssh.rs`, `russh.rs`
   - Pattern: POSIX username regex

2. **Add escalation method validation**
   - Create allowlist of supported methods
   - Return error for unknown methods

3. **Fix path injection in `set_ownership()`**
   - Add shell escaping for paths
   - Consider using Command args instead of string concatenation

### Short-term Actions (Medium)

4. **Implement custom Debug for password-containing structs**
   - Redact passwords in debug output

5. **Add secure password handling**
   - Use `secrecy` crate
   - Implement `Zeroize` trait

6. **Add CWD path validation**
   - Validate and escape working directory paths

### Long-term Actions (Low/Improvements)

7. **Add rate limiting for failed escalation attempts**

8. **Standardize `su` command syntax across modules**

9. **Add comprehensive security test suite**
   - Command injection tests
   - Path injection tests
   - Method validation tests

---

## 8. Code Locations Reference

| Issue | File | Lines | Function |
|-------|------|-------|----------|
| Username injection | `src/connection/russh.rs` | 578-598 | `build_command()` |
| Username injection | `src/connection/ssh.rs` | 386-421 | `build_command()` |
| Username injection | `src/connection/local.rs` | 45-86 | `build_command()` |
| Method fallback | `src/connection/local.rs` | 74-80 | `build_command()` |
| Path injection | `src/connection/local.rs` | 399 | `set_ownership()` |
| Debug password | `src/connection/mod.rs` | 182-198 | `ExecuteOptions` |
| CWD injection | `src/connection/russh.rs` | 573-574 | `build_command()` |

---

## 9. Appendix: Existing Test File Summary

The `/tests/become_tests.rs` file contains 14 test modules:

1. `become_basic` - Basic escalation tests
2. `become_methods` - Method support tests (sudo, su, doas, etc.)
3. `become_password` - Password handling tests
4. `become_flags` - Custom flag tests
5. `become_scope` - Play/block/task scope tests
6. `become_with_connection` - Connection type tests
7. `sudo_configuration` - Sudo-specific tests
8. `privilege_escalation_chain` - Nested escalation tests
9. `become_with_delegate` - Delegation tests
10. `become_security` - Security tests (partial coverage)
11. `mock_commands` - Mock command utilities
12. `integration` - Integration tests
13. `edge_cases` - Edge case handling
14. `performance` - Performance tests

---

## 10. Conclusion

The Rustible privilege escalation system has a solid foundation but requires security hardening before production use. The critical command injection vulnerability in username handling must be addressed immediately. The medium-severity findings should be resolved in the next development cycle.

**Overall Security Rating:** **NEEDS IMPROVEMENT** (3/5)

- Architecture: Good
- Input Validation: Needs Work
- Password Handling: Acceptable
- Test Coverage: Good (missing injection tests)
- Code Quality: Good

---

*End of Security Audit Report*
