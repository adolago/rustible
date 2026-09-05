//! Fuzz the production callback factory without executing callbacks.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = rustible_fuzz::plugin_resolution(data);
});
