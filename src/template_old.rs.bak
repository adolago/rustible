//! Template engine for Rustible (Jinja2-compatible)

use crate::error::Result;
use minijinja::Environment;
use std::collections::HashMap;

/// Template engine using minijinja
pub struct TemplateEngine {
    env: Environment<'static>,
}

impl TemplateEngine {
    /// Create a new template engine
    pub fn new() -> Self {
        let env = Environment::new();
        Self { env }
    }

    /// Render a template string
    pub fn render(
        &self,
        template: &str,
        vars: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let tmpl = self.env.template_from_str(template)?;
        let result = tmpl.render(vars)?;
        Ok(result)
    }

    /// Check if a string contains template syntax
    pub fn is_template(s: &str) -> bool {
        s.contains("{{") || s.contains("{%")
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}
