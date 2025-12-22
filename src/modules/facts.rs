//! Facts module - System fact gathering
//!
//! This module gathers facts about the target system including OS, hardware,
//! network, and other system information.

use super::{
    Module, ModuleContext, ModuleError, ModuleOutput, ModuleParams, ModuleResult, ParamExt,
};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

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

        let gather_all = gather_subset.contains(&"all".to_string());

        let mut all_facts = HashMap::new();

        // Always gather OS facts
        if gather_all
            || gather_subset.contains(&"os".to_string())
            || gather_subset.contains(&"min".to_string())
        {
            for (k, v) in Self::gather_os_facts() {
                all_facts.insert(k, v);
            }
        }

        // Gather hardware facts
        if gather_all || gather_subset.contains(&"hardware".to_string()) {
            for (k, v) in Self::gather_hardware_facts() {
                all_facts.insert(k, v);
            }
        }

        // Gather network facts
        if gather_all || gather_subset.contains(&"network".to_string()) {
            for (k, v) in Self::gather_network_facts() {
                all_facts.insert(k, v);
            }
        }

        // Gather date/time facts
        if gather_all || gather_subset.contains(&"date_time".to_string()) {
            for (k, v) in Self::gather_date_facts() {
                all_facts.insert(k, v);
            }
        }

        // Gather environment facts
        if gather_all || gather_subset.contains(&"env".to_string()) {
            for (k, v) in Self::gather_env_facts() {
                all_facts.insert(k, v);
            }
        }

        // Convert to serde_json::Value
        let facts_json: serde_json::Map<String, serde_json::Value> =
            all_facts.into_iter().collect();

        let _ = context;

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
}
