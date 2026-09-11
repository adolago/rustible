//! Replace module — substitute all matches of a pattern in a file.
//!
//! Ansible's `replace` edits every occurrence of a regular expression, unlike
//! `lineinfile`, which manages a single line. Both local and remote targets are
//! supported: a remote file is downloaded, edited and uploaded, so the same
//! substitution runs either way.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use crate::connection::{Connection, TransferOptions};
use crate::utils::{get_regex, secure_write_file};
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Module for replacing text in files
pub struct ReplaceModule;

/// What a substitution did to a file's content.
struct Substitution {
    content: String,
    changed: bool,
}

impl ReplaceModule {
    /// Apply the substitution to the given content.
    fn substitute(content: &str, pattern: &str, replacement: &str) -> ModuleResult<Substitution> {
        let regex = get_regex(pattern)
            .map_err(|error| ModuleError::InvalidParameter(format!("Invalid regexp: {}", error)))?;
        let replaced = regex.replace_all(content, replacement).into_owned();
        Ok(Substitution {
            changed: replaced != content,
            content: replaced,
        })
    }

    /// Read the parameters shared by both execution paths.
    fn params(
        params: &ModuleParams,
    ) -> ModuleResult<(String, String, String, bool, String, Option<u32>)> {
        let path = params.get_string_required("path")?;
        let pattern = params.get_string_required("regexp")?;
        let replacement = params.get_string("replace")?.unwrap_or_default();
        let backup = params.get_bool_or("backup", false);
        let backup_suffix = params
            .get_string("backup_suffix")?
            .unwrap_or_else(|| "~".to_string());
        let mode = params.get_u32("mode")?;
        Ok((path, pattern, replacement, backup, backup_suffix, mode))
    }

    /// Edit a file on the control node.
    fn execute_local(params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let (path, pattern, replacement, backup, backup_suffix, mode) = Self::params(params)?;
        let file = Path::new(&path);

        if !file.exists() {
            return Err(ModuleError::ExecutionFailed(format!(
                "File '{}' does not exist",
                path
            )));
        }

        let content = fs::read_to_string(file)?;
        let result = Self::substitute(&content, &pattern, &replacement)?;

        if !result.changed {
            return Ok(ModuleOutput::ok(format!(
                "No occurrence of the pattern in '{}'",
                path
            )));
        }

        if context.check_mode {
            let mut output = ModuleOutput::changed(format!("Would replace in '{}'", path));
            if context.diff_mode {
                output = output.with_diff(Diff::new(content, result.content));
            }
            return Ok(output);
        }

        if backup {
            let backup_path = format!("{}{}", path, backup_suffix);
            fs::copy(file, &backup_path)?;
        }

        secure_write_file(file, &result.content, false, mode)?;

        let mut output = ModuleOutput::changed(format!("Replaced in '{}'", path));
        if context.diff_mode {
            output = output.with_diff(Diff::new(content, result.content));
        }
        if backup {
            output = output.with_data(
                "backup_file",
                serde_json::json!(format!("{}{}", path, backup_suffix)),
            );
        }
        Ok(output)
    }

    /// Edit a file on a remote target through the connection.
    fn execute_remote(
        params: &ModuleParams,
        context: &ModuleContext,
        connection: &Arc<dyn Connection + Send + Sync>,
    ) -> ModuleResult<ModuleOutput> {
        let (path, pattern, replacement, backup, backup_suffix, mode) = Self::params(params)?;
        let connection = connection.clone();
        let check_mode = context.check_mode;
        let diff_mode = context.diff_mode;

        super::block_on_module_future(async move {
            let remote_path = Path::new(&path);
            if !connection.path_exists(remote_path).await.unwrap_or(false) {
                return Err(ModuleError::ExecutionFailed(format!(
                    "File '{}' does not exist",
                    path
                )));
            }

            let bytes = connection
                .download_content(remote_path)
                .await
                .map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to download file: {}", e))
                })?;
            let content = String::from_utf8_lossy(&bytes).into_owned();
            let result = Self::substitute(&content, &pattern, &replacement)?;

            if !result.changed {
                return Ok(ModuleOutput::ok(format!(
                    "No occurrence of the pattern in '{}'",
                    path
                )));
            }

            if check_mode {
                let mut output = ModuleOutput::changed(format!("Would replace in '{}'", path));
                if diff_mode {
                    output = output.with_diff(Diff::new(content, result.content));
                }
                return Ok(output);
            }

            if backup {
                let backup_path = format!("{}{}", path, backup_suffix);
                connection
                    .upload_content(&bytes, Path::new(&backup_path), None)
                    .await
                    .map_err(|e| {
                        ModuleError::ExecutionFailed(format!("Failed to create backup: {}", e))
                    })?;
            }

            let mut transfer = TransferOptions::new();
            if let Some(mode) = mode {
                transfer = transfer.with_mode(mode);
            }
            connection
                .upload_content(result.content.as_bytes(), remote_path, Some(transfer))
                .await
                .map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to upload file: {}", e))
                })?;

            let mut output = ModuleOutput::changed(format!("Replaced in '{}'", path));
            if diff_mode {
                output = output.with_diff(Diff::new(content, result.content));
            }
            if backup {
                output = output.with_data(
                    "backup_file",
                    serde_json::json!(format!("{}{}", path, backup_suffix)),
                );
            }
            Ok(output)
        })?
    }
}

impl Module for ReplaceModule {
    fn name(&self) -> &'static str {
        "replace"
    }

    fn description(&self) -> &'static str {
        "Replace all instances of a pattern in a file"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::NativeTransport
    }

    fn required_params(&self) -> &[&'static str] {
        &["path", "regexp"]
    }

    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        let pattern = params.get_string_required("regexp")?;
        get_regex(&pattern)
            .map_err(|error| ModuleError::InvalidParameter(format!("Invalid regexp: {}", error)))?;
        Ok(())
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        match &context.connection {
            Some(connection) if !connection.is_local() => {
                Self::execute_remote(params, context, connection)
            }
            _ => Self::execute_local(params, context),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn params(pairs: &[(&str, serde_json::Value)]) -> ModuleParams {
        let mut map: HashMap<String, serde_json::Value> = HashMap::new();
        for (key, value) in pairs {
            map.insert((*key).to_string(), value.clone());
        }
        map
    }

    #[test]
    fn test_substitute_replaces_every_match() {
        let result = ReplaceModule::substitute("a1 b2 c3", r"\d", "#").unwrap();
        assert_eq!(result.content, "a# b# c#");
        assert!(result.changed);
    }

    #[test]
    fn test_substitute_supports_capture_groups() {
        let result = ReplaceModule::substitute("port = 80", r"port = (\d+)", "port = 8$1").unwrap();
        assert_eq!(result.content, "port = 880");
    }

    #[test]
    fn test_substitute_reports_no_change() {
        let result = ReplaceModule::substitute("nothing here", r"\d", "#").unwrap();
        assert!(!result.changed);
    }

    #[test]
    fn test_replaces_in_a_local_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.conf");
        fs::write(&path, "port = 80\nhost = example\n").unwrap();

        let module = ReplaceModule;
        let output = module
            .execute(
                &params(&[
                    ("path", serde_json::json!(path.to_str().unwrap())),
                    ("regexp", serde_json::json!(r"port = \d+")),
                    ("replace", serde_json::json!("port = 8080")),
                ]),
                &ModuleContext::default(),
            )
            .unwrap();

        assert!(output.changed);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "port = 8080\nhost = example\n"
        );
    }

    #[test]
    fn test_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.conf");
        fs::write(&path, "port = 8080\n").unwrap();

        let module = ReplaceModule;
        let output = module
            .execute(
                &params(&[
                    ("path", serde_json::json!(path.to_str().unwrap())),
                    ("regexp", serde_json::json!(r"port = 80\b")),
                    ("replace", serde_json::json!("port = 8080")),
                ]),
                &ModuleContext::default(),
            )
            .unwrap();

        assert!(!output.changed, "a pattern that matches nothing is a no-op");
    }

    #[test]
    fn test_check_mode_leaves_the_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.conf");
        fs::write(&path, "port = 80\n").unwrap();

        let module = ReplaceModule;
        let context = ModuleContext {
            check_mode: true,
            ..Default::default()
        };
        let output = module
            .execute(
                &params(&[
                    ("path", serde_json::json!(path.to_str().unwrap())),
                    ("regexp", serde_json::json!(r"80")),
                    ("replace", serde_json::json!("8080")),
                ]),
                &context,
            )
            .unwrap();

        assert!(output.changed);
        assert_eq!(fs::read_to_string(&path).unwrap(), "port = 80\n");
    }

    #[test]
    fn test_missing_file_is_an_error() {
        let module = ReplaceModule;
        let error = module
            .execute(
                &params(&[
                    ("path", serde_json::json!("/nonexistent/rustible-replace")),
                    ("regexp", serde_json::json!("x")),
                    ("replace", serde_json::json!("y")),
                ]),
                &ModuleContext::default(),
            )
            .unwrap_err();
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn test_invalid_regexp_is_reported() {
        let module = ReplaceModule;
        let error = module
            .validate_params(&params(&[
                ("path", serde_json::json!("/tmp/x")),
                ("regexp", serde_json::json!("[")),
            ]))
            .unwrap_err();
        assert!(error.to_string().contains("Invalid regexp"));
    }
}
