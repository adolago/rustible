//! User module - User management
//!
//! This module manages user accounts on the system, supporting both local and remote execution.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

/// Desired state for a user
#[derive(Debug, Clone, PartialEq)]
pub enum UserState {
    Present,
    Absent,
}

impl UserState {
    fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "present" => Ok(UserState::Present),
            "absent" => Ok(UserState::Absent),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: present, absent",
                s
            ))),
        }
    }
}

/// Information about a user
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub comment: String,
    pub home: String,
    pub shell: String,
    pub groups: Vec<String>,
}

/// Module for user management
pub struct UserModule;

impl UserModule {
    fn user_exists(
        name: &str,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        if let Some(ref conn) = context.connection {
            // Remote execution via connection
            let result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    conn.execute(&format!("id {}", name), None).await
                })
            });

            match result {
                Ok(cmd_result) => Ok(cmd_result.success),
                Err(_) => Ok(false), // User doesn't exist
            }
        } else {
            // Local execution
            let output = Command::new("id")
                .arg(name)
                .output()
                .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to check user: {}", e)))?;

            Ok(output.status.success())
        }
    }

    fn get_user_info(
        name: &str,
        context: &ModuleContext,
    ) -> ModuleResult<Option<UserInfo>> {
        if let Some(ref conn) = context.connection {
            // Remote execution via connection
            let passwd_result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    conn.execute(&format!("getent passwd {}", name), None).await
                })
            });

            if let Ok(cmd_result) = passwd_result {
                if cmd_result.success {
                    let line = cmd_result.stdout.trim();
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 7 {
                        let uid = parts[2].parse().unwrap_or(0);
                        let gid = parts[3].parse().unwrap_or(0);

                        // Get groups
                        let groups_result = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                conn.execute(&format!("groups {}", name), None).await
                            })
                        });

                        let groups = groups_result
                            .ok()
                            .filter(|r| r.success)
                            .map(|r| {
                                r.stdout
                                    .split(':')
                                    .last()
                                    .unwrap_or("")
                                    .split_whitespace()
                                    .map(|s| s.to_string())
                                    .collect()
                            })
                            .unwrap_or_default();

                        return Ok(Some(UserInfo {
                            name: parts[0].to_string(),
                            uid,
                            gid,
                            comment: parts[4].to_string(),
                            home: parts[5].to_string(),
                            shell: parts[6].to_string(),
                            groups,
                        }));
                    }
                }
            }
            Ok(None)
        } else {
            // Local execution - read /etc/passwd
            let passwd = fs::read_to_string("/etc/passwd").map_err(|e| ModuleError::Io(e))?;

            for line in passwd.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 7 && parts[0] == name {
                    let uid = parts[2].parse().unwrap_or(0);
                    let gid = parts[3].parse().unwrap_or(0);

                    // Get groups
                    let groups_output = Command::new("groups").arg(name).output().ok();

                    let groups = groups_output
                        .map(|o| {
                            String::from_utf8_lossy(&o.stdout)
                                .split(':')
                                .last()
                                .unwrap_or("")
                                .split_whitespace()
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .unwrap_or_default();

                    return Ok(Some(UserInfo {
                        name: parts[0].to_string(),
                        uid,
                        gid,
                        comment: parts[4].to_string(),
                        home: parts[5].to_string(),
                        shell: parts[6].to_string(),
                        groups,
                    }));
                }
            }

            Ok(None)
        }
    }
}

impl Module for UserModule {
    fn name(&self) -> &'static str {
        "user"
    }

    fn description(&self) -> &'static str {
        "Manage user accounts"
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
        let name = params.get_string_required("name")?;
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());
        let state = UserState::from_str(&state_str)?;

        let user_exists = Self::user_exists(&name, context)?;

        match state {
            UserState::Absent => {
                if !user_exists {
                    return Ok(ModuleOutput::ok(format!("User '{}' already absent", name)));
                }

                if context.check_mode {
                    return Ok(ModuleOutput::changed(format!(
                        "Would remove user '{}'",
                        name
                    )));
                }

                // Delete user using connection.execute()
                let cmd_str = format!("userdel {}", name);
                if let Some(ref conn) = context.connection {
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            conn.execute(&cmd_str, None).await
                        })
                    })?;
                } else {
                    Command::new("userdel").arg(&name).output()?;
                }

                Ok(ModuleOutput::changed(format!("Removed user '{}'", name)))
            }

            UserState::Present => {
                let mut changed = false;

                if !user_exists {
                    if context.check_mode {
                        return Ok(ModuleOutput::changed(format!(
                            "Would create user '{}'",
                            name
                        )));
                    }

                    // Create user using connection.execute()
                    let cmd_str = format!("useradd -m {}", name);
                    if let Some(ref conn) = context.connection {
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                conn.execute(&cmd_str, None).await
                            })
                        })?;
                    } else {
                        Command::new("useradd").args(&["-m", &name]).output()?;
                    }

                    changed = true;
                }

                // Get final user info
                let user_info = Self::get_user_info(&name, context)?;
                let mut data = HashMap::new();

                if let Some(info) = user_info {
                    data.insert("uid".to_string(), serde_json::json!(info.uid));
                    data.insert("gid".to_string(), serde_json::json!(info.gid));
                    data.insert("home".to_string(), serde_json::json!(info.home));
                    data.insert("shell".to_string(), serde_json::json!(info.shell));
                    data.insert("groups".to_string(), serde_json::json!(info.groups));
                }

                let msg = if changed {
                    format!("Created user '{}'", name)
                } else {
                    format!("User '{}' is in desired state", name)
                };

                let mut output = if changed {
                    ModuleOutput::changed(msg)
                } else {
                    ModuleOutput::ok(msg)
                };

                for (k, v) in data {
                    output = output.with_data(k, v);
                }

                Ok(output)
            }
        }
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    fn diff(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let name = params.get_string_required("name")?;
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());
        let state = UserState::from_str(&state_str)?;

        let user_info = Self::get_user_info(&name, context)?;

        let before = if let Some(info) = &user_info {
            format!(
                "user: {}\nuid: {}\ngid: {}\nhome: {}\nshell: {}\ngroups: {}",
                info.name,
                info.uid,
                info.gid,
                info.home,
                info.shell,
                info.groups.join(",")
            )
        } else {
            "user: (absent)".to_string()
        };

        let after = match state {
            UserState::Absent => "user: (absent)".to_string(),
            UserState::Present => {
                if user_info.is_some() {
                    // Would need to compute differences based on params
                    before.clone()
                } else {
                    format!("user: {} (will be created)", name)
                }
            }
        };

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
    fn test_user_state_from_str() {
        assert_eq!(UserState::from_str("present").unwrap(), UserState::Present);
        assert_eq!(UserState::from_str("absent").unwrap(), UserState::Absent);
        assert!(UserState::from_str("invalid").is_err());
    }

    #[test]
    fn test_user_exists() {
        let context = ModuleContext::default();
        // root should always exist
        assert!(UserModule::user_exists("root", &context).unwrap());
        // Random user should not exist
        assert!(!UserModule::user_exists("nonexistent_user_12345", &context).unwrap());
    }

    #[test]
    fn test_get_user_info() {
        let context = ModuleContext::default();
        let info = UserModule::get_user_info("root", &context).unwrap();
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.name, "root");
        assert_eq!(info.uid, 0);
    }

    // Integration tests would require root access
}
