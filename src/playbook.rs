//! Playbook definitions and parsing.
//!
//! This module provides types for representing Ansible-compatible playbooks
//! with type-safe definitions and validation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{Error, Result};
use crate::vars::Variables;

/// Helper function for serde to check if Variables is empty
fn is_vars_empty(vars: &Variables) -> bool {
    vars.is_empty()
}

/// A playbook containing one or more plays.
///
/// Playbooks are the top-level configuration files in Rustible.
/// They contain a list of plays that define the automation workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playbook {
    /// Name of the playbook (optional, derived from filename if not set)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The plays in this playbook
    #[serde(flatten)]
    pub plays: Vec<Play>,

    /// Path to the playbook file (set during loading)
    #[serde(skip)]
    pub source_path: Option<std::path::PathBuf>,
}

impl Playbook {
    /// Loads a playbook from a YAML file.
    pub async fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            Error::playbook_parse(path, format!("Failed to read file: {}", e), None)
        })?;

        Self::from_yaml(&content, Some(path.to_path_buf()))
    }

    /// Parses a playbook from a YAML string.
    pub fn from_yaml(yaml: &str, source_path: Option<std::path::PathBuf>) -> Result<Self> {
        // Playbooks are a list of plays at the top level
        let plays: Vec<Play> = serde_yaml::from_str(yaml).map_err(|e| {
            Error::playbook_parse(
                source_path.as_ref().map_or("<string>".into(), |p| p.clone()),
                e.to_string(),
                None,
            )
        })?;

        let name = source_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().to_string());

        Ok(Self {
            name,
            plays,
            source_path,
        })
    }

    /// Validates the playbook structure.
    pub fn validate(&self) -> Result<()> {
        if self.plays.is_empty() {
            return Err(Error::PlaybookValidation(
                "Playbook must contain at least one play".to_string(),
            ));
        }

        for (idx, play) in self.plays.iter().enumerate() {
            play.validate().map_err(|e| {
                Error::PlaybookValidation(format!("Play {} validation failed: {}", idx + 1, e))
            })?;
        }

        Ok(())
    }

    /// Returns the number of plays.
    pub fn play_count(&self) -> usize {
        self.plays.len()
    }

    /// Returns total number of tasks across all plays.
    pub fn task_count(&self) -> usize {
        self.plays.iter().map(|p| p.tasks.len()).sum()
    }
}

/// A play within a playbook.
///
/// A play maps a selection of hosts to tasks to be executed on those hosts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Play {
    /// Name of the play
    #[serde(default)]
    pub name: String,

    /// Host pattern to match against inventory
    pub hosts: String,

    /// Whether to gather facts before executing tasks
    #[serde(default = "default_gather_facts")]
    pub gather_facts: bool,

    /// Subset of gathered facts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gather_subset: Option<Vec<String>>,

    /// Timeout for fact gathering in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gather_timeout: Option<u64>,

    /// Variables for this play
    #[serde(default, skip_serializing_if = "is_vars_empty")]
    pub vars: Variables,

    /// Variable files to load
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vars_files: Vec<String>,

    /// Roles to apply
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<RoleRef>,

    /// Pre-tasks to run before roles
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_tasks: Vec<Task>,

    /// Tasks to run after roles
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Task>,

    /// Post-tasks to run after tasks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_tasks: Vec<Task>,

    /// Handlers that can be notified
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub handlers: Vec<Handler>,

    /// Become configuration
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#become: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub become_user: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub become_method: Option<String>,

    /// Connection settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,

    /// Remote user
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_user: Option<String>,

    /// Port to connect on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// Execution strategy
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,

    /// Serial execution (batch size)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<SerialSpec>,

    /// Maximum failure percentage before aborting
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fail_percentage: Option<u8>,

    /// Whether to run handlers on failure
    #[serde(default)]
    pub force_handlers: bool,

    /// Whether to ignore unreachable hosts
    #[serde(default)]
    pub ignore_unreachable: bool,

    /// Module defaults
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub module_defaults: HashMap<String, serde_json::Value>,

    /// Environment variables
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub environment: HashMap<String, String>,

    /// Tags for filtering
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

fn default_gather_facts() -> bool {
    true
}

impl Play {
    /// Creates a new play with the given name and host pattern.
    pub fn new(name: impl Into<String>, hosts: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            hosts: hosts.into(),
            gather_facts: true,
            gather_subset: None,
            gather_timeout: None,
            vars: Variables::new(),
            vars_files: Vec::new(),
            roles: Vec::new(),
            pre_tasks: Vec::new(),
            tasks: Vec::new(),
            post_tasks: Vec::new(),
            handlers: Vec::new(),
            r#become: None,
            become_user: None,
            become_method: None,
            connection: None,
            remote_user: None,
            port: None,
            strategy: None,
            serial: None,
            max_fail_percentage: None,
            force_handlers: false,
            ignore_unreachable: false,
            module_defaults: HashMap::new(),
            environment: HashMap::new(),
            tags: Vec::new(),
        }
    }

    /// Validates the play structure.
    pub fn validate(&self) -> Result<()> {
        if self.hosts.is_empty() {
            return Err(Error::PlaybookValidation(
                "Play must specify hosts".to_string(),
            ));
        }

        // Validate tasks
        for task in self.all_tasks() {
            task.validate()?;
        }

        // Validate handlers
        for handler in &self.handlers {
            if handler.name.is_empty() {
                return Err(Error::PlaybookValidation(
                    "Handler must have a name".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Returns an iterator over all tasks (pre_tasks, tasks, post_tasks).
    pub fn all_tasks(&self) -> impl Iterator<Item = &Task> {
        self.pre_tasks
            .iter()
            .chain(self.tasks.iter())
            .chain(self.post_tasks.iter())
    }

    /// Returns the total number of tasks.
    pub fn task_count(&self) -> usize {
        self.pre_tasks.len() + self.tasks.len() + self.post_tasks.len()
    }
}

/// Reference to a role with optional parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RoleRef {
    /// Simple role name
    Simple(String),

    /// Role with configuration
    Full {
        /// Role name
        role: String,

        /// Role variables
        #[serde(default, flatten)]
        vars: HashMap<String, serde_json::Value>,

        /// When condition
        #[serde(skip_serializing_if = "Option::is_none")]
        when: Option<String>,

        /// Tags
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
    },
}

impl RoleRef {
    /// Returns the role name.
    pub fn name(&self) -> &str {
        match self {
            Self::Simple(name) => name,
            Self::Full { role, .. } => role,
        }
    }
}

/// A task to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Name of the task
    #[serde(default)]
    pub name: String,

    /// Module to execute (the key is the module name)
    #[serde(flatten)]
    pub module: TaskModule,

    /// Conditional execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<When>,

    /// Loop over items
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_: Option<serde_json::Value>,

    /// Alternative loop syntax
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with_items: Option<serde_json::Value>,

    /// Register result in variable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub register: Option<String>,

    /// Variable to store results for loop
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_control: Option<LoopControl>,

    /// Handlers to notify on change
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notify: Vec<String>,

    /// Whether to ignore errors
    #[serde(default)]
    pub ignore_errors: bool,

    /// Whether to ignore unreachable
    #[serde(default)]
    pub ignore_unreachable: bool,

    /// Become settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#become: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub become_user: Option<String>,

    /// Delegation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate_to: Option<String>,

    /// Run once
    #[serde(default)]
    pub run_once: bool,

    /// Changed when condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_when: Option<String>,

    /// Failed when condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_when: Option<String>,

    /// Tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Task-level variables
    #[serde(default, skip_serializing_if = "is_vars_empty")]
    pub vars: Variables,

    /// Environment variables
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub environment: HashMap<String, String>,

    /// Async execution timeout
    #[serde(skip_serializing_if = "Option::is_none")]
    pub async_: Option<u64>,

    /// Poll interval for async
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll: Option<u64>,

    /// Number of retries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,

    /// Delay between retries in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<u64>,

    /// Condition for retry success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
}

impl Task {
    /// Creates a new task.
    pub fn new(name: impl Into<String>, module: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            module: TaskModule {
                name: module.into(),
                args,
            },
            when: None,
            loop_: None,
            with_items: None,
            register: None,
            loop_control: None,
            notify: Vec::new(),
            ignore_errors: false,
            ignore_unreachable: false,
            r#become: None,
            become_user: None,
            delegate_to: None,
            run_once: false,
            changed_when: None,
            failed_when: None,
            tags: Vec::new(),
            vars: Variables::new(),
            environment: HashMap::new(),
            async_: None,
            poll: None,
            retries: None,
            delay: None,
            until: None,
        }
    }

    /// Validates the task.
    pub fn validate(&self) -> Result<()> {
        if self.module.name.is_empty() {
            return Err(Error::PlaybookValidation(
                "Task must specify a module".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the module name.
    pub fn module_name(&self) -> &str {
        &self.module.name
    }

    /// Returns the module arguments.
    pub fn module_args(&self) -> &serde_json::Value {
        &self.module.args
    }
}

/// Module invocation in a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskModule {
    /// Module name
    #[serde(skip)]
    pub name: String,

    /// Module arguments
    #[serde(flatten)]
    pub args: serde_json::Value,
}

/// Conditional expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum When {
    /// Single condition
    Single(String),
    /// Multiple conditions (AND)
    Multiple(Vec<String>),
}

impl When {
    /// Returns the conditions as a slice.
    pub fn conditions(&self) -> Vec<&str> {
        match self {
            Self::Single(s) => vec![s.as_str()],
            Self::Multiple(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

/// Loop control options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopControl {
    /// Variable name for current item
    #[serde(default = "default_loop_var")]
    pub loop_var: String,

    /// Variable name for item index
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_var: Option<String>,

    /// Label for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Pause between iterations in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause: Option<u64>,

    /// Extended loop info
    #[serde(default)]
    pub extended: bool,
}

fn default_loop_var() -> String {
    "item".to_string()
}

/// Serial execution specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SerialSpec {
    /// Fixed batch size
    Fixed(usize),
    /// Percentage of hosts
    Percentage(String),
    /// Progressive batch sizes
    Progressive(Vec<SerialSpec>),
}

/// A handler (special task triggered by notifications).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handler {
    /// Handler name (must match notify in tasks)
    pub name: String,

    /// The task to execute
    #[serde(flatten)]
    pub task: Task,

    /// Listen to additional names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listen: Vec<String>,
}

impl Handler {
    /// Creates a new handler.
    pub fn new(name: impl Into<String>, task: Task) -> Self {
        Self {
            name: name.into(),
            task,
            listen: Vec::new(),
        }
    }

    /// Returns all names this handler responds to.
    pub fn trigger_names(&self) -> Vec<&str> {
        let mut names = vec![self.name.as_str()];
        names.extend(self.listen.iter().map(String::as_str));
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_playbook() {
        let yaml = r#"
- name: Test Play
  hosts: all
  tasks:
    - name: Echo hello
      command: echo hello
"#;
        let result = Playbook::from_yaml(yaml, None);
        assert!(result.is_ok());
        let playbook = result.unwrap();
        assert_eq!(playbook.plays.len(), 1);
        assert_eq!(playbook.plays[0].name, "Test Play");
    }
}
