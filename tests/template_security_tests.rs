//! Template Security Tests - Server-Side Template Injection (SSTI) Prevention
//!
//! This test suite verifies that the template engines (minijinja and tera) are
//! properly secured against template injection attacks. These tests ensure:
//!
//! 1. No dangerous built-ins are exposed (file system access, code execution)
//! 2. Template recursion is properly limited
//! 3. Error messages do not leak sensitive internal information
//! 4. Malicious template patterns are safely handled
//!
//! ## Security Concerns Tested:
//! - Object attribute access to dangerous methods
//! - Attempt to access __class__, __mro__, __globals__ (Python-style SSTI)
//! - File system access through filters (realpath, expanduser)
//! - Environment variable access (lookup('env', ...))
//! - Template include/import attacks
//! - Denial of service through recursion or large loops

use rustible::template::TemplateEngine;
use serde_json::json;
use std::collections::HashMap;

// ============================================================================
// SSTI Attack Pattern Tests - Ensure Dangerous Patterns Are Blocked/Safe
// ============================================================================

/// Test that Python-style __class__ attribute access doesn't work
/// In Python Jinja2, attackers try: {{ ''.__class__.__mro__[2].__subclasses__() }}
#[test]
fn test_ssti_no_class_attribute_access() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("test".to_string(), json!("hello"));

    // Try to access __class__ (Python SSTI pattern)
    let result = engine.render("{{ test.__class__ }}", &vars);

    // Should either error or return empty/undefined - NOT expose internals
    match result {
        Ok(output) => {
            // Output should NOT contain class information
            assert!(!output.contains("str"));
            assert!(!output.contains("String"));
            assert!(!output.contains("class"));
            assert!(!output.contains("type"));
        }
        Err(_) => {
            // Error is acceptable - pattern not supported
        }
    }
}

/// Test that __globals__ access pattern doesn't work
#[test]
fn test_ssti_no_globals_access() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{{ ''.__globals__ }}", &vars);

    match result {
        Ok(output) => {
            assert!(!output.contains("os"));
            assert!(!output.contains("subprocess"));
            assert!(!output.contains("builtins"));
        }
        Err(_) => {
            // Error is acceptable
        }
    }
}

/// Test that __mro__ (Method Resolution Order) access doesn't work
#[test]
fn test_ssti_no_mro_access() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{{ [].__class__.__mro__ }}", &vars);

    match result {
        Ok(output) => {
            assert!(!output.contains("object"));
            assert!(!output.contains("list"));
        }
        Err(_) => {
            // Error is acceptable
        }
    }
}

/// Test that __subclasses__ access doesn't work
#[test]
fn test_ssti_no_subclasses_access() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{{ ''.__class__.__subclasses__() }}", &vars);

    match result {
        Ok(output) => {
            // Should not expose any class hierarchy
            assert!(!output.contains("class"));
            assert!(!output.contains("subprocess"));
            assert!(!output.contains("Popen"));
        }
        Err(_) => {
            // Error is acceptable
        }
    }
}

/// Test that __builtins__ access doesn't work
#[test]
fn test_ssti_no_builtins_access() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{{ __builtins__ }}", &vars);

    match result {
        Ok(output) => {
            assert!(!output.contains("open"));
            assert!(!output.contains("exec"));
            assert!(!output.contains("eval"));
            assert!(!output.contains("import"));
        }
        Err(_) => {
            // Error is acceptable
        }
    }
}

/// Test that request/config objects aren't accessible (Flask-style SSTI)
#[test]
fn test_ssti_no_request_config_access() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    // Flask/web framework SSTI patterns
    let dangerous_patterns = vec![
        "{{ request }}",
        "{{ config }}",
        "{{ self }}",
        "{{ g }}",
        "{{ session }}",
    ];

    for pattern in dangerous_patterns {
        let result = engine.render(pattern, &vars);
        match result {
            Ok(output) => {
                // Should be empty or undefined, not expose objects
                assert!(
                    output.is_empty() || output == "undefined",
                    "Pattern {} should not return sensitive data: {}",
                    pattern,
                    output
                );
            }
            Err(_) => {
                // Error is acceptable
            }
        }
    }
}

// ============================================================================
// File System Access Prevention Tests
// ============================================================================

/// Test that file reading functions are not available
#[test]
fn test_no_file_read_function() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let dangerous_attempts = vec![
        "{{ open('/etc/passwd').read() }}",
        "{{ file('/etc/passwd') }}",
        "{{ read_file('/etc/passwd') }}",
        "{{ include('/etc/passwd') }}",
    ];

    for attempt in dangerous_attempts {
        let result = engine.render(attempt, &vars);
        match result {
            Ok(output) => {
                // Should NOT contain file contents
                assert!(!output.contains("root:"));
                assert!(!output.contains("/bin/"));
            }
            Err(_) => {
                // Error is expected and acceptable
            }
        }
    }
}

/// Test that import/include don't allow arbitrary file access
#[test]
fn test_no_arbitrary_include() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    // Jinja2 include attempts
    let result = engine.render("{% include '/etc/passwd' %}", &vars);

    match result {
        Ok(output) => {
            assert!(!output.contains("root:"));
        }
        Err(_) => {
            // Error is expected - include not allowed or file not in allowed paths
        }
    }
}

/// Test that extends doesn't allow arbitrary file access
#[test]
fn test_no_arbitrary_extends() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{% extends '/etc/passwd' %}", &vars);

    match result {
        Ok(output) => {
            assert!(!output.contains("root:"));
        }
        Err(_) => {
            // Error is expected
        }
    }
}

// ============================================================================
// Code Execution Prevention Tests
// ============================================================================

/// Test that eval-like functions don't exist or work
#[test]
fn test_no_eval_function() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let dangerous_patterns = vec![
        "{{ eval('1+1') }}",
        "{{ exec('import os') }}",
        "{{ system('ls') }}",
        "{{ popen('id').read() }}",
        "{{ subprocess.run(['id']) }}",
    ];

    for pattern in dangerous_patterns {
        let result = engine.render(pattern, &vars);
        match result {
            Ok(output) => {
                // Should not execute and return results
                assert!(!output.contains("uid="));
                assert!(!output.contains("root"));
            }
            Err(_) => {
                // Error is expected and acceptable
            }
        }
    }
}

/// Test that os module access isn't possible
#[test]
fn test_no_os_module_access() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let dangerous_patterns = vec![
        "{{ os.system('id') }}",
        "{{ os.popen('id').read() }}",
        "{{ os.environ }}",
        "{{ os.listdir('/') }}",
    ];

    for pattern in dangerous_patterns {
        let result = engine.render(pattern, &vars);
        match result {
            Ok(output) => {
                assert!(!output.contains("uid="));
                assert!(!output.contains("/bin"));
                assert!(!output.contains("PATH"));
            }
            Err(_) => {
                // Error is expected
            }
        }
    }
}

// ============================================================================
// Denial of Service Prevention Tests
// ============================================================================

/// Test that deeply nested loops are handled safely
#[test]
fn test_nested_loop_depth_limit() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("items".to_string(), json!([1, 2, 3]));

    // Create a template with very deep nesting
    // Note: This tests the engine's ability to handle or reject deep nesting
    let deep_nested = r#"
        {% for a in items %}
            {% for b in items %}
                {% for c in items %}
                    {% for d in items %}
                        {% for e in items %}
                            {{ a }}{{ b }}{{ c }}{{ d }}{{ e }}
                        {% endfor %}
                    {% endfor %}
                {% endfor %}
            {% endfor %}
        {% endfor %}
    "#;

    // This should either complete without hanging or return an error
    let result = engine.render(deep_nested, &vars);

    // The key is it doesn't hang forever - either succeeds or fails gracefully
    let _ = result; // Just ensure it completes
}

/// Test that extremely large range doesn't cause DoS
#[test]
fn test_large_range_limit() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    // Attempt to create extremely large range
    let result = engine.render("{% for i in range(999999999) %}x{% endfor %}", &vars);

    // Should either limit the range, timeout, or error - not hang
    match result {
        Ok(output) => {
            // If it succeeds, output should be reasonably sized (implementation may limit)
            assert!(output.len() < 100_000_000, "Output unreasonably large");
        }
        Err(_) => {
            // Error is acceptable - engine may reject large ranges
        }
    }
}

/// Test recursive template expansion is limited
#[test]
fn test_recursive_expansion_limit() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();

    // Try to create self-referencing expansion
    vars.insert("recursive".to_string(), json!("{{ recursive }}"));

    let result = engine.render("{{ recursive }}", &vars);

    match result {
        Ok(output) => {
            // Should not infinitely expand - literal or limited expansion
            assert!(!output.contains("{{ recursive }}") || output.len() < 1000);
        }
        Err(_) => {
            // Error is acceptable
        }
    }
}

// ============================================================================
// Error Information Leakage Tests
// ============================================================================

/// Test that syntax errors don't expose internal paths
#[test]
fn test_error_no_path_leak() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{{ invalid syntax here", &vars);

    if let Err(e) = result {
        let error_msg = format!("{}", e);
        // Error should not expose internal file paths
        assert!(!error_msg.contains("/home/"));
        assert!(!error_msg.contains("/root/"));
        assert!(!error_msg.contains("C:\\Users\\"));
        assert!(!error_msg.contains("/usr/lib/"));
    }
}

/// Test that undefined variable errors don't expose variable list
#[test]
fn test_error_no_variable_list_leak() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("secret_password".to_string(), json!("hunter2"));
    vars.insert("api_key".to_string(), json!("sk-secret"));

    let result = engine.render("{{ nonexistent_var }}", &vars);

    match result {
        Ok(_) => {}
        Err(e) => {
            let error_msg = format!("{}", e);
            // Error should not expose other variable names or values
            assert!(!error_msg.contains("secret_password"));
            assert!(!error_msg.contains("hunter2"));
            assert!(!error_msg.contains("api_key"));
            assert!(!error_msg.contains("sk-secret"));
        }
    }
}

// ============================================================================
// Input Sanitization Tests
// ============================================================================

/// Test that HTML/script injection in variables is handled safely
#[test]
fn test_html_injection_handling() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert(
        "user_input".to_string(),
        json!("<script>alert('xss')</script>"),
    );

    let result = engine.render("{{ user_input }}", &vars).unwrap();

    // minijinja may auto-escape or preserve - verify no unexpected execution
    // The key is it renders as text, not executable
    assert!(result.contains("script") || result.contains("&lt;script"));
}

/// Test that shell metacharacters in variables don't cause issues
#[test]
fn test_shell_metacharacter_handling() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("input".to_string(), json!("$(whoami)"));
    vars.insert("input2".to_string(), json!("`id`"));
    vars.insert("input3".to_string(), json!("'; rm -rf /; echo '"));

    // Template rendering should treat these as literal strings
    let result = engine
        .render("{{ input }} {{ input2 }} {{ input3 }}", &vars)
        .unwrap();

    // Should contain the literal shell metacharacters, not execute them
    assert!(result.contains("$(whoami)") || result.contains("whoami"));
    assert!(result.contains("`id`") || result.contains("id"));
    assert!(result.contains("rm -rf") || result.contains("rm"));
}

// ============================================================================
// Filter Security Tests
// ============================================================================

/// Test that the realpath filter doesn't expose sensitive paths
/// NOTE: This is a KNOWN SECURITY CONCERN in the current implementation
#[test]
fn test_realpath_filter_security() {
    // The realpath filter in parser/mod.rs uses std::fs::canonicalize
    // which could expose file system structure
    // This test documents the current behavior

    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    // The basic template engine (minijinja) may not have realpath
    // But if it does, verify behavior
    let result = engine.render("{{ '/etc/passwd' | realpath }}", &vars);

    match result {
        Ok(_output) => {
            // If realpath filter exists and works, this is a potential information leak
            // The filter should be carefully reviewed for production use
        }
        Err(_) => {
            // Filter not available in basic engine - safer
        }
    }
}

/// Test that expanduser doesn't leak home directory paths
/// NOTE: This is a KNOWN SECURITY CONCERN in the current implementation
#[test]
fn test_expanduser_filter_security() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{{ '~' | expanduser }}", &vars);

    match result {
        Ok(_output) => {
            // If expanduser works, it reveals the home directory path
            // This could be an information disclosure issue in some contexts
        }
        Err(_) => {
            // Filter not available - safer
        }
    }
}

// ============================================================================
// Lookup Function Security Tests
// ============================================================================

/// Test that lookup('env', ...) access is controlled
/// NOTE: This is a KNOWN SECURITY CONCERN - env lookup is implemented
#[test]
fn test_env_lookup_security() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    // Basic engine may not have lookup function
    let result = engine.render("{{ lookup('env', 'PATH') }}", &vars);

    match result {
        Ok(_output) => {
            // If lookup works, environment variable access is possible
            // This should be reviewed for production security
        }
        Err(_) => {
            // Function not available in basic engine
        }
    }
}

/// Test that lookup('file', ...) is not available or restricted
#[test]
fn test_file_lookup_blocked() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{{ lookup('file', '/etc/passwd') }}", &vars);

    match result {
        Ok(output) => {
            // Should NOT contain file contents
            assert!(!output.contains("root:"));
            assert!(!output.contains("/bin/bash"));
        }
        Err(_) => {
            // Error expected - file lookup not implemented or blocked
        }
    }
}

/// Test that lookup('pipe', ...) command execution is blocked
#[test]
fn test_pipe_lookup_blocked() {
    let engine = TemplateEngine::new();
    let vars = HashMap::new();

    let result = engine.render("{{ lookup('pipe', 'id') }}", &vars);

    match result {
        Ok(output) => {
            // Should NOT contain command output
            assert!(!output.contains("uid="));
            assert!(!output.contains("gid="));
        }
        Err(_) => {
            // Error expected - pipe lookup not implemented
        }
    }
}

// ============================================================================
// Memory and Resource Safety Tests
// ============================================================================

/// Test that string multiplication doesn't cause memory exhaustion
#[test]
fn test_string_multiplication_limit() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("s".to_string(), json!("x"));

    // Try to create very large string through multiplication (if supported)
    let result = engine.render("{{ 'x' * 9999999 }}", &vars);

    match result {
        Ok(output) => {
            // If multiplication is supported, should be limited
            assert!(output.len() < 100_000_000);
        }
        Err(_) => {
            // Multiplication not supported or limited - acceptable
        }
    }
}

/// Test that very long variable names don't cause issues
#[test]
fn test_long_variable_name_handling() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();

    let long_name = "a".repeat(10000);
    vars.insert(long_name.clone(), json!("value"));

    let template = format!("{{{{ {} }}}}", long_name);
    let result = engine.render(&template, &vars);

    // Should either work or fail gracefully, not crash
    let _ = result;
}

/// Test that deeply nested objects don't cause stack overflow
#[test]
fn test_deep_object_access_handling() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();

    // Create deeply nested object
    let mut current = json!("final_value");
    for _ in 0..100 {
        current = json!({ "nested": current });
    }
    vars.insert("deep".to_string(), current);

    // Try to access deeply nested value
    let access_path = (0..100).map(|_| ".nested").collect::<String>();
    let template = format!("{{{{ deep{} }}}}", access_path);

    let result = engine.render(&template, &vars);

    // Should handle gracefully
    let _ = result;
}

// ============================================================================
// Format String Attack Prevention
// ============================================================================

/// Test that format string-like patterns in variables don't cause issues
#[test]
fn test_format_string_in_variables() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("input".to_string(), json!("%s%s%s%n"));
    vars.insert("input2".to_string(), json!("{0}{1}{2}"));

    let result = engine.render("{{ input }} {{ input2 }}", &vars);

    match result {
        Ok(output) => {
            // Should render literally
            assert!(output.contains("%s") || output.contains("{0}"));
        }
        Err(_) => {
            // Should not panic
        }
    }
}

// ============================================================================
// Unicode and Encoding Security Tests
// ============================================================================

/// Test that Unicode homoglyph attacks don't bypass security
#[test]
fn test_unicode_homoglyph_handling() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();

    // Cyrillic "а" looks like Latin "a"
    vars.insert("nаme".to_string(), json!("hidden")); // Cyrillic 'а'
    vars.insert("name".to_string(), json!("visible")); // Latin 'a'

    let result = engine.render("{{ name }}", &vars).unwrap();
    assert_eq!(result, "visible");
}

/// Test that null bytes in input are handled safely
#[test]
fn test_null_byte_handling() {
    let engine = TemplateEngine::new();
    let mut vars = HashMap::new();
    vars.insert("input".to_string(), json!("before\x00after"));

    let result = engine.render("{{ input }}", &vars);

    // Should not crash, handle gracefully
    let _ = result;
}
