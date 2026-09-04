//! Distributed state locking mechanism.
//!
//! Prevents concurrent modifications to infrastructure state by implementing
//! distributed locking with support for multiple backends and TTL expiration.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio::sync::RwLock;

/// Serializes read/modify/write of lock metadata across processes. The sidecar
/// must never be removed: unlinking an advisory-locked inode creates two locks.
pub(crate) struct FileMetadataGuard {
    #[cfg(unix)]
    _file: nix::fcntl::Flock<std::fs::File>,
}

pub(crate) async fn lock_file_metadata(
    path: PathBuf,
    timeout: std::time::Duration,
) -> std::io::Result<FileMetadataGuard> {
    #[cfg(unix)]
    {
        let started = std::time::Instant::now();
        loop {
            let path = path.clone();
            let result =
                tokio::task::spawn_blocking(move || -> std::io::Result<FileMetadataGuard> {
                    use std::os::unix::fs::OpenOptionsExt;
                    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut sidecar = path.into_os_string();
                    sidecar.push(".guard");
                    let file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .mode(0o600)
                        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                        .open(sidecar)?;
                    let file =
                        nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock)
                            .map_err(|(_, error)| {
                                std::io::Error::from_raw_os_error(error as i32)
                            })?;
                    Ok(FileMetadataGuard { _file: file })
                })
                .await
                .map_err(std::io::Error::other)?;
            match result {
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && started.elapsed() < timeout =>
                {
                    // Do not occupy blocking-pool threads while another owner
                    // needs the same pool to finish its metadata IO.
                    tokio::time::sleep(
                        std::time::Duration::from_millis(5)
                            .min(timeout.saturating_sub(started.elapsed())),
                    )
                    .await;
                }
                result => return result,
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, timeout);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Safe local state locking is not supported on this platform",
        ))
    }
}

/// Publish complete, private metadata atomically while the sidecar guard is held.
pub(crate) fn write_lock_metadata(path: &std::path::Path, content: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Errors that can occur in state locking operations.
#[derive(Debug, Error)]
pub enum LockError {
    #[error("Lock acquisition failed: {0}")]
    AcquireFailed(String),

    #[error("Lock is already held by: {holder} since {since}")]
    AlreadyHeld {
        holder: String,
        since: DateTime<Utc>,
    },

    #[error("Lock release failed: {0}")]
    ReleaseFailed(String),

    #[error("Lock has expired")]
    Expired,

    #[error("Lock validation failed: {0}")]
    ValidationFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Lock not found: {0}")]
    NotFound(String),
}

/// Result type for lock operations.
pub type LockResult<T> = Result<T, LockError>;

/// Metadata about a held lock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockMetadata {
    /// Unique lock identifier.
    pub lock_id: String,

    /// Entity holding the lock (e.g., hostname, process ID).
    pub holder: String,

    /// When the lock was acquired.
    pub acquired_at: DateTime<Utc>,

    /// When the lock will expire.
    pub expires_at: DateTime<Utc>,

    /// Lock description or purpose.
    pub description: Option<String>,

    /// Associated run ID if applicable.
    pub run_id: Option<String>,
}

impl LockMetadata {
    /// Create new lock metadata.
    pub fn new(
        lock_id: String,
        holder: String,
        ttl: Duration,
        description: Option<String>,
        run_id: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            lock_id,
            holder,
            acquired_at: now,
            expires_at: now + ttl,
            description,
            run_id,
        }
    }

    /// Check if the lock has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Get remaining time until expiration.
    pub fn remaining_ttl(&self) -> Duration {
        let now = Utc::now();
        if self.expires_at > now {
            self.expires_at - now
        } else {
            Duration::seconds(0)
        }
    }

    /// Extend the lock expiration time.
    pub fn extend(&mut self, ttl: Duration) {
        self.expires_at = Utc::now() + ttl;
    }
}

/// Trait for distributed lock backends.
#[async_trait]
pub trait LockBackend: Send + Sync {
    /// Acquire a lock with the given ID.
    async fn acquire(
        &self,
        lock_id: &str,
        holder: &str,
        ttl: Duration,
        description: Option<String>,
    ) -> LockResult<LockMetadata>;

    /// Release a held lock.
    async fn release(&self, lock_id: &str) -> LockResult<()>;

    /// Try to acquire a lock without waiting.
    async fn try_acquire(
        &self,
        lock_id: &str,
        holder: &str,
        ttl: Duration,
        description: Option<String>,
    ) -> LockResult<LockMetadata>;

    /// Check if a lock is held.
    async fn is_locked(&self, lock_id: &str) -> LockResult<bool>;

    /// Get metadata for a held lock.
    async fn get_metadata(&self, lock_id: &str) -> LockResult<Option<LockMetadata>>;

    /// Extend a lock's TTL.
    async fn extend(&self, lock_id: &str, ttl: Duration) -> LockResult<()>;

    /// List all active locks.
    async fn list_locks(&self) -> LockResult<Vec<LockMetadata>>;

    /// Release expired locks.
    async fn release_expired(&self) -> LockResult<usize>;
}

/// In-memory lock backend for testing and local operations.
#[derive(Debug, Clone)]
pub struct InMemoryLockBackend {
    locks: Arc<RwLock<HashMap<String, LockMetadata>>>,
}

impl InMemoryLockBackend {
    /// Create a new in-memory lock backend.
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryLockBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LockBackend for InMemoryLockBackend {
    async fn acquire(
        &self,
        lock_id: &str,
        holder: &str,
        ttl: Duration,
        description: Option<String>,
    ) -> LockResult<LockMetadata> {
        let mut locks = self.locks.write().await;

        if let Some(existing) = locks.get(lock_id) {
            if !existing.is_expired() {
                return Err(LockError::AlreadyHeld {
                    holder: existing.holder.clone(),
                    since: existing.acquired_at,
                });
            }
        }

        let metadata = LockMetadata::new(
            lock_id.to_string(),
            holder.to_string(),
            ttl,
            description,
            None,
        );

        locks.insert(lock_id.to_string(), metadata.clone());
        Ok(metadata)
    }

    async fn release(&self, lock_id: &str) -> LockResult<()> {
        let mut locks = self.locks.write().await;
        locks
            .remove(lock_id)
            .ok_or_else(|| LockError::NotFound(lock_id.to_string()))?;
        Ok(())
    }

    async fn try_acquire(
        &self,
        lock_id: &str,
        holder: &str,
        ttl: Duration,
        description: Option<String>,
    ) -> LockResult<LockMetadata> {
        self.acquire(lock_id, holder, ttl, description).await
    }

    async fn is_locked(&self, lock_id: &str) -> LockResult<bool> {
        let locks = self.locks.read().await;

        if let Some(metadata) = locks.get(lock_id) {
            Ok(!metadata.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn get_metadata(&self, lock_id: &str) -> LockResult<Option<LockMetadata>> {
        let locks = self.locks.read().await;

        if let Some(metadata) = locks.get(lock_id) {
            if metadata.is_expired() {
                Ok(None)
            } else {
                Ok(Some(metadata.clone()))
            }
        } else {
            Ok(None)
        }
    }

    async fn extend(&self, lock_id: &str, ttl: Duration) -> LockResult<()> {
        let mut locks = self.locks.write().await;

        let metadata = locks
            .get_mut(lock_id)
            .ok_or_else(|| LockError::NotFound(lock_id.to_string()))?;

        if metadata.is_expired() {
            return Err(LockError::Expired);
        }

        metadata.extend(ttl);
        Ok(())
    }

    async fn list_locks(&self) -> LockResult<Vec<LockMetadata>> {
        let locks = self.locks.read().await;

        Ok(locks
            .values()
            .filter(|m| !m.is_expired())
            .cloned()
            .collect())
    }

    async fn release_expired(&self) -> LockResult<usize> {
        let mut locks = self.locks.write().await;
        let now = Utc::now();

        let expired: Vec<String> = locks
            .iter()
            .filter(|(_, m)| m.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();

        for lock_id in &expired {
            locks.remove(lock_id);
        }

        Ok(expired.len())
    }
}

/// File-based lock backend for single-machine coordination.
#[derive(Debug, Clone)]
pub struct FileLockBackend {
    lock_dir: PathBuf,
}

impl FileLockBackend {
    /// Create a new file-based lock backend.
    pub fn new(lock_dir: impl Into<PathBuf>) -> Self {
        Self {
            lock_dir: lock_dir.into(),
        }
    }

    /// Get the path to a lock file.
    fn lock_path(&self, lock_id: &str) -> PathBuf {
        self.lock_dir.join(format!("{}.lock", lock_id))
    }

    /// Ensure lock directory exists.
    async fn ensure_dir(&self) -> LockResult<()> {
        fs::create_dir_all(&self.lock_dir).await?;
        Ok(())
    }

    /// Write lock metadata to file.
    async fn write_lock(&self, lock_id: &str, metadata: &LockMetadata) -> LockResult<()> {
        self.ensure_dir().await?;

        let path = self.lock_path(lock_id);
        let json = serde_json::to_string_pretty(metadata)
            .map_err(|e| LockError::Serialization(e.to_string()))?;

        write_lock_metadata(&path, json.as_bytes())?;
        Ok(())
    }

    /// Read lock metadata from file.
    async fn read_lock(&self, lock_id: &str) -> LockResult<Option<LockMetadata>> {
        let path = self.lock_path(lock_id);

        if !path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&path).await?;
        let metadata: LockMetadata =
            serde_json::from_str(&json).map_err(|e| LockError::Serialization(e.to_string()))?;

        Ok(Some(metadata))
    }
}

#[async_trait]
impl LockBackend for FileLockBackend {
    async fn acquire(
        &self,
        lock_id: &str,
        holder: &str,
        ttl: Duration,
        description: Option<String>,
    ) -> LockResult<LockMetadata> {
        let _guard = lock_file_metadata(self.lock_path(lock_id), std::time::Duration::ZERO).await?;
        if let Some(existing) = self.read_lock(lock_id).await? {
            if !existing.is_expired() {
                return Err(LockError::AlreadyHeld {
                    holder: existing.holder,
                    since: existing.acquired_at,
                });
            }
        }

        let metadata = LockMetadata::new(
            lock_id.to_string(),
            holder.to_string(),
            ttl,
            description,
            None,
        );

        self.write_lock(lock_id, &metadata).await?;
        Ok(metadata)
    }

    async fn release(&self, lock_id: &str) -> LockResult<()> {
        let path = self.lock_path(lock_id);
        let _guard = lock_file_metadata(path.clone(), std::time::Duration::from_secs(30)).await?;

        if !path.exists() {
            return Err(LockError::NotFound(lock_id.to_string()));
        }

        fs::remove_file(path).await?;
        Ok(())
    }

    async fn try_acquire(
        &self,
        lock_id: &str,
        holder: &str,
        ttl: Duration,
        description: Option<String>,
    ) -> LockResult<LockMetadata> {
        self.acquire(lock_id, holder, ttl, description).await
    }

    async fn is_locked(&self, lock_id: &str) -> LockResult<bool> {
        if let Some(metadata) = self.read_lock(lock_id).await? {
            Ok(!metadata.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn get_metadata(&self, lock_id: &str) -> LockResult<Option<LockMetadata>> {
        if let Some(metadata) = self.read_lock(lock_id).await? {
            if metadata.is_expired() {
                Ok(None)
            } else {
                Ok(Some(metadata))
            }
        } else {
            Ok(None)
        }
    }

    async fn extend(&self, lock_id: &str, ttl: Duration) -> LockResult<()> {
        let _guard =
            lock_file_metadata(self.lock_path(lock_id), std::time::Duration::from_secs(30)).await?;
        let mut metadata = self
            .read_lock(lock_id)
            .await?
            .ok_or_else(|| LockError::NotFound(lock_id.to_string()))?;

        if metadata.is_expired() {
            return Err(LockError::Expired);
        }

        metadata.extend(ttl);
        self.write_lock(lock_id, &metadata).await?;
        Ok(())
    }

    async fn list_locks(&self) -> LockResult<Vec<LockMetadata>> {
        let mut locks = Vec::new();

        if !self.lock_dir.exists() {
            return Ok(locks);
        }

        let mut entries = fs::read_dir(&self.lock_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("lock") {
                continue;
            }

            if let Some(metadata) = self
                .read_lock(
                    path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                        LockError::Serialization("Invalid lock filename".to_string())
                    })?,
                )
                .await?
            {
                if !metadata.is_expired() {
                    locks.push(metadata);
                }
            }
        }

        Ok(locks)
    }

    async fn release_expired(&self) -> LockResult<usize> {
        let mut count = 0;

        if !self.lock_dir.exists() {
            return Ok(0);
        }

        let mut entries = fs::read_dir(&self.lock_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("lock") {
                continue;
            }

            if let Some(lock_id) = path.file_stem().and_then(|s| s.to_str()) {
                let _guard =
                    lock_file_metadata(path.clone(), std::time::Duration::from_secs(30)).await?;
                if let Some(metadata) = self.read_lock(lock_id).await? {
                    if metadata.is_expired() {
                        fs::remove_file(&path).await?;
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }
}

/// Distributed state lock manager.
#[derive(Debug, Clone)]
pub struct StateLock<B: LockBackend> {
    backend: B,
    default_ttl: Duration,
    lock_timeout: Duration,
}

impl<B: LockBackend> StateLock<B> {
    /// Create a new state lock manager.
    pub fn new(backend: B, default_ttl: Duration) -> Self {
        Self {
            backend,
            default_ttl,
            lock_timeout: Duration::seconds(30),
        }
    }

    /// Set the lock acquisition timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.lock_timeout = timeout;
        self
    }

    /// Acquire a lock for a state file.
    pub async fn acquire_state_lock(
        &self,
        inventory: &str,
        description: Option<String>,
    ) -> LockResult<LockMetadata> {
        let lock_id = format!("state:{}", inventory);
        let holder = self.get_holder_identifier();
        self.backend
            .acquire(&lock_id, &holder, self.default_ttl, description)
            .await
    }

    /// Try to acquire a lock without waiting.
    pub async fn try_acquire_state_lock(
        &self,
        inventory: &str,
        description: Option<String>,
    ) -> LockResult<LockMetadata> {
        let lock_id = format!("state:{}", inventory);
        let holder = self.get_holder_identifier();
        self.backend
            .try_acquire(&lock_id, &holder, self.default_ttl, description)
            .await
    }

    /// Release a state lock.
    pub async fn release_state_lock(&self, inventory: &str) -> LockResult<()> {
        let lock_id = format!("state:{}", inventory);
        self.backend.release(&lock_id).await
    }

    /// Check if a state lock is held.
    pub async fn is_state_locked(&self, inventory: &str) -> LockResult<bool> {
        let lock_id = format!("state:{}", inventory);
        self.backend.is_locked(&lock_id).await
    }

    /// Get state lock metadata.
    pub async fn get_state_lock_metadata(
        &self,
        inventory: &str,
    ) -> LockResult<Option<LockMetadata>> {
        let lock_id = format!("state:{}", inventory);
        self.backend.get_metadata(&lock_id).await
    }

    /// Extend a state lock.
    pub async fn extend_state_lock(
        &self,
        inventory: &str,
        ttl: Option<Duration>,
    ) -> LockResult<()> {
        let lock_id = format!("state:{}", inventory);
        let ttl = ttl.unwrap_or(self.default_ttl);
        self.backend.extend(&lock_id, ttl).await
    }

    /// List all active locks.
    pub async fn list_active_locks(&self) -> LockResult<Vec<LockMetadata>> {
        self.backend.list_locks().await
    }

    /// Release expired locks.
    pub async fn cleanup_expired_locks(&self) -> LockResult<usize> {
        self.backend.release_expired().await
    }

    /// Get the holder identifier (hostname + process ID).
    fn get_holder_identifier(&self) -> String {
        let hostname = hostname::get()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".to_string());
        let pid = std::process::id();
        format!("{}:{}", hostname, pid)
    }
}

impl StateLock<InMemoryLockBackend> {
    /// Create an in-memory state lock.
    pub fn in_memory(default_ttl_seconds: i64) -> Self {
        Self::new(
            InMemoryLockBackend::new(),
            Duration::seconds(default_ttl_seconds),
        )
    }
}

impl StateLock<FileLockBackend> {
    /// Create a file-based state lock.
    pub fn file_based(lock_dir: impl Into<PathBuf>, default_ttl_seconds: i64) -> Self {
        Self::new(
            FileLockBackend::new(lock_dir),
            Duration::seconds(default_ttl_seconds),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_diligence_state_file_lock_has_one_concurrent_winner() {
        let dir = tempfile::tempdir().unwrap();
        let barrier = Arc::new(tokio::sync::Barrier::new(16));
        let mut tasks = Vec::new();
        for index in 0..16 {
            let backend = FileLockBackend::new(dir.path());
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                backend
                    .acquire(
                        "shared",
                        &format!("holder{index}"),
                        Duration::minutes(5),
                        None,
                    )
                    .await
            }));
        }
        let mut winners = 0;
        for task in tasks {
            if task.await.unwrap().is_ok() {
                winners += 1;
            }
        }
        assert_eq!(
            winners, 1,
            "concurrent state lock ownership is not exclusive"
        );
    }

    #[tokio::test]
    async fn test_in_memory_lock_acquire_release() {
        let lock = StateLock::in_memory(60);

        let metadata = lock
            .acquire_state_lock("test_inventory", Some("Test lock".to_string()))
            .await
            .unwrap();

        assert_eq!(metadata.lock_id, "state:test_inventory");
        assert!(!metadata.is_expired());

        lock.release_state_lock("test_inventory").await.unwrap();
    }

    #[tokio::test]
    async fn test_in_memory_lock_conflict() {
        let lock = StateLock::in_memory(60);

        lock.acquire_state_lock("test_inventory", Some("First lock".to_string()))
            .await
            .unwrap();

        let result = lock
            .try_acquire_state_lock("test_inventory", Some("Second lock".to_string()))
            .await;

        assert!(result.is_err());

        lock.release_state_lock("test_inventory").await.unwrap();
    }

    #[tokio::test]
    async fn test_in_memory_lock_expiration() {
        let lock = StateLock::in_memory(0);

        lock.acquire_state_lock("test_inventory", None)
            .await
            .unwrap();

        // Give it a moment to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Should be able to acquire again
        lock.acquire_state_lock("test_inventory", None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_in_memory_lock_extend() {
        let lock = StateLock::in_memory(1);

        lock.acquire_state_lock("test_inventory", None)
            .await
            .unwrap();

        lock.extend_state_lock("test_inventory", Some(Duration::seconds(10)))
            .await
            .unwrap();

        let metadata = lock
            .get_state_lock_metadata("test_inventory")
            .await
            .unwrap()
            .unwrap();

        assert!(!metadata.is_expired());
    }
}
