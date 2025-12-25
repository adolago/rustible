//! Run command - Execute a playbook
//!
//! This module implements the `run` subcommand for executing Ansible-like playbooks.

use super::{CommandContext, Runnable};
use crate::cli::output::{RecapStats, TaskStatus};
use anyhow::{Context, Result};
use clap::Parser;
use indexmap::IndexMap;
use regex::Regex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Arguments for the run command
#[derive(Parser, Debug, Clone)]
pub struct RunArgs {
    /// Path to the playbook file
    #[arg(required = true)]
    pub playbook: PathBuf,

    /// Tags to run (only tasks with these tags)
    #[arg(long, short = 't', action = clap::ArgAction::Append)]
    pub tags: Vec<String>,

    /// Tags to skip (skip tasks with these tags)
    #[arg(long, action = clap::ArgAction::Append)]
    pub skip_tags: Vec<String>,

    /// Start at a specific task
    #[arg(long)]
    pub start_at_task: Option<String>,

    /// Step through tasks one at a time
    #[arg(long)]
    pub step: bool,

    /// Ask for vault password
    #[arg(long)]
    pub ask_vault_pass: bool,

    /// Vault password file
    #[arg(long)]
    pub vault_password_file: Option<PathBuf>,

    /// Become (sudo/su)
    #[arg(short = 'b', long)]
    pub r#become: bool,

    /// Become method (sudo, su, etc.)
    #[arg(long, default_value = "sudo")]
    pub become_method: String,

    /// Become user
    #[arg(long, default_value = "root")]
    pub become_user: String,

    /// Ask for become password
    #[arg(short = 'K', long)]
    pub ask_become_pass: bool,

    /// Remote user
    #[arg(short = 'u', long)]
    pub user: Option<String>,

    /// Private key file
    #[arg(long)]
    pub private_key: Option<PathBuf>,

    /// SSH common args
    #[arg(long)]
    pub ssh_common_args: Option<String>,
}

impl RunArgs {
    /// Execute the run command
    pub async fn execute(&self, ctx: &mut CommandContext) -> Result<i32> {
        let start_time = Instant::now();

        // Validate playbook exists
        if !self.playbook.exists() {
            ctx.output.error(&format!(
                "Playbook file not found: {}",
                self.playbook.display()
            ));
            return Ok(1);
        }

        // Display banner
        ctx.output.banner(&format!(
            "PLAYBOOK: {}",
            self.playbook
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ));

        // Load playbook
        ctx.output.info("Loading playbook...");
        let playbook_content = std::fs::read_to_string(&self.playbook)
            .with_context(|| format!("Failed to read playbook: {}", self.playbook.display()))?;

        let playbook: serde_yaml::Value = serde_yaml::from_str(&playbook_content)
            .with_context(|| "Failed to parse playbook YAML")?;

        // Get inventory
        let inventory_path = ctx.inventory().cloned();
        if inventory_path.is_none() {
            ctx.output
                .warning("No inventory specified, using localhost");
        }

        // Validate limit pattern if specified
        if let Some(ref limit) = ctx.limit {
            if let Err(e) = Self::validate_limit_pattern(limit) {
                ctx.output.error(&e);
                return Ok(1);
            }
        }

        // Parse extra vars
        let extra_vars = ctx.parse_extra_vars()?;
        ctx.output.debug(&format!("Extra vars: {:?}", extra_vars));

        // Check mode notice
        if ctx.check_mode {
            ctx.output
                .warning("Running in CHECK MODE - no changes will be made");
        }

        // Initialize stats (wrapped in Arc<Mutex<>> for thread-safe parallel execution)
        let stats = Arc::new(Mutex::new(RecapStats::new()));

        // Process playbook plays
        if let Some(plays) = playbook.as_sequence() {
            for play in plays {
                self.execute_play(ctx, play, &stats).await?;
            }
        } else {
            ctx.output.error("Playbook must be a list of plays");
            return Ok(1);
        }

        // Close all pooled connections
        ctx.close_connections().await;

        // Print recap
        let stats_guard = stats.lock().await;
        ctx.output.recap(&stats_guard);

        // Print timing
        let duration = start_time.elapsed();
        ctx.output.info(&format!(
            "Playbook finished in {:.2}s",
            duration.as_secs_f64()
        ));

        // Return exit code
        if stats_guard.has_failures() {
            Ok(2)
        } else {
            Ok(0)
        }
    }

    /// Execute a single play
    async fn execute_play(
        &self,
        ctx: &mut CommandContext,
        play: &serde_yaml::Value,
        stats: &Arc<Mutex<RecapStats>>,
    ) -> Result<()> {
        // Get play name
        let play_name = play
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("Unnamed play");

        ctx.output.play_header(play_name);

        // Get hosts pattern
        let hosts_pattern = play
            .get("hosts")
            .and_then(|h| h.as_str())
            .unwrap_or("localhost");

        ctx.output.info(&format!("Target hosts: {}", hosts_pattern));

        // Get hosts from inventory (simplified for now)
        let hosts = self.resolve_hosts(ctx, hosts_pattern)?;

        if hosts.is_empty() {
            ctx.output
                .warning(&format!("No hosts matched pattern: {}", hosts_pattern));
            return Ok(());
        }

        // Extract play-level variables
        let mut vars: IndexMap<String, serde_yaml::Value> = IndexMap::new();

        // Add extra vars first (lowest precedence in this context)
        if let Ok(extra_vars) = ctx.parse_extra_vars() {
            for (k, v) in extra_vars {
                if let Ok(yaml_val) = serde_yaml::to_value(&v) {
                    vars.insert(k, yaml_val);
                }
            }
        }

        // Add play vars (higher precedence)
        if let Some(play_vars) = play.get("vars") {
            if let Some(mapping) = play_vars.as_mapping() {
                for (k, v) in mapping {
                    if let Some(key) = k.as_str() {
                        vars.insert(key.to_string(), v.clone());
                    }
                }
            }
        }

        // Get tasks
        let tasks = play
            .get("tasks")
            .and_then(|t| t.as_sequence())
            .cloned()
            .unwrap_or_default();

        // Execute tasks
        for task in &tasks {
            self.execute_task(ctx, task, &hosts, stats, &vars).await?;
        }

        Ok(())
    }

    /// Resolve hosts from pattern
    fn resolve_hosts(&self, ctx: &CommandContext, pattern: &str) -> Result<Vec<String>> {
        // Simplified host resolution
        // In a real implementation, this would parse the inventory file

        if pattern == "localhost" || pattern == "127.0.0.1" {
            return Ok(vec!["localhost".to_string()]);
        }

        if pattern == "all" {
            // Load from inventory if available
            if let Some(inv_path) = ctx.inventory() {
                if inv_path.exists() {
                    let content = std::fs::read_to_string(inv_path)?;
                    let inventory: serde_yaml::Value = serde_yaml::from_str(&content)?;

                    let mut hosts = Vec::new();
                    if let Some(all) = inventory.get("all") {
                        if let Some(host_list) = all.get("hosts") {
                            if let Some(map) = host_list.as_mapping() {
                                for (key, _) in map {
                                    if let Some(host) = key.as_str() {
                                        hosts.push(host.to_string());
                                    }
                                }
                            }
                        }
                    }
                    if !hosts.is_empty() {
                        return Ok(hosts);
                    }
                }
            }
        }

        // Apply limit if specified
        if let Some(ref limit) = ctx.limit {
            if pattern.contains(limit) || limit.contains(pattern) {
                return Ok(vec![limit.clone()]);
            }
        }

        // Default to the pattern itself as a hostname
        Ok(vec![pattern.to_string()])
    }

    /// Execute a single task
    async fn execute_task(
        &self,
        ctx: &mut CommandContext,
        task: &serde_yaml::Value,
        hosts: &[String],
        stats: &Arc<Mutex<RecapStats>>,
        vars: &IndexMap<String, serde_yaml::Value>,
    ) -> Result<()> {
        // Get task name
        let task_name = task
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("Unnamed task");

        // Check tags
        if !self.should_run_task(task) {
            let mut stats_guard = stats.lock().await;
            for host in hosts {
                stats_guard.record(host, TaskStatus::Skipped);
            }
            return Ok(());
        }

        ctx.output.task_header(task_name);

        // Check conditions (when)
        let when_condition = task.get("when");

        // Execute on each host
        for host in hosts {
            // Check when condition (simplified)
            if let Some(when) = when_condition {
                let condition = when.as_str().unwrap_or("true");
                if condition == "false" {
                    ctx.output.task_result(
                        host,
                        TaskStatus::Skipped,
                        Some("conditional check failed"),
                    );
                    stats.lock().await.record(host, TaskStatus::Skipped);
                    continue;
                }
            }

            // Determine the module being used
            let (module, _args) = self.detect_module(task);

            // In check mode, don't actually execute
            if ctx.check_mode {
                ctx.output.task_result(
                    host,
                    TaskStatus::Changed,
                    Some(&format!("[check mode] would run: {}", module)),
                );
                stats.lock().await.record(host, TaskStatus::Changed);
                continue;
            }

            // Execute the task (simplified)
            let result = self.execute_module(ctx, host, task, vars).await;

            match result {
                Ok(changed) => {
                    let status = if changed {
                        TaskStatus::Changed
                    } else {
                        TaskStatus::Ok
                    };
                    ctx.output.task_result(host, status, None);
                    stats.lock().await.record(host, status);
                }
                Err(e) => {
                    // Check for ignore_errors
                    let ignore_errors = task
                        .get("ignore_errors")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if ignore_errors {
                        ctx.output.task_result(
                            host,
                            TaskStatus::Ignored,
                            Some(&format!("ignored error: {}", e)),
                        );
                        stats.lock().await.record(host, TaskStatus::Ignored);
                    } else {
                        ctx.output
                            .task_result(host, TaskStatus::Failed, Some(&e.to_string()));
                        stats.lock().await.record(host, TaskStatus::Failed);
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a task should run based on tags
    fn should_run_task(&self, task: &serde_yaml::Value) -> bool {
        // If no tags specified, run everything
        if self.tags.is_empty() && self.skip_tags.is_empty() {
            return true;
        }

        let task_tags: Vec<String> = task
            .get("tags")
            .and_then(|t| {
                if let Some(s) = t.as_str() {
                    Some(vec![s.to_string()])
                } else if let Some(seq) = t.as_sequence() {
                    Some(
                        seq.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // Check skip_tags first
        for skip_tag in &self.skip_tags {
            if task_tags.contains(skip_tag) {
                return false;
            }
        }

        // Check tags
        if !self.tags.is_empty() {
            for tag in &self.tags {
                if task_tags.contains(tag) || tag == "all" {
                    return true;
                }
            }
            return false;
        }

        true
    }

    /// Detect which module a task is using
    fn detect_module<'a>(
        &self,
        task: &'a serde_yaml::Value,
    ) -> (&'static str, Option<&'a serde_yaml::Value>) {
        // Common modules to check for
        let modules = [
            "command",
            "shell",
            "copy",
            "file",
            "template",
            "package",
            "apt",
            "yum",
            "dnf",
            "pip",
            "service",
            "systemd",
            "user",
            "group",
            "git",
            "debug",
            "set_fact",
            "include_tasks",
            "import_tasks",
            "block",
        ];

        for module in modules {
            if let Some(args) = task.get(module) {
                return (module, Some(args));
            }
        }

        ("unknown", None)
    }

    /// Execute a module (simplified implementation)
    async fn execute_module(
        &self,
        ctx: &CommandContext,
        host: &str,
        task: &serde_yaml::Value,
        vars: &IndexMap<String, serde_yaml::Value>,
    ) -> Result<bool> {
        let (module, args) = self.detect_module(task);

        ctx.output
            .debug(&format!("Executing module '{}' on host '{}'", module, host));

        // Handle debug module locally
        if module == "debug" {
            if let Some(args) = args {
                if let Some(msg) = args.get("msg").and_then(|m| m.as_str()) {
                    let templated_msg = Self::template_string(msg, vars);
                    ctx.output.info(&format!("DEBUG: {}", templated_msg));
                }
                if let Some(var) = args.get("var").and_then(|v| v.as_str()) {
                    // Look up the variable value
                    let var_name = Self::template_string(var, vars);
                    if let Some(value) = vars.get(&var_name) {
                        ctx.output.info(&format!("DEBUG: {} = {:?}", var_name, value));
                    } else {
                        ctx.output.info(&format!("DEBUG: {} = <undefined>", var_name));
                    }
                }
            }
            return Ok(false);
        }

        // Handle set_fact locally (no remote execution needed)
        if module == "set_fact" {
            return Ok(true);
        }

        // For command/shell modules, execute remotely if not localhost
        if module == "command" || module == "shell" {
            let cmd = if let Some(args) = args {
                args.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| {
                        args.get("cmd")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };

            if cmd.is_empty() {
                return Err(anyhow::anyhow!("No command specified"));
            }

            if host == "localhost" || host == "127.0.0.1" {
                // Local execution
                ctx.output.debug(&format!("Local execution: {}", cmd));
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.is_empty() {
                    return Err(anyhow::anyhow!("Empty command"));
                }

                let output =
                    std::process::Command::new(if module == "shell" { "sh" } else { parts[0] })
                        .args(if module == "shell" {
                            vec!["-c", &cmd]
                        } else {
                            parts[1..].to_vec()
                        })
                        .output()
                        .map_err(|e| anyhow::anyhow!("Failed to execute command: {}", e))?;

                if output.status.success() {
                    return Ok(true);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow::anyhow!("Command failed: {}", stderr));
                }
            } else {
                // Remote execution via SSH
                return self.execute_remote_command(ctx, host, &cmd).await;
            }
        }

        // For other modules, simulate execution for now
        Ok(true)
    }

    /// Execute a command on a remote host via SSH
    /// Uses connection pooling to reuse connections across multiple commands
    async fn execute_remote_command(
        &self,
        ctx: &CommandContext,
        host: &str,
        cmd: &str,
    ) -> Result<bool> {
        // Get host connection details from inventory
        let (ansible_host, ansible_user, ansible_port, ansible_key) =
            self.get_host_connection_info(ctx, host)?;

        // Get or create a pooled connection
        let conn = ctx
            .get_connection(
                host,
                &ansible_host,
                &ansible_user,
                ansible_port,
                ansible_key.as_deref(),
            )
            .await?;

        // Execute command on the pooled connection
        let result = conn
            .execute(cmd, None)
            .await
            .map_err(|e| anyhow::anyhow!("Command execution failed: {}", e))?;

        if result.success {
            Ok(true)
        } else {
            Err(anyhow::anyhow!(
                "Command failed with exit code {}: {}",
                result.exit_code,
                if result.stderr.is_empty() {
                    result.stdout
                } else {
                    result.stderr
                }
            ))
        }
    }

    /// Get connection info for a host from inventory
    fn get_host_connection_info(
        &self,
        ctx: &CommandContext,
        host: &str,
    ) -> Result<(String, String, u16, Option<String>)> {
        // Try to load from inventory
        if let Some(inv_path) = ctx.inventory() {
            if inv_path.exists() {
                let content = std::fs::read_to_string(inv_path)?;
                let inventory: serde_yaml::Value = serde_yaml::from_str(&content)?;

                // Look for host-specific vars
                if let Some(all) = inventory.get("all") {
                    // Get global vars
                    let global_user = all
                        .get("vars")
                        .and_then(|v| v.get("ansible_user"))
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string());
                    let global_key = all
                        .get("vars")
                        .and_then(|v| v.get("ansible_ssh_private_key_file"))
                        .and_then(|k| k.as_str())
                        .map(|s| s.to_string());

                    // Get host-specific vars
                    if let Some(hosts) = all.get("hosts") {
                        if let Some(host_config) = hosts.get(host) {
                            let ansible_host = host_config
                                .get("ansible_host")
                                .and_then(|h| h.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| host.to_string());
                            let ansible_user = host_config
                                .get("ansible_user")
                                .and_then(|u| u.as_str())
                                .map(|s| s.to_string())
                                .or(global_user)
                                .unwrap_or_else(|| {
                                    std::env::var("USER").unwrap_or_else(|_| "root".to_string())
                                });
                            let ansible_port = host_config
                                .get("ansible_port")
                                .and_then(|p| p.as_u64())
                                .unwrap_or(22)
                                as u16;
                            let ansible_key = host_config
                                .get("ansible_ssh_private_key_file")
                                .and_then(|k| k.as_str())
                                .map(|s| s.to_string())
                                .or(global_key);

                            return Ok((ansible_host, ansible_user, ansible_port, ansible_key));
                        }
                    }
                }
            }
        }

        // Default: use host as-is with current user
        let user = self
            .user
            .clone()
            .unwrap_or_else(|| std::env::var("USER").unwrap_or_else(|_| "root".to_string()));
        let key = self
            .private_key
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        Ok((host.to_string(), user, 22, key))
    }

    /// Validate a limit pattern
    /// Returns an error message if the pattern is invalid
    fn validate_limit_pattern(limit: &str) -> std::result::Result<(), String> {
        // Check for limit from file (@filename)
        if let Some(file_path) = limit.strip_prefix('@') {
            let path = std::path::Path::new(file_path);
            if !path.exists() {
                return Err(format!("Limit file not found: {}", file_path));
            }
            return Ok(());
        }

        // Split by colon to check each part
        for part in limit.split(':') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Strip leading operators (!, &)
            let pattern = part.trim_start_matches('!').trim_start_matches('&');

            // Check for regex pattern
            if let Some(regex_str) = pattern.strip_prefix('~') {
                if regex::Regex::new(regex_str).is_err() {
                    return Err(format!("Invalid regex pattern in limit: {}", regex_str));
                }
            }
        }

        Ok(())
    }

    /// Template a string by replacing {{ variable }} patterns with values
    fn template_string(template: &str, vars: &IndexMap<String, serde_yaml::Value>) -> String {
        // Simple Jinja2-like templating for {{ variable }} syntax
        let re = Regex::new(r"\{\{\s*([^}]+?)\s*\}\}").unwrap();
        let mut result = template.to_string();

        for cap in re.captures_iter(template) {
            let full_match = cap.get(0).unwrap().as_str();
            let expr = cap.get(1).unwrap().as_str().trim();

            // Handle simple variable lookup (no filters for now)
            let var_name = expr.split('|').next().unwrap_or(expr).trim();

            if let Some(value) = vars.get(var_name) {
                let replacement = Self::yaml_value_to_string(value);
                result = result.replace(full_match, &replacement);
            }
            // If variable not found, leave the original template expression
        }

        result
    }

    /// Convert a YAML value to a display string
    fn yaml_value_to_string(value: &serde_yaml::Value) -> String {
        match value {
            serde_yaml::Value::Null => String::new(),
            serde_yaml::Value::Bool(b) => b.to_string(),
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::String(s) => s.clone(),
            serde_yaml::Value::Sequence(seq) => {
                let items: Vec<String> = seq.iter().map(Self::yaml_value_to_string).collect();
                format!("[{}]", items.join(", "))
            }
            serde_yaml::Value::Mapping(map) => {
                let items: Vec<String> = map
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{}: {}",
                            Self::yaml_value_to_string(k),
                            Self::yaml_value_to_string(v)
                        )
                    })
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
            serde_yaml::Value::Tagged(tagged) => Self::yaml_value_to_string(&tagged.value),
        }
    }
}

#[async_trait::async_trait]
impl Runnable for RunArgs {
    async fn run(&self, ctx: &mut CommandContext) -> Result<i32> {
        self.execute(ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_args_parsing() {
        let args = RunArgs::try_parse_from(["run", "playbook.yml"]).unwrap();
        assert_eq!(args.playbook, PathBuf::from("playbook.yml"));
    }

    #[test]
    fn test_run_args_with_tags() {
        let args = RunArgs::try_parse_from([
            "run",
            "playbook.yml",
            "--tags",
            "install",
            "--tags",
            "configure",
        ])
        .unwrap();
        assert_eq!(args.tags, vec!["install", "configure"]);
    }

    #[test]
    fn test_run_args_become() {
        let args =
            RunArgs::try_parse_from(["run", "playbook.yml", "--become", "--become-user", "admin"])
                .unwrap();
        assert!(args.r#become);
        assert_eq!(args.become_user, "admin");
    }
}
