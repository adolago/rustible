//! Yum module - Package management for RHEL/CentOS/Fedora systems
//!
//! This module manages packages using the yum package manager.
//! It supports installing, removing, and upgrading packages.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParallelizationHint, ParamExt,
};
use crate::connection::ExecuteOptions;
use std::collections::HashMap;

/// Desired state for a package
#[derive(Debug, Clone, PartialEq)]
pub enum YumState {
    Present,
    Absent,
    Latest,
}

impl YumState {
    fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "present" | "installed" => Ok(YumState::Present),
            "absent" | "removed" => Ok(YumState::Absent),
            "latest" => Ok(YumState::Latest),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: present, absent, latest",
                s
            ))),
        }
    }
}

/// Module for yum package management
pub struct YumModule;

impl YumModule {
    /// Check if a package is installed via remote connection
    async fn is_package_installed_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        package: &str,
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<bool> {
        let cmd = format!("rpm -q {}", package);
        match conn.execute(&cmd, options).await {
            Ok(result) => Ok(result.success),
            Err(_) => Ok(false),
        }
    }

    /// Get installed package version via remote connection
    async fn get_installed_version_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        package: &str,
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<Option<String>> {
        let cmd = format!("rpm -q --qf '%{{VERSION}}-%{{RELEASE}}' {}", package);
        match conn.execute(&cmd, options).await {
            Ok(result) if result.success => {
                let version = result.stdout.trim().to_string();
                if version.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(version))
                }
            }
            _ => Ok(None),
        }
    }

    /// Execute yum command via remote connection
    async fn run_yum_command_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        args: &[&str],
        packages: &[String],
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<(bool, String, String)> {
        let mut cmd_parts = vec!["yum"];
        cmd_parts.extend(args);
        cmd_parts.extend(packages.iter().map(|s| s.as_str()));

        let cmd = cmd_parts.join(" ");

        let result = conn
            .execute(&cmd, options)
            .await
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to execute yum: {}", e)))?;

        Ok((result.success, result.stdout, result.stderr))
    }

    /// Update yum cache via remote connection
    async fn update_cache_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<()> {
        let cmd = "yum makecache";
        let result = conn
            .execute(cmd, options)
            .await
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to update cache: {}", e)))?;

        if result.success {
            Ok(())
        } else {
            Err(ModuleError::ExecutionFailed(format!(
                "Failed to update cache: {}",
                result.stderr
            )))
        }
    }

    /// Build execution options with become/sudo if needed
    fn build_exec_options(context: &ModuleContext) -> ExecuteOptions {
        let mut options = ExecuteOptions::new();

        if context.r#become {
            options.escalate = true;
            options.escalate_user = context
                .become_user
                .clone()
                .or_else(|| Some("root".to_string()));
            options.escalate_method = context.become_method.clone();
        }

        if let Some(ref work_dir) = context.work_dir {
            options = options.with_cwd(work_dir);
        }

        options
    }
}

impl Module for YumModule {
    fn name(&self) -> &'static str {
        "yum"
    }

    fn description(&self) -> &'static str {
        "Manage packages with the yum package manager"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::RemoteCommand
    }

    fn parallelization_hint(&self) -> ParallelizationHint {
        // Yum uses locks - only one can run per host at a time
        ParallelizationHint::HostExclusive
    }

    fn required_params(&self) -> &[&'static str] {
        &["name"]
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        // Get packages - can be a single package or a list
        let packages: Vec<String> = if let Some(names) = params.get_vec_string("name")? {
            names
        } else {
            vec![params.get_string_required("name")?]
        };

        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());
        let state = YumState::from_str(&state_str)?;

        let update_cache = params.get_bool_or("update_cache", false);
        let disable_gpg_check = params.get_bool_or("disable_gpg_check", false);
        let enablerepo = params.get_string("enablerepo")?;
        let disablerepo = params.get_string("disablerepo")?;

        // Get connection from context
        let conn = context.connection.as_ref().ok_or_else(|| {
            ModuleError::ExecutionFailed(
                "No connection available in context. YUM module requires a remote connection."
                    .to_string(),
            )
        })?;

        // Build execution options with become/sudo
        let exec_options = Self::build_exec_options(context);

        // Use tokio runtime to execute async operations
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // Update cache if requested
                if update_cache && !context.check_mode {
                    Self::update_cache_remote(conn.as_ref(), Some(exec_options.clone())).await?;
                }

                // Track what we'll do
                let mut to_install: Vec<String> = Vec::new();
                let mut to_remove: Vec<String> = Vec::new();
                let mut already_ok: Vec<String> = Vec::new();

                // Check current state of packages
                for package in &packages {
                    let is_installed = Self::is_package_installed_remote(
                        conn.as_ref(),
                        package,
                        Some(exec_options.clone()),
                    )
                    .await?;

                    match state {
                        YumState::Present => {
                            if is_installed {
                                already_ok.push(package.clone());
                            } else {
                                to_install.push(package.clone());
                            }
                        }
                        YumState::Absent => {
                            if is_installed {
                                to_remove.push(package.clone());
                            } else {
                                already_ok.push(package.clone());
                            }
                        }
                        YumState::Latest => {
                            // For 'latest', we always try to install/upgrade
                            to_install.push(package.clone());
                        }
                    }
                }

                // Check mode - return what would happen
                if context.check_mode {
                    if to_install.is_empty() && to_remove.is_empty() {
                        return Ok(ModuleOutput::ok(format!(
                            "All packages already in desired state: {}",
                            already_ok.join(", ")
                        )));
                    }

                    let mut msg = String::new();
                    if !to_install.is_empty() {
                        msg.push_str(&format!("Would install: {}. ", to_install.join(", ")));
                    }
                    if !to_remove.is_empty() {
                        msg.push_str(&format!("Would remove: {}. ", to_remove.join(", ")));
                    }

                    return Ok(ModuleOutput::changed(msg.trim().to_string()));
                }

                // Perform the actual operations
                let mut changed = false;
                let mut results: HashMap<String, String> = HashMap::new();

                if !to_install.is_empty() {
                    let mut install_args = vec!["install", "-y"];

                    if disable_gpg_check {
                        install_args.push("--nogpgcheck");
                    }

                    if let Some(ref repo) = enablerepo {
                        install_args.push("--enablerepo");
                        install_args.push(repo);
                    }

                    if let Some(ref repo) = disablerepo {
                        install_args.push("--disablerepo");
                        install_args.push(repo);
                    }

                    let (success, stdout, stderr) = Self::run_yum_command_remote(
                        conn.as_ref(),
                        &install_args,
                        &to_install,
                        Some(exec_options.clone()),
                    )
                    .await?;

                    if !success {
                        return Err(ModuleError::ExecutionFailed(format!(
                            "Failed to install packages: {}",
                            if stderr.is_empty() { stdout } else { stderr }
                        )));
                    }

                    changed = true;
                    for pkg in &to_install {
                        results.insert(pkg.clone(), "installed".to_string());
                    }
                }

                if !to_remove.is_empty() {
                    let remove_args = vec!["remove", "-y"];
                    let (success, stdout, stderr) = Self::run_yum_command_remote(
                        conn.as_ref(),
                        &remove_args,
                        &to_remove,
                        Some(exec_options.clone()),
                    )
                    .await?;

                    if !success {
                        return Err(ModuleError::ExecutionFailed(format!(
                            "Failed to remove packages: {}",
                            if stderr.is_empty() { stdout } else { stderr }
                        )));
                    }

                    changed = true;
                    for pkg in &to_remove {
                        results.insert(pkg.clone(), "removed".to_string());
                    }
                }

                for pkg in &already_ok {
                    results.insert(pkg.clone(), "ok".to_string());
                }

                if changed {
                    let mut msg = String::new();
                    if !to_install.is_empty() {
                        msg.push_str(&format!("Installed: {}. ", to_install.join(", ")));
                    }
                    if !to_remove.is_empty() {
                        msg.push_str(&format!("Removed: {}. ", to_remove.join(", ")));
                    }

                    Ok(ModuleOutput::changed(msg.trim().to_string())
                        .with_data("results", serde_json::json!(results)))
                } else {
                    Ok(
                        ModuleOutput::ok("All packages already in desired state".to_string())
                            .with_data("results", serde_json::json!(results)),
                    )
                }
            })
        });

        result
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    fn diff(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let packages: Vec<String> = if let Some(names) = params.get_vec_string("name")? {
            names
        } else {
            vec![params.get_string_required("name")?]
        };

        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());
        let state = YumState::from_str(&state_str)?;

        // Get connection from context
        let conn = match context.connection.as_ref() {
            Some(c) => c,
            None => {
                // No connection available, return basic diff without checking remote state
                let mut before_lines = Vec::new();
                let mut after_lines = Vec::new();

                for package in &packages {
                    match state {
                        YumState::Present | YumState::Latest => {
                            before_lines.push(format!("{}: (unknown)", package));
                            after_lines.push(format!("{}: (will be installed/updated)", package));
                        }
                        YumState::Absent => {
                            before_lines.push(format!("{}: (unknown)", package));
                            after_lines.push(format!("{}: (will be removed)", package));
                        }
                    }
                }

                return Ok(Some(Diff::new(
                    before_lines.join("\n"),
                    after_lines.join("\n"),
                )));
            }
        };

        let exec_options = Self::build_exec_options(context);

        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut before_lines = Vec::new();
                let mut after_lines = Vec::new();

                for package in &packages {
                    let is_installed = Self::is_package_installed_remote(
                        conn.as_ref(),
                        package,
                        Some(exec_options.clone()),
                    )
                    .await?;

                    let version = Self::get_installed_version_remote(
                        conn.as_ref(),
                        package,
                        Some(exec_options.clone()),
                    )
                    .await?
                    .unwrap_or_default();

                    match state {
                        YumState::Present | YumState::Latest => {
                            if is_installed {
                                before_lines.push(format!("{}: {}", package, version));
                                after_lines.push(format!("{}: {}", package, version));
                            } else {
                                before_lines.push(format!("{}: (not installed)", package));
                                after_lines.push(format!("{}: (will be installed)", package));
                            }
                        }
                        YumState::Absent => {
                            if is_installed {
                                before_lines.push(format!("{}: {}", package, version));
                                after_lines.push(format!("{}: (will be removed)", package));
                            } else {
                                before_lines.push(format!("{}: (not installed)", package));
                                after_lines.push(format!("{}: (not installed)", package));
                            }
                        }
                    }
                }

                Ok(Some(Diff::new(
                    before_lines.join("\n"),
                    after_lines.join("\n"),
                )))
            })
        });

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yum_state_from_str() {
        assert_eq!(YumState::from_str("present").unwrap(), YumState::Present);
        assert_eq!(YumState::from_str("installed").unwrap(), YumState::Present);
        assert_eq!(YumState::from_str("absent").unwrap(), YumState::Absent);
        assert_eq!(YumState::from_str("removed").unwrap(), YumState::Absent);
        assert_eq!(YumState::from_str("latest").unwrap(), YumState::Latest);
        assert!(YumState::from_str("invalid").is_err());
    }

    #[test]
    fn test_yum_module_name() {
        let module = YumModule;
        assert_eq!(module.name(), "yum");
    }

    #[test]
    fn test_yum_module_classification() {
        let module = YumModule;
        assert_eq!(module.classification(), ModuleClassification::RemoteCommand);
    }

    #[test]
    fn test_yum_module_parallelization() {
        let module = YumModule;
        assert_eq!(
            module.parallelization_hint(),
            ParallelizationHint::HostExclusive
        );
    }

    #[test]
    fn test_yum_required_params() {
        let module = YumModule;
        assert_eq!(module.required_params(), &["name"]);
    }
}
