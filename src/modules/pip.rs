//! Pip module - Python package management
//!
//! This module manages Python packages using pip, supporting virtualenvs,
//! requirements files, and different package states.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParallelizationHint, ParamExt,
};
use std::collections::HashMap;
use std::process::Command;

/// Desired state for a pip package
#[derive(Debug, Clone, PartialEq)]
pub enum PipState {
    Present,
    Absent,
    Latest,
}

impl PipState {
    fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "present" | "installed" => Ok(PipState::Present),
            "absent" | "removed" => Ok(PipState::Absent),
            "latest" => Ok(PipState::Latest),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: present, absent, latest",
                s
            ))),
        }
    }
}

/// Module for pip package management
pub struct PipModule;

impl PipModule {
    /// Build the pip command based on virtualenv settings
    fn build_pip_command(&self, params: &ModuleParams) -> ModuleResult<String> {
        let executable = params
            .get_string("executable")?
            .unwrap_or_else(|| "pip3".to_string());

        // If virtualenv is specified, use the pip from that virtualenv
        if let Some(venv) = params.get_string("virtualenv")? {
            Ok(format!("{}/bin/pip", venv))
        } else {
            Ok(executable)
        }
    }

    /// Check if a package is installed
    fn is_package_installed(&self, pip_cmd: &str, package: &str) -> ModuleResult<bool> {
        let output = Command::new(pip_cmd)
            .arg("show")
            .arg(package)
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to check package status: {}", e))
            })?;
        Ok(output.status.success())
    }

    /// Get installed version of a package
    fn get_installed_version(&self, pip_cmd: &str, package: &str) -> ModuleResult<Option<String>> {
        let output = Command::new(pip_cmd)
            .arg("show")
            .arg(package)
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to get package version: {}", e))
            })?;

        if output.status.success() {
            // Parse the output to find the Version line
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(version) = line.strip_prefix("Version:") {
                    let version = version.trim().to_string();
                    if version.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(version));
                }
            }
            Ok(None)
        } else {
            Ok(None)
        }
    }

    /// Execute a pip command
    fn execute_pip_command(
        &self,
        pip_cmd: &str,
        args: &[&str],
    ) -> ModuleResult<(bool, String, String)> {
        let output = Command::new(pip_cmd)
            .args(args)
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to execute pip command: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok((output.status.success(), stdout, stderr))
    }

    /// Create a virtualenv if it doesn't exist
    fn ensure_virtualenv(&self, venv_path: &str) -> ModuleResult<bool> {
        // Check if virtualenv exists by checking for the activate script
        let activate_path = std::path::Path::new(venv_path).join("bin").join("activate");
        if activate_path.exists() {
            return Ok(false);
        }

        // Create virtualenv using safe argument passing
        let output = Command::new("python3")
            .arg("-m")
            .arg("venv")
            .arg(venv_path)
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to create virtualenv: {}", e))
            })?;

        if !output.status.success() {
            return Err(ModuleError::ExecutionFailed(format!(
                "Failed to create virtualenv: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(true)
    }

    /// Handle requirements file installation
    fn handle_requirements(
        &self,
        pip_cmd: &str,
        requirements: &str,
        state: &PipState,
        venv_created: bool,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        if *state == PipState::Absent {
            return Err(ModuleError::InvalidParameter(
                "state=absent is not supported with requirements parameter".to_string(),
            ));
        }

        if context.check_mode {
            let mut msg = String::new();
            if venv_created {
                msg.push_str("Would create virtualenv. ");
            }
            msg.push_str(&format!(
                "Would install packages from requirements file: {}",
                requirements
            ));
            return Ok(ModuleOutput::changed(msg));
        }

        let args = if *state == PipState::Latest {
            vec!["install", "--upgrade", "-r", requirements]
        } else {
            vec!["install", "-r", requirements]
        };

        let (success, stdout, stderr) = self.execute_pip_command(pip_cmd, &args)?;

        if !success {
            return Err(ModuleError::ExecutionFailed(format!(
                "Failed to install from requirements: {}",
                if stderr.is_empty() { stdout } else { stderr }
            )));
        }

        // Check if anything was actually installed by looking for "already satisfied" in output
        let changed = !stdout.contains("Requirement already satisfied");

        if changed || venv_created {
            Ok(ModuleOutput::changed(format!(
                "Installed packages from requirements file: {}",
                requirements
            ))
            .with_command_output(Some(stdout), Some(stderr), Some(0)))
        } else {
            Ok(
                ModuleOutput::ok("All requirements already satisfied".to_string())
                    .with_command_output(Some(stdout), Some(stderr), Some(0)),
            )
        }
    }
}

impl Module for PipModule {
    fn name(&self) -> &'static str {
        "pip"
    }

    fn description(&self) -> &'static str {
        "Manage Python packages with pip"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::RemoteCommand
    }

    fn parallelization_hint(&self) -> ParallelizationHint {
        // Pip can generally run in parallel, but virtualenv operations might conflict
        ParallelizationHint::FullyParallel
    }

    fn required_params(&self) -> &[&'static str] {
        // Either 'name' or 'requirements' must be provided
        &[]
    }

    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        // Must have either name or requirements
        if params.get("name").is_none() && params.get("requirements").is_none() {
            return Err(ModuleError::MissingParameter(
                "Either 'name' or 'requirements' must be provided".to_string(),
            ));
        }
        Ok(())
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let pip_cmd = self.build_pip_command(params)?;

        // Get state
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());
        let state = PipState::from_str(&state_str)?;

        // Handle virtualenv creation if needed
        let mut venv_created = false;
        if let Some(venv) = params.get_string("virtualenv")? {
            if !context.check_mode {
                venv_created = self.ensure_virtualenv(&venv)?;
            }
        }

        // Handle requirements file
        if let Some(requirements) = params.get_string("requirements")? {
            return self.handle_requirements(
                &pip_cmd,
                &requirements,
                &state,
                venv_created,
                context,
            );
        }

        // Handle individual packages
        let packages: Vec<String> = if let Some(names) = params.get_vec_string("name")? {
            names
        } else {
            vec![params.get_string_required("name")?]
        };

        let mut to_install: Vec<String> = Vec::new();
        let mut to_remove: Vec<String> = Vec::new();
        let mut already_ok: Vec<String> = Vec::new();

        for package in &packages {
            let is_installed = self.is_package_installed(&pip_cmd, package)?;

            match state {
                PipState::Present => {
                    if is_installed {
                        already_ok.push(package.clone());
                    } else {
                        to_install.push(package.clone());
                    }
                }
                PipState::Absent => {
                    if is_installed {
                        to_remove.push(package.clone());
                    } else {
                        already_ok.push(package.clone());
                    }
                }
                PipState::Latest => {
                    // For 'latest', we always try to install/upgrade
                    to_install.push(package.clone());
                }
            }
        }

        // Check mode - return what would happen
        if context.check_mode {
            if to_install.is_empty() && to_remove.is_empty() && !venv_created {
                return Ok(ModuleOutput::ok(format!(
                    "All packages already in desired state: {}",
                    already_ok.join(", ")
                )));
            }

            let mut msg = String::new();
            if venv_created {
                msg.push_str("Would create virtualenv. ");
            }
            if !to_install.is_empty() {
                msg.push_str(&format!("Would install: {}. ", to_install.join(", ")));
            }
            if !to_remove.is_empty() {
                msg.push_str(&format!("Would remove: {}. ", to_remove.join(", ")));
            }

            return Ok(ModuleOutput::changed(msg.trim().to_string()));
        }

        // Perform the actual operations
        let mut changed = venv_created;
        let mut results: HashMap<String, String> = HashMap::new();

        if !to_install.is_empty() {
            let install_args = if state == PipState::Latest {
                vec!["install", "--upgrade"]
            } else {
                vec!["install"]
            };

            let mut args = install_args;
            for pkg in &to_install {
                args.push(pkg);
            }

            let (success, stdout, stderr) = self.execute_pip_command(&pip_cmd, &args)?;

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
            let mut args = vec!["uninstall", "-y"];
            for pkg in &to_remove {
                args.push(pkg);
            }

            let (success, stdout, stderr) = self.execute_pip_command(&pip_cmd, &args)?;

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
            if venv_created {
                msg.push_str("Virtualenv created. ");
            }
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
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let pip_cmd = self.build_pip_command(params)?;

        // Get state
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());
        let state = PipState::from_str(&state_str)?;

        // Handle requirements file differently
        if let Some(requirements) = params.get_string("requirements")? {
            return Ok(Some(Diff::new(
                "(requirements not shown)",
                format!("Install from: {}", requirements),
            )));
        }

        // Handle individual packages
        let packages: Vec<String> = if let Some(names) = params.get_vec_string("name")? {
            names
        } else {
            vec![params.get_string_required("name")?]
        };

        let mut before_lines = Vec::new();
        let mut after_lines = Vec::new();

        for package in &packages {
            let is_installed = self.is_package_installed(&pip_cmd, package)?;
            let version = if is_installed {
                self.get_installed_version(&pip_cmd, package)?
                    .unwrap_or_default()
            } else {
                String::new()
            };

            match state {
                PipState::Present | PipState::Latest => {
                    if is_installed {
                        before_lines.push(format!("{}: {}", package, version));
                        if state == PipState::Latest {
                            after_lines.push(format!("{}: (will be upgraded)", package));
                        } else {
                            after_lines.push(format!("{}: {}", package, version));
                        }
                    } else {
                        before_lines.push(format!("{}: (not installed)", package));
                        after_lines.push(format!("{}: (will be installed)", package));
                    }
                }
                PipState::Absent => {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pip_state_from_str() {
        assert_eq!(PipState::from_str("present").unwrap(), PipState::Present);
        assert_eq!(PipState::from_str("installed").unwrap(), PipState::Present);
        assert_eq!(PipState::from_str("absent").unwrap(), PipState::Absent);
        assert_eq!(PipState::from_str("removed").unwrap(), PipState::Absent);
        assert_eq!(PipState::from_str("latest").unwrap(), PipState::Latest);
        assert!(PipState::from_str("invalid").is_err());
    }

    #[test]
    fn test_build_pip_command() {
        let module = PipModule;
        let mut params: ModuleParams = HashMap::new();

        // Default pip command
        let cmd = module.build_pip_command(&params).unwrap();
        assert_eq!(cmd, "pip3");

        // Custom executable
        params.insert("executable".to_string(), serde_json::json!("pip"));
        let cmd = module.build_pip_command(&params).unwrap();
        assert_eq!(cmd, "pip");

        // Virtualenv overrides executable
        params.insert("virtualenv".to_string(), serde_json::json!("/path/to/venv"));
        let cmd = module.build_pip_command(&params).unwrap();
        assert_eq!(cmd, "/path/to/venv/bin/pip");
    }

    #[test]
    fn test_validate_params() {
        let module = PipModule;

        // Missing both name and requirements
        let params: ModuleParams = HashMap::new();
        assert!(module.validate_params(&params).is_err());

        // Has name
        let mut params: ModuleParams = HashMap::new();
        params.insert("name".to_string(), serde_json::json!("requests"));
        assert!(module.validate_params(&params).is_ok());

        // Has requirements
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "requirements".to_string(),
            serde_json::json!("requirements.txt"),
        );
        assert!(module.validate_params(&params).is_ok());
    }
}
