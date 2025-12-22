//! Shell module - Execute shell commands
//!
//! This module executes commands through a shell, enabling shell features
//! like pipes, redirects, environment variable expansion, etc.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use std::process::Command;

/// Module for executing shell commands
pub struct ShellModule;

impl ShellModule {
    fn get_shell(&self, params: &ModuleParams) -> ModuleResult<(String, String)> {
        // Get shell executable
        let executable = params
            .get_string("executable")?
            .unwrap_or_else(|| "/bin/sh".to_string());

        // Different shells have different syntax for running commands
        let flag = if executable.ends_with("fish") {
            "-c".to_string()
        } else if executable.ends_with("cmd.exe") || executable.ends_with("cmd") {
            "/c".to_string()
        } else {
            "-c".to_string()
        };

        Ok((executable, flag))
    }

    fn check_creates_removes(&self, params: &ModuleParams) -> ModuleResult<Option<ModuleOutput>> {
        // Check 'creates' - skip if file exists
        if let Some(creates) = params.get_string("creates")? {
            if std::path::Path::new(&creates).exists() {
                return Ok(Some(ModuleOutput::ok(format!(
                    "Skipped, '{}' exists",
                    creates
                ))));
            }
        }

        // Check 'removes' - skip if file doesn't exist
        if let Some(removes) = params.get_string("removes")? {
            if !std::path::Path::new(&removes).exists() {
                return Ok(Some(ModuleOutput::ok(format!(
                    "Skipped, '{}' does not exist",
                    removes
                ))));
            }
        }

        Ok(None)
    }
}

impl Module for ShellModule {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Execute shell commands with full shell features"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::RemoteCommand
    }

    fn required_params(&self) -> &[&'static str] {
        &["cmd"]
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        // Check creates/removes conditions
        if let Some(output) = self.check_creates_removes(params)? {
            return Ok(output);
        }

        let cmd = params.get_string_required("cmd")?;

        // In check mode, return what would happen
        if context.check_mode {
            return Ok(ModuleOutput::changed(format!(
                "Would execute shell command: {}",
                cmd
            )));
        }

        let (shell, flag) = self.get_shell(params)?;

        let mut command = Command::new(&shell);
        command.arg(&flag).arg(&cmd);

        // Set working directory
        if let Some(chdir) = params.get_string("chdir")? {
            command.current_dir(&chdir);
        } else if let Some(ref work_dir) = context.work_dir {
            command.current_dir(work_dir);
        }

        // Set environment variables
        if let Some(serde_json::Value::Object(env)) = params.get("env") {
            for (key, value) in env {
                if let serde_json::Value::String(v) = value {
                    command.env(key, v);
                }
            }
        }

        // Handle stdin
        if let Some(stdin_data) = params.get_string("stdin")? {
            use std::io::Write;
            use std::process::Stdio;

            command.stdin(Stdio::piped());
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());

            let mut child = command.spawn().map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to spawn shell: {}", e))
            })?;

            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(stdin_data.as_bytes()).map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to write to stdin: {}", e))
                })?;
            }

            let output = child.wait_with_output().map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to wait for command: {}", e))
            })?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let rc = output.status.code().unwrap_or(-1);

            if output.status.success() {
                return Ok(
                    ModuleOutput::changed(format!("Shell command executed successfully"))
                        .with_command_output(Some(stdout), Some(stderr), Some(rc)),
                );
            } else {
                return Err(ModuleError::CommandFailed {
                    code: rc,
                    message: if stderr.is_empty() { stdout } else { stderr },
                });
            }
        }

        // Execute the command
        let output = command.output().map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to execute shell command: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let rc = output.status.code().unwrap_or(-1);

        // Check if command succeeded
        if output.status.success() {
            let mut result =
                ModuleOutput::changed("Shell command executed successfully".to_string())
                    .with_command_output(Some(stdout), Some(stderr.clone()), Some(rc));

            let warn_on_stderr = params.get_bool_or("warn", true);
            if warn_on_stderr && !stderr.is_empty() {
                result
                    .data
                    .insert("warnings".to_string(), serde_json::json!([stderr]));
            }

            Ok(result)
        } else {
            Err(ModuleError::CommandFailed {
                code: rc,
                message: if stderr.is_empty() { stdout } else { stderr },
            })
        }
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        // Check creates/removes conditions
        if let Some(output) = self.check_creates_removes(params)? {
            return Ok(output);
        }

        let cmd = params.get_string_required("cmd")?;
        let _ = context;

        Ok(
            ModuleOutput::changed(format!("Would execute shell command: {}", cmd))
                .with_diff(Diff::new("(none)", format!("Execute: {}", cmd))),
        )
    }

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let cmd = params.get_string_required("cmd")?;
        Ok(Some(Diff::new("(none)", format!("Execute: {}", cmd))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_shell_echo() {
        let module = ShellModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("echo hello"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.stdout.as_ref().unwrap().contains("hello"));
    }

    #[test]
    fn test_shell_pipe() {
        let module = ShellModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "cmd".to_string(),
            serde_json::json!("echo 'hello world' | grep hello"),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.stdout.as_ref().unwrap().contains("hello"));
    }

    #[test]
    fn test_shell_env_expansion() {
        let module = ShellModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("echo $HOME"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        // HOME should be expanded to a path
        assert!(!result.stdout.as_ref().unwrap().contains("$HOME"));
    }

    #[test]
    fn test_shell_check_mode() {
        let module = ShellModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("rm -rf /"));

        let context = ModuleContext::default().with_check_mode(true);
        let result = module.check(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.msg.contains("Would execute"));
    }

    #[test]
    fn test_shell_creates_exists() {
        let module = ShellModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("echo hello"));
        params.insert("creates".to_string(), serde_json::json!("/"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
        assert!(result.msg.contains("Skipped"));
    }

    #[test]
    fn test_shell_with_stdin() {
        let module = ShellModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("cat"));
        params.insert("stdin".to_string(), serde_json::json!("hello from stdin"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.stdout.as_ref().unwrap().contains("hello from stdin"));
    }
}
