//! DNF module - Fedora/RHEL package management
//!
//! This module manages packages using the DNF package manager on Fedora,
//! RHEL 8+, CentOS 8+, and other RPM-based distributions.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParallelizationHint, ParamExt,
};
use crate::connection::ExecuteOptions;
use std::collections::HashMap;

/// Desired state for a package
#[derive(Debug, Clone, PartialEq)]
pub enum DnfState {
    /// Package should be installed
    Present,
    /// Package should be removed
    Absent,
    /// Package should be at the latest version
    Latest,
}

impl DnfState {
    fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "present" | "installed" => Ok(DnfState::Present),
            "absent" | "removed" => Ok(DnfState::Absent),
            "latest" => Ok(DnfState::Latest),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: present, absent, latest",
                s
            ))),
        }
    }
}

/// Module for DNF package management
pub struct DnfModule;

impl DnfModule {
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

    /// Check if a package is installed via remote connection
    async fn is_package_installed_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        package: &str,
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<bool> {
        let cmd = format!("rpm -q {}", shell_escape(package));
        match conn.execute(&cmd, options).await {
            Ok(result) => Ok(result.success),
            Err(_) => Ok(false),
        }
    }

    /// Get installed package version via remote connection
    async fn get_package_version_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        package: &str,
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<Option<String>> {
        let cmd = format!("rpm -q --qf '%{{VERSION}}-%{{RELEASE}}' {}", shell_escape(package));
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

    /// Run a DNF command via remote connection
    async fn run_dnf_command_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        args: &[&str],
        packages: &[String],
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<(bool, String, String)> {
        let mut cmd_parts: Vec<String> = vec!["dnf".to_string()];
        cmd_parts.extend(args.iter().map(|s| s.to_string()));
        cmd_parts.extend(packages.iter().map(|s| shell_escape(s)));

        let cmd = cmd_parts.join(" ");

        let result = conn
            .execute(&cmd, options)
            .await
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to execute dnf: {}", e)))?;

        Ok((result.success, result.stdout, result.stderr))
    }

    /// Update DNF cache via remote connection
    async fn update_cache_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<()> {
        let cmd = "dnf makecache";
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
}

impl Module for DnfModule {
    fn name(&self) -> &'static str {
        "dnf"
    }

    fn description(&self) -> &'static str {
        "Manage packages with the DNF package manager"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::RemoteCommand
    }

    fn parallelization_hint(&self) -> ParallelizationHint {
        // DNF uses locks - only one can run per host at a time
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
        let state = DnfState::from_str(&state_str)?;
        let update_cache = params.get_bool_or("update_cache", false);

        // Get connection from context
        let conn = context.connection.as_ref().ok_or_else(|| {
            ModuleError::ExecutionFailed(
                "No connection available in context. DNF module requires a remote connection."
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
                        DnfState::Present => {
                            if is_installed {
                                already_ok.push(package.clone());
                            } else {
                                to_install.push(package.clone());
                            }
                        }
                        DnfState::Absent => {
                            if is_installed {
                                to_remove.push(package.clone());
                            } else {
                                already_ok.push(package.clone());
                            }
                        }
                        DnfState::Latest => {
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
                    let install_args = ["install", "-y"];
                    let (success, stdout, stderr) = Self::run_dnf_command_remote(
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
                    let remove_args = ["remove", "-y"];
                    let (success, stdout, stderr) = Self::run_dnf_command_remote(
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
        let state = DnfState::from_str(&state_str)?;

        // Get connection from context
        let conn = match context.connection.as_ref() {
            Some(c) => c,
            None => {
                // No connection available, return basic diff without checking remote state
                let mut before_lines = Vec::new();
                let mut after_lines = Vec::new();

                for package in &packages {
                    match state {
                        DnfState::Present | DnfState::Latest => {
                            before_lines.push(format!("{}: (unknown)", package));
                            after_lines.push(format!("{}: (will be installed/updated)", package));
                        }
                        DnfState::Absent => {
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

                    let version = Self::get_package_version_remote(
                        conn.as_ref(),
                        package,
                        Some(exec_options.clone()),
                    )
                    .await?
                    .unwrap_or_default();

                    match state {
                        DnfState::Present | DnfState::Latest => {
                            if is_installed {
                                before_lines.push(format!("{}: {}", package, version));
                                after_lines.push(format!("{}: {}", package, version));
                            } else {
                                before_lines.push(format!("{}: (not installed)", package));
                                after_lines.push(format!("{}: (will be installed)", package));
                            }
                        }
                        DnfState::Absent => {
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

/// Escape a string for safe use in shell commands
fn shell_escape(s: &str) -> String {
    // Simple escape: wrap in single quotes and escape any single quotes
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/')
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape() {
        assert_eq!(shell_escape("simple"), "simple");
        assert_eq!(shell_escape("nginx"), "nginx");
        assert_eq!(shell_escape("with space"), "'with space'");
        assert_eq!(shell_escape("with'quote"), "'with'\\''quote'");
        assert_eq!(shell_escape("pkg; rm -rf /"), "'pkg; rm -rf /'");
        assert_eq!(shell_escape("$(malicious)"), "'$(malicious)'");
        assert_eq!(shell_escape("`cmd`"), "'`cmd`'");
    }

    #[test]
    fn test_dnf_state_from_str() {
        assert_eq!(DnfState::from_str("present").unwrap(), DnfState::Present);
        assert_eq!(DnfState::from_str("installed").unwrap(), DnfState::Present);
        assert_eq!(DnfState::from_str("absent").unwrap(), DnfState::Absent);
        assert_eq!(DnfState::from_str("removed").unwrap(), DnfState::Absent);
        assert_eq!(DnfState::from_str("latest").unwrap(), DnfState::Latest);
        assert!(DnfState::from_str("invalid").is_err());
    }

    #[test]
    fn test_dnf_module_name() {
        let module = DnfModule;
        assert_eq!(module.name(), "dnf");
    }

    #[test]
    fn test_dnf_module_classification() {
        let module = DnfModule;
        assert_eq!(module.classification(), ModuleClassification::RemoteCommand);
    }

    #[test]
    fn test_dnf_module_parallelization_hint() {
        let module = DnfModule;
        assert_eq!(
            module.parallelization_hint(),
            ParallelizationHint::HostExclusive
        );
    }

    #[test]
    fn test_dnf_module_required_params() {
        let module = DnfModule;
        assert_eq!(module.required_params(), &["name"]);
    }
}
