//! Runtime handler for include_tasks and import_tasks
//!
//! This module provides the runtime execution logic for dynamically including
//! tasks during playbook execution.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::error::Result;
use crate::executor::runtime::RuntimeContext;
use crate::executor::{ExecutorError, ExecutorResult};
use crate::include::{extract_include_tasks, IncludeTasksSpec, TaskIncluder};
use crate::playbook::Task;
use crate::vars::VarStore;

/// Handler for processing include_tasks and import_tasks during execution
pub struct IncludeTasksHandler {
    includer: TaskIncluder,
}

impl IncludeTasksHandler {
    /// Create a new handler with the given base path
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            includer: TaskIncluder::new(base_path),
        }
    }

    /// Check if a task is an include_tasks directive
    pub fn is_include_tasks(task: &Task) -> bool {
        task.module_name() == "include_tasks"
    }

    /// Check if a task is an import_tasks directive
    pub fn is_import_tasks(task: &Task) -> bool {
        task.module_name() == "import_tasks"
    }

    /// Extract include_tasks specification from a task
    pub fn extract_spec(task: &Task) -> ExecutorResult<IncludeTasksSpec> {
        extract_include_tasks(task).ok_or_else(|| {
            ExecutorError::RuntimeError(
                "Failed to extract include_tasks specification from task".into(),
            )
        })
    }

    /// Load tasks from include_tasks (dynamic, with separate variable scope)
    pub async fn load_include_tasks(
        &self,
        spec: &IncludeTasksSpec,
        runtime: &Arc<RwLock<RuntimeContext>>,
        host: &str,
    ) -> Result<Vec<Task>> {
        info!("Loading include_tasks from: {}", spec.file);

        // Get current variable store for this host
        let parent_vars = {
            let rt = runtime.read().await;
            let merged_vars = rt.get_merged_vars(host);

            // Convert to VarStore
            let mut var_store = VarStore::new();
            for (key, value) in merged_vars {
                if let Ok(yaml_value) = serde_json::from_value(value) {
                    var_store.set(
                        key,
                        yaml_value,
                        crate::vars::VarPrecedence::PlayVars,
                    );
                }
            }
            var_store
        };

        // Load tasks with their own variable scope
        let (tasks, _task_vars) = self.includer.load_include_tasks(spec, &parent_vars).await?;

        // Store the include-specific variables in runtime for this host
        // This allows the included tasks to access the variables
        if !spec.vars.is_empty() {
            let mut rt = runtime.write().await;
            for (key, value) in &spec.vars {
                rt.set_host_var(host, key.clone(), value.clone());
            }
        }

        debug!("Loaded {} tasks from include_tasks", tasks.len());
        Ok(tasks)
    }

    /// Load tasks from import_tasks (static, merges variables)
    pub async fn load_import_tasks(
        &self,
        spec: &IncludeTasksSpec,
        runtime: &Arc<RwLock<RuntimeContext>>,
        host: &str,
    ) -> Result<Vec<Task>> {
        info!("Loading import_tasks from: {}", spec.file);

        // Get current variable store
        let mut var_store = {
            let rt = runtime.read().await;
            let merged_vars = rt.get_merged_vars(host);

            let mut var_store = VarStore::new();
            for (key, value) in merged_vars {
                if let Ok(yaml_value) = serde_json::from_value(value) {
                    var_store.set(
                        key,
                        yaml_value,
                        crate::vars::VarPrecedence::PlayVars,
                    );
                }
            }
            var_store
        };

        // Convert spec to ImportTasksSpec
        let import_spec = crate::include::ImportTasksSpec {
            file: spec.file.clone(),
            vars: spec.vars.clone(),
        };

        // Load tasks and merge variables
        let tasks = self.includer.load_import_tasks(&import_spec, &mut var_store).await?;

        // Merge imported variables back into runtime
        // Variables have already been merged into var_store by load_import_tasks
        // For proper integration, we'd need to sync these back to runtime
        // This is a simplified version - full implementation would handle variable precedence

        debug!("Loaded {} tasks from import_tasks", tasks.len());
        Ok(tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn test_is_include_tasks() {
        let task = Task::new("test", "include_tasks", serde_json::json!({"file": "test.yml"}));
        assert!(IncludeTasksHandler::is_include_tasks(&task));

        let task2 = Task::new("test", "debug", serde_json::json!({"msg": "test"}));
        assert!(!IncludeTasksHandler::is_include_tasks(&task2));
    }

    #[test]
    fn test_is_import_tasks() {
        let task = Task::new("test", "import_tasks", serde_json::json!({"file": "test.yml"}));
        assert!(IncludeTasksHandler::is_import_tasks(&task));

        let task2 = Task::new("test", "debug", serde_json::json!({"msg": "test"}));
        assert!(!IncludeTasksHandler::is_import_tasks(&task2));
    }

    #[tokio::test]
    async fn test_load_include_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let tasks_file = temp_dir.path().join("tasks.yml");

        std::fs::write(
            &tasks_file,
            r#"
- name: Task 1
  debug:
    msg: "Hello from included task"
- name: Task 2
  command: echo "test"
"#,
        )
        .unwrap();

        let handler = IncludeTasksHandler::new(temp_dir.path().to_path_buf());
        let runtime = Arc::new(RwLock::new(RuntimeContext::new()));

        let spec = IncludeTasksSpec::new("tasks.yml");
        let tasks = handler
            .load_include_tasks(&spec, &runtime, "localhost")
            .await
            .unwrap();

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "Task 1");
        assert_eq!(tasks[1].name, "Task 2");
    }

    #[tokio::test]
    async fn test_load_include_tasks_with_vars() {
        let temp_dir = TempDir::new().unwrap();
        let tasks_file = temp_dir.path().join("tasks.yml");

        std::fs::write(
            &tasks_file,
            r#"
- name: Use variable
  debug:
    msg: "{{ test_var }}"
"#,
        )
        .unwrap();

        let handler = IncludeTasksHandler::new(temp_dir.path().to_path_buf());
        let runtime = Arc::new(RwLock::new(RuntimeContext::new()));

        let mut spec = IncludeTasksSpec::new("tasks.yml");
        spec.vars.insert("test_var".to_string(), json!("test_value"));

        let tasks = handler
            .load_include_tasks(&spec, &runtime, "localhost")
            .await
            .unwrap();

        assert_eq!(tasks.len(), 1);

        // Check that variable was stored in runtime
        let rt = runtime.read().await;
        let vars = rt.get_merged_vars("localhost");
        assert!(vars.contains_key("test_var"));
    }
}
