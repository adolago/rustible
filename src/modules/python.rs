//! Python module fallback executor
//!
//! This module enables execution of any Ansible Python module, providing
//! backwards compatibility with the entire Ansible module ecosystem.
//!
//! It uses the AnsiballZ-style bundling format that Ansible uses:
//! 1. Find the Ansible module Python file
//! 2. Bundle it with arguments into a base64-encoded wrapper
//! 3. Transfer to remote host via SSH
//! 4. Execute with Python interpreter
//! 5. Parse JSON result

use super::{ModuleError, ModuleOutput, ModuleParams, ModuleResult};
use crate::connection::{CommandResult, Connection, ExecuteOptions};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Result from Ansible module execution (JSON format)
#[derive(Debug, Deserialize, Serialize)]
pub struct AnsibleModuleResult {
    /// Whether the module changed state
    #[serde(default)]
    pub changed: bool,

    /// Human-readable message
    #[serde(default)]
    pub msg: Option<String>,

    /// Whether the module failed
    #[serde(default)]
    pub failed: bool,

    /// Failure message
    #[serde(default)]
    pub failure_msg: Option<String>,

    /// Whether the task was skipped
    #[serde(default)]
    pub skipped: bool,

    /// Additional return values
    #[serde(flatten)]
    pub data: HashMap<String, serde_json::Value>,
}

/// Python module executor for Ansible backwards compatibility
pub struct PythonModuleExecutor {
    /// Paths to search for Ansible modules
    module_paths: Vec<PathBuf>,

    /// Cache of discovered module locations
    module_cache: HashMap<String, PathBuf>,
}

impl Default for PythonModuleExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonModuleExecutor {
    /// Create a new Python module executor with default search paths
    pub fn new() -> Self {
        let mut module_paths = Vec::new();

        // Standard Ansible module locations
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            // User collections
            module_paths.push(home.join(".ansible/collections"));
            // User modules
            module_paths.push(home.join(".ansible/plugins/modules"));
        }

        // System-wide locations
        module_paths.push(PathBuf::from("/usr/share/ansible/plugins/modules"));
        module_paths.push(PathBuf::from(
            "/usr/lib/python3/dist-packages/ansible/modules",
        ));

        // Check ANSIBLE_LIBRARY environment variable
        if let Some(lib_path) = std::env::var_os("ANSIBLE_LIBRARY") {
            for path in std::env::split_paths(&lib_path) {
                module_paths.push(path);
            }
        }

        Self {
            module_paths,
            module_cache: HashMap::new(),
        }
    }

    /// Add a custom module search path
    pub fn add_module_path(&mut self, path: impl Into<PathBuf>) {
        self.module_paths.insert(0, path.into());
    }

    /// Find an Ansible module by name
    ///
    /// Searches in order:
    /// 1. Module cache
    /// 2. User collections (~/.ansible/collections)
    /// 3. User modules (~/.ansible/plugins/modules)
    /// 4. System modules (/usr/share/ansible/...)
    pub fn find_module(&mut self, name: &str) -> Option<PathBuf> {
        // Check cache first
        if let Some(path) = self.module_cache.get(name) {
            if path.exists() {
                return Some(path.clone());
            }
        }

        // Handle fully-qualified collection names (e.g., "ansible.builtin.apt")
        let module_name = if name.contains('.') {
            name.rsplit('.').next().unwrap_or(name)
        } else {
            name
        };

        // Search all paths
        for base_path in &self.module_paths {
            if !base_path.exists() {
                continue;
            }

            // Try direct module file
            let direct = base_path.join(format!("{}.py", module_name));
            if direct.exists() {
                debug!("Found module {} at {}", name, direct.display());
                self.module_cache.insert(name.to_string(), direct.clone());
                return Some(direct);
            }

            // Try in subdirectories (Ansible organizes by category)
            if let Ok(entries) = std::fs::read_dir(base_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let module_file = path.join(format!("{}.py", module_name));
                        if module_file.exists() {
                            debug!("Found module {} at {}", name, module_file.display());
                            self.module_cache
                                .insert(name.to_string(), module_file.clone());
                            return Some(module_file);
                        }
                    }
                }
            }
        }

        warn!("Module {} not found in any search path", name);
        None
    }

    /// Bundle a module with its arguments into an AnsiballZ-style payload
    ///
    /// Returns a Python script that can be executed on the remote host
    pub fn bundle(&self, module_path: &Path, args: &ModuleParams) -> ModuleResult<String> {
        // Read the module source
        let module_source = std::fs::read_to_string(module_path).map_err(|e| {
            ModuleError::ExecutionFailed(format!(
                "Failed to read module {}: {}",
                module_path.display(),
                e
            ))
        })?;

        // Serialize arguments to JSON
        let args_json = serde_json::to_string(args).map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to serialize module arguments: {}", e))
        })?;

        // Base64 encode the module source
        let module_b64 = BASE64.encode(module_source.as_bytes());
        let args_b64 = BASE64.encode(args_json.as_bytes());

        // Create the wrapper script
        let wrapper = format!(
            r#"#!/usr/bin/env python
# -*- coding: utf-8 -*-
# Rustible AnsiballZ-compatible module wrapper
import sys
import os
import json
import base64
import tempfile

MODULE_B64 = '{module_b64}'
ARGS_B64 = '{args_b64}'

def main():
    # Decode module and args
    module_code = base64.b64decode(MODULE_B64).decode('utf-8')
    args_json = base64.b64decode(ARGS_B64).decode('utf-8')
    args = json.loads(args_json)
    
    # Create temp file for module
    fd, module_path = tempfile.mkstemp(suffix='.py', prefix='rustible_')
    try:
        with os.fdopen(fd, 'w') as f:
            f.write(module_code)
        
        # Set up module arguments in environment (Ansible style)
        os.environ['ANSIBLE_MODULE_ARGS'] = args_json
        
        # Import and execute the module
        import importlib.util
        spec = importlib.util.spec_from_file_location("ansible_module", module_path)
        module = importlib.util.module_from_spec(spec)
        
        # Capture result
        result = {{'changed': False, 'failed': False}}
        
        try:
            # Many Ansible modules have a main() that returns or prints JSON
            if hasattr(module, 'main'):
                spec.loader.exec_module(module)
                # Check if module.main exists after loading
                if callable(getattr(module, 'main', None)):
                    ret = module.main()
                    if isinstance(ret, dict):
                        result.update(ret)
            else:
                # Execute module directly
                spec.loader.exec_module(module)
        except SystemExit as e:
            # Ansible modules often call exit_json/fail_json which raises SystemExit
            pass
        except Exception as e:
            result['failed'] = True
            result['msg'] = str(e)
        
        print(json.dumps(result))
        
    finally:
        try:
            os.unlink(module_path)
        except:
            pass

if __name__ == '__main__':
    main()
"#
        );

        Ok(wrapper)
    }

    /// Execute a Python module on a remote connection
    pub async fn execute(
        &mut self,
        conn: &dyn Connection,
        module_name: &str,
        args: &ModuleParams,
        python_interpreter: &str,
    ) -> ModuleResult<ModuleOutput> {
        // Find the module
        let module_path = self.find_module(module_name)
            .ok_or_else(|| ModuleError::ModuleNotFound(format!(
                "Ansible module '{}' not found. Ensure Ansible is installed or check ANSIBLE_LIBRARY path.",
                module_name
            )))?;

        // Bundle the module
        let wrapper = self.bundle(&module_path, args)?;

        debug!(
            "Executing Python module {} via {} ({} bytes)",
            module_name,
            python_interpreter,
            wrapper.len()
        );

        // Execute on remote host
        // We pipe the script directly to Python for efficiency (like Ansible pipelining)
        let command = format!("{} -c {}", python_interpreter, shell_escape(&wrapper));

        let result = conn
            .execute(&command, Some(ExecuteOptions::new()))
            .await
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to execute Python module: {}", e))
            })?;

        // Parse the result
        self.parse_result(&result, module_name)
    }

    /// Parse the JSON result from Python module execution
    fn parse_result(
        &self,
        result: &CommandResult,
        module_name: &str,
    ) -> ModuleResult<ModuleOutput> {
        let stdout = result.stdout.trim();

        // Try to find JSON in the output (skip any non-JSON preamble)
        let json_start = stdout.find('{');
        let json_str = match json_start {
            Some(pos) => &stdout[pos..],
            None => stdout,
        };

        // Parse the JSON result
        let parsed: AnsibleModuleResult = serde_json::from_str(json_str).map_err(|e| {
            // If JSON parsing fails, check if it's a command error
            if result.exit_code != 0 {
                ModuleError::ExecutionFailed(format!(
                    "Module {} failed with exit code {}: {}",
                    module_name,
                    result.exit_code,
                    result.stderr.trim()
                ))
            } else {
                ModuleError::ExecutionFailed(format!(
                    "Failed to parse module {} output as JSON: {}. Output: {}",
                    module_name, e, stdout
                ))
            }
        })?;

        // Convert to ModuleOutput
        if parsed.failed {
            return Err(ModuleError::ExecutionFailed(
                parsed.msg.unwrap_or_else(|| "Module failed".to_string()),
            ));
        }

        let msg = parsed
            .msg
            .unwrap_or_else(|| format!("Module {} executed successfully", module_name));

        let mut output = if parsed.changed {
            ModuleOutput::changed(msg)
        } else {
            ModuleOutput::ok(msg)
        };

        // Add additional data from module result
        for (key, value) in parsed.data {
            // Skip internal keys
            if !matches!(key.as_str(), "changed" | "failed" | "msg" | "skipped") {
                output = output.with_data(key, value);
            }
        }

        Ok(output)
    }
}

/// Escape a string for shell execution
fn shell_escape(s: &str) -> String {
    // Use Python's ability to handle base64 to avoid shell escaping issues
    let b64 = BASE64.encode(s.as_bytes());
    format!(
        "\"import base64,sys;exec(base64.b64decode('{}').decode())\"",
        b64
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let executor = PythonModuleExecutor::new();
        assert!(!executor.module_paths.is_empty());
    }

    #[test]
    fn test_bundle_generation() {
        let executor = PythonModuleExecutor::new();
        let temp_module = std::env::temp_dir().join("test_module.py");
        std::fs::write(&temp_module, "def main(): return {'changed': True}").unwrap();

        let mut args = HashMap::new();
        args.insert("name".to_string(), serde_json::json!("test"));

        let bundle = executor.bundle(&temp_module, &args).unwrap();
        assert!(bundle.contains("MODULE_B64"));
        assert!(bundle.contains("ARGS_B64"));

        std::fs::remove_file(&temp_module).ok();
    }

    #[test]
    fn test_parse_success_result() {
        let executor = PythonModuleExecutor::new();
        let result = CommandResult {
            exit_code: 0,
            stdout: r#"{"changed": true, "msg": "Package installed"}"#.to_string(),
            stderr: String::new(),
            success: true,
        };

        let output = executor.parse_result(&result, "apt").unwrap();
        assert!(output.changed);
        assert!(output.msg.contains("Package installed"));
    }

    #[test]
    fn test_parse_failed_result() {
        let executor = PythonModuleExecutor::new();
        let result = CommandResult {
            exit_code: 0,
            stdout: r#"{"failed": true, "msg": "Permission denied"}"#.to_string(),
            stderr: String::new(),
            success: true,
        };

        let err = executor.parse_result(&result, "apt").unwrap_err();
        assert!(matches!(err, ModuleError::ExecutionFailed(_)));
    }
}
