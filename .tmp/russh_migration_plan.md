# Russh Migration Plan: ssh2 to russh

**Date:** 2025-12-22
**Author:** Hive 1 - Research & Architecture
**Status:** Ready for Implementation

---

## Executive Summary

This document outlines the comprehensive migration plan for Rustible's SSH connection layer from the synchronous `ssh2` crate to the async-native `russh` crate. The migration will provide significant performance improvements through native async I/O, eliminate C dependencies (libssh2), and better integrate with Rustible's Tokio-based async architecture.

### Key Benefits
- **Native async**: No more `spawn_blocking` wrappers around synchronous calls
- **Pure Rust**: Zero C dependencies, better cross-compilation and security auditing
- **Performance**: Direct Tokio integration eliminates thread context switching overhead
- **Modern crypto**: Ed25519 and ChaCha20-Poly1305 by default

---

## Table of Contents

1. [Current Implementation Analysis](#1-current-implementation-analysis)
2. [Key API Differences](#2-key-api-differences)
3. [Connection Trait Implementation](#3-connection-trait-implementation)
4. [Authentication Strategies](#4-authentication-strategies)
5. [Command Execution](#5-command-execution)
6. [SFTP Operations Mapping](#6-sftp-operations-mapping)
7. [Connection Pooling](#7-connection-pooling)
8. [Breaking Changes](#8-breaking-changes)
9. [Migration Steps](#9-migration-steps)
10. [Testing Strategy](#10-testing-strategy)
11. [Rollout Plan](#11-rollout-plan)

---

## 1. Current Implementation Analysis

### ssh2 Implementation (ssh.rs)

The current `SshConnection` implementation uses the `ssh2` crate with:

```rust
pub struct SshConnection {
    identifier: String,
    session: Arc<Mutex<Session>>,      // ssh2::Session wrapped for thread safety
    host_config: HostConfig,
    connected: Arc<Mutex<bool>>,
}
```

**Key Characteristics:**
- **Synchronous core**: All ssh2 calls are blocking
- **Thread pool usage**: Uses `tokio::task::spawn_blocking` for async compatibility
- **Mutex-based sharing**: `parking_lot::Mutex` wraps the session for thread safety
- **SFTP via ssh2**: Uses `ssh2::Sftp` for file operations

**Current Pain Points:**
1. Every SSH operation spawns a blocking task (thread pool overhead)
2. Cannot have truly concurrent channel operations on same session
3. Mutex contention under high load
4. Complex error handling through spawn_blocking boundaries

### Existing Russh Skeleton (russh.rs)

A partial implementation exists with:

```rust
pub struct RusshConnection {
    identifier: String,
    handle: Arc<Mutex<Option<Handle<ClientHandler>>>>,
    host_config: HostConfig,
    connected: Arc<Mutex<bool>>,
}
```

**Status of Existing Implementation:**

| Method | Status | Notes |
|--------|--------|-------|
| `connect()` | Implemented | Connection and authentication working |
| `execute()` | Implemented | Full channel/command execution |
| `upload()` | Implemented | SFTP file upload working |
| `upload_content()` | Implemented | SFTP content upload working |
| `download()` | Implemented | SFTP file download working |
| `download_content()` | Implemented | SFTP content download working |
| `path_exists()` | **NOT IMPLEMENTED** | Returns error |
| `is_directory()` | **NOT IMPLEMENTED** | Returns error |
| `stat()` | **NOT IMPLEMENTED** | Returns error |
| `is_alive()` | Partial | Only checks `connected` flag |
| `close()` | Partial | Only sets flag, no graceful disconnect |

**Known Issues in Current Skeleton:**
1. SSH agent authentication not implemented (hardcoded failure)
2. Mutex on handle prevents concurrent channel operations
3. SFTP session opened fresh for each operation (inefficient)
4. No proper connection health checking
5. Module is disabled due to API incompatibilities

---

## 2. Key API Differences

### Connection Establishment

| Aspect | ssh2 | russh |
|--------|------|-------|
| Connect | `TcpStream::connect()` + `Session::new()` | `russh::client::connect()` |
| Handshake | `session.handshake()` | Automatic with connect |
| Handler | Not required | Must implement `client::Handler` trait |
| Config | Set options on Session | Pass `Arc<Config>` to connect |

**ssh2 Pattern:**
```rust
let tcp = TcpStream::connect(addr)?;
let mut session = Session::new()?;
session.set_tcp_stream(tcp);
session.handshake()?;
```

**russh Pattern:**
```rust
struct ClientHandler;

impl client::Handler for ClientHandler {
    type Error = ConnectionError;

    async fn check_server_key(&mut self, key: &PublicKey) -> Result<bool, Self::Error> {
        Ok(true) // TODO: Verify against known_hosts
    }
}

let config = Arc::new(client::Config::default());
let handle = russh::client::connect(config, (host, port), ClientHandler).await?;
```

### Session/Handle Type

| ssh2 | russh |
|------|-------|
| `Session` - owns TCP connection | `Handle<H>` - async handle to session |
| Synchronous methods | Async methods |
| Single-threaded usage | Can be cloned for concurrent use |

### Channel Operations

| Operation | ssh2 | russh |
|-----------|------|-------|
| Open session | `session.channel_session()` | `handle.channel_open_session().await` |
| Execute | `channel.exec(cmd)` | `channel.exec(true, cmd).await` |
| Read stdout | `channel.read_to_string()` | Loop on `channel.wait().await` |
| Read stderr | `channel.stderr().read_to_string()` | `ChannelMsg::ExtendedData` with ext=1 |
| Exit status | `channel.exit_status()` | `ChannelMsg::ExitStatus` |
| Close | `channel.wait_close()` | `channel.eof().await` + wait for Close msg |

### SFTP

| Operation | ssh2 (Sftp) | russh-sftp (SftpSession) |
|-----------|-------------|--------------------------|
| Open session | `session.sftp()` | `SftpSession::new(channel.into_stream())` |
| Create file | `sftp.create(path)` | `sftp.create(path).await` |
| Open file | `sftp.open(path)` | `sftp.open(path).await` |
| Read | `file.read_to_end()` | `file.read_to_end().await` (AsyncReadExt) |
| Write | `file.write_all()` | `file.write_all().await` (AsyncWriteExt) |
| Stat | `sftp.stat(path)` | `sftp.metadata(path).await` |
| Set metadata | Via chmod command | `sftp.set_metadata(path, attrs).await` |
| Mkdir | `sftp.mkdir(path, mode)` | `sftp.create_dir(path).await` |

---

## 3. Connection Trait Implementation

The `Connection` trait requires these methods to be implemented:

```rust
#[async_trait]
pub trait Connection: Send + Sync {
    fn identifier(&self) -> &str;
    async fn is_alive(&self) -> bool;
    async fn execute(&self, command: &str, options: Option<ExecuteOptions>) -> ConnectionResult<CommandResult>;
    async fn upload(&self, local_path: &Path, remote_path: &Path, options: Option<TransferOptions>) -> ConnectionResult<()>;
    async fn upload_content(&self, content: &[u8], remote_path: &Path, options: Option<TransferOptions>) -> ConnectionResult<()>;
    async fn download(&self, remote_path: &Path, local_path: &Path) -> ConnectionResult<()>;
    async fn download_content(&self, remote_path: &Path) -> ConnectionResult<Vec<u8>>;
    async fn path_exists(&self, path: &Path) -> ConnectionResult<bool>;
    async fn is_directory(&self, path: &Path) -> ConnectionResult<bool>;
    async fn stat(&self, path: &Path) -> ConnectionResult<FileStat>;
    async fn close(&self) -> ConnectionResult<()>;
}
```

### Implementation Matrix

| Method | Russh Implementation | SFTP Required | Notes |
|--------|---------------------|---------------|-------|
| `identifier()` | Return stored string | No | Direct return |
| `is_alive()` | Send keepalive or check channel | No | Need to implement properly |
| `execute()` | Open channel, exec, read output | No | Working in skeleton |
| `upload()` | SFTP create + write | Yes | Working in skeleton |
| `upload_content()` | SFTP create + write | Yes | Working in skeleton |
| `download()` | SFTP open + read | Yes | Working in skeleton |
| `download_content()` | SFTP open + read | Yes | Working in skeleton |
| `path_exists()` | SFTP stat, check error | Yes | **Needs implementation** |
| `is_directory()` | SFTP stat, check file_type | Yes | **Needs implementation** |
| `stat()` | SFTP metadata | Yes | **Needs implementation** |
| `close()` | Disconnect session | No | **Needs proper implementation** |

### Recommended Struct Design

```rust
pub struct RusshConnection {
    /// Session identifier
    identifier: String,

    /// Russh client handle - cloneable for concurrent operations
    handle: Handle<ClientHandler>,

    /// Host configuration
    host_config: HostConfig,

    /// Connection state - use AtomicBool for lock-free checking
    connected: Arc<AtomicBool>,

    /// Cached SFTP session (optional optimization)
    sftp_session: Arc<tokio::sync::RwLock<Option<SftpSession>>>,
}
```

**Key Change:** Remove `Mutex` wrapper around `Handle` - russh handles are already `Clone` and can be used concurrently.

---

## 4. Authentication Strategies

### Authentication Priority Order

1. **SSH Agent** (if `use_agent` is true)
2. **Specific identity file** (from `host_config.identity_file`)
3. **Global identity files** (from `global_config.defaults.identity_files`)
4. **Default identity files** (~/.ssh/id_ed25519, id_ecdsa, id_rsa, id_dsa)
5. **Password** (if provided in `host_config.password`)

### SSH Agent Authentication

**Current Status:** Not implemented in skeleton (returns hardcoded error)

**Implementation Approach:**

```rust
use russh_keys::agent::client::AgentClient;

async fn try_agent_auth(
    session: &mut Handle<ClientHandler>,
    user: &str,
) -> ConnectionResult<()> {
    // Connect to SSH agent via SSH_AUTH_SOCK
    let agent_path = std::env::var("SSH_AUTH_SOCK")
        .map_err(|_| ConnectionError::AuthenticationFailed("SSH_AUTH_SOCK not set".into()))?;

    let stream = tokio::net::UnixStream::connect(&agent_path).await
        .map_err(|e| ConnectionError::AuthenticationFailed(format!("Agent connect: {}", e)))?;

    let mut agent = AgentClient::connect(stream).await
        .map_err(|e| ConnectionError::AuthenticationFailed(format!("Agent client: {}", e)))?;

    // Get identities from agent
    let identities = agent.request_identities().await
        .map_err(|e| ConnectionError::AuthenticationFailed(format!("Agent identities: {}", e)))?;

    // Try each identity
    for identity in identities {
        if session.authenticate_publickey(user, identity.key()).await.unwrap_or(false) {
            return Ok(());
        }
    }

    Err(ConnectionError::AuthenticationFailed("No suitable agent identity".into()))
}
```

**Required Dependency:**
```toml
russh-keys = { version = "0.45", features = ["agent"] }
```

### Public Key Authentication

**Current Status:** Implemented in skeleton

```rust
async fn try_key_auth(
    session: &mut Handle<ClientHandler>,
    user: &str,
    key_path: &Path,
    passphrase: Option<&str>,
) -> ConnectionResult<()> {
    let key_pair = russh_keys::load_secret_key(key_path, passphrase)
        .map_err(|e| ConnectionError::AuthenticationFailed(format!("Load key: {}", e)))?;

    let authenticated = session
        .authenticate_publickey(user, Arc::new(key_pair))
        .await
        .map_err(|e| ConnectionError::AuthenticationFailed(format!("Auth: {}", e)))?;

    if authenticated {
        Ok(())
    } else {
        Err(ConnectionError::AuthenticationFailed("Key rejected".into()))
    }
}
```

### Password Authentication

**Current Status:** Implemented in skeleton

```rust
async fn try_password_auth(
    session: &mut Handle<ClientHandler>,
    user: &str,
    password: &str,
) -> ConnectionResult<()> {
    let authenticated = session
        .authenticate_password(user, password)
        .await
        .map_err(|e| ConnectionError::AuthenticationFailed(format!("Password auth: {}", e)))?;

    if authenticated {
        Ok(())
    } else {
        Err(ConnectionError::AuthenticationFailed("Password rejected".into()))
    }
}
```

### Host Key Verification

**Current Status:** Blindly accepts all keys (insecure)

**Recommended Implementation:**

```rust
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

struct ClientHandler {
    known_hosts: HashSet<(String, russh_keys::key::PublicKey)>,
    strict_checking: bool,
    known_hosts_file: Option<PathBuf>,
}

impl ClientHandler {
    fn load_known_hosts(path: &Path) -> HashSet<(String, russh_keys::key::PublicKey)> {
        // Parse OpenSSH known_hosts format
        // Each line: hostname[,hostname]* keytype base64-key [comment]
        let mut hosts = HashSet::new();
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                // Parse and add to hosts set
            }
        }
        hosts
    }
}

#[async_trait]
impl client::Handler for ClientHandler {
    type Error = ConnectionError;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        if !self.strict_checking {
            return Ok(true);
        }

        // Check against known_hosts
        // If not found and strict, return Err or prompt user
        // If found but different key, return Err (MITM warning)

        Ok(true) // Placeholder
    }
}
```

---

## 5. Command Execution

### Current Pattern (Working)

```rust
async fn execute(&self, command: &str, options: Option<ExecuteOptions>) -> ConnectionResult<CommandResult> {
    let options = options.unwrap_or_default();
    let handle = self.get_handle()?;
    let full_command = Self::build_command(command, &options);

    let execute_future = async {
        // 1. Open channel
        let mut channel = handle.channel_open_session().await?;

        // 2. Set environment variables
        for (key, value) in &options.env {
            let _ = channel.request_env(true, key, value).await;
        }

        // 3. Execute command
        channel.exec(true, full_command).await?;

        // 4. Handle sudo password if needed
        if options.escalate && options.escalate_password.is_some() {
            channel.data(format!("{}\n", password).as_bytes()).await?;
        }

        // 5. Collect output
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = None;

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
                ChannelMsg::ExtendedData { ref data, ext: 1 } => stderr.extend_from_slice(data),
                ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                ChannelMsg::Close => break,
                _ => {}
            }
        }

        // 6. Return result
        let exit_code = exit_code.unwrap_or(-1);
        if exit_code == 0 {
            Ok(CommandResult::success(stdout_str, stderr_str))
        } else {
            Ok(CommandResult::failure(exit_code, stdout_str, stderr_str))
        }
    };

    // Apply timeout if specified
    if let Some(timeout_secs) = options.timeout {
        tokio::time::timeout(Duration::from_secs(timeout_secs), execute_future).await
    } else {
        execute_future.await
    }
}
```

### Improvements Needed

1. **Better error handling**: Map russh errors to ConnectionError properly
2. **Exit code edge cases**: Handle missing exit status gracefully
3. **Large output streaming**: Consider memory limits for stdout/stderr
4. **PTY support**: For interactive commands (future enhancement)

---

## 6. SFTP Operations Mapping

### Opening SFTP Session

```rust
async fn open_sftp(handle: &Handle<ClientHandler>) -> ConnectionResult<SftpSession> {
    let channel = handle.channel_open_session().await
        .map_err(|e| ConnectionError::TransferFailed(format!("Open channel: {}", e)))?;

    channel.request_subsystem(true, "sftp").await
        .map_err(|e| ConnectionError::TransferFailed(format!("Request SFTP: {}", e)))?;

    SftpSession::new(channel.into_stream()).await
        .map_err(|e| ConnectionError::TransferFailed(format!("SFTP session: {}", e)))
}
```

### File Stat Implementation (NEEDS IMPLEMENTATION)

```rust
async fn stat(&self, path: &Path) -> ConnectionResult<FileStat> {
    let handle = self.get_handle()?;
    let sftp = Self::open_sftp(&handle).await?;

    let metadata = sftp.metadata(path).await
        .map_err(|e| ConnectionError::TransferFailed(format!("Stat {}: {}", path.display(), e)))?;

    Ok(FileStat {
        size: metadata.size().unwrap_or(0),
        mode: metadata.permissions().unwrap_or(0),
        uid: metadata.uid().unwrap_or(0),
        gid: metadata.gid().unwrap_or(0),
        atime: metadata.atime().map(|t| t as i64).unwrap_or(0),
        mtime: metadata.mtime().map(|t| t as i64).unwrap_or(0),
        is_dir: metadata.is_dir(),
        is_file: metadata.is_file(),
        is_symlink: metadata.file_type().map(|t| t.is_symlink()).unwrap_or(false),
    })
}
```

### Path Exists Implementation (NEEDS IMPLEMENTATION)

```rust
async fn path_exists(&self, path: &Path) -> ConnectionResult<bool> {
    let handle = self.get_handle()?;
    let sftp = Self::open_sftp(&handle).await?;

    match sftp.metadata(path).await {
        Ok(_) => Ok(true),
        Err(e) if is_not_found_error(&e) => Ok(false),
        Err(e) => Err(ConnectionError::TransferFailed(format!("Check path: {}", e))),
    }
}

fn is_not_found_error(e: &russh_sftp::Error) -> bool {
    // Check if error indicates file not found (SSH_FX_NO_SUCH_FILE)
    matches!(e, russh_sftp::Error::Status(status) if status.code == StatusCode::NoSuchFile)
}
```

### Is Directory Implementation (NEEDS IMPLEMENTATION)

```rust
async fn is_directory(&self, path: &Path) -> ConnectionResult<bool> {
    let handle = self.get_handle()?;
    let sftp = Self::open_sftp(&handle).await?;

    match sftp.metadata(path).await {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(e) if is_not_found_error(&e) => Ok(false),
        Err(e) => Err(ConnectionError::TransferFailed(format!("Check dir: {}", e))),
    }
}
```

### Setting Permissions and Ownership

russh-sftp supports setting metadata via `set_metadata()`:

```rust
async fn set_file_attrs(
    sftp: &SftpSession,
    path: &Path,
    mode: Option<u32>,
    owner: Option<&str>,
    group: Option<&str>,
) -> ConnectionResult<()> {
    let mut attrs = russh_sftp::protocol::FileAttributes::default();

    if let Some(mode) = mode {
        attrs.set_permissions(mode);
    }

    // Note: Setting uid/gid requires numeric IDs
    // For owner/group names, need to resolve via getpwnam/getgrnam
    // This typically requires executing a command on the remote

    if mode.is_some() {
        sftp.set_metadata(path.to_path_buf(), attrs).await
            .map_err(|e| ConnectionError::TransferFailed(format!("Set attrs: {}", e)))?;
    }

    // For owner/group by name, fall back to chown command
    if owner.is_some() || group.is_some() {
        // Use execute() to run chown
    }

    Ok(())
}
```

---

## 7. Connection Pooling

### Current Pool Design

```rust
pub struct ConnectionPool {
    max_connections: usize,
    connections: HashMap<String, Arc<dyn Connection + Send + Sync>>,
}
```

### Russh-Specific Considerations

1. **Handle Cloning**: russh `Handle<H>` is `Clone` - can share one handle for multiple operations
2. **Channel Multiplexing**: Multiple channels can exist on one SSH connection
3. **Connection Reuse**: SSH connections are expensive to establish; reuse aggressively
4. **Health Checking**: Need proper keepalive/ping mechanism

### Recommended Pool Enhancements

```rust
pub struct RusshConnectionPool {
    /// Maximum connections per host
    max_per_host: usize,

    /// Connection cache: pool_key -> (Handle, last_used, channel_count)
    connections: Arc<RwLock<HashMap<String, PooledConnection>>>,

    /// Keepalive interval
    keepalive_interval: Duration,

    /// Idle timeout
    idle_timeout: Duration,
}

struct PooledConnection {
    handle: Handle<ClientHandler>,
    last_used: Instant,
    active_channels: AtomicUsize,
    host_config: HostConfig,
}

impl RusshConnectionPool {
    /// Get or create connection
    async fn get(&self, pool_key: &str) -> ConnectionResult<Handle<ClientHandler>> {
        // Check for existing healthy connection
        // If found and alive, increment channel count and return clone
        // If not found or dead, create new connection
    }

    /// Return connection to pool
    async fn release(&self, pool_key: &str) {
        // Decrement channel count
        // Update last_used timestamp
    }

    /// Background task: cleanup idle connections
    async fn cleanup_task(&self) {
        loop {
            tokio::time::sleep(self.keepalive_interval).await;
            // Check all connections
            // Send keepalive to active ones
            // Close idle ones past timeout
        }
    }
}
```

### Connection Health Check

```rust
async fn is_alive(&self) -> bool {
    if !self.connected.load(Ordering::Relaxed) {
        return false;
    }

    // Try to send a channel request that will be rejected but proves connectivity
    match self.handle.channel_open_session().await {
        Ok(channel) => {
            // Successfully opened channel, connection is alive
            // Close it immediately since we don't need it
            let _ = channel.eof().await;
            true
        }
        Err(_) => {
            // Failed to open channel, connection is dead
            self.connected.store(false, Ordering::Relaxed);
            false
        }
    }
}
```

---

## 8. Breaking Changes

### API Changes

| Area | Change | Impact |
|------|--------|--------|
| Feature flag | `russh` feature required | Cargo.toml update |
| Crypto backend | Must enable `aws-lc-rs` or `ring` | Cargo.toml update |
| Error types | New error variants from russh | Error handling updates |
| Handler trait | Must implement `client::Handler` | New struct required |

### Configuration Changes

| Setting | ssh2 | russh | Notes |
|---------|------|-------|-------|
| Compression | `session.set_compress(true)` | Via `Config` | Different API |
| Timeout | `session.set_timeout(ms)` | `Config::connection_timeout` | Duration vs ms |
| Keepalive | `session.keepalive_send()` | Channel probe | Different approach |

### Behavioral Changes

1. **Error messages**: Different error text from russh vs ssh2
2. **Timeout handling**: Native async timeout vs thread-based
3. **Channel closing**: Explicit EOF + wait for Close message
4. **Exit codes**: May arrive before or after EOF

### Dependency Changes

```toml
# Remove
ssh2 = "0.9"

# Add (update versions as needed)
russh = { version = "0.45", features = ["aws-lc-rs"] }
russh-keys = { version = "0.45" }
russh-sftp = { version = "2.0" }
```

---

## 9. Migration Steps

### Phase 1: Complete Skeleton Implementation (Priority: HIGH)

1. **Implement missing methods**
   - [ ] `path_exists()` - Use SFTP metadata
   - [ ] `is_directory()` - Use SFTP metadata
   - [ ] `stat()` - Use SFTP metadata
   - [ ] `is_alive()` - Proper health check
   - [ ] `close()` - Graceful disconnect

2. **Fix SSH agent authentication**
   - [ ] Add russh-keys agent feature
   - [ ] Implement Unix socket connection
   - [ ] Query agent identities
   - [ ] Try each identity

3. **Remove Mutex on Handle**
   - [ ] Change `Arc<Mutex<Option<Handle>>>` to `Handle`
   - [ ] Handle is already Clone and thread-safe

### Phase 2: SFTP Session Caching (Priority: MEDIUM)

1. **Add cached SFTP session**
   - [ ] Store in `RwLock<Option<SftpSession>>`
   - [ ] Lazy initialization on first SFTP operation
   - [ ] Invalidate on connection close

2. **Optimize file operations**
   - [ ] Reuse SFTP session across operations
   - [ ] Batch operations where possible

### Phase 3: Connection Pooling Enhancement (Priority: MEDIUM)

1. **Enhance pool for russh**
   - [ ] Track active channel count
   - [ ] Implement proper keepalive
   - [ ] Add idle connection cleanup

2. **Health monitoring**
   - [ ] Background health check task
   - [ ] Automatic reconnection on failure

### Phase 4: Host Key Verification (Priority: HIGH for security)

1. **Implement known_hosts parsing**
   - [ ] Parse OpenSSH known_hosts format
   - [ ] Support hashed hostnames
   - [ ] Support multiple key types

2. **Implement verification logic**
   - [ ] Check against known_hosts
   - [ ] Handle missing entries (prompt or fail)
   - [ ] Handle changed keys (security warning)

### Phase 5: Integration and Testing (Priority: HIGH)

1. **Enable russh module**
   - [ ] Uncomment module in mod.rs
   - [ ] Update feature flags
   - [ ] Fix any compilation errors

2. **Add connection factory support**
   - [ ] Add `RusshConnection` to factory
   - [ ] Feature-gate selection
   - [ ] Configuration option to choose backend

3. **Integration tests**
   - [ ] Test with real SSH server
   - [ ] Test with Docker container
   - [ ] Test authentication methods
   - [ ] Test file operations

---

## 10. Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_command_basic() {
        // Already implemented
    }

    #[test]
    fn test_build_command_with_escalation() {
        // Already implemented
    }

    #[test]
    fn test_builder_pattern() {
        // Already implemented
    }
}
```

### Integration Tests

```rust
#[tokio::test]
#[ignore] // Requires SSH server
async fn test_russh_connect_and_execute() {
    let conn = RusshConnectionBuilder::new("localhost")
        .port(22)
        .user("testuser")
        .private_key("/path/to/key")
        .connect()
        .await
        .unwrap();

    let result = conn.execute("whoami", None).await.unwrap();
    assert!(result.success);
    assert_eq!(result.stdout.trim(), "testuser");
}

#[tokio::test]
#[ignore]
async fn test_russh_sftp_operations() {
    let conn = /* ... */;

    // Upload
    conn.upload_content(b"test content", Path::new("/tmp/test.txt"), None).await.unwrap();

    // Check exists
    assert!(conn.path_exists(Path::new("/tmp/test.txt")).await.unwrap());

    // Download
    let content = conn.download_content(Path::new("/tmp/test.txt")).await.unwrap();
    assert_eq!(&content, b"test content");

    // Stat
    let stat = conn.stat(Path::new("/tmp/test.txt")).await.unwrap();
    assert!(stat.is_file);
    assert!(!stat.is_dir);
}
```

### Performance Tests

```rust
#[tokio::test]
#[ignore]
async fn benchmark_russh_vs_ssh2() {
    // Compare execution time for:
    // - Connection establishment
    // - Multiple command executions
    // - File transfers
    // - Concurrent operations
}
```

---

## 11. Rollout Plan

### Stage 1: Feature-Gated Implementation
- Complete russh implementation behind `russh` feature
- Keep ssh2 as default
- No breaking changes to existing code

### Stage 2: Parallel Operation
- Both backends available
- Configuration option to select backend
- Run integration tests with both

### Stage 3: Default Switch
- Make russh the default
- Keep ssh2 available via feature flag
- Update documentation

### Stage 4: Deprecation
- Mark ssh2 backend as deprecated
- Add deprecation warnings
- Plan removal timeline

### Stage 5: Removal
- Remove ssh2 dependency
- Remove ssh2 implementation
- Update documentation

---

## Appendix A: Error Mapping

| russh Error | ConnectionError |
|-------------|-----------------|
| `russh::Error::Disconnect` | `ConnectionClosed` |
| `russh::Error::Io` | `ConnectionFailed` |
| `russh::Error::Keys` | `AuthenticationFailed` |
| `russh::Error::NotAuthenticated` | `AuthenticationFailed` |
| `russh_sftp::Error::Status(NoSuchFile)` | Return `false` for path_exists |
| `russh_sftp::Error::Status(PermissionDenied)` | `TransferFailed` |
| `russh_sftp::Error::*` | `TransferFailed` |

---

## Appendix B: Configuration Mapping

### russh Client Config

```rust
let config = russh::client::Config {
    // Connection timeout
    connection_timeout: Some(Duration::from_secs(30)),

    // Keepalive interval (0 = disabled)
    keepalive_interval: Some(Duration::from_secs(15)),

    // Keepalive count max
    keepalive_max: 3,

    // Preferred algorithms (optional customization)
    preferred: PreferredAlgorithms::default(),

    // Compression (not directly supported, use algorithm preference)
    ..Default::default()
};
```

### HostConfig to russh Config Mapping

| HostConfig | russh Config | Notes |
|------------|--------------|-------|
| `connect_timeout` | `connection_timeout` | Direct mapping |
| `server_alive_interval` | `keepalive_interval` | Direct mapping |
| `server_alive_count_max` | `keepalive_max` | Direct mapping |
| `compression` | Algorithm preference | Indirect |
| `identity_file` | Used in auth | Not in Config |
| `proxy_jump` | Requires russh-config | Future enhancement |

---

## Summary

The migration from ssh2 to russh is well underway with a solid skeleton in place. The remaining work focuses on:

1. **Completing missing trait methods** (stat, path_exists, is_directory, close)
2. **Implementing SSH agent support**
3. **Adding proper host key verification**
4. **Optimizing SFTP session reuse**
5. **Enhancing connection pooling**
6. **Comprehensive testing**

The async-native nature of russh will provide significant performance benefits once the migration is complete, especially for parallel execution scenarios common in configuration management tools.

---

**Document Version:** 1.0
**Last Updated:** 2025-12-22
**Next Review:** After Phase 1 completion
