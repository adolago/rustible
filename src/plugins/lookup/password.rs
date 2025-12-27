//! Password Lookup Plugin
//!
//! Generates or retrieves passwords.

use super::{LookupContext, LookupOptions, LookupPlugin, LookupResult};
use rand::Rng;

/// Password lookup plugin
#[derive(Debug, Clone, Default)]
pub struct PasswordLookup;

impl PasswordLookup {
    /// Create a new PasswordLookup instance
    pub fn new() -> Self {
        Self
    }

    /// Generate a random password
    fn generate_password(&self, length: usize, chars: &str) -> String {
        let mut rng = rand::thread_rng();
        let chars: Vec<char> = chars.chars().collect();
        (0..length)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect()
    }
}

impl LookupPlugin for PasswordLookup {
    fn name(&self) -> &'static str {
        "password"
    }

    fn description(&self) -> &'static str {
        "Generates or retrieves passwords"
    }

    fn lookup(
        &self,
        terms: &[String],
        options: &LookupOptions,
        _context: &LookupContext,
    ) -> LookupResult<Vec<serde_json::Value>> {
        let length = options.get_u64("length").unwrap_or(16) as usize;
        let chars = options
            .get_string("chars")
            .unwrap_or_else(|| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_string());

        let mut results = Vec::new();

        for _term in terms {
            // For now, just generate a new password for each term
            // In a full implementation, this would store/retrieve passwords
            let password = self.generate_password(length, &chars);
            results.push(serde_json::Value::String(password));
        }

        Ok(results)
    }
}
