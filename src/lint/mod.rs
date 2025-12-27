//! Playbook linting and validation.
//!
//! This module provides comprehensive linting capabilities for Ansible/Rustible
//! playbooks, including:
//!
//! - YAML syntax validation
//! - Module parameter checking
//! - Best practices enforcement
//! - Security vulnerability detection
//!
//! # Example
//!
//! ```rust,ignore
//! use rustible::lint::{Linter, LintConfig};
//!
//! let config = LintConfig::default();
//! let linter = Linter::new(config);
//! let result = linter.check_file("playbook.yml")?;
//!
//! for issue in result.issues() {
//!     println!("{}: {}", issue.severity, issue.message);
//! }
//! ```

mod best_practices;
mod params;
mod types;
mod yaml;

pub use best_practices::BestPracticesChecker;
pub use params::{ModuleDef, ParamDef, ParamType, ParamChecker};
pub use types::{
    LintConfig, LintError, LintIssue, LintOpResult, LintResult, Location,
    RuleCategory, Severity,
};
pub use yaml::YamlChecker;
