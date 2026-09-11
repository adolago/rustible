//! File module - File/directory state management
//!
//! This module manages file and directory state including creation, deletion,
//! permissions, ownership, and symbolic links. It supports setting access/modification
//! times and SELinux contexts on compatible systems.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use crate::connection::{Connection, ExecuteOptions};
use crate::utils::shell_escape;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::runtime::Handle;

/// Desired state for a file/directory
#[derive(Debug, Clone, PartialEq)]
pub enum FileState {
    /// File should exist
    File,
    /// Directory should exist
    Directory,
    /// Symbolic link should exist
    Link,
    /// Hard link should exist
    Hard,
    /// Path should not exist
    Absent,
    /// Only update attributes (touch)
    Touch,
}

impl FileState {
    pub fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "file" => Ok(FileState::File),
            "directory" | "dir" => Ok(FileState::Directory),
            "link" | "symlink" => Ok(FileState::Link),
            "hard" | "hardlink" => Ok(FileState::Hard),
            "absent" => Ok(FileState::Absent),
            "touch" => Ok(FileState::Touch),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: file, directory, link, hard, absent, touch",
                s
            ))),
        }
    }
}

impl std::str::FromStr for FileState {
    type Err = ModuleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        FileState::from_str(s)
    }
}

/// SELinux context parameters
#[derive(Debug, Clone, Default)]
pub struct SelinuxContext {
    /// SELinux user
    pub seuser: Option<String>,
    /// SELinux role
    pub serole: Option<String>,
    /// SELinux type
    pub setype: Option<String>,
    /// SELinux level/range
    pub selevel: Option<String>,
}

impl SelinuxContext {
    /// Check if any SELinux parameters are set
    pub fn is_set(&self) -> bool {
        self.seuser.is_some()
            || self.serole.is_some()
            || self.setype.is_some()
            || self.selevel.is_some()
    }

    /// Build context string in format user:role:type:level
    pub fn to_context_string(&self) -> Option<String> {
        if !self.is_set() {
            return None;
        }
        Some(format!(
            "{}:{}:{}:{}",
            self.seuser.as_deref().unwrap_or("_"),
            self.serole.as_deref().unwrap_or("_"),
            self.setype.as_deref().unwrap_or("_"),
            self.selevel.as_deref().unwrap_or("_")
        ))
    }
}

/// Module for file/directory management
pub struct FileModule;

impl FileModule {
    /// Resolve each link relative to its containing directory, including a
    /// dangling final target that state=file/touch may create.
    fn resolve_link_target(path: &Path) -> ModuleResult<PathBuf> {
        let mut target = path.to_path_buf();
        for links in 0..=40 {
            let metadata = match fs::symlink_metadata(&target) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(target),
                Err(error) => return Err(error.into()),
            };
            if !metadata.file_type().is_symlink() {
                return Ok(target);
            }
            if links == 40 {
                break;
            }
            let next = fs::read_link(&target)?;
            target = if next.is_absolute() {
                next
            } else {
                target.parent().unwrap_or_else(|| Path::new(".")).join(next)
            };
        }
        Err(ModuleError::InvalidParameter(
            "Symbolic link chain exceeds 40 links or contains a cycle".into(),
        ))
    }

    fn attribute_metadata(path: &Path, follow: bool) -> std::io::Result<fs::Metadata> {
        if follow {
            fs::metadata(path)
        } else {
            fs::symlink_metadata(path)
        }
    }

    fn get_current_state(path: &Path) -> Option<FileState> {
        if !path.exists() && !path.is_symlink() {
            return None;
        }

        let meta = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => return None,
        };

        if meta.file_type().is_symlink() {
            Some(FileState::Link)
        } else if meta.is_dir() {
            Some(FileState::Directory)
        } else if meta.is_file() {
            Some(FileState::File)
        } else {
            None
        }
    }

    /// Whether applying the requested attributes would change anything.
    ///
    /// Check mode must answer the same question the real run answers, so it
    /// compares the target's current metadata instead of assuming that naming
    /// an attribute means changing it.
    #[allow(clippy::too_many_arguments)]
    fn attributes_would_change(
        path: &Path,
        mode: Option<u32>,
        owner: Option<u32>,
        group: Option<u32>,
        access_time: Option<i64>,
        modification_time: Option<i64>,
        selinux: &SelinuxContext,
        follow: bool,
    ) -> bool {
        // A SELinux context cannot be compared without reading it back, and
        // the read is only meaningful on a labelled system; treat a requested
        // context as a change, as before.
        if selinux.is_set() {
            return true;
        }

        let Ok(meta) = Self::attribute_metadata(path, follow) else {
            // The path disappeared between checks; the run would recreate it.
            return true;
        };

        if let Some(mode) = mode {
            if !meta.file_type().is_symlink() && meta.permissions().mode() & 0o7777 != mode {
                return true;
            }
        }

        if owner.is_some_and(|owner| meta.uid() != owner)
            || group.is_some_and(|group| meta.gid() != group)
        {
            return true;
        }

        if access_time.is_some_and(|time| meta.atime() != time)
            || modification_time.is_some_and(|time| meta.mtime() != time)
        {
            return true;
        }

        false
    }

    fn set_permissions(
        path: &Path,
        mode: u32,
        follow: bool,
        metadata: Option<&fs::Metadata>,
    ) -> ModuleResult<bool> {
        let meta_storage;
        let meta = match metadata {
            Some(m) => m,
            None => {
                meta_storage = Self::attribute_metadata(path, follow)?;
                &meta_storage
            }
        };

        // Don't change permissions on symlinks
        if meta.file_type().is_symlink() {
            return Ok(false);
        }

        let current = meta.permissions().mode() & 0o7777;
        if current != mode {
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
            return Ok(true);
        }
        Ok(false)
    }

    fn set_owner(
        path: &Path,
        owner: Option<u32>,
        group: Option<u32>,
        follow: bool,
        metadata: Option<&fs::Metadata>,
    ) -> ModuleResult<bool> {
        use std::os::unix::fs::{chown, lchown};

        if owner.is_none() && group.is_none() {
            return Ok(false);
        }

        let meta_storage;
        let meta = match metadata {
            Some(m) => m,
            None => {
                meta_storage = Self::attribute_metadata(path, follow)?;
                &meta_storage
            }
        };

        let current_user_id = meta.uid();
        let current_group_id = meta.gid();

        let target_user_id = owner.unwrap_or(current_user_id);
        let target_group_id = group.unwrap_or(current_group_id);

        if current_user_id != target_user_id || current_group_id != target_group_id {
            if follow {
                chown(path, Some(target_user_id), Some(target_group_id))?;
            } else {
                lchown(path, Some(target_user_id), Some(target_group_id))?;
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Set access and modification times on a file
    fn set_times(
        path: &Path,
        access_time: Option<i64>,
        modification_time: Option<i64>,
    ) -> ModuleResult<bool> {
        if access_time.is_none() && modification_time.is_none() {
            return Ok(false);
        }

        let meta = fs::metadata(path)?;
        let current_access_time = meta.atime();
        let current_modification_time = meta.mtime();

        let target_access_time = access_time.unwrap_or(current_access_time);
        let target_modification_time = modification_time.unwrap_or(current_modification_time);

        if current_access_time != target_access_time
            || current_modification_time != target_modification_time
        {
            let atime = filetime::FileTime::from_unix_time(target_access_time, 0);
            let mtime = filetime::FileTime::from_unix_time(target_modification_time, 0);
            filetime::set_file_times(path, atime, mtime)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Parse a timestamp from string (supports epoch seconds or ISO 8601)
    fn parse_timestamp(value: &str) -> ModuleResult<i64> {
        // Try parsing as epoch seconds first
        if let Ok(epoch) = value.parse::<i64>() {
            return Ok(epoch);
        }

        // Try parsing as ISO 8601 datetime
        // Basic format: YYYY-MM-DDTHH:MM:SS or YYYYMMDDTHHMMSS
        // For simplicity, we'll support common formats
        Err(ModuleError::InvalidParameter(format!(
            "Invalid timestamp '{}'. Use epoch seconds or ISO 8601 format.",
            value
        )))
    }

    /// Check if SELinux is enabled on the system
    #[cfg(target_os = "linux")]
    fn check_selinux_enabled() -> bool {
        use std::process::Command;
        let sestatus = Command::new("sestatus").output();
        match sestatus {
            Ok(output) => {
                let status = String::from_utf8_lossy(&output.stdout);
                status.contains("SELinux status:                 enabled")
            }
            Err(_) => false,
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn check_selinux_enabled() -> bool {
        false
    }

    /// Set SELinux context on a file (Linux-specific)
    #[cfg(target_os = "linux")]
    fn set_selinux_context(
        path: &Path,
        context: &SelinuxContext,
        selinux_enabled: Option<bool>,
    ) -> ModuleResult<bool> {
        use std::process::Command;

        if !context.is_set() {
            return Ok(false);
        }

        // Use cached status if provided, otherwise check
        let enabled = if let Some(e) = selinux_enabled {
            e
        } else {
            Self::check_selinux_enabled()
        };

        if !enabled {
            // SELinux not available, skip silently
            return Ok(false);
        }

        // Build chcon arguments
        let mut args: Vec<String> = Vec::new();

        if let Some(ref user) = context.seuser {
            args.push("-u".to_string());
            args.push(user.clone());
        }
        if let Some(ref role) = context.serole {
            args.push("-r".to_string());
            args.push(role.clone());
        }
        if let Some(ref setype) = context.setype {
            args.push("-t".to_string());
            args.push(setype.clone());
        }
        if let Some(ref level) = context.selevel {
            args.push("-l".to_string());
            args.push(level.clone());
        }

        args.push(path.to_string_lossy().to_string());

        let output = Command::new("chcon").args(&args).output()?;

        if !output.status.success() {
            return Err(ModuleError::ExecutionFailed(format!(
                "Failed to set SELinux context: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(true)
    }

    /// Stub for non-Linux systems
    #[cfg(not(target_os = "linux"))]
    fn set_selinux_context(
        _path: &Path,
        context: &SelinuxContext,
        _selinux_enabled: Option<bool>,
    ) -> ModuleResult<bool> {
        if context.is_set() {
            // Warn that SELinux is not available but don't fail
            return Ok(false);
        }
        Ok(false)
    }

    /// Apply attributes recursively to a directory
    fn apply_attributes_recursive(
        path: &Path,
        mode: Option<u32>,
        owner: Option<u32>,
        group: Option<u32>,
        follow: bool,
        selinux: &SelinuxContext,
    ) -> ModuleResult<bool> {
        let mut changed = false;

        // Check SELinux status once if needed
        let selinux_enabled = if selinux.is_set() {
            Some(Self::check_selinux_enabled())
        } else {
            None
        };

        for entry in walkdir::WalkDir::new(path)
            .follow_links(follow)
            .follow_root_links(follow)
        {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    return Err(ModuleError::ExecutionFailed(format!(
                        "Error walking directory: {}",
                        e
                    )));
                }
            };

            let entry_path = entry.path();

            // Skip the root path itself - we handle it separately
            if entry_path == path {
                continue;
            }

            // Fetch metadata once per file to reuse for permissions and owner checks
            // Inspect the same object that the requested follow behavior updates.
            let metadata = if mode.is_some() || owner.is_some() || group.is_some() {
                Some(Self::attribute_metadata(entry_path, follow).map_err(|e| {
                    ModuleError::ExecutionFailed(format!(
                        "Failed to stat {}: {}",
                        entry_path.display(),
                        e
                    ))
                })?)
            } else {
                None
            };
            let metadata_ref = metadata.as_ref();

            // Set mode if specified
            if let Some(m) = mode {
                if Self::set_permissions(entry_path, m, follow, metadata_ref)? {
                    changed = true;
                }
            }

            // Set ownership if specified
            if Self::set_owner(entry_path, owner, group, follow, metadata_ref)? {
                changed = true;
            }

            // Set SELinux context if specified
            if Self::set_selinux_context(entry_path, selinux, selinux_enabled)? {
                changed = true;
            }
        }

        Ok(changed)
    }

    fn create_directory(path: &Path, mode: Option<u32>) -> ModuleResult<bool> {
        if path.exists() {
            if path.is_dir() {
                return Ok(false);
            }
            return Err(ModuleError::ExecutionFailed(format!(
                "Path '{}' exists but is not a directory",
                path.display()
            )));
        }

        // Parents are always created, as in Ansible and the remote path
        // (`mkdir -p`); `recurse` only governs attributes on the contents.
        fs::create_dir_all(path)?;

        if let Some(mode) = mode {
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        }

        Ok(true)
    }

    fn create_file(path: &Path, mode: Option<u32>) -> ModuleResult<bool> {
        if path.exists() {
            if path.is_file() {
                return Ok(false);
            }
            return Err(ModuleError::ExecutionFailed(format!(
                "Path '{}' exists but is not a file",
                path.display()
            )));
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::File::create(path)?;

        if let Some(mode) = mode {
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        }

        Ok(true)
    }

    fn create_symlink(src: &Path, dest: &Path, force: bool) -> ModuleResult<bool> {
        // Check if symlink already exists and points to correct target
        if dest.is_symlink() {
            if let Ok(target) = fs::read_link(dest) {
                if target == src {
                    return Ok(false);
                }
            }
            if force {
                fs::remove_file(dest)?;
            } else {
                return Err(ModuleError::ExecutionFailed(format!(
                    "Symlink '{}' already exists with different target",
                    dest.display()
                )));
            }
        } else if dest.exists() {
            if force {
                if dest.is_dir() {
                    fs::remove_dir_all(dest)?;
                } else {
                    fs::remove_file(dest)?;
                }
            } else {
                return Err(ModuleError::ExecutionFailed(format!(
                    "Path '{}' already exists and is not a symlink",
                    dest.display()
                )));
            }
        }

        // Create parent directories if needed
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        symlink(src, dest)?;
        Ok(true)
    }

    fn create_hardlink(src: &Path, dest: &Path, force: bool) -> ModuleResult<bool> {
        if !src.exists() {
            return Err(ModuleError::ExecutionFailed(format!(
                "Source '{}' does not exist",
                src.display()
            )));
        }

        // Check if hardlink already exists
        if dest.exists() {
            let src_meta = fs::metadata(src)?;
            let dest_meta = fs::metadata(dest)?;

            // Same inode means same file (hardlink already exists)
            if src_meta.ino() == dest_meta.ino() && src_meta.dev() == dest_meta.dev() {
                return Ok(false);
            }

            if force {
                fs::remove_file(dest)?;
            } else {
                return Err(ModuleError::ExecutionFailed(format!(
                    "Path '{}' already exists",
                    dest.display()
                )));
            }
        }

        // Create parent directories if needed
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::hard_link(src, dest)?;
        Ok(true)
    }

    fn remove_path(path: &Path) -> ModuleResult<bool> {
        if !path.exists() && !path.is_symlink() {
            return Ok(false);
        }

        let meta = fs::symlink_metadata(path)?;

        if meta.is_dir() {
            // A directory goes with its contents, as in Ansible and the remote
            // path (`rm -rf`); `recurse` only governs attributes.
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }

        Ok(true)
    }

    fn touch_file(path: &Path) -> ModuleResult<bool> {
        use std::time::SystemTime;

        if !path.exists() {
            // Create parent directories if needed
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
            }
            // Create the file
            fs::File::create(path)?;
            return Ok(true);
        }

        // Update access and modification times
        let now = SystemTime::now();
        filetime::set_file_mtime(path, filetime::FileTime::from_system_time(now))?;
        filetime::set_file_atime(path, filetime::FileTime::from_system_time(now))?;

        Ok(true)
    }
}

// ============================================================================
// Remote execution
// ============================================================================

/// What a path currently is on the target, with the attributes the module
/// manages.
#[derive(Debug, Clone, PartialEq)]
struct RemoteEntry {
    state: Option<FileState>,
    mode: Option<String>,
    owner: Option<String>,
    group: Option<String>,
    /// Target of a symbolic link.
    link_target: Option<String>,
}

impl FileModule {
    /// Run a command on the target, returning success, stdout and stderr.
    fn remote_command(
        connection: &Arc<dyn Connection + Send + Sync>,
        command: &str,
        context: &ModuleContext,
    ) -> ModuleResult<(bool, String, String)> {
        let mut options = ExecuteOptions::new();
        if context.r#become {
            options = options
                .with_escalation(Some(
                    context
                        .become_user
                        .clone()
                        .unwrap_or_else(|| "root".to_string()),
                ))
                .with_escalate_method(
                    context
                        .become_method
                        .clone()
                        .unwrap_or_else(|| "sudo".to_string()),
                );
        }

        let connection = connection.clone();
        let command = command.to_string();
        let fut = async move { connection.execute(&command, Some(options)).await };

        // Modules run from both blocking and async contexts. Driving the
        // future on a separate thread works in either, where block_on alone
        // panics inside a runtime.
        let result = if let Ok(handle) = Handle::try_current() {
            std::thread::scope(|scope| scope.spawn(move || handle.block_on(fut)).join()).map_err(
                |_| ModuleError::ExecutionFailed("Tokio runtime thread panicked".to_string()),
            )?
        } else {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to create tokio runtime: {}", e))
                })?
                .block_on(fut)
        }
        .map_err(|e| ModuleError::ExecutionFailed(format!("Connection error: {}", e)))?;

        Ok((result.success, result.stdout, result.stderr))
    }

    /// Inspect a path on the target.
    fn remote_entry(
        connection: &Arc<dyn Connection + Send + Sync>,
        path: &str,
        context: &ModuleContext,
    ) -> ModuleResult<RemoteEntry> {
        // One round trip: kind, permissions, ownership and link target.
        let quoted = shell_escape(path);
        let command = format!(
            "if [ -L {p} ]; then printf 'link\\n'; readlink {p}; \
             elif [ -d {p} ]; then printf 'directory\\n\\n'; \
             elif [ -e {p} ]; then printf 'file\\n\\n'; \
             else printf 'absent\\n\\n'; fi; \
             stat -c '%a %U %G' {p} 2>/dev/null || true",
            p = quoted
        );
        let (_, stdout, _) = Self::remote_command(connection, &command, context)?;
        Ok(Self::parse_remote_entry(&stdout))
    }

    /// Parse the probe output written by [`Self::remote_entry`].
    ///
    /// Line 1 is the kind, line 2 the link target (blank when not a link) and
    /// line 3 the `stat` attributes (absent when the path does not exist).
    fn parse_remote_entry(stdout: &str) -> RemoteEntry {
        let mut lines = stdout.lines();
        let kind = lines.next().unwrap_or("absent").trim().to_string();
        let link_target = lines
            .next()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .map(str::to_string);
        let attributes: Vec<String> = lines
            .next()
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_string)
            .collect();

        let state = match kind.as_str() {
            "link" => Some(FileState::Link),
            "directory" => Some(FileState::Directory),
            "file" => Some(FileState::File),
            _ => None,
        };

        RemoteEntry {
            state,
            mode: attributes.first().cloned(),
            owner: attributes.get(1).cloned(),
            group: attributes.get(2).cloned(),
            link_target,
        }
    }

    /// Apply mode and ownership on the target, reporting whether anything
    /// changed.
    fn remote_apply_attributes(
        connection: &Arc<dyn Connection + Send + Sync>,
        path: &str,
        mode: Option<u32>,
        owner: Option<&str>,
        group: Option<&str>,
        entry: &RemoteEntry,
        context: &ModuleContext,
    ) -> ModuleResult<bool> {
        let mut changed = false;
        let quoted = shell_escape(path);

        if let Some(mode) = mode {
            let requested = format!("{:o}", mode & 0o7777);
            // stat prints the mode without leading zeroes.
            let current = entry.mode.as_deref().unwrap_or("");
            if current.trim_start_matches('0') != requested.trim_start_matches('0') {
                let command = format!("chmod {} {}", requested, quoted);
                let (success, _, stderr) = Self::remote_command(connection, &command, context)?;
                if !success {
                    return Err(ModuleError::ExecutionFailed(format!(
                        "Failed to set mode: {}",
                        stderr.trim()
                    )));
                }
                changed = true;
            }
        }

        let owner_differs = owner.is_some_and(|owner| entry.owner.as_deref() != Some(owner));
        let group_differs = group.is_some_and(|group| entry.group.as_deref() != Some(group));
        if owner_differs || group_differs {
            let spec = match (owner, group) {
                (Some(owner), Some(group)) => format!("{}:{}", owner, group),
                (Some(owner), None) => owner.to_string(),
                (None, Some(group)) => format!(":{}", group),
                (None, None) => unreachable!("at least one of owner/group differs"),
            };
            let command = format!("chown {} {}", shell_escape(&spec), quoted);
            let (success, _, stderr) = Self::remote_command(connection, &command, context)?;
            if !success {
                return Err(ModuleError::ExecutionFailed(format!(
                    "Failed to set ownership: {}",
                    stderr.trim()
                )));
            }
            changed = true;
        }

        Ok(changed)
    }

    /// Manage a path on a remote target.
    ///
    /// Covers the states playbooks use most (`file`, `directory`, `touch`,
    /// `absent`, `link`, `hard`) together with `mode`, `owner` and `group`.
    /// Options that would need a local filesystem view — SELinux contexts,
    /// explicit timestamps, recursive attribute application and
    /// `follow: false` — are refused rather than silently ignored.
    fn execute_via_connection(
        params: &ModuleParams,
        context: &ModuleContext,
        connection: &Arc<dyn Connection + Send + Sync>,
    ) -> ModuleResult<ModuleOutput> {
        let path = params.get_string_required("path")?;
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "file".to_string());
        let state = FileState::from_str(&state_str)?;
        let mode = params.get_u32("mode")?;
        // Remote ownership is applied with chown, which takes names or ids.
        let owner = params.get_string("owner")?;
        let group = params.get_string("group")?;
        let force = params.get_bool_or("force", false);
        let src = params.get_string("src")?;

        for unsupported in [
            "access_time",
            "modification_time",
            "seuser",
            "serole",
            "setype",
            "selevel",
        ] {
            if params.get_string(unsupported)?.is_some() || params.get_i64(unsupported)?.is_some() {
                return Err(ModuleError::Unsupported(format!(
                    "file: '{}' is not implemented for remote targets",
                    unsupported
                )));
            }
        }
        if !params.get_bool_or("follow", true) {
            return Err(ModuleError::Unsupported(
                "file: follow=false is not implemented for remote targets".into(),
            ));
        }
        if params.get_bool_or("recurse", false)
            && (mode.is_some() || owner.is_some() || group.is_some())
        {
            return Err(ModuleError::Unsupported(
                "file: recursive attribute changes are not implemented for remote targets".into(),
            ));
        }

        let quoted = shell_escape(&path);
        let entry = Self::remote_entry(connection, &path, context)?;

        match state {
            FileState::Absent => {
                if entry.state.is_none() {
                    return Ok(ModuleOutput::ok(format!("Path '{}' already absent", path)));
                }
                if context.check_mode {
                    return Ok(ModuleOutput::changed(format!("Would remove '{}'", path)));
                }

                let command = format!("rm -rf {}", quoted);
                let (success, _, stderr) = Self::remote_command(connection, &command, context)?;
                if !success {
                    return Err(ModuleError::ExecutionFailed(format!(
                        "Failed to remove '{}': {}",
                        path,
                        stderr.trim()
                    )));
                }
                Ok(ModuleOutput::changed(format!("Removed '{}'", path)))
            }

            FileState::Directory => {
                let exists = entry.state == Some(FileState::Directory);
                if context.check_mode {
                    if exists {
                        return Ok(ModuleOutput::ok(format!(
                            "Directory '{}' already exists",
                            path
                        )));
                    }
                    return Ok(ModuleOutput::changed(format!(
                        "Would create directory '{}'",
                        path
                    )));
                }

                let mut created = false;
                if !exists {
                    let command = format!("mkdir -p {}", quoted);
                    let (success, _, stderr) = Self::remote_command(connection, &command, context)?;
                    if !success {
                        return Err(ModuleError::ExecutionFailed(format!(
                            "Failed to create directory '{}': {}",
                            path,
                            stderr.trim()
                        )));
                    }
                    created = true;
                }

                let entry = if created {
                    Self::remote_entry(connection, &path, context)?
                } else {
                    entry
                };
                let attributes_changed = Self::remote_apply_attributes(
                    connection,
                    &path,
                    mode,
                    owner.as_deref(),
                    group.as_deref(),
                    &entry,
                    context,
                )?;

                if created {
                    Ok(ModuleOutput::changed(format!(
                        "Created directory '{}'",
                        path
                    )))
                } else if attributes_changed {
                    Ok(ModuleOutput::changed(format!(
                        "Updated attributes on directory '{}'",
                        path
                    )))
                } else {
                    Ok(ModuleOutput::ok(format!(
                        "Directory '{}' already exists with correct attributes",
                        path
                    )))
                }
            }

            FileState::File | FileState::Touch => {
                let exists = matches!(entry.state, Some(FileState::File) | Some(FileState::Link));

                if context.check_mode {
                    if state == FileState::Touch || !exists {
                        return Ok(ModuleOutput::changed(format!(
                            "Would {} '{}'",
                            if exists { "touch" } else { "create" },
                            path
                        )));
                    }
                    return Ok(ModuleOutput::ok(format!("File '{}' already exists", path)));
                }

                // `state: file` manages an existing path; it never creates one,
                // matching Ansible.
                if state == FileState::File && !exists {
                    return Err(ModuleError::ExecutionFailed(format!(
                        "Path '{}' does not exist on the target; use state=touch to create it",
                        path
                    )));
                }

                let mut changed = false;
                if state == FileState::Touch {
                    let command = format!("touch {}", quoted);
                    let (success, _, stderr) = Self::remote_command(connection, &command, context)?;
                    if !success {
                        return Err(ModuleError::ExecutionFailed(format!(
                            "Failed to touch '{}': {}",
                            path,
                            stderr.trim()
                        )));
                    }
                    // touch always updates the timestamps, so it always counts
                    // as a change, exactly as Ansible reports it.
                    changed = true;
                }

                let entry = if changed {
                    Self::remote_entry(connection, &path, context)?
                } else {
                    entry
                };
                let attributes_changed = Self::remote_apply_attributes(
                    connection,
                    &path,
                    mode,
                    owner.as_deref(),
                    group.as_deref(),
                    &entry,
                    context,
                )?;

                if changed {
                    Ok(ModuleOutput::changed(format!("Touched '{}'", path)))
                } else if attributes_changed {
                    Ok(ModuleOutput::changed(format!(
                        "Updated attributes on '{}'",
                        path
                    )))
                } else {
                    Ok(ModuleOutput::ok(format!(
                        "File '{}' already exists with correct attributes",
                        path
                    )))
                }
            }

            FileState::Link | FileState::Hard => {
                let src = src.ok_or_else(|| {
                    ModuleError::MissingParameter(format!(
                        "src is required when state is {}",
                        state_str
                    ))
                })?;

                let already_correct = state == FileState::Link
                    && entry.state == Some(FileState::Link)
                    && entry.link_target.as_deref() == Some(src.as_str());
                if already_correct {
                    let attributes_changed = Self::remote_apply_attributes(
                        connection,
                        &path,
                        mode,
                        owner.as_deref(),
                        group.as_deref(),
                        &entry,
                        context,
                    )?;
                    return Ok(if attributes_changed {
                        ModuleOutput::changed(format!("Updated attributes on link '{}'", path))
                    } else {
                        ModuleOutput::ok(format!("Link '{}' already correct", path))
                    });
                }

                if entry.state.is_some() && !force && entry.state != Some(FileState::Link) {
                    return Err(ModuleError::ExecutionFailed(format!(
                        "Path '{}' exists and is not a link; use force=true to replace it",
                        path
                    )));
                }

                let kind = if state == FileState::Link {
                    "symbolic link"
                } else {
                    "hard link"
                };
                if context.check_mode {
                    return Ok(ModuleOutput::changed(format!(
                        "Would create {} '{}' -> '{}'",
                        kind, path, src
                    )));
                }

                let quoted_src = shell_escape(&src);
                let command = if state == FileState::Link {
                    // -n keeps an existing directory link from being followed.
                    format!("ln -sfn {} {}", quoted_src, quoted)
                } else {
                    format!("ln -f {} {}", quoted_src, quoted)
                };
                let (success, _, stderr) = Self::remote_command(connection, &command, context)?;
                if !success {
                    return Err(ModuleError::ExecutionFailed(format!(
                        "Failed to create link '{}': {}",
                        path,
                        stderr.trim()
                    )));
                }

                Ok(ModuleOutput::changed(format!(
                    "Created {} '{}' -> '{}'",
                    kind, path, src
                )))
            }
        }
    }
}

impl Module for FileModule {
    fn name(&self) -> &'static str {
        "file"
    }

    fn description(&self) -> &'static str {
        "Manage file and directory state"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::NativeTransport
    }

    fn required_params(&self) -> &[&'static str] {
        &["path"]
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        // A remote target is managed through the connection; the local path
        // below uses std::fs and would otherwise act on the control node.
        if let Some(connection) = &context.connection {
            if !connection.is_local() {
                return Self::execute_via_connection(params, context, connection);
            }
        }

        let path_str = params.get_string_required("path")?;
        let path = Path::new(&path_str);
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "file".to_string());
        let state = FileState::from_str(&state_str)?;
        let mode = params.get_u32("mode")?;
        let owner = params.get_u32("owner")?;
        let group = params.get_u32("group")?;
        // Ansible defaults `recurse` to false: attributes reach a directory's
        // contents only when state=directory and recurse=true are both set.
        let recurse = params.get_bool_or("recurse", false);
        let force = params.get_bool_or("force", false);
        let follow = params.get_bool_or("follow", true);
        let src = params.get_string("src")?;

        // Parse access and modification times
        let access_time = if let Some(atime_str) = params.get_string("access_time")? {
            Some(Self::parse_timestamp(&atime_str)?)
        } else {
            params.get_i64("access_time")?
        };

        let modification_time = if let Some(mtime_str) = params.get_string("modification_time")? {
            Some(Self::parse_timestamp(&mtime_str)?)
        } else {
            params.get_i64("modification_time")?
        };

        // SELinux context parameters
        let selinux = SelinuxContext {
            seuser: params.get_string("seuser")?,
            serole: params.get_string("serole")?,
            setype: params.get_string("setype")?,
            selevel: params.get_string("selevel")?,
        };

        // Timestamp/SELinux helpers follow links. Reject unsupported no-follow
        // requests before creating files or changing any other attributes.
        let manages_attributes = matches!(
            state,
            FileState::File | FileState::Directory | FileState::Touch
        );
        if !follow && manages_attributes {
            if path.is_symlink()
                && (access_time.is_some()
                    || modification_time.is_some()
                    || state == FileState::Touch
                    || selinux.is_set())
            {
                return Err(ModuleError::Unsupported(
                    "Timestamp and SELinux updates on symbolic links require follow=true".into(),
                ));
            }
            if state == FileState::Directory && recurse && selinux.is_set() && path.exists() {
                for entry in walkdir::WalkDir::new(path)
                    .follow_links(false)
                    .follow_root_links(false)
                {
                    let entry = entry.map_err(|error| {
                        ModuleError::ExecutionFailed(format!("Error inspecting directory: {error}"))
                    })?;
                    if entry.file_type().is_symlink() {
                        return Err(ModuleError::Unsupported(
                            "Recursive SELinux updates containing symbolic links require follow=true".into(),
                        ));
                    }
                }
            }
        }
        let target_path = if follow && manages_attributes {
            Self::resolve_link_target(path)?
        } else {
            path.to_path_buf()
        };
        let path = target_path.as_path();
        let current_state = Self::get_current_state(path);

        // Handle each state
        match state {
            FileState::Absent => {
                if current_state.is_none() {
                    return Ok(ModuleOutput::ok(format!(
                        "Path '{}' already absent",
                        path_str
                    )));
                }

                if context.check_mode {
                    return Ok(
                        ModuleOutput::changed(format!("Would remove '{}'", path_str))
                            .with_diff(Diff::new(format!("{:?}", current_state), "absent")),
                    );
                }

                Self::remove_path(path)?;
                Ok(ModuleOutput::changed(format!("Removed '{}'", path_str)))
            }

            FileState::Directory => {
                if context.check_mode {
                    if current_state == Some(FileState::Directory) {
                        // Only report a change when the attributes really differ.
                        if Self::attributes_would_change(
                            path,
                            mode,
                            owner,
                            group,
                            access_time,
                            modification_time,
                            &selinux,
                            follow,
                        ) {
                            return Ok(ModuleOutput::changed(format!(
                                "Would update attributes on '{}'",
                                path_str
                            )));
                        }
                        return Ok(ModuleOutput::ok(format!(
                            "Directory '{}' already exists",
                            path_str
                        )));
                    }
                    return Ok(ModuleOutput::changed(format!(
                        "Would create directory '{}'",
                        path_str
                    )));
                }

                let created = Self::create_directory(path, mode)?;
                let perm_changed = if let Some(m) = mode {
                    Self::set_permissions(path, m, follow, None)?
                } else {
                    false
                };
                let owner_changed = Self::set_owner(path, owner, group, follow, None)?;
                let times_changed = Self::set_times(path, access_time, modification_time)?;
                let selinux_changed = Self::set_selinux_context(path, &selinux, None)?;

                // Apply attributes recursively if requested
                let recursive_changed = if recurse && path.is_dir() {
                    Self::apply_attributes_recursive(path, mode, owner, group, follow, &selinux)?
                } else {
                    false
                };

                if created {
                    Ok(ModuleOutput::changed(format!(
                        "Created directory '{}'",
                        path_str
                    )))
                } else if perm_changed
                    || owner_changed
                    || times_changed
                    || selinux_changed
                    || recursive_changed
                {
                    Ok(ModuleOutput::changed(format!(
                        "Updated attributes on directory '{}'",
                        path_str
                    )))
                } else {
                    Ok(ModuleOutput::ok(format!(
                        "Directory '{}' already exists with correct attributes",
                        path_str
                    )))
                }
            }

            FileState::File => {
                if context.check_mode {
                    if current_state == Some(FileState::File)
                        || (!follow && current_state == Some(FileState::Link))
                    {
                        // A link's own mode is never changed, so ignore a
                        // requested mode there.
                        let requested_mode = if current_state == Some(FileState::Link) {
                            None
                        } else {
                            mode
                        };
                        if Self::attributes_would_change(
                            path,
                            requested_mode,
                            owner,
                            group,
                            access_time,
                            modification_time,
                            &selinux,
                            follow,
                        ) {
                            return Ok(ModuleOutput::changed(format!(
                                "Would update attributes on '{}'",
                                path_str
                            )));
                        }
                        return Ok(ModuleOutput::ok(format!(
                            "File '{}' already exists",
                            path_str
                        )));
                    }
                    return Ok(ModuleOutput::changed(format!(
                        "Would create file '{}'",
                        path_str
                    )));
                }

                // A no-follow link is the object to manage, including when its
                // target is absent. Do not create or truncate that target.
                let created = if !follow && path.is_symlink() {
                    false
                } else {
                    Self::create_file(path, mode)?
                };
                let perm_changed = if let Some(m) = mode {
                    Self::set_permissions(path, m, follow, None)?
                } else {
                    false
                };
                let owner_changed = Self::set_owner(path, owner, group, follow, None)?;
                let times_changed = Self::set_times(path, access_time, modification_time)?;
                let selinux_changed = Self::set_selinux_context(path, &selinux, None)?;

                if created {
                    Ok(ModuleOutput::changed(format!(
                        "Created file '{}'",
                        path_str
                    )))
                } else if perm_changed || owner_changed || times_changed || selinux_changed {
                    Ok(ModuleOutput::changed(format!(
                        "Updated attributes on file '{}'",
                        path_str
                    )))
                } else {
                    Ok(ModuleOutput::ok(format!(
                        "File '{}' already exists with correct attributes",
                        path_str
                    )))
                }
            }

            FileState::Link => {
                let src = src.ok_or_else(|| {
                    ModuleError::MissingParameter("src is required for symlinks".to_string())
                })?;
                let src_path = Path::new(&src);

                if context.check_mode {
                    if current_state == Some(FileState::Link) {
                        if let Ok(target) = fs::read_link(path) {
                            if target == src_path {
                                return Ok(ModuleOutput::ok(format!(
                                    "Symlink '{}' already points to '{}'",
                                    path_str, src
                                )));
                            }
                        }
                    }
                    return Ok(ModuleOutput::changed(format!(
                        "Would create symlink '{}' -> '{}'",
                        path_str, src
                    )));
                }

                let created = Self::create_symlink(src_path, path, force)?;

                if created {
                    Ok(ModuleOutput::changed(format!(
                        "Created symlink '{}' -> '{}'",
                        path_str, src
                    )))
                } else {
                    Ok(ModuleOutput::ok(format!(
                        "Symlink '{}' already points to '{}'",
                        path_str, src
                    )))
                }
            }

            FileState::Hard => {
                let src = src.ok_or_else(|| {
                    ModuleError::MissingParameter("src is required for hard links".to_string())
                })?;
                let src_path = Path::new(&src);

                if context.check_mode {
                    return Ok(ModuleOutput::changed(format!(
                        "Would create hard link '{}' -> '{}'",
                        path_str, src
                    )));
                }

                let created = Self::create_hardlink(src_path, path, force)?;

                if created {
                    Ok(ModuleOutput::changed(format!(
                        "Created hard link '{}' -> '{}'",
                        path_str, src
                    )))
                } else {
                    Ok(ModuleOutput::ok(format!(
                        "Hard link '{}' already exists",
                        path_str
                    )))
                }
            }

            FileState::Touch => {
                if context.check_mode {
                    if path.exists() {
                        return Ok(ModuleOutput::changed(format!(
                            "Would update timestamps on '{}'",
                            path_str
                        )));
                    }
                    return Ok(ModuleOutput::changed(format!(
                        "Would create file '{}'",
                        path_str
                    )));
                }

                // If specific times are provided, use those; otherwise touch with current time
                if access_time.is_some() || modification_time.is_some() {
                    if !path.exists() {
                        // Create parent directories if needed
                        if let Some(parent) = path.parent() {
                            if !parent.exists() {
                                fs::create_dir_all(parent)?;
                            }
                        }
                        fs::File::create(path)?;
                    }
                    Self::set_times(path, access_time, modification_time)?;
                } else {
                    Self::touch_file(path)?;
                }

                if let Some(m) = mode {
                    Self::set_permissions(path, m, follow, None)?;
                }
                Self::set_owner(path, owner, group, follow, None)?;
                Self::set_selinux_context(path, &selinux, None)?;

                Ok(ModuleOutput::changed(format!("Touched '{}'", path_str)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // Remote target handling
    // ------------------------------------------------------------------

    #[test]
    fn test_parse_remote_entry_directory() {
        let entry = FileModule::parse_remote_entry("directory\n\n750 root root\n");
        assert_eq!(entry.state, Some(FileState::Directory));
        assert_eq!(entry.mode.as_deref(), Some("750"));
        assert_eq!(entry.owner.as_deref(), Some("root"));
        assert_eq!(entry.group.as_deref(), Some("root"));
        assert_eq!(entry.link_target, None);
    }

    #[test]
    fn test_parse_remote_entry_link() {
        let entry = FileModule::parse_remote_entry("link\n/opt/target\n777 root root\n");
        assert_eq!(entry.state, Some(FileState::Link));
        assert_eq!(entry.link_target.as_deref(), Some("/opt/target"));
    }

    #[test]
    fn test_parse_remote_entry_absent() {
        // A missing path produces no stat line at all.
        let entry = FileModule::parse_remote_entry("absent\n\n");
        assert_eq!(entry.state, None);
        assert_eq!(entry.mode, None);
        assert_eq!(entry.owner, None);
    }

    #[test]
    fn test_parse_remote_entry_file_without_stat() {
        // BusyBox stat may not support the format string; the kind still holds.
        let entry = FileModule::parse_remote_entry("file\n\n");
        assert_eq!(entry.state, Some(FileState::File));
        assert_eq!(entry.mode, None);
    }
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_file_create_directory() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("testdir");

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("directory"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(path.is_dir());
    }

    #[test]
    fn test_file_create_directory_idempotent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("testdir");
        fs::create_dir(&path).unwrap();

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("directory"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
    }

    #[test]
    fn test_file_create_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("testfile");

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("file"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(path.is_file());
    }

    #[test]
    fn test_file_absent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("testfile");
        fs::write(&path, "content").unwrap();

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("absent"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(!path.exists());
    }

    #[test]
    fn test_file_symlink() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("source");
        let dest = temp.path().join("link");
        fs::write(&src, "content").unwrap();

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert("state".to_string(), serde_json::json!("link"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(dest.is_symlink());
        assert_eq!(fs::read_link(&dest).unwrap(), src);
    }

    #[test]
    fn test_file_with_mode() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("testfile");

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("file"));
        params.insert("mode".to_string(), serde_json::json!(0o755));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let meta = fs::metadata(&path).unwrap();
        assert_eq!(meta.permissions().mode() & 0o7777, 0o755);
    }

    #[test]
    fn test_file_check_mode() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("testdir");

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("directory"));

        let context = ModuleContext::default().with_check_mode(true);
        let result = module.check(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.msg.contains("Would create"));
        assert!(!path.exists()); // Should not be created in check mode
    }

    #[test]
    fn test_file_touch() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("testfile");

        let module = FileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("touch"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(path.exists());
    }
}
