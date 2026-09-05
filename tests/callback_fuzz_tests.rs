//! Bounded property tests for actual Rustible callback APIs.
//! PROPTEST_CASES controls the shared proptest default; no per-group override.
use proptest::prelude::*;
use rustible::callback::factory::{PluginFactory, PluginFactoryErrorKind};
use rustible::callback::types::ResultInfo;

#[path = "common/callback_fuzz.rs"]
mod harness;

#[test]
fn real_configuration_uses_production_verbosity_bounds() {
    let config = harness::callback_config(br#"{"verbosity":255,"show_diff":true}"#);
    assert_eq!(config.verbosity, 4);
    assert!(config.show_diff);
}

#[test]
fn real_event_preserves_host_and_handler_classification() {
    let event = harness::callback_event(
        br#"{"event_type":"handler_triggered","handler_name":"reload","notifying_task":"config","host":"fixture"}"#,
    );
    assert_eq!(event.event_type(), "handler_triggered");
    assert_eq!(event.host(), Some("fixture"));
    assert!(event.is_handler_event());
    assert!(!event.is_failure());
}

#[test]
fn real_factory_accepts_canonical_names_and_rejects_invented_aliases() {
    for name in PluginFactory::available_plugin_names() {
        assert!(
            harness::plugin_resolution(name.as_bytes()).is_ok(),
            "{name}"
        );
        assert!(harness::plugin_resolution(name.to_uppercase().as_bytes()).is_ok());
    }
    assert!(harness::plugin_resolution(b"quiet").is_ok());
    assert!(harness::plugin_resolution(b"silent").is_ok());
    for name in [
        "min",
        "json@2.0",
        "rustible.callback.json",
        "not-a-plugin",
        "",
    ] {
        let error = match harness::plugin_resolution(name.as_bytes()) {
            Ok(_) => panic!("unsupported name accepted: {name}"),
            Err(error) => error,
        };
        assert_eq!(error.kind, PluginFactoryErrorKind::UnknownPlugin);
    }
}

#[test]
fn real_output_truncates_at_a_utf8_boundary() {
    let output = "界".repeat(3334);
    let result = ResultInfo::ok().with_output(0, output.clone(), output);
    assert!(result.output_truncated);
    assert_eq!(
        result.stdout,
        Some(format!(
            "{}... (truncated, 10002 bytes total)",
            "界".repeat(3333)
        ))
    );
    assert_eq!(result.stdout, result.stderr);
}

#[test]
fn real_output_keeps_short_unicode_and_ascii_contracts() {
    let result = ResultInfo::ok().with_output(0, "a界".into(), "x".repeat(10001));
    assert_eq!(result.stdout.as_deref(), Some("a界"));
    assert_eq!(
        result.stderr,
        Some(format!(
            "{}... (truncated, 10001 bytes total)",
            "x".repeat(10000)
        ))
    );
    assert!(result.output_truncated);
}

#[test]
fn malformed_duration_returns_an_error_instead_of_panicking() {
    let mut value = serde_json::to_value(ResultInfo::ok()).unwrap();
    value["duration"] = serde_json::json!({"secs": u64::MAX, "nanos": u32::MAX});
    assert!(serde_json::from_value::<ResultInfo>(value).is_err());
}

#[test]
fn duration_preserves_normalization_and_the_largest_valid_value() {
    let mut value = serde_json::to_value(ResultInfo::ok()).unwrap();
    value["duration"] = serde_json::json!({"secs": 0, "nanos": u32::MAX});
    let normalized: ResultInfo = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(normalized.duration, std::time::Duration::new(4, 294_967_295));
    value["duration"] = serde_json::json!({"secs": u64::MAX, "nanos": 999_999_999});
    let boundary: ResultInfo = serde_json::from_value(value).unwrap();
    assert_eq!(boundary.duration, std::time::Duration::MAX);
}

#[test]
fn shared_fuzz_entrypoints_handle_empty_invalid_and_boundary_inputs() {
    for input in [
        Vec::new(),
        vec![0xff; 65_536],
        "界".repeat(3334).into_bytes(),
    ] {
        harness::callback_config(&input);
        harness::callback_event(&input);
        let _ = harness::plugin_resolution(&input);
        harness::large_event_data(&input);
    }
}

proptest! {
    #[test]
    fn production_configuration_handles_bounded_input(data in prop::collection::vec(any::<u8>(), 0..4096)) {
        harness::callback_config(&data);
    }

    #[test]
    fn production_events_handle_bounded_input(data in prop::collection::vec(any::<u8>(), 0..4096)) {
        harness::callback_event(&data);
    }

    #[test]
    fn production_factory_handles_unicode_names(name in "\\PC{0,256}") {
        let _ = harness::plugin_resolution(name.as_bytes());
    }

    #[test]
    fn production_output_handles_unicode_at_the_cutoff(
        prefix in 9997usize..=10001,
        scalar in prop::sample::select(vec!['é', '界', '😀']),
    ) {
        let input = format!("{}{}suffix", "a".repeat(prefix), scalar);
        let result = harness::large_event_data(input.as_bytes());
        prop_assert!(result.output_truncated);
        prop_assert!(result.stdout.as_ref().unwrap().len() <= 10064);
    }

    #[test]
    fn production_output_handles_bounded_bytes(data in prop::collection::vec(any::<u8>(), 0..=65_536)) {
        let result = harness::large_event_data(&data);
        prop_assert!(result.stdout.as_ref().unwrap().len() <= 10064);
        prop_assert_eq!(result.stdout, result.stderr);
    }
}
