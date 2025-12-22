//! Command module - Execute arbitrary commands
//!
//! This module executes commands directly without going through a shell.
//! For shell commands (pipes, redirects, etc.), use the shell module.

use super::{
    Diff, Module, ModuleContext, ModuleError, ModuleOutput, ModuleParams, ModuleResult, ParamExt,
};
use std::process::Command;

/// Module for executing commands directly
pub struct CommandModule;

impl CommandModule {
    fn build_command(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<Command> {
        let cmd = params.get_string_required("cmd")?;
        let argv = params.get_vec_string("argv")?;

        let mut command = if let Some(argv) = argv {
            // If argv is provided, use the first element as the command
            if argv.is_empty() {
                return Err(ModuleError::InvalidParameter(
                    "argv cannot be empty".to_string(),
                ));
            }
            let mut cmd = Command::new(&argv[0]);
            if argv.len() > 1 {
                cmd.args(&argv[1..]);
            }
            cmd
        } else {
            // Parse the command string into arguments
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.is_empty() {
                return Err(ModuleError::InvalidParameter(
                    "cmd cannot be empty".to_string(),
                ));
            }
            let mut cmd = Command::new(parts[0]);
            if parts.len() > 1 {
                cmd.args(&parts[1..]);
            }
            cmd
        };

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

        Ok(command)
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

impl Module for CommandModule {
    fn name(&self) -> &'static str {
        "command"
    }

    fn description(&self) -> &'static str {
        "Execute commands without going through a shell"
    }

    fn required_params(&self) -> &[&'static str] {
        &["cmd"]
    }

    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        // Must have either cmd or argv
        if params.get("cmd").is_none() && params.get("argv").is_none() {
            return Err(ModuleError::MissingParameter(
                "Either 'cmd' or 'argv' must be provided".to_string(),
            ));
        }
        Ok(())
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

        // In check mode, return what would happen
        if context.check_mode {
            let cmd = params
                .get_string("cmd")?
                .unwrap_or_else(|| "command".to_string());
            return Ok(ModuleOutput::changed(format!("Would execute: {}", cmd)));
        }

        let mut command = self.build_command(params, context)?;
        let cmd_display = params
            .get_string("cmd")?
            .unwrap_or_else(|| "command".to_string());

        // Execute the command
        let output = command.output().map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to execute '{}': {}", cmd_display, e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let rc = output.status.code().unwrap_or(-1);

        // Check if command succeeded
        let warn_on_stderr = params.get_bool_or("warn", true);

        if output.status.success() {
            let mut result =
                ModuleOutput::changed(format!("Command '{}' executed successfully", cmd_display))
                    .with_command_output(Some(stdout), Some(stderr.clone()), Some(rc));

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

        let cmd = params
            .get_string("cmd")?
            .unwrap_or_else(|| "command".to_string());
        let _ = context;

        Ok(ModuleOutput::changed(format!("Would execute: {}", cmd))
            .with_diff(Diff::new("(none)", format!("Execute: {}", cmd))))
    }

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let cmd = params
            .get_string("cmd")?
            .unwrap_or_else(|| "command".to_string());
        Ok(Some(Diff::new("(none)", format!("Execute: {}", cmd))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_command_echo() {
        let module = CommandModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("echo hello"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.stdout.as_ref().unwrap().contains("hello"));
        assert_eq!(result.rc, Some(0));
    }

    #[test]
    fn test_command_with_argv() {
        let module = CommandModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "argv".to_string(),
            serde_json::json!(["echo", "hello", "world"]),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.stdout.as_ref().unwrap().contains("hello world"));
    }

    #[test]
    fn test_command_creates_exists() {
        let module = CommandModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("echo hello"));
        params.insert("creates".to_string(), serde_json::json!("/"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
        assert!(result.msg.contains("Skipped"));
    }

    #[test]
    fn test_command_check_mode() {
        let module = CommandModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("rm -rf /"));

        let context = ModuleContext::default().with_check_mode(true);
        let result = module.check(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.msg.contains("Would execute"));
    }

    #[test]
    fn test_command_fails() {
        let module = CommandModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("cmd".to_string(), serde_json::json!("false"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context);

        assert!(result.is_err());
        if let Err(ModuleError::CommandFailed { code, .. }) = result {
            assert_ne!(code, 0);
        } else {
            panic!("Expected CommandFailed error");
        }
    }
}
