//! Line-in-file module - Manage lines in text files
//!
//! This module ensures a particular line is in a file, or replaces an existing
//! line using a back-referenced regular expression.

use super::{
    Diff, Module, ModuleContext, ModuleError, ModuleOutput, ModuleParams, ModuleResult, ParamExt,
};
use regex::Regex;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Desired state for a line
#[derive(Debug, Clone, PartialEq)]
pub enum LineState {
    Present,
    Absent,
}

impl LineState {
    fn from_str(s: &str) -> ModuleResult<Self> {
        match s.to_lowercase().as_str() {
            "present" => Ok(LineState::Present),
            "absent" => Ok(LineState::Absent),
            _ => Err(ModuleError::InvalidParameter(format!(
                "Invalid state '{}'. Valid states: present, absent",
                s
            ))),
        }
    }
}

/// Where to insert a new line
#[derive(Debug, Clone, PartialEq)]
pub enum InsertPosition {
    /// After the line matching the regex
    AfterMatch,
    /// Before the line matching the regex
    BeforeMatch,
    /// At the beginning of file
    BeginningOfFile,
    /// At the end of file (default)
    EndOfFile,
}

/// Module for line-in-file operations
pub struct LineinfileModule;

impl LineinfileModule {
    fn read_file(path: &Path) -> ModuleResult<Vec<String>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)?;
        Ok(content.lines().map(|s| s.to_string()).collect())
    }

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

        fs::write(path, content)?;

        if let Some(mode) = mode {
            fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        }

        Ok(())
    }

    fn create_backup(path: &Path, suffix: &str) -> ModuleResult<Option<String>> {
        if path.exists() {
            let backup_path = format!("{}{}", path.display(), suffix);
            fs::copy(path, &backup_path)?;
            Ok(Some(backup_path))
        } else {
            Ok(None)
        }
    }

    fn find_insert_position(
        lines: &[String],
        insertafter: Option<&str>,
        insertbefore: Option<&str>,
    ) -> ModuleResult<(InsertPosition, Option<usize>)> {
        if let Some(pattern) = insertafter {
            match pattern.to_uppercase().as_str() {
                "EOF" => Ok((InsertPosition::EndOfFile, None)),
                "BOF" => Ok((InsertPosition::BeginningOfFile, None)),
                _ => {
                    let re = Regex::new(pattern).map_err(|e| {
                        ModuleError::InvalidParameter(format!("Invalid insertafter regex: {}", e))
                    })?;
                    let idx = lines.iter().rposition(|l| re.is_match(l));
                    Ok((InsertPosition::AfterMatch, idx))
                }
            }
        } else if let Some(pattern) = insertbefore {
            match pattern.to_uppercase().as_str() {
                "EOF" => Ok((InsertPosition::EndOfFile, None)),
                "BOF" => Ok((InsertPosition::BeginningOfFile, None)),
                _ => {
                    let re = Regex::new(pattern).map_err(|e| {
                        ModuleError::InvalidParameter(format!("Invalid insertbefore regex: {}", e))
                    })?;
                    let idx = lines.iter().position(|l| re.is_match(l));
                    Ok((InsertPosition::BeforeMatch, idx))
                }
            }
        } else {
            Ok((InsertPosition::EndOfFile, None))
        }
    }

    fn ensure_line_present(
        lines: &mut Vec<String>,
        line: &str,
        regexp: Option<&Regex>,
        insertafter: Option<&str>,
        insertbefore: Option<&str>,
        firstmatch: bool,
    ) -> ModuleResult<bool> {
        // Check if the exact line already exists
        if lines.iter().any(|l| l == line) {
            // If using regexp, check if it matches the same line
            if let Some(re) = regexp {
                if lines.iter().any(|l| l == line && re.is_match(l)) {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }

        // If we have a regexp, find and replace matching lines
        if let Some(re) = regexp {
            let matching_indices: Vec<usize> = lines
                .iter()
                .enumerate()
                .filter(|(_, l)| re.is_match(l))
                .map(|(i, _)| i)
                .collect();

            if !matching_indices.is_empty() {
                if firstmatch {
                    // Replace only the first match
                    lines[matching_indices[0]] = line.to_string();
                } else {
                    // Replace all matches
                    for &idx in &matching_indices {
                        lines[idx] = line.to_string();
                    }
                }
                return Ok(true);
            }
        }

        // No match found or no regexp - insert the line
        let (position, match_idx) = Self::find_insert_position(lines, insertafter, insertbefore)?;

        match position {
            InsertPosition::BeginningOfFile => {
                lines.insert(0, line.to_string());
            }
            InsertPosition::EndOfFile => {
                lines.push(line.to_string());
            }
            InsertPosition::AfterMatch => {
                if let Some(idx) = match_idx {
                    lines.insert(idx + 1, line.to_string());
                } else {
                    // Pattern not found - append to end
                    lines.push(line.to_string());
                }
            }
            InsertPosition::BeforeMatch => {
                if let Some(idx) = match_idx {
                    lines.insert(idx, line.to_string());
                } else {
                    // Pattern not found - append to end
                    lines.push(line.to_string());
                }
            }
        }

        Ok(true)
    }

    fn ensure_line_absent(
        lines: &mut Vec<String>,
        line: Option<&str>,
        regexp: Option<&Regex>,
    ) -> ModuleResult<bool> {
        let original_len = lines.len();

        lines.retain(|l| {
            if let Some(re) = regexp {
                !re.is_match(l)
            } else if let Some(line_str) = line {
                l != line_str
            } else {
                true
            }
        });

        Ok(lines.len() != original_len)
    }

    fn apply_backrefs(line: &str, regexp: &Regex, original: &str) -> String {
        if let Some(captures) = regexp.captures(original) {
            let mut result = line.to_string();

            // Replace \1, \2, etc. with captured groups
            for i in 0..captures.len() {
                if let Some(m) = captures.get(i) {
                    result = result.replace(&format!("\\{}", i), m.as_str());
                }
            }

            result
        } else {
            line.to_string()
        }
    }
}

impl Module for LineinfileModule {
    fn name(&self) -> &'static str {
        "lineinfile"
    }

    fn description(&self) -> &'static str {
        "Ensure a particular line is in a file"
    }

    fn required_params(&self) -> &[&'static str] {
        &["path"]
    }

    fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
        let state = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());

        if state == "present" {
            if params.get("line").is_none() && params.get("regexp").is_none() {
                return Err(ModuleError::MissingParameter(
                    "Either 'line' or 'regexp' is required for state=present".to_string(),
                ));
            }
        } else if state == "absent" {
            if params.get("line").is_none() && params.get("regexp").is_none() {
                return Err(ModuleError::MissingParameter(
                    "Either 'line' or 'regexp' is required for state=absent".to_string(),
                ));
            }
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
        let state = LineState::from_str(&state_str)?;
        let line = params.get_string("line")?;
        let regexp_str = params.get_string("regexp")?;
        let insertafter = params.get_string("insertafter")?;
        let insertbefore = params.get_string("insertbefore")?;
        let create = params.get_bool_or("create", false);
        let backup = params.get_bool_or("backup", false);
        let backup_suffix = params
            .get_string("backup_suffix")?
            .unwrap_or_else(|| "~".to_string());
        let firstmatch = params.get_bool_or("firstmatch", false);
        let backrefs = params.get_bool_or("backrefs", false);
        let mode = params.get_u32("mode")?;

        // Compile regexp if provided
        let regexp = if let Some(ref re_str) = regexp_str {
            Some(
                Regex::new(re_str)
                    .map_err(|e| ModuleError::InvalidParameter(format!("Invalid regexp: {}", e)))?,
            )
        } else {
            None
        };

        // Check if file exists
        if !path.exists() && !create {
            return Err(ModuleError::ExecutionFailed(format!(
                "File '{}' does not exist",
                path_str
            )));
        }

        // Read current content
        let mut lines = Self::read_file(path)?;
        let original_lines = lines.clone();

        // Apply changes based on state
        let changed = match state {
            LineState::Present => {
                let line_str = line.as_ref().ok_or_else(|| {
                    ModuleError::MissingParameter("line is required for state=present".to_string())
                })?;

                // Handle backrefs
                let final_line = if backrefs {
                    if let Some(ref re) = regexp {
                        // Find the matching line and apply backrefs
                        let matching_line = lines.iter().find(|l| re.is_match(l));
                        if let Some(orig) = matching_line {
                            Self::apply_backrefs(line_str, re, orig)
                        } else {
                            // No match - line won't be added when using backrefs
                            return Ok(ModuleOutput::ok(format!(
                                "No match for regexp in '{}'",
                                path_str
                            )));
                        }
                    } else {
                        line_str.clone()
                    }
                } else {
                    line_str.clone()
                };

                Self::ensure_line_present(
                    &mut lines,
                    &final_line,
                    regexp.as_ref(),
                    insertafter.as_deref(),
                    insertbefore.as_deref(),
                    firstmatch,
                )?
            }
            LineState::Absent => {
                Self::ensure_line_absent(&mut lines, line.as_deref(), regexp.as_ref())?
            }
        };

        if !changed {
            return Ok(ModuleOutput::ok(format!(
                "File '{}' already has desired content",
                path_str
            )));
        }

        // In check mode, don't actually write
        if context.check_mode {
            let diff = if context.diff_mode {
                Some(Diff::new(original_lines.join("\n"), lines.join("\n")))
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
        let backup_file = if backup {
            Self::create_backup(path, &backup_suffix)?
        } else {
            None
        };

        // Write the file
        Self::write_file(path, &lines, create, mode)?;

        let mut output = ModuleOutput::changed(format!("Modified '{}'", path_str));

        if let Some(backup_path) = backup_file {
            output = output.with_data("backup_file", serde_json::json!(backup_path));
        }

        if context.diff_mode {
            output = output.with_diff(Diff::new(original_lines.join("\n"), lines.join("\n")));
        }

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
        let path_str = params.get_string_required("path")?;
        let path = Path::new(&path_str);
        let state_str = params
            .get_string("state")?
            .unwrap_or_else(|| "present".to_string());
        let state = LineState::from_str(&state_str)?;
        let line = params.get_string("line")?;
        let regexp_str = params.get_string("regexp")?;
        let insertafter = params.get_string("insertafter")?;
        let insertbefore = params.get_string("insertbefore")?;
        let firstmatch = params.get_bool_or("firstmatch", false);

        let regexp = if let Some(ref re_str) = regexp_str {
            Some(
                Regex::new(re_str)
                    .map_err(|e| ModuleError::InvalidParameter(format!("Invalid regexp: {}", e)))?,
            )
        } else {
            None
        };

        let mut lines = Self::read_file(path)?;
        let original_lines = lines.clone();

        let changed = match state {
            LineState::Present => {
                if let Some(line_str) = line {
                    Self::ensure_line_present(
                        &mut lines,
                        &line_str,
                        regexp.as_ref(),
                        insertafter.as_deref(),
                        insertbefore.as_deref(),
                        firstmatch,
                    )?
                } else {
                    false
                }
            }
            LineState::Absent => {
                Self::ensure_line_absent(&mut lines, line.as_deref(), regexp.as_ref())?
            }
        };

        if changed {
            Ok(Some(Diff::new(original_lines.join("\n"), lines.join("\n"))))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_lineinfile_add_line() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\n").unwrap();

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("line".to_string(), serde_json::json!("line3"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("line3"));
    }

    #[test]
    fn test_lineinfile_idempotent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("line".to_string(), serde_json::json!("line2"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
    }

    #[test]
    fn test_lineinfile_regexp_replace() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "key=old_value\nother=stuff\n").unwrap();

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("regexp".to_string(), serde_json::json!("^key="));
        params.insert("line".to_string(), serde_json::json!("key=new_value"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("key=new_value"));
        assert!(!content.contains("key=old_value"));
    }

    #[test]
    fn test_lineinfile_absent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("line".to_string(), serde_json::json!("line2"));
        params.insert("state".to_string(), serde_json::json!("absent"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("line2"));
    }

    #[test]
    fn test_lineinfile_insertafter() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("line".to_string(), serde_json::json!("new_line"));
        params.insert("insertafter".to_string(), serde_json::json!("^line1"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let lines: Vec<_> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(String::from)
            .collect();
        assert_eq!(lines[1], "new_line");
    }

    #[test]
    fn test_lineinfile_insertbefore() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("line".to_string(), serde_json::json!("new_line"));
        params.insert("insertbefore".to_string(), serde_json::json!("^line3"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let lines: Vec<_> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(String::from)
            .collect();
        assert_eq!(lines[2], "new_line");
    }

    #[test]
    fn test_lineinfile_create() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("new_file.txt");

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("line".to_string(), serde_json::json!("new_line"));
        params.insert("create".to_string(), serde_json::json!(true));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("new_line"));
    }

    #[test]
    fn test_lineinfile_check_mode() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "line1\nline2\n").unwrap();

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("line".to_string(), serde_json::json!("line3"));

        let context = ModuleContext::default().with_check_mode(true);
        let result = module.check(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.msg.contains("Would modify"));

        // File should not be modified
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("line3"));
    }

    #[test]
    fn test_lineinfile_regexp_absent() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.txt");
        fs::write(&path, "# comment1\nkey=value\n# comment2\n").unwrap();

        let module = LineinfileModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::json!(path.to_str().unwrap()),
        );
        params.insert("regexp".to_string(), serde_json::json!("^#"));
        params.insert("state".to_string(), serde_json::json!("absent"));

        let context = ModuleContext::default();
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("#"));
        assert!(content.contains("key=value"));
    }
}
