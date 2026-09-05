//! Fuzz production callback configuration serde and in-memory operations.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    rustible_fuzz::callback_config(data);
});
