//! Fuzz production callback event serde and classification.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rustible_fuzz::callback_event(data);
});
