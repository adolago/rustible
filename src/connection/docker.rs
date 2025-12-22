//! Docker connection module
//!
//! This module provides connectivity to Docker containers using the
//! docker CLI commands. It allows executing commands inside containers
//! and copying files to/from containers.

use async_trait::async_trait;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, trace};

use super::{
    CommandResult, Connection, ConnectionError, ConnectionResult, ExecuteOptions, FileStat,
    TransferOptions,
};

/// Docker connection for executing commands inside containers
#[derive(Debug, Clone)]
pub struct DockerConnection {
    /// Container ID or name
    container: String,
    /// Docker executable path (default: "docker")
    docker_path: String,
    /// Whether to use docker compose exec instead of docker exec
    use_compose: bool,
    /// Service name for docker compose
    compose_service: Option<String>,
}

impl DockerConnection {
    /// Create a new Docker connection
    pub fn new(container: impl Into<String>) -> Self {
        Self {
            container: container.into(),
            docker_path: "docker".to_string(),
            use_compose: false,
            compose_service: None,
        }
    }

    /// Create a new Docker connection with a custom docker path
    pub fn with_docker_path(container: impl Into<String>, docker_path: impl Into<String>) -> Self {
        Self {
            container: container.into(),
            docker_path: docker_path.into(),
            use_compose: false,
            compose_service: None,
        }
    }

    /// Create a Docker Compose connection
    pub fn compose(service: impl Into<String>) -> Self {
        Self {
            container: String::new(),
            docker_path: "docker".to_string(),
            use_compose: true,
            compose_service: Some(service.into()),
        }
    }

    /// Set the container ID
    pub fn container(mut self, container: impl Into<String>) -> Self {
        self.container = container.into();
        self
    }

    /// Build the docker exec command
    fn build_exec_command(&self, command: &str, options: &ExecuteOptions) -> Command {
        let mut cmd = Command::new(&self.docker_path);

        if self.use_compose {
            cmd.arg("compose").arg("exec");

            // Add compose-specific options
            cmd.arg("-T"); // Disable pseudo-TTY

            if let Some(service) = &self.compose_service {
                cmd.arg(service);
            }
        } else {
            cmd.arg("exec");

            // Add docker exec options
            cmd.arg("-i"); // Keep STDIN open

            // Set user if escalation is requested
            if options.escalate {
                let user = options.escalate_user.as_deref().unwrap_or("root");
                cmd.arg("-u").arg(user);
            }

            // Set working directory
            if let Some(cwd) = &options.cwd {
                cmd.arg("-w").arg(cwd);
            }

            // Set environment variables
            for (key, value) in &options.env {
                cmd.arg("-e").arg(format!("{}={}", key, value));
            }

            cmd.arg(&self.container);
        }

        // Add the actual command
        cmd.arg("sh").arg("-c").arg(command);

        // Configure stdio
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd
    }

    /// Build docker cp command for uploading
    fn build_cp_to_container_command(&self, local_path: &Path, remote_path: &Path) -> Command {
        let mut cmd = Command::new(&self.docker_path);

        cmd.arg("cp")
            .arg(local_path)
            .arg(format!("{}:{}", self.container, remote_path.display()));

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd
    }

    /// Build docker cp command for downloading
    fn build_cp_from_container_command(&self, remote_path: &Path, local_path: &Path) -> Command {
        let mut cmd = Command::new(&self.docker_path);

        cmd.arg("cp")
            .arg(format!("{}:{}", self.container, remote_path.display()))
            .arg(local_path);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd
    }

    /// Check if container is running
    async fn is_container_running(&self) -> ConnectionResult<bool> {
        let mut cmd = Command::new(&self.docker_path);

        cmd.arg("inspect")
            .arg("-f")
            .arg("{{.State.Running}}")
            .arg(&self.container)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            ConnectionError::DockerError(format!("Failed to inspect container: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim() == "true")
    }

    /// Get container info
    pub async fn container_info(&self) -> ConnectionResult<ContainerInfo> {
        let mut cmd = Command::new(&self.docker_path);

        cmd.arg("inspect")
            .arg("-f")
            .arg("{{.Id}}|{{.Name}}|{{.State.Running}}|{{.Config.Image}}")
            .arg(&self.container)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            ConnectionError::DockerError(format!("Failed to inspect container: {}", e))
        })?;

        if !output.status.success() {
            return Err(ConnectionError::DockerError(format!(
                "Container not found: {}",
                self.container
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = stdout.trim().split('|').collect();

        if parts.len() != 4 {
            return Err(ConnectionError::DockerError(
                "Invalid container info format".to_string(),
            ));
        }

        Ok(ContainerInfo {
            id: parts[0].to_string(),
            name: parts[1].trim_start_matches('/').to_string(),
            running: parts[2] == "true",
            image: parts[3].to_string(),
        })
    }
}

/// Container information
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Container ID
    pub id: String,
    /// Container name
    pub name: String,
    /// Whether container is running
    pub running: bool,
    /// Image name
    pub image: String,
}

#[async_trait]
impl Connection for DockerConnection {
    fn identifier(&self) -> &str {
        &self.container
    }

    async fn is_alive(&self) -> bool {
        self.is_container_running().await.unwrap_or(false)
    }

    async fn execute(
        &self,
        command: &str,
        options: Option<ExecuteOptions>,
    ) -> ConnectionResult<CommandResult> {
        let options = options.unwrap_or_default();

        // Verify container is running
        if !self.is_container_running().await? {
            return Err(ConnectionError::DockerError(format!(
                "Container {} is not running",
                self.container
            )));
        }

        debug!(
            container = %self.container,
            command = %command,
            "Executing command in Docker container"
        );

        let mut cmd = self.build_exec_command(command, &options);

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            ConnectionError::ExecutionFailed(format!("Failed to execute docker exec: {}", e))
        })?;

        // Wait for the process with optional timeout
        let output = if let Some(timeout_secs) = options.timeout {
            let timeout = tokio::time::Duration::from_secs(timeout_secs);
            let wait_future = child.wait_with_output();
            match tokio::time::timeout(timeout, wait_future).await {
                Ok(result) => result.map_err(|e| {
                    ConnectionError::ExecutionFailed(format!("Failed to wait for process: {}", e))
                })?,
                Err(_) => {
                    // Timeout occurred
                    return Err(ConnectionError::Timeout(timeout_secs));
                }
            }
        } else {
            child.wait_with_output().await.map_err(|e| {
                ConnectionError::ExecutionFailed(format!("Failed to wait for process: {}", e))
            })?
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        trace!(
            exit_code = %exit_code,
            stdout_len = %stdout.len(),
            stderr_len = %stderr.len(),
            "Docker exec completed"
        );

        if output.status.success() {
            Ok(CommandResult::success(stdout, stderr))
        } else {
            Ok(CommandResult::failure(exit_code, stdout, stderr))
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
            container = %self.container,
            "Uploading file to Docker container"
        );

        // Create parent directories if needed
        if options.create_dirs {
            if let Some(parent) = remote_path.parent() {
                let mkdir_cmd = format!("mkdir -p {}", parent.display());
                self.execute(&mkdir_cmd, None).await?;
            }
        }

        // Copy file to container
        let mut cmd = self.build_cp_to_container_command(local_path, remote_path);
        let output = cmd.output().await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to execute docker cp: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ConnectionError::TransferFailed(format!(
                "docker cp failed: {}",
                stderr
            )));
        }

        // Set permissions if specified
        if let Some(mode) = options.mode {
            let chmod_cmd = format!("chmod {:o} {}", mode, remote_path.display());
            self.execute(&chmod_cmd, None).await?;
        }

        // Set owner/group if specified
        if options.owner.is_some() || options.group.is_some() {
            let ownership = match (&options.owner, &options.group) {
                (Some(o), Some(g)) => format!("{}:{}", o, g),
                (Some(o), None) => o.to_string(),
                (None, Some(g)) => format!(":{}", g),
                (None, None) => return Ok(()),
            };

            let chown_cmd = format!("chown {} {}", ownership, remote_path.display());
            self.execute(&chown_cmd, None).await?;
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
            container = %self.container,
            size = %content.len(),
            "Uploading content to Docker container"
        );

        // Create a temporary file
        let temp_file = tempfile::NamedTempFile::new().map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to create temp file: {}", e))
        })?;

        // Write content to temp file
        std::fs::write(temp_file.path(), content).map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to write temp file: {}", e))
        })?;

        // Upload temp file
        self.upload(temp_file.path(), remote_path, Some(options))
            .await
    }

    async fn download(&self, remote_path: &Path, local_path: &Path) -> ConnectionResult<()> {
        debug!(
            remote = %remote_path.display(),
            local = %local_path.display(),
            container = %self.container,
            "Downloading file from Docker container"
        );

        // Create parent directories for local file
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ConnectionError::TransferFailed(format!("Failed to create local directory: {}", e))
            })?;
        }

        // Copy file from container
        let mut cmd = self.build_cp_from_container_command(remote_path, local_path);
        let output = cmd.output().await.map_err(|e| {
            ConnectionError::TransferFailed(format!("Failed to execute docker cp: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ConnectionError::TransferFailed(format!(
                "docker cp failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    async fn download_content(&self, remote_path: &Path) -> ConnectionResult<Vec<u8>> {
        debug!(
            remote = %remote_path.display(),
            container = %self.container,
            "Downloading content from Docker container"
        );

        // Use cat to read file content
        let command = format!("cat {}", remote_path.display());
        let result = self.execute(&command, None).await?;

        if !result.success {
            return Err(ConnectionError::TransferFailed(format!(
                "Failed to read file: {}",
                result.stderr
            )));
        }

        Ok(result.stdout.into_bytes())
    }

    async fn path_exists(&self, path: &Path) -> ConnectionResult<bool> {
        let command = format!("test -e {} && echo yes || echo no", path.display());
        let result = self.execute(&command, None).await?;
        Ok(result.stdout.trim() == "yes")
    }

    async fn is_directory(&self, path: &Path) -> ConnectionResult<bool> {
        let command = format!("test -d {} && echo yes || echo no", path.display());
        let result = self.execute(&command, None).await?;
        Ok(result.stdout.trim() == "yes")
    }

    async fn stat(&self, path: &Path) -> ConnectionResult<FileStat> {
        // Use stat command to get file info
        let command = format!(
            "stat -c '%s|%a|%u|%g|%X|%Y|%F' {}",
            path.display()
        );
        let result = self.execute(&command, None).await?;

        if !result.success {
            return Err(ConnectionError::TransferFailed(format!(
                "Failed to stat file: {}",
                result.stderr
            )));
        }

        let parts: Vec<&str> = result.stdout.trim().split('|').collect();
        if parts.len() != 7 {
            return Err(ConnectionError::TransferFailed(
                "Invalid stat output".to_string(),
            ));
        }

        let file_type = parts[6];

        Ok(FileStat {
            size: parts[0].parse().unwrap_or(0),
            mode: u32::from_str_radix(parts[1], 8).unwrap_or(0),
            uid: parts[2].parse().unwrap_or(0),
            gid: parts[3].parse().unwrap_or(0),
            atime: parts[4].parse().unwrap_or(0),
            mtime: parts[5].parse().unwrap_or(0),
            is_dir: file_type.contains("directory"),
            is_file: file_type.contains("regular"),
            is_symlink: file_type.contains("symbolic link"),
        })
    }

    async fn close(&self) -> ConnectionResult<()> {
        // Nothing to close for docker connection
        // The container continues running
        Ok(())
    }
}

/// Builder for Docker connections
pub struct DockerConnectionBuilder {
    container: Option<String>,
    docker_path: String,
    use_compose: bool,
    compose_service: Option<String>,
}

impl DockerConnectionBuilder {
    /// Create a new Docker connection builder
    pub fn new() -> Self {
        Self {
            container: None,
            docker_path: "docker".to_string(),
            use_compose: false,
            compose_service: None,
        }
    }

    /// Set the container ID or name
    pub fn container(mut self, container: impl Into<String>) -> Self {
        self.container = Some(container.into());
        self
    }

    /// Set the docker executable path
    pub fn docker_path(mut self, path: impl Into<String>) -> Self {
        self.docker_path = path.into();
        self
    }

    /// Use docker compose exec
    pub fn compose(mut self, service: impl Into<String>) -> Self {
        self.use_compose = true;
        self.compose_service = Some(service.into());
        self
    }

    /// Build the connection
    pub fn build(self) -> ConnectionResult<DockerConnection> {
        let container = self.container.ok_or_else(|| {
            ConnectionError::InvalidConfig("Container name or ID is required".to_string())
        })?;

        Ok(DockerConnection {
            container,
            docker_path: self.docker_path,
            use_compose: self.use_compose,
            compose_service: self.compose_service,
        })
    }
}

impl Default for DockerConnectionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// List running Docker containers
pub async fn list_containers() -> ConnectionResult<Vec<ContainerInfo>> {
    let mut cmd = Command::new("docker");

    cmd.arg("ps")
        .arg("--format")
        .arg("{{.ID}}|{{.Names}}|{{.Image}}")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().await.map_err(|e| {
        ConnectionError::DockerError(format!("Failed to list containers: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ConnectionError::DockerError(format!(
            "docker ps failed: {}",
            stderr
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut containers = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 3 {
            containers.push(ContainerInfo {
                id: parts[0].to_string(),
                name: parts[1].to_string(),
                running: true,
                image: parts[2].to_string(),
            });
        }
    }

    Ok(containers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_connection_new() {
        let conn = DockerConnection::new("my-container");
        assert_eq!(conn.container, "my-container");
        assert_eq!(conn.docker_path, "docker");
        assert!(!conn.use_compose);
    }

    #[test]
    fn test_docker_connection_compose() {
        let conn = DockerConnection::compose("web");
        assert!(conn.use_compose);
        assert_eq!(conn.compose_service, Some("web".to_string()));
    }

    #[test]
    fn test_docker_connection_builder() {
        let conn = DockerConnectionBuilder::new()
            .container("test-container")
            .docker_path("/usr/local/bin/docker")
            .build()
            .unwrap();

        assert_eq!(conn.container, "test-container");
        assert_eq!(conn.docker_path, "/usr/local/bin/docker");
    }

    #[test]
    fn test_docker_connection_builder_no_container() {
        let result = DockerConnectionBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_build_exec_command() {
        let conn = DockerConnection::new("my-container");
        let options = ExecuteOptions::default();

        // We can't easily test the command output, but we can verify it doesn't panic
        let _ = conn.build_exec_command("echo hello", &options);
    }

    #[test]
    fn test_build_exec_command_with_options() {
        let conn = DockerConnection::new("my-container");
        let options = ExecuteOptions::new()
            .with_cwd("/app")
            .with_env("FOO", "bar")
            .with_escalation(Some("root".to_string()));

        let _ = conn.build_exec_command("echo hello", &options);
    }
}
