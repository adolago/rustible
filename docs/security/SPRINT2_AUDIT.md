# Sprint 2 Security Audit Report

**Audit Date:** 2025-12-25
**Auditor:** Security Review Agent
**Scope:** Sprint 2 Code - Include Path Handling, Delegation Security, Privilege Escalation, Vault Handling, Variable Templating

---

## Executive Summary

This security audit covers the Sprint 2 implementation focusing on task inclusion, delegation, privilege escalation, vault encryption, and variable templating. The audit identified several security considerations, with one critical path traversal vulnerability in the include system that requires immediate attention.

**Overall Risk Level:** Medium-High

| Severity | Count |
|----------|-------|
| Critical | 1 |
| High | 2 |
| Medium | 4 |
| Low | 3 |

---

## 1. Include Path Handling Security

### Files Reviewed:
- `/home/artur/Repositories/rustible/src/include.rs`
- `/home/artur/Repositories/rustible/src/executor/include_handler.rs`
- `/home/artur/Repositories/rustible/src/executor/task.rs` (execute_include_tasks, execute_include_vars)

### Finding 1.1: Path Traversal Vulnerability in Include System

**Severity:** Critical
**Status:** Open
**Location:** `src/include.rs:181-194` (`resolve_path` function)

**Description:**
The `TaskIncluder::resolve_path()` function does not validate that resolved paths remain within the project directory. An attacker with control over playbook content could use path traversal sequences (`../`) to include files from arbitrary locations on the filesystem.

```rust
fn resolve_path(&self, file: &str) -> Result<PathBuf> {
    let path = Path::new(file);

    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        self.base_path.join(path)
    };

    if !resolved.exists() {
        return Err(Error::FileNotFound(resolved));
    }

    Ok(resolved)  // No validation that path is within base_path!
}
```

**Attack Vector:**
```yaml
- name: Malicious include
  include_tasks: "../../../etc/cron.d/malicious.yml"
```

**Recommendation:**
1. Use `std::fs::canonicalize()` to resolve the final path
2. Verify the canonical path starts with the canonical base_path
3. Reject absolute paths that don't start with base_path

```rust
fn resolve_path(&self, file: &str) -> Result<PathBuf> {
    let path = Path::new(file);

    // Reject absolute paths outside base
    if path.is_absolute() {
        let canonical = std::fs::canonicalize(path)?;
        let base_canonical = std::fs::canonicalize(&self.base_path)?;
        if !canonical.starts_with(&base_canonical) {
            return Err(Error::SecurityViolation(
                "Include path escapes project directory".into()
            ));
        }
        return Ok(canonical);
    }

    // For relative paths, resolve and verify
    let resolved = self.base_path.join(path);
    let canonical = std::fs::canonicalize(&resolved)?;
    let base_canonical = std::fs::canonicalize(&self.base_path)?;

    if !canonical.starts_with(&base_canonical) {
        return Err(Error::SecurityViolation(
            "Include path escapes project directory".into()
        ));
    }

    Ok(canonical)
}
```

### Finding 1.2: Similar Path Traversal in include_vars

**Severity:** Critical
**Status:** Open
**Location:** `src/executor/task.rs:1203-1258` (`execute_include_vars`)

**Description:**
The `execute_include_vars` function resolves file paths without validating they remain within the project directory. Both file and directory modes are affected.

```rust
let resolved_path = if std::path::Path::new(file_path).is_absolute() {
    std::path::PathBuf::from(file_path)
} else {
    base_path.join(file_path)  // No traversal check
};
```

**Recommendation:**
Apply the same path validation as recommended for Finding 1.1.

---

## 2. Delegation Security

### Files Reviewed:
- `/home/artur/Repositories/rustible/src/executor/task.rs` (delegate_to handling, lines 388-412)
- `/home/artur/Repositories/rustible/src/playbook.rs`

### Finding 2.1: Delegation Host Validation

**Severity:** Medium
**Status:** Open
**Location:** `src/executor/task.rs:388-412`

**Description:**
When `delegate_to` is specified, the code creates a new execution context with the delegate host but does not validate that:
1. The delegate host is a valid inventory host
2. The delegate host has appropriate connection credentials
3. The caller is authorized to delegate to that host

```rust
let (execution_ctx, fact_storage_ctx) = if let Some(ref delegate_host) = self.delegate_to {
    debug!("Delegating task to host: {}", delegate_host);

    // Create execution context for the delegate host
    let mut delegate_ctx = ctx.clone();
    delegate_ctx.host = delegate_host.clone();  // No validation!
    ...
}
```

**Recommendation:**
1. Validate delegate_host exists in the inventory
2. Log delegation events for audit purposes
3. Consider adding a configuration option to restrict delegation to specific host groups

### Finding 2.2: delegate_facts Security

**Severity:** Low
**Status:** Open
**Location:** `src/executor/task.rs:397-406`

**Description:**
The `delegate_facts` option allows storing facts on either the delegate host or the original host. While functionally correct, this could lead to confusion in multi-tenant scenarios where facts from one context could affect another.

**Recommendation:**
1. Document the security implications of delegate_facts
2. Consider adding namespace isolation for facts in multi-tenant deployments

---

## 3. Privilege Escalation (Become) Handling

### Files Reviewed:
- `/home/artur/Repositories/rustible/src/connection/local.rs`
- `/home/artur/Repositories/rustible/src/connection/ssh.rs`
- `/home/artur/Repositories/rustible/src/connection/russh.rs`
- `/home/artur/Repositories/rustible/src/executor/task.rs`

### Finding 3.1: Escalation Password in Command Construction

**Severity:** Medium
**Status:** Partially Addressed
**Location:** `src/connection/russh.rs:582-597`, `src/connection/ssh.rs:395-414`, `src/connection/local.rs:46-77`

**Description:**
The escalation password is passed via stdin when using `sudo -S`, which is secure. However, the code could be more defensive:

```rust
// ssh.rs:400-404
"sudo" => {
    if options.escalate_password.is_some() {
        parts.push(format!("sudo -S -u {} -- ", escalate_user));
    } else {
        parts.push(format!("sudo -u {} -- ", escalate_user));
    }
}
```

**Positive Observations:**
- Password is passed via stdin, not command line (good)
- Uses `--` to separate sudo args from command (good)
- Password is not logged in debug output

**Concerns:**
1. No validation of `escalate_method` - unknown methods default to sudo
2. No timeout on password prompt could cause hangs
3. The `su` method uses `-c` which requires proper quoting

**Recommendation:**
1. Add explicit validation of escalate_method values
2. Add timeout handling for password prompts
3. Document security implications of each escalation method

### Finding 3.2: Become User Validation

**Severity:** Low
**Status:** Open
**Location:** All connection modules

**Description:**
The `become_user` value is used directly in command construction without validation. While shell escaping is applied in some contexts, a malicious become_user could potentially inject commands.

```rust
// local.rs:53
c.arg("-u").arg(escalate_user);  // Safe - passed as separate arg
```

```rust
// ssh.rs:402 - Potential risk with format string
parts.push(format!("sudo -S -u {} -- ", escalate_user));
```

**Recommendation:**
1. Validate become_user matches expected username patterns (alphanumeric, underscore, hyphen)
2. Use proper escaping consistently across all connection types

---

## 4. Vault Handling Security

### Files Reviewed:
- `/home/artur/Repositories/rustible/src/vault.rs`
- `/home/artur/Repositories/rustible/tests/security_tests.rs`

### Finding 4.1: Strong Encryption Implementation

**Severity:** N/A (Positive Finding)
**Status:** Verified Secure

**Description:**
The vault implementation uses industry-standard cryptographic primitives:
- AES-256-GCM for authenticated encryption
- Argon2id for key derivation (memory-hard, resistant to GPU attacks)
- Random salt and nonce per encryption

```rust
// Positive: Strong crypto choices
let cipher = Aes256Gcm::new(&key);
let argon2 = Argon2::default();
```

### Finding 4.2: Password Memory Handling

**Severity:** Medium
**Status:** Open
**Location:** `src/vault.rs:17-28`

**Description:**
The vault password is stored as a plain `String` in memory. While Rust's memory safety prevents use-after-free, the password could persist in memory after the Vault is dropped.

```rust
pub struct Vault {
    password: String,  // Not securely zeroed on drop
}
```

**Recommendation:**
1. Consider using `secrecy::Secret<String>` or `zeroize` crate to zero memory on drop
2. Implement `Drop` trait to explicitly clear sensitive data

```rust
use zeroize::Zeroize;

pub struct Vault {
    password: zeroize::Zeroizing<String>,
}
```

### Finding 4.3: Empty Password Allowed

**Severity:** Low
**Status:** Open
**Location:** `src/vault.rs:24-28`

**Description:**
The vault allows empty passwords, which provides no security. While this is the user's choice, a warning should be issued.

**Recommendation:**
Add a warning when encrypting with an empty password, or require minimum password length.

---

## 5. Variable Templating Security

### Files Reviewed:
- `/home/artur/Repositories/rustible/src/template.rs`
- `/home/artur/Repositories/rustible/src/modules/template.rs`
- `/home/artur/Repositories/rustible/src/executor/task.rs` (template_string, template_value)

### Finding 5.1: Template Engine Security (Positive)

**Severity:** N/A (Positive Finding)
**Status:** Verified Secure

**Description:**
The template implementation uses minijinja/Tera, which provides secure defaults:
- No arbitrary code execution
- No file system access from templates
- User input is treated as data, not code

```rust
// User input in variables is escaped, not interpreted as template code
let result = engine.render("User said: {{ user_input }}", &vars).unwrap();
// {{ dangerous_var }} in user_input is rendered literally
```

### Finding 5.2: Jinja2 Filter Security

**Severity:** Low
**Status:** Open
**Location:** `src/modules/template.rs:36-103`

**Description:**
Custom Tera filters are registered (default, upper, lower, trim, replace, join). These appear safe, but the `replace` filter could be used for subtle attacks if patterns aren't carefully considered.

**Recommendation:**
Document that templates should be treated as code and reviewed with the same scrutiny as application code.

---

## 6. Additional Security Observations

### Finding 6.1: Shell Command Escaping

**Severity:** High
**Status:** Partially Addressed
**Location:** `src/modules/command.rs:349-358`, `src/modules/template.rs:21-30`

**Description:**
Shell escaping is implemented but inconsistently applied. The `shell_escape` function handles basic cases but may miss edge cases:

```rust
fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/') {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}
```

**Concern:** The allowed unescaped characters include `/` which could be problematic in some contexts.

**Recommendation:**
1. Use a well-tested escaping library like `shell-escape` crate
2. Apply consistent escaping across all modules

### Finding 6.2: Command Module vs Shell Module Distinction

**Severity:** N/A (Positive Finding)
**Status:** Verified Secure

**Description:**
The separation between `command` (no shell) and `shell` (with shell) modules is correctly implemented:
- `CommandModule` uses `Command::new()` with separate args (safe)
- `ShellModule` explicitly passes through shell (documented risk)

---

## Recommendations Summary

### Critical Priority (Fix Immediately)
1. **Path Traversal in Include System** - Add canonicalization and boundary checks to `resolve_path()` and `execute_include_vars()`

### High Priority (Fix Before Release)
2. **Shell Escaping Consistency** - Standardize shell escaping across all modules
3. **Delegation Host Validation** - Validate delegate_to hosts exist in inventory

### Medium Priority (Fix Soon)
4. **Vault Password Memory** - Use zeroize crate for secure memory handling
5. **Escalation Method Validation** - Whitelist valid become methods
6. **Escalation Timeout** - Add timeout for password prompts
7. **Delegation Audit Logging** - Log all delegation events

### Low Priority (Track for Future)
8. **Empty Vault Password Warning** - Warn on empty passwords
9. **delegate_facts Documentation** - Document security implications
10. **Become User Validation** - Validate username format

---

## Test Coverage Assessment

The existing security tests in `tests/security_tests.rs` provide good coverage for:
- Vault encryption/decryption
- AES-GCM tampering detection
- Argon2 timing resistance
- Password not in error messages
- Template injection prevention
- Command module shell metacharacter handling
- File mode permissions
- Check mode safety
- Symlink safety

**Missing Test Coverage:**
- Path traversal in include_tasks
- Path traversal in include_vars
- Delegation to non-existent hosts
- Malformed become_user values
- Very long file paths (DoS potential)

---

## Appendix: Files Audited

| File | Lines | Security-Relevant |
|------|-------|-------------------|
| src/include.rs | 375 | Yes - Path handling |
| src/executor/include_handler.rs | 229 | Yes - Include execution |
| src/executor/task.rs | 1817 | Yes - Delegation, become, templating |
| src/vault.rs | 134 | Yes - Encryption |
| src/template.rs | 41 | Yes - Template engine |
| src/modules/template.rs | 753 | Yes - Template module |
| src/modules/command.rs | 501 | Yes - Command execution |
| src/modules/shell.rs | 501 | Yes - Shell execution |
| src/connection/local.rs | ~200 | Yes - Privilege escalation |
| src/connection/ssh.rs | ~1000 | Yes - Privilege escalation |
| src/connection/russh.rs | ~3200 | Yes - Privilege escalation |
| tests/security_tests.rs | 954 | Security test coverage |

---

**Report Generated:** 2025-12-25
**Next Review:** Recommended after critical fixes are applied
