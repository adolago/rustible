//! User module - User management
//!
//! This module manages user accounts on the system.

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
    fn user_exists(name: &str) -> ModuleResult<bool> {
        let output = Command::new("id")
            .arg(name)
            .output()
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to check user: {}", e)))?;

        Ok(output.status.success())
    }

    fn get_user_info(name: &str) -> ModuleResult<Option<UserInfo>> {
        // Read /etc/passwd to get user info
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

    fn create_user(
        name: &str,
        uid: Option<u32>,
        group: Option<&str>,
        groups: Option<&[String]>,
        home: Option<&str>,
        shell: Option<&str>,
        comment: Option<&str>,
        create_home: bool,
        system: bool,
    ) -> ModuleResult<()> {
        let mut cmd = Command::new("useradd");

        if let Some(uid) = uid {
            cmd.args(["-u", &uid.to_string()]);
        }

        if let Some(group) = group {
            cmd.args(["-g", group]);
        }

        if let Some(groups) = groups {
            if !groups.is_empty() {
                cmd.args(["-G", &groups.join(",")]);
            }
        }

        if let Some(home) = home {
            cmd.args(["-d", home]);
        }

        if let Some(shell) = shell {
            cmd.args(["-s", shell]);
        }

        if let Some(comment) = comment {
            cmd.args(["-c", comment]);
        }

        if create_home {
            cmd.arg("-m");
        } else {
            cmd.arg("-M");
        }

        if system {
            cmd.arg("-r");
        }

        cmd.arg(name);

        let output = cmd
            .output()
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to create user: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(ModuleError::ExecutionFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn modify_user(
        name: &str,
        uid: Option<u32>,
        group: Option<&str>,
        groups: Option<&[String]>,
        append_groups: bool,
        home: Option<&str>,
        shell: Option<&str>,
        comment: Option<&str>,
        move_home: bool,
    ) -> ModuleResult<bool> {
        let current = Self::get_user_info(name)?
            .ok_or_else(|| ModuleError::ExecutionFailed(format!("User '{}' not found", name)))?;

        let mut needs_change = false;
        let mut cmd = Command::new("usermod");

        if let Some(uid) = uid {
            if current.uid != uid {
                cmd.args(["-u", &uid.to_string()]);
                needs_change = true;
            }
        }

        if let Some(group) = group {
            // Would need to look up group name to compare
            cmd.args(["-g", group]);
            needs_change = true;
        }

        if let Some(groups) = groups {
            if !groups.is_empty() {
                let groups_str = groups.join(",");
                if append_groups {
                    cmd.args(["-a", "-G", &groups_str]);
                } else {
                    cmd.args(["-G", &groups_str]);
                }
                needs_change = true;
            }
        }

        if let Some(home) = home {
            if current.home != home {
                cmd.args(["-d", home]);
                if move_home {
                    cmd.arg("-m");
                }
                needs_change = true;
            }
        }

        if let Some(shell) = shell {
            if current.shell != shell {
                cmd.args(["-s", shell]);
                needs_change = true;
            }
        }

        if let Some(comment) = comment {
            if current.comment != comment {
                cmd.args(["-c", comment]);
                needs_change = true;
            }
        }

        if !needs_change {
            return Ok(false);
        }

        cmd.arg(name);

        let output = cmd
            .output()
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to modify user: {}", e)))?;

        if output.status.success() {
            Ok(true)
        } else {
            Err(ModuleError::ExecutionFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn delete_user(name: &str, remove_home: bool, force: bool) -> ModuleResult<()> {
        let mut cmd = Command::new("userdel");

        if remove_home {
            cmd.arg("-r");
        }

        if force {
            cmd.arg("-f");
        }

        cmd.arg(name);

        let output = cmd
            .output()
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to delete user: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(ModuleError::ExecutionFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }

    fn set_password(name: &str, password: &str, encrypted: bool) -> ModuleResult<()> {
        use std::io::Write;
        use std::process::Stdio;

        if encrypted {
            // Use chpasswd with encrypted password
            let mut cmd = Command::new("chpasswd")
                .arg("-e")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to set password: {}", e))
                })?;

            if let Some(ref mut stdin) = cmd.stdin {
                writeln!(stdin, "{}:{}", name, password).map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to write password: {}", e))
                })?;
            }

            let output = cmd.wait_with_output().map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to set password: {}", e))
            })?;

            if !output.status.success() {
                return Err(ModuleError::ExecutionFailed(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }
        } else {
            // Use chpasswd with plain password
            let mut cmd = Command::new("chpasswd")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to set password: {}", e))
                })?;

            if let Some(ref mut stdin) = cmd.stdin {
                writeln!(stdin, "{}:{}", name, password).map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to write password: {}", e))
                })?;
            }

            let output = cmd.wait_with_output().map_err(|e| {
                ModuleError::ExecutionFailed(format!("Failed to set password: {}", e))
            })?;

            if !output.status.success() {
                return Err(ModuleError::ExecutionFailed(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }
        }

        Ok(())
    }

    fn generate_ssh_key(
        name: &str,
        ssh_key_type: &str,
        ssh_key_bits: u32,
        ssh_key_file: Option<&str>,
        ssh_key_comment: Option<&str>,
        ssh_key_passphrase: Option<&str>,
    ) -> ModuleResult<bool> {
        let user_info = Self::get_user_info(name)?
            .ok_or_else(|| ModuleError::ExecutionFailed(format!("User '{}' not found", name)))?;

        let key_file = ssh_key_file
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/.ssh/id_{}", user_info.home, ssh_key_type));

        // Check if key already exists
        if std::path::Path::new(&key_file).exists() {
            return Ok(false);
        }

        // Create .ssh directory if needed
        let ssh_dir = format!("{}/.ssh", user_info.home);
        if !std::path::Path::new(&ssh_dir).exists() {
            fs::create_dir_all(&ssh_dir)?;
            // Set ownership to user
            Command::new("chown")
                .args([&format!("{}:{}", user_info.uid, user_info.gid), &ssh_dir])
                .output()?;
            // Set permissions
            Command::new("chmod").args(["700", &ssh_dir]).output()?;
        }

        let mut cmd = Command::new("ssh-keygen");
        cmd.args(["-t", ssh_key_type]);
        cmd.args(["-b", &ssh_key_bits.to_string()]);
        cmd.args(["-f", &key_file]);

        if let Some(comment) = ssh_key_comment {
            cmd.args(["-C", comment]);
        }

        if let Some(passphrase) = ssh_key_passphrase {
            cmd.args(["-N", passphrase]);
        } else {
            cmd.args(["-N", ""]);
        }

        let output = cmd.output().map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to generate SSH key: {}", e))
        })?;

        if !output.status.success() {
            return Err(ModuleError::ExecutionFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        // Set ownership
        Command::new("chown")
            .args([
                &format!("{}:{}", user_info.uid, user_info.gid),
                &key_file,
                &format!("{}.pub", key_file),
            ])
            .output()?;

        // Set permissions
        Command::new("chmod").args(["600", &key_file]).output()?;
        Command::new("chmod")
            .args(["644", &format!("{}.pub", key_file)])
            .output()?;

        Ok(true)
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

        let uid = params.get_u32("uid")?;
        let group = params.get_string("group")?;
        let groups = params.get_vec_string("groups")?;
        let append_groups = params.get_bool_or("append", false);
        let home = params.get_string("home")?;
        let shell = params.get_string("shell")?;
        let comment = params.get_string("comment")?;
        let create_home = params.get_bool_or("create_home", true);
        let move_home = params.get_bool_or("move_home", false);
        let system = params.get_bool_or("system", false);
        let remove_home = params.get_bool_or("remove", false);
        let force = params.get_bool_or("force", false);
        let password = params.get_string("password")?;
        let password_encrypted = params.get_bool_or("password_encrypted", true);
        let generate_ssh_key = params.get_bool_or("generate_ssh_key", false);
        let ssh_key_type = params
            .get_string("ssh_key_type")?
            .unwrap_or_else(|| "rsa".to_string());
        let ssh_key_bits = params.get_u32("ssh_key_bits")?.unwrap_or(4096);
        let ssh_key_file = params.get_string("ssh_key_file")?;
        let ssh_key_comment = params.get_string("ssh_key_comment")?;
        let ssh_key_passphrase = params.get_string("ssh_key_passphrase")?;

        let user_exists = Self::user_exists(&name)?;

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

                Self::delete_user(&name, remove_home, force)?;
                Ok(ModuleOutput::changed(format!("Removed user '{}'", name)))
            }

            UserState::Present => {
                let mut changed = false;
                let mut messages = Vec::new();

                if !user_exists {
                    if context.check_mode {
                        return Ok(ModuleOutput::changed(format!(
                            "Would create user '{}'",
                            name
                        )));
                    }

                    Self::create_user(
                        &name,
                        uid,
                        group.as_deref(),
                        groups.as_deref(),
                        home.as_deref(),
                        shell.as_deref(),
                        comment.as_deref(),
                        create_home,
                        system,
                    )?;

                    changed = true;
                    messages.push(format!("Created user '{}'", name));
                } else {
                    // Modify existing user
                    if context.check_mode {
                        // Just check if modification would be needed
                        return Ok(ModuleOutput::changed(format!(
                            "Would modify user '{}'",
                            name
                        )));
                    }

                    let modified = Self::modify_user(
                        &name,
                        uid,
                        group.as_deref(),
                        groups.as_deref(),
                        append_groups,
                        home.as_deref(),
                        shell.as_deref(),
                        comment.as_deref(),
                        move_home,
                    )?;

                    if modified {
                        changed = true;
                        messages.push(format!("Modified user '{}'", name));
                    }
                }

                // Set password if provided
                if let Some(ref pwd) = password {
                    if context.check_mode {
                        messages.push("Would set password".to_string());
                        changed = true;
                    } else {
                        Self::set_password(&name, pwd, password_encrypted)?;
                        messages.push("Set password".to_string());
                        changed = true;
                    }
                }

                // Generate SSH key if requested
                if generate_ssh_key {
                    if context.check_mode {
                        messages.push("Would generate SSH key".to_string());
                        changed = true;
                    } else {
                        let key_generated = Self::generate_ssh_key(
                            &name,
                            &ssh_key_type,
                            ssh_key_bits,
                            ssh_key_file.as_deref(),
                            ssh_key_comment.as_deref(),
                            ssh_key_passphrase.as_deref(),
                        )?;

                        if key_generated {
                            messages.push("Generated SSH key".to_string());
                            changed = true;
                        }
                    }
                }

                // Get final user info
                let user_info = Self::get_user_info(&name)?;
                let mut data = HashMap::new();

                if let Some(info) = user_info {
                    data.insert("uid".to_string(), serde_json::json!(info.uid));
                    data.insert("gid".to_string(), serde_json::json!(info.gid));
                    data.insert("home".to_string(), serde_json::json!(info.home));
                    data.insert("shell".to_string(), serde_json::json!(info.shell));
                    data.insert("groups".to_string(), serde_json::json!(info.groups));
                }

                let msg = if messages.is_empty() {
                    format!("User '{}' is in desired state", name)
                } else {
                    messages.join(". ")
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

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let name = params.get_string_required("name")?;
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());
        let state = UserState::from_str(&state_str)?;

        let user_info = Self::get_user_info(&name)?;

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
        // root should always exist
        assert!(UserModule::user_exists("root").unwrap());
        // Random user should not exist
        assert!(!UserModule::user_exists("nonexistent_user_12345").unwrap());
    }

    #[test]
    fn test_get_user_info() {
        let info = UserModule::get_user_info("root").unwrap();
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.name, "root");
        assert_eq!(info.uid, 0);
    }

    // Integration tests would require root access
}
