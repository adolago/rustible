//! Git module - Git repository management
//!
//! This module manages git repositories including cloning, updating,
//! and checking out specific versions or branches.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use std::path::Path;
use std::process::Command;

/// Module for git repository management
pub struct GitModule;

impl GitModule {
    /// Check if git is installed
    fn check_git_installed() -> ModuleResult<bool> {
        let output = Command::new("git")
            .arg("--version")
            .output()
            .map_err(|_| ModuleError::ExecutionFailed("git is not installed".to_string()))?;
        Ok(output.status.success())
    }

    /// Check if a directory is a git repository
    fn is_git_repo(dest: &str) -> bool {
        Path::new(&format!("{}/.git", dest)).exists()
    }

    /// Get the current HEAD commit hash
    fn get_current_version(dest: &str) -> ModuleResult<Option<String>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(dest)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to get current version: {}", e))
            })?;

        if output.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    /// Get the remote URL of the repository
    fn get_remote_url(dest: &str) -> ModuleResult<Option<String>> {
        let output = Command::new("git")
            .arg("-C")
            .arg(dest)
            .arg("config")
            .arg("--get")
            .arg("remote.origin.url")
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to get remote URL: {}", e))
            })?;

        if output.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    /// Clone a git repository
    fn clone_repo(
        repo: &str,
        dest: &str,
        version: Option<&str>,
        depth: Option<u32>,
        _context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let mut command = Command::new("git");
        command.arg("clone");

        if let Some(d) = depth {
            command.arg("--depth").arg(d.to_string());
        }

        if let Some(v) = version {
            command.arg("--branch").arg(v);
        }

        command.arg(repo).arg(dest);

        let output = command.output().map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to clone repository: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ModuleError::CommandFailed {
                code: output.status.code().unwrap_or(-1),
                message: stderr.to_string(),
            });
        }

        Ok(ModuleOutput::changed(format!(
            "Cloned repository '{}' to '{}'",
            repo, dest
        )))
    }

    /// Update (pull) a git repository
    fn update_repo(
        dest: &str,
        version: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String)> {
        // Get current version before update
        let before_version =
            Self::get_current_version(dest)?.unwrap_or_else(|| "unknown".to_string());

        if context.check_mode {
            return Ok((false, before_version));
        }

        // Fetch updates
        let fetch_output = Command::new("git")
            .arg("-C")
            .arg(dest)
            .arg("fetch")
            .arg("origin")
            .output()
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to fetch updates: {}", e)))?;

        if !fetch_output.status.success() {
            return Err(ModuleError::CommandFailed {
                code: fetch_output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&fetch_output.stderr).to_string(),
            });
        }

        // Checkout the specified version or default branch
        let checkout_target = version.unwrap_or("origin/HEAD");
        let checkout_output = Command::new("git")
            .arg("-C")
            .arg(dest)
            .arg("checkout")
            .arg(checkout_target)
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to checkout version: {}", e))
            })?;

        if !checkout_output.status.success() {
            return Err(ModuleError::CommandFailed {
                code: checkout_output.status.code().unwrap_or(-1),
                message: String::from_utf8_lossy(&checkout_output.stderr).to_string(),
            });
        }

        // If on a branch, pull the latest changes
        if version.is_none() || !version.unwrap().starts_with("v") {
            let _ = Command::new("git")
                .arg("-C")
                .arg(dest)
                .arg("pull")
                .arg("--ff-only")
                .output();
        }

        // Get version after update
        let after_version =
            Self::get_current_version(dest)?.unwrap_or_else(|| "unknown".to_string());

        let changed = before_version != after_version;
        Ok((changed, after_version))
    }
}

impl Module for GitModule {
    fn name(&self) -> &'static str {
        "git"
    }

    fn description(&self) -> &'static str {
        "Manage git repositories - clone, update, and checkout versions"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::RemoteCommand
    }

    fn required_params(&self) -> &[&'static str] {
        &["repo", "dest"]
    }

    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        // Validate required parameters
        if params.get("repo").is_none() {
            return Err(ModuleError::MissingParameter("repo".to_string()));
        }
        if params.get("dest").is_none() {
            return Err(ModuleError::MissingParameter("dest".to_string()));
        }

        // Validate depth if provided
        if let Some(depth) = params.get_u32("depth")? {
            if depth == 0 {
                return Err(ModuleError::InvalidParameter(
                    "depth must be greater than 0".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        // Extract parameters
        let repo = params.get_string_required("repo")?;
        let dest = params.get_string_required("dest")?;
        let version = params.get_string("version")?;
        let depth = params.get_u32("depth")?;
        let update = params.get_bool_or("update", true);

        // Check if git is installed
        if !Self::check_git_installed()? {
            return Err(ModuleError::ExecutionFailed(
                "git is not installed on the system".to_string(),
            ));
        }

        // Check if destination exists and is a git repo
        let is_repo = Self::is_git_repo(&dest);

        if !is_repo {
            // Repository doesn't exist, clone it
            if context.check_mode {
                return Ok(ModuleOutput::changed(format!(
                    "Would clone repository '{}' to '{}'",
                    repo, dest
                ))
                .with_diff(Diff::new("repository absent", format!("clone {}", repo))));
            }

            return Self::clone_repo(&repo, &dest, version.as_deref(), depth, context);
        }

        // Repository exists - check if it's the same repo
        let current_remote = Self::get_remote_url(&dest)?;
        if let Some(current) = current_remote {
            if current != repo {
                return Err(ModuleError::ExecutionFailed(format!(
                    "Destination '{}' is a git repository for '{}', not '{}'",
                    dest, current, repo
                )));
            }
        } else {
            return Err(ModuleError::ExecutionFailed(format!(
                "Destination '{}' exists but is not a valid git repository",
                dest
            )));
        }

        // Repository exists and is correct, update if requested
        if update {
            let (changed, new_version) = Self::update_repo(&dest, version.as_deref(), context)?;

            if changed {
                Ok(ModuleOutput::changed(format!(
                    "Updated repository to version '{}'",
                    new_version
                ))
                .with_data("version", serde_json::json!(new_version)))
            } else {
                Ok(
                    ModuleOutput::ok(format!("Repository already at version '{}'", new_version))
                        .with_data("version", serde_json::json!(new_version)),
                )
            }
        } else {
            // Just check current version
            let current_version =
                Self::get_current_version(&dest)?.unwrap_or_else(|| "unknown".to_string());

            Ok(ModuleOutput::ok(format!(
                "Repository exists at version '{}'",
                current_version
            ))
            .with_data("version", serde_json::json!(current_version)))
        }
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let dest = params.get_string_required("dest")?;
        let repo = params.get_string_required("repo")?;

        let is_repo = Self::is_git_repo(&dest);

        if !is_repo {
            Ok(Some(Diff::new(
                "repository absent",
                format!("clone {}", repo),
            )))
        } else {
            let current_version =
                Self::get_current_version(&dest)?.unwrap_or_else(|| "unknown".to_string());
            let version = params
                .get_string("version")?
                .unwrap_or_else(|| "latest".to_string());

            Ok(Some(Diff::new(
                format!("version: {}", current_version),
                format!("version: {}", version),
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_git_module_validate_params() {
        let module = GitModule;

        // Missing repo
        let mut params: ModuleParams = HashMap::new();
        params.insert("dest".to_string(), serde_json::json!("/tmp/test"));
        assert!(module.validate_params(&params).is_err());

        // Missing dest
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "repo".to_string(),
            serde_json::json!("https://github.com/test/repo"),
        );
        assert!(module.validate_params(&params).is_err());

        // Valid params
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "repo".to_string(),
            serde_json::json!("https://github.com/test/repo"),
        );
        params.insert("dest".to_string(), serde_json::json!("/tmp/test"));
        assert!(module.validate_params(&params).is_ok());
    }

    #[test]
    fn test_git_module_validate_depth() {
        let module = GitModule;

        // Invalid depth (0)
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "repo".to_string(),
            serde_json::json!("https://github.com/test/repo"),
        );
        params.insert("dest".to_string(), serde_json::json!("/tmp/test"));
        params.insert("depth".to_string(), serde_json::json!(0));
        assert!(module.validate_params(&params).is_err());

        // Valid depth
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "repo".to_string(),
            serde_json::json!("https://github.com/test/repo"),
        );
        params.insert("dest".to_string(), serde_json::json!("/tmp/test"));
        params.insert("depth".to_string(), serde_json::json!(1));
        assert!(module.validate_params(&params).is_ok());
    }

    #[test]
    fn test_git_module_check_mode() {
        let module = GitModule;
        let temp = TempDir::new().unwrap();
        let dest_path = temp.path().join("test-repo");

        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "repo".to_string(),
            serde_json::json!("https://github.com/test/repo"),
        );
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest_path.to_str().unwrap()),
        );

        let context = ModuleContext::default().with_check_mode(true);
        let result = module.check(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.msg.contains("Would clone"));
        assert!(!dest_path.exists()); // Should not be created in check mode
    }

    #[test]
    fn test_git_module_name_and_description() {
        let module = GitModule;
        assert_eq!(module.name(), "git");
        assert!(!module.description().is_empty());
    }

    #[test]
    fn test_git_module_required_params() {
        let module = GitModule;
        let required = module.required_params();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&"repo"));
        assert!(required.contains(&"dest"));
    }
}
