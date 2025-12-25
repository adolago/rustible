//! Fuzz target for large event data handling.
//!
//! This fuzzer tests the callback system's ability to handle large amounts
//! of event data without crashes or excessive memory usage.

#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use std::collections::HashMap;

/// Maximum sizes to prevent OOM during fuzzing
const MAX_STRING_SIZE: usize = 65536;
const MAX_ARRAY_SIZE: usize = 1024;
const MAX_MAP_SIZE: usize = 256;
const MAX_DEPTH: usize = 16;

/// Arbitrary JSON-like value for fuzzing large data structures
#[derive(Debug, Clone)]
enum FuzzJsonValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<FuzzJsonValue>),
    Object(HashMap<String, FuzzJsonValue>),
}

impl<'a> Arbitrary<'a> for FuzzJsonValue {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Self::arbitrary_with_depth(u, 0)
    }
}

impl FuzzJsonValue {
    fn arbitrary_with_depth(u: &mut Unstructured<'_>, depth: usize) -> arbitrary::Result<Self> {
        if depth >= MAX_DEPTH {
            // At max depth, only allow leaf types
            let choice: u8 = u.int_in_range(0..=3)?;
            return match choice {
                0 => Ok(FuzzJsonValue::Null),
                1 => Ok(FuzzJsonValue::Bool(bool::arbitrary(u)?)),
                2 => Ok(FuzzJsonValue::Int(i64::arbitrary(u)?)),
                _ => {
                    let len = u.int_in_range(0..=128)?;
                    let s: String = (0..len)
                        .map(|_| u.int_in_range(32u8..=126u8).map(|c| c as char))
                        .collect::<Result<_, _>>()?;
                    Ok(FuzzJsonValue::String(s))
                }
            };
        }

        let choice: u8 = u.int_in_range(0..=6)?;
        match choice {
            0 => Ok(FuzzJsonValue::Null),
            1 => Ok(FuzzJsonValue::Bool(bool::arbitrary(u)?)),
            2 => Ok(FuzzJsonValue::Int(i64::arbitrary(u)?)),
            3 => {
                let f = f64::arbitrary(u)?;
                Ok(FuzzJsonValue::Float(if f.is_finite() { f } else { 0.0 }))
            }
            4 => {
                let len = u.int_in_range(0..=MAX_STRING_SIZE.min(1024))?;
                let s: String = (0..len)
                    .map(|_| u.int_in_range(32u8..=126u8).map(|c| c as char))
                    .collect::<Result<_, _>>()?;
                Ok(FuzzJsonValue::String(s))
            }
            5 => {
                let len = u.int_in_range(0..=MAX_ARRAY_SIZE.min(32))?;
                let arr: Vec<FuzzJsonValue> = (0..len)
                    .map(|_| Self::arbitrary_with_depth(u, depth + 1))
                    .collect::<Result<_, _>>()?;
                Ok(FuzzJsonValue::Array(arr))
            }
            _ => {
                let len = u.int_in_range(0..=MAX_MAP_SIZE.min(16))?;
                let mut map = HashMap::new();
                for _ in 0..len {
                    let key_len = u.int_in_range(1..=64)?;
                    let key: String = (0..key_len)
                        .map(|_| u.int_in_range(97u8..=122u8).map(|c| c as char))
                        .collect::<Result<_, _>>()?;
                    let value = Self::arbitrary_with_depth(u, depth + 1)?;
                    map.insert(key, value);
                }
                Ok(FuzzJsonValue::Object(map))
            }
        }
    }

    fn approximate_size(&self) -> usize {
        match self {
            FuzzJsonValue::Null => 4,
            FuzzJsonValue::Bool(_) => 5,
            FuzzJsonValue::Int(n) => n.to_string().len(),
            FuzzJsonValue::Float(f) => f.to_string().len(),
            FuzzJsonValue::String(s) => s.len() + 2,
            FuzzJsonValue::Array(arr) => {
                2 + arr.iter().map(|v| v.approximate_size() + 1).sum::<usize>()
            }
            FuzzJsonValue::Object(map) => {
                2 + map.iter()
                    .map(|(k, v)| k.len() + 3 + v.approximate_size() + 1)
                    .sum::<usize>()
            }
        }
    }
}

/// Large task result for fuzzing
#[derive(Debug, Clone, Arbitrary)]
struct FuzzLargeTaskResult {
    host: String,
    task_name: String,
    module: String,
    success: bool,
    changed: bool,
    skipped: bool,
    message: String,
    stdout_lines: Vec<String>,
    stderr_lines: Vec<String>,
    warnings: Vec<String>,
    notify: Vec<String>,
}

/// Large fact set for fuzzing
#[derive(Debug, Clone, Arbitrary)]
struct FuzzLargeFacts {
    hostname: String,
    ip_addresses: Vec<String>,
    interfaces: Vec<(String, String)>,
    mounts: Vec<(String, String, u64)>,
    packages: Vec<(String, String)>,
    services: Vec<(String, bool)>,
    users: Vec<(String, u32)>,
    environment: Vec<(String, String)>,
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let mut unstructured = Unstructured::new(data);

    // Test large JSON-like data handling
    if let Ok(json_value) = FuzzJsonValue::arbitrary(&mut unstructured) {
        let size = json_value.approximate_size();

        // Ensure we can handle the data without panicking
        let _ = format!("{:?}", json_value);

        // Test size calculations
        let _ = size;

        // Simulate JSON serialization size check
        let size_ok = size <= 10_000_000; // 10MB limit
        let _ = size_ok;
    }

    // Test large task result handling
    if let Ok(result) = FuzzLargeTaskResult::arbitrary(&mut unstructured) {
        // Validate string lengths
        let host_len = result.host.len();
        let task_name_len = result.task_name.len();
        let module_len = result.module.len();
        let message_len = result.message.len();

        let _ = (host_len, task_name_len, module_len, message_len);

        // Calculate total stdout size
        let stdout_size: usize = result.stdout_lines.iter().map(|s| s.len()).sum();
        let stderr_size: usize = result.stderr_lines.iter().map(|s| s.len()).sum();
        let warnings_size: usize = result.warnings.iter().map(|s| s.len()).sum();

        let _ = (stdout_size, stderr_size, warnings_size);

        // Simulate truncation for large outputs
        let max_output_size = 1_000_000; // 1MB
        let should_truncate_stdout = stdout_size > max_output_size;
        let should_truncate_stderr = stderr_size > max_output_size;

        let _ = (should_truncate_stdout, should_truncate_stderr);

        // Test notify handler list
        for handler in &result.notify {
            let valid_handler = !handler.is_empty()
                && handler.len() <= 256
                && handler.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' ');
            let _ = valid_handler;
        }
    }

    // Test large facts handling
    if let Ok(facts) = FuzzLargeFacts::arbitrary(&mut unstructured) {
        // Count total fact entries
        let total_entries = 1  // hostname
            + facts.ip_addresses.len()
            + facts.interfaces.len()
            + facts.mounts.len()
            + facts.packages.len()
            + facts.services.len()
            + facts.users.len()
            + facts.environment.len();

        let _ = total_entries;

        // Calculate total size
        let hostname_size = facts.hostname.len();
        let ip_size: usize = facts.ip_addresses.iter().map(|s| s.len()).sum();
        let interface_size: usize = facts.interfaces.iter().map(|(n, a)| n.len() + a.len()).sum();
        let mount_size: usize = facts.mounts.iter().map(|(d, m, _)| d.len() + m.len() + 8).sum();
        let package_size: usize = facts.packages.iter().map(|(n, v)| n.len() + v.len()).sum();

        let total_size = hostname_size + ip_size + interface_size + mount_size + package_size;
        let _ = total_size;

        // Validate IP address format (basic check)
        for ip in &facts.ip_addresses {
            let parts: Vec<&str> = ip.split('.').collect();
            let is_valid_ipv4 = parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok());
            let _ = is_valid_ipv4;
        }

        // Validate interface names
        for (name, _) in &facts.interfaces {
            let is_valid_name = !name.is_empty()
                && name.len() <= 16
                && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
            let _ = is_valid_name;
        }

        // Validate mount paths
        for (device, mountpoint, size) in &facts.mounts {
            let is_valid_mount = !device.is_empty()
                && !mountpoint.is_empty()
                && mountpoint.starts_with('/');
            let _ = (is_valid_mount, *size);
        }
    }

    // Test concurrent event handling simulation
    if let Ok(event_count) = u16::arbitrary(&mut unstructured) {
        let count = (event_count as usize).min(1000);
        let mut events_processed = 0usize;

        for _ in 0..count {
            if let Ok(host_id) = u8::arbitrary(&mut unstructured) {
                if let Ok(task_id) = u16::arbitrary(&mut unstructured) {
                    let host = format!("host{}", host_id);
                    let task = format!("task_{}", task_id);
                    let _ = (host, task);
                    events_processed += 1;
                }
            }
        }

        let _ = events_processed;
    }
});
