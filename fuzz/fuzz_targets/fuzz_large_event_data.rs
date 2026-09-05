//! Fuzz production callback output truncation and result serde.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rustible_fuzz::large_event_data(data);
});
