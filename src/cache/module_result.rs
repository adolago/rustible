//! Module Result Caching for Idempotent Operations
//!
//! This module provides intelligent caching of module execution results to avoid
//! redundant operations. The cache uses a composite key based on:
//! - Module name
//! - Module parameters (normalized and hashed)
//! - Host context (hostname, become user)
//! - Check mode state
//!
//! # Cache Key Strategy
//!
//! The cache key is designed to be:
//! 1. **Deterministic**: Same inputs always produce the same key
//! 2. **Collision-resistant**: Different operations produce different keys
//! 3. **Efficient**: Fast to compute using xxhash
//!
//! # Idempotency Detection
//!
//! Modules are classified by their idempotency characteristics:
//! - **Fully Idempotent**: Safe to cache indefinitely (e.g., stat, file with state=directory)
//! - **State-Based Idempotent**: Cache valid until state changes (e.g., copy, template)
//! - **Non-Idempotent**: Never cache (e.g., shell, command without creates/removes)
//!
//! # Performance Benefits
//!
//! - Avoid redundant remote commands for already-completed tasks
//! - Skip file transfers when content hasn't changed
//! - Reduce SSH round-trips for idempotent checks

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tracing::{debug, trace};

use super::{CacheConfig, CacheMetrics};

/// Cache key for module results
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleCacheKey {
    /// Module name
    pub module: String,
    /// Hash of normalized parameters
    pub params_hash: u64,
    /// Target host
    pub host: String,
    /// Whether running in check mode
    pub check_mode: bool,
    /// Become user if applicable
    pub become_user: Option<String>,
}

impl ModuleCacheKey {
    /// Create a new cache key
    pub fn new(
        module: impl Into<String>,
        params: &HashMap<String, JsonValue>,
        host: impl Into<String>,
        check_mode: bool,
        become_user: Option<String>,
    ) -> Self {
        Self {
            module: module.into(),
            params_hash: Self::hash_params(params),
            host: host.into(),
            check_mode,
            become_user,
        }
    }

    /// Hash parameters deterministically
    fn hash_params(params: &HashMap<String, JsonValue>) -> u64 {
        use std::collections::BTreeMap;
        // Sort params for deterministic hashing
        let sorted: BTreeMap<_, _> = params.iter().collect();

        // Use the default hasher for simplicity and portability
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        for (key, value) in sorted {
            key.hash(&mut hasher);
            // Hash the JSON string representation for values
            let json_str = serde_json::to_string(value).unwrap_or_default();
            json_str.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Create a string representation for logging
    pub fn to_display_string(&self) -> String {
        format!(
            "{}@{}[{:016x}]{}",
            self.module,
            self.host,
            self.params_hash,
            if self.check_mode { "(check)" } else { "" }
        )
    }
}

/// Cached module result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedModuleResult {
    /// Whether the module changed anything
    pub changed: bool,
    /// Human-readable message
    pub msg: String,
    /// Whether the execution was successful
    pub success: bool,
    /// Optional diff information
    pub diff: Option<CachedDiff>,
    /// Additional data returned by the module
    #[serde(default)]
    pub data: HashMap<String, JsonValue>,
    /// When this result was cached
    #[serde(skip)]
    pub cached_at: Option<Instant>,
    /// Time-to-live for this entry
    #[serde(skip)]
    pub ttl: Option<Duration>,
}

/// Cached diff information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedDiff {
    pub before: String,
    pub after: String,
    pub details: Option<String>,
}

/// Idempotency classification for modules
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotencyClass {
    /// Always safe to cache (stat, debug, assert)
    FullyIdempotent,
    /// Safe to cache based on file state (copy, template, file)
    StateBasedIdempotent,
    /// Safe to cache only with creates/removes (command, shell)
    ConditionallyIdempotent,
    /// Never cache (raw, script without creates)
    NonIdempotent,
}

impl IdempotencyClass {
    /// Get the default TTL for this idempotency class
    pub fn default_ttl(&self) -> Option<Duration> {
        match self {
            IdempotencyClass::FullyIdempotent => Some(Duration::from_secs(600)), // 10 minutes
            IdempotencyClass::StateBasedIdempotent => Some(Duration::from_secs(120)), // 2 minutes
            IdempotencyClass::ConditionallyIdempotent => Some(Duration::from_secs(60)), // 1 minute
            IdempotencyClass::NonIdempotent => None,                             // Never cache
        }
    }

    /// Determine if caching is allowed
    pub fn is_cacheable(&self) -> bool {
        !matches!(self, IdempotencyClass::NonIdempotent)
    }
}

/// Classify a module by its idempotency characteristics
pub fn classify_module_idempotency(
    module: &str,
    params: &HashMap<String, JsonValue>,
) -> IdempotencyClass {
    match module {
        // Fully idempotent - read-only or pure logic
        "stat" | "debug" | "assert" | "fail" | "meta" | "set_fact" | "gather_facts" => {
            IdempotencyClass::FullyIdempotent
        }

        // State-based idempotent - file operations with idempotent semantics
        "copy" | "template" | "file" | "lineinfile" | "blockinfile" => {
            IdempotencyClass::StateBasedIdempotent
        }

        // Package managers are idempotent when state is specified
        "apt" | "dnf" | "yum" | "pip" | "package" => {
            if params.contains_key("state") {
                IdempotencyClass::StateBasedIdempotent
            } else {
                IdempotencyClass::ConditionallyIdempotent
            }
        }

        // Service is idempotent when managing state
        "service" | "systemd" => {
            if params.contains_key("state") || params.contains_key("enabled") {
                IdempotencyClass::StateBasedIdempotent
            } else {
                IdempotencyClass::ConditionallyIdempotent
            }
        }

        // User/group management is idempotent
        "user" | "group" => IdempotencyClass::StateBasedIdempotent,

        // Git with specific version is idempotent
        "git" => {
            if params.contains_key("version") || params.contains_key("refspec") {
                IdempotencyClass::StateBasedIdempotent
            } else {
                IdempotencyClass::ConditionallyIdempotent
            }
        }

        // Command/shell are only cacheable with creates/removes
        "command" | "shell" | "raw" => {
            if params.contains_key("creates") || params.contains_key("removes") {
                IdempotencyClass::ConditionallyIdempotent
            } else {
                IdempotencyClass::NonIdempotent
            }
        }

        // Script is non-idempotent unless creates/removes specified
        "script" => {
            if params.contains_key("creates") || params.contains_key("removes") {
                IdempotencyClass::ConditionallyIdempotent
            } else {
                IdempotencyClass::NonIdempotent
            }
        }

        // Unknown modules default to non-idempotent for safety
        _ => IdempotencyClass::NonIdempotent,
    }
}

/// Internal cache entry
struct CacheEntry {
    result: CachedModuleResult,
    created_at: Instant,
    expires_at: Option<Instant>,
    access_count: AtomicU64,
    last_accessed: RwLock<Instant>,
    size_bytes: usize,
}

impl CacheEntry {
    fn new(result: CachedModuleResult, ttl: Option<Duration>) -> Self {
        let now = Instant::now();
        let size_bytes =
            std::mem::size_of::<CachedModuleResult>() + result.msg.len() + result.data.len() * 64; // Estimate

        Self {
            result,
            created_at: now,
            expires_at: ttl.map(|d| now + d),
            access_count: AtomicU64::new(0),
            last_accessed: RwLock::new(now),
            size_bytes,
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at
            .map(|e| Instant::now() >= e)
            .unwrap_or(false)
    }

    fn record_access(&self) {
        self.access_count.fetch_add(1, Ordering::Relaxed);
        *self.last_accessed.write() = Instant::now();
    }
}

/// Module result cache
pub struct ModuleResultCache {
    entries: DashMap<ModuleCacheKey, CacheEntry>,
    config: CacheConfig,
    metrics: Arc<CacheMetrics>,
}

impl ModuleResultCache {
    /// Create a new module result cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: DashMap::with_capacity(config.max_entries.min(1000)),
            metrics: Arc::new(CacheMetrics::new()),
            config,
        }
    }

    /// Get a cached result if available
    pub fn get(&self, key: &ModuleCacheKey) -> Option<CachedModuleResult> {
        if self.config.max_entries == 0 {
            self.metrics.record_miss();
            return None;
        }

        if let Some(entry) = self.entries.get(key) {
            if entry.is_expired() {
                drop(entry);
                self.entries.remove(key);
                self.metrics.record_miss();
                self.metrics.record_eviction();
                debug!("Cache miss (expired): {}", key.to_display_string());
                return None;
            }

            entry.record_access();
            self.metrics.record_hit();
            trace!("Cache hit: {}", key.to_display_string());
            Some(entry.result.clone())
        } else {
            self.metrics.record_miss();
            trace!("Cache miss: {}", key.to_display_string());
            None
        }
    }

    /// Store a result in the cache
    pub fn put(
        &self,
        key: ModuleCacheKey,
        result: CachedModuleResult,
        idempotency: IdempotencyClass,
    ) {
        if !idempotency.is_cacheable() {
            trace!(
                "Not caching non-idempotent result: {}",
                key.to_display_string()
            );
            return;
        }

        if self.config.max_entries == 0 {
            return;
        }

        // Evict if at capacity
        if self.entries.len() >= self.config.max_entries {
            self.evict_lru();
        }

        let ttl = idempotency.default_ttl().or(Some(self.config.default_ttl));
        let entry = CacheEntry::new(result, ttl);

        let size = entry.size_bytes;
        self.entries.insert(key.clone(), entry);
        self.metrics
            .entries
            .store(self.entries.len(), Ordering::Relaxed);
        self.metrics.memory_bytes.fetch_add(size, Ordering::Relaxed);

        trace!("Cached result: {}", key.to_display_string());
    }

    /// Invalidate a specific cache entry
    pub fn invalidate(&self, key: &ModuleCacheKey) {
        if let Some((_, entry)) = self.entries.remove(key) {
            self.metrics
                .entries
                .store(self.entries.len(), Ordering::Relaxed);
            self.metrics.memory_bytes.fetch_sub(
                entry
                    .size_bytes
                    .min(self.metrics.memory_bytes.load(Ordering::Relaxed)),
                Ordering::Relaxed,
            );
            self.metrics.record_invalidation();
            debug!("Invalidated cache: {}", key.to_display_string());
        }
    }

    /// Invalidate all entries for a specific host
    pub fn invalidate_host(&self, host: &str) {
        let keys_to_remove: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.key().host == host)
            .map(|e| e.key().clone())
            .collect();

        for key in keys_to_remove {
            self.invalidate(&key);
        }
    }

    /// Invalidate all entries for a specific module
    pub fn invalidate_module(&self, module: &str) {
        let keys_to_remove: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.key().module == module)
            .map(|e| e.key().clone())
            .collect();

        for key in keys_to_remove {
            self.invalidate(&key);
        }
    }

    /// Clear all cached results
    pub fn clear(&self) {
        let count = self.entries.len();
        self.entries.clear();
        self.metrics.entries.store(0, Ordering::Relaxed);
        self.metrics.memory_bytes.store(0, Ordering::Relaxed);
        for _ in 0..count {
            self.metrics.record_invalidation();
        }
    }

    /// Get cache metrics
    pub fn metrics(&self) -> Arc<CacheMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Get the number of cached entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Cleanup expired entries
    pub fn cleanup_expired(&self) -> usize {
        let mut removed = 0;
        let keys_to_remove: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.value().is_expired())
            .map(|e| e.key().clone())
            .collect();

        for key in keys_to_remove {
            if let Some((_, entry)) = self.entries.remove(&key) {
                self.metrics.memory_bytes.fetch_sub(
                    entry
                        .size_bytes
                        .min(self.metrics.memory_bytes.load(Ordering::Relaxed)),
                    Ordering::Relaxed,
                );
                removed += 1;
                self.metrics.record_eviction();
            }
        }

        self.metrics
            .entries
            .store(self.entries.len(), Ordering::Relaxed);
        removed
    }

    /// Evict least recently used entry
    fn evict_lru(&self) {
        let mut oldest: Option<(ModuleCacheKey, Instant)> = None;

        for entry in self.entries.iter() {
            let last_accessed = *entry.value().last_accessed.read();
            if oldest.is_none() || last_accessed < oldest.as_ref().unwrap().1 {
                oldest = Some((entry.key().clone(), last_accessed));
            }
        }

        if let Some((key, _)) = oldest {
            if let Some((_, entry)) = self.entries.remove(&key) {
                self.metrics.memory_bytes.fetch_sub(
                    entry
                        .size_bytes
                        .min(self.metrics.memory_bytes.load(Ordering::Relaxed)),
                    Ordering::Relaxed,
                );
                self.metrics.record_eviction();
            }
        }
    }
}

impl Default for ModuleResultCache {
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_creation() {
        let mut params = HashMap::new();
        params.insert("name".to_string(), JsonValue::String("nginx".to_string()));
        params.insert(
            "state".to_string(),
            JsonValue::String("present".to_string()),
        );

        let key = ModuleCacheKey::new("apt", &params, "host1", false, None);

        assert_eq!(key.module, "apt");
        assert_eq!(key.host, "host1");
        assert!(!key.check_mode);
    }

    #[test]
    fn test_cache_key_determinism() {
        let mut params1 = HashMap::new();
        params1.insert("name".to_string(), JsonValue::String("nginx".to_string()));
        params1.insert(
            "state".to_string(),
            JsonValue::String("present".to_string()),
        );

        let mut params2 = HashMap::new();
        // Insert in different order
        params2.insert(
            "state".to_string(),
            JsonValue::String("present".to_string()),
        );
        params2.insert("name".to_string(), JsonValue::String("nginx".to_string()));

        let key1 = ModuleCacheKey::new("apt", &params1, "host1", false, None);
        let key2 = ModuleCacheKey::new("apt", &params2, "host1", false, None);

        // Should produce same hash regardless of insertion order
        assert_eq!(key1.params_hash, key2.params_hash);
    }

    #[test]
    fn test_idempotency_classification() {
        let params = HashMap::new();

        assert_eq!(
            classify_module_idempotency("stat", &params),
            IdempotencyClass::FullyIdempotent
        );

        assert_eq!(
            classify_module_idempotency("copy", &params),
            IdempotencyClass::StateBasedIdempotent
        );

        assert_eq!(
            classify_module_idempotency("command", &params),
            IdempotencyClass::NonIdempotent
        );

        let mut cmd_params = HashMap::new();
        cmd_params.insert(
            "creates".to_string(),
            JsonValue::String("/tmp/marker".to_string()),
        );

        assert_eq!(
            classify_module_idempotency("command", &cmd_params),
            IdempotencyClass::ConditionallyIdempotent
        );
    }

    #[test]
    fn test_cache_put_get() {
        let cache = ModuleResultCache::new(CacheConfig::default());

        let mut params = HashMap::new();
        params.insert("name".to_string(), JsonValue::String("nginx".to_string()));

        let key = ModuleCacheKey::new("apt", &params, "host1", false, None);
        let result = CachedModuleResult {
            changed: false,
            msg: "Package already installed".to_string(),
            success: true,
            diff: None,
            data: HashMap::new(),
            cached_at: None,
            ttl: None,
        };

        cache.put(
            key.clone(),
            result.clone(),
            IdempotencyClass::StateBasedIdempotent,
        );

        let cached = cache.get(&key);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().msg, "Package already installed");
    }

    #[test]
    fn test_cache_non_idempotent_not_cached() {
        let cache = ModuleResultCache::new(CacheConfig::default());

        let params = HashMap::new();
        let key = ModuleCacheKey::new("shell", &params, "host1", false, None);
        let result = CachedModuleResult {
            changed: true,
            msg: "Command executed".to_string(),
            success: true,
            diff: None,
            data: HashMap::new(),
            cached_at: None,
            ttl: None,
        };

        cache.put(key.clone(), result, IdempotencyClass::NonIdempotent);

        // Should not be cached
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = ModuleResultCache::new(CacheConfig::default());

        let mut params = HashMap::new();
        params.insert("name".to_string(), JsonValue::String("nginx".to_string()));

        let key = ModuleCacheKey::new("apt", &params, "host1", false, None);
        let result = CachedModuleResult {
            changed: false,
            msg: "ok".to_string(),
            success: true,
            diff: None,
            data: HashMap::new(),
            cached_at: None,
            ttl: None,
        };

        cache.put(key.clone(), result, IdempotencyClass::StateBasedIdempotent);
        assert!(cache.get(&key).is_some());

        cache.invalidate(&key);
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cache_host_invalidation() {
        let cache = ModuleResultCache::new(CacheConfig::default());

        let params = HashMap::new();

        let key1 = ModuleCacheKey::new("stat", &params, "host1", false, None);
        let key2 = ModuleCacheKey::new("stat", &params, "host2", false, None);

        let result = CachedModuleResult {
            changed: false,
            msg: "ok".to_string(),
            success: true,
            diff: None,
            data: HashMap::new(),
            cached_at: None,
            ttl: None,
        };

        cache.put(
            key1.clone(),
            result.clone(),
            IdempotencyClass::FullyIdempotent,
        );
        cache.put(key2.clone(), result, IdempotencyClass::FullyIdempotent);

        cache.invalidate_host("host1");

        assert!(cache.get(&key1).is_none());
        assert!(cache.get(&key2).is_some());
    }
}
