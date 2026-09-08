//! Service module - Service management
//!
//! This module manages system services using systemd (or other init systems).
//! It supports both local and remote execution via the connection interface.
//!
//! ## Supported Init Systems
//!
//! - **Systemd**: Modern init system (default on most Linux distributions)
//! - **SysV**: Traditional init scripts (/etc/init.d)
//! - **OpenRC**: Used by Gentoo/Alpine
//! - **Upstart**: Legacy Ubuntu (deprecated)
//! - **Launchd**: macOS service management
//!
//! ## Parameters
//!
//! - `name`: Service name (required)
//! - `state`: Desired state (started, stopped, restarted, reloaded)
//! - `enabled`: Whether service should start on boot
//! - `pattern`: Process pattern for status detection (for services without proper status)
//! - `runlevel`: Runlevel(s) for sysvinit enable/disable
//! - `sleep`: Seconds to wait between stop and start during restart
//! - `use_systemctl`: Force use of systemctl even if service command available
//! - `daemon_reload`: Reload systemd daemon before action
//! - `daemon_reexec`: Re-execute systemd manager
//!
//! ## Service name patterns (systemd only)
//!
//! A `name` containing `*`, `?` or `[` is a systemd unit glob. It is expanded with
//! `systemctl list-units --all` and every matched unit is processed on its own.
//! The result carries `pattern`, `matched_count` and a `services` array with one
//! entry per unit (`name` and `changed`, plus `message`, or `failed` and `error`).
//!
//! - A unit that cannot be started, stopped, restarted, reloaded, enabled or
//!   disabled fails the task. The remaining units are still processed, the
//!   message lists every unit's outcome and `changed` reports whether anything
//!   was modified before the failure, per unit and overall (a unit that was
//!   enabled and then failed to start counts as changed).
//! - A pattern that matches nothing fails when the requested outcome needs a
//!   unit to exist: `state=started`, `state=restarted`, `state=reloaded` or
//!   `enabled=true`, just as Ansible fails for an unknown service.
//!   `state=stopped`, `enabled=false` and status-only calls are already
//!   satisfied, so they succeed unchanged with `matched_count: 0`. Check mode
//!   applies the same rule.

use super::{
    validate_command_args, Diff, Module, ModuleClassification, ModuleContext, ModuleError,
    ModuleOutput, ModuleParams, ModuleResult, ParamExt,
};
use crate::connection::{CommandResult, Connection, ExecuteOptions};
use crate::utils::shell_escape;
use std::sync::Arc;
use std::time::Duration;

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
    ///
    /// Detection priority:
    /// 1. Check for running systemd (PID 1 is systemd or /run/systemd/system exists)
    /// 2. Check for systemctl binary
    /// 3. Check for OpenRC (rc-service)
    /// 4. Check for launchctl (macOS)
    /// 5. Check for Upstart (/etc/init)
    /// 6. Fall back to SysV if /etc/init.d exists
    async fn detect_async(
        connection: &dyn Connection,
        use_systemctl: Option<bool>,
    ) -> Option<Self> {
        // If user explicitly wants systemctl, check if it's available
        if use_systemctl == Some(true) {
            let result = connection
                .execute(
                    "command -v systemctl >/dev/null 2>&1 && echo yes || echo no",
                    None,
                )
                .await;
            if let Ok(result) = result {
                if result.stdout.trim() == "yes" {
                    return Some(InitSystem::Systemd);
                }
            }
        }

        // Check if systemd is running as PID 1 (most reliable check)
        let result = connection
            .execute(
                "ps -p 1 -o comm= 2>/dev/null | grep -q systemd && echo yes || echo no",
                None,
            )
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::Systemd);
            }
        }

        // Check for /run/systemd/system directory (systemd is active)
        let result = connection
            .execute("test -d /run/systemd/system && echo yes || echo no", None)
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::Systemd);
            }
        }

        // Check for systemctl binary (might be systemd but not running as init)
        let result = connection
            .execute(
                "command -v systemctl >/dev/null 2>&1 && echo yes || echo no",
                None,
            )
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                // Verify systemctl works
                let verify = connection
                    .execute(
                        "systemctl --version >/dev/null 2>&1 && echo yes || echo no",
                        None,
                    )
                    .await;
                if let Ok(verify) = verify {
                    if verify.stdout.trim() == "yes" {
                        return Some(InitSystem::Systemd);
                    }
                }
            }
        }

        // Check for OpenRC (Gentoo, Alpine)
        let result = connection
            .execute(
                "command -v rc-service >/dev/null 2>&1 && echo yes || echo no",
                None,
            )
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::OpenRC);
            }
        }

        // Check for launchctl (macOS)
        let result = connection
            .execute(
                "command -v launchctl >/dev/null 2>&1 && echo yes || echo no",
                None,
            )
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                return Some(InitSystem::Launchd);
            }
        }

        // Check for Upstart (/etc/init with .conf files, not systemd)
        let result = connection
            .execute(
                "test -d /etc/init && ls /etc/init/*.conf >/dev/null 2>&1 && echo yes || echo no",
                None,
            )
            .await;
        if let Ok(result) = result {
            if result.stdout.trim() == "yes" {
                // Make sure it's not systemd
                let not_systemd = connection
                    .execute("test ! -d /run/systemd/system && echo yes || echo no", None)
                    .await;
                if let Ok(ns) = not_systemd {
                    if ns.stdout.trim() == "yes" {
                        return Some(InitSystem::Upstart);
                    }
                }
            }
        }

        // Check for SysV init scripts (fallback)
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

    /// Get the service binary/command for this init system
    #[allow(dead_code)]
    fn service_command(&self) -> &'static str {
        match self {
            InitSystem::Systemd => "systemctl",
            InitSystem::SysV => "service",
            InitSystem::OpenRC => "rc-service",
            InitSystem::Upstart => "initctl",
            InitSystem::Launchd => "launchctl",
        }
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
    pub fn from_str(s: &str) -> ModuleResult<Self> {
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

impl std::str::FromStr for ServiceState {
    type Err = ModuleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ServiceState::from_str(s)
    }
}

/// Service module configuration parsed from parameters
#[derive(Debug, Clone)]
struct ServiceConfig {
    /// Service name (supports wildcards for pattern matching)
    name: String,
    /// Desired state
    state: Option<ServiceState>,
    /// Whether to enable at boot
    enabled: Option<bool>,
    /// Pattern to match in process table for status detection
    pattern: Option<String>,
    /// Runlevel(s) for sysvinit enable/disable (e.g., "2345" or "default")
    runlevel: Option<String>,
    /// Seconds to sleep between stop and start during restart
    sleep: Option<u64>,
    /// Force use of systemctl even if service command is available
    use_systemctl: Option<bool>,
    /// Reload systemd daemon before action
    daemon_reload: bool,
    /// Re-execute systemd manager
    daemon_reexec: bool,
    /// Additional arguments passed to the service command
    arguments: Option<String>,
}

impl ServiceConfig {
    fn from_params(params: &ModuleParams) -> ModuleResult<Self> {
        let name = params.get_string_required("name")?;
        let state = if let Some(s) = params.get_string("state")? {
            Some(ServiceState::from_str(&s)?)
        } else {
            None
        };

        // Parse sleep as i64 and convert to u64 (must be non-negative)
        let sleep = match params.get_i64("sleep")? {
            Some(s) if s >= 0 => Some(s as u64),
            Some(s) => {
                return Err(ModuleError::InvalidParameter(format!(
                    "sleep must be a non-negative integer, got {}",
                    s
                )))
            }
            None => None,
        };

        let arguments = params.get_string("arguments")?;
        if let Some(ref args) = arguments {
            // Validate arguments to prevent shell injection
            validate_command_args(args)?;
        }

        Ok(Self {
            name,
            state,
            enabled: params.get_bool("enabled")?,
            pattern: params.get_string("pattern")?,
            runlevel: params.get_string("runlevel")?,
            sleep,
            use_systemctl: params.get_bool("use_systemctl")?,
            daemon_reload: params.get_bool_or("daemon_reload", false),
            daemon_reexec: params.get_bool_or("daemon_reexec", false),
            arguments,
        })
    }

    /// Check if the service name contains wildcards
    fn has_pattern(&self) -> bool {
        self.name.contains('*') || self.name.contains('?') || self.name.contains('[')
    }

    /// The requested outcome that cannot be met without an existing unit, if any
    ///
    /// Starting, restarting, reloading and enabling need a unit to act on, while
    /// `stopped`, `enabled=false` and status-only calls hold when nothing exists.
    fn outcome_requiring_service(&self) -> Option<&'static str> {
        match self.state {
            Some(ServiceState::Started) => Some("state=started"),
            Some(ServiceState::Restarted) => Some("state=restarted"),
            Some(ServiceState::Reloaded) => Some("state=reloaded"),
            Some(ServiceState::Stopped) | None => {
                (self.enabled == Some(true)).then_some("enabled=true")
            }
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
                escalate_password: context.become_password.clone(),
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
        connection
            .execute(command, options)
            .await
            .map_err(|e| ModuleError::ExecutionFailed(format!("Connection execute failed: {}", e)))
    }

    /// Expand service pattern to list of matching services (systemd only)
    async fn expand_service_pattern(
        connection: &dyn Connection,
        pattern: &str,
        context: &ModuleContext,
    ) -> ModuleResult<Vec<String>> {
        // Use systemctl list-units to find matching services
        let cmd = format!(
            "systemctl list-units --type=service --all --no-legend --no-pager {} | awk '{{print $1}}'",
            shell_escape(pattern)
        );
        let result = Self::execute_command(connection, &cmd, context).await?;

        if result.success {
            let services: Vec<String> = result
                .stdout
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(services)
        } else {
            // Fallback: try list-unit-files for services that might not be loaded
            let cmd = format!(
                "systemctl list-unit-files --type=service --no-legend --no-pager {} | awk '{{print $1}}'",
                shell_escape(pattern)
            );
            let result = Self::execute_command(connection, &cmd, context).await?;

            let services: Vec<String> = result
                .stdout
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(services)
        }
    }

    /// Check if process matching pattern is running
    async fn check_pattern_running(
        connection: &dyn Connection,
        pattern: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        // Use pgrep for pattern matching
        let cmd = format!(
            "pgrep -f {} >/dev/null 2>&1 && echo yes || echo no",
            shell_escape(pattern)
        );
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.stdout.trim() == "yes")
    }

    /// Sleep for specified duration (async-friendly)
    async fn sleep_seconds(seconds: u64) {
        tokio::time::sleep(Duration::from_secs(seconds)).await;
    }

    /// Check if service is active (systemd)
    async fn systemd_is_active(
        connection: &dyn Connection,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let cmd = format!("systemctl is-active {}", shell_escape(service));
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.success)
    }

    /// Check if service is enabled (systemd)
    async fn systemd_is_enabled(
        connection: &dyn Connection,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let cmd = format!("systemctl is-enabled {}", shell_escape(service));
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
        let cmd = format!("systemctl {} {}", action, shell_escape(service));
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
        let cmd = format!("service {} status", shell_escape(service));
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.success)
    }

    /// Execute a SysV action
    #[allow(dead_code)]
    async fn sysv_action(
        connection: &dyn Connection,
        service: &str,
        action: &str,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        let cmd = format!("service {} {}", shell_escape(service), action);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok((result.success, result.stdout, result.stderr))
    }

    /// Check if service is active (OpenRC)
    async fn openrc_is_active(
        connection: &dyn Connection,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let cmd = format!("rc-service {} status", shell_escape(service));
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
        let cmd = format!("rc-service {} {}", shell_escape(service), action);
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok((result.success, result.stdout, result.stderr))
    }

    /// Check if service is enabled (OpenRC)
    async fn openrc_is_enabled(
        connection: &dyn Connection,
        service: &str,
        runlevel: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let rl = runlevel.unwrap_or("default");
        let cmd = format!(
            "rc-update show {} 2>/dev/null | grep -q {} && echo yes || echo no",
            shell_escape(rl),
            shell_escape(service)
        );
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.stdout.trim() == "yes")
    }

    /// Enable/disable service (OpenRC)
    async fn openrc_enable(
        connection: &dyn Connection,
        service: &str,
        enable: bool,
        runlevel: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        let rl = runlevel.unwrap_or("default");
        let action = if enable { "add" } else { "del" };
        let cmd = format!(
            "rc-update {} {} {}",
            action,
            shell_escape(service),
            shell_escape(rl)
        );
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok((result.success, result.stdout, result.stderr))
    }

    /// Check if service is enabled (SysV)
    async fn sysv_is_enabled(
        connection: &dyn Connection,
        service: &str,
        runlevel: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        // Try chkconfig first (RHEL/CentOS)
        let cmd = "command -v chkconfig >/dev/null 2>&1 && echo yes || echo no";
        let has_chkconfig = Self::execute_command(connection, cmd, context).await?;

        if has_chkconfig.stdout.trim() == "yes" {
            let cmd = format!(
                "chkconfig --list {} 2>/dev/null | grep -E ':on' && echo yes || echo no",
                shell_escape(service)
            );
            let result = Self::execute_command(connection, &cmd, context).await?;
            return Ok(result.stdout.contains("yes"));
        }

        // Try update-rc.d / invoke-rc.d (Debian/Ubuntu)
        let cmd = "command -v update-rc.d >/dev/null 2>&1 && echo yes || echo no";
        let has_updaterc = Self::execute_command(connection, cmd, context).await?;

        if has_updaterc.stdout.trim() == "yes" {
            // Check for runlevel symlinks
            let rl = runlevel.unwrap_or("2");
            let cmd = format!(
                "test -f /etc/rc{}.d/S??{} && echo yes || echo no",
                shell_escape(rl),
                shell_escape(service)
            );
            let result = Self::execute_command(connection, &cmd, context).await?;
            return Ok(result.stdout.trim() == "yes");
        }

        // Fallback: check for any S* symlink in rcN.d directories
        let cmd = format!(
            "ls /etc/rc*.d/S??{} 2>/dev/null | head -1 | grep -q . && echo yes || echo no",
            shell_escape(service)
        );
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.stdout.trim() == "yes")
    }

    /// Enable/disable service (SysV)
    async fn sysv_enable(
        connection: &dyn Connection,
        service: &str,
        enable: bool,
        runlevel: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        // Try chkconfig first (RHEL/CentOS)
        let cmd = "command -v chkconfig >/dev/null 2>&1 && echo yes || echo no";
        let has_chkconfig = Self::execute_command(connection, cmd, context).await?;

        if has_chkconfig.stdout.trim() == "yes" {
            let action = if enable { "on" } else { "off" };
            let cmd = if let Some(rl) = runlevel {
                format!(
                    "chkconfig --level {} {} {}",
                    shell_escape(rl),
                    shell_escape(service),
                    action
                )
            } else {
                format!("chkconfig {} {}", shell_escape(service), action)
            };
            let result = Self::execute_command(connection, &cmd, context).await?;
            return Ok((result.success, result.stdout, result.stderr));
        }

        // Try update-rc.d (Debian/Ubuntu)
        let cmd = "command -v update-rc.d >/dev/null 2>&1 && echo yes || echo no";
        let has_updaterc = Self::execute_command(connection, cmd, context).await?;

        if has_updaterc.stdout.trim() == "yes" {
            let cmd = if enable {
                format!("update-rc.d {} defaults", shell_escape(service))
            } else {
                format!("update-rc.d -f {} remove", shell_escape(service))
            };
            let result = Self::execute_command(connection, &cmd, context).await?;
            return Ok((result.success, result.stdout, result.stderr));
        }

        Err(ModuleError::Unsupported(
            "No supported service enable/disable tool found (chkconfig or update-rc.d)".to_string(),
        ))
    }

    /// Execute Upstart action
    async fn upstart_action(
        connection: &dyn Connection,
        service: &str,
        action: &str,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        let cmd = format!("initctl {} {}", action, shell_escape(service));
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok((result.success, result.stdout, result.stderr))
    }

    /// Check if Upstart service is active
    async fn upstart_is_active(
        connection: &dyn Connection,
        service: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let cmd = format!(
            "initctl status {} 2>/dev/null | grep -q 'running' && echo yes || echo no",
            shell_escape(service)
        );
        let result = Self::execute_command(connection, &cmd, context).await?;
        Ok(result.stdout.trim() == "yes")
    }

    /// Re-execute systemd manager
    async fn systemd_daemon_reexec(
        connection: &dyn Connection,
        context: &ModuleContext,
    ) -> ModuleResult<()> {
        let result = Self::execute_command(connection, "systemctl daemon-reexec", context).await?;
        if result.success {
            Ok(())
        } else {
            Err(ModuleError::ExecutionFailed(format!(
                "Failed to re-execute systemd daemon: {}",
                result.stderr
            )))
        }
    }

    /// Check if service is active for any init system
    async fn is_active(
        connection: &dyn Connection,
        init: &InitSystem,
        service: &str,
        pattern: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        // First try init-system-specific status check
        let result = match init {
            InitSystem::Systemd => Self::systemd_is_active(connection, service, context).await,
            InitSystem::SysV => Self::sysv_is_active(connection, service, context).await,
            InitSystem::OpenRC => Self::openrc_is_active(connection, service, context).await,
            InitSystem::Upstart => Self::upstart_is_active(connection, service, context).await,
            InitSystem::Launchd => Err(ModuleError::Unsupported(
                "Launchd not fully supported yet".to_string(),
            )),
        };

        // If we have a pattern and the service status check failed or returned false,
        // try pattern matching as fallback
        if let Some(pat) = pattern {
            match &result {
                Ok(false) | Err(_) => {
                    return Self::check_pattern_running(connection, pat, context).await;
                }
                Ok(true) => return result,
            }
        }

        result
    }

    /// Check if service is enabled for any init system
    async fn is_enabled(
        connection: &dyn Connection,
        init: &InitSystem,
        service: &str,
        runlevel: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        match init {
            InitSystem::Systemd => Self::systemd_is_enabled(connection, service, context).await,
            InitSystem::SysV => Self::sysv_is_enabled(connection, service, runlevel, context).await,
            InitSystem::OpenRC => {
                Self::openrc_is_enabled(connection, service, runlevel, context).await
            }
            InitSystem::Upstart => {
                // Upstart services are enabled by default if their .conf exists
                let cmd = format!(
                    "test -f /etc/init/{}.conf && echo yes || echo no",
                    shell_escape(service)
                );
                let result = Self::execute_command(connection, &cmd, context).await?;
                Ok(result.stdout.trim() == "yes")
            }
            InitSystem::Launchd => Err(ModuleError::Unsupported(
                "Launchd enable check not supported yet".to_string(),
            )),
        }
    }

    /// Enable or disable service for any init system
    async fn set_enabled(
        connection: &dyn Connection,
        init: &InitSystem,
        service: &str,
        enable: bool,
        runlevel: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        match init {
            InitSystem::Systemd => {
                let action = if enable { "enable" } else { "disable" };
                Self::systemd_action(connection, service, action, context).await
            }
            InitSystem::SysV => {
                Self::sysv_enable(connection, service, enable, runlevel, context).await
            }
            InitSystem::OpenRC => {
                Self::openrc_enable(connection, service, enable, runlevel, context).await
            }
            InitSystem::Upstart => {
                // Upstart uses override files to disable services
                if enable {
                    let cmd = format!("rm -f /etc/init/{}.override", shell_escape(service));
                    let result = Self::execute_command(connection, &cmd, context).await?;
                    Ok((result.success, result.stdout, result.stderr))
                } else {
                    let cmd = format!(
                        "echo 'manual' > /etc/init/{}.override",
                        shell_escape(service)
                    );
                    let result = Self::execute_command(connection, &cmd, context).await?;
                    Ok((result.success, result.stdout, result.stderr))
                }
            }
            InitSystem::Launchd => Err(ModuleError::Unsupported(
                "Launchd enable/disable not supported yet".to_string(),
            )),
        }
    }

    /// Execute a service action for any init system
    async fn service_action(
        connection: &dyn Connection,
        init: &InitSystem,
        service: &str,
        action: &str,
        arguments: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        match init {
            InitSystem::Systemd => {
                let cmd = if let Some(args) = arguments {
                    // arguments are already validated via validate_command_args in ServiceConfig::from_params
                    format!(
                        "systemctl {} {} {}",
                        action,
                        shell_escape(service),
                        args.trim()
                    )
                } else {
                    format!("systemctl {} {}", action, shell_escape(service))
                };
                let result = Self::execute_command(connection, &cmd, context).await?;
                Ok((result.success, result.stdout, result.stderr))
            }
            InitSystem::SysV => {
                let cmd = if let Some(args) = arguments {
                    format!(
                        "service {} {} {}",
                        shell_escape(service),
                        action,
                        args.trim()
                    )
                } else {
                    format!("service {} {}", shell_escape(service), action)
                };
                let result = Self::execute_command(connection, &cmd, context).await?;
                Ok((result.success, result.stdout, result.stderr))
            }
            InitSystem::OpenRC => Self::openrc_action(connection, service, action, context).await,
            InitSystem::Upstart => Self::upstart_action(connection, service, action, context).await,
            InitSystem::Launchd => Err(ModuleError::Unsupported(
                "Launchd actions not fully supported yet".to_string(),
            )),
        }
    }

    /// Perform restart with optional sleep between stop and start
    async fn restart_with_sleep(
        connection: &dyn Connection,
        init: &InitSystem,
        service: &str,
        sleep_secs: Option<u64>,
        arguments: Option<&str>,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        if let Some(secs) = sleep_secs {
            // Stop, sleep, start
            let (stop_ok, stop_out, stop_err) =
                Self::service_action(connection, init, service, "stop", arguments, context).await?;

            if !stop_ok {
                return Ok((false, stop_out, stop_err));
            }

            Self::sleep_seconds(secs).await;

            let (start_ok, start_out, start_err) =
                Self::service_action(connection, init, service, "start", arguments, context)
                    .await?;

            Ok((
                start_ok,
                format!("{}\n{}", stop_out, start_out),
                format!("{}\n{}", stop_err, start_err),
            ))
        } else {
            // Use native restart
            Self::service_action(connection, init, service, "restart", arguments, context).await
        }
    }

    /// Execute the service module with async connection
    async fn execute_async(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
        connection: Arc<dyn Connection + Send + Sync>,
    ) -> ModuleResult<ModuleOutput> {
        let config = ServiceConfig::from_params(params)?;

        // Detect init system
        let init = InitSystem::detect_async(connection.as_ref(), config.use_systemctl)
            .await
            .ok_or_else(|| {
                ModuleError::ExecutionFailed("Could not detect init system".to_string())
            })?;

        // Handle wildcard patterns (systemd only)
        if config.has_pattern() {
            if init != InitSystem::Systemd {
                return Err(ModuleError::InvalidParameter(
                    "Service name patterns (wildcards) are only supported with systemd".to_string(),
                ));
            }
            return self
                .execute_pattern_async(&config, context, connection, &init)
                .await;
        }

        self.execute_single_service_async(&config, context, connection, &init)
            .await
    }

    /// Execute module for a single service
    async fn execute_single_service_async(
        &self,
        config: &ServiceConfig,
        context: &ModuleContext,
        connection: Arc<dyn Connection + Send + Sync>,
        init: &InitSystem,
    ) -> ModuleResult<ModuleOutput> {
        let mut changed = false;
        self.apply_single_service(config, context, connection, init, &mut changed)
            .await
    }

    /// Apply the requested state to one service
    ///
    /// `changed` records whether the service was modified so far, so a caller
    /// still learns about a partial change (for example a unit enabled before it
    /// failed to start) when this returns an error.
    async fn apply_single_service(
        &self,
        config: &ServiceConfig,
        context: &ModuleContext,
        connection: Arc<dyn Connection + Send + Sync>,
        init: &InitSystem,
        changed: &mut bool,
    ) -> ModuleResult<ModuleOutput> {
        let service = &config.name;
        let mut messages = Vec::new();

        // Handle daemon-reexec for systemd (must come before daemon-reload)
        if config.daemon_reexec && *init == InitSystem::Systemd {
            if context.check_mode {
                messages.push("Would re-execute systemd daemon".to_string());
            } else {
                Self::systemd_daemon_reexec(connection.as_ref(), context).await?;
                messages.push("Re-executed systemd daemon".to_string());
                *changed = true;
            }
        }

        // Handle daemon-reload for systemd
        if config.daemon_reload && *init == InitSystem::Systemd {
            if context.check_mode {
                messages.push("Would reload systemd daemon".to_string());
            } else {
                Self::systemd_daemon_reload(connection.as_ref(), context).await?;
                messages.push("Reloaded systemd daemon".to_string());
                *changed = true;
            }
        }

        // Handle enabled state (now supports all init systems)
        if let Some(should_enable) = config.enabled {
            let runlevel = config.runlevel.as_deref();
            let is_enabled =
                Self::is_enabled(connection.as_ref(), init, service, runlevel, context)
                    .await
                    .unwrap_or(false);

            if should_enable != is_enabled {
                if context.check_mode {
                    let action = if should_enable { "enable" } else { "disable" };
                    messages.push(format!("Would {} service '{}'", action, service));
                    *changed = true;
                } else {
                    let action_word = if should_enable { "enable" } else { "disable" };
                    let (success, _, stderr) = Self::set_enabled(
                        connection.as_ref(),
                        init,
                        service,
                        should_enable,
                        runlevel,
                        context,
                    )
                    .await?;

                    if !success {
                        return Err(ModuleError::ExecutionFailed(format!(
                            "Failed to {} service '{}': {}",
                            action_word, service, stderr
                        )));
                    }

                    messages.push(format!("{}d service '{}'", action_word, service));
                    *changed = true;
                }
            }
        }

        // Handle state
        if let Some(ref desired_state) = config.state {
            let pattern = config.pattern.as_deref();
            let is_active =
                Self::is_active(connection.as_ref(), init, service, pattern, context).await?;

            match desired_state {
                ServiceState::Started => {
                    if !is_active {
                        if context.check_mode {
                            messages.push(format!("Would start service '{}'", service));
                            *changed = true;
                        } else {
                            let (success, _, stderr) = Self::service_action(
                                connection.as_ref(),
                                init,
                                service,
                                "start",
                                config.arguments.as_deref(),
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
                            *changed = true;
                        }
                    } else {
                        messages.push(format!("Service '{}' is already running", service));
                    }
                }

                ServiceState::Stopped => {
                    if is_active {
                        if context.check_mode {
                            messages.push(format!("Would stop service '{}'", service));
                            *changed = true;
                        } else {
                            let (success, _, stderr) = Self::service_action(
                                connection.as_ref(),
                                init,
                                service,
                                "stop",
                                config.arguments.as_deref(),
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
                            *changed = true;
                        }
                    } else {
                        messages.push(format!("Service '{}' is already stopped", service));
                    }
                }

                ServiceState::Restarted => {
                    if context.check_mode {
                        messages.push(format!("Would restart service '{}'", service));
                        *changed = true;
                    } else {
                        let (success, _, stderr) = Self::restart_with_sleep(
                            connection.as_ref(),
                            init,
                            service,
                            config.sleep,
                            config.arguments.as_deref(),
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
                        *changed = true;
                    }
                }

                ServiceState::Reloaded => {
                    if context.check_mode {
                        messages.push(format!("Would reload service '{}'", service));
                        *changed = true;
                    } else {
                        let (success, _, _stderr) = Self::service_action(
                            connection.as_ref(),
                            init,
                            service,
                            "reload",
                            config.arguments.as_deref(),
                            context,
                        )
                        .await?;

                        if !success {
                            // Try reload-or-restart as fallback for systemd
                            if *init == InitSystem::Systemd {
                                let (success2, _, stderr2) = Self::systemd_action(
                                    connection.as_ref(),
                                    service,
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
                                // For non-systemd, try restart as fallback
                                let (success2, _, stderr2) = Self::service_action(
                                    connection.as_ref(),
                                    init,
                                    service,
                                    "restart",
                                    config.arguments.as_deref(),
                                    context,
                                )
                                .await?;
                                if !success2 {
                                    return Err(ModuleError::ExecutionFailed(format!(
                                        "Failed to reload service '{}' (restart fallback also failed): {}",
                                        service, stderr2
                                    )));
                                }
                                messages.push(format!(
                                    "Restarted service '{}' (reload not supported)",
                                    service
                                ));
                                *changed = true;
                                // Skip the normal reload message
                                return self
                                    .build_output(
                                        service,
                                        init,
                                        connection.as_ref(),
                                        context,
                                        *changed,
                                        messages,
                                    )
                                    .await;
                            }
                        }

                        messages.push(format!("Reloaded service '{}'", service));
                        *changed = true;
                    }
                }
            }
        }

        self.build_output(
            service,
            init,
            connection.as_ref(),
            context,
            *changed,
            messages,
        )
        .await
    }

    /// Execute module for pattern/wildcard service names
    ///
    /// Each matched unit runs through `apply_single_service`; the module docs
    /// describe how per-unit failures and empty matches are reported.
    async fn execute_pattern_async(
        &self,
        config: &ServiceConfig,
        context: &ModuleContext,
        connection: Arc<dyn Connection + Send + Sync>,
        init: &InitSystem,
    ) -> ModuleResult<ModuleOutput> {
        let services =
            Self::expand_service_pattern(connection.as_ref(), &config.name, context).await?;

        if services.is_empty() {
            let output = match config.outcome_requiring_service() {
                Some(outcome) => ModuleOutput::failed(format!(
                    "No services matched pattern '{}', but {} needs at least one matching unit",
                    config.name, outcome
                )),
                None => ModuleOutput::ok(format!("No services matched pattern '{}'", config.name)),
            };
            return Ok(Self::with_pattern_data(output, &config.name, Vec::new()));
        }

        let mut total_changed = false;
        let mut failed_count = 0usize;
        let mut all_messages = Vec::new();
        let mut service_results = Vec::new();

        for service in &services {
            // Create a config for this specific service
            let service_config = ServiceConfig {
                name: service.clone(),
                state: config.state.clone(),
                enabled: config.enabled,
                pattern: config.pattern.clone(),
                runlevel: config.runlevel.clone(),
                sleep: config.sleep,
                use_systemctl: config.use_systemctl,
                daemon_reload: false, // Only do once
                daemon_reexec: false, // Only do once
                arguments: config.arguments.clone(),
            };

            let mut unit_changed = false;
            match self
                .apply_single_service(
                    &service_config,
                    context,
                    connection.clone(),
                    init,
                    &mut unit_changed,
                )
                .await
            {
                Ok(output) => {
                    if output.changed {
                        total_changed = true;
                    }
                    all_messages.push(format!("{}: {}", service, output.msg));
                    service_results.push(serde_json::json!({
                        "name": service,
                        "changed": output.changed,
                        "message": output.msg
                    }));
                }
                Err(e) => {
                    // A unit can be modified before a later step on it fails,
                    // for example enabled and then unable to start
                    failed_count += 1;
                    total_changed |= unit_changed;
                    all_messages.push(format!("{}: FAILED - {}", service, e));
                    service_results.push(serde_json::json!({
                        "name": service,
                        "changed": unit_changed,
                        "failed": true,
                        "error": e.to_string()
                    }));
                }
            }
        }

        let details = all_messages.join("; ");
        let output = if failed_count > 0 {
            let mut output = ModuleOutput::failed(format!(
                "{} of {} services matching '{}' failed: {}",
                failed_count,
                services.len(),
                config.name,
                details
            ));
            // Units changed before the failure are still reported as changed
            output.changed = total_changed;
            output
        } else {
            let msg = format!(
                "Processed {} services matching '{}': {}",
                services.len(),
                config.name,
                details
            );
            if total_changed {
                ModuleOutput::changed(msg)
            } else {
                ModuleOutput::ok(msg)
            }
        };

        Ok(Self::with_pattern_data(
            output,
            &config.name,
            service_results,
        ))
    }

    /// Attach the per-unit results of a pattern run to the output
    fn with_pattern_data(
        output: ModuleOutput,
        pattern: &str,
        service_results: Vec<serde_json::Value>,
    ) -> ModuleOutput {
        let matched_count = service_results.len();
        output
            .with_data("services", serde_json::Value::Array(service_results))
            .with_data("pattern", serde_json::json!(pattern))
            .with_data("matched_count", serde_json::json!(matched_count))
    }

    /// Build the output with current service status
    async fn build_output(
        &self,
        service: &str,
        init: &InitSystem,
        connection: &dyn Connection,
        context: &ModuleContext,
        changed: bool,
        messages: Vec<String>,
    ) -> ModuleResult<ModuleOutput> {
        let msg = if messages.is_empty() {
            format!("Service '{}' is in desired state", service)
        } else {
            messages.join(". ")
        };

        // Get current status for output
        let status = match init {
            InitSystem::Systemd => {
                let is_active = Self::systemd_is_active(connection, service, context)
                    .await
                    .unwrap_or(false);
                let is_enabled = Self::systemd_is_enabled(connection, service, context)
                    .await
                    .unwrap_or(false);
                serde_json::json!({
                    "active": is_active,
                    "enabled": is_enabled,
                    "init_system": "systemd"
                })
            }
            _ => {
                let is_active = Self::is_active(connection, init, service, None, context)
                    .await
                    .unwrap_or(false);
                let is_enabled = Self::is_enabled(connection, init, service, None, context)
                    .await
                    .unwrap_or(false);
                serde_json::json!({
                    "active": is_active,
                    "enabled": is_enabled,
                    "init_system": format!("{:?}", init).to_lowercase()
                })
            }
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
        let config = ServiceConfig::from_params(params)?;

        let init = match InitSystem::detect_async(connection.as_ref(), config.use_systemctl).await {
            Some(i) => i,
            None => return Ok(None),
        };

        // Pattern matching not supported in diff
        if config.has_pattern() {
            return Ok(None);
        }

        let service = &config.name;
        let pattern = config.pattern.as_deref();
        let runlevel = config.runlevel.as_deref();

        let is_active = Self::is_active(connection.as_ref(), &init, service, pattern, context)
            .await
            .unwrap_or(false);
        let is_enabled = Self::is_enabled(connection.as_ref(), &init, service, runlevel, context)
            .await
            .unwrap_or(false);

        let mut before_lines = Vec::new();
        let mut after_lines = Vec::new();

        // Init system info
        before_lines.push(format!("init_system: {:?}", init).to_lowercase());
        after_lines.push(format!("init_system: {:?}", init).to_lowercase());

        // Active state
        before_lines.push(format!("active: {}", if is_active { "yes" } else { "no" }));

        if let Some(ref desired_state) = config.state {
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

        // Enabled state (now supports all init systems)
        before_lines.push(format!(
            "enabled: {}",
            if is_enabled { "yes" } else { "no" }
        ));

        if let Some(should_enable) = config.enabled {
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

        // Spawn blocking task on a separate thread to avoid runtime nesting issues
        let params = params.clone();
        let context = context.clone();
        let module = self;
        std::thread::scope(|s| {
            s.spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| {
                        ModuleError::ExecutionFailed(format!("Failed to create runtime: {}", e))
                    })?;

                rt.block_on(module.execute_async(&params, &context, connection))
            })
            .join()
            .map_err(|_| ModuleError::ExecutionFailed("Thread panicked".to_string()))?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{ConnectionResult, FileStat, TransferOptions};
    use crate::modules::ModuleStatus;
    use async_trait::async_trait;
    use std::path::Path;
    use std::sync::Mutex;

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

    #[test]
    fn test_service_config_from_params() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx"));
        params.insert("state".to_string(), serde_json::json!("started"));
        params.insert("enabled".to_string(), serde_json::json!(true));

        let config = ServiceConfig::from_params(&params).unwrap();
        assert_eq!(config.name, "nginx");
        assert_eq!(config.state, Some(ServiceState::Started));
        assert_eq!(config.enabled, Some(true));
        assert_eq!(config.pattern, None);
        assert_eq!(config.runlevel, None);
        assert_eq!(config.sleep, None);
    }

    #[test]
    fn test_service_config_with_all_params() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("httpd"));
        params.insert("state".to_string(), serde_json::json!("restarted"));
        params.insert("enabled".to_string(), serde_json::json!(true));
        params.insert("pattern".to_string(), serde_json::json!("httpd"));
        params.insert("runlevel".to_string(), serde_json::json!("2345"));
        params.insert("sleep".to_string(), serde_json::json!(5));
        params.insert("use_systemctl".to_string(), serde_json::json!(true));
        params.insert("daemon_reload".to_string(), serde_json::json!(true));
        params.insert("daemon_reexec".to_string(), serde_json::json!(false));
        params.insert("arguments".to_string(), serde_json::json!("--no-block"));

        let config = ServiceConfig::from_params(&params).unwrap();
        assert_eq!(config.name, "httpd");
        assert_eq!(config.state, Some(ServiceState::Restarted));
        assert_eq!(config.enabled, Some(true));
        assert_eq!(config.pattern, Some("httpd".to_string()));
        assert_eq!(config.runlevel, Some("2345".to_string()));
        assert_eq!(config.sleep, Some(5));
        assert_eq!(config.use_systemctl, Some(true));
        assert!(config.daemon_reload);
        assert!(!config.daemon_reexec);
        assert_eq!(config.arguments, Some("--no-block".to_string()));
    }

    #[test]
    fn test_service_config_pattern_detection() {
        let mut params = ModuleParams::new();

        // No pattern
        params.insert("name".to_string(), serde_json::json!("nginx"));
        let config = ServiceConfig::from_params(&params).unwrap();
        assert!(!config.has_pattern());

        // Wildcard pattern
        params.insert("name".to_string(), serde_json::json!("nginx*"));
        let config = ServiceConfig::from_params(&params).unwrap();
        assert!(config.has_pattern());

        // Question mark pattern
        params.insert("name".to_string(), serde_json::json!("nginx?"));
        let config = ServiceConfig::from_params(&params).unwrap();
        assert!(config.has_pattern());

        // Bracket pattern
        params.insert("name".to_string(), serde_json::json!("nginx[0-9]"));
        let config = ServiceConfig::from_params(&params).unwrap();
        assert!(config.has_pattern());
    }

    #[test]
    fn test_service_config_negative_sleep_rejected() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx"));
        params.insert("sleep".to_string(), serde_json::json!(-1));

        let result = ServiceConfig::from_params(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_service_config_invalid_args_rejected() {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!("nginx"));
        params.insert(
            "arguments".to_string(),
            serde_json::json!("--foo; rm -rf /"),
        );

        let result = ServiceConfig::from_params(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_init_system_service_command() {
        assert_eq!(InitSystem::Systemd.service_command(), "systemctl");
        assert_eq!(InitSystem::SysV.service_command(), "service");
        assert_eq!(InitSystem::OpenRC.service_command(), "rc-service");
        assert_eq!(InitSystem::Upstart.service_command(), "initctl");
        assert_eq!(InitSystem::Launchd.service_command(), "launchctl");
    }

    type Responder = Box<dyn Fn(&str) -> CommandResult + Send + Sync>;

    /// Connection that answers commands from a closure and records what ran
    struct MockConnection {
        respond: Responder,
        commands: Mutex<Vec<String>>,
    }

    impl MockConnection {
        fn new(respond: impl Fn(&str) -> CommandResult + Send + Sync + 'static) -> Arc<Self> {
            Arc::new(Self {
                respond: Box::new(respond),
                commands: Mutex::new(Vec::new()),
            })
        }

        fn commands(&self) -> Vec<String> {
            self.commands.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Connection for MockConnection {
        fn identifier(&self) -> &str {
            "mock"
        }

        async fn is_alive(&self) -> bool {
            true
        }

        async fn execute(
            &self,
            command: &str,
            _options: Option<ExecuteOptions>,
        ) -> ConnectionResult<CommandResult> {
            self.commands.lock().unwrap().push(command.to_string());
            Ok((self.respond)(command))
        }

        async fn upload(
            &self,
            _local: &Path,
            _remote: &Path,
            _opts: Option<TransferOptions>,
        ) -> ConnectionResult<()> {
            unreachable!("the service module never transfers files")
        }

        async fn upload_content(
            &self,
            _content: &[u8],
            _remote: &Path,
            _opts: Option<TransferOptions>,
        ) -> ConnectionResult<()> {
            unreachable!("the service module never transfers files")
        }

        async fn download(&self, _remote: &Path, _local: &Path) -> ConnectionResult<()> {
            unreachable!("the service module never transfers files")
        }

        async fn download_content(&self, _remote: &Path) -> ConnectionResult<Vec<u8>> {
            unreachable!("the service module never transfers files")
        }

        async fn path_exists(&self, _path: &Path) -> ConnectionResult<bool> {
            unreachable!("the service module never inspects paths")
        }

        async fn is_directory(&self, _path: &Path) -> ConnectionResult<bool> {
            unreachable!("the service module never inspects paths")
        }

        async fn stat(&self, _path: &Path) -> ConnectionResult<FileStat> {
            unreachable!("the service module never inspects paths")
        }

        async fn close(&self) -> ConnectionResult<()> {
            Ok(())
        }
    }

    const PATTERN: &str = "php*-fpm.service";

    /// Answer the systemd probe and the `systemctl` queries as a host whose
    /// matching units are `units` (one per line), all inactive and disabled
    fn respond_systemd(cmd: &str, units: &str) -> Option<CommandResult> {
        if cmd.contains("grep -q systemd") {
            Some(CommandResult::success("yes\n".to_string(), String::new()))
        } else if cmd.contains("systemctl list-units") {
            Some(CommandResult::success(units.to_string(), String::new()))
        } else if cmd.contains("systemctl is-active") {
            Some(CommandResult::failure(
                3,
                "inactive\n".to_string(),
                String::new(),
            ))
        } else if cmd.contains("systemctl is-enabled") {
            Some(CommandResult::failure(
                1,
                "disabled\n".to_string(),
                String::new(),
            ))
        } else {
            None
        }
    }

    fn start_failure(unit: &str) -> CommandResult {
        CommandResult::failure(1, String::new(), format!("Job for {} failed", unit))
    }

    fn pattern_params(state: Option<&str>, enabled: Option<bool>) -> ModuleParams {
        let mut params = ModuleParams::new();
        params.insert("name".to_string(), serde_json::json!(PATTERN));
        if let Some(state) = state {
            params.insert("state".to_string(), serde_json::json!(state));
        }
        if let Some(enabled) = enabled {
            params.insert("enabled".to_string(), serde_json::json!(enabled));
        }
        params
    }

    async fn run_pattern(
        conn: Arc<MockConnection>,
        params: ModuleParams,
        check_mode: bool,
    ) -> ModuleOutput {
        let context = ModuleContext::new()
            .with_check_mode(check_mode)
            .with_connection(conn.clone());
        ServiceModule
            .execute_async(&params, &context, conn)
            .await
            .expect("pattern runs report failures in the output, not as Err")
    }

    #[tokio::test]
    async fn test_pattern_matched_service_failure_fails_task() {
        let conn = MockConnection::new(|cmd| {
            respond_systemd(cmd, "php8.3-fpm.service\n").unwrap_or_else(|| {
                assert_eq!(cmd, "systemctl start php8.3-fpm.service");
                start_failure("php8.3-fpm.service")
            })
        });

        let output = run_pattern(conn.clone(), pattern_params(Some("started"), None), false).await;

        assert_eq!(output.status, ModuleStatus::Failed);
        assert!(!output.changed);
        assert!(
            output
                .msg
                .contains("1 of 1 services matching 'php*-fpm.service' failed"),
            "{}",
            output.msg
        );
        assert!(
            output.msg.contains(
                "php8.3-fpm.service: FAILED - Execution failed: Failed to start service \
                 'php8.3-fpm.service': Job for php8.3-fpm.service failed"
            ),
            "{}",
            output.msg
        );
        assert_eq!(output.data["pattern"], PATTERN);
        assert_eq!(output.data["matched_count"], 1);
        assert_eq!(output.data["services"][0]["name"], "php8.3-fpm.service");
        assert_eq!(output.data["services"][0]["changed"], false);
        assert_eq!(output.data["services"][0]["failed"], true);
        assert!(conn
            .commands()
            .contains(&"systemctl start php8.3-fpm.service".to_string()));
    }

    #[tokio::test]
    async fn test_pattern_partial_change_before_failure_is_reported() {
        // The unit gets enabled and then fails to start: that is still a change
        let conn = MockConnection::new(|cmd| {
            respond_systemd(cmd, "php8.3-fpm.service\n").unwrap_or_else(|| match cmd {
                "systemctl enable php8.3-fpm.service" => {
                    CommandResult::success(String::new(), String::new())
                }
                "systemctl start php8.3-fpm.service" => start_failure("php8.3-fpm.service"),
                other => panic!("unexpected command: {other}"),
            })
        });

        let output = run_pattern(
            conn.clone(),
            pattern_params(Some("started"), Some(true)),
            false,
        )
        .await;

        assert_eq!(output.status, ModuleStatus::Failed);
        assert!(output.changed, "the enable step already modified the unit");
        assert_eq!(output.data["services"][0]["changed"], true);
        assert_eq!(output.data["services"][0]["failed"], true);
        assert!(
            output
                .msg
                .contains("Failed to start service 'php8.3-fpm.service'"),
            "{}",
            output.msg
        );
        let commands = conn.commands();
        let enabled_at = commands
            .iter()
            .position(|c| c == "systemctl enable php8.3-fpm.service")
            .expect("the unit was enabled");
        let started_at = commands
            .iter()
            .position(|c| c == "systemctl start php8.3-fpm.service")
            .expect("the unit was started");
        assert!(enabled_at < started_at, "{commands:?}");
    }

    #[tokio::test]
    async fn test_pattern_no_match_policy() {
        // (state, enabled, check_mode, requested outcome that needs a unit)
        let cases = [
            (Some("started"), None, false, Some("state=started")),
            (Some("started"), None, true, Some("state=started")),
            (Some("restarted"), None, false, Some("state=restarted")),
            (Some("reloaded"), None, false, Some("state=reloaded")),
            (None, Some(true), false, Some("enabled=true")),
            (Some("stopped"), Some(true), false, Some("enabled=true")),
            (Some("stopped"), None, false, None),
            (Some("stopped"), Some(false), false, None),
            (None, Some(false), false, None),
            (None, None, false, None),
        ];

        for (state, enabled, check_mode, outcome) in cases {
            let case = format!("state={state:?} enabled={enabled:?} check_mode={check_mode}");
            let conn = MockConnection::new(|cmd| {
                respond_systemd(cmd, "")
                    .unwrap_or_else(|| panic!("no unit should be touched, got: {cmd}"))
            });

            let output =
                run_pattern(conn.clone(), pattern_params(state, enabled), check_mode).await;

            assert!(
                output
                    .msg
                    .contains("No services matched pattern 'php*-fpm.service'"),
                "{case}: {}",
                output.msg
            );
            assert!(!output.changed, "{case}");
            assert_eq!(output.data["matched_count"], 0, "{case}");
            assert_eq!(output.data["services"], serde_json::json!([]), "{case}");
            match outcome {
                Some(outcome) => {
                    assert_eq!(output.status, ModuleStatus::Failed, "{case}");
                    assert!(
                        output
                            .msg
                            .contains(&format!("but {outcome} needs at least one matching unit")),
                        "{case}: {}",
                        output.msg
                    );
                }
                None => assert_eq!(output.status, ModuleStatus::Ok, "{case}: {}", output.msg),
            }
            // Only the init probe and the pattern expansion ran
            assert_eq!(conn.commands().len(), 2, "{case}: {:?}", conn.commands());
        }
    }

    #[tokio::test]
    async fn test_pattern_mixed_results_fail_with_both_messages() {
        // The failing unit comes first, so the loop has to carry on past it
        let conn = MockConnection::new(|cmd| {
            respond_systemd(cmd, "php8.3-fpm.service\nphp8.2-fpm.service\n").unwrap_or_else(|| {
                match cmd {
                    "systemctl start php8.3-fpm.service" => start_failure("php8.3-fpm.service"),
                    "systemctl start php8.2-fpm.service" => {
                        CommandResult::success(String::new(), String::new())
                    }
                    other => panic!("unexpected command: {other}"),
                }
            })
        });

        let output = run_pattern(conn.clone(), pattern_params(Some("started"), None), false).await;

        assert_eq!(output.status, ModuleStatus::Failed);
        assert!(output.changed, "the unit that did start is still a change");
        assert!(
            output
                .msg
                .contains("1 of 2 services matching 'php*-fpm.service' failed"),
            "{}",
            output.msg
        );
        assert!(
            output.msg.contains(
                "php8.3-fpm.service: FAILED - Execution failed: Failed to start service \
                 'php8.3-fpm.service': Job for php8.3-fpm.service failed"
            ),
            "{}",
            output.msg
        );
        assert!(
            output
                .msg
                .contains("php8.2-fpm.service: Started service 'php8.2-fpm.service'"),
            "{}",
            output.msg
        );
        assert_eq!(output.data["matched_count"], 2);
        let services = &output.data["services"];
        assert_eq!(services[0]["name"], "php8.3-fpm.service");
        assert_eq!(services[0]["failed"], true);
        assert_eq!(services[1]["name"], "php8.2-fpm.service");
        assert_eq!(services[1]["changed"], true);
        assert_eq!(
            services[1]["message"],
            "Started service 'php8.2-fpm.service'"
        );

        let commands = conn.commands();
        let failed_at = commands
            .iter()
            .position(|c| c == "systemctl start php8.3-fpm.service")
            .expect("the failing unit was started");
        let started_at = commands
            .iter()
            .position(|c| c == "systemctl start php8.2-fpm.service")
            .expect("the remaining unit was still started");
        assert!(failed_at < started_at, "{commands:?}");
    }

    #[tokio::test]
    async fn test_pattern_all_matched_services_succeed() {
        let conn = MockConnection::new(|cmd| {
            respond_systemd(cmd, "php8.3-fpm.service\nphp8.2-fpm.service\n").unwrap_or_else(|| {
                assert!(
                    cmd.starts_with("systemctl start php8."),
                    "unexpected command: {cmd}"
                );
                CommandResult::success(String::new(), String::new())
            })
        });

        let output = run_pattern(conn, pattern_params(Some("started"), None), false).await;

        assert_eq!(output.status, ModuleStatus::Changed);
        assert!(output.changed);
        assert!(
            output
                .msg
                .starts_with("Processed 2 services matching 'php*-fpm.service': "),
            "{}",
            output.msg
        );
        assert_eq!(output.data["matched_count"], 2);
        let services = output.data["services"].as_array().expect("services array");
        assert_eq!(services.len(), 2);
        assert!(services
            .iter()
            .all(|s| s["changed"] == true && s.get("failed").is_none()));
    }
}
