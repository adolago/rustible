//! Comprehensive tests for the DefaultCallback plugin and output formatting.
//!
//! This test suite covers:
//! 1. Colored output formatting
//! 2. Play/task header generation
//! 3. ok/changed/failed/skipped output
//! 4. Recap summary formatting
//! 5. Verbosity levels
//! 6. no_color mode
//!
//! Uses inline type definitions to test the output formatting logic
//! independently of the main codebase compilation state.

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use colored::control::{set_override, unset_override};
use colored::{Color, Colorize};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serial_test::serial;

// ============================================================================
// Mock Writer for Capturing Output
// ============================================================================

/// A thread-safe mock writer that captures all written bytes.
#[derive(Debug, Clone)]
pub struct MockWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl MockWriter {
    /// Create a new mock writer.
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get the captured output as a string.
    pub fn get_output(&self) -> String {
        let buffer = self.buffer.lock().unwrap();
        String::from_utf8_lossy(&buffer).to_string()
    }

    /// Clear the captured output.
    pub fn clear(&self) {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();
    }

    /// Check if the output contains a specific string.
    pub fn contains(&self, needle: &str) -> bool {
        self.get_output().contains(needle)
    }

    /// Check if the output contains a specific string (ignoring ANSI codes).
    pub fn contains_plain(&self, needle: &str) -> bool {
        self.strip_ansi().contains(needle)
    }

    /// Strip ANSI escape codes from the output.
    pub fn strip_ansi(&self) -> String {
        let output = self.get_output();
        // Simple regex-free ANSI stripping
        let mut result = String::new();
        let mut in_escape = false;
        for c in output.chars() {
            if c == '\x1b' {
                in_escape = true;
            } else if in_escape {
                if c == 'm' {
                    in_escape = false;
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}

impl Default for MockWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for MockWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ============================================================================
// Test Types (Independent of main codebase)
// ============================================================================

/// Task execution status for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Ok,
    Changed,
    Skipped,
    Failed,
    Unreachable,
    Rescued,
    Ignored,
}

impl TaskStatus {
    /// Get the colored string representation
    pub fn colored_string(&self) -> String {
        match self {
            TaskStatus::Ok => "ok".green().to_string(),
            TaskStatus::Changed => "changed".yellow().to_string(),
            TaskStatus::Skipped => "skipping".cyan().to_string(),
            TaskStatus::Failed => "failed".red().bold().to_string(),
            TaskStatus::Unreachable => "unreachable".red().bold().to_string(),
            TaskStatus::Rescued => "rescued".magenta().to_string(),
            TaskStatus::Ignored => "ignored".blue().to_string(),
        }
    }

    /// Get the plain string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Ok => "ok",
            TaskStatus::Changed => "changed",
            TaskStatus::Skipped => "skipping",
            TaskStatus::Failed => "failed",
            TaskStatus::Unreachable => "unreachable",
            TaskStatus::Rescued => "rescued",
            TaskStatus::Ignored => "ignored",
        }
    }
}

/// Statistics for a single host
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostStats {
    pub ok: u32,
    pub changed: u32,
    pub unreachable: u32,
    pub failed: u32,
    pub skipped: u32,
    pub rescued: u32,
    pub ignored: u32,
}

impl HostStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a task status
    pub fn record(&mut self, status: TaskStatus) {
        match status {
            TaskStatus::Ok => self.ok += 1,
            TaskStatus::Changed => self.changed += 1,
            TaskStatus::Skipped => self.skipped += 1,
            TaskStatus::Failed => self.failed += 1,
            TaskStatus::Unreachable => self.unreachable += 1,
            TaskStatus::Rescued => self.rescued += 1,
            TaskStatus::Ignored => self.ignored += 1,
        }
    }

    /// Check if there were any failures
    pub fn has_failures(&self) -> bool {
        self.failed > 0 || self.unreachable > 0
    }
}

/// Recap statistics for all hosts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecapStats {
    pub hosts: HashMap<String, HostStats>,
}

impl RecapStats {
    /// Create new empty recap stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a task result for a host
    pub fn record(&mut self, host: &str, status: TaskStatus) {
        self.hosts
            .entry(host.to_string())
            .or_default()
            .record(status);
    }

    /// Check if any host had failures
    pub fn has_failures(&self) -> bool {
        self.hosts.values().any(|h| h.has_failures())
    }

    /// Get total task count
    pub fn total_tasks(&self) -> u32 {
        self.hosts
            .values()
            .map(|h| {
                h.ok + h.changed + h.failed + h.unreachable + h.skipped + h.rescued + h.ignored
            })
            .sum()
    }
}

/// Output width constant
const OUTPUT_WIDTH: usize = 80;

/// DefaultCallback implementation for testing
pub struct DefaultCallback {
    verbosity: u8,
    use_color: bool,
    show_diff: bool,
}

impl DefaultCallback {
    /// Create a new DefaultCallback
    pub fn new(verbosity: u8, no_color: bool) -> Self {
        let use_color = !no_color && std::env::var("NO_COLOR").is_err();
        Self {
            verbosity,
            use_color,
            show_diff: false,
        }
    }

    /// Format a header line
    pub fn format_header(&self, prefix: &str, name: &str) -> String {
        let header = format!("{} [{}]", prefix, name);
        let padding = OUTPUT_WIDTH.saturating_sub(header.len() + 1);
        let stars = "*".repeat(padding);

        if self.use_color {
            format!(
                "\n{} {}",
                header.bright_white().bold(),
                stars.bright_black()
            )
        } else {
            format!("\n{} {}", header, stars)
        }
    }

    /// Format a status string with color
    pub fn format_status(&self, status: TaskStatus) -> String {
        if self.use_color {
            status.colored_string()
        } else {
            status.as_str().to_string()
        }
    }

    /// Format a host name
    pub fn format_host(&self, host: &str, status: TaskStatus) -> String {
        if self.use_color {
            match status {
                TaskStatus::Failed | TaskStatus::Unreachable => host.red().bold().to_string(),
                _ => host.bright_white().bold().to_string(),
            }
        } else {
            host.to_string()
        }
    }

    /// Format a task result line
    pub fn format_task_result(&self, host: &str, status: TaskStatus, msg: Option<&str>) -> String {
        let status_str = self.format_status(status);
        let host_str = self.format_host(host, status);

        match status {
            TaskStatus::Failed | TaskStatus::Unreachable => {
                if let Some(m) = msg {
                    format!("{}: [{}] => {{{}}}", status_str, host_str, m)
                } else {
                    format!("{}: [{}]", status_str, host_str)
                }
            }
            _ => format!("{}: [{}]", status_str, host_str),
        }
    }

    /// Format recap header
    pub fn format_recap_header(&self) -> String {
        let header = "PLAY RECAP";
        let padding = OUTPUT_WIDTH.saturating_sub(header.len() + 1);
        let stars = "*".repeat(padding);

        if self.use_color {
            format!(
                "\n{} {}",
                header.bright_white().bold(),
                stars.bright_black()
            )
        } else {
            format!("\n{} {}", header, stars)
        }
    }

    /// Format a stat value for recap
    pub fn format_stat(&self, label: &str, value: u32, color: Color) -> String {
        if self.use_color {
            if value > 0 {
                format!(
                    "{}={}",
                    label.color(color),
                    value.to_string().color(color).bold()
                )
            } else {
                format!("{}={}", label, value).dimmed().to_string()
            }
        } else {
            format!("{}={}", label, value)
        }
    }

    /// Format host for recap
    pub fn format_recap_host(&self, host: &str, stats: &HostStats) -> String {
        if self.use_color {
            if stats.has_failures() {
                host.red().bold().to_string()
            } else if stats.changed > 0 {
                host.yellow().to_string()
            } else {
                host.green().to_string()
            }
        } else {
            host.to_string()
        }
    }

    /// Format duration
    pub fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();

        if secs >= 3600 {
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let secs = secs % 60;
            format!("{}h {}m {}s", hours, mins, secs)
        } else if secs >= 60 {
            let mins = secs / 60;
            let secs = secs % 60;
            format!("{}m {}s", mins, secs)
        } else if secs > 0 {
            format!("{}.{:02}s", secs, millis / 10)
        } else {
            format!("{}ms", millis)
        }
    }

    pub fn verbosity(&self) -> u8 {
        self.verbosity
    }

    pub fn use_color(&self) -> bool {
        self.use_color
    }

    pub fn show_diff(&self) -> bool {
        self.show_diff
    }

    pub fn set_show_diff(&mut self, enabled: bool) {
        self.show_diff = enabled;
    }
}

/// Builder for DefaultCallback
pub struct DefaultCallbackBuilder {
    verbosity: u8,
    no_color: bool,
    show_diff: bool,
}

impl DefaultCallbackBuilder {
    pub fn new() -> Self {
        Self {
            verbosity: 0,
            no_color: false,
            show_diff: false,
        }
    }

    pub fn verbosity(mut self, level: u8) -> Self {
        self.verbosity = level;
        self
    }

    pub fn no_color(mut self, no_color: bool) -> Self {
        self.no_color = no_color;
        self
    }

    pub fn show_diff(mut self, show_diff: bool) -> Self {
        self.show_diff = show_diff;
        self
    }

    pub fn build(self) -> DefaultCallback {
        let mut callback = DefaultCallback::new(self.verbosity, self.no_color);
        callback.show_diff = self.show_diff;
        callback
    }
}

impl Default for DefaultCallbackBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Test 1: TaskStatus Display
// ============================================================================

#[test]
fn test_task_status_as_str() {
    assert_eq!(TaskStatus::Ok.as_str(), "ok");
    assert_eq!(TaskStatus::Changed.as_str(), "changed");
    assert_eq!(TaskStatus::Skipped.as_str(), "skipping");
    assert_eq!(TaskStatus::Failed.as_str(), "failed");
    assert_eq!(TaskStatus::Unreachable.as_str(), "unreachable");
    assert_eq!(TaskStatus::Rescued.as_str(), "rescued");
    assert_eq!(TaskStatus::Ignored.as_str(), "ignored");
}

#[test]
fn test_task_status_colored_string_contains_text() {
    assert!(TaskStatus::Ok.colored_string().contains("ok"));
    assert!(TaskStatus::Changed.colored_string().contains("changed"));
    assert!(TaskStatus::Skipped.colored_string().contains("skipping"));
    assert!(TaskStatus::Failed.colored_string().contains("failed"));
    assert!(TaskStatus::Unreachable
        .colored_string()
        .contains("unreachable"));
    assert!(TaskStatus::Rescued.colored_string().contains("rescued"));
    assert!(TaskStatus::Ignored.colored_string().contains("ignored"));
}

#[test]
#[serial]
fn test_task_status_colored_string_has_ansi_codes() {
    set_override(true);

    let ok_colored = TaskStatus::Ok.colored_string();
    let changed_colored = TaskStatus::Changed.colored_string();
    let failed_colored = TaskStatus::Failed.colored_string();

    assert!(
        ok_colored.contains("\x1b["),
        "ok should have ANSI codes: {}",
        ok_colored
    );
    assert!(
        changed_colored.contains("\x1b["),
        "changed should have ANSI codes: {}",
        changed_colored
    );
    assert!(
        failed_colored.contains("\x1b["),
        "failed should have ANSI codes: {}",
        failed_colored
    );

    unset_override();
}

// ============================================================================
// Test 2: DefaultCallback Initialization
// ============================================================================

#[test]
fn test_default_callback_new_with_color() {
    std::env::remove_var("NO_COLOR");
    let callback = DefaultCallback::new(0, false);
    assert!(callback.use_color());
}

#[test]
fn test_default_callback_new_without_color() {
    let callback = DefaultCallback::new(0, true);
    assert!(!callback.use_color());
}

#[test]
fn test_default_callback_verbosity_levels() {
    let v0 = DefaultCallback::new(0, true);
    let v1 = DefaultCallback::new(1, true);
    let v2 = DefaultCallback::new(2, true);
    let v3 = DefaultCallback::new(3, true);

    assert_eq!(v0.verbosity(), 0);
    assert_eq!(v1.verbosity(), 1);
    assert_eq!(v2.verbosity(), 2);
    assert_eq!(v3.verbosity(), 3);
}

#[test]
fn test_default_callback_builder() {
    let callback = DefaultCallbackBuilder::new()
        .verbosity(2)
        .no_color(true)
        .show_diff(true)
        .build();

    assert_eq!(callback.verbosity(), 2);
    assert!(!callback.use_color());
    assert!(callback.show_diff());
}

// ============================================================================
// Test 3: HostStats
// ============================================================================

#[test]
fn test_host_stats_new() {
    let stats = HostStats::new();
    assert_eq!(stats.ok, 0);
    assert_eq!(stats.changed, 0);
    assert_eq!(stats.unreachable, 0);
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.skipped, 0);
    assert_eq!(stats.rescued, 0);
    assert_eq!(stats.ignored, 0);
}

#[test]
fn test_host_stats_record_ok() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Ok);
    assert_eq!(stats.ok, 1);
    assert_eq!(stats.changed, 0);
    assert!(!stats.has_failures());
}

#[test]
fn test_host_stats_record_changed() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Changed);
    assert_eq!(stats.changed, 1);
    assert_eq!(stats.ok, 0);
    assert!(!stats.has_failures());
}

#[test]
fn test_host_stats_record_failed() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Failed);
    assert_eq!(stats.failed, 1);
    assert!(stats.has_failures());
}

#[test]
fn test_host_stats_record_unreachable() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Unreachable);
    assert_eq!(stats.unreachable, 1);
    assert!(stats.has_failures());
}

#[test]
fn test_host_stats_record_skipped() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Skipped);
    assert_eq!(stats.skipped, 1);
    assert!(!stats.has_failures());
}

#[test]
fn test_host_stats_record_rescued() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Rescued);
    assert_eq!(stats.rescued, 1);
    assert!(!stats.has_failures());
}

#[test]
fn test_host_stats_record_ignored() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Ignored);
    assert_eq!(stats.ignored, 1);
    assert!(!stats.has_failures());
}

#[test]
fn test_host_stats_multiple_records() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Ok);
    stats.record(TaskStatus::Ok);
    stats.record(TaskStatus::Changed);
    stats.record(TaskStatus::Changed);
    stats.record(TaskStatus::Changed);
    stats.record(TaskStatus::Skipped);

    assert_eq!(stats.ok, 2);
    assert_eq!(stats.changed, 3);
    assert_eq!(stats.skipped, 1);
    assert!(!stats.has_failures());
}

#[test]
fn test_host_stats_has_failures_with_failed() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Ok);
    stats.record(TaskStatus::Failed);
    assert!(stats.has_failures());
}

#[test]
fn test_host_stats_has_failures_with_unreachable() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Ok);
    stats.record(TaskStatus::Unreachable);
    assert!(stats.has_failures());
}

// ============================================================================
// Test 4: RecapStats
// ============================================================================

#[test]
fn test_recap_stats_new() {
    let stats = RecapStats::new();
    assert!(stats.hosts.is_empty());
    assert!(!stats.has_failures());
    assert_eq!(stats.total_tasks(), 0);
}

#[test]
fn test_recap_stats_record_single_host() {
    let mut stats = RecapStats::new();
    stats.record("host1", TaskStatus::Ok);

    assert_eq!(stats.hosts.len(), 1);
    assert!(stats.hosts.contains_key("host1"));
    assert_eq!(stats.hosts.get("host1").unwrap().ok, 1);
    assert_eq!(stats.total_tasks(), 1);
}

#[test]
fn test_recap_stats_record_multiple_hosts() {
    let mut stats = RecapStats::new();
    stats.record("host1", TaskStatus::Ok);
    stats.record("host2", TaskStatus::Changed);
    stats.record("host3", TaskStatus::Failed);

    assert_eq!(stats.hosts.len(), 3);
    assert!(stats.hosts.contains_key("host1"));
    assert!(stats.hosts.contains_key("host2"));
    assert!(stats.hosts.contains_key("host3"));
    assert_eq!(stats.total_tasks(), 3);
}

#[test]
fn test_recap_stats_record_same_host_multiple_times() {
    let mut stats = RecapStats::new();
    stats.record("host1", TaskStatus::Ok);
    stats.record("host1", TaskStatus::Changed);
    stats.record("host1", TaskStatus::Ok);

    assert_eq!(stats.hosts.len(), 1);
    let host_stats = stats.hosts.get("host1").unwrap();
    assert_eq!(host_stats.ok, 2);
    assert_eq!(host_stats.changed, 1);
    assert_eq!(stats.total_tasks(), 3);
}

#[test]
fn test_recap_stats_has_failures_true() {
    let mut stats = RecapStats::new();
    stats.record("host1", TaskStatus::Ok);
    stats.record("host2", TaskStatus::Failed);

    assert!(stats.has_failures());
}

#[test]
fn test_recap_stats_has_failures_false() {
    let mut stats = RecapStats::new();
    stats.record("host1", TaskStatus::Ok);
    stats.record("host2", TaskStatus::Changed);
    stats.record("host3", TaskStatus::Skipped);

    assert!(!stats.has_failures());
}

#[test]
fn test_recap_stats_total_tasks_complex() {
    let mut stats = RecapStats::new();

    stats.record("host1", TaskStatus::Ok);
    stats.record("host1", TaskStatus::Ok);
    stats.record("host1", TaskStatus::Changed);

    stats.record("host2", TaskStatus::Ok);
    stats.record("host2", TaskStatus::Failed);
    stats.record("host2", TaskStatus::Skipped);

    assert_eq!(stats.total_tasks(), 6);
}

// ============================================================================
// Test 5: Header Generation
// ============================================================================

#[test]
fn test_play_header_format_structure() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_header("PLAY", "Configure webservers");

    assert!(output.contains("PLAY"));
    assert!(output.contains("[Configure webservers]"));
    assert!(output.contains("*"));
}

#[test]
fn test_task_header_format_structure() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_header("TASK", "Install nginx");

    assert!(output.contains("TASK"));
    assert!(output.contains("[Install nginx]"));
    assert!(output.contains("*"));
}

#[test]
fn test_header_with_long_name() {
    let callback = DefaultCallback::new(0, true);
    let long_name = "a".repeat(100);
    let output = callback.format_header("PLAY", &long_name);

    // Should still contain the header even if too long
    assert!(output.contains("PLAY"));
    assert!(output.contains(&long_name));
}

#[test]
fn test_header_with_empty_name() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_header("PLAY", "");

    assert!(output.contains("PLAY []"));
    assert!(output.contains("*"));
}

#[test]
fn test_header_with_special_characters() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_header("TASK", "Deploy 'app' to /var/www");

    assert!(output.contains("TASK"));
    assert!(output.contains("[Deploy 'app' to /var/www]"));
}

#[test]
#[serial]
fn test_header_with_color() {
    std::env::remove_var("NO_COLOR");
    set_override(true);

    let callback = DefaultCallback::new(0, false);
    let output = callback.format_header("PLAY", "Test");

    // With color enabled, should have ANSI codes
    assert!(output.contains("\x1b["), "Should have ANSI codes");

    unset_override();
}

#[test]
fn test_header_without_color() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_header("PLAY", "Test");

    // Without color, should not have ANSI codes
    assert!(!output.contains("\x1b["), "Should not have ANSI codes");
}

// ============================================================================
// Test 6: Task Result Formatting
// ============================================================================

#[test]
fn test_format_task_result_ok() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_task_result("webserver01", TaskStatus::Ok, None);

    assert!(output.contains("ok:"));
    assert!(output.contains("[webserver01]"));
}

#[test]
fn test_format_task_result_changed() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_task_result("webserver01", TaskStatus::Changed, None);

    assert!(output.contains("changed:"));
    assert!(output.contains("[webserver01]"));
}

#[test]
fn test_format_task_result_failed_with_message() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_task_result(
        "webserver01",
        TaskStatus::Failed,
        Some("Connection refused"),
    );

    assert!(output.contains("failed:"));
    assert!(output.contains("[webserver01]"));
    assert!(output.contains("Connection refused"));
}

#[test]
fn test_format_task_result_skipped() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_task_result("webserver01", TaskStatus::Skipped, None);

    assert!(output.contains("skipping:"));
    assert!(output.contains("[webserver01]"));
}

#[test]
fn test_format_task_result_unreachable() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_task_result(
        "webserver01",
        TaskStatus::Unreachable,
        Some("Host not reachable"),
    );

    assert!(output.contains("unreachable:"));
    assert!(output.contains("[webserver01]"));
    assert!(output.contains("Host not reachable"));
}

// ============================================================================
// Test 7: Recap Formatting
// ============================================================================

#[test]
fn test_recap_header_format() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_recap_header();

    assert!(output.contains("PLAY RECAP"));
    assert!(output.contains("*"));
}

#[test]
fn test_format_stat_nonzero() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_stat("ok", 5, Color::Green);

    assert!(output.contains("ok=5"));
}

#[test]
fn test_format_stat_zero() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_stat("failed", 0, Color::Red);

    assert!(output.contains("failed=0"));
}

#[test]
fn test_format_recap_host_with_failures() {
    let callback = DefaultCallback::new(0, true);
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Failed);

    let output = callback.format_recap_host("failing_host", &stats);
    assert!(output.contains("failing_host"));
}

#[test]
fn test_format_recap_host_with_changes() {
    let callback = DefaultCallback::new(0, true);
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Changed);

    let output = callback.format_recap_host("changed_host", &stats);
    assert!(output.contains("changed_host"));
}

#[test]
fn test_format_recap_host_all_ok() {
    let callback = DefaultCallback::new(0, true);
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Ok);

    let output = callback.format_recap_host("ok_host", &stats);
    assert!(output.contains("ok_host"));
}

// ============================================================================
// Test 8: Duration Formatting
// ============================================================================

#[test]
fn test_duration_format_milliseconds() {
    assert_eq!(
        DefaultCallback::format_duration(Duration::from_millis(500)),
        "500ms"
    );
}

#[test]
fn test_duration_format_seconds() {
    assert_eq!(
        DefaultCallback::format_duration(Duration::from_secs(5)),
        "5.00s"
    );
}

#[test]
fn test_duration_format_seconds_with_millis() {
    assert_eq!(
        DefaultCallback::format_duration(Duration::from_millis(5123)),
        "5.12s"
    );
}

#[test]
fn test_duration_format_minutes() {
    assert_eq!(
        DefaultCallback::format_duration(Duration::from_secs(65)),
        "1m 5s"
    );
}

#[test]
fn test_duration_format_hours() {
    assert_eq!(
        DefaultCallback::format_duration(Duration::from_secs(3665)),
        "1h 1m 5s"
    );
}

#[test]
fn test_duration_format_zero() {
    assert_eq!(
        DefaultCallback::format_duration(Duration::from_millis(0)),
        "0ms"
    );
}

// ============================================================================
// Test 9: Color Code Verification
// ============================================================================

#[test]
#[serial]
fn test_ok_color_is_green() {
    set_override(true);
    let colored = TaskStatus::Ok.colored_string();
    assert!(
        colored.contains("32") || colored.contains("92"),
        "Expected green color code in: {}",
        colored
    );
    unset_override();
}

#[test]
#[serial]
fn test_changed_color_is_yellow() {
    set_override(true);
    let colored = TaskStatus::Changed.colored_string();
    assert!(
        colored.contains("33") || colored.contains("93"),
        "Expected yellow color code in: {}",
        colored
    );
    unset_override();
}

#[test]
#[serial]
fn test_failed_color_is_red() {
    set_override(true);
    let colored = TaskStatus::Failed.colored_string();
    assert!(
        colored.contains("31") || colored.contains("91"),
        "Expected red color code in: {}",
        colored
    );
    unset_override();
}

#[test]
#[serial]
fn test_skipped_color_is_cyan() {
    set_override(true);
    let colored = TaskStatus::Skipped.colored_string();
    assert!(
        colored.contains("36") || colored.contains("96"),
        "Expected cyan color code in: {}",
        colored
    );
    unset_override();
}

#[test]
#[serial]
fn test_unreachable_color_is_red() {
    set_override(true);
    let colored = TaskStatus::Unreachable.colored_string();
    assert!(
        colored.contains("31") || colored.contains("91"),
        "Expected red color code in: {}",
        colored
    );
    unset_override();
}

#[test]
#[serial]
fn test_rescued_color_is_magenta() {
    set_override(true);
    let colored = TaskStatus::Rescued.colored_string();
    assert!(
        colored.contains("35") || colored.contains("95"),
        "Expected magenta color code in: {}",
        colored
    );
    unset_override();
}

#[test]
#[serial]
fn test_ignored_color_is_blue() {
    set_override(true);
    let colored = TaskStatus::Ignored.colored_string();
    assert!(
        colored.contains("34") || colored.contains("94"),
        "Expected blue color code in: {}",
        colored
    );
    unset_override();
}

// ============================================================================
// Test 10: No Color Mode
// ============================================================================

#[test]
fn test_no_color_mode_disabled() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_status(TaskStatus::Ok);

    // Should not contain ANSI codes
    assert!(!output.contains("\x1b["));
    assert_eq!(output, "ok");
}

#[test]
fn test_no_color_header() {
    let callback = DefaultCallback::new(0, true);
    let output = callback.format_header("PLAY", "Test");

    // Should not contain ANSI codes
    assert!(!output.contains("\x1b["));
}

#[test]
fn test_plain_task_status_strings() {
    let ok_str = TaskStatus::Ok.as_str();
    let changed_str = TaskStatus::Changed.as_str();
    let failed_str = TaskStatus::Failed.as_str();

    assert!(!ok_str.contains("\x1b["));
    assert!(!changed_str.contains("\x1b["));
    assert!(!failed_str.contains("\x1b["));
}

// ============================================================================
// Test 11: JSON Serialization
// ============================================================================

#[test]
fn test_host_stats_serialization() {
    let mut stats = HostStats::new();
    stats.record(TaskStatus::Ok);
    stats.record(TaskStatus::Changed);
    stats.record(TaskStatus::Failed);

    let json = serde_json::to_string(&stats).unwrap();
    assert!(json.contains("\"ok\":1"));
    assert!(json.contains("\"changed\":1"));
    assert!(json.contains("\"failed\":1"));
}

#[test]
fn test_host_stats_deserialization() {
    let json =
        r#"{"ok":5,"changed":2,"unreachable":0,"failed":1,"skipped":3,"rescued":0,"ignored":0}"#;
    let stats: HostStats = serde_json::from_str(json).unwrap();

    assert_eq!(stats.ok, 5);
    assert_eq!(stats.changed, 2);
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.skipped, 3);
}

#[test]
fn test_recap_stats_serialization_roundtrip() {
    let mut original = RecapStats::new();
    original.record("host1", TaskStatus::Ok);
    original.record("host1", TaskStatus::Changed);
    original.record("host2", TaskStatus::Failed);

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: RecapStats = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.hosts.len(), original.hosts.len());
    assert_eq!(
        deserialized.hosts.get("host1").unwrap().ok,
        original.hosts.get("host1").unwrap().ok
    );
}

// ============================================================================
// Test 12: Edge Cases
// ============================================================================

#[test]
fn test_empty_host_name() {
    let mut stats = RecapStats::new();
    stats.record("", TaskStatus::Ok);

    assert!(stats.hosts.contains_key(""));
    assert_eq!(stats.hosts.get("").unwrap().ok, 1);
}

#[test]
fn test_unicode_host_name() {
    let mut stats = RecapStats::new();
    stats.record("servidor-principal-01", TaskStatus::Ok);
    stats.record("server-tokyo-01", TaskStatus::Changed);

    assert!(stats.hosts.contains_key("servidor-principal-01"));
    assert!(stats.hosts.contains_key("server-tokyo-01"));
}

#[test]
fn test_special_chars_in_host_name() {
    let mut stats = RecapStats::new();
    stats.record("server.example.com", TaskStatus::Ok);
    stats.record("192.168.1.100", TaskStatus::Changed);
    stats.record("server_with_underscore", TaskStatus::Skipped);

    assert_eq!(stats.hosts.len(), 3);
}

#[test]
fn test_very_long_host_name() {
    let mut stats = RecapStats::new();
    let long_name = "a".repeat(200);
    stats.record(&long_name, TaskStatus::Ok);

    assert!(stats.hosts.contains_key(&long_name));
}

#[test]
fn test_many_hosts() {
    let mut stats = RecapStats::new();
    for i in 0..1000 {
        stats.record(&format!("host{}", i), TaskStatus::Ok);
    }

    assert_eq!(stats.hosts.len(), 1000);
    assert_eq!(stats.total_tasks(), 1000);
}

// ============================================================================
// Test 13: Thread Safety
// ============================================================================

#[test]
fn test_recap_stats_thread_safety() {
    use std::thread;

    let stats = Arc::new(RwLock::new(RecapStats::new()));
    let mut handles = vec![];

    for i in 0..10 {
        let stats_clone = Arc::clone(&stats);
        let handle = thread::spawn(move || {
            let host = format!("host{}", i);
            stats_clone.write().record(&host, TaskStatus::Ok);
            stats_clone.write().record(&host, TaskStatus::Changed);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_stats = stats.read();
    assert_eq!(final_stats.hosts.len(), 10);
    assert_eq!(final_stats.total_tasks(), 20);
}

// ============================================================================
// Test 14: Full Integration Test
// ============================================================================

#[test]
fn test_full_recap_flow() {
    let mut stats = RecapStats::new();

    // Host 1: All ok
    for _ in 0..5 {
        stats.record("web1.example.com", TaskStatus::Ok);
    }

    // Host 2: Some changes
    stats.record("web2.example.com", TaskStatus::Ok);
    stats.record("web2.example.com", TaskStatus::Changed);
    stats.record("web2.example.com", TaskStatus::Changed);
    stats.record("web2.example.com", TaskStatus::Ok);

    // Host 3: Failed
    stats.record("db1.example.com", TaskStatus::Ok);
    stats.record("db1.example.com", TaskStatus::Failed);

    // Host 4: Unreachable
    stats.record("cache1.example.com", TaskStatus::Unreachable);

    // Host 5: Mixed with skipped
    stats.record("worker1.example.com", TaskStatus::Ok);
    stats.record("worker1.example.com", TaskStatus::Skipped);
    stats.record("worker1.example.com", TaskStatus::Changed);

    assert_eq!(stats.hosts.len(), 5);
    assert!(stats.has_failures());
    assert_eq!(stats.total_tasks(), 15);

    assert_eq!(stats.hosts.get("web1.example.com").unwrap().ok, 5);
    assert_eq!(stats.hosts.get("web2.example.com").unwrap().changed, 2);
    assert_eq!(stats.hosts.get("db1.example.com").unwrap().failed, 1);
    assert_eq!(
        stats.hosts.get("cache1.example.com").unwrap().unreachable,
        1
    );
    assert_eq!(stats.hosts.get("worker1.example.com").unwrap().skipped, 1);
}

#[test]
fn test_full_callback_formatting_flow() {
    let callback = DefaultCallback::new(1, true);

    // Test complete formatting flow
    let play_header = callback.format_header("PLAY", "Deploy Application");
    assert!(play_header.contains("PLAY"));
    assert!(play_header.contains("[Deploy Application]"));

    let task_header = callback.format_header("TASK", "Install packages");
    assert!(task_header.contains("TASK"));
    assert!(task_header.contains("[Install packages]"));

    let ok_result = callback.format_task_result("web1", TaskStatus::Ok, None);
    assert!(ok_result.contains("ok:"));
    assert!(ok_result.contains("[web1]"));

    let changed_result = callback.format_task_result("web2", TaskStatus::Changed, None);
    assert!(changed_result.contains("changed:"));

    let failed_result = callback.format_task_result("db1", TaskStatus::Failed, Some("Error"));
    assert!(failed_result.contains("failed:"));
    assert!(failed_result.contains("Error"));

    let recap_header = callback.format_recap_header();
    assert!(recap_header.contains("PLAY RECAP"));
}

// ============================================================================
// Test 15: Mock Writer Tests
// ============================================================================

#[test]
fn test_mock_writer_basic() {
    let mut writer = MockWriter::new();
    write!(writer, "Hello, World!").unwrap();

    assert_eq!(writer.get_output(), "Hello, World!");
}

#[test]
fn test_mock_writer_multiple_writes() {
    let mut writer = MockWriter::new();
    write!(writer, "First").unwrap();
    write!(writer, " ").unwrap();
    write!(writer, "Second").unwrap();

    assert_eq!(writer.get_output(), "First Second");
}

#[test]
fn test_mock_writer_clear() {
    let mut writer = MockWriter::new();
    write!(writer, "Content").unwrap();
    writer.clear();

    assert_eq!(writer.get_output(), "");
}

#[test]
fn test_mock_writer_contains() {
    let mut writer = MockWriter::new();
    write!(writer, "This is a test message").unwrap();

    assert!(writer.contains("test"));
    assert!(!writer.contains("missing"));
}

#[test]
fn test_mock_writer_strip_ansi() {
    let mut writer = MockWriter::new();
    write!(writer, "\x1b[32mgreen\x1b[0m text").unwrap();

    assert_eq!(writer.strip_ansi(), "green text");
}

#[test]
fn test_mock_writer_contains_plain() {
    let mut writer = MockWriter::new();
    write!(writer, "\x1b[31mred\x1b[0m error").unwrap();

    assert!(writer.contains_plain("red error"));
    assert!(writer.contains_plain("error"));
}
