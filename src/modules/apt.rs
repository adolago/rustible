//! Apt module - Debian/Ubuntu package management
//!
//! This module manages packages using the APT package manager on Debian-based systems.
//! It supports installing, removing, and upgrading packages, as well as updating the package cache.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParallelizationHint, ParamExt,
};
use crate::connection::ExecuteOptions;
use std::collections::HashMap;

/// Desired state for a package
#[derive(Debug, Clone, PartialEq)]
pub enum AptState {
    Present,
    Absent,
    Latest,
}

impl AptState {
    fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "present" | "installed" => Ok(AptState::Present),
            "absent" | "removed" => Ok(AptState::Absent),
            "latest" => Ok(AptState::Latest),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: present, absent, latest",
                s
            ))),
        }
    }
}

/// Module for APT package management
pub struct AptModule;

impl AptModule {
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

    /// Check if a package is installed using dpkg via remote connection
    async fn is_package_installed_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        package: &str,
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<bool> {
        let cmd = format!(
            "dpkg -s {} 2>/dev/null | grep -q '^Status:.*installed'",
            shell_escape(package)
        );
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
        let cmd = format!(
            "dpkg-query -W -f='${{Version}}' {} 2>/dev/null",
            shell_escape(package)
        );
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

    /// Update the apt cache via remote connection
    async fn update_cache_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<()> {
        let cmd = "apt-get update";
        let result = conn.execute(cmd, options).await.map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to update apt cache: {}", e))
        })?;

        if !result.success {
            return Err(ModuleError::ExecutionFailed(format!(
                "Failed to update apt cache: {}",
                result.stderr
            )));
        }

        Ok(())
    }

    /// Install packages via remote connection
    async fn install_packages_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        packages: &[String],
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<()> {
        let pkg_list: Vec<String> = packages.iter().map(|p| shell_escape(p)).collect();
        let cmd = format!(
            "DEBIAN_FRONTEND=noninteractive apt-get install -y {}",
            pkg_list.join(" ")
        );

        let result = conn.execute(&cmd, options).await.map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to install packages: {}", e))
        })?;

        if !result.success {
            return Err(ModuleError::ExecutionFailed(format!(
                "Failed to install packages: {}",
                result.stderr
            )));
        }

        Ok(())
    }

    /// Remove packages via remote connection
    async fn remove_packages_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        packages: &[String],
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<()> {
        let pkg_list: Vec<String> = packages.iter().map(|p| shell_escape(p)).collect();
        let cmd = format!(
            "DEBIAN_FRONTEND=noninteractive apt-get remove -y {}",
            pkg_list.join(" ")
        );

        let result = conn.execute(&cmd, options).await.map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to remove packages: {}", e))
        })?;

        if !result.success {
            return Err(ModuleError::ExecutionFailed(format!(
                "Failed to remove packages: {}",
                result.stderr
            )));
        }

        Ok(())
    }

    /// Upgrade packages to latest version via remote connection
    async fn upgrade_packages_remote(
        conn: &(dyn crate::connection::Connection + Send + Sync),
        packages: &[String],
        options: Option<ExecuteOptions>,
    ) -> ModuleResult<()> {
        let pkg_list: Vec<String> = packages.iter().map(|p| shell_escape(p)).collect();
        let cmd = format!(
            "DEBIAN_FRONTEND=noninteractive apt-get install --only-upgrade -y {}",
            pkg_list.join(" ")
        );

        let result = conn.execute(&cmd, options).await.map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to upgrade packages: {}", e))
        })?;

        if !result.success {
            return Err(ModuleError::ExecutionFailed(format!(
                "Failed to upgrade packages: {}",
                result.stderr
            )));
        }

        Ok(())
    }
}

impl Module for AptModule {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn description(&self) -> &'static str {
        "Manage packages with the APT package manager"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::RemoteCommand
    }

    fn parallelization_hint(&self) -> ParallelizationHint {
        // APT uses locks - only one can run per host at a time
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
        let state = AptState::from_str(&state_str)?;
        let update_cache = params.get_bool_or("update_cache", false);

        // Get connection from context
        let conn = context.connection.as_ref().ok_or_else(|| {
            ModuleError::ExecutionFailed(
                "No connection available in context. APT module requires a remote connection."
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
                let mut to_upgrade: Vec<String> = Vec::new();
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
                        AptState::Present => {
                            if is_installed {
                                already_ok.push(package.clone());
                            } else {
                                to_install.push(package.clone());
                            }
                        }
                        AptState::Absent => {
                            if is_installed {
                                to_remove.push(package.clone());
                            } else {
                                already_ok.push(package.clone());
                            }
                        }
                        AptState::Latest => {
                            if is_installed {
                                to_upgrade.push(package.clone());
                            } else {
                                to_install.push(package.clone());
                            }
                        }
                    }
                }

                // Check mode - return what would happen
                if context.check_mode {
                    if to_install.is_empty() && to_remove.is_empty() && to_upgrade.is_empty() {
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
                    if !to_upgrade.is_empty() {
                        msg.push_str(&format!("Would upgrade: {}. ", to_upgrade.join(", ")));
                    }

                    return Ok(ModuleOutput::changed(msg.trim().to_string()));
                }

                // Perform the actual operations
                let mut changed = false;
                let mut results: HashMap<String, String> = HashMap::new();

                if !to_install.is_empty() {
                    Self::install_packages_remote(
                        conn.as_ref(),
                        &to_install,
                        Some(exec_options.clone()),
                    )
                    .await?;
                    changed = true;
                    for pkg in &to_install {
                        results.insert(pkg.clone(), "installed".to_string());
                    }
                }

                if !to_upgrade.is_empty() {
                    Self::upgrade_packages_remote(
                        conn.as_ref(),
                        &to_upgrade,
                        Some(exec_options.clone()),
                    )
                    .await?;
                    changed = true;
                    for pkg in &to_upgrade {
                        results.insert(pkg.clone(), "upgraded".to_string());
                    }
                }

                if !to_remove.is_empty() {
                    Self::remove_packages_remote(
                        conn.as_ref(),
                        &to_remove,
                        Some(exec_options.clone()),
                    )
                    .await?;
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
                    if !to_upgrade.is_empty() {
                        msg.push_str(&format!("Upgraded: {}. ", to_upgrade.join(", ")));
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
        let state = AptState::from_str(&state_str)?;

        // Get connection from context
        let conn = match context.connection.as_ref() {
            Some(c) => c,
            None => {
                // No connection available, return basic diff without checking remote state
                let mut before_lines = Vec::new();
                let mut after_lines = Vec::new();

                for package in &packages {
                    match state {
                        AptState::Present | AptState::Latest => {
                            before_lines.push(format!("{}: (unknown)", package));
                            after_lines.push(format!("{}: (will be installed/updated)", package));
                        }
                        AptState::Absent => {
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
                        AptState::Present => {
                            if is_installed {
                                before_lines.push(format!("{}: {}", package, version));
                                after_lines.push(format!("{}: {}", package, version));
                            } else {
                                before_lines.push(format!("{}: (not installed)", package));
                                after_lines.push(format!("{}: (will be installed)", package));
                            }
                        }
                        AptState::Absent => {
                            if is_installed {
                                before_lines.push(format!("{}: {}", package, version));
                                after_lines.push(format!("{}: (will be removed)", package));
                            } else {
                                before_lines.push(format!("{}: (not installed)", package));
                                after_lines.push(format!("{}: (not installed)", package));
                            }
                        }
                        AptState::Latest => {
                            if is_installed {
                                before_lines.push(format!("{}: {}", package, version));
                                after_lines.push(format!("{}: (will be upgraded)", package));
                            } else {
                                before_lines.push(format!("{}: (not installed)", package));
                                after_lines.push(format!("{}: (will be installed)", package));
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
    fn test_apt_state_from_str() {
        assert_eq!(AptState::from_str("present").unwrap(), AptState::Present);
        assert_eq!(AptState::from_str("installed").unwrap(), AptState::Present);
        assert_eq!(AptState::from_str("absent").unwrap(), AptState::Absent);
        assert_eq!(AptState::from_str("removed").unwrap(), AptState::Absent);
        assert_eq!(AptState::from_str("latest").unwrap(), AptState::Latest);
        assert!(AptState::from_str("invalid").is_err());
    }

    #[test]
    fn test_apt_module_name() {
        let module = AptModule;
        assert_eq!(module.name(), "apt");
    }

    #[test]
    fn test_apt_module_classification() {
        let module = AptModule;
        assert_eq!(module.classification(), ModuleClassification::RemoteCommand);
    }

    #[test]
    fn test_apt_module_parallelization() {
        let module = AptModule;
        assert_eq!(
            module.parallelization_hint(),
            ParallelizationHint::HostExclusive
        );
    }

    #[test]
    fn test_apt_module_required_params() {
        let module = AptModule;
        assert_eq!(module.required_params(), &["name"]);
    }

    // Integration tests would require actual apt access
    // These are unit tests for the parsing/configuration logic

    #[test]
    fn test_shell_escape() {
        assert_eq!(shell_escape("simple"), "simple");
        assert_eq!(shell_escape("nginx"), "nginx");
        assert_eq!(shell_escape("with space"), "'with space'");
        assert_eq!(shell_escape("with'quote"), "'with'\\''quote'");
        assert_eq!(shell_escape("pkg; rm -rf /"), "'pkg; rm -rf /'");
        assert_eq!(shell_escape("$(whoami)"), "'$(whoami)'");
        assert_eq!(shell_escape("`id`"), "'`id`'");
    }
}

/// Escape a string for safe use in shell commands
fn shell_escape(s: &str) -> String {
    // Simple escape: wrap in single quotes and escape any single quotes
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' || c == '+')
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
