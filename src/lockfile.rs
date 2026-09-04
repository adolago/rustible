//! Lockfile Support for Reproducible Playbook Execution
//!
//! This module provides lockfile functionality similar to Cargo.lock or package-lock.json,
//! enabling reproducible playbook executions by pinning versions of:
//!
//! - Ansible Galaxy roles and collections
//! - Python module dependencies
//! - External resources (URLs, git refs)
//!
//! ## Usage
//!
//! ```bash
//! # Generate a lockfile from a playbook
//! rustible lock playbook.yml
//!
//! # Run with locked versions (fail if lockfile outdated)
//! rustible run --frozen playbook.yml
//!
//! # Update lockfile
//! rustible lock --update playbook.yml
//! ```
//!
//! ## Lockfile Format
//!
//! The lockfile uses TOML format for Rust ecosystem consistency.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors that can occur during lockfile operations
#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("Lockfile not found: {0}")]
    NotFound(PathBuf),

    #[error("Lockfile is outdated - playbook has changed. Run 'rustible lock' to update.")]
    Outdated,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("Integrity check failed for {name}: expected {expected}, got {actual}")]
    IntegrityFailed {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("Dependency resolution failed: {0}")]
    ResolutionFailed(String),

    #[error("Version constraint not satisfied: {0}")]
    VersionConstraint(String),
}

/// Result type for lockfile operations
pub type LockfileResult<T> = Result<T, LockfileError>;

/// The main lockfile structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile format version
    pub version: u32,
    /// When the lockfile was generated
    pub created_at: DateTime<Utc>,
    /// When the lockfile was last updated
    pub updated_at: DateTime<Utc>,
    /// SHA256 hash of the playbook content
    pub playbook_hash: String,
    /// Path to the playbook (relative)
    pub playbook_path: String,
    /// Locked Galaxy roles
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roles: BTreeMap<String, LockedRole>,
    /// Locked Galaxy collections
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub collections: BTreeMap<String, LockedCollection>,
    /// Locked external resources (URLs, files)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resources: BTreeMap<String, LockedResource>,
    /// Metadata
    #[serde(default, skip_serializing_if = "LockfileMetadata::is_empty")]
    pub metadata: LockfileMetadata,
}

/// Metadata about the lockfile
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockfileMetadata {
    /// Rustible version that generated the lockfile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator_version: Option<String>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Custom user metadata
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom: BTreeMap<String, String>,
}

impl LockfileMetadata {
    fn is_empty(&self) -> bool {
        self.generator_version.is_none() && self.description.is_none() && self.custom.is_empty()
    }
}

/// A locked Galaxy role
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedRole {
    /// Role name (namespace.name format)
    pub name: String,
    /// Exact version
    pub version: String,
    /// Source URL or Galaxy reference
    pub source: DependencySource,
    /// SHA256 checksum of the role archive
    pub checksum: String,
    /// Dependencies of this role
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

/// A locked Galaxy collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedCollection {
    /// Collection FQCN (namespace.name)
    pub name: String,
    /// Exact version
    pub version: String,
    /// Source URL or Galaxy reference
    pub source: DependencySource,
    /// SHA256 checksum
    pub checksum: String,
    /// Dependencies
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

/// A locked external resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedResource {
    /// Resource identifier
    pub name: String,
    /// Resource type
    pub resource_type: ResourceType,
    /// URL or path
    pub location: String,
    /// SHA256 checksum (for files/downloads)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    /// Git commit SHA (for git resources)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
}

/// Source of a dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DependencySource {
    /// Ansible Galaxy
    Galaxy { server: Option<String> },
    /// Git repository
    Git { url: String, ref_: String },
    /// Direct URL
    Url { url: String },
    /// Local path
    Local { path: String },
}

/// Type of external resource
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    /// HTTP/HTTPS URL
    Url,
    /// Git repository
    Git,
    /// Local file
    File,
    /// S3 object
    S3,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            playbook_hash: String::new(),
            playbook_path: String::new(),
            roles: BTreeMap::new(),
            collections: BTreeMap::new(),
            resources: BTreeMap::new(),
            metadata: LockfileMetadata {
                generator_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                ..Default::default()
            },
        }
    }
}

impl Lockfile {
    /// Create a new lockfile for a playbook
    pub fn new(playbook_path: impl AsRef<Path>) -> LockfileResult<Self> {
        let path = playbook_path.as_ref();
        let content = fs::read_to_string(path)?;
        let hash = compute_hash(&content);

        Ok(Self {
            playbook_hash: hash,
            playbook_path: path.to_string_lossy().to_string(),
            ..Default::default()
        })
    }

    /// Load a lockfile from disk
    pub fn load(path: impl AsRef<Path>) -> LockfileResult<Self> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|_| LockfileError::NotFound(path.as_ref().to_path_buf()))?;
        let lockfile: Lockfile = toml::from_str(&content)?;
        Ok(lockfile)
    }

    /// Save the lockfile to disk
    pub fn save(&self, path: impl AsRef<Path>) -> LockfileResult<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get the default lockfile path for a playbook
    pub fn default_path(playbook_path: impl AsRef<Path>) -> PathBuf {
        let path = playbook_path.as_ref();
        let parent = path.parent().unwrap_or(Path::new("."));
        parent.join("rustible.lock")
    }

    /// Check if the lockfile matches the playbook
    pub fn verify_playbook(&self, playbook_path: impl AsRef<Path>) -> LockfileResult<()> {
        let content = fs::read_to_string(playbook_path)?;
        let current_hash = compute_hash(&content);

        if current_hash != self.playbook_hash {
            return Err(LockfileError::Outdated);
        }

        Ok(())
    }

    /// Add a role to the lockfile
    pub fn add_role(&mut self, role: LockedRole) {
        self.roles.insert(role.name.clone(), role);
        self.updated_at = Utc::now();
    }

    /// Add a collection to the lockfile
    pub fn add_collection(&mut self, collection: LockedCollection) {
        self.collections.insert(collection.name.clone(), collection);
        self.updated_at = Utc::now();
    }

    /// Add an external resource
    pub fn add_resource(&mut self, resource: LockedResource) {
        self.resources.insert(resource.name.clone(), resource);
        self.updated_at = Utc::now();
    }

    /// Get a locked role by name
    pub fn get_role(&self, name: &str) -> Option<&LockedRole> {
        self.roles.get(name)
    }

    /// Get a locked collection by name
    pub fn get_collection(&self, name: &str) -> Option<&LockedCollection> {
        self.collections.get(name)
    }

    /// Verify regular local file checksums with bounded read buffers.
    /// Unsupported dependency sources and special files fail closed.
    ///
    /// Role/collection installation paths and remote content are not resolved by
    /// this API, so their integrity cannot be established here.
    pub fn verify_integrity(&self) -> LockfileResult<()> {
        if !self.roles.is_empty() || !self.collections.is_empty() {
            return Err(LockfileError::ResolutionFailed(
                "Integrity verification for roles and collections is not supported; no installed artifacts were verified".into(),
            ));
        }
        for resource in self.resources.values() {
            if resource.resource_type != ResourceType::File {
                return Err(LockfileError::ResolutionFailed(format!(
                    "Integrity verification for {:?} resource '{}' is not supported",
                    resource.resource_type, resource.name,
                )));
            }
            let expected = resource.checksum.as_deref().ok_or_else(|| {
                LockfileError::ResolutionFailed(format!(
                    "No SHA256 checksum recorded for local resource '{}'",
                    resource.name,
                ))
            })?;
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                // Check the opened descriptor, not only a racy path lookup.
                // A FIFO swapped into the path must not wait for a writer.
                options.custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC);
            }
            let mut file = options.open(&resource.location)?;
            if !file.metadata()?.is_file() {
                return Err(LockfileError::ResolutionFailed(format!(
                    "Integrity verification requires a regular local file for resource '{}'",
                    resource.name,
                )));
            }
            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 8192];
            loop {
                match file.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => hasher.update(&buffer[..count]),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error.into()),
                }
            }
            let actual = format!("{:x}", hasher.finalize());
            if !expected.eq_ignore_ascii_case(&actual) {
                return Err(LockfileError::IntegrityFailed {
                    name: resource.name.clone(),
                    expected: expected.to_string(),
                    actual,
                });
            }
        }
        Ok(())
    }

    /// Check if lockfile has any locked dependencies
    pub fn is_empty(&self) -> bool {
        self.roles.is_empty() && self.collections.is_empty() && self.resources.is_empty()
    }

    /// Get total number of locked items
    pub fn len(&self) -> usize {
        self.roles.len() + self.collections.len() + self.resources.len()
    }

    /// Update the playbook hash
    pub fn update_playbook_hash(&mut self, playbook_path: impl AsRef<Path>) -> LockfileResult<()> {
        let content = fs::read_to_string(playbook_path.as_ref())?;
        self.playbook_hash = compute_hash(&content);
        self.playbook_path = playbook_path.as_ref().to_string_lossy().to_string();
        self.updated_at = Utc::now();
        Ok(())
    }
}

/// Compute SHA256 hash of content
fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Lockfile manager for playbook execution
#[derive(Debug)]
pub struct LockfileManager {
    /// Current lockfile
    lockfile: Option<Lockfile>,
    /// Path to the lockfile
    lockfile_path: PathBuf,
    /// Frozen mode (fail if lockfile outdated)
    frozen: bool,
}

impl LockfileManager {
    /// Create a new lockfile manager
    pub fn new(playbook_path: impl AsRef<Path>) -> Self {
        let lockfile_path = Lockfile::default_path(&playbook_path);
        Self {
            lockfile: None,
            lockfile_path,
            frozen: false,
        }
    }

    /// Enable frozen mode
    pub fn frozen(mut self, frozen: bool) -> Self {
        self.frozen = frozen;
        self
    }

    /// Set custom lockfile path
    pub fn with_lockfile_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.lockfile_path = path.into();
        self
    }

    /// Load existing lockfile if present
    pub fn load(&mut self) -> LockfileResult<bool> {
        if self.lockfile_path.exists() {
            self.lockfile = Some(Lockfile::load(&self.lockfile_path)?);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the current lockfile
    pub fn lockfile(&self) -> Option<&Lockfile> {
        self.lockfile.as_ref()
    }

    /// Get mutable reference to lockfile, creating if needed
    pub fn lockfile_mut(
        &mut self,
        playbook_path: impl AsRef<Path>,
    ) -> LockfileResult<&mut Lockfile> {
        if self.lockfile.is_none() {
            self.lockfile = Some(Lockfile::new(playbook_path)?);
        }
        Ok(self.lockfile.as_mut().unwrap())
    }

    /// Check if running in frozen mode
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    /// Verify lockfile matches playbook (for frozen mode)
    pub fn verify(&self, playbook_path: impl AsRef<Path>) -> LockfileResult<()> {
        if self.frozen {
            match &self.lockfile {
                Some(lf) => lf.verify_playbook(playbook_path),
                None => Err(LockfileError::NotFound(self.lockfile_path.clone())),
            }
        } else {
            Ok(())
        }
    }

    /// Save the lockfile
    pub fn save(&self) -> LockfileResult<()> {
        if let Some(ref lf) = self.lockfile {
            lf.save(&self.lockfile_path)?;
        }
        Ok(())
    }

    /// Get locked version of a role (if available)
    pub fn get_locked_role_version(&self, role_name: &str) -> Option<&str> {
        self.lockfile
            .as_ref()
            .and_then(|lf| lf.get_role(role_name))
            .map(|r| r.version.as_str())
    }

    /// Get locked version of a collection (if available)
    pub fn get_locked_collection_version(&self, collection_name: &str) -> Option<&str> {
        self.lockfile
            .as_ref()
            .and_then(|lf| lf.get_collection(collection_name))
            .map(|c| c.version.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_diligence_integrity_rejects_unverified_dependencies() {
        let mut lockfile = Lockfile::default();
        assert!(lockfile.verify_integrity().is_ok());
        lockfile.add_role(LockedRole {
            name: "test.role".into(),
            version: "1.0.0".into(),
            source: DependencySource::Galaxy { server: None },
            checksum: "not-verified".into(),
            dependencies: Vec::new(),
        });
        assert!(lockfile.verify_integrity().is_err());
    }

    #[test]
    fn test_diligence_integrity_rejects_missing_or_modified_local_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("resource");
        let mut lockfile = Lockfile::default();
        lockfile.add_resource(LockedResource {
            name: "local-fixture".into(),
            resource_type: ResourceType::File,
            location: path.to_string_lossy().into_owned(),
            checksum: Some(compute_hash("expected")),
            git_ref: None,
        });
        assert!(lockfile.verify_integrity().is_err());
        fs::write(&path, "modified").unwrap();
        assert!(lockfile.verify_integrity().is_err());
        fs::write(&path, "expected").unwrap();
        assert!(lockfile.verify_integrity().is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn test_diligence_integrity_fifo_child() {
        let Ok(path) = std::env::var("RUSTIBLE_DILIGENCE_INTEGRITY_FIFO") else {
            return;
        };
        let mut lockfile = Lockfile::default();
        lockfile.add_resource(LockedResource {
            name: "private-fifo".into(),
            resource_type: ResourceType::File,
            location: path,
            checksum: Some(compute_hash("")),
            git_ref: None,
        });
        assert!(matches!(
            lockfile.verify_integrity(),
            Err(LockfileError::ResolutionFailed(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_diligence_integrity_rejects_fifo_without_waiting_for_writer() {
        let dir = TempDir::new().unwrap();
        let fifo = dir.path().join("fifo");
        nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRUSR).unwrap();
        let alias = dir.path().join("fifo-link");
        std::os::unix::fs::symlink(&fifo, &alias).unwrap();
        for path in [&fifo, &alias] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["test_diligence_integrity_fifo_child", "--nocapture"])
                .env("RUSTIBLE_DILIGENCE_INTEGRITY_FIFO", path)
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if std::time::Instant::now() >= deadline {
                    child.kill().unwrap();
                    break child.wait().unwrap();
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            assert!(
                status.success(),
                "non-regular input blocked or was accepted"
            );
        }
    }

    #[test]
    fn test_diligence_integrity_hashes_multiple_chunks() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("resource");
        let bytes: Vec<u8> = (0..(9 * 8192 + 3))
            .map(|index| (index % 251) as u8)
            .collect();
        fs::write(&path, &bytes).unwrap();
        let mut lockfile = Lockfile::default();
        lockfile.add_resource(LockedResource {
            name: "multiple-chunks".into(),
            resource_type: ResourceType::File,
            location: path.to_string_lossy().into_owned(),
            checksum: Some(format!("{:x}", Sha256::digest(&bytes))),
            git_ref: None,
        });
        assert!(lockfile.verify_integrity().is_ok());
        #[cfg(unix)]
        {
            let alias = dir.path().join("regular-link");
            std::os::unix::fs::symlink(&path, &alias).unwrap();
            lockfile
                .resources
                .get_mut("multiple-chunks")
                .unwrap()
                .location = alias.to_string_lossy().into_owned();
            assert!(lockfile.verify_integrity().is_ok());
        }
    }

    #[test]
    fn test_lockfile_creation() {
        let temp = TempDir::new().unwrap();
        let playbook_path = temp.path().join("playbook.yml");
        fs::write(&playbook_path, "- hosts: all\n  tasks: []").unwrap();

        let lockfile = Lockfile::new(&playbook_path).unwrap();
        assert!(!lockfile.playbook_hash.is_empty());
        assert_eq!(lockfile.version, 1);
    }

    #[test]
    fn test_lockfile_save_load() {
        let temp = TempDir::new().unwrap();
        let playbook_path = temp.path().join("playbook.yml");
        let lockfile_path = temp.path().join("rustible.lock");

        fs::write(&playbook_path, "- hosts: all\n  tasks: []").unwrap();

        let mut lockfile = Lockfile::new(&playbook_path).unwrap();
        lockfile.add_role(LockedRole {
            name: "geerlingguy.nginx".to_string(),
            version: "3.1.0".to_string(),
            source: DependencySource::Galaxy { server: None },
            checksum: "abc123".to_string(),
            dependencies: vec![],
        });

        lockfile.save(&lockfile_path).unwrap();

        let loaded = Lockfile::load(&lockfile_path).unwrap();
        assert_eq!(loaded.roles.len(), 1);
        assert!(loaded.get_role("geerlingguy.nginx").is_some());
    }

    #[test]
    fn test_lockfile_verify_playbook() {
        let temp = TempDir::new().unwrap();
        let playbook_path = temp.path().join("playbook.yml");

        fs::write(&playbook_path, "- hosts: all").unwrap();
        let lockfile = Lockfile::new(&playbook_path).unwrap();

        // Should pass with same content
        assert!(lockfile.verify_playbook(&playbook_path).is_ok());

        // Should fail with modified content
        fs::write(&playbook_path, "- hosts: webservers").unwrap();
        assert!(matches!(
            lockfile.verify_playbook(&playbook_path),
            Err(LockfileError::Outdated)
        ));
    }

    #[test]
    fn test_compute_hash() {
        let hash1 = compute_hash("hello world");
        let hash2 = compute_hash("hello world");
        let hash3 = compute_hash("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA256 hex
    }

    #[test]
    fn test_lockfile_manager() {
        let temp = TempDir::new().unwrap();
        let playbook_path = temp.path().join("playbook.yml");

        fs::write(&playbook_path, "- hosts: all").unwrap();

        let mut manager = LockfileManager::new(&playbook_path);
        assert!(!manager.load().unwrap()); // No lockfile yet

        let lf = manager.lockfile_mut(&playbook_path).unwrap();
        lf.add_collection(LockedCollection {
            name: "community.general".to_string(),
            version: "8.0.0".to_string(),
            source: DependencySource::Galaxy { server: None },
            checksum: "def456".to_string(),
            dependencies: vec![],
        });

        manager.save().unwrap();

        // Reload
        let mut manager2 = LockfileManager::new(&playbook_path);
        assert!(manager2.load().unwrap());
        assert_eq!(
            manager2.get_locked_collection_version("community.general"),
            Some("8.0.0")
        );
    }

    #[test]
    fn test_lockfile_toml_format() {
        let temp = TempDir::new().unwrap();
        let playbook_path = temp.path().join("playbook.yml");
        let lockfile_path = temp.path().join("rustible.lock");

        fs::write(&playbook_path, "- hosts: all").unwrap();

        let mut lockfile = Lockfile::new(&playbook_path).unwrap();
        lockfile.add_role(LockedRole {
            name: "test.role".to_string(),
            version: "1.0.0".to_string(),
            source: DependencySource::Git {
                url: "https://github.com/test/role.git".to_string(),
                ref_: "v1.0.0".to_string(),
            },
            checksum: "abc".to_string(),
            dependencies: vec!["dep.role".to_string()],
        });

        lockfile.save(&lockfile_path).unwrap();

        let content = fs::read_to_string(&lockfile_path).unwrap();
        assert!(content.contains("[roles.\"test.role\"]"));
        assert!(content.contains("version = \"1.0.0\""));
    }
}
