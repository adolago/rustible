//! Build script.
//!
//! Records the host target triple so the agent builder can tell a native build
//! from a cross build. Deriving the triple from `std::env::consts` produces
//! strings cargo rejects (`x86_64-linux-gnu`), so take the one cargo itself
//! reports.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=RUSTIBLE_HOST_TARGET={}", target);
    println!("cargo:rerun-if-changed=build.rs");
}
