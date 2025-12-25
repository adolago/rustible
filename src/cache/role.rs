//! Role Caching
//!
//! This module provides caching for loaded role definitions.
//! Roles often contain multiple files (tasks, handlers, vars, defaults, templates)
//! and caching the parsed structures provides significant speedup for repeated use.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::{Cache, CacheConfig, CacheDependency, CacheMetrics, CacheType};
use crate::executor::playbook::Role;
use crate::executor::task::{Handler, Task};

/// Key for role cache entries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoleCacheKey {
    /// Role name
    pub name: String,
    /// Path to the role directory
    pub path: Option<PathBuf>,
    /// tasks_from override
    pub tasks_from: Option<String>,
    /// vars_from override
    pub vars_from: Option<String>,
    /// defaults_from override
    pub defaults_from: Option<String>,
}

impl RoleCacheKey {
    /// Create a simple key with just the role name
    pub fn simple(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: None,
            tasks_from: None,
            vars_from: None,
            defaults_from: None,
        }
    }

    /// Create a key with role name and path
    pub fn with_path(name: impl Into<String>, path: PathBuf) -> Self {
        Self {
            name: name.into(),
            path: Some(path),
            tasks_from: None,
            vars_from: None,
            defaults_from: None,
        }
    }

    /// Create a key with overrides
    pub fn with_overrides(
        name: impl Into<String>,
        path: Option<PathBuf>,
        tasks_from: Option<String>,
        vars_from: Option<String>,
        defaults_from: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            path,
            tasks_from,
            vars_from,
            defaults_from,
        }
    }
}

/// Cached role data
#[derive(Debug, Clone)]
pub struct CachedRole {
    /// The parsed role
    pub role: Role,
    /// Role directory path
    pub role_path: Option<PathBuf>,
    /// All files that make up this role
    pub component_files: Vec<RoleFile>,
    /// Time taken to load (for metrics)
    pub load_time_ms: u64,
    /// Metadata about the role
    pub metadata: RoleMetadata,
}

/// A file that is part of a role
#[derive(Debug, Clone)]
pub struct RoleFile {
    /// Path to the file
    pub path: PathBuf,
    /// Type of file
    pub file_type: RoleFileType,
    /// Modification time when cached
    pub modified_at: Option<SystemTime>,
}

impl RoleFile {
    /// Create a new role file
    pub fn new(path: PathBuf, file_type: RoleFileType) -> Self {
        let modified_at = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok();
        Self {
            path,
            file_type,
            modified_at,
        }
    }

    /// Check if the file has been modified
    pub fn is_modified(&self) -> bool {
        if let Some(cached_mtime) = self.modified_at {
            if let Ok(current_mtime) = std::fs::metadata(&self.path).and_then(|m| m.modified()) {
                return current_mtime != cached_mtime;
            }
        }
        // If we can't check, assume modified
        true
    }
}

/// Types of files in a role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleFileType {
    Tasks,
    Handlers,
    Defaults,
    Vars,
    Templates,
    Files,
    Meta,
    Library,
}

/// Metadata about a role
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleMetadata {
    /// Role author
    pub author: Option<String>,
    /// Role description
    pub description: Option<String>,
    /// Minimum ansible version
    pub min_ansible_version: Option<String>,
    /// Role dependencies
    pub dependencies: Vec<String>,
    /// Platforms supported
    pub platforms: Vec<String>,
    /// Galaxy tags
    pub galaxy_tags: Vec<String>,
}

impl CachedRole {
    /// Create a new cached role
    pub fn new(role: Role, role_path: Option<PathBuf>) -> Self {
        let component_files = Self::discover_role_files(&role_path);

        Self {
            role,
            role_path,
            component_files,
            load_time_ms: 0,
            metadata: RoleMetadata::default(),
        }
    }

    /// Create with load timing
    pub fn with_load_time(mut self, load_time_ms: u64) -> Self {
        self.load_time_ms = load_time_ms;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: RoleMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Discover all files in a role directory
    fn discover_role_files(role_path: &Option<PathBuf>) -> Vec<RoleFile> {
        let Some(path) = role_path else {
            return Vec::new();
        };

        let mut files = Vec::new();

        // Check each standard role directory
        let directories = [
            ("tasks", RoleFileType::Tasks),
            ("handlers", RoleFileType::Handlers),
            ("defaults", RoleFileType::Defaults),
            ("vars", RoleFileType::Vars),
            ("templates", RoleFileType::Templates),
            ("files", RoleFileType::Files),
            ("meta", RoleFileType::Meta),
            ("library", RoleFileType::Library),
        ];

        for (dir_name, file_type) in directories {
            let dir_path = path.join(dir_name);
            if dir_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&dir_path) {
                    for entry in entries.flatten() {
                        if entry.path().is_file() {
                            files.push(RoleFile::new(entry.path(), file_type));
                        }
                    }
                }
            }
        }

        files
    }

    /// Check if any component files have been modified
    pub fn is_modified(&self) -> bool {
        self.component_files.iter().any(|f| f.is_modified())
    }

    /// Get file paths for dependency tracking
    pub fn get_dependency_paths(&self) -> Vec<PathBuf> {
        self.component_files.iter().map(|f| f.path.clone()).collect()
    }

    /// Estimate memory size
    pub fn size_bytes(&self) -> usize {
        // Rough estimation
        self.role.name.len() +
        self.role.tasks.len() * 200 +
        self.role.handlers.len() * 150 +
        serde_json::to_string(&self.role.defaults)
            .map(|s| s.len())
            .unwrap_or(500) +
        serde_json::to_string(&self.role.vars)
            .map(|s| s.len())
            .unwrap_or(500) +
        self.component_files.len() * 100
    }
}

/// Role cache for storing loaded roles
pub struct RoleCache {
    pub(crate) cache: Cache<RoleCacheKey, CachedRole>,
    /// Map of role name to path for quick lookup
    name_to_path: dashmap::DashMap<String, PathBuf>,
    /// Configuration
    config: RoleCacheConfig,
}

/// Configuration specific to role caching
#[derive(Debug, Clone)]
pub struct RoleCacheConfig {
    /// TTL for role cache entries
    pub role_ttl: Duration,
    /// Whether to validate role files on access
    pub validate_files: bool,
    /// Role search paths
    pub role_paths: Vec<PathBuf>,
}

impl Default for RoleCacheConfig {
    fn default() -> Self {
        Self {
            role_ttl: Duration::from_secs(600), // 10 minutes
            validate_files: true,
            role_paths: vec![
                PathBuf::from("./roles"),
                dirs::home_dir()
                    .map(|p| p.join(".ansible/roles"))
                    .unwrap_or_else(|| PathBuf::from("~/.ansible/roles")),
                PathBuf::from("/etc/ansible/roles"),
            ],
        }
    }
}

impl RoleCache {
    /// Create a new role cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Cache::new(CacheType::Role, config),
            name_to_path: dashmap::DashMap::new(),
            config: RoleCacheConfig::default(),
        }
    }

    /// Create with custom role cache configuration
    pub fn with_role_config(config: CacheConfig, role_config: RoleCacheConfig) -> Self {
        Self {
            cache: Cache::new(CacheType::Role, config),
            name_to_path: dashmap::DashMap::new(),
            config: role_config,
        }
    }

    /// Get a cached role by key
    pub fn get(&self, key: &RoleCacheKey) -> Option<Role> {
        self.cache.get(key)
            .filter(|cached| {
                if self.config.validate_files {
                    !cached.is_modified()
                } else {
                    true
                }
            })
            .map(|cached| cached.role)
    }

    /// Get a cached role by name (searches in standard paths)
    pub fn get_by_name(&self, name: &str) -> Option<Role> {
        // First check if we have a cached path
        if let Some(path) = self.name_to_path.get(name) {
            return self.get(&RoleCacheKey::with_path(name, path.value().clone()));
        }

        // Search in role paths
        for role_path in &self.config.role_paths {
            let full_path = role_path.join(name);
            if full_path.exists() {
                self.name_to_path.insert(name.to_string(), full_path.clone());
                return self.get(&RoleCacheKey::with_path(name, full_path));
            }
        }

        // Try simple key
        self.get(&RoleCacheKey::simple(name))
    }

    /// Store a loaded role
    pub fn insert(&self, key: RoleCacheKey, role: Role) {
        let path = key.path.clone();
        let cached = CachedRole::new(role, path.clone());
        let size = cached.size_bytes();

        // Create dependencies from component files
        let deps: Vec<_> = cached.component_files.iter()
            .filter_map(|f| CacheDependency::file(f.path.clone()))
            .collect();

        // Store name to path mapping
        if let Some(p) = path {
            self.name_to_path.insert(key.name.clone(), p);
        }

        self.cache.insert_with_dependencies(key, cached, deps, size);
    }

    /// Store a loaded role with timing information
    pub fn insert_with_timing(&self, key: RoleCacheKey, role: Role, load_time_ms: u64) {
        let path = key.path.clone();
        let cached = CachedRole::new(role, path.clone())
            .with_load_time(load_time_ms);
        let size = cached.size_bytes();

        let deps: Vec<_> = cached.component_files.iter()
            .filter_map(|f| CacheDependency::file(f.path.clone()))
            .collect();

        if let Some(p) = path {
            self.name_to_path.insert(key.name.clone(), p);
        }

        self.cache.insert_with_dependencies(key, cached, deps, size);
    }

    /// Invalidate a cached role for a specific path
    pub fn invalidate_file(&self, path: &PathBuf) {
        // Find and remove any roles that depend on this file
        let keys_to_remove: Vec<_> = self.cache.entries.iter()
            .filter(|entry| {
                entry.value().value.component_files.iter()
                    .any(|f| &f.path == path)
            })
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
            self.name_to_path.remove(&key.name);
        }
    }

    /// Invalidate a role by name
    pub fn invalidate_by_name(&self, name: &str) {
        let keys_to_remove: Vec<_> = self.cache.entries.iter()
            .filter(|entry| entry.key().name == name)
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.cache.remove(&key);
        }
        self.name_to_path.remove(name);
    }

    /// Clear all cached roles
    pub fn clear(&self) {
        self.cache.clear();
        self.name_to_path.clear();
    }

    /// Get the number of cached entries
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get cache metrics
    pub fn metrics(&self) -> Arc<CacheMetrics> {
        self.cache.metrics()
    }

    /// Cleanup expired entries
    pub fn cleanup_expired(&self) -> usize {
        let removed = self.cache.cleanup_expired();

        // Also clean up name_to_path for removed entries
        let remaining_names: std::collections::HashSet<_> = self.cache.entries.iter()
            .map(|e| e.key().name.clone())
            .collect();

        let names_to_remove: Vec<_> = self.name_to_path.iter()
            .filter(|e| !remaining_names.contains(e.key()))
            .map(|e| e.key().clone())
            .collect();

        for name in names_to_remove {
            self.name_to_path.remove(&name);
        }

        removed
    }

    /// Get all cached role names
    pub fn role_names(&self) -> Vec<String> {
        self.cache.entries.iter()
            .map(|e| e.key().name.clone())
            .collect()
    }

    /// Add a role search path
    pub fn add_role_path(&mut self, path: PathBuf) {
        if !self.config.role_paths.contains(&path) {
            self.config.role_paths.push(path);
        }
    }

    /// Find a role path by name
    pub fn find_role_path(&self, name: &str) -> Option<PathBuf> {
        for role_path in &self.config.role_paths {
            let full_path = role_path.join(name);
            if full_path.exists() {
                return Some(full_path);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_role() -> Role {
        Role::new("test-role")
    }

    #[test]
    fn test_role_cache_basic() {
        let cache = RoleCache::new(CacheConfig::default());
        let key = RoleCacheKey::simple("test-role");

        cache.insert(key.clone(), sample_role());

        let cached = cache.get(&key).unwrap();
        assert_eq!(cached.name, "test-role");
    }

    #[test]
    fn test_role_cache_key_with_overrides() {
        let key = RoleCacheKey::with_overrides(
            "my-role",
            Some(PathBuf::from("/path/to/role")),
            Some("install.yml".to_string()),
            None,
            None,
        );

        assert_eq!(key.name, "my-role");
        assert_eq!(key.tasks_from, Some("install.yml".to_string()));
    }

    #[test]
    fn test_role_cache_invalidate() {
        let cache = RoleCache::new(CacheConfig::default());

        cache.insert(RoleCacheKey::simple("role1"), sample_role());
        cache.insert(RoleCacheKey::simple("role2"), sample_role());

        cache.invalidate_by_name("role1");

        assert!(cache.get(&RoleCacheKey::simple("role1")).is_none());
        assert!(cache.get(&RoleCacheKey::simple("role2")).is_some());
    }

    #[test]
    fn test_role_file_discovery() {
        // This test would need actual role directories
        // For now, just test with None path
        let cached = CachedRole::new(sample_role(), None);
        assert!(cached.component_files.is_empty());
    }

    #[test]
    fn test_cached_role_size() {
        let cached = CachedRole::new(sample_role(), None);
        let size = cached.size_bytes();

        assert!(size > 0);
        assert!(size < 10000);
    }
}
