//! Variable Caching
//!
//! This module provides caching for resolved variable contexts.
//! Variable resolution, especially with complex Jinja2 templates,
//! can be expensive. Caching reduces template rendering time by ~80%.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::{Cache, CacheConfig, CacheDependency, CacheMetrics, CacheType};

/// Key for variable cache entries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariableCacheKey {
    /// Host this variable set applies to
    pub hostname: Option<String>,
    /// Play name
    pub play_name: Option<String>,
    /// Task name (if task-specific)
    pub task_name: Option<String>,
    /// Scope identifier
    pub scope: VariableScope,
}

impl VariableCacheKey {
    /// Create a global scope key
    pub fn global() -> Self {
        Self {
            hostname: None,
            play_name: None,
            task_name: None,
            scope: VariableScope::Global,
        }
    }

    /// Create a play scope key
    pub fn play(play_name: impl Into<String>) -> Self {
        Self {
            hostname: None,
            play_name: Some(play_name.into()),
            task_name: None,
            scope: VariableScope::Play,
        }
    }

    /// Create a host scope key
    pub fn host(hostname: impl Into<String>) -> Self {
        Self {
            hostname: Some(hostname.into()),
            play_name: None,
            task_name: None,
            scope: VariableScope::Host,
        }
    }

    /// Create a host-play scope key
    pub fn host_play(hostname: impl Into<String>, play_name: impl Into<String>) -> Self {
        Self {
            hostname: Some(hostname.into()),
            play_name: Some(play_name.into()),
            task_name: None,
            scope: VariableScope::HostPlay,
        }
    }

    /// Create a task scope key
    pub fn task(hostname: impl Into<String>, play_name: impl Into<String>, task_name: impl Into<String>) -> Self {
        Self {
            hostname: Some(hostname.into()),
            play_name: Some(play_name.into()),
            task_name: Some(task_name.into()),
            scope: VariableScope::Task,
        }
    }
}

/// Variable scope levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VariableScope {
    /// Global/playbook level
    Global,
    /// Play level
    Play,
    /// Host level
    Host,
    /// Host + Play level
    HostPlay,
    /// Task level
    Task,
}

/// Cached variable set
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachedVariables {
    /// The variables
    pub variables: IndexMap<String, JsonValue>,
    /// Source files that contributed to these variables
    pub source_files: Vec<PathBuf>,
    /// Whether these variables include vault-encrypted values
    pub has_vault_values: bool,
    /// Hash of the variable content for quick comparison
    pub content_hash: Option<u64>,
}

impl CachedVariables {
    /// Create new cached variables
    pub fn new(variables: IndexMap<String, JsonValue>) -> Self {
        let content_hash = Self::compute_hash(&variables);
        Self {
            variables,
            source_files: Vec::new(),
            has_vault_values: false,
            content_hash: Some(content_hash),
        }
    }

    /// Add a source file
    pub fn with_source(mut self, path: PathBuf) -> Self {
        self.source_files.push(path);
        self
    }

    /// Mark as containing vault values
    pub fn with_vault(mut self) -> Self {
        self.has_vault_values = true;
        self
    }

    /// Get a variable value
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.variables.get(key)
    }

    /// Get a variable as a string
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.variables.get(key).and_then(|v| v.as_str())
    }

    /// Merge with another variable set (other has higher precedence)
    pub fn merge(&mut self, other: &CachedVariables) {
        for (key, value) in &other.variables {
            self.variables.insert(key.clone(), value.clone());
        }
        self.source_files.extend(other.source_files.clone());
        self.has_vault_values = self.has_vault_values || other.has_vault_values;
        self.content_hash = Some(Self::compute_hash(&self.variables));
    }

    /// Estimate memory size
    pub fn size_bytes(&self) -> usize {
        serde_json::to_string(&self.variables)
            .map(|s| s.len())
            .unwrap_or(1000) +
        self.source_files.iter().map(|p| p.to_string_lossy().len()).sum::<usize>()
    }

    /// Compute a hash of the variables for comparison
    fn compute_hash(variables: &IndexMap<String, JsonValue>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // Sort keys for deterministic hashing
        let mut keys: Vec<_> = variables.keys().collect();
        keys.sort();

        for key in keys {
            key.hash(&mut hasher);
            if let Ok(json) = serde_json::to_string(&variables[key]) {
                json.hash(&mut hasher);
            }
        }

        hasher.finish()
    }

    /// Check if content matches another set
    pub fn content_matches(&self, other: &CachedVariables) -> bool {
        match (self.content_hash, other.content_hash) {
            (Some(h1), Some(h2)) => h1 == h2,
            _ => false,
        }
    }
}

/// Variable cache for storing resolved variable contexts
pub struct VariableCache {
    pub(crate) cache: Cache<VariableCacheKey, CachedVariables>,
    /// Template result cache (template hash -> result)
    template_cache: Cache<String, String>,
    /// Configuration
    config: VariableCacheConfig,
}

/// Configuration specific to variable caching
#[derive(Debug, Clone)]
pub struct VariableCacheConfig {
    /// TTL for variable cache entries
    pub variable_ttl: Duration,
    /// TTL for template results
    pub template_ttl: Duration,
    /// Whether to cache template results
    pub cache_templates: bool,
    /// Maximum template result size to cache
    pub max_template_size: usize,
}

impl Default for VariableCacheConfig {
    fn default() -> Self {
        Self {
            variable_ttl: Duration::from_secs(300), // 5 minutes
            template_ttl: Duration::from_secs(60),   // 1 minute (templates may use dynamic data)
            cache_templates: true,
            max_template_size: 64 * 1024, // 64 KB
        }
    }
}

impl VariableCache {
    /// Create a new variable cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Cache::new(CacheType::Variable, config.clone()),
            template_cache: Cache::new(CacheType::Template, config),
            config: VariableCacheConfig::default(),
        }
    }

    /// Create with custom variable cache configuration
    pub fn with_variable_config(config: CacheConfig, variable_config: VariableCacheConfig) -> Self {
        Self {
            cache: Cache::new(CacheType::Variable, config.clone()),
            template_cache: Cache::new(CacheType::Template, config),
            config: variable_config,
        }
    }

    /// Get cached variables
    pub fn get(&self, key: &VariableCacheKey) -> Option<CachedVariables> {
        self.cache.get(key)
    }

    /// Get cached variables for a host
    pub fn get_host(&self, hostname: &str) -> Option<CachedVariables> {
        self.cache.get(&VariableCacheKey::host(hostname))
    }

    /// Get cached global variables
    pub fn get_global(&self) -> Option<CachedVariables> {
        self.cache.get(&VariableCacheKey::global())
    }

    /// Store variables
    pub fn insert(&self, key: VariableCacheKey, variables: CachedVariables) {
        let size = variables.size_bytes();

        // Create dependencies from source files
        let deps: Vec<_> = variables.source_files.iter()
            .filter_map(|p| CacheDependency::file(p.clone()))
            .collect();

        self.cache.insert_with_dependencies(key, variables, deps, size);
    }

    /// Store host variables
    pub fn insert_host(&self, hostname: &str, variables: CachedVariables) {
        self.insert(VariableCacheKey::host(hostname), variables);
    }

    /// Store global variables
    pub fn insert_global(&self, variables: CachedVariables) {
        self.insert(VariableCacheKey::global(), variables);
    }

    /// Get a cached template result
    pub fn get_template(&self, template: &str, variables_hash: u64) -> Option<String> {
        if !self.config.cache_templates {
            return None;
        }

        let key = Self::template_key(template, variables_hash);
        self.template_cache.get(&key)
    }

    /// Cache a template result
    pub fn insert_template(&self, template: &str, variables_hash: u64, result: String) {
        if !self.config.cache_templates || result.len() > self.config.max_template_size {
            return;
        }

        let key = Self::template_key(template, variables_hash);
        let size = result.len();
        self.template_cache.insert_with_ttl(key, result, Some(self.config.template_ttl), size);
    }

    /// Generate template cache key
    fn template_key(template: &str, variables_hash: u64) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        template.hash(&mut hasher);
        variables_hash.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Invalidate variables for a host
    pub fn invalidate_host(&self, hostname: &str) {
        // Invalidate all scopes that include this host
        let keys_to_remove: Vec<_> = self.cache.entries.iter()
            .filter(|entry| entry.key().hostname.as_deref() == Some(hostname))
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
        }
    }

    /// Invalidate variables from a specific file
    pub fn invalidate_file(&self, path: &PathBuf) {
        let keys_to_remove: Vec<_> = self.cache.entries.iter()
            .filter(|entry| entry.value().value.source_files.contains(path))
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
        }
    }

    /// Clear all cached variables
    pub fn clear(&self) {
        self.cache.clear();
        self.template_cache.clear();
    }

    /// Get the number of cached entries
    pub fn len(&self) -> usize {
        self.cache.len() + self.template_cache.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty() && self.template_cache.is_empty()
    }

    /// Get cache metrics
    pub fn metrics(&self) -> Arc<CacheMetrics> {
        self.cache.metrics()
    }

    /// Cleanup expired entries
    pub fn cleanup_expired(&self) -> usize {
        self.cache.cleanup_expired() + self.template_cache.cleanup_expired()
    }

    /// Build a merged variable set from multiple scopes
    pub fn build_merged(&self, hostname: &str, play_name: Option<&str>) -> CachedVariables {
        let mut merged = CachedVariables::default();

        // Merge in order of precedence (lowest to highest)
        if let Some(global) = self.get_global() {
            merged.merge(&global);
        }

        if let Some(play_name) = play_name {
            if let Some(play_vars) = self.get(&VariableCacheKey::play(play_name)) {
                merged.merge(&play_vars);
            }
        }

        if let Some(host_vars) = self.get_host(hostname) {
            merged.merge(&host_vars);
        }

        if let Some(play_name) = play_name {
            if let Some(host_play_vars) = self.get(&VariableCacheKey::host_play(hostname, play_name)) {
                merged.merge(&host_play_vars);
            }
        }

        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_variables() -> IndexMap<String, JsonValue> {
        let mut vars = IndexMap::new();
        vars.insert("app_name".to_string(), JsonValue::String("myapp".to_string()));
        vars.insert("app_port".to_string(), JsonValue::Number(8080.into()));
        vars.insert("debug".to_string(), JsonValue::Bool(true));
        vars
    }

    #[test]
    fn test_variable_cache_basic() {
        let cache = VariableCache::new(CacheConfig::default());

        cache.insert_global(CachedVariables::new(sample_variables()));

        let cached = cache.get_global().unwrap();
        assert_eq!(cached.get_str("app_name"), Some("myapp"));
    }

    #[test]
    fn test_variable_cache_host() {
        let cache = VariableCache::new(CacheConfig::default());

        cache.insert_host("host1", CachedVariables::new(sample_variables()));

        let cached = cache.get_host("host1").unwrap();
        assert!(cached.get("app_port").is_some());
    }

    #[test]
    fn test_variable_cache_merge() {
        let cache = VariableCache::new(CacheConfig::default());

        let mut global = sample_variables();
        global.insert("global_var".to_string(), JsonValue::String("global".to_string()));
        cache.insert_global(CachedVariables::new(global));

        let mut host = IndexMap::new();
        host.insert("host_var".to_string(), JsonValue::String("host".to_string()));
        host.insert("app_name".to_string(), JsonValue::String("overridden".to_string()));
        cache.insert_host("host1", CachedVariables::new(host));

        let merged = cache.build_merged("host1", None);
        assert_eq!(merged.get_str("global_var"), Some("global"));
        assert_eq!(merged.get_str("host_var"), Some("host"));
        assert_eq!(merged.get_str("app_name"), Some("overridden")); // Host overrides global
    }

    #[test]
    fn test_variable_cache_template() {
        let cache = VariableCache::new(CacheConfig::default());

        let template = "Hello {{ name }}!";
        let vars_hash = 12345u64;
        let result = "Hello World!".to_string();

        cache.insert_template(template, vars_hash, result.clone());

        let cached = cache.get_template(template, vars_hash).unwrap();
        assert_eq!(cached, result);

        // Different hash should miss
        assert!(cache.get_template(template, 99999).is_none());
    }

    #[test]
    fn test_variable_cache_invalidate() {
        let cache = VariableCache::new(CacheConfig::default());

        cache.insert_host("host1", CachedVariables::new(sample_variables()));
        cache.insert_host("host2", CachedVariables::new(sample_variables()));

        cache.invalidate_host("host1");

        assert!(cache.get_host("host1").is_none());
        assert!(cache.get_host("host2").is_some());
    }

    #[test]
    fn test_cached_variables_content_hash() {
        let vars1 = CachedVariables::new(sample_variables());
        let vars2 = CachedVariables::new(sample_variables());

        // Same content should have same hash
        assert!(vars1.content_matches(&vars2));

        // Different content should not match
        let mut different = sample_variables();
        different.insert("extra".to_string(), JsonValue::String("value".to_string()));
        let vars3 = CachedVariables::new(different);

        assert!(!vars1.content_matches(&vars3));
    }
}
