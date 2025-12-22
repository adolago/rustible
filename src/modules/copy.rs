//! Copy module - Copy files to destination
//!
//! This module copies files from a source to a destination, with support for
//! permissions, ownership, and backup creation.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

/// Module for copying files
pub struct CopyModule;

impl CopyModule {
    fn get_file_checksum(path: &Path) -> std::io::Result<String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut file = fs::File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        let mut hasher = DefaultHasher::new();
        contents.hash(&mut hasher);
        Ok(format!("{:x}", hasher.finish()))
    }

    fn create_backup(dest: &Path, backup_suffix: &str) -> ModuleResult<Option<String>> {
        if dest.exists() {
            let backup_path = format!("{}{}", dest.display(), backup_suffix);
            fs::copy(dest, &backup_path)?;
            Ok(Some(backup_path))
        } else {
            Ok(None)
        }
    }

    fn set_permissions(path: &Path, mode: Option<u32>) -> ModuleResult<bool> {
        if let Some(mode) = mode {
            let current = fs::metadata(path)?.permissions().mode() & 0o7777;
            if current != mode {
                fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn files_differ(src: &Path, dest: &Path) -> std::io::Result<bool> {
        if !dest.exists() {
            return Ok(true);
        }

        let src_meta = fs::metadata(src)?;
        let dest_meta = fs::metadata(dest)?;

        // Quick check: different sizes means different content
        if src_meta.len() != dest_meta.len() {
            return Ok(true);
        }

        // Compare checksums
        let src_checksum = Self::get_file_checksum(src)?;
        let dest_checksum = Self::get_file_checksum(dest)?;

        Ok(src_checksum != dest_checksum)
    }

    fn copy_content(content: &str, dest: &Path) -> ModuleResult<()> {
        let mut file = fs::File::create(dest)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    fn copy_file(src: &Path, dest: &Path, force: bool) -> ModuleResult<()> {
        if dest.exists() && !force {
            let dest_meta = fs::metadata(dest)?;
            if dest_meta.permissions().readonly() {
                return Err(ModuleError::PermissionDenied(format!(
                    "Destination '{}' is read-only and force is not set",
                    dest.display()
                )));
            }
        }

        fs::copy(src, dest)?;
        Ok(())
    }
}

impl Module for CopyModule {
    fn name(&self) -> &'static str {
        "copy"
    }

    fn description(&self) -> &'static str {
        "Copy files to a destination"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::NativeTransport
    }

    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        // Must have either src or content
        if params.get("src").is_none() && params.get("content").is_none() {
            return Err(ModuleError::MissingParameter(
                "Either 'src' or 'content' must be provided".to_string(),
            ));
        }

        // Must have dest
        if params.get("dest").is_none() {
            return Err(ModuleError::MissingParameter("dest".to_string()));
        }

        Ok(())
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let dest = params.get_string_required("dest")?;
        let dest_path = Path::new(&dest);
        let src = params.get_string("src")?;
        let content = params.get_string("content")?;
        let force = params.get_bool_or("force", true);
        let backup = params.get_bool_or("backup", false);
        let backup_suffix = params
            .get_string("backup_suffix")?
            .unwrap_or_else(|| "~".to_string());
        let mode = params.get_u32("mode")?;

        // Determine if we're copying from src or content
        let (source_content, src_display) = if let Some(ref content_str) = content {
            (Some(content_str.clone()), "(content)".to_string())
        } else if let Some(ref src_str) = src {
            let src_path = Path::new(src_str);
            if !src_path.exists() {
                return Err(ModuleError::ExecutionFailed(format!(
                    "Source file '{}' does not exist",
                    src_str
                )));
            }
            (None, src_str.clone())
        } else {
            return Err(ModuleError::MissingParameter(
                "Either 'src' or 'content' must be provided".to_string(),
            ));
        };

        // Check if dest is a directory
        let final_dest = if dest_path.is_dir() {
            if let Some(ref src_str) = src {
                let src_path = Path::new(src_str);
                dest_path.join(src_path.file_name().ok_or_else(|| {
                    ModuleError::InvalidParameter(
                        "Cannot determine filename from source".to_string(),
                    )
                })?)
            } else {
                return Err(ModuleError::InvalidParameter(
                    "Cannot copy content to a directory without specifying filename".to_string(),
                ));
            }
        } else {
            dest_path.to_path_buf()
        };

        // Check if copy is needed
        let needs_copy = if let Some(ref src_str) = src {
            let src_path = Path::new(src_str);
            Self::files_differ(src_path, &final_dest)?
        } else {
            // For content, always check
            if final_dest.exists() {
                let mut existing = String::new();
                fs::File::open(&final_dest)?.read_to_string(&mut existing)?;
                existing != source_content.as_ref().unwrap().as_str()
            } else {
                true
            }
        };

        if !needs_copy {
            // Check if only permissions need updating
            let perm_changed = if let Some(m) = mode {
                if final_dest.exists() {
                    let current = fs::metadata(&final_dest)?.permissions().mode() & 0o7777;
                    current != m
                } else {
                    false
                }
            } else {
                false
            };

            if perm_changed {
                if context.check_mode {
                    return Ok(ModuleOutput::changed(format!(
                        "Would change permissions on '{}'",
                        final_dest.display()
                    )));
                }
                Self::set_permissions(&final_dest, mode)?;
                return Ok(ModuleOutput::changed(format!(
                    "Changed permissions on '{}'",
                    final_dest.display()
                )));
            }

            return Ok(ModuleOutput::ok(format!(
                "File '{}' is already up to date",
                final_dest.display()
            )));
        }

        // In check mode, return what would happen
        if context.check_mode {
            let diff = if context.diff_mode {
                if let Some(ref content_str) = source_content {
                    let before = if final_dest.exists() {
                        fs::read_to_string(&final_dest).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    Some(Diff::new(before, content_str.clone()))
                } else {
                    Some(Diff::new(
                        format!("(current state of {})", final_dest.display()),
                        format!("(contents of {})", src_display),
                    ))
                }
            } else {
                None
            };

            let mut output = ModuleOutput::changed(format!(
                "Would copy {} to '{}'",
                src_display,
                final_dest.display()
            ));

            if let Some(d) = diff {
                output = output.with_diff(d);
            }

            return Ok(output);
        }

        // Create backup if requested
        let backup_file = if backup {
            Self::create_backup(&final_dest, &backup_suffix)?
        } else {
            None
        };

        // Create parent directories if needed
        if let Some(parent) = final_dest.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        // Perform the copy
        if let Some(ref content_str) = source_content {
            Self::copy_content(content_str, &final_dest)?;
        } else if let Some(ref src_str) = src {
            let src_path = Path::new(src_str);
            Self::copy_file(src_path, &final_dest, force)?;
        }

        // Set permissions
        let perm_changed = Self::set_permissions(&final_dest, mode)?;

        let mut output = ModuleOutput::changed(format!(
            "Copied {} to '{}'",
            src_display,
            final_dest.display()
        ));

        if let Some(backup_path) = backup_file {
            output = output.with_data("backup_file", serde_json::json!(backup_path));
        }

        if perm_changed {
            output = output.with_data("mode_changed", serde_json::json!(true));
        }

        // Add file info to output
        let meta = fs::metadata(&final_dest)?;
        output = output
            .with_data("dest", serde_json::json!(final_dest.to_string_lossy()))
            .with_data("size", serde_json::json!(meta.len()))
            .with_data(
                "mode",
                serde_json::json!(format!("{:o}", meta.permissions().mode() & 0o7777)),
            )
            .with_data("uid", serde_json::json!(meta.uid()))
            .with_data("gid", serde_json::json!(meta.gid()));

        Ok(output)
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    fn diff(&self, params: &ModuleParams, _context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let dest = params.get_string_required("dest")?;
        let dest_path = Path::new(&dest);
        let content = params.get_string("content")?;

        if let Some(content_str) = content {
            let before = if dest_path.exists() {
                fs::read_to_string(dest_path).unwrap_or_default()
            } else {
                String::new()
            };
            return Ok(Some(Diff::new(before, content_str)));
        }

        let src = params.get_string("src")?;
        if let Some(src_str) = src {
            let src_path = Path::new(&src_str);
            if src_path.exists() {
                let src_content =
                    fs::read_to_string(src_path).unwrap_or_else(|_| "(binary file)".to_string());
                let dest_content = if dest_path.exists() {
                    fs::read_to_string(dest_path).unwrap_or_else(|_| "(binary file)".to_string())
                } else {
                    String::new()
                };
                return Ok(Some(Diff::new(dest_content, src_content)));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_copy_content() {
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join("test.txt");

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("content".to_string(), serde_json::json!("Hello, World!"));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(dest.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "Hello, World!");
    }

    #[test]
    fn test_copy_file() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("source.txt");
        let dest = temp.path().join("dest.txt");

        fs::write(&src, "Source content").unwrap();

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(dest.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "Source content");
    }

    #[test]
    fn test_copy_idempotent() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("source.txt");
        let dest = temp.path().join("dest.txt");

        fs::write(&src, "Same content").unwrap();
        fs::write(&dest, "Same content").unwrap();

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
    }

    #[test]
    fn test_copy_with_mode() {
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join("test.txt");

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("content".to_string(), serde_json::json!("Hello"));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );
        params.insert("mode".to_string(), serde_json::json!(0o755));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let meta = fs::metadata(&dest).unwrap();
        assert_eq!(meta.permissions().mode() & 0o7777, 0o755);
    }

    #[test]
    fn test_copy_check_mode() {
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join("test.txt");

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("content".to_string(), serde_json::json!("Hello"));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let context = ModuleContext::default().with_check_mode(true);
        let result = module.check(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.msg.contains("Would copy"));
        assert!(!dest.exists()); // File should not be created in check mode
    }

    #[test]
    fn test_copy_with_backup() {
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join("test.txt");

        // Create existing file
        fs::write(&dest, "Old content").unwrap();

        let module = CopyModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("content".to_string(), serde_json::json!("New content"));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );
        params.insert("backup".to_string(), serde_json::json!(true));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.data.contains_key("backup_file"));

        let backup_path = temp.path().join("test.txt~");
        assert!(backup_path.exists());
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "Old content");
    }
}
