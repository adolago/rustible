//! Facts module - System fact gathering
//!
//! This module gathers facts about the target system including OS, hardware,
//! network, and other system information.
//!
//! ## Remote Facts Gathering
//!
//! Facts can be gathered remotely via SSH or other connections using the async
//! `gather_facts_via_connection` function. This executes commands on the remote
//! host instead of locally.

use super::{Module, ModuleContext, ModuleOutput, ModuleParams, ModuleResult, ParamExt};
use crate::connection::Connection;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::sync::Arc;
use tracing::debug;

/// Module for gathering system facts
pub struct FactsModule;

impl FactsModule {
    fn gather_os_facts() -> HashMap<String, serde_json::Value> {
        let mut facts = HashMap::new();

        // Get hostname
        if let Ok(output) = Command::new("hostname").arg("-f").output() {
            if output.status.success() {
                let hostname = String::from_utf8_lossy(&output.stdout).trim().to_string();
                facts.insert("hostname".to_string(), serde_json::json!(hostname));

                // Also get short hostname
                if let Some(short) = hostname.split('.').next() {
                    facts.insert("hostname_short".to_string(), serde_json::json!(short));
                }
            }
        }

        // Get kernel info via uname
        if let Ok(output) = Command::new("uname").arg("-s").output() {
            if output.status.success() {
                facts.insert(
                    "system".to_string(),
                    serde_json::json!(String::from_utf8_lossy(&output.stdout).trim()),
                );
            }
        }

        if let Ok(output) = Command::new("uname").arg("-r").output() {
            if output.status.success() {
                facts.insert(
                    "kernel".to_string(),
                    serde_json::json!(String::from_utf8_lossy(&output.stdout).trim()),
                );
            }
        }

        if let Ok(output) = Command::new("uname").arg("-m").output() {
            if output.status.success() {
                let arch = String::from_utf8_lossy(&output.stdout).trim().to_string();
                facts.insert("architecture".to_string(), serde_json::json!(arch));

                // Map to common architecture names
                let machine = match arch.as_str() {
                    "x86_64" | "amd64" => "x86_64",
                    "aarch64" | "arm64" => "aarch64",
                    "armv7l" => "armv7l",
                    "i686" | "i386" => "i386",
                    _ => &arch,
                };
                facts.insert("machine".to_string(), serde_json::json!(machine));
            }
        }

        // Get OS release info
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    let value = value.trim_matches('"');
                    match key {
                        "ID" => {
                            facts.insert("distribution".to_string(), serde_json::json!(value));
                        }
                        "VERSION_ID" => {
                            facts.insert(
                                "distribution_version".to_string(),
                                serde_json::json!(value),
                            );
                        }
                        "ID_LIKE" => {
                            facts.insert("os_family".to_string(), serde_json::json!(value));
                        }
                        "PRETTY_NAME" => {
                            facts.insert(
                                "distribution_pretty_name".to_string(),
                                serde_json::json!(value),
                            );
                        }
                        "VERSION_CODENAME" => {
                            facts.insert(
                                "distribution_codename".to_string(),
                                serde_json::json!(value),
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        // Determine OS family if not set
        if !facts.contains_key("os_family") {
            if let Some(serde_json::Value::String(distro)) = facts.get("distribution") {
                let family = match distro.to_lowercase().as_str() {
                    "ubuntu" | "debian" | "linuxmint" | "pop" | "elementary" => "debian",
                    "fedora" | "centos" | "rhel" | "rocky" | "alma" | "oracle" => "redhat",
                    "arch" | "manjaro" | "endeavouros" => "arch",
                    "opensuse" | "sles" => "suse",
                    "alpine" => "alpine",
                    "gentoo" => "gentoo",
                    _ => "unknown",
                };
                facts.insert("os_family".to_string(), serde_json::json!(family));
            }
        }

        // Get current user
        if let Ok(output) = Command::new("whoami").output() {
            if output.status.success() {
                facts.insert(
                    "user_id".to_string(),
                    serde_json::json!(String::from_utf8_lossy(&output.stdout).trim()),
                );
            }
        }

        // Get user's UID
        if let Ok(output) = Command::new("id").arg("-u").output() {
            if output.status.success() {
                if let Ok(uid) = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<u32>()
                {
                    facts.insert("user_uid".to_string(), serde_json::json!(uid));
                }
            }
        }

        // Get user's GID
        if let Ok(output) = Command::new("id").arg("-g").output() {
            if output.status.success() {
                if let Ok(gid) = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<u32>()
                {
                    facts.insert("user_gid".to_string(), serde_json::json!(gid));
                }
            }
        }

        facts
    }

    fn gather_hardware_facts() -> HashMap<String, serde_json::Value> {
        let mut facts = HashMap::new();

        // Get CPU info
        if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
            let mut processor_count = 0;
            let mut model_name = String::new();
            let mut cpu_cores = 0;

            for line in content.lines() {
                if line.starts_with("processor") {
                    processor_count += 1;
                } else if line.starts_with("model name") {
                    if let Some((_, value)) = line.split_once(':') {
                        model_name = value.trim().to_string();
                    }
                } else if line.starts_with("cpu cores") {
                    if let Some((_, value)) = line.split_once(':') {
                        cpu_cores = value.trim().parse().unwrap_or(0);
                    }
                }
            }

            facts.insert(
                "processor_count".to_string(),
                serde_json::json!(processor_count),
            );
            if !model_name.is_empty() {
                facts.insert("processor".to_string(), serde_json::json!(model_name));
            }
            if cpu_cores > 0 {
                facts.insert("processor_cores".to_string(), serde_json::json!(cpu_cores));
            }
        }

        // Get memory info
        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            facts.insert("memtotal_mb".to_string(), serde_json::json!(kb / 1024));
                        }
                    }
                } else if line.starts_with("MemFree:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            facts.insert("memfree_mb".to_string(), serde_json::json!(kb / 1024));
                        }
                    }
                } else if line.starts_with("SwapTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            facts.insert("swaptotal_mb".to_string(), serde_json::json!(kb / 1024));
                        }
                    }
                }
            }
        }

        // Get disk info - root filesystem
        if let Ok(output) = Command::new("df").args(["-B1", "/"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().nth(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        if let Ok(total) = parts[1].parse::<u64>() {
                            facts.insert("disk_total_bytes".to_string(), serde_json::json!(total));
                        }
                        if let Ok(used) = parts[2].parse::<u64>() {
                            facts.insert("disk_used_bytes".to_string(), serde_json::json!(used));
                        }
                        if let Ok(avail) = parts[3].parse::<u64>() {
                            facts.insert(
                                "disk_available_bytes".to_string(),
                                serde_json::json!(avail),
                            );
                        }
                    }
                }
            }
        }

        facts
    }

    fn gather_network_facts() -> HashMap<String, serde_json::Value> {
        let mut facts = HashMap::new();
        let mut interfaces: Vec<serde_json::Value> = Vec::new();

        // Get network interfaces
        if let Ok(entries) = fs::read_dir("/sys/class/net") {
            for entry in entries.filter_map(|e| e.ok()) {
                let iface_name = entry.file_name().to_string_lossy().to_string();

                // Skip loopback
                if iface_name == "lo" {
                    continue;
                }

                let mut iface_info = serde_json::Map::new();
                iface_info.insert("device".to_string(), serde_json::json!(iface_name.clone()));

                // Get MAC address
                let mac_path = entry.path().join("address");
                if let Ok(mac) = fs::read_to_string(&mac_path) {
                    let mac = mac.trim();
                    if mac != "00:00:00:00:00:00" {
                        iface_info.insert("macaddress".to_string(), serde_json::json!(mac));
                    }
                }

                // Get MTU
                let mtu_path = entry.path().join("mtu");
                if let Ok(mtu) = fs::read_to_string(&mtu_path) {
                    if let Ok(mtu) = mtu.trim().parse::<u32>() {
                        iface_info.insert("mtu".to_string(), serde_json::json!(mtu));
                    }
                }

                // Get operstate
                let state_path = entry.path().join("operstate");
                if let Ok(state) = fs::read_to_string(&state_path) {
                    iface_info.insert(
                        "active".to_string(),
                        serde_json::json!(state.trim() == "up"),
                    );
                }

                interfaces.push(serde_json::Value::Object(iface_info));
            }
        }

        facts.insert("interfaces".to_string(), serde_json::json!(interfaces));

        // Get default IPv4 address
        if let Ok(output) = Command::new("ip")
            .args(["route", "get", "1.1.1.1"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for part in stdout.split_whitespace() {
                    // Look for src keyword followed by IP
                    if part == "src" {
                        if let Some(ip) = stdout.split("src ").nth(1) {
                            if let Some(ip) = ip.split_whitespace().next() {
                                facts.insert("default_ipv4".to_string(), serde_json::json!(ip));
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Get FQDN
        if let Ok(output) = Command::new("hostname").arg("-f").output() {
            if output.status.success() {
                facts.insert(
                    "fqdn".to_string(),
                    serde_json::json!(String::from_utf8_lossy(&output.stdout).trim()),
                );
            }
        }

        facts
    }

    fn gather_date_facts() -> HashMap<String, serde_json::Value> {
        let mut facts = HashMap::new();

        // Get current date/time info
        if let Ok(output) = Command::new("date").arg("+%Y-%m-%d %H:%M:%S %Z").output() {
            if output.status.success() {
                facts.insert(
                    "date_time".to_string(),
                    serde_json::json!(String::from_utf8_lossy(&output.stdout).trim()),
                );
            }
        }

        // Get epoch
        if let Ok(output) = Command::new("date").arg("+%s").output() {
            if output.status.success() {
                if let Ok(epoch) = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<u64>()
                {
                    facts.insert("epoch".to_string(), serde_json::json!(epoch));
                }
            }
        }

        // Get timezone
        if let Ok(tz) = fs::read_to_string("/etc/timezone") {
            facts.insert("timezone".to_string(), serde_json::json!(tz.trim()));
        } else if let Ok(link) = fs::read_link("/etc/localtime") {
            // Extract timezone from symlink path
            let path = link.to_string_lossy();
            if let Some(tz) = path.strip_prefix("/usr/share/zoneinfo/") {
                facts.insert("timezone".to_string(), serde_json::json!(tz));
            }
        }

        // Get uptime
        if let Ok(content) = fs::read_to_string("/proc/uptime") {
            if let Some(seconds_str) = content.split_whitespace().next() {
                if let Ok(seconds) = seconds_str.parse::<f64>() {
                    facts.insert(
                        "uptime_seconds".to_string(),
                        serde_json::json!(seconds as u64),
                    );
                }
            }
        }

        facts
    }

    fn gather_env_facts() -> HashMap<String, serde_json::Value> {
        let mut facts = HashMap::new();
        let mut env_vars = serde_json::Map::new();

        // Get important environment variables
        for (key, value) in std::env::vars() {
            match key.as_str() {
                "PATH" | "HOME" | "USER" | "SHELL" | "LANG" | "LC_ALL" | "TERM" | "PWD" => {
                    env_vars.insert(key, serde_json::json!(value));
                }
                _ => {}
            }
        }

        facts.insert("env".to_string(), serde_json::Value::Object(env_vars));

        // Get Python version if available
        if let Ok(output) = Command::new("python3").arg("--version").output() {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                if let Some(ver) = version.strip_prefix("Python ") {
                    facts.insert("python_version".to_string(), serde_json::json!(ver.trim()));
                }
            }
        }

        facts
    }
}

// ============================================================================
// Remote Facts Gathering via Connection
// ============================================================================

/// Commands issued while gathering facts from a remote host.
///
/// Fact gathering asks a host about twenty small questions. Sent one at a time
/// each answer costs a full round trip, which dominates the cost of connecting
/// to a host. `prefetch` sends them as a single shell script and splits the
/// answers apart, so the common case is one round trip instead of twenty.
///
/// Any command that was not prefetched — or a host whose shell cannot run the
/// batch script, such as a Windows target — falls back to executing that one
/// command on its own, so behavior is unchanged either way.
struct RemoteCommands<'a> {
    connection: &'a Arc<dyn Connection + Send + Sync>,
    outputs: HashMap<String, Option<String>>,
}

impl<'a> RemoteCommands<'a> {
    fn new(connection: &'a Arc<dyn Connection + Send + Sync>) -> Self {
        Self {
            connection,
            outputs: HashMap::new(),
        }
    }

    /// Run `commands` in one remote invocation and remember their output.
    async fn prefetch(&mut self, commands: &[&str]) {
        let pending: Vec<&str> = commands
            .iter()
            .copied()
            .filter(|command| !self.outputs.contains_key(*command))
            .collect();
        if pending.is_empty() {
            return;
        }

        // A per-batch nonce keeps a command's own output from being mistaken
        // for a delimiter.
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let script = build_batch_script(&nonce, &pending);

        let output = match self.connection.execute(&script, None).await {
            Ok(result) => result.stdout,
            Err(e) => {
                debug!("Batched fact commands failed, falling back: {}", e);
                return;
            }
        };

        match parse_batch_output(&nonce, &output, &pending) {
            Some(parsed) => {
                for (command, value) in parsed {
                    self.outputs.insert(command, value);
                }
            }
            None => {
                debug!("Batched fact output was not parseable, falling back to single commands");
            }
        }
    }

    /// The trimmed stdout of a command, executing it if it was not prefetched.
    async fn get(&mut self, command: &str) -> Option<String> {
        if let Some(cached) = self.outputs.get(command) {
            return cached.clone();
        }
        let value = execute_and_get_output(self.connection, command).await;
        self.outputs.insert(command.to_string(), value.clone());
        value
    }

    /// The content of a remote file.
    async fn read_file(&mut self, path: &str) -> Option<String> {
        self.get(&read_file_command(path)).await
    }
}

/// The command used to read a remote file, also used as its cache key.
fn read_file_command(path: &str) -> String {
    format!("cat {}", path)
}

/// Build a shell script that runs each command between marker lines.
fn build_batch_script(nonce: &str, commands: &[&str]) -> String {
    let mut script = String::new();
    for (index, command) in commands.iter().enumerate() {
        script.push_str(&format!("echo RUSTIBLE_{}_{}_BEGIN\n", nonce, index));
        script.push_str(command);
        script.push('\n');
        script.push_str(&format!("echo RUSTIBLE_{}_{}_END\n", nonce, index));
    }
    script
}

/// Split batched output back into per-command results.
///
/// Returns `None` when the markers are missing, which means the host did not
/// run the script as expected and the commands should be retried one by one.
fn parse_batch_output(
    nonce: &str,
    output: &str,
    commands: &[&str],
) -> Option<HashMap<String, Option<String>>> {
    let mut parsed = HashMap::new();
    let mut current: Option<(usize, Vec<&str>)> = None;

    for line in output.lines() {
        let line_trimmed = line.trim_end_matches('\r');
        if let Some(rest) = line_trimmed.strip_prefix(&format!("RUSTIBLE_{}_", nonce)) {
            if let Some(index) = rest.strip_suffix("_BEGIN") {
                let index: usize = index.parse().ok()?;
                current = Some((index, Vec::new()));
                continue;
            }
            if let Some(index) = rest.strip_suffix("_END") {
                let index: usize = index.parse().ok()?;
                let (open_index, collected) = current.take()?;
                if open_index != index {
                    return None;
                }
                let command = commands.get(index)?;
                let joined = collected.join("\n");
                let trimmed = joined.trim();
                parsed.insert(
                    command.to_string(),
                    // An empty answer means the command produced nothing, which
                    // callers treat the same as a failure.
                    (!trimmed.is_empty()).then(|| trimmed.to_string()),
                );
                continue;
            }
        }
        if let Some((_, collected)) = current.as_mut() {
            collected.push(line);
        }
    }

    // A truncated batch (connection dropped mid-script) is not usable.
    if current.is_some() || parsed.len() != commands.len() {
        return None;
    }
    Some(parsed)
}

/// Commands the OS fact gatherer issues.
const OS_FACT_COMMANDS: &[&str] = &[
    "hostname -f",
    "uname -s",
    "uname -r",
    "uname -m",
    "cat /etc/os-release",
    "whoami",
    "id -u",
    "id -g",
];

/// Commands the hardware fact gatherer issues.
const HARDWARE_FACT_COMMANDS: &[&str] = &["cat /proc/cpuinfo", "cat /proc/meminfo", "df -B1 /"];

/// Commands the network fact gatherer issues before it knows the interfaces.
const NETWORK_FACT_COMMANDS: &[&str] = &[
    "ls -1 /sys/class/net 2>/dev/null",
    "ip route get 1.1.1.1 2>/dev/null",
    "hostname -f",
];

/// Commands the date/time fact gatherer issues.
const DATE_FACT_COMMANDS: &[&str] = &[
    "date '+%Y-%m-%d %H:%M:%S %Z'",
    "date +%s",
    "cat /etc/timezone",
    "readlink /etc/localtime",
    "cat /proc/uptime",
];

/// Command that collects the environment variables Rustible reports.
const ENV_COMMAND: &str =
    "printenv PATH HOME USER SHELL LANG LC_ALL TERM PWD 2>/dev/null || echo ''";

/// Commands the environment fact gatherer issues.
const ENV_FACT_COMMANDS: &[&str] = &[ENV_COMMAND, "python3 --version 2>&1"];

/// Gather facts from a remote host via a Connection.
///
/// This function executes commands on the remote host using the provided
/// connection and parses the output to build a facts map. The commands for the
/// requested subsets are sent as one batch first, so a host is normally asked
/// once rather than once per fact.
///
/// # Arguments
///
/// * `connection` - The connection to use for remote execution
/// * `gather_subset` - Optional list of fact categories to gather ("all", "os", "hardware", etc.)
///
/// # Returns
///
/// A map of fact names to their values, similar to local facts gathering.
pub async fn gather_facts_via_connection(
    connection: &Arc<dyn Connection + Send + Sync>,
    gather_subset: Option<&[String]>,
) -> HashMap<String, serde_json::Value> {
    let gather_all = gather_subset
        .map(|s| s.iter().any(|x| x == "all"))
        .unwrap_or(true);
    let wants = |names: &[&str]| -> bool {
        gather_all
            || gather_subset
                .map(|subset| subset.iter().any(|entry| names.contains(&entry.as_str())))
                .unwrap_or(false)
    };

    let want_os = wants(&["os", "min"]);
    let want_hardware = wants(&["hardware"]);
    let want_network = wants(&["network"]);
    let want_date = wants(&["date_time"]);
    let want_env = wants(&["env"]);

    let mut commands = RemoteCommands::new(connection);

    // One round trip for everything the selected subsets need.
    let mut batch: Vec<&str> = Vec::new();
    for (wanted, group) in [
        (want_os, OS_FACT_COMMANDS),
        (want_hardware, HARDWARE_FACT_COMMANDS),
        (want_network, NETWORK_FACT_COMMANDS),
        (want_date, DATE_FACT_COMMANDS),
        (want_env, ENV_FACT_COMMANDS),
    ] {
        if wanted {
            batch.extend_from_slice(group);
        }
    }
    batch.sort_unstable();
    batch.dedup();
    if !batch.is_empty() {
        commands.prefetch(&batch).await;
    }

    let mut all_facts = HashMap::new();

    if want_os {
        all_facts.extend(gather_os_facts_remote(&mut commands).await);
    }
    if want_hardware {
        all_facts.extend(gather_hardware_facts_remote(&mut commands).await);
    }
    if want_network {
        all_facts.extend(gather_network_facts_remote(&mut commands).await);
    }
    if want_date {
        all_facts.extend(gather_date_facts_remote(&mut commands).await);
    }
    if want_env {
        all_facts.extend(gather_env_facts_remote(&mut commands).await);
    }

    all_facts
}

/// Helper to execute a command and get stdout if successful
async fn execute_and_get_output(
    connection: &Arc<dyn Connection + Send + Sync>,
    command: &str,
) -> Option<String> {
    match connection.execute(command, None).await {
        Ok(result) if result.success => Some(result.stdout.trim().to_string()),
        Ok(result) => {
            debug!(
                "Command '{}' failed with exit code {}: {}",
                command, result.exit_code, result.stderr
            );
            None
        }
        Err(e) => {
            debug!("Command '{}' execution error: {}", command, e);
            None
        }
    }
}

/// Gather OS facts from remote host
async fn gather_os_facts_remote(
    commands: &mut RemoteCommands<'_>,
) -> HashMap<String, serde_json::Value> {
    let mut facts = HashMap::new();

    // Get hostname
    if let Some(hostname) = commands.get("hostname -f").await {
        facts.insert("hostname".to_string(), serde_json::json!(hostname));
        if let Some(short) = hostname.split('.').next() {
            facts.insert("hostname_short".to_string(), serde_json::json!(short));
        }
    }

    // Get kernel info via uname
    if let Some(system) = commands.get("uname -s").await {
        facts.insert("system".to_string(), serde_json::json!(system));
    }

    if let Some(kernel) = commands.get("uname -r").await {
        facts.insert("kernel".to_string(), serde_json::json!(kernel));
    }

    if let Some(arch) = commands.get("uname -m").await {
        facts.insert("architecture".to_string(), serde_json::json!(&arch));

        // Map to common architecture names
        let machine = match arch.as_str() {
            "x86_64" | "amd64" => "x86_64",
            "aarch64" | "arm64" => "aarch64",
            "armv7l" => "armv7l",
            "i686" | "i386" => "i386",
            _ => &arch,
        };
        facts.insert("machine".to_string(), serde_json::json!(machine));
    }

    // Get OS release info
    if let Some(content) = commands.read_file("/etc/os-release").await {
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim_matches('"');
                match key {
                    "ID" => {
                        facts.insert("distribution".to_string(), serde_json::json!(value));
                    }
                    "VERSION_ID" => {
                        facts.insert("distribution_version".to_string(), serde_json::json!(value));
                    }
                    "ID_LIKE" => {
                        facts.insert("os_family".to_string(), serde_json::json!(value));
                    }
                    "PRETTY_NAME" => {
                        facts.insert(
                            "distribution_pretty_name".to_string(),
                            serde_json::json!(value),
                        );
                    }
                    "VERSION_CODENAME" => {
                        facts.insert(
                            "distribution_codename".to_string(),
                            serde_json::json!(value),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    // Determine OS family if not set
    if !facts.contains_key("os_family") {
        if let Some(serde_json::Value::String(distro)) = facts.get("distribution") {
            let family = match distro.to_lowercase().as_str() {
                "ubuntu" | "debian" | "linuxmint" | "pop" | "elementary" => "debian",
                "fedora" | "centos" | "rhel" | "rocky" | "alma" | "oracle" => "redhat",
                "arch" | "manjaro" | "endeavouros" => "arch",
                "opensuse" | "sles" => "suse",
                "alpine" => "alpine",
                "gentoo" => "gentoo",
                _ => "unknown",
            };
            facts.insert("os_family".to_string(), serde_json::json!(family));
        }
    }

    // Get current user
    if let Some(user) = commands.get("whoami").await {
        facts.insert("user_id".to_string(), serde_json::json!(user));
    }

    // Get user's UID
    if let Some(uid_str) = commands.get("id -u").await {
        if let Ok(uid) = uid_str.parse::<u32>() {
            facts.insert("user_uid".to_string(), serde_json::json!(uid));
        }
    }

    // Get user's GID
    if let Some(gid_str) = commands.get("id -g").await {
        if let Ok(gid) = gid_str.parse::<u32>() {
            facts.insert("user_gid".to_string(), serde_json::json!(gid));
        }
    }

    facts
}

/// Gather hardware facts from remote host
async fn gather_hardware_facts_remote(
    commands: &mut RemoteCommands<'_>,
) -> HashMap<String, serde_json::Value> {
    let mut facts = HashMap::new();

    // Get CPU info
    if let Some(content) = commands.read_file("/proc/cpuinfo").await {
        let mut processor_count = 0;
        let mut model_name = String::new();
        let mut cpu_cores = 0;

        for line in content.lines() {
            if line.starts_with("processor") {
                processor_count += 1;
            } else if line.starts_with("model name") {
                if let Some((_, value)) = line.split_once(':') {
                    model_name = value.trim().to_string();
                }
            } else if line.starts_with("cpu cores") {
                if let Some((_, value)) = line.split_once(':') {
                    cpu_cores = value.trim().parse().unwrap_or(0);
                }
            }
        }

        facts.insert(
            "processor_count".to_string(),
            serde_json::json!(processor_count),
        );
        if !model_name.is_empty() {
            facts.insert("processor".to_string(), serde_json::json!(model_name));
        }
        if cpu_cores > 0 {
            facts.insert("processor_cores".to_string(), serde_json::json!(cpu_cores));
        }
    }

    // Get memory info
    if let Some(content) = commands.read_file("/proc/meminfo").await {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        facts.insert("memtotal_mb".to_string(), serde_json::json!(kb / 1024));
                    }
                }
            } else if line.starts_with("MemFree:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        facts.insert("memfree_mb".to_string(), serde_json::json!(kb / 1024));
                    }
                }
            } else if line.starts_with("SwapTotal:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        facts.insert("swaptotal_mb".to_string(), serde_json::json!(kb / 1024));
                    }
                }
            }
        }
    }

    // Get disk info - root filesystem
    if let Some(stdout) = commands.get("df -B1 /").await {
        if let Some(line) = stdout.lines().nth(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                if let Ok(total) = parts[1].parse::<u64>() {
                    facts.insert("disk_total_bytes".to_string(), serde_json::json!(total));
                }
                if let Ok(used) = parts[2].parse::<u64>() {
                    facts.insert("disk_used_bytes".to_string(), serde_json::json!(used));
                }
                if let Ok(avail) = parts[3].parse::<u64>() {
                    facts.insert("disk_available_bytes".to_string(), serde_json::json!(avail));
                }
            }
        }
    }

    facts
}

/// Gather network facts from remote host
async fn gather_network_facts_remote(
    commands: &mut RemoteCommands<'_>,
) -> HashMap<String, serde_json::Value> {
    let mut facts = HashMap::new();
    let mut interfaces: Vec<serde_json::Value> = Vec::new();

    // Get network interfaces by listing /sys/class/net
    if let Some(iface_list) = commands.get("ls -1 /sys/class/net 2>/dev/null").await {
        let names: Vec<String> = iface_list
            .lines()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty() && name != "lo")
            .collect();

        // The per-interface reads are only known once the list is back, so
        // they cost a second round trip rather than three per interface.
        let per_interface: Vec<String> = names
            .iter()
            .flat_map(|name| {
                [
                    read_file_command(&format!("/sys/class/net/{}/address", name)),
                    read_file_command(&format!("/sys/class/net/{}/mtu", name)),
                    read_file_command(&format!("/sys/class/net/{}/operstate", name)),
                ]
            })
            .collect();
        if !per_interface.is_empty() {
            let borrowed: Vec<&str> = per_interface.iter().map(String::as_str).collect();
            commands.prefetch(&borrowed).await;
        }

        for iface_name in names {
            let mut iface_info = serde_json::Map::new();
            iface_info.insert("device".to_string(), serde_json::json!(iface_name.clone()));

            // Get MAC address
            if let Some(mac) = commands
                .read_file(&format!("/sys/class/net/{}/address", iface_name))
                .await
            {
                let mac = mac.trim();
                if mac != "00:00:00:00:00:00" {
                    iface_info.insert("macaddress".to_string(), serde_json::json!(mac));
                }
            }

            // Get MTU
            if let Some(mtu_str) = commands
                .read_file(&format!("/sys/class/net/{}/mtu", iface_name))
                .await
            {
                if let Ok(mtu) = mtu_str.trim().parse::<u32>() {
                    iface_info.insert("mtu".to_string(), serde_json::json!(mtu));
                }
            }

            // Get operstate
            if let Some(state) = commands
                .read_file(&format!("/sys/class/net/{}/operstate", iface_name))
                .await
            {
                iface_info.insert(
                    "active".to_string(),
                    serde_json::json!(state.trim() == "up"),
                );
            }

            interfaces.push(serde_json::Value::Object(iface_info));
        }
    }

    facts.insert("interfaces".to_string(), serde_json::json!(interfaces));

    // Get default IPv4 address
    if let Some(stdout) = commands.get("ip route get 1.1.1.1 2>/dev/null").await {
        if let Some(ip) = stdout.split("src ").nth(1) {
            if let Some(ip) = ip.split_whitespace().next() {
                facts.insert("default_ipv4".to_string(), serde_json::json!(ip));
            }
        }
    }

    // Get FQDN
    if let Some(fqdn) = commands.get("hostname -f").await {
        facts.insert("fqdn".to_string(), serde_json::json!(fqdn));
    }

    facts
}

/// Gather date/time facts from remote host
async fn gather_date_facts_remote(
    commands: &mut RemoteCommands<'_>,
) -> HashMap<String, serde_json::Value> {
    let mut facts = HashMap::new();

    // Get current date/time info
    if let Some(datetime) = commands.get("date '+%Y-%m-%d %H:%M:%S %Z'").await {
        facts.insert("date_time".to_string(), serde_json::json!(datetime));
    }

    // Get epoch
    if let Some(epoch_str) = commands.get("date +%s").await {
        if let Ok(epoch) = epoch_str.parse::<u64>() {
            facts.insert("epoch".to_string(), serde_json::json!(epoch));
        }
    }

    // Get timezone
    if let Some(tz) = commands.read_file("/etc/timezone").await {
        facts.insert("timezone".to_string(), serde_json::json!(tz.trim()));
    } else if let Some(link) = commands.get("readlink /etc/localtime").await {
        // Extract timezone from symlink path
        if let Some(tz) = link.strip_prefix("/usr/share/zoneinfo/") {
            facts.insert("timezone".to_string(), serde_json::json!(tz));
        }
    }

    // Get uptime
    if let Some(content) = commands.read_file("/proc/uptime").await {
        if let Some(seconds_str) = content.split_whitespace().next() {
            if let Ok(seconds) = seconds_str.parse::<f64>() {
                facts.insert(
                    "uptime_seconds".to_string(),
                    serde_json::json!(seconds as u64),
                );
            }
        }
    }

    facts
}

/// Gather environment facts from remote host
async fn gather_env_facts_remote(
    commands: &mut RemoteCommands<'_>,
) -> HashMap<String, serde_json::Value> {
    let mut facts = HashMap::new();
    let mut env_vars = serde_json::Map::new();

    // Get important environment variables using printenv
    if let Some(env_output) = commands.get(ENV_COMMAND).await {
        for line in env_output.lines() {
            if let Some((key, value)) = line.split_once('=') {
                env_vars.insert(key.to_string(), serde_json::json!(value));
            }
        }
    }

    facts.insert("env".to_string(), serde_json::Value::Object(env_vars));

    // Get Python version if available
    if let Some(version_output) = commands.get("python3 --version 2>&1").await {
        if let Some(ver) = version_output.strip_prefix("Python ") {
            facts.insert("python_version".to_string(), serde_json::json!(ver.trim()));
        }
    }

    facts
}

impl Module for FactsModule {
    fn name(&self) -> &'static str {
        "gather_facts"
    }

    fn description(&self) -> &'static str {
        "Gather facts about the target system"
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let gather_subset = params
            .get_vec_string("gather_subset")?
            .unwrap_or_else(|| vec!["all".to_string()]);

        let all_facts = if let Some(ref conn) = context.connection {
            // Remote execution: use the async gather_facts_via_connection
            let conn = conn.clone();
            let subset = gather_subset.clone();
            let handle = tokio::runtime::Handle::try_current().map_err(|_| {
                crate::modules::ModuleError::ExecutionFailed(
                    "No tokio runtime available".to_string(),
                )
            })?;
            std::thread::scope(|s| {
                s.spawn(|| handle.block_on(gather_facts_via_connection(&conn, Some(&subset))))
                    .join()
                    .map_err(|_| {
                        crate::modules::ModuleError::ExecutionFailed(
                            "Facts gathering thread panicked".to_string(),
                        )
                    })
            })?
        } else {
            // Local fallback: use the synchronous local methods
            let gather_all = gather_subset.contains(&"all".to_string());
            let mut facts = HashMap::new();

            if gather_all
                || gather_subset.contains(&"os".to_string())
                || gather_subset.contains(&"min".to_string())
            {
                for (k, v) in Self::gather_os_facts() {
                    facts.insert(k, v);
                }
            }

            if gather_all || gather_subset.contains(&"hardware".to_string()) {
                for (k, v) in Self::gather_hardware_facts() {
                    facts.insert(k, v);
                }
            }

            if gather_all || gather_subset.contains(&"network".to_string()) {
                for (k, v) in Self::gather_network_facts() {
                    facts.insert(k, v);
                }
            }

            if gather_all || gather_subset.contains(&"date_time".to_string()) {
                for (k, v) in Self::gather_date_facts() {
                    facts.insert(k, v);
                }
            }

            if gather_all || gather_subset.contains(&"env".to_string()) {
                for (k, v) in Self::gather_env_facts() {
                    facts.insert(k, v);
                }
            }

            facts
        };

        // Convert to serde_json::Value
        let facts_json: serde_json::Map<String, serde_json::Value> =
            all_facts.into_iter().collect();

        Ok(ModuleOutput::ok("Facts gathered successfully")
            .with_data("ansible_facts", serde_json::Value::Object(facts_json)))
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        // Fact gathering is read-only, so check mode behaves the same
        self.execute(params, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_batch_script_wraps_each_command() {
        let script = build_batch_script("nonce", &["uname -s", "id -u"]);
        assert_eq!(
            script,
            "echo RUSTIBLE_nonce_0_BEGIN\nuname -s\necho RUSTIBLE_nonce_0_END\n\
             echo RUSTIBLE_nonce_1_BEGIN\nid -u\necho RUSTIBLE_nonce_1_END\n"
        );
    }

    #[test]
    fn test_parse_batch_output_splits_results() {
        let output = "RUSTIBLE_n_0_BEGIN\nLinux\nRUSTIBLE_n_0_END\n\
                      RUSTIBLE_n_1_BEGIN\n\nRUSTIBLE_n_1_END\n";
        let parsed = parse_batch_output("n", output, &["uname -s", "missing"]).unwrap();
        assert_eq!(parsed["uname -s"], Some("Linux".to_string()));
        assert_eq!(
            parsed["missing"], None,
            "a command with no output is reported as absent"
        );
    }

    #[test]
    fn test_parse_batch_output_keeps_multiline_content() {
        let output = "RUSTIBLE_n_0_BEGIN\nfirst\nsecond\nRUSTIBLE_n_0_END\n";
        let parsed = parse_batch_output("n", output, &["cat file"]).unwrap();
        assert_eq!(parsed["cat file"], Some("first\nsecond".to_string()));
    }

    #[test]
    fn test_parse_batch_output_rejects_truncated_output() {
        // A connection dropped mid-script leaves an unterminated section.
        let output = "RUSTIBLE_n_0_BEGIN\nLinux\n";
        assert!(parse_batch_output("n", output, &["uname -s"]).is_none());
    }

    #[test]
    fn test_parse_batch_output_rejects_unmarked_output() {
        // A shell that could not run the script produces no markers at all.
        assert!(parse_batch_output("n", "command not found\n", &["uname -s"]).is_none());
    }

    #[tokio::test]
    async fn test_prefetch_answers_every_command_in_one_batch() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let mut commands = RemoteCommands::new(&conn);
        commands.prefetch(&["echo one", "echo two", "true"]).await;

        // prefetch only records answers when the batch parsed cleanly, so a
        // populated map proves the single round trip worked.
        assert_eq!(commands.outputs.len(), 3);
        assert_eq!(commands.outputs["echo one"], Some("one".to_string()));
        assert_eq!(commands.outputs["echo two"], Some("two".to_string()));
        assert_eq!(commands.outputs["true"], None);
    }

    #[tokio::test]
    async fn test_get_falls_back_to_a_single_command() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let mut commands = RemoteCommands::new(&conn);
        assert_eq!(
            commands.get("echo fallback").await,
            Some("fallback".to_string())
        );
    }

    #[test]
    fn test_gather_os_facts() {
        let facts = FactsModule::gather_os_facts();

        // Should always have some OS facts on Linux
        assert!(facts.contains_key("system") || facts.contains_key("hostname"));
    }

    #[test]
    fn test_gather_hardware_facts() {
        let facts = FactsModule::gather_hardware_facts();

        // Should have processor count on Linux
        if std::path::Path::new("/proc/cpuinfo").exists() {
            assert!(facts.contains_key("processor_count"));
        }
    }

    #[test]
    fn test_gather_network_facts() {
        let facts = FactsModule::gather_network_facts();

        // Should have interfaces on Linux
        if std::path::Path::new("/sys/class/net").exists() {
            assert!(facts.contains_key("interfaces"));
        }
    }

    #[test]
    fn test_facts_module_execute() {
        let module = FactsModule;
        let params: ModuleParams = HashMap::new();
        let context = ModuleContext::default();

        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
        assert!(result.data.contains_key("ansible_facts"));
    }

    #[test]
    fn test_facts_module_with_subset() {
        let module = FactsModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "gather_subset".to_string(),
            serde_json::json!(["os", "hardware"]),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
        assert!(result.data.contains_key("ansible_facts"));
    }

    // ========================================================================
    // Remote Facts Gathering Tests
    // ========================================================================

    #[tokio::test]
    async fn test_gather_facts_via_connection_all() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let facts = gather_facts_via_connection(&conn, None).await;

        // Should have OS facts
        assert!(
            facts.contains_key("system") || facts.contains_key("hostname"),
            "Expected OS facts"
        );

        // Should have hardware facts if /proc exists
        if std::path::Path::new("/proc/cpuinfo").exists() {
            assert!(
                facts.contains_key("processor_count"),
                "Expected processor_count fact"
            );
        }
    }

    #[tokio::test]
    async fn test_gather_facts_via_connection_os_subset() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let subset = vec!["os".to_string()];
        let facts = gather_facts_via_connection(&conn, Some(&subset)).await;

        // Should have OS facts
        assert!(
            facts.contains_key("system") || facts.contains_key("hostname"),
            "Expected OS facts with 'os' subset"
        );

        // Should NOT have hardware-specific facts when only 'os' is requested
        // Note: processor_count is a hardware fact, not an OS fact
    }

    #[tokio::test]
    async fn test_gather_facts_via_connection_hardware_subset() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let subset = vec!["hardware".to_string()];
        let facts = gather_facts_via_connection(&conn, Some(&subset)).await;

        // Should have hardware facts if /proc exists
        if std::path::Path::new("/proc/cpuinfo").exists() {
            assert!(
                facts.contains_key("processor_count"),
                "Expected hardware facts with 'hardware' subset"
            );
        }
    }

    #[tokio::test]
    async fn test_gather_os_facts_remote() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let facts = gather_os_facts_remote(&mut RemoteCommands::new(&conn)).await;

        // Should get hostname
        assert!(
            facts.contains_key("hostname") || facts.contains_key("system"),
            "Expected hostname or system fact"
        );

        // Should get user info
        assert!(facts.contains_key("user_id"), "Expected user_id fact");
    }

    #[tokio::test]
    async fn test_gather_hardware_facts_remote() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let facts = gather_hardware_facts_remote(&mut RemoteCommands::new(&conn)).await;

        // Should have processor info on Linux
        if std::path::Path::new("/proc/cpuinfo").exists() {
            assert!(
                facts.contains_key("processor_count"),
                "Expected processor_count"
            );
        }

        // Should have memory info on Linux
        if std::path::Path::new("/proc/meminfo").exists() {
            assert!(facts.contains_key("memtotal_mb"), "Expected memtotal_mb");
        }
    }

    #[tokio::test]
    async fn test_gather_network_facts_remote() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let facts = gather_network_facts_remote(&mut RemoteCommands::new(&conn)).await;

        // Should have interfaces list
        assert!(facts.contains_key("interfaces"), "Expected interfaces fact");
    }

    #[tokio::test]
    async fn test_gather_date_facts_remote() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let facts = gather_date_facts_remote(&mut RemoteCommands::new(&conn)).await;

        // Should have date/time info
        assert!(facts.contains_key("date_time"), "Expected date_time fact");
        assert!(facts.contains_key("epoch"), "Expected epoch fact");
    }

    #[tokio::test]
    async fn test_gather_env_facts_remote() {
        use crate::connection::local::LocalConnection;

        let conn: Arc<dyn Connection + Send + Sync> = Arc::new(LocalConnection::new());
        let facts = gather_env_facts_remote(&mut RemoteCommands::new(&conn)).await;

        // Should have env fact
        assert!(facts.contains_key("env"), "Expected env fact");
    }
}
