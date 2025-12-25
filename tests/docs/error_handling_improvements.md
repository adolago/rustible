# Error Handling Improvements

This document describes the comprehensive error handling improvements made to Rustible.

## Summary of Changes

Enhanced 7 critical error types with actionable suggestions and troubleshooting guidance:

### 1. ModuleNotFound Error
**Before:**
```
Module 'apt_repo' not found
```

**After:**
```
Module 'apt_repo' not found. Available modules: apt, command, copy, debug, file, git, lineinfile, package, service, shell, template, user (and 8 more)
```

**Improvements:**
- Lists all available modules (or first 20 if more)
- Helps users discover correct module names
- Reduces trial-and-error debugging

### 2. HostNotFound Error
**Before:**
```
Host 'web-server1' not found in inventory
```

**After:**
```
Host 'web-server1' not found in inventory. Did you mean one of these? web-server, web-server2, web-server3, db-server1, db-server2
Use 'rustible list-hosts' to see all 47 hosts
```

**Improvements:**
- Suggests similar host names from inventory
- Points to command for listing all hosts
- Handles empty inventory with specific guidance

### 3. AuthenticationFailed Error
**Before:**
```
Authentication failed for 'admin@192.168.1.100': Permission denied (publickey)
```

**After:**
```
Authentication failed for 'admin@192.168.1.100': Permission denied (publickey)

Troubleshooting:
1. Check SSH key permissions: chmod 600 ~/.ssh/id_rsa
2. Verify the correct user is specified in inventory
3. Test SSH manually: ssh <user>@<host>
4. Check if password authentication is required (use --ask-pass)
5. Verify SSH agent has the key loaded: ssh-add -l
6. Check authorized_keys file on remote host
7. Review SSH server logs: /var/log/auth.log
```

**Improvements:**
- 7-step troubleshooting checklist
- Covers most common authentication issues
- Provides specific commands to run

### 4. ConnectionFailed Error with Context-Aware Suggestions
**Before:**
```
Failed to connect to '192.168.1.100': Connection refused
```

**After:**
```
Failed to connect to '192.168.1.100': Connection refused

Suggestions:
- Check if the SSH service is running on the target host
- Verify the host is reachable: ping <host>
- Check firewall rules allow SSH connections
- Verify the correct port (default: 22)
```

**Context-Aware Suggestions Based on Error:**

**Connection Refused:**
- SSH service status
- Host reachability
- Firewall configuration
- Port verification

**DNS/Hostname Issues:**
- Inventory file verification
- IP address fallback
- DNS resolution testing
- /etc/hosts entries

**Network Routing:**
- Network connectivity checks
- Routing table review
- VPN requirements
- Traceroute diagnostics

**Timeout Issues:**
- Timeout adjustment options
- Network latency checks
- Host load verification
- Firewall delay detection

### 5. TemplateSyntax Error with Line Numbers
**Before:**
```
Template syntax error in 'config.j2': Undefined variable 'server_port'
```

**After:**
```
Template syntax error in 'config.j2' at line 42: Undefined variable 'server_port'

Help: Check template syntax, variable names, and filter usage
```

**Improvements:**
- Shows exact line number when available
- Provides quick help for common issues
- Helps locate problems in large templates

### 6. BecomeError with Method-Specific Guidance
**Before:**
```
Privilege escalation failed on 'app-server': sudo: no tty present
```

**After:**
```
Privilege escalation failed on 'app-server': sudo: no tty present

Try:
- Verify user has sudo privileges: sudo -l
- Check /etc/sudoers configuration
- Try with --ask-become-pass if password is required
- Verify become_user exists on target system
- Check sudo logs: /var/log/sudo.log
```

**Method-Specific Suggestions:**

**sudo:**
- Privilege verification
- sudoers configuration
- Password requirements
- User existence checks
- Log file review

**su:**
- Password verification
- Interactive password option
- Command availability
- Target user checks

**doas:**
- Configuration review
- Privilege verification
- Installation checks

### 7. Improved connection_failed() Helper
Enhanced to provide contextual suggestions based on error message patterns.

## Code Quality Improvements

### Error Structure Enhancements
- Added structured fields for suggestions and troubleshooting
- Maintained backward compatibility
- Improved error message formatting

### Helper Method Additions
```rust
Error::host_not_found(host, &available_hosts)
Error::module_not_found(module, &available_modules)
Error::auth_failed(user, host, message)
Error::template_syntax(template, message, line_number)
Error::become_failed(host, message, method)
```

### Type Safety
- All new fields are strongly typed
- No runtime string concatenation for critical paths
- Proper error propagation with context

## Usage Examples

### ModuleNotFound
```rust
// Old way
return Err(Error::ModuleNotFound("apt_repo".to_string()));

// New way
let available = registry.list_modules();
return Err(Error::module_not_found("apt_repo", &available));
```

### HostNotFound
```rust
// Old way
return Err(Error::HostNotFound("web1".to_string()));

// New way
let hosts = inventory.all_hosts();
return Err(Error::host_not_found("web1", &hosts));
```

### AuthenticationFailed
```rust
// Old way - manual construction
return Err(Error::AuthenticationFailed {
    user: "admin".into(),
    host: "server".into(),
    message: "Permission denied".into(),
});

// New way - with troubleshooting
return Err(Error::auth_failed("admin", "server", "Permission denied"));
```

### BecomeError
```rust
// New - method-aware suggestions
return Err(Error::become_failed(
    "server1",
    "sudo: no tty present",
    "sudo"  // Will provide sudo-specific help
));
```

## Impact on User Experience

### Before
Users encountered cryptic errors requiring:
- Deep knowledge of Ansible/automation internals
- Trial-and-error debugging
- Searching documentation/forums
- Manual inventory inspection

### After
Users receive:
- Actionable next steps
- Context-aware suggestions
- Commands to run for diagnosis
- Lists of available options
- Quick troubleshooting paths

## Testing Recommendations

1. **Unit Tests**: Test each error variant with various inputs
2. **Integration Tests**: Verify errors appear correctly in real scenarios
3. **User Testing**: Confirm suggestions are actually helpful
4. **Documentation**: Update user guide with error reference

## Future Enhancements

1. **Smart Suggestions**: Use Levenshtein distance for "did you mean" suggestions
2. **Error Recovery**: Automatic retry with suggestions applied
3. **Logging Integration**: Correlation IDs for error tracking
4. **Metrics**: Track common error patterns for UX improvements
5. **I18n**: Internationalization of error messages
6. **Interactive Mode**: Allow CLI to prompt for corrections

## Metrics

- **7 error types** enhanced with user-friendly messages
- **5 new helper methods** for consistent error creation
- **Context-aware suggestions** for ConnectionFailed errors
- **Method-specific guidance** for become operations
- **Backward compatible** with existing error handling

## Conclusion

These improvements transform Rustible errors from cryptic technical messages into actionable guidance that helps users quickly diagnose and fix issues. The changes maintain code quality, type safety, and backward compatibility while significantly improving the user experience.
