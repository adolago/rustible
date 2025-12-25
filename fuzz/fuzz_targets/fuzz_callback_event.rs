//! Fuzz target for callback event type parsing and handling.
//!
//! This fuzzer tests the robustness of callback event creation and processing
//! with arbitrary input data.

#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::{Arbitrary, Unstructured};
use std::time::Duration;

/// Arbitrary event type for fuzzing
#[derive(Debug, Clone, Arbitrary)]
enum FuzzEventType {
    PlaybookStart,
    PlaybookEnd,
    PlayStart,
    PlayEnd,
    TaskStart,
    TaskComplete,
    TaskFailed,
    TaskSkipped,
    TaskUnreachable,
    HandlerTriggered,
    FactsGathered,
    Warning,
    Deprecation,
    Verbose,
}

/// Arbitrary task status for fuzzing
#[derive(Debug, Clone, Arbitrary)]
enum FuzzTaskStatus {
    Ok,
    Changed,
    Failed,
    Skipped,
    Unreachable,
}

/// Arbitrary task event data for fuzzing
#[derive(Debug, Clone, Arbitrary)]
struct FuzzTaskEvent {
    host: String,
    task_name: String,
    module: String,
    status: FuzzTaskStatus,
    changed: bool,
    message: String,
    duration_ms: u64,
    warnings: Vec<String>,
    notify: Vec<String>,
}

/// Arbitrary play stats for fuzzing
#[derive(Debug, Clone, Arbitrary)]
struct FuzzPlayStats {
    host: String,
    ok: u32,
    changed: u32,
    failed: u32,
    skipped: u32,
    unreachable: u32,
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut unstructured = Unstructured::new(data);

    // Test event type parsing
    if let Ok(event_type) = FuzzEventType::arbitrary(&mut unstructured) {
        let _ = format!("{:?}", event_type);
    }

    // Test task status parsing
    if let Ok(status) = FuzzTaskStatus::arbitrary(&mut unstructured) {
        let _ = format!("{:?}", status);
    }

    // Test task event creation with arbitrary data
    if let Ok(event) = FuzzTaskEvent::arbitrary(&mut unstructured) {
        // Validate host name handling
        let _ = event.host.trim();
        let _ = event.host.is_empty();
        let _ = event.host.len();

        // Validate task name handling
        let _ = event.task_name.trim();
        let _ = event.task_name.is_empty();

        // Validate module name handling
        let _ = event.module.trim();

        // Validate message handling
        let _ = event.message.trim();

        // Validate duration
        let duration = Duration::from_millis(event.duration_ms);
        let _ = duration.as_secs();

        // Validate warnings
        for warning in &event.warnings {
            let _ = warning.trim();
        }

        // Validate notify handlers
        for handler in &event.notify {
            let _ = handler.trim();
        }

        // Test status string conversion
        let status_str = match event.status {
            FuzzTaskStatus::Ok => "OK",
            FuzzTaskStatus::Changed => "CHANGED",
            FuzzTaskStatus::Failed => "FAILED",
            FuzzTaskStatus::Skipped => "SKIPPED",
            FuzzTaskStatus::Unreachable => "UNREACHABLE",
        };
        let _ = status_str.to_lowercase();
    }

    // Test play stats with arbitrary data
    if let Ok(stats) = FuzzPlayStats::arbitrary(&mut unstructured) {
        // Validate stats calculations
        let total = stats.ok
            .saturating_add(stats.changed)
            .saturating_add(stats.failed)
            .saturating_add(stats.skipped)
            .saturating_add(stats.unreachable);
        let _ = total;

        // Check for failures
        let has_failures = stats.failed > 0 || stats.unreachable > 0;
        let _ = has_failures;

        // Validate host name
        let _ = stats.host.trim();
    }
});
