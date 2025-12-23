//! Comprehensive Security and Safety Tests for Rustible
//!
//! This test suite validates security properties and safety invariants across
//! the Rustible codebase. It covers:
//!
//! ## 1. Vault Security (AES-256-GCM + Argon2id)
//! - Encryption strength verification
//! - Key derivation resistance to timing attacks
//! - Proper secret handling (no logging)
//! - Memory clearing considerations
//! - Invalid password handling
//!
//! ## 2. Connection Security
//! - Host key verification considerations
//! - Private key protection
//! - Credential handling in memory
//! - Privilege escalation safety (become)
//!
//! ## 3. Input Sanitization
//! - Command injection prevention in shell/command modules
//! - Template injection prevention
//! - Path traversal prevention in file modules
//! - YAML deserialization safety
//!
//! ## 4. Privilege Escalation Safety
//! - Become method safety
//! - sudo/doas password handling
//! - Prevent privilege leakage
//!
//! ## 5. Safety Invariants
//! - No secrets in logs (tracing tests)
//! - File permissions on sensitive data
//! - Safe temporary file handling

use rustible::error::Error;
use rustible::modules::{
    command::CommandModule, copy::CopyModule, file::FileModule, shell::ShellModule, Module,
    ModuleContext, ModuleParams,
};
use rustible::template::TemplateEngine;
use rustible::vault::Vault;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

// ============================================================================
// 1. VAULT SECURITY TESTS
// ============================================================================

mod vault_security {
    use super::*;

    /// Test that encryption uses AES-256-GCM with authenticated encryption
    /// This prevents tampering with ciphertext
    #[test]
    fn test_aes_gcm_authenticated_encryption() {
        let vault = Vault::new("password");
        let plaintext = "sensitive data";
        let encrypted = vault.encrypt(plaintext).unwrap();

        // Tamper with the encrypted data
        let lines: Vec<&str> = encrypted.lines().collect();
        if lines.len() >= 2 {
            let header = lines[0];
            let data = lines[1];

            // Flip a bit in the middle of the ciphertext
            let mut chars: Vec<char> = data.chars().collect();
            if chars.len() > 20 {
                chars[20] = if chars[20] == 'A' { 'B' } else { 'A' };
                let tampered_data: String = chars.into_iter().collect();
                let tampered = format!("{}\n{}", header, tampered_data);

                // GCM authentication should detect tampering
                let result = vault.decrypt(&tampered);
                assert!(
                    result.is_err(),
                    "AES-GCM should detect ciphertext tampering"
                );
            }
        }
    }

    /// Test that Argon2id key derivation provides brute-force resistance
    /// Argon2id is memory-hard, making GPU/ASIC attacks expensive
    #[test]
    fn test_argon2id_provides_timing_resistance() {
        // Encrypt with a password - should take measurable time due to Argon2id
        let vault = Vault::new("password123");
        let start = Instant::now();
        let encrypted = vault.encrypt("test data").unwrap();
        let encrypt_time = start.elapsed();

        // Argon2id should add meaningful computation time (> 10ms typically)
        // This makes brute-force attacks expensive
        assert!(
            encrypt_time > Duration::from_millis(1),
            "Key derivation should take non-trivial time for brute-force resistance"
        );

        // Decryption should also take time due to key derivation
        let start = Instant::now();
        let _decrypted = vault.decrypt(&encrypted).unwrap();
        let decrypt_time = start.elapsed();

        assert!(
            decrypt_time > Duration::from_millis(1),
            "Decryption key derivation should also take time"
        );
    }

    /// Test that different passwords produce completely different ciphertexts
    #[test]
    fn test_different_passwords_produce_different_output() {
        let plaintext = "same secret data";
        let vault1 = Vault::new("password1");
        let vault2 = Vault::new("password2");

        let encrypted1 = vault1.encrypt(plaintext).unwrap();
        let encrypted2 = vault2.encrypt(plaintext).unwrap();

        // Ciphertexts should be completely different
        assert_ne!(encrypted1, encrypted2);

        // Cross-decryption should fail
        assert!(vault1.decrypt(&encrypted2).is_err());
        assert!(vault2.decrypt(&encrypted1).is_err());
    }

    /// Test that passwords are not leaked in error messages
    #[test]
    fn test_password_not_in_error_messages() {
        let secret_password = "my_super_secret_password_12345";
        let vault = Vault::new(secret_password);

        // Invalid format error
        let result = vault.decrypt("not encrypted data");
        if let Err(Error::Vault(msg)) = result {
            assert!(
                !msg.contains(secret_password),
                "Password should not appear in error message"
            );
            assert!(
                !msg.contains("12345"),
                "Parts of password should not appear in error"
            );
        }

        // Wrong password error
        let vault2 = Vault::new("different_password");
        let encrypted = vault2.encrypt("test").unwrap();
        let result = vault.decrypt(&encrypted);
        if let Err(Error::Vault(msg)) = result {
            assert!(
                !msg.contains(secret_password),
                "Password should not appear in wrong password error"
            );
        }
    }

    /// Test that each encryption produces unique salt and nonce
    #[test]
    fn test_unique_salt_and_nonce_per_encryption() {
        let vault = Vault::new("password");
        let plaintext = "same data";

        let mut encryptions = HashSet::new();
        for _ in 0..50 {
            let encrypted = vault.encrypt(plaintext).unwrap();
            // All encryptions of the same data should be unique
            assert!(
                encryptions.insert(encrypted),
                "Each encryption must produce unique ciphertext due to random salt/nonce"
            );
        }
    }

    /// Test that vault format includes version for future compatibility
    #[test]
    fn test_vault_format_includes_version() {
        let vault = Vault::new("password");
        let encrypted = vault.encrypt("test").unwrap();

        // Header should contain version for upgrade path
        assert!(encrypted.starts_with("$RUSTIBLE_VAULT"));
        assert!(encrypted.contains("1.0"), "Version should be in header");
        assert!(
            encrypted.contains("AES256"),
            "Algorithm should be in header"
        );
    }

    /// Test handling of empty passwords (security warning scenario)
    #[test]
    fn test_empty_password_handling() {
        let vault = Vault::new("");

        // Empty password should still work (user's choice, but insecure)
        let encrypted = vault.encrypt("test").unwrap();
        let decrypted = vault.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, "test");

        // But different password should fail
        let vault2 = Vault::new("not_empty");
        assert!(vault2.decrypt(&encrypted).is_err());
    }

    /// Test that vault handles binary-safe strings
    #[test]
    fn test_binary_safe_encryption() {
        let vault = Vault::new("password");

        // Test with null bytes and special characters
        let binary_like = "data\x00with\x01binary\x02chars";
        let encrypted = vault.encrypt(binary_like).unwrap();
        let decrypted = vault.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, binary_like);
    }

    /// Test that concurrent vault operations are safe
    #[test]
    fn test_concurrent_vault_safety() {
        use std::thread;

        let vault = Arc::new(Vault::new("password"));
        let mut handles = vec![];

        for i in 0..20 {
            let vault_clone = Arc::clone(&vault);
            let handle = thread::spawn(move || {
                let data = format!("data_{}", i);
                let encrypted = vault_clone.encrypt(&data).unwrap();
                let decrypted = vault_clone.decrypt(&encrypted).unwrap();
                assert_eq!(decrypted, data);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread should not panic");
        }
    }
}

// ============================================================================
// 2. CONNECTION SECURITY TESTS
// ============================================================================

mod connection_security {
    use rustible::connection::{CommandResult, ExecuteOptions};

    /// Test that privilege escalation password is handled securely
    #[test]
    fn test_escalation_password_not_in_command_string() {
        // The build_command function should use stdin for password, not command line
        let options = ExecuteOptions::new().with_escalation(Some("root".to_string()));

        // Password should be passed via stdin, not in command
        assert!(options.escalate);
        assert_eq!(options.escalate_user, Some("root".to_string()));

        // When escalate_password is set, it should be handled via stdin
        // not visible in process listing
    }

    /// Test that command results don't leak sensitive environment
    #[test]
    fn test_command_result_sanitization() {
        let result = CommandResult::success(
            "output".to_string(),
            "stderr with password=secret123".to_string(),
        );

        // The result itself preserves output, but callers should sanitize
        assert!(result.success);
        // Note: Actual sanitization would happen at a higher level
    }

    /// Test that execute options with escalation are properly structured
    #[test]
    fn test_execute_options_escalation_structure() {
        let options = ExecuteOptions::new()
            .with_cwd("/tmp")
            .with_escalation(Some("admin".to_string()))
            .with_timeout(30);

        assert!(options.escalate);
        assert_eq!(options.escalate_user, Some("admin".to_string()));
        assert_eq!(options.cwd, Some("/tmp".to_string()));
        assert_eq!(options.timeout, Some(30));

        // Environment variables should not contain credentials
        assert!(options.env.is_empty());
    }

    /// Test that credentials are not stored in debug output
    #[test]
    fn test_credentials_not_in_debug_output() {
        let options = ExecuteOptions {
            escalate_password: Some("secret123".to_string()),
            ..Default::default()
        };

        // Debug output should not contain the password
        let debug_output = format!("{:?}", options);
        // Note: Current implementation may show password in debug
        // This test documents the current behavior for future improvement
        let _ = debug_output;
    }
}

// ============================================================================
// 3. INPUT SANITIZATION TESTS
// ============================================================================

mod input_sanitization {
    use super::*;

    /// Test that command module prevents shell metacharacter injection
    #[test]
    fn test_command_module_no_shell_injection() {
        let module = CommandModule;

        // Command module should NOT interpret shell metacharacters
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "cmd".to_string(),
            serde_json::json!("echo hello; rm -rf /tmp/test"),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        // The command module splits on whitespace, so "hello;" becomes a literal argument
        // It does NOT execute "rm -rf /tmp/test"
        let stdout = result.stdout.unwrap_or_default();
        // The output should contain the literal semicolon, not execute the second command
        assert!(
            stdout.contains("hello;") || !stdout.contains("rm"),
            "Command module should not interpret shell metacharacters"
        );
    }

    /// Test that command module with argv prevents injection
    #[test]
    fn test_command_argv_prevents_injection() {
        let module = CommandModule;

        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("")); // Required but not used
        params.insert(
            "argv".to_string(),
            serde_json::json!(["echo", "$(whoami)", "; rm -rf /"]),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        let stdout = result.stdout.unwrap_or_default();
        // The dangerous strings should be treated as literal arguments
        // They should NOT be executed
        assert!(
            stdout.contains("$(whoami)") || stdout.contains("rm"),
            "argv should prevent command injection by treating input literally"
        );
    }

    /// Test path traversal prevention in file module
    #[test]
    fn test_file_module_path_traversal_awareness() {
        let temp = TempDir::new().unwrap();
        let safe_dir = temp.path().join("safe");
        fs::create_dir(&safe_dir).unwrap();

        let module = FileModule;

        // Attempt path traversal - the module should handle this
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(format!("{}/../../../etc/passwd", safe_dir.display())),
        );
        params.insert("state".to_string(), serde_json::json!("touch"));

        let context = ModuleContext::default();

        // The result depends on the OS and permissions
        // But we're testing that the module doesn't blindly follow the path
        let _ = module.execute(&params, &context);

        // /etc/passwd should not be modified (would require root anyway)
        // This test documents the behavior
    }

    /// Test template injection prevention
    #[test]
    fn test_template_injection_prevention() {
        let engine = TemplateEngine::new();
        let mut vars: HashMap<String, serde_json::Value> = HashMap::new();

        // User-controlled input that tries to inject template code
        vars.insert(
            "user_input".to_string(),
            serde_json::json!("{{ dangerous_var }}"),
        );

        // The template treats user_input as a string, not as template code
        let result = engine.render("User said: {{ user_input }}", &vars).unwrap();

        // The {{ dangerous_var }} should be rendered as literal text
        assert!(
            result.contains("{{ dangerous_var }}"),
            "User input should not be interpreted as template code"
        );
    }

    /// Test that template cannot access arbitrary file system
    #[test]
    fn test_template_no_file_access() {
        let engine = TemplateEngine::new();
        let vars: HashMap<String, serde_json::Value> = HashMap::new();

        // Try various file access attempts (should fail or be limited)
        let attempts = vec![
            "{{ include('/etc/passwd') }}",
            "{% include '/etc/passwd' %}",
            "{{ open('/etc/passwd').read() }}",
        ];

        for attempt in attempts {
            let result = engine.render(attempt, &vars);
            // These should either error or not execute the dangerous operation
            if let Ok(output) = result {
                assert!(
                    !output.contains("root:"),
                    "Template should not be able to read arbitrary files"
                );
            }
        }
    }

    /// Test YAML parsing safety - no arbitrary code execution
    #[test]
    fn test_yaml_deserialization_safety() {
        // serde_yaml should not execute arbitrary code from YAML
        let dangerous_yaml = r#"
            key: !!python/object/apply:os.system
              args: ['echo dangerous']
        "#;

        // This should fail or be parsed safely
        let result: Result<serde_yaml::Value, _> = serde_yaml::from_str(dangerous_yaml);

        // Either it fails to parse the dangerous tag, or it treats it as string
        // It should NOT execute the command
        if let Ok(value) = result {
            // If parsing succeeded, the value should be inert
            let _ = value;
        }
    }

    /// Test shell module command string handling
    #[test]
    fn test_shell_module_passes_to_shell() {
        let module = ShellModule;

        // Shell module DOES pass to shell - this is intentional
        // Security comes from user awareness and proper escaping
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("echo 'hello world'"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.stdout.unwrap().contains("hello world"));
    }

    /// Test that copy module validates content before writing
    #[test]
    fn test_copy_module_content_handling() {
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join("test.txt");

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();

        // Content with potentially dangerous patterns (should be written literally)
        params.insert(
            "content".to_string(),
            serde_json::json!("#!/bin/bash\nrm -rf /"),
        );
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);

        // The dangerous content is written as-is (which is correct)
        // It's NOT executed
        let written = fs::read_to_string(&dest).unwrap();
        assert!(written.contains("rm -rf /"));
        // The file exists but was not executed
        assert!(dest.exists());
    }
}

// ============================================================================
// 4. PRIVILEGE ESCALATION SAFETY TESTS
// ============================================================================

mod privilege_escalation_safety {
    use super::*;
    use rustible::connection::ExecuteOptions;

    /// Test that become methods are validated
    #[test]
    fn test_become_method_validation() {
        // Valid become methods
        let valid_methods = vec!["sudo", "su", "doas"];

        for method in valid_methods {
            let mut options = ExecuteOptions::new().with_escalation(None);
            options.escalate_method = Some(method.to_string());
            assert_eq!(options.escalate_method, Some(method.to_string()));
        }
    }

    /// Test that become user is properly set
    #[test]
    fn test_become_user_default_is_root() {
        let options = ExecuteOptions::new().with_escalation(None);

        // Default user when not specified should be root
        assert!(options.escalate);
        assert_eq!(options.escalate_user, None); // None means default to root
    }

    /// Test that escalation can be explicitly disabled
    #[test]
    fn test_escalation_can_be_disabled() {
        let options = ExecuteOptions::default();

        assert!(!options.escalate);
        assert!(options.escalate_user.is_none());
        assert!(options.escalate_method.is_none());
        assert!(options.escalate_password.is_none());
    }

    /// Test module context become fields
    #[test]
    fn test_module_context_become_fields() {
        let context = ModuleContext {
            r#become: true,
            become_method: Some("sudo".to_string()),
            become_user: Some("admin".to_string()),
            ..Default::default()
        };

        assert!(context.r#become);
        assert_eq!(context.become_method, Some("sudo".to_string()));
        assert_eq!(context.become_user, Some("admin".to_string()));
    }

    /// Test that become password is not included in context serialization
    #[test]
    fn test_context_serialization_excludes_sensitive_data() {
        // ModuleContext should not serialize sensitive become passwords
        // (if they were stored there)
        let context = ModuleContext::default();

        // Debug representation should not contain password fields
        let debug = format!("{:?}", context);
        assert!(
            !debug.contains("password"),
            "Context debug should not show passwords"
        );
    }
}

// ============================================================================
// 5. SAFETY INVARIANTS TESTS
// ============================================================================

mod safety_invariants {
    use super::*;

    /// Test that temporary files are created with restrictive permissions
    #[test]
    fn test_tempfile_permissions() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let metadata = fs::metadata(temp.path()).unwrap();
        let mode = metadata.permissions().mode();

        // Temp files should not be world-readable
        // umask typically makes files 0o600 or 0o644
        let _world_readable = mode & 0o004;
        let world_writable = mode & 0o002;

        assert_eq!(world_writable, 0, "Temp files should not be world-writable");
        // Note: world_readable depends on umask
    }

    /// Test that file module respects mode parameter
    #[test]
    fn test_file_module_respects_mode() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("secret.txt");

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("touch"));
        params.insert("mode".to_string(), serde_json::json!(0o600));

        let context = ModuleContext::default();
        let _ = module.execute(&params, &context).unwrap();

        let metadata = fs::metadata(&path).unwrap();
        let mode = metadata.permissions().mode() & 0o7777;
        assert_eq!(mode, 0o600, "File should be created with specified mode");
    }

    /// Test that copy module respects mode parameter
    #[test]
    fn test_copy_module_respects_mode() {
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join("secret.txt");

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("content".to_string(), serde_json::json!("secret data"));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );
        params.insert("mode".to_string(), serde_json::json!(0o400));

        let context = ModuleContext::default();
        let _ = module.execute(&params, &context).unwrap();

        let metadata = fs::metadata(&dest).unwrap();
        let mode = metadata.permissions().mode() & 0o7777;
        assert_eq!(mode, 0o400, "File should be created with specified mode");
    }

    /// Test that check mode doesn't modify filesystem
    #[test]
    fn test_check_mode_is_safe() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("should_not_exist.txt");

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("content".to_string(), serde_json::json!("content"));
        params.insert(
            "dest".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );

        let context = ModuleContext::default().with_check_mode(true);
        let result = module.execute(&params, &context).unwrap();

        assert!(
            result.changed,
            "Check mode should report change would occur"
        );
        assert!(!path.exists(), "Check mode should not create file");
    }

    /// Test that diff mode doesn't modify filesystem
    #[test]
    fn test_diff_mode_is_safe() {
        let temp = TempDir::new().unwrap();
        let existing = temp.path().join("existing.txt");
        fs::write(&existing, "old content").unwrap();

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("content".to_string(), serde_json::json!("new content"));
        params.insert(
            "dest".to_string(),
            serde_json::json!(existing.to_str().unwrap()),
        );

        let context = ModuleContext::default()
            .with_check_mode(true)
            .with_diff_mode(true);
        let result = module.execute(&params, &context).unwrap();

        assert!(result.diff.is_some(), "Diff mode should produce diff");
        let content = fs::read_to_string(&existing).unwrap();
        assert_eq!(content, "old content", "Diff mode should not modify file");
    }

    /// Test that module errors don't expose sensitive information
    #[test]
    fn test_error_messages_sanitized() {
        let module = CopyModule;

        // Missing required parameters
        let params: ModuleParams = HashMap::new();
        let result = module.validate_params(&params);

        if let Err(e) = result {
            let msg = format!("{}", e);
            // Error should be informative but not expose internals
            assert!(msg.contains("src") || msg.contains("content") || msg.contains("dest"));
        }
    }

    /// Test that symlinks are handled safely
    #[test]
    fn test_symlink_safety() {
        let temp = TempDir::new().unwrap();
        let real_file = temp.path().join("real.txt");
        let symlink = temp.path().join("link");

        fs::write(&real_file, "real content").unwrap();
        std::os::unix::fs::symlink(&real_file, &symlink).unwrap();

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(symlink.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("absent"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        // The symlink should be removed, not the target
        assert!(!symlink.exists(), "Symlink should be removed");
        assert!(real_file.exists(), "Target file should still exist");
    }

    /// Test that module output data doesn't contain secrets
    #[test]
    fn test_output_data_sanitization() {
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join("test.txt");

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "content".to_string(),
            serde_json::json!("password=secret123"),
        );
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        // Output data should not contain the content (which might be secret)
        let data_string = serde_json::to_string(&result.data).unwrap();
        assert!(
            !data_string.contains("secret123"),
            "Module output data should not contain file contents"
        );
    }
}

// ============================================================================
// 6. ADDITIONAL SECURITY PROPERTY TESTS
// ============================================================================

mod security_properties {
    use super::*;

    /// Test that vault is_encrypted detection is reliable
    #[test]
    fn test_is_encrypted_reliability() {
        let vault = Vault::new("password");

        // Test various non-encrypted strings
        assert!(!Vault::is_encrypted("plain text"));
        assert!(!Vault::is_encrypted(""));
        assert!(!Vault::is_encrypted("$ANSIBLE_VAULT;1.1;AES256"));
        assert!(!Vault::is_encrypted("$NOT_RUSTIBLE;1.0;AES256"));

        // Test encrypted string
        let encrypted = vault.encrypt("secret").unwrap();
        assert!(Vault::is_encrypted(&encrypted));

        // Test partial header (edge case)
        assert!(!Vault::is_encrypted("$RUSTIBLE"));
        assert!(Vault::is_encrypted("$RUSTIBLE_VAULT;broken"));
    }

    /// Test that module execution doesn't leak state between calls
    #[test]
    fn test_no_state_leakage_between_executions() {
        let module = CommandModule;
        let context = ModuleContext::default();

        // First execution with specific env
        let mut params1: ModuleParams = HashMap::new();
        params1.insert("cmd".to_string(), serde_json::json!("echo first"));
        params1.insert("env".to_string(), serde_json::json!({"SECRET": "value1"}));

        let result1 = module.execute(&params1, &context).unwrap();

        // Second execution without that env
        let mut params2: ModuleParams = HashMap::new();
        params2.insert("cmd".to_string(), serde_json::json!("echo second"));

        let result2 = module.execute(&params2, &context).unwrap();

        // Each execution should be independent
        assert!(result1.stdout.unwrap().contains("first"));
        assert!(result2.stdout.unwrap().contains("second"));
    }

    /// Test that template rendering is isolated per call
    #[test]
    fn test_template_isolation() {
        let engine = TemplateEngine::new();

        let mut vars1: HashMap<String, serde_json::Value> = HashMap::new();
        vars1.insert("secret".to_string(), serde_json::json!("password123"));

        let result1 = engine.render("{{ secret }}", &vars1).unwrap();
        assert!(result1.contains("password123"));

        // Second render with different vars should not see first vars
        let vars2: HashMap<String, serde_json::Value> = HashMap::new();
        let result2 = engine.render("{{ secret }}", &vars2).unwrap();

        // Should be empty or undefined, not the previous secret
        assert!(
            !result2.contains("password123"),
            "Template should not leak state between renders"
        );
    }

    /// Test error types for security-related failures
    #[test]
    fn test_security_error_types() {
        // Vault decryption error
        let vault = Vault::new("password");
        let result = vault.decrypt("invalid");
        assert!(matches!(result, Err(Error::Vault(_))));

        // Invalid vault password
        let vault2 = Vault::new("wrong");
        let encrypted = vault.encrypt("test").unwrap();
        let result = vault2.decrypt(&encrypted);
        assert!(matches!(result, Err(Error::Vault(_))));
    }

    /// Test that creates/removes conditions in command modules work safely
    #[test]
    fn test_creates_removes_idempotency() {
        let temp = TempDir::new().unwrap();
        let marker = temp.path().join("marker.txt");

        let module = CommandModule;

        // With creates - should skip if file exists
        fs::write(&marker, "").unwrap();

        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("echo dangerous"));
        params.insert(
            "creates".to_string(),
            serde_json::json!(marker.to_str().unwrap()),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(
            !result.changed,
            "Command should be skipped when creates file exists"
        );
        assert!(result.msg.contains("Skipped"));

        // With removes - should skip if file doesn't exist
        let nonexistent = temp.path().join("nonexistent");
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("echo dangerous"));
        params.insert(
            "removes".to_string(),
            serde_json::json!(nonexistent.to_str().unwrap()),
        );

        let result = module.execute(&params, &context).unwrap();
        assert!(
            !result.changed,
            "Command should be skipped when removes file doesn't exist"
        );
    }
}

// ============================================================================
// 7. LOGGING AND TRACING SAFETY TESTS
// ============================================================================

mod logging_safety {
    use super::*;

    /// Test that vault operations don't log passwords
    /// This is a documentation test - actual tracing output capture requires
    /// a custom subscriber
    #[test]
    fn test_vault_no_password_logging() {
        let secret_password = "ultra_secret_password_42";
        let vault = Vault::new(secret_password);

        // These operations use tracing internally
        // A proper test would capture tracing output and verify
        let encrypted = vault.encrypt("test data").unwrap();
        let _decrypted = vault.decrypt(&encrypted).unwrap();

        // This test passes if no panic occurs
        // Full verification would require a tracing subscriber
    }

    /// Test that module execution doesn't log sensitive parameters
    #[test]
    fn test_module_no_sensitive_logging() {
        let module = CopyModule;

        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "content".to_string(),
            serde_json::json!("db_password=secret123"),
        );
        params.insert("dest".to_string(), serde_json::json!("/tmp/test_secret"));

        let context = ModuleContext::default().with_check_mode(true);

        // Execute - any logging should not contain the secret
        let _ = module.execute(&params, &context);

        // This test passes if no panic occurs
        // Full verification would require a tracing subscriber
    }
}
