//! Russh connection module
//!
//! This module provides SSH connectivity using the russh crate.
//! Russh is a modern, async-native SSH library that provides better
//! performance and integration with Tokio compared to ssh2.

use async_trait::async_trait;
use russh::client::{Handle, Handler};
use russh::keys::key::PublicKey;
use russh::keys::load_secret_key;
use russh::ChannelMsg;
use russh_sftp::client::SftpSession;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tracing::{debug, trace, warn};

/// Threshold for using streaming uploads (1MB)
const STREAM_THRESHOLD: u64 = 1024 * 1024;

/// Chunk size for streaming transfers (64KB)
const CHUNK_SIZE: usize = 64 * 1024;

use super::config::{
    default_identity_files, expand_path, ConnectionConfig, HostConfig, RetryConfig,
};
use super::{
    CommandResult, Connection, ConnectionError, ConnectionResult, ExecuteOptions, FileStat,
    RusshError, TransferOptions,
};

/// Escape a path for safe use in shell commands
///
/// Uses single quotes and escapes any single quotes within the string.
/// This is the safest way to pass arbitrary paths to shell commands.
fn escape_shell_arg(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Client handler for russh
struct ClientHandler;

#[async_trait]
impl Handler for ClientHandler {
    type Error = RusshError;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        // TODO: Implement proper host key verification
        // For now, accept all keys (similar to StrictHostKeyChecking=no)
        Ok(true)
    }
}

/// Russh connection implementation using russh crate
///
/// This implementation uses RwLock instead of Mutex for the handle to reduce
/// lock contention during parallel operations. Most operations only need read
/// access to get a reference to the Handle for opening channels - only close()
/// needs write access to take ownership of the handle.
pub struct RusshConnection {
    /// Session identifier
    identifier: String,
    /// Russh client handle - uses RwLock for better parallel performance
    /// Read lock: channel operations (execute, upload, download, etc.)
    /// Write lock: close operation only
    handle: Arc<RwLock<Option<Handle<ClientHandler>>>>,
    /// Host configuration
    host_config: HostConfig,
    /// Whether the connection is established
    connected: Arc<AtomicBool>,
}

impl RusshConnection {
    /// Build command string with options (no environment variables)
    fn build_command(command: &str, options: &ExecuteOptions) -> String {
        let mut parts = Vec::new();

        // Add working directory
        if let Some(cwd) = &options.cwd {
            parts.push(format!("cd {} && ", cwd));
        }

        // Handle privilege escalation
        if options.escalate {
            let escalate_method = options.escalate_method.as_deref().unwrap_or("sudo");
            let escalate_user = options.escalate_user.as_deref().unwrap_or("root");

            match escalate_method {
                "sudo" => {
                    if options.escalate_password.is_some() {
                        parts.push(format!("sudo -S -u {} -- ", escalate_user));
                    } else {
                        parts.push(format!("sudo -u {} -- ", escalate_user));
                    }
                }
                "su" => {
                    parts.push(format!("su - {} -c ", escalate_user));
                }
                "doas" => {
                    parts.push(format!("doas -u {} ", escalate_user));
                }
                _ => {
                    parts.push(format!("sudo -u {} -- ", escalate_user));
                }
            }
        }

        parts.push(command.to_string());
        parts.concat()
    }

    /// Build command string with options, including environment variables
    ///
    /// Since russh doesn't support the SSH request_env protocol, we prepend
    /// environment variable exports to the command.
    fn build_command_with_env(command: &str, options: &ExecuteOptions) -> String {
        let mut parts = Vec::new();

        // Prepend environment variables as exports
        if !options.env.is_empty() {
            for (key, value) in &options.env {
                // Use export to set environment variables
                // Escape the value to handle special characters
                let escaped_value = value.replace('\'', "'\\''");
                parts.push(format!("export {}='{}'; ", key, escaped_value));
            }
        }

        // Add the rest of the command using the base build_command
        parts.push(Self::build_command(command, options));
        parts.concat()
    }

    /// Open an SFTP session
    async fn open_sftp(handle: &Handle<ClientHandler>) -> ConnectionResult<SftpSession> {
        let channel = handle.channel_open_session().await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to open channel: {}", e))
        })?;

        channel.request_subsystem(true, "sftp").await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to request SFTP subsystem: {}", e))
        })?;

        SftpSession::new(channel.into_stream()).await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to create SFTP session: {}", e))
        })
    }

    /// Create remote directories recursively via SFTP
    async fn create_remote_dirs_sftp(sftp: &SftpSession, path: &Path) -> ConnectionResult<()> {
        let mut current = PathBuf::new();

        for component in path.components() {
            current.push(component);

            // Skip root
            if current.to_string_lossy() == "/" {
                continue;
            }

            // Try to create directory (ignore error if it already exists)
            let _ = sftp.create_dir(current.to_string_lossy().to_string()).await;
        }

        Ok(())
    }
}

impl RusshConnection {
    /// Connect to a remote host via SSH using russh
    pub async fn connect(
        host: &str,
        port: u16,
        user: &str,
        host_config: Option<HostConfig>,
        global_config: &ConnectionConfig,
    ) -> ConnectionResult<Self> {
        let host_config = host_config.unwrap_or_else(|| global_config.get_host_merged(host));
        let retry_config = host_config.retry_config();

        let actual_host = host_config.hostname.as_deref().unwrap_or(host);
        let actual_port = host_config.port.unwrap_or(port);
        let actual_user = host_config.user.as_deref().unwrap_or(user);
        let timeout = host_config.timeout_duration();

        debug!(
            host = %actual_host,
            port = %actual_port,
            user = %actual_user,
            "Connecting via SSH (russh)"
        );

        let identifier = format!("{}@{}:{}", actual_user, actual_host, actual_port);

        // Connect with retry logic
        let handle = Self::connect_with_retry(
            actual_host,
            actual_port,
            actual_user,
            &host_config,
            global_config,
            timeout,
            &retry_config,
        )
        .await?;

        Ok(Self {
            identifier,
            handle: Arc::new(RwLock::new(Some(handle))),
            host_config,
            connected: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Connect with retry logic
    async fn connect_with_retry(
        host: &str,
        port: u16,
        user: &str,
        host_config: &HostConfig,
        global_config: &ConnectionConfig,
        timeout: Duration,
        retry_config: &RetryConfig,
    ) -> ConnectionResult<Handle<ClientHandler>> {
        let mut last_error = None;

        for attempt in 0..=retry_config.max_retries {
            if attempt > 0 {
                let delay = retry_config.delay_for_attempt(attempt - 1);
                debug!(attempt = %attempt, delay = ?delay, "Retrying SSH connection");
                tokio::time::sleep(delay).await;
            }

            match Self::do_connect(host, port, user, host_config, global_config, timeout).await {
                Ok(handle) => return Ok(handle),
                Err(e) => {
                    warn!(attempt = %attempt, error = %e, "SSH connection attempt failed");
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ConnectionError::ConnectionFailed("Unknown connection error".to_string())
        }))
    }

    /// Perform the actual connection
    async fn do_connect(
        host: &str,
        port: u16,
        user: &str,
        host_config: &HostConfig,
        global_config: &ConnectionConfig,
        timeout: Duration,
    ) -> ConnectionResult<Handle<ClientHandler>> {
        // Create optimized russh client configuration
        let mut config = russh::client::Config::default();
        config.inactivity_timeout = Some(timeout);
        // Optimize preferred algorithms for faster negotiation
        // Modern servers typically support these fast algorithms
        config.preferred = russh::Preferred {
            // Prefer fast key exchange algorithms
            kex: std::borrow::Cow::Borrowed(&[
                russh::kex::CURVE25519,
                russh::kex::CURVE25519_PRE_RFC_8731,
            ]),
            // Prefer fast ciphers (only use AES-256-GCM as AES-128-GCM isn't available)
            cipher: std::borrow::Cow::Borrowed(&[
                russh::cipher::CHACHA20_POLY1305,
                russh::cipher::AES_256_GCM,
            ]),
            // Prefer fast key types
            key: std::borrow::Cow::Borrowed(&[
                russh::keys::key::ED25519,
                russh::keys::key::RSA_SHA2_256,
                russh::keys::key::RSA_SHA2_512,
            ]),
            // Prefer fast MACs (not used with AEAD ciphers but needed for fallback)
            mac: std::borrow::Cow::Borrowed(&[
                russh::mac::HMAC_SHA256,
                russh::mac::HMAC_SHA512,
            ]),
            // No compression for speed
            compression: std::borrow::Cow::Borrowed(&[
                russh::compression::NONE,
            ]),
        };
        let config = Arc::new(config);

        // Connect to the SSH server
        let addr = format!("{}:{}", host, port);
        let socket = tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr))
            .await
            .map_err(|_| ConnectionError::Timeout(timeout.as_secs()))?
            .map_err(|e| {
                ConnectionError::ConnectionFailed(format!("Failed to connect to {}: {}", addr, e))
            })?;

        // Enable TCP_NODELAY for lower latency
        socket.set_nodelay(true).map_err(|e| {
            ConnectionError::ConnectionFailed(format!("Failed to set TCP_NODELAY: {}", e))
        })?;

        let mut session = russh::client::connect_stream(config, socket, ClientHandler)
            .await
            .map_err(|e| {
                ConnectionError::ConnectionFailed(format!("SSH handshake failed: {}", e))
            })?;

        // Authenticate
        Self::authenticate(&mut session, user, host_config, global_config).await?;

        debug!("SSH connection established successfully");
        Ok(session)
    }

    /// Perform SSH authentication
    async fn authenticate(
        session: &mut Handle<ClientHandler>,
        user: &str,
        host_config: &HostConfig,
        global_config: &ConnectionConfig,
    ) -> ConnectionResult<()> {
        // Try SSH agent first if enabled
        if global_config.defaults.use_agent {
            if Self::try_agent_auth(session, user).await.is_ok() {
                debug!("Authenticated using SSH agent");
                return Ok(());
            }
        }

        // Try key-based authentication
        // 1. Try specific identity file if configured
        if let Some(identity_file) = &host_config.identity_file {
            let key_path = expand_path(identity_file);
            if Self::try_key_auth(session, user, &key_path, host_config.password.as_deref())
                .await
                .is_ok()
            {
                debug!(key = %key_path.display(), "Authenticated using key");
                return Ok(());
            }
        }

        // 2. Try default identity files from global config
        for identity_file in &global_config.defaults.identity_files {
            let key_path = expand_path(identity_file);
            if Self::try_key_auth(session, user, &key_path, host_config.password.as_deref())
                .await
                .is_ok()
            {
                debug!(key = %key_path.display(), "Authenticated using key");
                return Ok(());
            }
        }

        // 3. Try default identity files from ~/.ssh/
        for key_path in default_identity_files() {
            if Self::try_key_auth(session, user, &key_path, host_config.password.as_deref())
                .await
                .is_ok()
            {
                debug!(key = %key_path.display(), "Authenticated using key");
                return Ok(());
            }
        }

        // Try password authentication
        if let Some(password) = &host_config.password {
            let authenticated = session
                .authenticate_password(user, password)
                .await
                .map_err(|e| {
                    ConnectionError::AuthenticationFailed(format!(
                        "Password authentication failed: {}",
                        e
                    ))
                })?;

            if authenticated {
                debug!("Authenticated using password");
                return Ok(());
            }
        }

        Err(ConnectionError::AuthenticationFailed(
            "All authentication methods failed".to_string(),
        ))
    }

    /// Try SSH agent authentication
    async fn try_agent_auth(
        _session: &mut Handle<ClientHandler>,
        _user: &str,
    ) -> ConnectionResult<()> {
        // russh's agent support requires the russh-agent crate
        // For now, we'll skip agent support and rely on key files
        // TODO: Add russh-agent dependency and implement agent support
        Err(ConnectionError::AuthenticationFailed(
            "SSH agent authentication not yet implemented".to_string(),
        ))
    }

    /// Try key-based authentication
    ///
    /// Supports Ed25519 and RSA keys, with or without passphrases.
    /// The key is loaded using russh_keys::load_secret_key which automatically
    /// detects the key type (Ed25519, RSA, etc.) based on the file format.
    async fn try_key_auth(
        session: &mut Handle<ClientHandler>,
        user: &str,
        key_path: &Path,
        passphrase: Option<&str>,
    ) -> ConnectionResult<()> {
        if !key_path.exists() {
            return Err(ConnectionError::AuthenticationFailed(format!(
                "Key file not found: {}",
                key_path.display()
            )));
        }

        // Load the private key
        let key_pair = if let Some(pass) = passphrase {
            // Load with passphrase
            load_secret_key(key_path, Some(pass)).map_err(|e| {
                ConnectionError::AuthenticationFailed(format!(
                    "Failed to load key {} with passphrase: {}",
                    key_path.display(),
                    e
                ))
            })?
        } else {
            // Try loading without passphrase first
            load_secret_key(key_path, None).map_err(|e| {
                ConnectionError::AuthenticationFailed(format!(
                    "Failed to load key {}: {}",
                    key_path.display(),
                    e
                ))
            })?
        };

        // Authenticate with the key
        let authenticated = session
            .authenticate_publickey(user, Arc::new(key_pair))
            .await
            .map_err(|e| {
                ConnectionError::AuthenticationFailed(format!(
                    "Key authentication failed for {}: {}",
                    key_path.display(),
                    e
                ))
            })?;

        if authenticated {
            Ok(())
        } else {
            Err(ConnectionError::AuthenticationFailed(
                "Key authentication failed".to_string(),
            ))
        }
    }
}


#[async_trait]
impl Connection for RusshConnection {
    fn identifier(&self) -> &str {
        &self.identifier
    }

    async fn is_alive(&self) -> bool {
        // Check if we're marked as connected (lock-free check)
        if !self.connected.load(Ordering::SeqCst) {
            return false;
        }

        // Check if we have a handle using read lock (allows concurrent checks)
        let has_handle = self.handle.read().await.is_some();
        if !has_handle {
            return false;
        }

        // We consider the connection alive if it's marked as connected and has a handle
        // A full health check would require opening a channel, but that's expensive
        // The connection will be marked as dead when an operation fails
        true
    }

    async fn execute(
        &self,
        command: &str,
        options: Option<ExecuteOptions>,
    ) -> ConnectionResult<CommandResult> {
        let options = options.unwrap_or_default();

        // Build the full command with options
        // Prepend environment variables to the command since russh doesn't have request_env
        let full_command = Self::build_command_with_env(command, &options);

        trace!(command = %full_command, "Executing remote command");

        // Execute the command with optional timeout
        let execute_future = async {
            // Get the handle using read lock - allows concurrent channel opens
            // We only hold the lock briefly to open a channel
            let handle_guard = self.handle.read().await;
            let handle: &Handle<ClientHandler> = handle_guard
                .as_ref()
                .ok_or_else(|| ConnectionError::ConnectionClosed)?;

            // 1. Open a channel (while holding read lock)
            let mut channel = handle.channel_open_session().await.map_err(|e| {
                ConnectionError::ExecutionFailed(format!("Failed to open channel: {}", e))
            })?;

            // Drop the handle guard to release the read lock
            drop(handle_guard);

            // 2. Execute the command
            channel.exec(true, full_command).await.map_err(|e| {
                ConnectionError::ExecutionFailed(format!("Failed to execute command: {}", e))
            })?;

            // Handle escalation password if needed
            if options.escalate && options.escalate_password.is_some() {
                use tokio::io::AsyncReadExt;
                let password = options.escalate_password.as_ref().unwrap();
                let password_data = format!("{}\n", password);
                let mut cursor = tokio::io::BufReader::new(password_data.as_bytes());
                channel
                    .data(&mut cursor)
                    .await
                    .map_err(|e| {
                        ConnectionError::ExecutionFailed(format!("Failed to write password: {}", e))
                    })?;
            }

            // 3. Capture stdout/stderr
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let mut exit_code = None;

            // Read all messages from the channel
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { ref data } => {
                        stdout.extend_from_slice(data);
                    }
                    ChannelMsg::ExtendedData { ref data, ext } => {
                        // Extended data type 1 is stderr
                        if ext == 1 {
                            stderr.extend_from_slice(data);
                        }
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        exit_code = Some(exit_status);
                    }
                    ChannelMsg::Eof => {
                        // End of file, continue reading until channel closes
                    }
                    ChannelMsg::Close => {
                        // Channel closed, we're done
                        break;
                    }
                    _ => {
                        // Ignore other message types
                    }
                }
            }

            // Wait for channel to close
            let _ = channel.eof().await.map_err(|e| {
                ConnectionError::ExecutionFailed(format!("Failed to send EOF: {}", e))
            });

            // 4. Return CommandResult
            // Exit status from SSH is u32, but we need i32 for CommandResult
            // Use i32::MAX for unknown exit code (None case) as it indicates an error
            let exit_code: i32 = exit_code.map(|e| e as i32).unwrap_or(i32::MAX);
            let stdout_str = String::from_utf8_lossy(&stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&stderr).to_string();

            trace!(exit_code = %exit_code, "Command completed");

            if exit_code == 0 {
                Ok(CommandResult::success(stdout_str, stderr_str))
            } else {
                Ok(CommandResult::failure(exit_code, stdout_str, stderr_str))
            }
        };

        // Apply timeout if specified
        if let Some(timeout_secs) = options.timeout {
            match tokio::time::timeout(Duration::from_secs(timeout_secs), execute_future).await {
                Ok(result) => result,
                Err(_) => Err(ConnectionError::Timeout(timeout_secs)),
            }
        } else {
            execute_future.await
        }
    }

    async fn upload(
        &self,
        local_path: &Path,
        remote_path: &Path,
        options: Option<TransferOptions>,
    ) -> ConnectionResult<()> {
        let options = options.unwrap_or_default();

        debug!(
            local = %local_path.display(),
            remote = %remote_path.display(),
            "Uploading file via SFTP"
        );

        // Get handle using read lock - allows concurrent uploads
        let handle_guard = self.handle.read().await;
        let handle = handle_guard
            .as_ref()
            .ok_or_else(|| ConnectionError::ConnectionClosed)?;

        // Open SFTP session (while holding read lock)
        let mut sftp = Self::open_sftp(handle).await?;

        // Release the read lock immediately after opening SFTP session
        drop(handle_guard);

        // Create parent directories if needed
        if options.create_dirs {
            if let Some(parent) = remote_path.parent() {
                Self::create_remote_dirs_sftp(&sftp, parent).await?;
            }
        }

        // Read local file
        let content = tokio::fs::read(local_path).await.map_err(|e| {
            ConnectionError::TransferFailed(format!(
                "Failed to read local file {}: {}",
                local_path.display(),
                e
            ))
        })?;

        // Create/open remote file for writing
        let remote_path_str = remote_path.to_string_lossy().to_string();
        let mut remote_file = sftp.create(&remote_path_str).await.map_err(|e| {
            ConnectionError::TransferFailed(format!(
                "Failed to create remote file {}: {}",
                remote_path.display(),
                e
            ))
        })?;

        // Write content to remote file
        remote_file.write_all(&content).await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to write to remote file: {}", e))
        })?;

        // Close the file
        drop(remote_file);

        // Set permissions using setstat
        if let Some(mode) = options.mode {
            let mut attrs = russh_sftp::protocol::FileAttributes::default();
            attrs.permissions = Some(mode);
            sftp.set_metadata(&remote_path_str, attrs)
                .await
                .map_err(|e| {
                    ConnectionError::TransferFailed(format!(
                        "Failed to set file permissions: {}",
                        e
                    ))
                })?;
        }

        // Drop the SFTP session before using execute()
        drop(sftp);

        // Set owner/group if specified using chown command
        if options.owner.is_some() || options.group.is_some() {
            let escaped_path = escape_shell_arg(&remote_path.to_string_lossy());
            let owner_group = match (&options.owner, &options.group) {
                (Some(owner), Some(group)) => format!("{}:{}", owner, group),
                (Some(owner), None) => owner.clone(),
                (None, Some(group)) => format!(":{}", group),
                (None, None) => unreachable!(),
            };
            let chown_cmd = format!("chown {} {}", owner_group, escaped_path);
            let result = self.execute(&chown_cmd, None).await?;
            if !result.success {
                warn!(
                    "Failed to set owner/group on {}: {}",
                    remote_path.display(),
                    result.stderr
                );
            }
        }

        Ok(())
    }

    async fn upload_content(
        &self,
        content: &[u8],
        remote_path: &Path,
        options: Option<TransferOptions>,
    ) -> ConnectionResult<()> {
        let options = options.unwrap_or_default();

        debug!(
            remote = %remote_path.display(),
            size = %content.len(),
            "Uploading content via SFTP"
        );

        // Get handle using read lock - allows concurrent uploads
        let handle_guard = self.handle.read().await;
        let handle = handle_guard
            .as_ref()
            .ok_or_else(|| ConnectionError::ConnectionClosed)?;

        // Open SFTP session (while holding read lock)
        let mut sftp = Self::open_sftp(handle).await?;

        // Release the read lock immediately after opening SFTP session
        drop(handle_guard);

        // Create parent directories if needed
        if options.create_dirs {
            if let Some(parent) = remote_path.parent() {
                Self::create_remote_dirs_sftp(&sftp, parent).await?;
            }
        }

        // Create/open remote file for writing
        let remote_path_str = remote_path.to_string_lossy().to_string();
        let mut remote_file = sftp.create(&remote_path_str).await.map_err(|e| {
            ConnectionError::TransferFailed(format!(
                "Failed to create remote file {}: {}",
                remote_path.display(),
                e
            ))
        })?;

        // Write content to remote file
        remote_file.write_all(content).await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to write to remote file: {}", e))
        })?;

        // Close the file
        drop(remote_file);

        // Set permissions using setstat
        if let Some(mode) = options.mode {
            let mut attrs = russh_sftp::protocol::FileAttributes::default();
            attrs.permissions = Some(mode);
            sftp.set_metadata(&remote_path_str, attrs)
                .await
                .map_err(|e| {
                    ConnectionError::TransferFailed(format!(
                        "Failed to set file permissions: {}",
                        e
                    ))
                })?;
        }

        // Drop the SFTP session before using execute()
        drop(sftp);

        // Set owner/group if specified using chown command
        if options.owner.is_some() || options.group.is_some() {
            let escaped_path = escape_shell_arg(&remote_path.to_string_lossy());
            let owner_group = match (&options.owner, &options.group) {
                (Some(owner), Some(group)) => format!("{}:{}", owner, group),
                (Some(owner), None) => owner.clone(),
                (None, Some(group)) => format!(":{}", group),
                (None, None) => unreachable!(),
            };
            let chown_cmd = format!("chown {} {}", owner_group, escaped_path);
            let result = self.execute(&chown_cmd, None).await?;
            if !result.success {
                warn!(
                    "Failed to set owner/group on {}: {}",
                    remote_path.display(),
                    result.stderr
                );
            }
        }

        Ok(())
    }

    async fn download(&self, remote_path: &Path, local_path: &Path) -> ConnectionResult<()> {
        debug!(
            remote = %remote_path.display(),
            local = %local_path.display(),
            "Downloading file via russh SFTP"
        );

        // Get handle using read lock - allows concurrent downloads
        let handle_guard = self.handle.read().await;
        let handle = handle_guard
            .as_ref()
            .ok_or_else(|| ConnectionError::ConnectionClosed)?;

        // Open SFTP session (while holding read lock)
        let sftp = Self::open_sftp(handle).await?;

        // Release the read lock immediately after opening SFTP session
        drop(handle_guard);

        // Open remote file for reading
        let remote_path_str = remote_path.to_string_lossy().to_string();
        let mut remote_file = sftp.open(&remote_path_str).await.map_err(|e| {
            ConnectionError::TransferFailed(format!(
                "Failed to open remote file {}: {}",
                remote_path.display(),
                e
            ))
        })?;

        // Read content from remote file
        let mut content = Vec::new();
        remote_file.read_to_end(&mut content).await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to read remote file: {}", e))
        })?;

        // Create parent directories for local file
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ConnectionError::TransferFailed(format!(
                    "Failed to create local directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        // Write local file
        tokio::fs::write(local_path, &content).await.map_err(|e| {
            ConnectionError::TransferFailed(format!(
                "Failed to write local file {}: {}",
                local_path.display(),
                e
            ))
        })?;

        debug!("Download completed successfully");
        Ok(())
    }

    async fn download_content(&self, remote_path: &Path) -> ConnectionResult<Vec<u8>> {
        debug!(remote = %remote_path.display(), "Downloading content via russh SFTP");

        // Get handle using read lock - allows concurrent downloads
        let handle_guard = self.handle.read().await;
        let handle = handle_guard
            .as_ref()
            .ok_or_else(|| ConnectionError::ConnectionClosed)?;

        // Open SFTP session (while holding read lock)
        let sftp = Self::open_sftp(handle).await?;

        // Release the read lock immediately after opening SFTP session
        drop(handle_guard);

        // Open remote file for reading
        let remote_path_str = remote_path.to_string_lossy().to_string();
        let mut remote_file = sftp.open(&remote_path_str).await.map_err(|e| {
            ConnectionError::TransferFailed(format!(
                "Failed to open remote file {}: {}",
                remote_path.display(),
                e
            ))
        })?;

        // Read content from remote file
        let mut content = Vec::new();
        remote_file.read_to_end(&mut content).await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to read remote file: {}", e))
        })?;

        debug!(size = %content.len(), "Content download completed successfully");
        Ok(content)
    }

    async fn path_exists(&self, path: &Path) -> ConnectionResult<bool> {
        trace!(path = %path.display(), "Checking if path exists via SFTP");

        // Get handle using read lock - allows concurrent checks
        let handle_guard = self.handle.read().await;
        let handle = handle_guard
            .as_ref()
            .ok_or_else(|| ConnectionError::ConnectionClosed)?;

        // Open SFTP session (while holding read lock)
        let sftp = Self::open_sftp(handle).await?;

        // Release the read lock immediately after opening SFTP session
        drop(handle_guard);

        // Use try_exists to check if path exists
        let path_str = path.to_string_lossy().to_string();
        match sftp.try_exists(&path_str).await {
            Ok(exists) => Ok(exists),
            Err(e) => {
                // Log the error but treat certain errors as "does not exist"
                debug!(path = %path.display(), error = %e, "Error checking path existence");
                Ok(false)
            }
        }
    }

    async fn is_directory(&self, path: &Path) -> ConnectionResult<bool> {
        trace!(path = %path.display(), "Checking if path is directory via SFTP");

        // Get handle using read lock - allows concurrent checks
        let handle_guard = self.handle.read().await;
        let handle = handle_guard
            .as_ref()
            .ok_or_else(|| ConnectionError::ConnectionClosed)?;

        // Open SFTP session (while holding read lock)
        let sftp = Self::open_sftp(handle).await?;

        // Release the read lock immediately after opening SFTP session
        drop(handle_guard);

        // Get metadata and check if it's a directory
        let path_str = path.to_string_lossy().to_string();
        match sftp.metadata(&path_str).await {
            Ok(attrs) => Ok(attrs.is_dir()),
            Err(_) => Ok(false),
        }
    }

    async fn stat(&self, path: &Path) -> ConnectionResult<FileStat> {
        trace!(path = %path.display(), "Getting file stats via SFTP");

        // Get handle using read lock - allows concurrent stat calls
        let handle_guard = self.handle.read().await;
        let handle = handle_guard
            .as_ref()
            .ok_or_else(|| ConnectionError::ConnectionClosed)?;

        // Open SFTP session (while holding read lock)
        let sftp = Self::open_sftp(handle).await?;

        // Release the read lock immediately after opening SFTP session
        drop(handle_guard);

        let path_str = path.to_string_lossy().to_string();

        // First get symlink metadata to determine if it's a symlink
        let is_symlink = match sftp.symlink_metadata(&path_str).await {
            Ok(attrs) => attrs.is_symlink(),
            Err(_) => false,
        };

        // Get regular metadata (follows symlinks)
        let attrs = sftp.metadata(&path_str).await.map_err(|e| {
            // Check for common SFTP error conditions
            let error_str = e.to_string().to_lowercase();
            if error_str.contains("no such file") || error_str.contains("not found") {
                ConnectionError::TransferFailed(format!(
                    "File not found: {}",
                    path.display()
                ))
            } else if error_str.contains("permission denied") {
                ConnectionError::TransferFailed(format!(
                    "Permission denied: {}",
                    path.display()
                ))
            } else {
                ConnectionError::TransferFailed(format!(
                    "Failed to stat {}: {}",
                    path.display(),
                    e
                ))
            }
        })?;

        // Extract file attributes from russh-sftp FileAttributes
        let size = attrs.size.unwrap_or(0);
        let mode = attrs.permissions.unwrap_or(0);
        let uid = attrs.uid.unwrap_or(0);
        let gid = attrs.gid.unwrap_or(0);
        let atime = attrs.atime.map(|t| t as i64).unwrap_or(0);
        let mtime = attrs.mtime.map(|t| t as i64).unwrap_or(0);

        Ok(FileStat {
            size,
            mode,
            uid,
            gid,
            atime,
            mtime,
            is_dir: attrs.is_dir(),
            is_file: attrs.is_regular(),
            is_symlink,
        })
    }


    async fn close(&self) -> ConnectionResult<()> {
        debug!("Closing SSH connection");

        // Mark as disconnected first (lock-free)
        self.connected.store(false, Ordering::SeqCst);

        // Take the handle out using write lock - this is the only write operation
        let handle = {
            let mut handle_guard = self.handle.write().await;
            handle_guard.take()
        };

        // Close the connection if we had one
        if let Some(handle) = handle {
            // Request disconnect from the SSH server
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "Connection closed by client", "en")
                .await;
        }

        Ok(())
    }
}

/// Maximum number of concurrent SSH channels to use for batch execution.
/// SSH spec allows many more, but we stay conservative to avoid overwhelming servers.
const MAX_CONCURRENT_CHANNELS: usize = 10;

impl RusshConnection {
    /// Execute multiple commands in parallel using channel multiplexing.
    ///
    /// This method opens multiple SSH channels on the same connection and executes
    /// commands concurrently. Results are returned in the same order as the input commands.
    ///
    /// # Arguments
    ///
    /// * `commands` - A slice of command strings to execute
    /// * `options` - Optional execution options applied to all commands
    ///
    /// # Returns
    ///
    /// A vector of results in the same order as the input commands. Each command
    /// either succeeds with a `CommandResult` or fails with a `ConnectionError`.
    /// If one command fails, others continue executing.
    ///
    /// # Limits
    ///
    /// * Maximum 10 concurrent channels to avoid overwhelming SSH servers
    /// * Per-command timeout (from options), not total timeout
    ///
    /// # Example
    ///
    /// ```ignore
    /// let commands = vec![
    ///     "hostname".to_string(),
    ///     "uptime".to_string(),
    ///     "date".to_string(),
    /// ];
    /// let results = conn.execute_batch(&commands, None).await;
    /// for (cmd, result) in commands.iter().zip(results.iter()) {
    ///     match result {
    ///         Ok(r) => println!("{}: {}", cmd, r.stdout),
    ///         Err(e) => eprintln!("{}: error: {}", cmd, e),
    ///     }
    /// }
    /// ```
    pub async fn execute_batch(
        &self,
        commands: &[String],
        options: Option<ExecuteOptions>,
    ) -> Vec<ConnectionResult<CommandResult>> {
        if commands.is_empty() {
            return Vec::new();
        }

        // Quick check if connection is closed
        if !self.connected.load(Ordering::SeqCst) {
            return commands
                .iter()
                .map(|_| Err(ConnectionError::ConnectionClosed))
                .collect();
        }

        let options = options.unwrap_or_default();
        let timeout_duration = options.timeout.map(Duration::from_secs);

        debug!(
            command_count = %commands.len(),
            "Executing batch of commands with channel multiplexing"
        );

        // Get a clone of the handle Arc for spawning tasks
        let handle_arc = self.handle.clone();

        // Prepare all command strings upfront
        let prepared_commands: Vec<(usize, String)> = commands
            .iter()
            .enumerate()
            .map(|(idx, cmd)| (idx, Self::build_command_with_env(cmd, &options)))
            .collect();

        // Use semaphore to limit concurrent channels
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CHANNELS));

        // Spawn tasks for each command
        let mut tasks: Vec<tokio::task::JoinHandle<(usize, ConnectionResult<CommandResult>)>> =
            Vec::with_capacity(commands.len());

        for (idx, full_command) in prepared_commands {
            let sem = semaphore.clone();
            let handle_arc = handle_arc.clone();
            let escalate = options.escalate;
            let escalate_password = options.escalate_password.clone();
            let timeout_dur = timeout_duration;

            let task = tokio::spawn(async move {
                // Acquire semaphore permit (limits concurrent channels)
                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => {
                        return (
                            idx,
                            Err(ConnectionError::ExecutionFailed(
                                "Semaphore closed".to_string(),
                            )),
                        );
                    }
                };

                let result = Self::execute_single_channel(
                    &handle_arc,
                    &full_command,
                    escalate,
                    escalate_password,
                    timeout_dur,
                )
                .await;

                (idx, result)
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete and collect results
        let task_results = futures::future::join_all(tasks).await;

        // Collect results in order
        let mut results: Vec<ConnectionResult<CommandResult>> =
            Vec::with_capacity(commands.len());

        // Initialize with error placeholders
        for idx in 0..commands.len() {
            results.push(Err(ConnectionError::ExecutionFailed(format!(
                "Command {} failed to execute (task error)",
                idx
            ))));
        }

        // Fill in actual results
        for task_result in task_results {
            match task_result {
                Ok((idx, result)) => {
                    results[idx] = result;
                }
                Err(join_error) => {
                    // This happens if the task panicked - shouldn't normally occur
                    warn!(error = %join_error, "Task panicked during batch execution");
                }
            }
        }

        results
    }

    /// Execute a single command on a new channel.
    ///
    /// This is a helper method used by `execute_batch` to run one command
    /// on its own SSH channel. It opens a new channel, executes the command,
    /// collects output, and returns the result.
    async fn execute_single_channel(
        handle_arc: &Arc<RwLock<Option<Handle<ClientHandler>>>>,
        full_command: &str,
        escalate: bool,
        escalate_password: Option<String>,
        timeout_duration: Option<Duration>,
    ) -> ConnectionResult<CommandResult> {
        let execute_future = async {
            // Get the handle using read lock - allows concurrent channel opens
            let handle_guard = handle_arc.read().await;
            let handle = handle_guard
                .as_ref()
                .ok_or_else(|| ConnectionError::ConnectionClosed)?;

            // Open a new channel
            let mut channel = handle.channel_open_session().await.map_err(|e| {
                ConnectionError::ExecutionFailed(format!("Failed to open channel: {}", e))
            })?;

            // Release the lock immediately after opening the channel
            drop(handle_guard);

            // Execute the command on this channel
            channel.exec(true, full_command).await.map_err(|e| {
                ConnectionError::ExecutionFailed(format!("Failed to execute command: {}", e))
            })?;

            // Handle escalation password if needed
            if escalate && escalate_password.is_some() {
                let password = escalate_password.as_ref().unwrap();
                let password_data = format!("{}\n", password);
                let mut cursor = tokio::io::BufReader::new(password_data.as_bytes());
                channel.data(&mut cursor).await.map_err(|e| {
                    ConnectionError::ExecutionFailed(format!("Failed to write password: {}", e))
                })?;
            }

            // Collect stdout, stderr, and exit code
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let mut exit_code = None;

            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { ref data } => {
                        stdout.extend_from_slice(data);
                    }
                    ChannelMsg::ExtendedData { ref data, ext } => {
                        if ext == 1 {
                            stderr.extend_from_slice(data);
                        }
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        exit_code = Some(exit_status);
                    }
                    ChannelMsg::Eof | ChannelMsg::Close => {
                        if matches!(msg, ChannelMsg::Close) {
                            break;
                        }
                    }
                    _ => {}
                }
            }

            // Send EOF to cleanly close our side
            let _ = channel.eof().await;

            // Build result
            let exit_code = exit_code.map(|e| e as i32).unwrap_or(i32::MAX);
            let stdout_str = String::from_utf8_lossy(&stdout).to_string();
            let stderr_str = String::from_utf8_lossy(&stderr).to_string();

            trace!(exit_code = %exit_code, "Channel command completed");

            if exit_code == 0 {
                Ok(CommandResult::success(stdout_str, stderr_str))
            } else {
                Ok(CommandResult::failure(exit_code, stdout_str, stderr_str))
            }
        };

        // Apply per-command timeout
        if let Some(timeout) = timeout_duration {
            match tokio::time::timeout(timeout, execute_future).await {
                Ok(result) => result,
                Err(_) => Err(ConnectionError::Timeout(timeout.as_secs())),
            }
        } else {
            execute_future.await
        }
    }
}

impl std::fmt::Debug for RusshConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let connected = self.connected.load(Ordering::Relaxed);
        f.debug_struct("RusshConnection")
            .field("identifier", &self.identifier)
            .field("connected", &connected)
            .finish()
    }
}

/// Builder for Russh connections
pub struct RusshConnectionBuilder {
    host: String,
    port: u16,
    user: String,
    password: Option<String>,
    private_key: Option<String>,
    timeout: Option<u64>,
    compression: bool,
}

impl RusshConnectionBuilder {
    /// Create a new Russh connection builder
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: 22,
            user: std::env::var("USER").unwrap_or_else(|_| "root".to_string()),
            password: None,
            private_key: None,
            timeout: Some(30),
            compression: false,
        }
    }

    /// Set the port
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the username
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = user.into();
        self
    }

    /// Set the password
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Set the private key path
    pub fn private_key(mut self, path: impl Into<String>) -> Self {
        self.private_key = Some(path.into());
        self
    }

    /// Set the connection timeout
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Enable compression
    pub fn compression(mut self, enabled: bool) -> Self {
        self.compression = enabled;
        self
    }

    /// Build and connect
    pub async fn connect(self) -> ConnectionResult<RusshConnection> {
        let host_config = HostConfig {
            hostname: Some(self.host.clone()),
            port: Some(self.port),
            user: Some(self.user.clone()),
            password: self.password,
            identity_file: self.private_key,
            connect_timeout: self.timeout,
            compression: self.compression,
            ..Default::default()
        };

        let config = ConnectionConfig::default();
        RusshConnection::connect(
            &self.host,
            self.port,
            &self.user,
            Some(host_config),
            &config,
        )
        .await
    }
}


// ============================================================================
// SSH Request Pipelining
// ============================================================================

/// A pending command in the pipeline
#[derive(Debug, Clone)]
pub struct PendingCommand {
    /// The command string to execute
    command: String,
    /// Options for command execution
    options: ExecuteOptions,
}

impl PendingCommand {
    /// Create a new pending command
    pub fn new(command: impl Into<String>, options: Option<ExecuteOptions>) -> Self {
        Self {
            command: command.into(),
            options: options.unwrap_or_default(),
        }
    }

    /// Get the command string
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Get the execution options
    pub fn options(&self) -> &ExecuteOptions {
        &self.options
    }
}

/// SSH request pipelining executor
///
/// This struct enables true SSH pipelining by opening multiple channels
/// before previous commands finish, executing all commands, and then
/// collecting all results. This significantly reduces latency when
/// executing multiple commands on a remote host.
///
/// # How It Works
///
/// SSH allows multiple channels to be opened on a single connection.
/// Traditional execution waits for each command to complete before
/// starting the next. With pipelining:
///
/// 1. All SSH channels are opened in parallel (without waiting)
/// 2. All commands are executed on their respective channels
/// 3. All outputs are collected concurrently
///
/// This eliminates the round-trip latency between commands.
///
/// # Difference from `execute_batch`
///
/// While `execute_batch` executes commands in parallel using spawned tasks,
/// `PipelinedExecutor` provides a builder pattern for queuing commands
/// without any network activity until `flush()` is called. This allows
/// for more efficient batching when commands are added incrementally.
///
/// # Example
///
/// ```ignore
/// use rustible::connection::russh::{RusshConnection, PipelinedExecutor};
///
/// let conn = RusshConnection::connect(...).await?;
/// let mut pipeline = conn.pipeline();
///
/// // Queue commands - these don't execute yet
/// pipeline.queue("echo 'hello'", None);
/// pipeline.queue("echo 'world'", None);
/// pipeline.queue("date", None);
///
/// // Flush executes all commands with pipelining
/// let results = pipeline.flush().await;
/// for result in results {
///     println!("{:?}", result);
/// }
/// ```
///
/// # Memory Usage
///
/// The pipeline stores commands in memory until flush() is called.
/// For very large numbers of commands, consider batching into smaller
/// pipelines to limit memory usage.
pub struct PipelinedExecutor<'a> {
    /// Reference to the underlying SSH connection
    connection: &'a RusshConnection,
    /// Queue of pending commands to execute
    pending: Vec<PendingCommand>,
    /// Default timeout for all commands (in seconds)
    default_timeout: Option<u64>,
}

impl<'a> PipelinedExecutor<'a> {
    /// Create a new pipelined executor for the given connection
    pub fn new(connection: &'a RusshConnection) -> Self {
        Self {
            connection,
            pending: Vec::new(),
            default_timeout: None,
        }
    }

    /// Create a new pipelined executor with a default timeout
    pub fn with_timeout(connection: &'a RusshConnection, timeout_secs: u64) -> Self {
        Self {
            connection,
            pending: Vec::new(),
            default_timeout: Some(timeout_secs),
        }
    }

    /// Create a new pipelined executor with pre-allocated capacity
    pub fn with_capacity(connection: &'a RusshConnection, capacity: usize) -> Self {
        Self {
            connection,
            pending: Vec::with_capacity(capacity),
            default_timeout: None,
        }
    }

    /// Queue a command for execution without blocking
    ///
    /// This method adds a command to the internal queue. The command
    /// will not be executed until `flush()` is called.
    ///
    /// # Arguments
    ///
    /// * `command` - The shell command to execute
    /// * `options` - Optional execution options (cwd, env, timeout, etc.)
    pub fn queue(&mut self, command: impl Into<String>, options: Option<ExecuteOptions>) {
        self.pending.push(PendingCommand::new(command, options));
    }

    /// Queue multiple commands at once
    ///
    /// All commands will use default execution options.
    pub fn queue_all<I, S>(&mut self, commands: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for cmd in commands {
            self.queue(cmd, None);
        }
    }

    /// Queue a command with specific options
    pub fn queue_with_options(&mut self, command: impl Into<String>, options: ExecuteOptions) {
        self.pending.push(PendingCommand::new(command, Some(options)));
    }

    /// Get the number of pending commands
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Check if there are any pending commands
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Clear all pending commands without executing them
    pub fn clear(&mut self) {
        self.pending.clear();
    }

    /// Get a reference to pending commands
    pub fn pending(&self) -> &[PendingCommand] {
        &self.pending
    }

    /// Set the default timeout for all commands
    pub fn set_default_timeout(&mut self, timeout_secs: Option<u64>) {
        self.default_timeout = timeout_secs;
    }

    /// Flush the pipeline: send all commands and collect all responses
    ///
    /// This is the core pipelining method. It works by:
    /// 1. Opening all SSH channels in parallel (without waiting for previous ones)
    /// 2. Executing all commands on their respective channels
    /// 3. Collecting all outputs concurrently
    ///
    /// Returns a vector of results in the same order as commands were queued.
    ///
    /// # Errors
    ///
    /// Individual command failures are returned in the result vector.
    /// If the connection is closed, all commands will return `ConnectionClosed` errors.
    pub async fn flush(mut self) -> Vec<ConnectionResult<CommandResult>> {
        if self.pending.is_empty() {
            return Vec::new();
        }

        // Take ownership of pending commands (leaves empty vec to satisfy Drop)
        let commands = std::mem::take(&mut self.pending);
        let num_commands = commands.len();
        let default_timeout = self.default_timeout;

        debug!(
            num_commands = %num_commands,
            "Flushing pipelined commands"
        );

        // Get the SSH handle - use read() since we only need to open channels
        let handle_guard = self.connection.handle.read().await;
        let handle = match handle_guard.as_ref() {
            Some(h) => h,
            None => {
                // Connection is closed, return errors for all commands
                return (0..num_commands)
                    .map(|_| Err(ConnectionError::ConnectionClosed))
                    .collect();
            }
        };

        // Phase 1: Open all channels in parallel
        // This is the key insight for pipelining - we can open channels
        // before the previous ones complete their command execution
        trace!("Phase 1: Opening {} channels in parallel", num_commands);

        let channel_futures: Vec<_> = (0..num_commands)
            .map(|_| handle.channel_open_session())
            .collect();

        let channel_results = futures::future::join_all(channel_futures).await;

        // Drop the handle guard early to allow other operations
        drop(handle_guard);

        // Collect opened channels, tracking which ones failed
        let mut channels: Vec<Option<russh::Channel<russh::client::Msg>>> = Vec::with_capacity(num_commands);
        let mut channel_errors: Vec<Option<ConnectionError>> = (0..num_commands).map(|_| None).collect();

        for (idx, result) in channel_results.into_iter().enumerate() {
            match result {
                Ok(channel) => {
                    channels.push(Some(channel));
                }
                Err(e) => {
                    channels.push(None);
                    channel_errors[idx] = Some(ConnectionError::ExecutionFailed(
                        format!("Failed to open channel: {}", e)
                    ));
                }
            }
        }

        // Phase 2: Execute commands on all channels
        // Build the full command strings and execute them
        trace!("Phase 2: Executing {} commands", num_commands);

        for (idx, cmd) in commands.iter().enumerate() {
            if channel_errors[idx].is_some() {
                continue; // Skip if channel open failed
            }

            if let Some(Some(channel)) = channels.get_mut(idx) {
                let full_command = RusshConnection::build_command_with_env(&cmd.command, &cmd.options);

                if let Err(e) = channel.exec(true, full_command).await {
                    // Mark this channel as failed
                    channels[idx] = None;
                    channel_errors[idx] = Some(ConnectionError::ExecutionFailed(
                        format!("Failed to execute command: {}", e)
                    ));
                }
            }
        }

        // Handle escalation passwords if needed (for commands that require it)
        for (idx, cmd) in commands.iter().enumerate() {
            if channel_errors[idx].is_some() {
                continue;
            }

            if cmd.options.escalate && cmd.options.escalate_password.is_some() {
                if let Some(Some(channel)) = channels.get_mut(idx) {
                    let password = cmd.options.escalate_password.as_ref().unwrap();
                    let password_data = format!("{}\n", password);
                    let mut cursor = tokio::io::BufReader::new(password_data.as_bytes());

                    if let Err(e) = channel.data(&mut cursor).await {
                        channels[idx] = None;
                        channel_errors[idx] = Some(ConnectionError::ExecutionFailed(
                            format!("Failed to write password: {}", e)
                        ));
                    }
                }
            }
        }

        // Phase 3: Collect outputs from all channels concurrently
        trace!("Phase 3: Collecting outputs from {} channels", num_commands);

        let collect_futures: Vec<_> = channels
            .into_iter()
            .zip(channel_errors.into_iter())
            .zip(commands.iter())
            .map(|((channel_opt, error_opt), cmd)| {
                let timeout = cmd.options.timeout.or(default_timeout);

                async move {
                    // If we already have an error, return it
                    if let Some(e) = error_opt {
                        return Err(e);
                    }

                    // If channel is None, something went wrong
                    let Some(mut channel) = channel_opt else {
                        return Err(ConnectionError::ExecutionFailed(
                            "Channel not available".to_string()
                        ));
                    };

                    // Collect output with optional timeout
                    let collect_output = async {
                        let mut stdout = Vec::new();
                        let mut stderr = Vec::new();
                        let mut exit_code = None;

                        while let Some(msg) = channel.wait().await {
                            match msg {
                                ChannelMsg::Data { ref data } => {
                                    stdout.extend_from_slice(data);
                                }
                                ChannelMsg::ExtendedData { ref data, ext } => {
                                    if ext == 1 {
                                        stderr.extend_from_slice(data);
                                    }
                                }
                                ChannelMsg::ExitStatus { exit_status } => {
                                    exit_code = Some(exit_status);
                                }
                                ChannelMsg::Eof | ChannelMsg::Close => {
                                    if matches!(msg, ChannelMsg::Close) {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }

                        // Send EOF to properly close the channel
                        let _ = channel.eof().await;

                        let exit_code = exit_code.map(|e| e as i32).unwrap_or(i32::MAX);
                        let stdout_str = String::from_utf8_lossy(&stdout).to_string();
                        let stderr_str = String::from_utf8_lossy(&stderr).to_string();

                        if exit_code == 0 {
                            Ok(CommandResult::success(stdout_str, stderr_str))
                        } else {
                            Ok(CommandResult::failure(exit_code, stdout_str, stderr_str))
                        }
                    };

                    if let Some(timeout_secs) = timeout {
                        match tokio::time::timeout(
                            Duration::from_secs(timeout_secs),
                            collect_output
                        ).await {
                            Ok(result) => result,
                            Err(_) => Err(ConnectionError::Timeout(timeout_secs)),
                        }
                    } else {
                        collect_output.await
                    }
                }
            })
            .collect();

        let results = futures::future::join_all(collect_futures).await;

        debug!(
            num_commands = %num_commands,
            successful = %results.iter().filter(|r| r.is_ok()).count(),
            "Pipeline flush completed"
        );

        results
    }

    /// Flush the pipeline and return results only for successful commands
    ///
    /// This is a convenience method that filters out failed commands
    /// and returns only successful results.
    pub async fn flush_ok(self) -> Vec<CommandResult> {
        self.flush()
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Flush the pipeline and return the first error if any command fails
    ///
    /// Returns Ok with all results if all commands succeed, or the first
    /// error encountered.
    pub async fn flush_all_ok(self) -> ConnectionResult<Vec<CommandResult>> {
        let results = self.flush().await;
        let mut ok_results = Vec::with_capacity(results.len());

        for result in results {
            match result {
                Ok(r) => ok_results.push(r),
                Err(e) => return Err(e),
            }
        }

        Ok(ok_results)
    }

    /// Flush and collect results with their original commands
    ///
    /// Returns tuples of (command, result) for easy correlation.
    pub async fn flush_with_commands(self) -> Vec<(String, ConnectionResult<CommandResult>)> {
        let commands: Vec<String> = self.pending.iter().map(|c| c.command.clone()).collect();
        let results = self.flush().await;

        commands.into_iter().zip(results).collect()
    }
}

impl RusshConnection {
    /// Create a new pipelined executor for this connection
    ///
    /// This allows executing multiple commands with minimal latency
    /// by leveraging SSH channel pipelining.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut pipeline = connection.pipeline();
    /// pipeline.queue("ls -la", None);
    /// pipeline.queue("df -h", None);
    /// pipeline.queue("free -m", None);
    /// let results = pipeline.flush().await;
    /// ```
    pub fn pipeline(&self) -> PipelinedExecutor<'_> {
        PipelinedExecutor::new(self)
    }

    /// Create a new pipelined executor with a default timeout
    ///
    /// All commands will use this timeout unless overridden in their options.
    pub fn pipeline_with_timeout(&self, timeout_secs: u64) -> PipelinedExecutor<'_> {
        PipelinedExecutor::with_timeout(self, timeout_secs)
    }

    /// Create a new pipelined executor with pre-allocated capacity
    ///
    /// Use this when you know approximately how many commands you'll execute.
    pub fn pipeline_with_capacity(&self, capacity: usize) -> PipelinedExecutor<'_> {
        PipelinedExecutor::with_capacity(self, capacity)
    }

    /// Execute multiple commands with pipelining (convenience method)
    ///
    /// This is a convenience method that creates a pipeline, queues all commands,
    /// and flushes in one call.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let results = connection.execute_pipelined(&[
    ///     "echo hello",
    ///     "echo world",
    ///     "date",
    /// ]).await;
    /// ```
    pub async fn execute_pipelined<I, S>(&self, commands: I) -> Vec<ConnectionResult<CommandResult>>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut pipeline = self.pipeline();
        pipeline.queue_all(commands);
        pipeline.flush().await
    }
}

/// Drop implementation ensures pending commands are logged if not flushed
impl<'a> Drop for PipelinedExecutor<'a> {
    fn drop(&mut self) {
        if !self.pending.is_empty() {
            warn!(
                pending_count = %self.pending.len(),
                "PipelinedExecutor dropped with pending commands that were not flushed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_russh_connection_builder() {
        let builder = RusshConnectionBuilder::new("example.com")
            .port(2222)
            .user("admin")
            .compression(true);

        assert_eq!(builder.host, "example.com");
        assert_eq!(builder.port, 2222);
        assert_eq!(builder.user, "admin");
        assert!(builder.compression);
    }

    #[test]
    fn test_build_command_basic() {
        let options = ExecuteOptions::default();
        let cmd = RusshConnection::build_command("echo hello", &options);
        assert_eq!(cmd, "echo hello");
    }

    #[test]
    fn test_build_command_with_cwd() {
        let options = ExecuteOptions::new().with_cwd("/tmp");
        let cmd = RusshConnection::build_command("echo hello", &options);
        assert_eq!(cmd, "cd /tmp && echo hello");
    }

    #[test]
    fn test_build_command_with_escalation() {
        let options = ExecuteOptions::new().with_escalation(Some("admin".to_string()));
        let cmd = RusshConnection::build_command("echo hello", &options);
        assert_eq!(cmd, "sudo -u admin -- echo hello");
    }

    #[test]
    fn test_build_command_with_cwd_and_escalation() {
        let options = ExecuteOptions::new()
            .with_cwd("/var/log")
            .with_escalation(None);
        let cmd = RusshConnection::build_command("cat syslog", &options);
        assert_eq!(cmd, "cd /var/log && sudo -u root -- cat syslog");
    }

    #[test]
    fn test_escape_shell_arg_simple() {
        assert_eq!(escape_shell_arg("hello"), "'hello'");
    }

    #[test]
    fn test_escape_shell_arg_with_spaces() {
        assert_eq!(escape_shell_arg("/path/with spaces/file.txt"), "'/path/with spaces/file.txt'");
    }

    #[test]
    fn test_escape_shell_arg_with_quotes() {
        assert_eq!(escape_shell_arg("it's a test"), "'it'\\''s a test'");
    }

    #[test]
    fn test_escape_shell_arg_with_special_chars() {
        assert_eq!(escape_shell_arg("test$var`cmd`"), "'test$var`cmd`'");
    }

    #[test]
    fn test_max_concurrent_channels_constant() {
        // Ensure we have a reasonable limit on concurrent channels
        assert_eq!(MAX_CONCURRENT_CHANNELS, 10);
        assert!(MAX_CONCURRENT_CHANNELS > 0);
        assert!(MAX_CONCURRENT_CHANNELS <= 20); // SSH servers typically support at least 10
    }

    #[test]
    fn test_pending_command_new() {
        let cmd = PendingCommand::new("echo hello", None);
        assert_eq!(cmd.command(), "echo hello");
        assert_eq!(cmd.options().cwd, None);
        assert!(!cmd.options().escalate);
    }

    #[test]
    fn test_pending_command_with_options() {
        let options = ExecuteOptions::new()
            .with_cwd("/tmp")
            .with_timeout(30);
        let cmd = PendingCommand::new("echo hello", Some(options));
        assert_eq!(cmd.command(), "echo hello");
        assert_eq!(cmd.options().cwd, Some("/tmp".to_string()));
        assert_eq!(cmd.options().timeout, Some(30));
    }
}
