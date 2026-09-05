//! Built-in operations baseline policy pack.
//!
//! Declared checks currently lack evaluators and fail as unsupported:
//! `max-forks`, `require-limit`, and `deny-localhost-in-prod`.

use crate::policy::pack::manifest::{PackCategory, PackParameter, PolicyPackManifest};

/// Return the manifest for the built-in operations baseline pack.
pub fn manifest() -> PolicyPackManifest {
    PolicyPackManifest {
        name: "operations-baseline".into(),
        version: "1.0.0".into(),
        description:
            "Unavailable operations checks: fork limits, production limits and localhost targeting"
                .into(),
        category: PackCategory::Operations,
        rules: vec![
            "max-forks".into(),
            "require-limit".into(),
            "deny-localhost-in-prod".into(),
        ],
        parameters: vec![PackParameter {
            name: "max_forks".into(),
            description: "Reserved metadata; fork limit evaluation is unsupported".into(),
            param_type: "integer".into(),
            default_value: Some("50".into()),
            required: false,
        }],
    }
}
