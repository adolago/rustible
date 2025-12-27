//! Template Lookup Plugin
//!
//! Renders templates with variable interpolation.

use super::{LookupContext, LookupError, LookupOptions, LookupPlugin, LookupResult};
use minijinja::Environment;

/// Template lookup plugin
#[derive(Debug, Clone, Default)]
pub struct TemplateLookup;

impl TemplateLookup {
    /// Create a new TemplateLookup instance
    pub fn new() -> Self {
        Self
    }
}

impl LookupPlugin for TemplateLookup {
    fn name(&self) -> &'static str {
        "template"
    }

    fn description(&self) -> &'static str {
        "Renders templates with variable interpolation"
    }

    fn lookup(
        &self,
        terms: &[String],
        _options: &LookupOptions,
        context: &LookupContext,
    ) -> LookupResult<Vec<serde_json::Value>> {
        let mut results = Vec::new();
        let env = Environment::new();

        for term in terms {
            let template = env.template_from_str(term).map_err(|e| {
                LookupError::TemplateError(e.to_string())
            })?;

            let rendered = template.render(&context.variables).map_err(|e| {
                LookupError::TemplateError(e.to_string())
            })?;

            results.push(serde_json::Value::String(rendered));
        }

        Ok(results)
    }
}
