//! Shared, bounded production entrypoints for property tests and libFuzzer.
//!
//! These call production APIs. They do not load environment/file configuration,
//! execute tasks, emit callbacks, open user-selected paths, or use the network.
//! Plugin construction may probe the terminal and NO_COLOR display preference.

use rustible::callback::config::CallbackConfig;
use rustible::callback::factory::{PluginFactory, PluginResult};
use rustible::callback::types::{CallbackEvent, ResultInfo};
use rustible::traits::ExecutionCallback;
use std::sync::Arc;

pub const MAX_INPUT_BYTES: usize = 65_536;

fn bounded(data: &[u8]) -> &[u8] {
    &data[..data.len().min(MAX_INPUT_BYTES)]
}

/// Exercise real serde configuration and in-memory plugin/verbosity operations.
pub fn callback_config(data: &[u8]) -> CallbackConfig {
    let data = bounded(data);
    let mut config = serde_json::from_slice::<CallbackConfig>(data).unwrap_or_default();
    config.set_verbosity(config.verbosity);
    assert!(config.verbosity <= 4);
    let name = String::from_utf8_lossy(&data[..data.len().min(256)]);
    config.enable_plugin(&name);
    assert!(config.is_plugin_enabled(&name));
    config.disable_plugin(&name);
    assert!(!config.is_plugin_enabled(&name));
    let serialized = serde_json::to_vec(&config).expect("configuration must serialize");
    let restored: CallbackConfig =
        serde_json::from_slice(&serialized).expect("serialized configuration must parse");
    assert_eq!(
        serde_json::to_value(&config).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    config
}

/// Exercise actual event deserialization, classification and JSON round trips.
pub fn callback_event(data: &[u8]) -> CallbackEvent {
    let data = bounded(data);
    let event =
        serde_json::from_slice::<CallbackEvent>(data).unwrap_or_else(|_| CallbackEvent::Warning {
            msg: String::from_utf8_lossy(data).into_owned(),
            host: None,
        });
    let serialized = serde_json::to_vec(&event).expect("event must serialize");
    let restored: CallbackEvent =
        serde_json::from_slice(&serialized).expect("serialized event must parse");
    assert_eq!(event.event_type(), restored.event_type());
    assert_eq!(event.host(), restored.host());
    assert_eq!(event.is_failure(), restored.is_failure());
    assert_eq!(event.is_handler_event(), restored.is_handler_event());
    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    event
}

/// Exercise the real factory, including rejection of unsupported names.
/// Only construction occurs; callback methods are never invoked.
/// Name lookup and construction must agree for every input.
pub fn plugin_resolution(data: &[u8]) -> PluginResult<Arc<dyn ExecutionCallback>> {
    let name = String::from_utf8_lossy(bounded(data));
    let exists = PluginFactory::plugin_exists(&name);
    let result = PluginFactory::create(&name, &CallbackConfig::default());
    assert_eq!(
        exists,
        result.is_ok(),
        "plugin_exists and create must agree for {name:?}"
    );
    result
}

/// Exercise the actual bounded-output path with arbitrary UTF-8 and JSON data.
pub fn large_event_data(data: &[u8]) -> ResultInfo {
    let data = bounded(data);
    let output = String::from_utf8_lossy(data).into_owned();
    let mut result = ResultInfo::ok().with_output(0, output.clone(), output);
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        result.data.insert("payload".to_owned(), value);
    }
    let serialized = serde_json::to_vec(&result).expect("result must serialize");
    let restored: ResultInfo =
        serde_json::from_slice(&serialized).expect("serialized result must parse");
    assert_eq!(result.stdout, restored.stdout);
    assert_eq!(result.stderr, restored.stderr);
    assert_eq!(result.output_truncated, restored.output_truncated);
    assert_eq!(result.data, restored.data);
    result
}
