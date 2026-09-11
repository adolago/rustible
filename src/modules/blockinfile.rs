//! Block-in-file module - Manage text blocks in files
//!
//! This module inserts, updates, or removes blocks of multi-line text
//! surrounded by customizable marker comments.

use super::{
    Diff, Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use crate::connection::{Connection, TransferOptions};
use crate::utils::secure_write_file;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Desired state for a block
#[derive(Debug, Clone, PartialEq)]
pub enum BlockState {
    Present,
    Absent,
}

impl BlockState {
    pub fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "present" => Ok(BlockState::Present),
            "absent" => Ok(BlockState::Absent),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: present, absent",
                s
            ))),
        }
    }
}

impl std::str::FromStr for BlockState {
    type Err = ModuleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        BlockState::from_str(s)
    }
}

/// Module for block-in-file operations
pub struct BlockinfileModule;

impl BlockinfileModule {
    /// Read file content into lines
    fn read_file(path: &Path) -> ModuleResult<Vec<String>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)?;
        Ok(content.lines().map(|s| s.to_string()).collect())
    }

    /// Write lines to file
    fn write_file(
        path: &Path,
        lines: &[String],
        create: bool,
        mode: Option<u32>,
    ) -> ModuleResult<()> {
        if !path.exists() && !create {
            return Err(ModuleError::ExecutionFailed(format!(
                "File '{}' does not exist and create=false",
                path.display()
            )));
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        let content = if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        };

        secure_write_file(path, &content, create, mode).map_err(|e| {
            ModuleError::ExecutionFailed(format!(
                "Failed to write file '{}': {}",
                path.display(),
                e
            ))
        })?;

        Ok(())
    }

    /// Create a backup of the file
    fn create_backup(path: &Path, suffix: &str) -> ModuleResult<Option<String>> {
        if path.exists() {
            let backup_path = format!("{}{}", path.display(), suffix);
            fs::copy(path, &backup_path)?;
            Ok(Some(backup_path))
        } else {
            Ok(None)
        }
    }

    /// Generate marker lines
    fn create_markers(marker: &str) -> (String, String) {
        let begin_marker = marker.replace("{mark}", "BEGIN");
        let end_marker = marker.replace("{mark}", "END");
        (begin_marker, end_marker)
    }

    /// Find the block boundaries in the file
    fn find_block(
        lines: &[String],
        begin_marker: &str,
        end_marker: &str,
    ) -> Option<(usize, usize)> {
        let mut begin_idx = None;
        let mut end_idx = None;

        for (i, line) in lines.iter().enumerate() {
            if line.contains(begin_marker) {
                begin_idx = Some(i);
            } else if line.contains(end_marker) && begin_idx.is_some() {
                end_idx = Some(i);
                break;
            }
        }

        match (begin_idx, end_idx) {
            (Some(start), Some(end)) if start < end => Some((start, end)),
            _ => None,
        }
    }

    /// Insert or update a block in the file
    fn ensure_block_present(
        lines: &mut Vec<String>,
        block: &str,
        begin_marker: &str,
        end_marker: &str,
        insertafter: Option<&str>,
        insertbefore: Option<&str>,
    ) -> ModuleResult<bool> {
        // Optimization: Avoid creating Vec<String> for block content initially.
        // We'll use iterators for comparison and insertion.

        // Check if block already exists
        if let Some((start, end)) = Self::find_block(lines, begin_marker, end_marker) {
            let current_block_slice = &lines[(start + 1)..end];

            // Check if content is the same efficiently
            // Compare slice of Strings with iterator of &str lines
            let mut matches = true;
            if current_block_slice.len() != block.lines().count() {
                matches = false;
            } else {
                for (line, block_line) in current_block_slice.iter().zip(block.lines()) {
                    if line != block_line {
                        matches = false;
                        break;
                    }
                }
            }

            if matches {
                return Ok(false); // No changes needed
            }

            // Replace the block content
            // We can splice directly from the iterator, mapping to String
            lines.splice((start + 1)..end, block.lines().map(|s| s.to_string()));
            return Ok(true);
        }

        // Block doesn't exist, insert it
        let insert_pos = if let Some(pattern) = insertbefore {
            match pattern.to_uppercase().as_str() {
                "BOF" => 0,
                _ => {
                    // Find first line matching pattern
                    lines
                        .iter()
                        .position(|l| l.contains(pattern))
                        .unwrap_or(lines.len())
                }
            }
        } else if let Some(pattern) = insertafter {
            match pattern.to_uppercase().as_str() {
                "EOF" => lines.len(),
                _ => {
                    // Find last line matching pattern
                    lines
                        .iter()
                        .rposition(|l| l.contains(pattern))
                        .map(|pos| pos + 1)
                        .unwrap_or(lines.len())
                }
            }
        } else {
            lines.len() // Default to EOF
        };

        // Build complete block with markers
        let mut new_block = vec![begin_marker.to_string()];
        new_block.extend(block.lines().map(|s| s.to_string()));
        new_block.push(end_marker.to_string());

        // Insert the block
        lines.splice(insert_pos..insert_pos, new_block);

        Ok(true)
    }

    /// Remove a block from the file
    fn ensure_block_absent(
        lines: &mut Vec<String>,
        begin_marker: &str,
        end_marker: &str,
    ) -> ModuleResult<bool> {
        if let Some((start, end)) = Self::find_block(lines, begin_marker, end_marker) {
            // Remove the entire block including markers
            lines.drain(start..=end);
            Ok(true)
        } else {
            Ok(false) // Block doesn't exist, no changes needed
        }
    }
}

impl BlockinfileModule {
    /// Manage a block in a file on a remote target.
    ///
    /// The file is downloaded, edited with the same helpers the local path
    /// uses, and uploaded again, so both paths agree on marker handling and
    /// insertion order.
    #[allow(clippy::too_many_arguments)]
    fn execute_remote(
        context: &ModuleContext,
        connection: &Arc<dyn Connection + Send + Sync>,
        path: &str,
        state: BlockState,
        block: Option<String>,
        begin_marker: String,
        end_marker: String,
        insertafter: Option<String>,
        insertbefore: Option<String>,
        create: bool,
        backup: bool,
        backup_suffix: String,
        mode: Option<u32>,
    ) -> ModuleResult<ModuleOutput> {
        let connection = connection.clone();
        let path = path.to_string();
        let check_mode = context.check_mode;
        let diff_mode = context.diff_mode;

        crate::modules::block_on_module_future(async move {
            let remote_path = Path::new(&path);
            let file_exists = connection.path_exists(remote_path).await.unwrap_or(false);

            if !file_exists && !create {
                return Err(ModuleError::ExecutionFailed(format!(
                    "File '{}' does not exist",
                    path
                )));
            }

            let file_bytes = if file_exists {
                connection
                    .download_content(remote_path)
                    .await
                    .map_err(|e| {
                        ModuleError::ExecutionFailed(format!("Failed to download file: {}", e))
                    })?
            } else {
                Vec::new()
            };

            let content = String::from_utf8_lossy(&file_bytes);
            let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
            let original_lines = diff_mode.then(|| lines.clone());

            let changed = match state {
                BlockState::Present => {
                    let block = block.ok_or_else(|| {
                        ModuleError::MissingParameter(
                            "block is required for state=present".to_string(),
                        )
                    })?;
                    Self::ensure_block_present(
                        &mut lines,
                        &block,
                        &begin_marker,
                        &end_marker,
                        insertafter.as_deref(),
                        insertbefore.as_deref(),
                    )?
                }
                BlockState::Absent => {
                    Self::ensure_block_absent(&mut lines, &begin_marker, &end_marker)?
                }
            };

            if !changed {
                if mode.is_some() {
                    // Uploading unchanged content to apply a mode would rewrite
                    // the file and move its mtime.
                    return Err(ModuleError::Unsupported(
                        "Remote mode-only updates are unsupported by blockinfile; no file was uploaded"
                            .to_string(),
                    ));
                }
                return Ok(ModuleOutput::ok(format!(
                    "File '{}' already has desired block state",
                    path
                )));
            }

            if check_mode {
                let mut output = ModuleOutput::changed(format!("Would modify '{}'", path));
                if let Some(original) = &original_lines {
                    output = output.with_diff(Diff::new(original.join("\n"), lines.join("\n")));
                }
                return Ok(output);
            }

            if backup && file_exists {
                let backup_path = format!("{}{}", path, backup_suffix);
                connection
                    .upload_content(&file_bytes, Path::new(&backup_path), None)
                    .await
                    .map_err(|e| {
                        ModuleError::ExecutionFailed(format!("Failed to create backup: {}", e))
                    })?;
            }

            let new_content = if lines.is_empty() {
                String::new()
            } else {
                format!("{}\n", lines.join("\n"))
            };

            let mut transfer = TransferOptions::new().with_create_dirs();
            if let Some(mode) = mode {
                transfer = transfer.with_mode(mode);
            }
            connection
                .upload_content(new_content.as_bytes(), remote_path, Some(transfer))
                .await
                .map_err(|e| {
                    ModuleError::ExecutionFailed(format!("Failed to upload file: {}", e))
                })?;

            let mut output = ModuleOutput::changed(format!("Modified '{}'", path));
            if let Some(original) = &original_lines {
                output = output.with_diff(Diff::new(original.join("\n"), lines.join("\n")));
            }
            if backup && file_exists {
                output = output.with_data(
                    "backup_file",
                    serde_json::json!(format!("{}{}", path, backup_suffix)),
                );
            }
            Ok(output)
        })?
    }
}

impl Module for BlockinfileModule {
    fn name(&self) -> &'static str {
        "blockinfile"
    }

    fn description(&self) -> &'static str {
        "Insert/update/remove a block of multi-line text surrounded by markers"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::NativeTransport
    }

    fn required_params(&self) -> &[&'static str] {
        &["path"]
    }

    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        let state = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());

        if state == "present" && params.get("block").is_none() {
            return Err(ModuleError::MissingParameter(
                "Parameter 'block' is required for state=present".to_string(),
            ));
        }

        Ok(())
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
            .unwrap_or_else(|| "present".to_string());
        let state = BlockState::from_str(&state_str)?;
        let block = params.get_string("block")?;
        let marker = params
            .get_string("marker")?
            .unwrap_or_else(|| "# {mark} ANSIBLE MANAGED BLOCK".to_string());
        let insertafter = params.get_string("insertafter")?;
        let insertbefore = params.get_string("insertbefore")?;
        let create = params.get_bool_or("create", false);
        let backup = params.get_bool_or("backup", false);
        let backup_suffix = params
            .get_string("backup_suffix")?
            .unwrap_or_else(|| "~".to_string());
        let mode = params.get_u32("mode")?;

        let (begin_marker, end_marker) = Self::create_markers(&marker);

        // A remote target is edited through the connection; the local path
        // below uses std::fs and would otherwise edit the control node.
        if let Some(connection) = &context.connection {
            if !connection.is_local() {
                return Self::execute_remote(
                    context,
                    connection,
                    &path_str,
                    state,
                    block,
                    begin_marker,
                    end_marker,
                    insertafter,
                    insertbefore,
                    create,
                    backup,
                    backup_suffix,
                    mode,
                );
            }
        }

        // Check if file exists
        if !path.exists() && !create {
            return Err(ModuleError::ExecutionFailed(format!(
                "File '{}' does not exist",
                path_str
            )));
        }

        // Read current content
        let mut lines = Self::read_file(path)?;
        // Only clone if diff mode needs it
        let original_lines = if context.diff_mode {
            Some(lines.clone())
        } else {
            None
        };

        // Apply changes based on state
        let content_changed = match state {
            BlockState::Present => {
                let block_str = block.as_ref().ok_or_else(|| {
                    ModuleError::MissingParameter("block is required for state=present".to_string())
                })?;

                Self::ensure_block_present(
                    &mut lines,
                    block_str,
                    &begin_marker,
                    &end_marker,
                    insertafter.as_deref(),
                    insertbefore.as_deref(),
                )?
            }
            BlockState::Absent => {
                Self::ensure_block_absent(&mut lines, &begin_marker, &end_marker)?
            }
        };

        #[cfg(unix)]
        let mode_changed = if let Some(desired) = mode.filter(|_| path.exists()) {
            use std::os::unix::fs::PermissionsExt;
            (fs::metadata(path)?.permissions().mode() & 0o7777) != (desired & 0o7777)
        } else {
            false
        };
        #[cfg(not(unix))]
        let mode_changed = false;

        if !content_changed && !mode_changed {
            return Ok(ModuleOutput::ok(format!(
                "File '{}' already has desired block state",
                path_str
            )));
        }

        // In check mode, don't actually write
        if context.check_mode {
            let diff = if context.diff_mode && content_changed {
                Some(Diff::new(
                    original_lines.as_ref().unwrap().join("\n"),
                    lines.join("\n"),
                ))
            } else {
                None
            };

            let mut output = ModuleOutput::changed(format!("Would modify '{}'", path_str));

            if let Some(d) = diff {
                output = output.with_diff(d);
            }

            return Ok(output);
        }

        // Create backup if requested
        let backup_file = if backup && content_changed {
            Self::create_backup(path, &backup_suffix)?
        } else {
            None
        };

        if content_changed {
            Self::write_file(path, &lines, create, mode)?;
        } else {
            // A permission-only change must preserve the original bytes and mtime.
            #[cfg(unix)]
            if let Some(desired) = mode {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(path, fs::Permissions::from_mode(desired))?;
            }
        }

        let mut output = ModuleOutput::changed(format!("Modified '{}'", path_str));

        if let Some(backup_path) = backup_file {
            output = output.with_data("backup_file", serde_json::json!(backup_path));
        }

        if context.diff_mode && content_changed {
            output = output.with_diff(Diff::new(
                original_lines.as_ref().unwrap().join("\n"),
                lines.join("\n"),
            ));
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_blockinfile_insert() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\n").unwrap();

        let module = BlockinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert(
            "block".to_string(),
            serde_json::json!("block line 1\nblock line 2"),
        );
        params.insert(
            "marker".to_string(),
            serde_json::json!("# {mark} TEST BLOCK"),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# BEGIN TEST BLOCK"));
        assert!(content.contains("block line 1"));
        assert!(content.contains("block line 2"));
        assert!(content.contains("# END TEST BLOCK"));
    }

    #[test]
    fn test_blockinfile_idempotent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(
            &path,
            "line1\n# BEGIN TEST BLOCK\nblock line 1\nblock line 2\n# END TEST BLOCK\nline2\n",
        )
        .unwrap();

        let module = BlockinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert(
            "block".to_string(),
            serde_json::json!("block line 1\nblock line 2"),
        );
        params.insert(
            "marker".to_string(),
            serde_json::json!("# {mark} TEST BLOCK"),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
    }

    #[test]
    fn test_blockinfile_update() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(
            &path,
            "line1\n# BEGIN TEST BLOCK\nold content\n# END TEST BLOCK\nline2\n",
        )
        .unwrap();

        let module = BlockinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("block".to_string(), serde_json::json!("new content"));
        params.insert(
            "marker".to_string(),
            serde_json::json!("# {mark} TEST BLOCK"),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("new content"));
        assert!(!content.contains("old content"));
    }

    #[test]
    fn test_blockinfile_absent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(
            &path,
            "line1\n# BEGIN TEST BLOCK\nblock content\n# END TEST BLOCK\nline2\n",
        )
        .unwrap();

        let module = BlockinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("state".to_string(), serde_json::json!("absent"));
        params.insert(
            "marker".to_string(),
            serde_json::json!("# {mark} TEST BLOCK"),
        );

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("BEGIN TEST BLOCK"));
        assert!(!content.contains("block content"));
        assert!(!content.contains("END TEST BLOCK"));
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
    }

    #[test]
    fn test_blockinfile_create() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("new_file.txt");

        let module = BlockinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("block".to_string(), serde_json::json!("new block"));
        params.insert("create".to_string(), serde_json::json!(true));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("new block"));
    }

    #[test]
    fn test_blockinfile_insertafter() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let module = BlockinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("block".to_string(), serde_json::json!("block content"));
        params.insert("insertafter".to_string(), serde_json::json!("line1"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = content.lines().collect();

        // Find BEGIN marker position
        let begin_pos = lines.iter().position(|l| l.contains("BEGIN")).unwrap();
        // It should be after line1 (which is at index 0)
        assert!(begin_pos > 0);
    }

    #[test]
    fn test_blockinfile_check_mode() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\n").unwrap();

        let module = BlockinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("block".to_string(), serde_json::json!("block content"));

        let context = ModuleContext::default().with_check_mode(true);
        let result = module.check(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.msg.contains("Would modify"));

        // File should not be modified
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("block content"));
    }
}
