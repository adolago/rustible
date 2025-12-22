//! File module - File/directory state management
//!
//! This module manages file and directory state including creation, deletion,
//! permissions, ownership, and symbolic links.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::Path;

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
    fn from_str(s: &str) -> ModuleResult<Self> {
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

/// Module for file/directory management
pub struct FileModule;

impl FileModule {
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

    fn set_permissions(path: &Path, mode: u32) -> ModuleResult<bool> {
        let meta = fs::symlink_metadata(path)?;

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

    fn set_owner(path: &Path, owner: Option<u32>, group: Option<u32>) -> ModuleResult<bool> {
        use std::os::unix::fs::chown;

        let meta = fs::symlink_metadata(path)?;
        let current_uid = meta.uid();
        let current_gid = meta.gid();

        let new_uid = owner.unwrap_or(current_uid);
        let new_gid = group.unwrap_or(current_gid);

        if current_uid != new_uid || current_gid != new_gid {
            chown(path, Some(new_uid), Some(new_gid))?;
            return Ok(true);
        }
        Ok(false)
    }

    fn create_directory(path: &Path, mode: Option<u32>, recurse: bool) -> ModuleResult<bool> {
        if path.exists() {
            if path.is_dir() {
                return Ok(false);
            } else {
                return Err(ModuleError::ExecutionFailed(format!(
                    "Path '{}' exists but is not a directory",
                    path.display()
                )));
            }
        }

        if recurse {
            fs::create_dir_all(path)?;
        } else {
            fs::create_dir(path)?;
        }

        if let Some(mode) = mode {
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        }

        Ok(true)
    }

    fn create_file(path: &Path, mode: Option<u32>) -> ModuleResult<bool> {
        if path.exists() {
            if path.is_file() {
                return Ok(false);
            } else {
                return Err(ModuleError::ExecutionFailed(format!(
                    "Path '{}' exists but is not a file",
                    path.display()
                )));
            }
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

    fn remove_path(path: &Path, recurse: bool) -> ModuleResult<bool> {
        if !path.exists() && !path.is_symlink() {
            return Ok(false);
        }

        let meta = fs::symlink_metadata(path)?;

        if meta.is_dir() {
            if recurse {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_dir(path)?;
            }
        } else {
            fs::remove_file(path)?;
        }

        Ok(true)
    }

    fn touch_file(path: &Path) -> ModuleResult<bool> {
        use std::time::SystemTime;

        if !path.exists() {
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
        let path_str = params.get_string_required("path")?;
        let path = Path::new(&path_str);
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "file".to_string());
        let state = FileState::from_str(&state_str)?;
        let mode = params.get_u32("mode")?;
        let owner = params.get_u32("owner")?;
        let group = params.get_u32("group")?;
        let recurse = params.get_bool_or("recurse", false);
        let force = params.get_bool_or("force", false);
        let src = params.get_string("src")?;

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

                Self::remove_path(path, recurse)?;
                Ok(ModuleOutput::changed(format!("Removed '{}'", path_str)))
            }

            FileState::Directory => {
                if context.check_mode {
                    if current_state == Some(FileState::Directory) {
                        // Check if permissions need changing
                        if mode.is_some() || owner.is_some() || group.is_some() {
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

                let created = Self::create_directory(path, mode, recurse)?;
                let perm_changed = if let Some(m) = mode {
                    Self::set_permissions(path, m)?
                } else {
                    false
                };
                let owner_changed = Self::set_owner(path, owner, group)?;

                if created {
                    Ok(ModuleOutput::changed(format!(
                        "Created directory '{}'",
                        path_str
                    )))
                } else if perm_changed || owner_changed {
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
                    if current_state == Some(FileState::File) {
                        if mode.is_some() || owner.is_some() || group.is_some() {
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

                let created = Self::create_file(path, mode)?;
                let perm_changed = if let Some(m) = mode {
                    Self::set_permissions(path, m)?
                } else {
                    false
                };
                let owner_changed = Self::set_owner(path, owner, group)?;

                if created {
                    Ok(ModuleOutput::changed(format!(
                        "Created file '{}'",
                        path_str
                    )))
                } else if perm_changed || owner_changed {
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

                Self::touch_file(path)?;

                if let Some(m) = mode {
                    Self::set_permissions(path, m)?;
                }
                Self::set_owner(path, owner, group)?;

                Ok(ModuleOutput::changed(format!("Touched '{}'", path_str)))
            }
        }
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let path_str = params.get_string_required("path")?;
        let path = Path::new(&path_str);
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "file".to_string());
        let state = FileState::from_str(&state_str)?;

        let current_state = Self::get_current_state(path);

        let before = match current_state {
            Some(FileState::File) => "file exists".to_string(),
            Some(FileState::Directory) => "directory exists".to_string(),
            Some(FileState::Link) => {
                if let Ok(target) = fs::read_link(path) {
                    format!("symlink -> {}", target.display())
                } else {
                    "symlink".to_string()
                }
            }
            Some(FileState::Hard) => "hard link".to_string(),
            None => "absent".to_string(),
            _ => "unknown".to_string(),
        };

        let after = match state {
            FileState::File => "file".to_string(),
            FileState::Directory => "directory".to_string(),
            FileState::Link => {
                if let Some(src) = params.get_string("src")? {
                    format!("symlink -> {}", src)
                } else {
                    "symlink".to_string()
                }
            }
            FileState::Hard => "hard link".to_string(),
            FileState::Absent => "absent".to_string(),
            FileState::Touch => "touched".to_string(),
        };

        if before == after {
            Ok(None)
        } else {
            Ok(Some(Diff::new(before, after)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
