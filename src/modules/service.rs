//! Service module - Service management
//!
//! This module manages system services using systemd (or other init systems).
//! It supports both local and remote execution via the connection interface.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use crate::connection::{CommandResult, Connection, ExecuteOptions};
use std::sync::Arc;

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
    /// Detect the init system on a target via connection
    async fn detect_async(connection: &dyn Connection) -> Option<Self> {
        // Check for systemd first (most common)
        let result = connection
            .execute("test -d /run/systemd/system && echo yes || echo no", None)
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::Systemd);
            }
        }

        // Check for systemctl
        let result = connection
            .execute("which systemctl >/dev/null 2>&1 && echo yes || echo no", None)
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::Systemd);
            }
        }

        // Check for OpenRC
        let result = connection
            .execute("which rc-service >/dev/null 2>&1 && echo yes || echo no", None)
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::OpenRC);
            }
        }

        // Check for launchctl (macOS)
        let result = connection
            .execute("which launchctl >/dev/null 2>&1 && echo yes || echo no", None)
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::Launchd);
            }
        }

        // Check for SysV init scripts
        let result = connection
            .execute("test -d /etc/init.d && echo yes || echo no", None)
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::SysV);
            }
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
    /// Build execute options with privilege escalation if needed
    fn build_execute_options(context: &ModuleContext) -> Option<ExecuteOptions> {
        if context.r#become {
            Some(ExecuteOptions {
                escalate: true,
                escalate_user: context.become_user.clone(),
                escalate_method: context.become_method.clone(),
                ..Default::default()
            })
        } else {
            None
        }
    }

    /// Execute a command via connection
    async fn execute_command(
        connection: &dyn Connection,
        command: &str,
        context: &ModuleContext,
    ) -> ModuleResult<CommandResult> {
        let options = Self::build_execute_options(context);
        connection.execute(command, options).await.map_err(|e| {
            ModuleError::ExecutionFailed(format!("Connection execute failed: {}", e))
        })
    }

    /// Check if service is active (systemd)
    async fn systemd_is_active(
        connection: &dyn Connection,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let cmd = format!("systemctl is-active {}", service);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.success)
    }

    /// Check if service is enabled (systemd)
    async fn systemd_is_enabled(
        connection: &dyn Connection,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let cmd = format!("systemctl is-enabled {}", service);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.success)
    }

    /// Execute a systemd action
    async fn systemd_action(
        connection: &dyn Connection,
        service: &str,
        action: &str,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        let cmd = format!("systemctl {} {}", action, service);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok((result.success, result.stdout, result.stderr))
    }

    /// Reload systemd daemon
    async fn systemd_daemon_reload(
        connection: &dyn Connection,
        context: &ModuleContext,
    ) -> ModuleResult<()> {
        let result = Self::execute_command(connection, "systemctl daemon-reload", context).await?;
        if result.success {
            Ok(())
        } else {
            Err(ModuleError::ExecutionFailed(result.stderr))
        }
    }

    /// Check if service is active (SysV)
    async fn sysv_is_active(
        connection: &dyn Connection,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let cmd = format!("service {} status", service);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.success)
    }

    /// Execute a SysV action
    async fn sysv_action(
        connection: &dyn Connection,
        service: &str,
        action: &str,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        let cmd = format!("service {} {}", service, action);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok((result.success, result.stdout, result.stderr))
    }

    /// Check if service is active (OpenRC)
    async fn openrc_is_active(
        connection: &dyn Connection,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let cmd = format!("rc-service {} status", service);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.success)
    }

    /// Execute an OpenRC action
    async fn openrc_action(
        connection: &dyn Connection,
        service: &str,
        action: &str,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        let cmd = format!("rc-service {} {}", service, action);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok((result.success, result.stdout, result.stderr))
    }

    /// Check if service is active for any init system
    async fn is_active(
        connection: &dyn Connection,
        init: &InitSystem,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        match init {
            InitSystem::Systemd => Self::systemd_is_active(connection, service, context).await,
            InitSystem::SysV => Self::sysv_is_active(connection, service, context).await,
            InitSystem::OpenRC => Self::openrc_is_active(connection, service, context).await,
            _ => Err(ModuleError::Unsupported(format!(
                "Init system {:?} not fully supported yet",
                init
            ))),
        }
    }

    /// Execute a service action for any init system
    async fn service_action(
        connection: &dyn Connection,
        init: &InitSystem,
        service: &str,
        action: &str,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        match init {
            InitSystem::Systemd => {
                Self::systemd_action(connection, service, action, context).await
            }
            InitSystem::SysV => Self::sysv_action(connection, service, action, context).await,
            InitSystem::OpenRC => Self::openrc_action(connection, service, action, context).await,
            _ => Err(ModuleError::Unsupported(format!(
                "Init system {:?} not fully supported yet",
                init
            ))),
        }
    }

    /// Execute the service module with async connection
    async fn execute_async(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
        connection: Arc<dyn Connection + Send + Sync>,
    ) -> ModuleResult<ModuleOutput> {
        let service = params.get_string_required("name")?;
        let state = params.get_string("state")?;
        let enabled = params.get_bool("enabled")?;
        let daemon_reload = params.get_bool_or("daemon_reload", false);

        let init = InitSystem::detect_async(connection.as_ref())
            .await
            .ok_or_else(|| {
                ModuleError::ExecutionFailed("Could not detect init system".to_string())
            })?;

        let mut changed = false;
        let mut messages = Vec::new();

        // Handle daemon-reload for systemd
        if daemon_reload && init == InitSystem::Systemd {
            if context.check_mode {
                messages.push("Would reload systemd daemon".to_string());
            } else {
                Self::systemd_daemon_reload(connection.as_ref(), context).await?;
                messages.push("Reloaded systemd daemon".to_string());
                changed = true;
            }
        }

        // Handle enabled state
        if let Some(should_enable) = enabled {
            if init == InitSystem::Systemd {
                let is_enabled =
                    Self::systemd_is_enabled(connection.as_ref(), &service, context).await?;

                if should_enable != is_enabled {
                    if context.check_mode {
                        let action = if should_enable { "enable" } else { "disable" };
                        messages.push(format!("Would {} service '{}'", action, service));
                        changed = true;
                    } else {
                        let action = if should_enable { "enable" } else { "disable" };
                        let (success, _, stderr) =
                            Self::systemd_action(connection.as_ref(), &service, action, context)
                                .await?;

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
            let is_active =
                Self::is_active(connection.as_ref(), &init, &service, context).await?;

            match desired_state {
                ServiceState::Started => {
                    if !is_active {
                        if context.check_mode {
                            messages.push(format!("Would start service '{}'", service));
                            changed = true;
                        } else {
                            let (success, _, stderr) = Self::service_action(
                                connection.as_ref(),
                                &init,
                                &service,
                                "start",
                                context,
                            )
                            .await?;

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
                            let (success, _, stderr) = Self::service_action(
                                connection.as_ref(),
                                &init,
                                &service,
                                "stop",
                                context,
                            )
                            .await?;

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
                        let (success, _, stderr) = Self::service_action(
                            connection.as_ref(),
                            &init,
                            &service,
                            "restart",
                            context,
                        )
                        .await?;

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
                        let (success, _, stderr) = Self::service_action(
                            connection.as_ref(),
                            &init,
                            &service,
                            "reload",
                            context,
                        )
                        .await?;

                        if !success {
                            // Try reload-or-restart as fallback for systemd
                            if init == InitSystem::Systemd {
                                let (success2, _, stderr2) = Self::systemd_action(
                                    connection.as_ref(),
                                    &service,
                                    "reload-or-restart",
                                    context,
                                )
                                .await?;
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
            let is_active = Self::systemd_is_active(connection.as_ref(), &service, context)
                .await
                .unwrap_or(false);
            let is_enabled = Self::systemd_is_enabled(connection.as_ref(), &service, context)
                .await
                .unwrap_or(false);
            serde_json::json!({
                "active": is_active,
                "enabled": is_enabled
            })
        } else {
            let is_active = Self::is_active(connection.as_ref(), &init, &service, context)
                .await
                .unwrap_or(false);
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

    /// Execute diff with async connection
    async fn diff_async(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
        connection: Arc<dyn Connection + Send + Sync>,
    ) -> ModuleResult<Option<Diff>> {
        let service = params.get_string_required("name")?;
        let state = params.get_string("state")?;
        let enabled = params.get_bool("enabled")?;

        let init = match InitSystem::detect_async(connection.as_ref()).await {
            Some(i) => i,
            None => return Ok(None),
        };

        let is_active = Self::is_active(connection.as_ref(), &init, &service, context)
            .await
            .unwrap_or(false);
        let is_enabled = if init == InitSystem::Systemd {
            Self::systemd_is_enabled(connection.as_ref(), &service, context)
                .await
                .unwrap_or(false)
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

impl Module for ServiceModule {
    fn name(&self) -> &'static str {
        "service"
    }

    fn description(&self) -> &'static str {
        "Manage system services"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::RemoteCommand
    }

    fn required_params(&self) -> &[&'static str] {
        &["name"]
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        // Get connection from context
        let connection = context.connection.clone().ok_or_else(|| {
            ModuleError::ExecutionFailed(
                "No connection available for service module execution".to_string(),
            )
        })?;

        // Use tokio runtime to execute async code
        let handle = tokio::runtime::Handle::try_current().map_err(|_| {
            ModuleError::ExecutionFailed("No tokio runtime available".to_string())
        })?;

        handle.block_on(self.execute_async(params, context, connection))
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    fn diff(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        // Get connection from context
        let connection = match context.connection.clone() {
            Some(c) => c,
            None => return Ok(None),
        };

        // Use tokio runtime to execute async code
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };

        handle.block_on(self.diff_async(params, context, connection))
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
    fn test_service_module_metadata() {
        let module = ServiceModule;
        assert_eq!(module.name(), "service");
        assert_eq!(module.classification(), ModuleClassification::RemoteCommand);
        assert_eq!(module.required_params(), &["name"]);
    }

    // Integration tests would require actual services and a connection
}
