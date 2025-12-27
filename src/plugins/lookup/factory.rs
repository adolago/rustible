//! Lookup Plugin Factory and Registry
//!
//! Provides factory pattern for creating lookup plugins and a registry
//! for managing available plugins.

use super::{
    CsvFileLookup, EnvLookup, FileLookup, LookupContext, LookupError, LookupOptions,
    LookupPlugin, LookupResult, PasswordLookup, PipeLookup, TemplateLookup,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Factory for creating lookup plugins
#[derive(Debug, Default)]
pub struct LookupFactory;

impl LookupFactory {
    /// Create a new factory
    pub fn new() -> Self {
        Self
    }

    /// Create a lookup plugin by name
    pub fn create(&self, name: &str) -> Option<Arc<dyn LookupPlugin>> {
        match name {
            "env" => Some(Arc::new(EnvLookup::new())),
            "file" => Some(Arc::new(FileLookup::new())),
            "csvfile" => Some(Arc::new(CsvFileLookup::new())),
            "password" => Some(Arc::new(PasswordLookup::new())),
            "pipe" => Some(Arc::new(PipeLookup::new())),
            "template" => Some(Arc::new(TemplateLookup::new())),
            _ => None,
        }
    }
}

/// Registry for lookup plugins
#[derive(Debug, Default)]
pub struct LookupRegistry {
    plugins: HashMap<String, Arc<dyn LookupPlugin>>,
}

impl LookupRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Create a registry with all built-in plugins
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(EnvLookup::new()));
        registry.register(Arc::new(FileLookup::new()));
        registry.register(Arc::new(CsvFileLookup::new()));
        registry.register(Arc::new(PasswordLookup::new()));
        registry.register(Arc::new(PipeLookup::new()));
        registry.register(Arc::new(TemplateLookup::new()));
        registry
    }

    /// Register a lookup plugin
    pub fn register(&mut self, plugin: Arc<dyn LookupPlugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    /// Get a lookup plugin by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn LookupPlugin>> {
        self.plugins.get(name).cloned()
    }

    /// Execute a lookup
    pub fn lookup(
        &self,
        name: &str,
        terms: &[&str],
        context: &LookupContext,
    ) -> LookupResult<Vec<serde_json::Value>> {
        self.lookup_with_options(name, terms, &LookupOptions::default(), context)
    }

    /// Execute a lookup with options
    pub fn lookup_with_options(
        &self,
        name: &str,
        terms: &[&str],
        options: &LookupOptions,
        context: &LookupContext,
    ) -> LookupResult<Vec<serde_json::Value>> {
        let plugin = self
            .get(name)
            .ok_or_else(|| LookupError::NotFound(name.to_string()))?;

        let terms: Vec<String> = terms.iter().map(|s| s.to_string()).collect();
        plugin.lookup(&terms, options, context)
    }

    /// List all registered plugin names
    pub fn list(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }
}
