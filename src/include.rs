//! Task and variable inclusion system
//!
//! This module provides support for:
//! - include_tasks: Dynamic task inclusion with separate variable scope
//! - import_tasks: Static task inclusion merged at parse time
//! - include_vars: Loading variables from files during execution

use crate::error::{Error, Result};
use crate::playbook::Task;
use crate::vars::{VarPrecedence, VarStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Security error for path traversal attempts
#[derive(Debug, Clone)]
pub struct PathTraversalError {
    /// The requested path that violated security constraints
    pub requested_path: PathBuf,
    /// The base directory that should contain all includes
    pub base_path: PathBuf,
}

impl std::fmt::Display for PathTraversalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Path traversal detected: '{}' escapes base directory '{}'",
            self.requested_path.display(),
            self.base_path.display()
        )
    }
}

impl std::error::Error for PathTraversalError {}

/// Specification for including tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncludeTasksSpec {
    /// Path to the file containing tasks
    pub file: String,

    /// Variables to pass to the included tasks
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub vars: HashMap<String, serde_json::Value>,

    /// Whether to apply parent tags to included tasks
    #[serde(default = "default_true")]
    pub apply_tags: bool,
}

fn default_true() -> bool {
    true
}

impl IncludeTasksSpec {
    /// Create a new include_tasks specification
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            vars: HashMap::new(),
            apply_tags: true,
        }
    }

    /// Add a variable to pass to included tasks
    pub fn with_var(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.vars.insert(key.into(), value);
        self
    }

    /// Set whether to apply parent tags
    pub fn with_apply_tags(mut self, apply: bool) -> Self {
        self.apply_tags = apply;
        self
    }
}

/// Specification for importing tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportTasksSpec {
    /// Path to the file containing tasks
    pub file: String,

    /// Variables to pass to the imported tasks
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub vars: HashMap<String, serde_json::Value>,
}

impl ImportTasksSpec {
    /// Create a new import_tasks specification
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            vars: HashMap::new(),
        }
    }

    /// Add a variable to pass to imported tasks
    pub fn with_var(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.vars.insert(key.into(), value);
        self
    }
}

/// Task inclusion/import handler
pub struct TaskIncluder {
    /// Base path for resolving relative includes
    base_path: PathBuf,
}

impl TaskIncluder {
    /// Create a new task includer with the given base path
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Load tasks from a YAML file for include_tasks (dynamic)
    ///
    /// This creates a separate variable scope for the included tasks
    pub async fn load_include_tasks(
        &self,
        spec: &IncludeTasksSpec,
        parent_vars: &VarStore,
    ) -> Result<(Vec<Task>, VarStore)> {
        let file_path = self.resolve_path(&spec.file)?;
        let tasks = self.load_tasks_from_file(&file_path).await?;

        // Create a new variable scope for included tasks
        let mut var_scope = parent_vars.clone();

        // Add include parameters at IncludeParams precedence
        for (key, value) in &spec.vars {
            let yaml_value = serde_json::from_value(value.clone()).map_err(|e| Error::Other {
                message: format!("Failed to convert include var: {}", e),
                source: None,
            })?;
            var_scope.set(key.clone(), yaml_value, VarPrecedence::IncludeParams);
        }

        Ok((tasks, var_scope))
    }

    /// Load tasks from a YAML file for import_tasks (static)
    ///
    /// This merges variables at parse time
    pub async fn load_import_tasks(
        &self,
        spec: &ImportTasksSpec,
        parent_vars: &mut VarStore,
    ) -> Result<Vec<Task>> {
        let file_path = self.resolve_path(&spec.file)?;
        let mut tasks = self.load_tasks_from_file(&file_path).await?;

        // Merge import variables directly into parent scope
        for (key, value) in &spec.vars {
            let yaml_value = serde_json::from_value(value.clone()).map_err(|e| Error::Other {
                message: format!("Failed to convert import var: {}", e),
                source: None,
            })?;
            parent_vars.set(key.clone(), yaml_value, VarPrecedence::IncludeParams);
        }

        // Apply parent tags to imported tasks if needed
        // Note: In import_tasks, tags are always applied
        for _task in &mut tasks {
            // Tasks can access the merged parent variables directly
        }

        Ok(tasks)
    }

    /// Load variables from a file
    pub async fn load_vars_from_file(
        &self,
        file_path: impl AsRef<Path>,
        var_store: &mut VarStore,
    ) -> Result<()> {
        let resolved_path = self.resolve_path(file_path.as_ref().to_str().unwrap())?;

        let content = tokio::fs::read_to_string(&resolved_path)
            .await
            .map_err(|_e| Error::VariablesFileNotFound(resolved_path.clone()))?;

        // Parse YAML variables
        let vars: indexmap::IndexMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)
            .map_err(|e| Error::Other {
                message: format!("Failed to parse variables file: {}", e),
                source: Some(Box::new(e)),
            })?;

        // Add to variable store at IncludeVars precedence
        var_store.set_many_from_file(vars, VarPrecedence::IncludeVars, &resolved_path);

        Ok(())
    }

    /// Resolve a file path relative to the base path with path traversal protection.
    ///
    /// This function validates that the resolved path stays within the base directory
    /// to prevent path traversal attacks (e.g., `../../../etc/passwd`).
    ///
    /// # Security
    ///
    /// - Absolute paths are rejected unless they are within the base directory
    /// - Paths containing `..` components that escape the base directory are rejected
    /// - Symlinks are resolved and validated to prevent symlink-based traversal
    fn resolve_path(&self, file: &str) -> Result<PathBuf> {
        let path = Path::new(file);

        // Get the canonical base path for comparison
        let canonical_base = self.base_path.canonicalize().map_err(|e| Error::Other {
            message: format!(
                "Failed to canonicalize base path '{}': {}",
                self.base_path.display(),
                e
            ),
            source: Some(Box::new(e)),
        })?;

        // Construct the full path
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_path.join(path)
        };

        // Check if file exists first for better error messages
        if !full_path.exists() {
            return Err(Error::FileNotFound(full_path));
        }

        // Canonicalize to resolve symlinks and .. components
        let canonical_path = full_path.canonicalize().map_err(|e| Error::Other {
            message: format!(
                "Failed to canonicalize include path '{}': {}",
                full_path.display(),
                e
            ),
            source: Some(Box::new(e)),
        })?;

        // Security check: ensure the canonical path is within the base directory
        if !canonical_path.starts_with(&canonical_base) {
            return Err(Error::Other {
                message: format!(
                    "Security violation: Path traversal detected. \
                     Path '{}' (resolved to '{}') escapes base directory '{}'",
                    file,
                    canonical_path.display(),
                    canonical_base.display()
                ),
                source: Some(Box::new(PathTraversalError {
                    requested_path: PathBuf::from(file),
                    base_path: self.base_path.clone(),
                })),
            });
        }

        Ok(canonical_path)
    }

    /// Load tasks from a YAML file
    async fn load_tasks_from_file(&self, path: &Path) -> Result<Vec<Task>> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            Error::playbook_parse(
                path,
                format!("Failed to read file: {}", e),
                Some(Box::new(e)),
            )
        })?;

        let tasks: Vec<Task> = serde_yaml::from_str(&content).map_err(|e| {
            Error::playbook_parse(
                path,
                format!("Failed to parse tasks: {}", e),
                Some(Box::new(e)),
            )
        })?;

        Ok(tasks)
    }
}

/// Helper to expand included/imported tasks in place
pub async fn expand_task_includes(
    tasks: &[Task],
    includer: &TaskIncluder,
    var_store: &mut VarStore,
) -> Result<Vec<Task>> {
    let mut expanded = Vec::new();

    for task in tasks {
        // Check if this is an import_tasks (needs static expansion)
        if let Some(import_spec) = extract_import_tasks(task) {
            let imported = includer.load_import_tasks(&import_spec, var_store).await?;
            expanded.extend(imported);
        } else {
            // Keep the task as-is (include_tasks will be expanded at runtime)
            expanded.push(task.clone());
        }
    }

    Ok(expanded)
}

/// Extract import_tasks spec from a task if present
fn extract_import_tasks(task: &Task) -> Option<ImportTasksSpec> {
    // Check if task has import_tasks module
    if task.module_name() == "import_tasks" {
        // Try to parse from task args
        if let Ok(spec) = serde_json::from_value::<ImportTasksSpec>(task.module_args().clone()) {
            return Some(spec);
        }
        // Fallback: simple string file path
        if let Some(file_obj) = task.module_args().as_object() {
            if let Some(file_val) = file_obj.get("file") {
                if let Some(file_str) = file_val.as_str() {
                    return Some(ImportTasksSpec::new(file_str));
                }
            }
        }
    }
    None
}

/// Extract include_tasks spec from a task if present
pub fn extract_include_tasks(task: &Task) -> Option<IncludeTasksSpec> {
    if task.module_name() == "include_tasks" {
        // Try to parse from task args
        if let Ok(spec) = serde_json::from_value::<IncludeTasksSpec>(task.module_args().clone()) {
            return Some(spec);
        }
        // Fallback: simple string file path
        if let Some(file_obj) = task.module_args().as_object() {
            if let Some(file_val) = file_obj.get("file") {
                if let Some(file_str) = file_val.as_str() {
                    return Some(IncludeTasksSpec::new(file_str));
                }
            }
        }
    }
    None
}

/// Extract include_vars file path from a task if present
pub fn extract_include_vars(task: &Task) -> Option<String> {
    if task.module_name() == "include_vars" {
        if let Some(file_obj) = task.module_args().as_object() {
            if let Some(file_val) = file_obj.get("file") {
                if let Some(file_str) = file_val.as_str() {
                    return Some(file_str.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_include_tasks_basic() {
        let temp_dir = TempDir::new().unwrap();
        let tasks_file = temp_dir.path().join("tasks.yml");

        let mut file = std::fs::File::create(&tasks_file).unwrap();
        write!(
            file,
            r#"
- name: Task 1
  debug:
    msg: "Hello from included task"
- name: Task 2
  debug:
    msg: "Another task"
"#
        )
        .unwrap();

        let includer = TaskIncluder::new(temp_dir.path());
        let spec = IncludeTasksSpec::new("tasks.yml");
        let var_store = VarStore::new();

        let (tasks, _scope) = includer
            .load_include_tasks(&spec, &var_store)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "Task 1");
    }

    #[tokio::test]
    async fn test_include_vars() {
        let temp_dir = TempDir::new().unwrap();
        let vars_file = temp_dir.path().join("vars.yml");

        let mut file = std::fs::File::create(&vars_file).unwrap();
        write!(
            file,
            r#"
test_var: "test_value"
number_var: 42
"#
        )
        .unwrap();

        let includer = TaskIncluder::new(temp_dir.path());
        let mut var_store = VarStore::new();

        includer
            .load_vars_from_file("vars.yml", &mut var_store)
            .await
            .unwrap();

        assert!(var_store.contains("test_var"));
        assert!(var_store.contains("number_var"));
    }

    #[tokio::test]
    async fn test_import_tasks_merges_vars() {
        let temp_dir = TempDir::new().unwrap();
        let tasks_file = temp_dir.path().join("tasks.yml");

        let mut file = std::fs::File::create(&tasks_file).unwrap();
        write!(
            file,
            r#"
- name: Import test
  debug:
    msg: "{{ imported_var }}"
"#
        )
        .unwrap();

        let includer = TaskIncluder::new(temp_dir.path());
        let mut spec = ImportTasksSpec::new("tasks.yml");
        spec = spec.with_var("imported_var", serde_json::json!("imported_value"));

        let mut var_store = VarStore::new();
        let tasks = includer
            .load_import_tasks(&spec, &mut var_store)
            .await
            .unwrap();

        assert_eq!(tasks.len(), 1);
        assert!(var_store.contains("imported_var"));
    }
}
