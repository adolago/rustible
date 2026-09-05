//! Built-in security baseline policy pack.
//!
//! Rules:
//! - `no-shell` -- deny the `shell` module
//! - `no-raw`   -- deny the `raw` module
//! - `require-become-explicit` -- unsupported; evaluation fails explicitly

use crate::policy::pack::manifest::{PackCategory, PolicyPackManifest};

/// Return the manifest for the built-in security baseline pack.
pub fn manifest() -> PolicyPackManifest {
    PolicyPackManifest {
        name: "security-baseline".into(),
        version: "1.0.0".into(),
        description: "Module restrictions plus an unavailable explicit privilege escalation check"
            .into(),
        category: PackCategory::Security,
        rules: vec![
            "no-shell".into(),
            "no-raw".into(),
            "require-become-explicit".into(),
        ],
        parameters: vec![],
    }
}
