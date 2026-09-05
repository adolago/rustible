//! Use the same production entrypoints as the normal Cargo regression suite.
#[path = "../../tests/common/callback_fuzz.rs"]
mod harness;

pub use harness::*;
