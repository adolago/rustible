//! Service module - Service management
//!
//! This module manages system services using systemd (or other init systems).

use super::{
    Diff, Module, ModuleContext, ModuleError, ModuleOutput, ModuleParams, ModuleResult, ParamExt,
};
use std::process::Command;

/// Supported init systems
#[derive(Debug, Clone, PartialEq)]
pub enum InitSystem {
    Systemd,
    SysV,
    Upstart,
    OpenRC,
    Launchd,
}

impl InitSystem {
    fn detect() -> Option<Self> {
        // Check for systemd first (most common)
        if std::path::Path::new("/run/systemd/system").exists() {
            return Some(InitSystem::Systemd);
        }

        // Check for other init systems
        if Command::new("which")
            .arg("systemctl")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(InitSystem::Systemd);
        }

        if Command::new("which")
            .arg("rc-service")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(InitSystem::OpenRC);
        }

        if Command::new("which")
            .arg("launchctl")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(InitSystem::Launchd);
        }

        if std::path::Path::new("/etc/init.d").exists() {
            return Some(InitSystem::SysV);
        }

        None
    }
}

/// Desired state for a service
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceState {
    Started,
    Stopped,
    Restarted,
    Reloaded,
}

impl ServiceState {
    fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "started" | "running" => Ok(ServiceState::Started),
            "stopped" => Ok(ServiceState::Stopped),
            "restarted" => Ok(ServiceState::Restarted),
            "reloaded" => Ok(ServiceState::Reloaded),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: started, stopped, restarted, reloaded",
                s
            ))),
        }
    }
}

/// Module for service management
pub struct ServiceModule;

impl ServiceModule {
    fn systemd_is_active(service: &str) -> ModuleResult<bool> {
        let output = Command::new("systemctl")
            .args(["is-active", service])
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to check service status: {}", e))
            })?;

        Ok(output.status.success())
    }

    fn systemd_is_enabled(service: &str) -> ModuleResult<bool> {
        let output = Command::new("systemctl")
            .args(["is-enabled", service])
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!(
                    "Failed to check service enabled status: {}",
                    e
                ))
            })?;

        Ok(output.status.success())
    }

    fn systemd_action(service: &str, action: &str) -> ModuleResult<(bool, String, String)> {
        let output = Command::new("systemctl")
            .args([action, service])
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to {} service: {}", action, e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok((output.status.success(), stdout, stderr))
    }

    fn systemd_daemon_reload() -> ModuleResult<()> {
        let output = Command::new("systemctl")
            .arg("daemon-reload")
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to reload systemd: {}", e))
            })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(ModuleError::ExecutionFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn sysv_is_active(service: &str) -> ModuleResult<bool> {
        let output = Command::new("service")
            .args([service, "status"])
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to check service status: {}", e))
            })?;

        Ok(output.status.success())
    }

    fn sysv_action(service: &str, action: &str) -> ModuleResult<(bool, String, String)> {
        let output = Command::new("service")
            .args([service, action])
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to {} service: {}", action, e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok((output.status.success(), stdout, stderr))
    }

    fn openrc_is_active(service: &str) -> ModuleResult<bool> {
        let output = Command::new("rc-service")
            .args([service, "status"])
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to check service status: {}", e))
            })?;

        Ok(output.status.success())
    }

    fn openrc_action(service: &str, action: &str) -> ModuleResult<(bool, String, String)> {
        let output = Command::new("rc-service")
            .args([service, action])
            .output()
            .map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to {} service: {}", action, e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok((output.status.success(), stdout, stderr))
    }

    fn is_active(init: &InitSystem, service: &str) -> ModuleResult<bool> {
        match init {
            InitSystem::Systemd => Self::systemd_is_active(service),
            InitSystem::SysV => Self::sysv_is_active(service),
            InitSystem::OpenRC => Self::openrc_is_active(service),
            _ => Err(ModuleError::Unsupported(format!(
                "Init system {:?} not fully supported yet",
                init
            ))),
        }
    }

    fn service_action(
        init: &InitSystem,
        service: &str,
        action: &str,
    ) -> ModuleResult<(bool, String, String)> {
        match init {
            InitSystem::Systemd => Self::systemd_action(service, action),
            InitSystem::SysV => Self::sysv_action(service, action),
            InitSystem::OpenRC => Self::openrc_action(service, action),
            _ => Err(ModuleError::Unsupported(format!(
                "Init system {:?} not fully supported yet",
                init
            ))),
        }
    }
}

impl Module for ServiceModule {
    fn name(&self) -> &'static str {
        "service"
    }

    fn description(&self) -> &'static str {
        "Manage system services"
    }

    fn required_params(&self) -> &[&'static str] {
        &["name"]
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let service = params.get_string_required("name")?;
        let state = params.get_string("state")?;
        let enabled = params.get_bool("enabled")?;
        let daemon_reload = params.get_bool_or("daemon_reload", false);

        let init = InitSystem::detect().ok_or_else(|| {
            ModuleError::ExecutionFailed("Could not detect init system".to_string())
        })?;

        let mut changed = false;
        let mut messages = Vec::new();

        // Handle daemon-reload for systemd
        if daemon_reload && init == InitSystem::Systemd {
            if context.check_mode {
                messages.push("Would reload systemd daemon".to_string());
            } else {
                Self::systemd_daemon_reload()?;
                messages.push("Reloaded systemd daemon".to_string());
                changed = true;
            }
        }

        // Handle enabled state
        if let Some(should_enable) = enabled {
            if init == InitSystem::Systemd {
                let is_enabled = Self::systemd_is_enabled(&service)?;

                if should_enable != is_enabled {
                    if context.check_mode {
                        let action = if should_enable { "enable" } else { "disable" };
                        messages.push(format!("Would {} service '{}'", action, service));
                        changed = true;
                    } else {
                        let action = if should_enable { "enable" } else { "disable" };
                        let (success, _, stderr) = Self::systemd_action(&service, action)?;

                        if !success {
                            return Err(ModuleError::ExecutionFailed(format!(
                                "Failed to {} service '{}': {}",
                                action, service, stderr
                            )));
                        }

                        messages.push(format!("{}d service '{}'", action, service));
                        changed = true;
                    }
                }
            }
        }

        // Handle state
        if let Some(state_str) = state {
            let desired_state = ServiceState::from_str(&state_str)?;
            let is_active = Self::is_active(&init, &service)?;

            match desired_state {
                ServiceState::Started => {
                    if !is_active {
                        if context.check_mode {
                            messages.push(format!("Would start service '{}'", service));
                            changed = true;
                        } else {
                            let (success, _, stderr) =
                                Self::service_action(&init, &service, "start")?;

                            if !success {
                                return Err(ModuleError::ExecutionFailed(format!(
                                    "Failed to start service '{}': {}",
                                    service, stderr
                                )));
                            }

                            messages.push(format!("Started service '{}'", service));
                            changed = true;
                        }
                    } else {
                        messages.push(format!("Service '{}' is already running", service));
                    }
                }

                ServiceState::Stopped => {
                    if is_active {
                        if context.check_mode {
                            messages.push(format!("Would stop service '{}'", service));
                            changed = true;
                        } else {
                            let (success, _, stderr) =
                                Self::service_action(&init, &service, "stop")?;

                            if !success {
                                return Err(ModuleError::ExecutionFailed(format!(
                                    "Failed to stop service '{}': {}",
                                    service, stderr
                                )));
                            }

                            messages.push(format!("Stopped service '{}'", service));
                            changed = true;
                        }
                    } else {
                        messages.push(format!("Service '{}' is already stopped", service));
                    }
                }

                ServiceState::Restarted => {
                    if context.check_mode {
                        messages.push(format!("Would restart service '{}'", service));
                        changed = true;
                    } else {
                        let (success, _, stderr) =
                            Self::service_action(&init, &service, "restart")?;

                        if !success {
                            return Err(ModuleError::ExecutionFailed(format!(
                                "Failed to restart service '{}': {}",
                                service, stderr
                            )));
                        }

                        messages.push(format!("Restarted service '{}'", service));
                        changed = true;
                    }
                }

                ServiceState::Reloaded => {
                    if context.check_mode {
                        messages.push(format!("Would reload service '{}'", service));
                        changed = true;
                    } else {
                        let (success, _, stderr) = Self::service_action(&init, &service, "reload")?;

                        if !success {
                            // Try reload-or-restart as fallback
                            if init == InitSystem::Systemd {
                                let (success2, _, stderr2) =
                                    Self::systemd_action(&service, "reload-or-restart")?;
                                if !success2 {
                                    return Err(ModuleError::ExecutionFailed(format!(
                                        "Failed to reload service '{}': {}",
                                        service, stderr2
                                    )));
                                }
                            } else {
                                return Err(ModuleError::ExecutionFailed(format!(
                                    "Failed to reload service '{}': {}",
                                    service, stderr
                                )));
                            }
                        }

                        messages.push(format!("Reloaded service '{}'", service));
                        changed = true;
                    }
                }
            }
        }

        let msg = if messages.is_empty() {
            format!("Service '{}' is in desired state", service)
        } else {
            messages.join(". ")
        };

        // Get current status for output
        let status = if init == InitSystem::Systemd {
            let is_active = Self::systemd_is_active(&service).unwrap_or(false);
            let is_enabled = Self::systemd_is_enabled(&service).unwrap_or(false);
            serde_json::json!({
                "active": is_active,
                "enabled": is_enabled
            })
        } else {
            let is_active = Self::is_active(&init, &service).unwrap_or(false);
            serde_json::json!({
                "active": is_active
            })
        };

        if changed {
            Ok(ModuleOutput::changed(msg).with_data("status", status))
        } else {
            Ok(ModuleOutput::ok(msg).with_data("status", status))
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
        let service = params.get_string_required("name")?;
        let state = params.get_string("state")?;
        let enabled = params.get_bool("enabled")?;

        let init = match InitSystem::detect() {
            Some(i) => i,
            None => return Ok(None),
        };

        let is_active = Self::is_active(&init, &service).unwrap_or(false);
        let is_enabled = if init == InitSystem::Systemd {
            Self::systemd_is_enabled(&service).unwrap_or(false)
        } else {
            false
        };

        let mut before_lines = Vec::new();
        let mut after_lines = Vec::new();

        before_lines.push(format!("active: {}", if is_active { "yes" } else { "no" }));

        if let Some(state_str) = state {
            let desired_state = ServiceState::from_str(&state_str)?;
            let will_be_active = match desired_state {
                ServiceState::Started => true,
                ServiceState::Stopped => false,
                ServiceState::Restarted | ServiceState::Reloaded => is_active,
            };
            after_lines.push(format!(
                "active: {}",
                if will_be_active { "yes" } else { "no" }
            ));
        } else {
            after_lines.push(format!("active: {}", if is_active { "yes" } else { "no" }));
        }

        if init == InitSystem::Systemd {
            before_lines.push(format!(
                "enabled: {}",
                if is_enabled { "yes" } else { "no" }
            ));

            if let Some(should_enable) = enabled {
                after_lines.push(format!(
                    "enabled: {}",
                    if should_enable { "yes" } else { "no" }
                ));
            } else {
                after_lines.push(format!(
                    "enabled: {}",
                    if is_enabled { "yes" } else { "no" }
                ));
            }
        }

        let before = before_lines.join("\n");
        let after = after_lines.join("\n");

        if before == after {
            Ok(None)
        } else {
            Ok(Some(Diff::new(before, after)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_state_from_str() {
        assert_eq!(
            ServiceState::from_str("started").unwrap(),
            ServiceState::Started
        );
        assert_eq!(
            ServiceState::from_str("running").unwrap(),
            ServiceState::Started
        );
        assert_eq!(
            ServiceState::from_str("stopped").unwrap(),
            ServiceState::Stopped
        );
        assert_eq!(
            ServiceState::from_str("restarted").unwrap(),
            ServiceState::Restarted
        );
        assert_eq!(
            ServiceState::from_str("reloaded").unwrap(),
            ServiceState::Reloaded
        );
        assert!(ServiceState::from_str("invalid").is_err());
    }

    #[test]
    fn test_init_system_detection() {
        // This will return something on most systems
        let init = InitSystem::detect();
        // Just verify it doesn't panic
        let _ = init;
    }

    // Integration tests would require root access and actual services
}
