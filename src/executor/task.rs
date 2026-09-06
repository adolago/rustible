//! Task definition and execution for Rustible
//!
//! This module provides:
//! - Task struct with module, args, when conditions, loops
//! - Task result handling
//! - Changed/ok/failed states
//!
//! # Performance Optimizations
//!
//! This module includes several hot path optimizations:
//! - Cached regex patterns using `once_cell::sync::Lazy`
//! - Inline hints for frequently called functions
//! - Reduced allocations in template processing

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, instrument, warn};

// ============================================================================
// PERFORMANCE: Cached regex patterns for hot path template processing
// ============================================================================

/// Cached regex for template variable extraction: {{ variable }}
/// This regex is compiled once and reused across all template operations.
static TEMPLATE_VAR_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\{\{\s*([^}]+?)\s*\}\}").expect("Invalid template regex"));

/// Cached regex for checking if string contains template syntax
#[allow(dead_code)]
static TEMPLATE_CHECK_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\{\{|\{%").expect("Invalid template check regex"));

use crate::executor::parallelization::ParallelizationManager;
use crate::executor::runtime::{ExecutionContext, RegisteredResult, RuntimeContext};
use crate::executor::{ExecutorError, ExecutorResult};
use crate::modules::ModuleRegistry;
use crate::template::get_engine;

/// Status of a task execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TaskStatus {
    /// Task completed successfully without changes
    #[default]
    Ok,
    /// Task completed successfully with changes
    Changed,
    /// Task failed
    Failed,
    /// Task was skipped (condition not met)
    Skipped,
    /// Host was unreachable
    Unreachable,
}

/// Result of executing a task
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskResult {
    /// Final status of the task
    pub status: TaskStatus,
    /// Whether something was changed
    pub changed: bool,
    /// Optional message from the task
    pub msg: Option<String>,
    /// Module-specific result data
    pub result: Option<JsonValue>,
    /// Diff showing what changed (if diff_mode enabled)
    pub diff: Option<TaskDiff>,
}

impl TaskResult {
    /// Create a successful result
    pub fn ok() -> Self {
        Self {
            status: TaskStatus::Ok,
            changed: false,
            ..Default::default()
        }
    }

    /// Create a changed result
    pub fn changed() -> Self {
        Self {
            status: TaskStatus::Changed,
            changed: true,
            ..Default::default()
        }
    }

    /// Create a failed result
    pub fn failed(msg: impl Into<String>) -> Self {
        Self {
            status: TaskStatus::Failed,
            changed: false,
            msg: Some(msg.into()),
            ..Default::default()
        }
    }

    /// Create a skipped result
    pub fn skipped(msg: impl Into<String>) -> Self {
        Self {
            status: TaskStatus::Skipped,
            changed: false,
            msg: Some(msg.into()),
            ..Default::default()
        }
    }

    /// Create an unreachable result
    pub fn unreachable(msg: impl Into<String>) -> Self {
        Self {
            status: TaskStatus::Unreachable,
            changed: false,
            msg: Some(msg.into()),
            ..Default::default()
        }
    }

    /// Set the result data
    pub fn with_result(mut self, result: JsonValue) -> Self {
        self.result = Some(result);
        self
    }

    /// Set the message
    pub fn with_msg(mut self, msg: impl Into<String>) -> Self {
        self.msg = Some(msg.into());
        self
    }

    /// Set the diff
    pub fn with_diff(mut self, diff: TaskDiff) -> Self {
        self.diff = Some(diff);
        self
    }

    /// Convert to RegisteredResult
    pub fn to_registered(
        &self,
        stdout: Option<String>,
        stderr: Option<String>,
    ) -> RegisteredResult {
        let mut registered: RegisteredResult = self
            .result
            .clone()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default();
        if let Some(JsonValue::Object(data)) = &self.result {
            registered.data = data
                .iter()
                .filter(|(key, _)| {
                    !matches!(
                        key.as_str(),
                        "changed"
                            | "failed"
                            | "skipped"
                            | "rc"
                            | "stdout"
                            | "stdout_lines"
                            | "stderr"
                            | "stderr_lines"
                            | "msg"
                            | "results"
                    )
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            registered.rc = data
                .get("rc")
                .and_then(JsonValue::as_i64)
                .and_then(|n| i32::try_from(n).ok());
            registered.stdout = data
                .get("stdout")
                .and_then(JsonValue::as_str)
                .map(String::from);
            registered.stderr = data
                .get("stderr")
                .and_then(JsonValue::as_str)
                .map(String::from);
        }
        registered.changed = self.changed;
        registered.failed = matches!(self.status, TaskStatus::Failed | TaskStatus::Unreachable);
        registered.skipped = self.status == TaskStatus::Skipped;
        registered.stdout = stdout.or(registered.stdout);
        registered.stderr = stderr.or(registered.stderr);
        registered.stdout_lines = registered
            .stdout
            .as_ref()
            .map(|s| s.lines().map(String::from).collect());
        registered.stderr_lines = registered
            .stderr
            .as_ref()
            .map(|s| s.lines().map(String::from).collect());
        registered.msg = self.msg.clone();
        registered
    }
}

/// Diff showing before/after state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDiff {
    pub before: Option<String>,
    pub after: Option<String>,
    pub before_header: Option<String>,
    pub after_header: Option<String>,
}

/// A handler that can be notified by tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handler {
    /// Handler name (used for notification)
    pub name: String,
    /// Module to execute
    pub module: String,
    /// Module arguments
    #[serde(default)]
    pub args: IndexMap<String, JsonValue>,
    /// Optional when condition
    pub when: Option<String>,
    /// Listen for multiple notification names
    #[serde(default)]
    pub listen: Vec<String>,
}

/// Loop control options for customizing loop behavior
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoopControl {
    /// Variable name for current item (default: "item")
    #[serde(default = "default_loop_var")]
    pub loop_var: String,
    /// Variable name for item index
    #[serde(default)]
    pub index_var: Option<String>,
    /// Label for display (template evaluated per item)
    #[serde(default)]
    pub label: Option<String>,
    /// Pause between iterations in seconds
    #[serde(default)]
    pub pause: Option<u64>,
    /// Enable extended loop information (revindex, revindex0, etc.)
    #[serde(default)]
    pub extended: bool,
}

/// A task to be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Task name (displayed during execution)
    pub name: String,
    /// Module to execute
    pub module: String,
    /// Module arguments
    #[serde(default)]
    pub args: IndexMap<String, JsonValue>,
    /// Conditional expression (Jinja2-like)
    #[serde(default)]
    pub when: Option<String>,
    /// Handlers to notify on change
    #[serde(default)]
    pub notify: Vec<String>,
    /// Variable name to register result
    #[serde(default)]
    pub register: Option<String>,
    /// Items to loop over
    #[serde(default)]
    pub loop_items: Option<LoopSource>,
    /// Loop variable name (default: "item")
    #[serde(default = "default_loop_var")]
    pub loop_var: String,
    /// Loop control options
    #[serde(default)]
    pub loop_control: Option<LoopControl>,
    /// Whether to ignore errors
    #[serde(default)]
    pub ignore_errors: bool,
    /// Custom condition to determine if task changed
    #[serde(default)]
    pub changed_when: Option<String>,
    /// Custom condition to determine if task failed
    #[serde(default)]
    pub failed_when: Option<String>,
    /// Delegate task to another host
    #[serde(default)]
    pub delegate_to: Option<String>,
    /// Whether facts should be set on the delegated host instead of the original host
    #[serde(default)]
    pub delegate_facts: Option<bool>,
    /// Run task only once (not on each host)
    #[serde(default)]
    pub run_once: bool,
    /// Tags for task filtering
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether to become another user
    #[serde(default)]
    pub r#become: bool,
    /// User to become
    #[serde(default)]
    pub become_user: Option<String>,
    /// Block ID this task belongs to (if part of block/rescue/always)
    #[serde(default)]
    pub block_id: Option<String>,
    /// Task type within a block
    #[serde(default)]
    pub block_role: BlockRole,
    /// Number of retries for until loop
    #[serde(default)]
    pub retries: Option<u32>,
    /// Delay between retries in seconds
    #[serde(default)]
    pub delay: Option<u64>,
    /// Until condition for retry loop
    #[serde(default)]
    pub until: Option<String>,
    /// Task-level variables
    #[serde(default)]
    pub vars: IndexMap<String, JsonValue>,
}

/// Role of a task within a block structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockRole {
    /// Normal task or task in the main block section
    #[default]
    Normal,
    /// Task in the rescue section (runs on block failure)
    Rescue,
    /// Task in the always section (runs regardless)
    Always,
}

/// Source for loop iteration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoopSource {
    /// Static list of items
    Items(Vec<JsonValue>),
    /// Template string to evaluate for items
    Template(String),
}

fn default_loop_var() -> String {
    "item".to_string()
}

impl Default for Task {
    fn default() -> Self {
        Self {
            name: String::new(),
            module: String::new(),
            args: IndexMap::new(),
            when: None,
            notify: Vec::new(),
            register: None,
            loop_items: None,
            loop_var: default_loop_var(),
            loop_control: None,
            ignore_errors: false,
            changed_when: None,
            failed_when: None,
            delegate_to: None,
            delegate_facts: None,
            run_once: false,
            tags: Vec::new(),
            r#become: false,
            become_user: None,
            block_id: None,
            block_role: BlockRole::Normal,
            retries: None,
            delay: None,
            until: None,
            vars: IndexMap::new(),
        }
    }
}

/// Convert from playbook::Task to executor::task::Task
impl From<crate::playbook::Task> for Task {
    fn from(pt: crate::playbook::Task) -> Self {
        // Convert args from serde_json::Value to IndexMap
        let args = if let Some(obj) = pt.module.args.as_object() {
            obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            IndexMap::from([("_raw_params".to_string(), pt.module.args.clone())])
        };

        // Convert when condition
        let when = pt.when.map(|w| match w {
            crate::playbook::When::Single(s) => s,
            crate::playbook::When::Multiple(v) => v.join(" and "),
        });

        // Convert loop items from various sources
        // Priority: loop > with_items > with_dict > with_fileglob
        let loop_items = if let Some(v) = pt.loop_.or(pt.with_items) {
            // Standard loop or with_items - expect array
            if let Some(arr) = v.as_array() {
                Some(LoopSource::Items(arr.clone()))
            } else {
                v.as_str().map(|s| LoopSource::Template(s.to_string()))
            }
        } else if let Some(v) = pt.with_dict {
            // with_dict - convert dict to list of {key, value} objects
            if let Some(obj) = v.as_object() {
                let items: Vec<JsonValue> = obj
                    .iter()
                    .map(|(k, val)| serde_json::json!({"key": k, "value": val}))
                    .collect();
                Some(LoopSource::Items(items))
            } else {
                None
            }
        } else if let Some(v) = pt.with_fileglob {
            // with_fileglob - for now just pass patterns as strings
            // (actual glob expansion happens at runtime)
            if let Some(arr) = v.as_array() {
                Some(LoopSource::Items(arr.clone()))
            } else if v.is_string() {
                Some(LoopSource::Items(vec![v]))
            } else {
                None
            }
        } else {
            None
        };

        // Get loop_var from loop_control if available
        let loop_var = pt
            .loop_control
            .as_ref()
            .map(|lc| lc.loop_var.clone())
            .unwrap_or_else(default_loop_var);

        // Convert loop_control from playbook to executor format
        let loop_control = pt.loop_control.as_ref().map(|lc| LoopControl {
            loop_var: lc.loop_var.clone(),
            index_var: lc.index_var.clone(),
            label: lc.label.clone(),
            pause: lc.pause,
            extended: lc.extended,
        });

        Self {
            name: pt.name,
            module: pt.module.name,
            args,
            when,
            notify: pt.notify,
            register: pt.register,
            loop_items,
            loop_var,
            loop_control,
            ignore_errors: pt.ignore_errors,
            changed_when: pt.changed_when,
            failed_when: pt.failed_when,
            delegate_to: pt.delegate_to,
            delegate_facts: pt.delegate_facts,
            run_once: pt.run_once,
            tags: pt.tags,
            r#become: pt.r#become.unwrap_or(false),
            become_user: pt.become_user,
            block_id: None,
            block_role: BlockRole::Normal,
            retries: pt.retries,
            delay: pt.delay,
            until: pt.until,
            vars: pt.vars.as_map().clone(),
        }
    }
}

impl Task {
    /// Create a new task with the given name and module
    pub fn new(name: impl Into<String>, module: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            module: module.into(),
            ..Default::default()
        }
    }

    /// Add an argument to the task
    pub fn arg(mut self, key: impl Into<String>, value: impl Into<JsonValue>) -> Self {
        self.args.insert(key.into(), value.into());
        self
    }

    /// Set the when condition
    pub fn when(mut self, condition: impl Into<String>) -> Self {
        self.when = Some(condition.into());
        self
    }

    /// Add a handler to notify
    pub fn notify(mut self, handler: impl Into<String>) -> Self {
        self.notify.push(handler.into());
        self
    }

    /// Set the register variable
    pub fn register(mut self, name: impl Into<String>) -> Self {
        self.register = Some(name.into());
        self
    }

    /// Set loop items
    pub fn loop_over(mut self, items: Vec<JsonValue>) -> Self {
        self.loop_items = Some(LoopSource::Items(items));
        self
    }

    /// Set the loop variable name
    pub fn loop_var(mut self, name: impl Into<String>) -> Self {
        self.loop_var = name.into();
        self
    }

    /// Set ignore_errors
    pub fn ignore_errors(mut self, ignore: bool) -> Self {
        self.ignore_errors = ignore;
        self
    }

    /// Execute the task
    #[instrument(skip(self, ctx, runtime, handlers, notified, parallelization_manager, module_registry, batch_processor), fields(task_name = %self.name, host = %ctx.host))]
    pub async fn execute(
        &self,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
        handlers: &Arc<RwLock<HashMap<String, Handler>>>,
        notified: &Arc<Mutex<std::collections::HashSet<String>>>,
        parallelization_manager: &Arc<ParallelizationManager>,
        module_registry: &Arc<ModuleRegistry>,
        batch_processor: &Arc<crate::executor::batch_processor::BatchProcessor>,
        pipelining: bool,
    ) -> ExecutorResult<TaskResult> {
        info!("Executing task: {}", self.name);

        // Evaluate when condition
        if let Some(ref condition) = self.when {
            let should_run = self.evaluate_condition(condition, ctx, runtime).await?;
            if !should_run {
                debug!("Task skipped due to when condition: {}", condition);
                return Ok(TaskResult::skipped(format!(
                    "Skipped: condition '{}' was false",
                    condition
                )));
            }
        }

        // Handle delegation - create appropriate context for execution and fact storage
        // Decide contexts on the same canonical name that dispatch uses, so
        // `ansible.legacy.set_fact` behaves exactly like `set_fact`.
        let canonical_module = ModuleRegistry::normalize_module_name(&self.module);
        let (mut execution_ctx, fact_storage_ctx) =
            if let Some(ref delegate_host) = self.delegate_to {
                debug!("Delegating task to host: {}", delegate_host);

                // Create execution context for the delegate host (where task actually runs)
                let mut delegate_ctx = ctx.clone();
                delegate_ctx.host = delegate_host.clone();
                if delegate_host != &ctx.host {
                    delegate_ctx.connection = None;
                }

                // Create fact storage context based on delegate_facts setting
                // If delegate_facts is true, store on delegate host; otherwise on original host
                let fact_ctx = if self.delegate_facts.unwrap_or(false) {
                    // Facts go to delegate host
                    let mut fact_ctx = ctx.clone();
                    fact_ctx.host = delegate_host.clone();
                    fact_ctx
                } else {
                    // Facts go to original host (default behavior)
                    ctx.clone()
                };

                (delegate_ctx, fact_ctx)
            } else {
                // No delegation - both execution and facts use the same context
                (ctx.clone(), ctx.clone())
            };

        execution_ctx.r#become |= self.r#become;
        if let Some(user) = &self.become_user {
            execution_ctx.r#become_user = user.clone();
        }

        // Handle loops - for set_fact, use fact_storage_ctx; for others, use execution_ctx
        if let Some(ref loop_source) = self.loop_items {
            let resolved_items = match loop_source {
                LoopSource::Items(items) => items.clone(),
                LoopSource::Template(template) => {
                    // Resolve template to get items
                    let rt = runtime.read().await;
                    let vars: std::collections::HashMap<String, JsonValue> = rt
                        .get_merged_vars(&execution_ctx.host)
                        .into_iter()
                        .collect();
                    let engine = get_engine();
                    let rendered = engine.render(template, &vars).map_err(|e| {
                        crate::executor::ExecutorError::RuntimeError(format!(
                            "Failed to resolve loop template '{}': {}",
                            template, e
                        ))
                    })?;
                    serde_json::from_str::<Vec<JsonValue>>(&rendered)
                        .unwrap_or_else(|_| vec![JsonValue::String(rendered)])
                }
            };
            let loop_ctx = if canonical_module == "set_fact" {
                &fact_storage_ctx
            } else {
                &execution_ctx
            };
            return self
                .execute_loop(
                    &resolved_items,
                    loop_ctx,
                    runtime,
                    handlers,
                    notified,
                    parallelization_manager,
                    module_registry,
                    batch_processor,
                    pipelining,
                )
                .await;
        }

        // Execute the module - use fact_storage_ctx for set_fact to ensure facts go to right host
        let module_ctx = if canonical_module == "set_fact" {
            &fact_storage_ctx
        } else {
            &execution_ctx
        };

        // Handle until/retries/delay retry logic
        let result = if self.until.is_some() {
            self.execute_with_retry(
                module_ctx,
                runtime,
                handlers,
                notified,
                parallelization_manager,
                module_registry,
            )
            .await?
        } else {
            self.execute_module(
                module_ctx,
                runtime,
                handlers,
                notified,
                parallelization_manager,
                module_registry,
            )
            .await?
        };

        if result.status == TaskStatus::Unreachable {
            if let Some(name) = &self.register {
                self.register_result(name, &result, ctx, runtime).await?;
            }
            return Ok(result);
        }

        // Extract and store ansible_facts from module results
        // Many modules (like gather_facts, setup, etc.) return facts in their result
        if let Some(ref result_data) = result.result {
            if let Some(ansible_facts) = result_data.get("ansible_facts") {
                if let Some(facts_obj) = ansible_facts.as_object() {
                    let mut rt = runtime.write().await;
                    let fact_target = &ctx.host;
                    for (key, value) in facts_obj {
                        rt.set_host_fact(fact_target, key.clone(), value.clone());
                        debug!(
                            "Stored fact '{}' from module result for host '{}'",
                            key, fact_target
                        );
                    }
                }
            }
        }

        // Conditions must see this attempt's result, not a stale previous registration.
        if let Some(ref register_name) = self.register {
            self.register_result(register_name, &result, ctx, runtime)
                .await?;
        }

        // Apply changed_when override - use execution context for condition evaluation
        let result = self
            .apply_changed_when(result, &execution_ctx, runtime)
            .await?;

        // Apply failed_when override - use execution context for condition evaluation
        let result = self
            .apply_failed_when(result, &execution_ctx, runtime)
            .await?;

        // Register result if needed - always register on the original host
        if let Some(ref register_name) = self.register {
            self.register_result(register_name, &result, ctx, runtime)
                .await?;
        }

        // Notify handlers if task changed
        if result.changed && !matches!(result.status, TaskStatus::Failed | TaskStatus::Unreachable)
        {
            for handler_name in &self.notify {
                let mut notified = notified.lock().await;
                notified.insert(
                    serde_json::to_string(&(&ctx.host, handler_name))
                        .expect("notification tuple is serializable"),
                );
                debug!("Notified handler: {}", handler_name);
            }
        }

        // Handle ignore_errors
        if result.status == TaskStatus::Failed && self.ignore_errors {
            warn!("Task failed but ignore_errors is set");
            return Ok(TaskResult {
                status: TaskStatus::Ok,
                changed: false,
                msg: Some(format!("Ignored error: {}", result.msg.unwrap_or_default())),
                result: result.result,
                diff: result.diff,
            });
        }

        Ok(result)
    }

    /// Execute task in a loop
    async fn execute_loop(
        &self,
        items: &[JsonValue],
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
        handlers: &Arc<RwLock<HashMap<String, Handler>>>,
        notified: &Arc<Mutex<std::collections::HashSet<String>>>,
        parallelization_manager: &Arc<ParallelizationManager>,
        module_registry: &Arc<ModuleRegistry>,
        _batch_processor: &Arc<crate::executor::batch_processor::BatchProcessor>,
        _pipelining: bool,
    ) -> ExecutorResult<TaskResult> {
        let total_items = items.len();
        debug!("Executing loop with {} items", total_items);

        // Execute every item independently. The former batch path rewrote command
        // arguments and fabricated per-item outcomes; re-enable only after semantic
        // equivalence (including conditions, transport and failures) is verified.

        // Pre-allocate with known capacity
        let mut loop_results = Vec::with_capacity(total_items);
        let mut any_changed = false;
        let mut any_failed = false;
        let mut any_unreachable = false;

        // Extract loop_control options - avoid repeated Option access in loop
        let loop_control = self.loop_control.as_ref();
        let pause_seconds = loop_control.and_then(|lc| lc.pause);
        let index_var = loop_control.and_then(|lc| lc.index_var.as_ref());
        let extended = loop_control.map(|lc| lc.extended).unwrap_or(false);

        // Pre-allocate static string keys to avoid repeated allocations in loop
        static ANSIBLE_LOOP_KEY: &str = "ansible_loop";

        for (index, item) in items.iter().enumerate() {
            // Pause between iterations (but not before the first)
            if index > 0 {
                if let Some(pause) = pause_seconds {
                    if pause > 0 {
                        debug!("Pausing {} seconds between loop iterations", pause);
                        tokio::time::sleep(tokio::time::Duration::from_secs(pause)).await;
                    }
                }
            }

            // Set loop variables
            {
                let mut rt = runtime.write().await;
                // Clone loop_var only once per loop iteration (unavoidable for runtime storage)
                rt.set_task_var(self.loop_var.clone(), item.clone());

                // Set index_var if specified - avoid clone when possible
                if let Some(idx_var) = index_var {
                    rt.set_task_var(idx_var.clone(), serde_json::json!(index));
                }

                // Build ansible_loop object
                let mut ansible_loop = serde_json::json!({
                    "index": index + 1,  // 1-based index
                    "index0": index,     // 0-based index
                    "first": index == 0,
                    "last": index == total_items - 1,
                    "length": total_items,
                });

                // Add extended loop info if enabled
                if extended {
                    let revindex = total_items - index; // 1-based reverse index
                    let revindex0 = total_items - index - 1; // 0-based reverse index
                    let loop_obj = ansible_loop.as_object_mut().unwrap();
                    loop_obj.insert("revindex".to_string(), serde_json::json!(revindex));
                    loop_obj.insert("revindex0".to_string(), serde_json::json!(revindex0));
                    loop_obj.insert("allitems".to_string(), serde_json::json!(items));
                    loop_obj.insert(
                        "previtem".to_string(),
                        if index > 0 {
                            items[index - 1].clone()
                        } else {
                            JsonValue::Null
                        },
                    );
                    loop_obj.insert(
                        "nextitem".to_string(),
                        if index < total_items - 1 {
                            items[index + 1].clone()
                        } else {
                            JsonValue::Null
                        },
                    );
                }

                rt.set_task_var(ANSIBLE_LOOP_KEY.to_string(), ansible_loop);
            }

            // Execute for this item with parallelization enforcement
            let result = self
                .execute_module(
                    ctx,
                    runtime,
                    handlers,
                    notified,
                    parallelization_manager,
                    module_registry,
                )
                .await?;

            // Extract and store ansible_facts from module results in loops
            if let Some(ref result_data) = result.result {
                if let Some(ansible_facts) = result_data.get("ansible_facts") {
                    if let Some(facts_obj) = ansible_facts.as_object() {
                        let mut rt = runtime.write().await;
                        for (key, value) in facts_obj {
                            rt.set_host_fact(&ctx.host, key.clone(), value.clone());
                            debug!(
                                "Stored fact '{}' from loop iteration for host '{}'",
                                key, ctx.host
                            );
                        }
                    }
                }
            }

            if result.changed {
                any_changed = true;
            }
            if matches!(result.status, TaskStatus::Failed | TaskStatus::Unreachable) {
                any_failed = true;
                any_unreachable |= result.status == TaskStatus::Unreachable;
                if any_unreachable || !self.ignore_errors {
                    // Stop on first failure unless ignore_errors
                    loop_results.push(result.to_registered(None, None));
                    break;
                }
            }

            loop_results.push(result.to_registered(None, None));
        }

        // Clear only the loop-specific variables, preserving other task vars
        // This allows for future nested loop support
        {
            let mut rt = runtime.write().await;
            let mut vars_to_clear = vec![self.loop_var.as_str(), "ansible_loop"];
            if let Some(idx_var) = index_var {
                vars_to_clear.push(idx_var.as_str());
            }
            rt.remove_task_vars(&vars_to_clear);
        }

        // Create combined result
        let status = if any_unreachable {
            TaskStatus::Unreachable
        } else if any_failed && !self.ignore_errors {
            TaskStatus::Failed
        } else if any_changed {
            TaskStatus::Changed
        } else {
            TaskStatus::Ok
        };

        let result = TaskResult {
            status,
            changed: any_changed,
            msg: Some(format!("Completed {} loop iterations", loop_results.len())),
            result: Some(serde_json::to_value(&loop_results).unwrap_or(JsonValue::Null)),
            diff: None,
        };

        // Register combined result if needed
        if let Some(ref register_name) = self.register {
            let mut registered = RegisteredResult::ok(any_changed);
            registered.failed = any_unreachable || (any_failed && !self.ignore_errors);
            registered.results = Some(loop_results);

            let mut rt = runtime.write().await;
            rt.register_result(&ctx.host, register_name.clone(), registered);
        }

        // Notify handlers if anything changed
        if any_changed && !any_failed {
            for handler_name in &self.notify {
                let mut n = notified.lock().await;
                n.insert(
                    serde_json::to_string(&(&ctx.host, handler_name))
                        .expect("notification tuple is serializable"),
                );
            }
        }

        Ok(result)
    }

    /// Execute task with until/retries/delay retry logic
    async fn execute_with_retry(
        &self,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
        handlers: &Arc<RwLock<HashMap<String, Handler>>>,
        notified: &Arc<Mutex<std::collections::HashSet<String>>>,
        parallelization_manager: &Arc<ParallelizationManager>,
        module_registry: &Arc<ModuleRegistry>,
    ) -> ExecutorResult<TaskResult> {
        let max_retries = self.retries.unwrap_or(3);
        let delay_seconds = self.delay.unwrap_or(5);
        let until_condition = self.until.as_ref().expect("until condition must be set");

        debug!(
            "Executing with retry: max_retries={}, delay={}s, until='{}'",
            max_retries, delay_seconds, until_condition
        );

        let mut last_result: Option<TaskResult>;
        let mut attempt = 0;

        loop {
            attempt += 1;
            debug!("Retry attempt {} of {}", attempt, max_retries + 1);

            // Execute the module
            let result = self
                .execute_module(
                    ctx,
                    runtime,
                    handlers,
                    notified,
                    parallelization_manager,
                    module_registry,
                )
                .await?;

            if result.status == TaskStatus::Unreachable {
                return Ok(result);
            }

            // Extract and store ansible_facts from module results during retries
            if let Some(ref result_data) = result.result {
                if let Some(ansible_facts) = result_data.get("ansible_facts") {
                    if let Some(facts_obj) = ansible_facts.as_object() {
                        let mut rt = runtime.write().await;
                        for (key, value) in facts_obj {
                            rt.set_host_fact(&ctx.host, key.clone(), value.clone());
                            debug!(
                                "Stored fact '{}' from retry attempt {} for host '{}'",
                                key, attempt, ctx.host
                            );
                        }
                    }
                }
            }

            // Register the result for condition evaluation
            if let Some(ref register_name) = self.register {
                self.register_result(register_name, &result, ctx, runtime)
                    .await?;
            }

            // Evaluate the until condition
            let condition_met = self
                .evaluate_condition(until_condition, ctx, runtime)
                .await?;

            if condition_met {
                debug!(
                    "Until condition '{}' met after {} attempt(s)",
                    until_condition, attempt
                );
                return Ok(result);
            }

            // Store the last result
            last_result = Some(result);

            // Check if we've exhausted retries
            if attempt > max_retries {
                debug!("Max retries ({}) exhausted, condition not met", max_retries);
                break;
            }

            // Wait before retrying
            if delay_seconds > 0 {
                debug!("Waiting {} seconds before retry", delay_seconds);
                tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;
            }
        }

        // Return failure after exhausting retries
        Ok(TaskResult {
            status: TaskStatus::Failed,
            changed: false,
            msg: Some(format!(
                "Retries exhausted ({}). Until condition '{}' never met",
                max_retries, until_condition
            )),
            result: last_result.as_ref().and_then(|r| r.result.clone()),
            diff: None,
        })
    }

    /// Execute the actual module
    async fn execute_module(
        &self,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
        handlers: &Arc<RwLock<HashMap<String, Handler>>>,
        notified: &Arc<Mutex<std::collections::HashSet<String>>>,
        parallelization_manager: &Arc<ParallelizationManager>,
        module_registry: &Arc<ModuleRegistry>,
    ) -> ExecutorResult<TaskResult> {
        // Template the arguments
        let args = self.template_args(ctx, runtime).await?;

        debug!("Module: {}", self.module);

        // Enforce parallelization constraints based on module hint
        // Get the module's parallelization hint from the shared registry (avoids rebuilding)
        let hint = {
            if let Some(module) = module_registry.get(&self.module) {
                module.parallelization_hint()
            } else {
                // For unknown modules (Python fallback), use FullyParallel as default
                crate::modules::ParallelizationHint::FullyParallel
            }
        };

        // Acquire parallelization guard - this will block if necessary based on the hint
        // The guard is automatically released when it goes out of scope (when this function returns)
        // Every explicitly local inventory entry is the same machine, so aliases
        // such as web1 and web2 with `ansible_connection: local` must never run
        // module code at the same time: package databases, crontabs and other
        // controller state would race. Modules that declared no constraint are
        // therefore made host-exclusive on local targets, and every local target
        // shares one lock key.
        // The include wrappers run their children through this same manager while
        // holding their own guard, so they must never take the shared lock.
        const LOCAL_TARGET_LOCK_KEY: &str = "localhost";
        // Dispatch on the same canonical name the registry resolves, so the
        // guards below see `copy` for `ansible.legacy.copy` as well.
        let module_name = ModuleRegistry::normalize_module_name(&self.module);
        let include_wrapper = matches!(module_name, "include_tasks" | "import_tasks");
        let connection_kind = self.configured_transport(ctx, runtime).await;
        let (hint, lock_host) = if Self::is_local_target(connection_kind.as_deref(), ctx) {
            let hint = match hint {
                crate::modules::ParallelizationHint::FullyParallel if !include_wrapper => {
                    crate::modules::ParallelizationHint::HostExclusive
                }
                other => other,
            };
            (hint, LOCAL_TARGET_LOCK_KEY)
        } else {
            (hint, ctx.host.as_str())
        };
        let _parallelization_guard = parallelization_manager
            .acquire(hint, lock_host, &self.module)
            .await;

        match module_name {
            "debug" => self.execute_debug(&args, ctx).await,
            "set_fact" => self.execute_set_fact(&args, ctx, runtime).await,
            "fail" => self.execute_fail(&args).await,
            "assert" => self.execute_assert(&args, ctx, runtime).await,
            "pause" => self.execute_pause(&args).await,
            "include_vars" => self.execute_include_vars(&args, ctx, runtime).await,
            "include_tasks" | "import_tasks" => {
                self.execute_include_tasks(
                    &args,
                    ctx,
                    runtime,
                    handlers,
                    notified,
                    parallelization_manager,
                    module_registry,
                )
                .await
            }
            "meta" => self.execute_meta(&args).await,
            _ => {
                self.execute_native(module_name, &args, ctx, runtime, module_registry)
                    .await
            }
        }
    }

    /// The transport configured for this task's host: a task-level
    /// `ansible_connection` override, else the inventory value.
    async fn configured_transport(
        &self,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> Option<String> {
        let runtime = runtime.read().await;
        self.vars
            .get("ansible_connection")
            .map(|value| {
                value
                    .as_str()
                    .unwrap_or("__invalid_transport__")
                    .to_string()
            })
            .or_else(|| runtime.configured_connection(&ctx.host))
    }

    /// Whether the task executes on the control node itself.
    fn is_local_target(connection_kind: Option<&str>, ctx: &ExecutionContext) -> bool {
        connection_kind == Some("local")
            || (connection_kind.is_none()
                && ctx.connection.is_none()
                && matches!(ctx.host.as_str(), "localhost" | "127.0.0.1" | "::1"))
    }

    /// Native modules may run locally only for an explicitly local inventory target.
    /// A missing or unsupported remote connection must never become a local fallback.
    async fn execute_native(
        &self,
        module_name: &str,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
        registry: &Arc<ModuleRegistry>,
    ) -> ExecutorResult<TaskResult> {
        let vars = runtime.read().await.get_merged_vars(&ctx.host);
        let connection_kind = self.configured_transport(ctx, runtime).await;
        let local = Self::is_local_target(connection_kind.as_deref(), ctx);
        if local && module_name == "get_url" {
            return Ok(TaskResult::failed(
                "Local get_url destination writes are not implemented; refusing execution",
            ));
        }
        if !local && ctx.connection.is_none() {
            return Ok(TaskResult::unreachable(
                "Remote execution requires an established connection; local fallback is disabled",
            ));
        }
        // These implementations have a reviewed transport path. Classification alone
        // is not sufficient: several filesystem modules ignore ModuleContext.connection.
        if !local
            && !matches!(
                module_name,
                "command" | "shell" | "copy" | "template" | "gather_facts" | "setup"
            )
        {
            return Ok(TaskResult::failed(format!(
                "Module '{module_name}' does not have a verified remote transport"
            )));
        }
        if ctx.r#become && (local || !matches!(module_name, "command" | "shell")) {
            return Ok(TaskResult::failed(
                "Privilege escalation is not verified for this module; refusing execution",
            ));
        }
        if module_name == "gather_facts" || module_name == "setup" {
            return self.execute_gather_facts(args, ctx).await;
        }
        if !registry.contains(module_name) {
            return Ok(TaskResult::failed(format!(
                "Module '{module_name}' is unavailable; Python fallback is not implemented"
            )));
        }
        let mut facts = std::collections::HashMap::new();
        if local {
            facts.insert(
                "os_family".to_string(),
                JsonValue::String(if cfg!(windows) { "Windows" } else { "Unix" }.to_string()),
            );
        } else if let Some(value) = vars
            .get("ansible_os_family")
            .or_else(|| vars.get("os_family"))
        {
            facts.insert("os_family".to_string(), value.clone());
        }
        // Modules with their own local execution path keep running on the control
        // node without a connection object: their remote path would drop local-only
        // behaviour (copy's `validate` staging, shell's `stdin`, chdir-relative
        // `creates`/`removes`). Every other module only knows how to run through a
        // connection, so an explicitly local target hands it a real local connection
        // instead of failing with "No connection available". A connection the
        // context carries for remote use is never reused for local execution.
        const LOCAL_DISPATCH_MODULES: &[&str] = &[
            "authorized_key",
            "command",
            "copy",
            "lineinfile",
            "shell",
            "stat",
            "template",
        ];
        let connection: Option<Arc<dyn crate::connection::Connection + Send + Sync>> = if local {
            if LOCAL_DISPATCH_MODULES.contains(&module_name) {
                None
            } else {
                Some(Arc::new(crate::connection::local::LocalConnection::new()))
            }
        } else {
            ctx.connection.clone()
        };
        let module_ctx = crate::modules::ModuleContext {
            check_mode: ctx.check_mode,
            diff_mode: ctx.diff_mode,
            verbosity: ctx.verbosity,
            vars: vars.into_iter().collect(),
            facts,
            work_dir: None,
            r#become: ctx.r#become,
            become_method: Some(ctx.r#become_method.clone()),
            become_user: Some(ctx.r#become_user.clone()),
            become_password: ctx.become_password.clone(),
            connection,
        };
        let params = args.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let registry = Arc::clone(registry);
        let module_name = module_name.to_string();
        // Native modules use synchronous APIs, including connection bridges. Keep
        // those calls off the async scheduler to avoid blocking unrelated hosts.
        let output = tokio::task::spawn_blocking(move || {
            registry.execute(&module_name, &params, &module_ctx)
        })
        .await
        .map_err(|e| ExecutorError::RuntimeError(format!("Native module worker failed: {e}")))?;
        match output {
            Ok(output) => {
                let result = output.to_result_json();
                let status = match output.status {
                    crate::modules::ModuleStatus::Failed => TaskStatus::Failed,
                    crate::modules::ModuleStatus::Skipped => TaskStatus::Skipped,
                    _ if output.changed => TaskStatus::Changed,
                    _ => TaskStatus::Ok,
                };
                Ok(TaskResult {
                    status,
                    changed: output.changed,
                    msg: Some(output.msg),
                    result: Some(result),
                    diff: output.diff.map(|d| TaskDiff {
                        before: Some(d.before),
                        after: Some(d.after),
                        before_header: None,
                        after_header: None,
                    }),
                })
            }
            Err(crate::modules::ModuleError::CommandFailed { code, message }) => {
                Ok(TaskResult::failed(message).with_result(serde_json::json!({"rc": code})))
            }
            Err(error) => Ok(TaskResult::failed(error.to_string())),
        }
    }

    /// Template arguments using variables
    async fn template_args(
        &self,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<IndexMap<String, JsonValue>> {
        let rt = runtime.read().await;
        let vars = rt.get_merged_vars(&ctx.host);
        let mut result = IndexMap::new();

        let engine = get_engine();
        for (key, value) in &self.args {
            let templated = engine.render_value(value, &vars)?;
            result.insert(key.clone(), templated.into_owned());
        }

        Ok(result)
    }

    /// Evaluate a when condition
    pub(crate) async fn evaluate_condition(
        &self,
        condition: &str,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<bool> {
        let rt = runtime.read().await;
        let vars = rt.get_merged_vars(&ctx.host);

        get_engine()
            .evaluate_condition(condition, &vars)
            .map_err(ExecutorError::from)
    }

    /// Apply changed_when override
    async fn apply_changed_when(
        &self,
        mut result: TaskResult,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<TaskResult> {
        if let Some(ref condition) = self.changed_when {
            let should_be_changed = self.evaluate_condition(condition, ctx, runtime).await?;
            result.changed = should_be_changed;
            if matches!(result.status, TaskStatus::Ok | TaskStatus::Changed) {
                result.status = if should_be_changed {
                    TaskStatus::Changed
                } else {
                    TaskStatus::Ok
                };
            }
        }
        Ok(result)
    }

    /// Apply failed_when override
    async fn apply_failed_when(
        &self,
        mut result: TaskResult,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<TaskResult> {
        if let Some(ref condition) = self.failed_when {
            let should_fail = self.evaluate_condition(condition, ctx, runtime).await?;
            if !should_fail && result.status == TaskStatus::Failed {
                result.status = if result.changed {
                    TaskStatus::Changed
                } else {
                    TaskStatus::Ok
                };
            }
            if should_fail {
                result.status = TaskStatus::Failed;
                result.msg = Some(format!(
                    "Failed due to failed_when condition: {}",
                    condition
                ));
            }
        }
        Ok(result)
    }

    /// Register task result
    async fn register_result(
        &self,
        name: &str,
        result: &TaskResult,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<()> {
        let registered = result.to_registered(None, None);

        let mut rt = runtime.write().await;
        rt.register_result(&ctx.host, name.to_string(), registered);

        Ok(())
    }

    // Module implementations

    async fn execute_debug(
        &self,
        args: &IndexMap<String, JsonValue>,
        _ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        if let Some(msg) = args.get("msg") {
            info!("DEBUG: {}", msg);
            Ok(TaskResult::ok().with_msg(format!("{}", msg)))
        } else if let Some(var) = args.get("var") {
            info!("DEBUG: {} = {:?}", var, var);
            Ok(TaskResult::ok().with_result(var.clone()))
        } else {
            Ok(TaskResult::ok())
        }
    }

    async fn execute_gather_facts(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        use crate::modules::{Module, ModuleContext};

        // Get gather_subset from args if provided
        let gather_subset = args
            .get("gather_subset")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            });

        // Check if we have a remote connection - if so, gather facts remotely
        if let Some(ref connection) = ctx.connection {
            debug!(
                host = %ctx.host,
                "Gathering facts remotely via connection"
            );

            // Gather facts via the connection
            let facts = crate::modules::facts::gather_facts_via_connection(
                connection,
                gather_subset.as_deref(),
            )
            .await;

            let mut result = TaskResult::ok();
            result.msg = Some("Facts gathered successfully (remote)".to_string());

            // Wrap facts in ansible_facts key for compatibility
            let mut data = std::collections::HashMap::new();
            let facts_json: serde_json::Map<String, serde_json::Value> =
                facts.into_iter().collect();
            data.insert(
                "ansible_facts".to_string(),
                serde_json::Value::Object(facts_json),
            );

            result.result = Some(serde_json::to_value(&data).unwrap_or_default());

            return Ok(result);
        }

        // No connection or local connection - use local facts gathering
        debug!(
            host = %ctx.host,
            "Gathering facts locally"
        );

        // Convert args to ModuleParams
        let mut params: std::collections::HashMap<String, serde_json::Value> =
            std::collections::HashMap::new();
        if let Some(subset) = gather_subset {
            params.insert("gather_subset".to_string(), serde_json::json!(subset));
        }

        // Create module context
        let module_ctx = ModuleContext::default().with_verbosity(ctx.verbosity);

        // Execute the facts module locally
        let facts_module = crate::modules::facts::FactsModule;
        match facts_module.execute(&params, &module_ctx) {
            Ok(output) => {
                let mut result = TaskResult::ok();
                result.msg = Some(output.msg.clone());

                // Include ansible_facts in the result so they can be stored
                if !output.data.is_empty() {
                    result.result = Some(serde_json::to_value(&output.data).unwrap_or_default());
                }

                Ok(result)
            }
            Err(e) => Err(ExecutorError::TaskFailed(format!(
                "gather_facts failed: {}",
                e
            ))),
        }
    }

    async fn execute_set_fact(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<TaskResult> {
        let mut rt = runtime.write().await;

        let mut facts_set = Vec::new();

        // Determine the target host for fact storage based on delegation
        // Note: ctx.host is already set to the delegated host if delegation is active
        // The caller (execute method) handles the delegation logic and passes the
        // appropriate host context
        let fact_target = &ctx.host;

        for (key, value) in args {
            if key != "cacheable" {
                // Use set_host_fact instead of set_host_var for proper precedence
                // Facts set by set_fact should have SetFact precedence level
                rt.set_host_fact(fact_target, key.clone(), value.clone());
                debug!(
                    "Set fact '{}' = {:?} for host '{}'",
                    key, value, fact_target
                );
                facts_set.push(key.clone());
            }
        }

        let message = if facts_set.len() == 1 {
            format!("Set fact: {}", facts_set[0])
        } else {
            format!("Set {} facts: {}", facts_set.len(), facts_set.join(", "))
        };

        Ok(TaskResult::ok().with_msg(message))
    }

    async fn execute_fail(&self, args: &IndexMap<String, JsonValue>) -> ExecutorResult<TaskResult> {
        let msg = args
            .get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("Failed as requested");

        Ok(TaskResult::failed(msg))
    }

    async fn execute_assert(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<TaskResult> {
        let that = args
            .get("that")
            .ok_or_else(|| ExecutorError::RuntimeError("assert requires 'that' argument".into()))?;

        let conditions: Vec<&str> = match that {
            JsonValue::String(s) => vec![s.as_str()],
            JsonValue::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
            _ => {
                return Err(ExecutorError::RuntimeError(
                    "assert 'that' must be string or array".into(),
                ))
            }
        };

        for condition in conditions {
            let result = self.evaluate_condition(condition, ctx, runtime).await?;
            if !result {
                let fail_msg = args
                    .get("fail_msg")
                    .or_else(|| args.get("msg"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Assertion failed");

                return Ok(TaskResult::failed(format!("{}: {}", fail_msg, condition)));
            }
        }

        let success_msg = args
            .get("success_msg")
            .and_then(|v| v.as_str())
            .unwrap_or("All assertions passed");

        Ok(TaskResult::ok().with_msg(success_msg))
    }

    async fn execute_pause(
        &self,
        args: &IndexMap<String, JsonValue>,
    ) -> ExecutorResult<TaskResult> {
        let seconds = args.get("seconds").and_then(|v| v.as_u64()).unwrap_or(0);

        if seconds > 0 {
            debug!("Pausing for {} seconds", seconds);
            tokio::time::sleep(tokio::time::Duration::from_secs(seconds)).await;
        }

        Ok(TaskResult::ok().with_msg(format!("Paused for {} seconds", seconds)))
    }

    /// Validate that a path is safe and within the allowed base directory.
    ///
    /// This function prevents path traversal attacks by:
    /// 1. Rejecting paths containing ".." traversal components
    /// 2. Canonicalizing paths to resolve symlinks
    /// 3. Ensuring the resolved path stays within the base directory
    ///
    /// # Security
    ///
    /// This is a critical security function. All file operations that load
    /// external content (variables, tasks, etc.) MUST use this validation
    /// to prevent unauthorized file access.
    fn validate_include_path(
        requested_path: &str,
        base_path: &std::path::Path,
    ) -> ExecutorResult<std::path::PathBuf> {
        use std::path::{Path, PathBuf};

        // Early rejection of obvious path traversal attempts
        // Check for ".." in path components (handles both Unix and Windows separators)
        if requested_path.contains("..") {
            warn!(
                "Security: Rejecting path traversal attempt in include_vars: '{}'",
                requested_path
            );
            return Err(ExecutorError::RuntimeError(format!(
                "Security violation: Path traversal detected in '{}'. \
                 Paths containing '..' are not allowed for security reasons.",
                requested_path
            )));
        }

        let path = Path::new(requested_path);

        // Construct the full path
        let full_path = if path.is_absolute() {
            PathBuf::from(requested_path)
        } else {
            base_path.join(requested_path)
        };

        // Check if the path exists before canonicalizing
        if !full_path.exists() {
            return Err(ExecutorError::RuntimeError(format!(
                "include_vars path not found: {}",
                full_path.display()
            )));
        }

        // Canonicalize base path for comparison
        let canonical_base = base_path.canonicalize().map_err(|e| {
            ExecutorError::RuntimeError(format!(
                "Failed to resolve base path '{}': {}",
                base_path.display(),
                e
            ))
        })?;

        // Canonicalize the requested path to resolve symlinks and normalize
        let canonical_path = full_path.canonicalize().map_err(|e| {
            ExecutorError::RuntimeError(format!(
                "Failed to resolve include_vars path '{}': {}",
                full_path.display(),
                e
            ))
        })?;

        // Security check: ensure the canonical path is within the base directory
        if !canonical_path.starts_with(&canonical_base) {
            warn!(
                "Security: Path traversal blocked - '{}' (resolved to '{}') escapes base '{}'",
                requested_path,
                canonical_path.display(),
                canonical_base.display()
            );
            return Err(ExecutorError::RuntimeError(format!(
                "Security violation: Path '{}' resolves to '{}' which is outside \
                 the allowed directory '{}'. This may indicate a path traversal attack.",
                requested_path,
                canonical_path.display(),
                canonical_base.display()
            )));
        }

        debug!(
            "Path validated: '{}' -> '{}' (within '{}')",
            requested_path,
            canonical_path.display(),
            canonical_base.display()
        );

        Ok(canonical_path)
    }

    async fn execute_include_vars(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<TaskResult> {
        // Get file or dir parameter
        let file = args
            .get("file")
            .or_else(|| args.get("_raw_params"))
            .and_then(|v| v.as_str());
        let dir = args.get("dir").and_then(|v| v.as_str());
        let name = args.get("name").and_then(|v| v.as_str());

        if file.is_none() && dir.is_none() {
            return Err(ExecutorError::RuntimeError(
                "include_vars requires 'file' or 'dir' parameter".into(),
            ));
        }

        if file.is_some() && dir.is_some() {
            return Err(ExecutorError::RuntimeError(
                "include_vars cannot have both 'file' and 'dir' parameters".into(),
            ));
        }

        // Determine base path from playbook directory, falling back to current directory
        let base_path = {
            let rt = runtime.read().await;
            rt.get_playbook_dir().unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            })
        };

        let mut all_vars: IndexMap<String, JsonValue> = IndexMap::new();
        let source: String;

        if let Some(file_path) = file {
            // Validate and resolve the file path with security checks
            let resolved_path = Self::validate_include_path(file_path, &base_path)?;

            let content = tokio::fs::read_to_string(&resolved_path)
                .await
                .map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to read include_vars file {}: {}",
                        resolved_path.display(),
                        e
                    ))
                })?;

            // Parse as YAML (which also handles JSON)
            let vars: IndexMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)
                .map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to parse include_vars file {}: {}",
                        resolved_path.display(),
                        e
                    ))
                })?;

            // Convert YAML values to JSON values
            for (key, value) in vars {
                let json_value = serde_json::to_value(&value).map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to convert variable {}: {}",
                        key, e
                    ))
                })?;
                all_vars.insert(key, json_value);
            }

            source = resolved_path.display().to_string();
        } else if let Some(dir_path) = dir {
            // Validate and resolve the directory path with security checks
            let resolved_path = Self::validate_include_path(dir_path, &base_path)?;

            if !resolved_path.is_dir() {
                return Err(ExecutorError::RuntimeError(format!(
                    "include_vars path is not a directory: {}",
                    resolved_path.display()
                )));
            }

            // Read and sort files by name for predictable ordering
            let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&resolved_path)
                .map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to read directory {}: {}",
                        resolved_path.display(),
                        e
                    ))
                })?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && (p.extension() == Some("yml".as_ref())
                            || p.extension() == Some("yaml".as_ref())
                            || p.extension() == Some("json".as_ref()))
                })
                .collect();

            files.sort();

            // Validate each file in the directory is within the base path
            // This protects against symlink attacks within the directory
            for file_path in &files {
                let canonical_file = file_path.canonicalize().map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to resolve file path '{}': {}",
                        file_path.display(),
                        e
                    ))
                })?;

                let canonical_base = base_path.canonicalize().map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to resolve base path '{}': {}",
                        base_path.display(),
                        e
                    ))
                })?;

                if !canonical_file.starts_with(&canonical_base) {
                    warn!(
                        "Security: Symlink escape blocked - '{}' (resolved to '{}') escapes base '{}'",
                        file_path.display(),
                        canonical_file.display(),
                        canonical_base.display()
                    );
                    return Err(ExecutorError::RuntimeError(format!(
                        "Security violation: File '{}' in include_vars directory resolves to '{}' \
                         which is outside the allowed directory '{}'. This may indicate a symlink attack.",
                        file_path.display(),
                        canonical_file.display(),
                        canonical_base.display()
                    )));
                }
            }

            // Load each file and merge variables
            for file_path in &files {
                let content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to read file {}: {}",
                        file_path.display(),
                        e
                    ))
                })?;

                let vars: IndexMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)
                    .map_err(|e| {
                        ExecutorError::RuntimeError(format!(
                            "Failed to parse file {}: {}",
                            file_path.display(),
                            e
                        ))
                    })?;

                for (key, value) in vars {
                    let json_value = serde_json::to_value(&value).map_err(|e| {
                        ExecutorError::RuntimeError(format!(
                            "Failed to convert variable {}: {}",
                            key, e
                        ))
                    })?;
                    all_vars.insert(key, json_value);
                }
            }

            source = format!("{}/*.yml", resolved_path.display());
        } else {
            return Err(ExecutorError::RuntimeError(
                "include_vars requires 'file' or 'dir' parameter".into(),
            ));
        }

        // If 'name' parameter is specified, scope all variables under that key
        let final_vars = if let Some(scope_name) = name {
            let mut scoped = IndexMap::new();
            scoped.insert(
                scope_name.to_string(),
                JsonValue::Object(all_vars.into_iter().collect()),
            );
            scoped
        } else {
            all_vars
        };

        let var_count = final_vars.len();

        // Store variables in the runtime context for the current host
        {
            let mut rt = runtime.write().await;
            for (key, value) in &final_vars {
                rt.set_host_var(&ctx.host, key.clone(), value.clone());
            }
        }

        info!(
            "Loaded {} variable(s) from {} for host {}",
            var_count, source, ctx.host
        );

        Ok(TaskResult::ok().with_msg(format!("Loaded {} variable(s) from {}", var_count, source)))
    }

    async fn execute_include_tasks(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
        handlers: &Arc<RwLock<HashMap<String, Handler>>>,
        notified: &Arc<Mutex<std::collections::HashSet<String>>>,
        parallelization_manager: &Arc<ParallelizationManager>,
        module_registry: &Arc<ModuleRegistry>,
    ) -> ExecutorResult<TaskResult> {
        let file = args
            .get("file")
            .or_else(|| args.get("_raw_params"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutorError::RuntimeError("include_tasks requires file path".into())
            })?;

        info!("Including tasks from: {}", file);

        // Determine base path from the playbook directory, falling back to current directory
        let base_path = {
            let rt = runtime.read().await;
            rt.get_playbook_dir().unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            })
        };
        let handler = crate::executor::include_handler::IncludeTasksHandler::new(base_path);

        // Build the include spec with any variables passed
        let mut spec = crate::include::IncludeTasksSpec::new(file);

        // Add any variables passed to include_tasks
        if let Some(vars) = args.get("vars").and_then(|v| v.as_object()) {
            for (key, value) in vars {
                spec = spec.with_var(key, value.clone());
            }
        }

        // Load tasks from the file (returns playbook::Task)
        let playbook_tasks = handler
            .load_include_tasks(&spec, runtime, &ctx.host)
            .await
            .map_err(|e| {
                ExecutorError::RuntimeError(format!("Failed to load include_tasks: {}", e))
            })?;

        debug!("Loaded {} tasks from {}", playbook_tasks.len(), file);

        // Convert playbook::Task to executor::task::Task and execute
        let mut total_changed = false;
        let mut task_count = 0;
        let mut failed = false;
        let mut unreachable = false;

        for playbook_task in playbook_tasks {
            // Convert to executor task
            let executor_task: Task = playbook_task.into();
            // Use Box::pin to handle async recursion
            // Included tasks get their own default batch processor
            let include_batch_processor =
                Arc::new(crate::executor::batch_processor::BatchProcessor::new(
                    crate::executor::batch_processor::BatchConfig::default(),
                ));
            let result = Box::pin(executor_task.execute(
                ctx,
                runtime,
                handlers,
                notified,
                parallelization_manager,
                module_registry,
                &include_batch_processor,
                true, // pipelining enabled for included tasks
            ))
            .await?;

            task_count += 1;
            if result.changed {
                total_changed = true;
            }
            if matches!(result.status, TaskStatus::Failed | TaskStatus::Unreachable) {
                unreachable |= result.status == TaskStatus::Unreachable;
                failed = true;
                break;
            }
        }

        if unreachable {
            Ok(TaskResult::unreachable(
                "Included task target is unreachable",
            ))
        } else if failed {
            Ok(TaskResult::failed(format!(
                "Included {} tasks from {}, execution failed",
                task_count, file
            )))
        } else {
            let mut result = if total_changed {
                TaskResult::changed()
            } else {
                TaskResult::ok()
            };
            result.msg = Some(format!("Included {} tasks from {}", task_count, file));
            Ok(result)
        }
    }

    async fn execute_meta(&self, args: &IndexMap<String, JsonValue>) -> ExecutorResult<TaskResult> {
        let action = args
            .get("_raw_params")
            .or_else(|| args.get("action"))
            .and_then(|v| v.as_str())
            .unwrap_or("noop");

        match action {
            "noop" => Ok(TaskResult::ok()),
            _ => Ok(TaskResult::failed(format!(
                "Meta action '{action}' is not supported in this execution context"
            ))),
        }
    }
}

/// Template a value using variables
///
/// # Performance
/// Hot path function with optimizations:
/// - Early return for non-templatable values (numbers, bools, null)
/// - Inline hint for better compiler optimization
#[inline]
fn template_value(
    value: &JsonValue,
    vars: &IndexMap<String, JsonValue>,
) -> ExecutorResult<JsonValue> {
    match value {
        // OPTIMIZATION: Non-templatable primitives - fast path with clone
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => Ok(value.clone()),
        JsonValue::String(s) => {
            // OPTIMIZATION: Fast path if no template syntax
            if !s.contains("{{") {
                return Ok(value.clone());
            }
            let templated = template_string(s, vars)?;
            // Try to parse as JSON if it looks like a value
            if let Ok(parsed) = serde_json::from_str::<JsonValue>(&templated) {
                if !matches!(parsed, JsonValue::Object(_)) {
                    return Ok(parsed);
                }
            }
            Ok(JsonValue::String(templated))
        }
        JsonValue::Array(arr) => {
            let templated: Result<Vec<_>, _> =
                arr.iter().map(|v| template_value(v, vars)).collect();
            Ok(JsonValue::Array(templated?))
        }
        JsonValue::Object(obj) => {
            let mut result = serde_json::Map::new();
            for (k, v) in obj {
                let templated_key = template_string(k, vars)?;
                let templated_value = template_value(v, vars)?;
                result.insert(templated_key, templated_value);
            }
            Ok(JsonValue::Object(result))
        }
    }
}

/// Template a string using variables
///
/// # Performance
/// Uses cached regex pattern (TEMPLATE_VAR_REGEX) to avoid recompilation.
/// For strings without template syntax, returns early to avoid regex overhead.
#[inline]
fn template_string(template: &str, vars: &IndexMap<String, JsonValue>) -> ExecutorResult<String> {
    // OPTIMIZATION: Fast path - if no template syntax, return early
    if !template.contains("{{") {
        return Ok(template.to_string());
    }

    // Simple Jinja2-like templating
    // Handle {{ variable }} syntax
    let mut result = template.to_string();

    // OPTIMIZATION: Use cached regex pattern instead of recompiling
    for cap in TEMPLATE_VAR_REGEX.captures_iter(template) {
        let full_match = cap.get(0).unwrap().as_str();
        let expr = cap.get(1).unwrap().as_str().trim();

        let value = evaluate_variable_expression(expr, vars)?;
        let replacement = json_to_string(&value);
        result = result.replace(full_match, &replacement);
    }

    Ok(result)
}

/// Evaluate a variable expression (e.g., "foo.bar" or "foo['bar']")
///
/// # Performance
/// This is a hot path function - called for every template variable.
/// Uses inline hint and avoids unnecessary allocations.
#[inline]
fn evaluate_variable_expression(
    expr: &str,
    vars: &IndexMap<String, JsonValue>,
) -> ExecutorResult<JsonValue> {
    // Handle simple variable lookup
    let parts: Vec<&str> = expr.split('.').collect();

    if parts.is_empty() {
        return Ok(JsonValue::Null);
    }

    // Get root variable
    let root = parts[0].trim();
    let mut value = vars.get(root).cloned().unwrap_or(JsonValue::Null);

    // Navigate nested properties
    for part in &parts[1..] {
        let key = part.trim();
        value = match &value {
            JsonValue::Object(obj) => obj.get(key).cloned().unwrap_or(JsonValue::Null),
            JsonValue::Array(arr) => {
                if let Ok(idx) = key.parse::<usize>() {
                    arr.get(idx).cloned().unwrap_or(JsonValue::Null)
                } else {
                    JsonValue::Null
                }
            }
            _ => JsonValue::Null,
        };
    }

    Ok(value)
}

/// Convert JSON value to string for templating
///
/// # Performance
/// Hot path function - called for every template variable substitution.
#[inline]
fn json_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => String::new(),
        JsonValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => s.clone(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

/// Find the position of the matching closing parenthesis
///
/// # Performance
/// Used in expression parsing - inline for better optimization.
#[inline]
fn find_matching_paren(expr: &str, open_pos: usize) -> Option<usize> {
    let bytes = expr.as_bytes();
    let mut depth = 1;
    let mut pos = open_pos + 1;

    while pos < bytes.len() {
        match bytes[pos] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            _ => {}
        }
        pos += 1;
    }
    None
}

/// Find position of operator outside parentheses (returns rightmost match for left-associativity)
///
/// # Performance
/// Hot path for expression parsing - inline for better optimization.
#[inline]
fn find_operator_outside_parens(expr: &str, op: &str) -> Option<usize> {
    let mut depth = 0;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    let mut last_match: Option<usize> = None;

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {
                if depth == 0
                    && i + op_bytes.len() <= bytes.len()
                    && &bytes[i..i + op_bytes.len()] == op_bytes
                {
                    last_match = Some(i);
                }
            }
        }
        i += 1;
    }
    last_match
}

/// Compare two JSON values with ordering
///
/// # Performance
/// Inline hint for hot path comparisons.
#[inline]
fn compare_values(left: &JsonValue, right: &JsonValue) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (JsonValue::Number(l), JsonValue::Number(r)) => {
            let lf = l.as_f64()?;
            let rf = r.as_f64()?;
            lf.partial_cmp(&rf)
        }
        (JsonValue::String(l), JsonValue::String(r)) => Some(l.cmp(r)),
        (JsonValue::String(l), JsonValue::Number(r)) => {
            if let Ok(lf) = l.parse::<f64>() {
                lf.partial_cmp(&r.as_f64()?)
            } else {
                None
            }
        }
        (JsonValue::Number(l), JsonValue::String(r)) => {
            if let Ok(rf) = r.parse::<f64>() {
                l.as_f64()?.partial_cmp(&rf)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Compare version strings (e.g., "1.2.3" vs "1.3.0")
fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parse_parts = |v: &str| -> Vec<i64> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<i64>().ok())
            .collect()
    };

    let p1 = parse_parts(v1);
    let p2 = parse_parts(v2);

    for i in 0..std::cmp::max(p1.len(), p2.len()) {
        let n1 = p1.get(i).copied().unwrap_or(0);
        let n2 = p2.get(i).copied().unwrap_or(0);
        match n1.cmp(&n2) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// Evaluate a Jinja2 test expression (e.g., "is string", "is match('pattern')")
fn evaluate_jinja_test(
    value: &JsonValue,
    test_name: &str,
    test_arg: Option<&str>,
    vars: &IndexMap<String, JsonValue>,
) -> bool {
    match test_name {
        "defined" => !value.is_null(),
        "undefined" => value.is_null(),
        "none" | "null" => value.is_null(),
        "true" => matches!(value, JsonValue::Bool(true)),
        "false" => matches!(value, JsonValue::Bool(false)),
        "boolean" | "bool" => matches!(value, JsonValue::Bool(_)),
        "string" => matches!(value, JsonValue::String(_)),
        "number" | "integer" | "float" => matches!(value, JsonValue::Number(_)),
        "mapping" | "dict" => matches!(value, JsonValue::Object(_)),
        "iterable" | "sequence" => matches!(value, JsonValue::Array(_) | JsonValue::String(_)),
        "callable" => false, // Rust values are not callable in Jinja2 sense
        "sameas" => {
            if let Some(arg) = test_arg {
                let other = vars.get(arg.trim()).unwrap_or(&JsonValue::Null);
                std::ptr::eq(value, other) || value == other
            } else {
                false
            }
        }
        "empty" => match value {
            JsonValue::Null => true,
            JsonValue::String(s) => s.is_empty(),
            JsonValue::Array(a) => a.is_empty(),
            JsonValue::Object(o) => o.is_empty(),
            _ => false,
        },
        "even" => {
            if let JsonValue::Number(n) = value {
                n.as_i64().map(|i| i % 2 == 0).unwrap_or(false)
            } else {
                false
            }
        }
        "odd" => {
            if let JsonValue::Number(n) = value {
                n.as_i64().map(|i| i % 2 != 0).unwrap_or(false)
            } else {
                false
            }
        }
        "lower" => {
            if let JsonValue::String(s) = value {
                s.chars().all(|c| !c.is_alphabetic() || c.is_lowercase())
            } else {
                false
            }
        }
        "upper" => {
            if let JsonValue::String(s) = value {
                s.chars().all(|c| !c.is_alphabetic() || c.is_uppercase())
            } else {
                false
            }
        }
        "match" | "regex" => {
            if let (JsonValue::String(s), Some(pattern)) = (value, test_arg) {
                let pattern = pattern.trim().trim_matches(|c| c == '\'' || c == '"');
                crate::utils::get_regex(pattern)
                    .map(|re| re.is_match(s))
                    .unwrap_or(false)
            } else {
                false
            }
        }
        "search" => {
            if let (JsonValue::String(s), Some(pattern)) = (value, test_arg) {
                let pattern = pattern.trim().trim_matches(|c| c == '\'' || c == '"');
                crate::utils::get_regex(pattern)
                    .map(|re| re.find(s).is_some())
                    .unwrap_or(false)
            } else {
                false
            }
        }
        "divisibleby" => {
            if let (JsonValue::Number(n), Some(arg)) = (value, test_arg) {
                let arg = arg.trim().trim_matches(|c| c == '\'' || c == '"');
                if let (Some(val), Ok(div)) = (n.as_i64(), arg.parse::<i64>()) {
                    div != 0 && val % div == 0
                } else {
                    false
                }
            } else {
                false
            }
        }
        "startswith" => {
            if let (JsonValue::String(s), Some(arg)) = (value, test_arg) {
                let prefix = arg.trim().trim_matches(|c| c == '\'' || c == '"');
                s.starts_with(prefix)
            } else {
                false
            }
        }
        "endswith" => {
            if let (JsonValue::String(s), Some(arg)) = (value, test_arg) {
                let suffix = arg.trim().trim_matches(|c| c == '\'' || c == '"');
                s.ends_with(suffix)
            } else {
                false
            }
        }
        "version" | "version_compare" => {
            if let (JsonValue::String(val), Some(args)) = (value, test_arg) {
                let parts: Vec<&str> = args.split(',').collect();
                if parts.len() >= 2 {
                    let arg1 = parts[0].trim().trim_matches(|c| c == '\'' || c == '"');
                    let arg2 = parts[1].trim().trim_matches(|c| c == '\'' || c == '"');
                    let (op, version) = if [
                        "<", ">", "<=", ">=", "==", "!=", "lt", "gt", "le", "ge", "eq", "ne",
                    ]
                    .contains(&arg1)
                    {
                        (arg1, arg2)
                    } else {
                        (arg2, arg1)
                    };
                    let cmp = compare_versions(val, version);
                    match op {
                        "<" | "lt" => cmp == std::cmp::Ordering::Less,
                        ">" | "gt" => cmp == std::cmp::Ordering::Greater,
                        "<=" | "le" => cmp != std::cmp::Ordering::Greater,
                        ">=" | "ge" => cmp != std::cmp::Ordering::Less,
                        "==" | "eq" => cmp == std::cmp::Ordering::Equal,
                        "!=" | "ne" => cmp != std::cmp::Ordering::Equal,
                        _ => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }
        "subset" => {
            if let (JsonValue::Array(subset), Some(arg)) = (value, test_arg) {
                if let Some(JsonValue::Array(superset)) = vars.get(arg.trim()) {
                    subset.iter().all(|item| superset.contains(item))
                } else {
                    false
                }
            } else {
                false
            }
        }
        "superset" => {
            if let (JsonValue::Array(superset), Some(arg)) = (value, test_arg) {
                if let Some(JsonValue::Array(subset)) = vars.get(arg.trim()) {
                    subset.iter().all(|item| superset.contains(item))
                } else {
                    false
                }
            } else {
                false
            }
        }
        "in" => {
            if let Some(arg) = test_arg {
                if let Some(container) = vars.get(arg.trim()) {
                    match container {
                        JsonValue::Array(arr) => arr.contains(value),
                        JsonValue::String(s) => {
                            if let JsonValue::String(v) = value {
                                s.contains(v.as_str())
                            } else {
                                false
                            }
                        }
                        JsonValue::Object(obj) => {
                            if let JsonValue::String(k) = value {
                                obj.contains_key(k)
                            } else {
                                false
                            }
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        }
        "truthy" => is_truthy(value),
        "falsy" => !is_truthy(value),
        "abs" => matches!(value, JsonValue::Number(_)),
        _ => false,
    }
}

/// Evaluate a conditional expression
fn evaluate_expression(expr: &str, vars: &IndexMap<String, JsonValue>) -> ExecutorResult<bool> {
    let expr = expr.trim();

    // Handle empty expression
    if expr.is_empty() {
        return Ok(true);
    }

    // Handle simple boolean expressions
    if expr == "true" || expr == "True" {
        return Ok(true);
    }
    if expr == "false" || expr == "False" {
        return Ok(false);
    }

    // Handle parenthesized expressions first
    if expr.starts_with('(') {
        if let Some(close_pos) = find_matching_paren(expr, 0) {
            if close_pos == expr.len() - 1 {
                return evaluate_expression(&expr[1..close_pos], vars);
            }
        }
    }

    // Handle 'or' expressions (lowest precedence, check first for left-to-right)
    if let Some(pos) = find_operator_outside_parens(expr, " or ") {
        let left = &expr[..pos];
        let right = &expr[pos + 4..];
        return Ok(
            evaluate_expression(left.trim(), vars)? || evaluate_expression(right.trim(), vars)?
        );
    }

    // Handle 'and' expressions
    if let Some(pos) = find_operator_outside_parens(expr, " and ") {
        let left = &expr[..pos];
        let right = &expr[pos + 5..];
        return Ok(
            evaluate_expression(left.trim(), vars)? && evaluate_expression(right.trim(), vars)?
        );
    }

    // Handle 'not' expressions (must be at the start)
    if let Some(inner) = expr.strip_prefix("not ") {
        return Ok(!evaluate_expression(inner.trim(), vars)?);
    }

    // Handle 'not in' expressions (must check before 'in')
    if let Some(pos) = find_operator_outside_parens(expr, " not in ") {
        let left_str = expr[..pos].trim();
        let right_str = expr[pos + 8..].trim();
        let left = evaluate_variable_expression(left_str, vars)?;
        let right = parse_value(right_str, vars)?;

        let result = match right {
            JsonValue::Array(arr) => arr.contains(&left),
            JsonValue::String(s) => {
                if let JsonValue::String(l) = &left {
                    s.contains(l.as_str())
                } else {
                    false
                }
            }
            JsonValue::Object(obj) => {
                if let JsonValue::String(k) = &left {
                    obj.contains_key(k)
                } else {
                    false
                }
            }
            _ => false,
        };
        return Ok(!result);
    }

    // Handle comparison operators (check >= and <= before > and <)
    if let Some(pos) = find_operator_outside_parens(expr, " >= ") {
        let left = evaluate_variable_expression(expr[..pos].trim(), vars)?;
        let right_str = expr[pos + 4..].trim();
        let right = parse_value(right_str, vars)?;
        return Ok(compare_values(&left, &right)
            .map(|c| c != std::cmp::Ordering::Less)
            .unwrap_or(false));
    }

    if let Some(pos) = find_operator_outside_parens(expr, " <= ") {
        let left = evaluate_variable_expression(expr[..pos].trim(), vars)?;
        let right_str = expr[pos + 4..].trim();
        let right = parse_value(right_str, vars)?;
        return Ok(compare_values(&left, &right)
            .map(|c| c != std::cmp::Ordering::Greater)
            .unwrap_or(false));
    }

    if let Some(pos) = find_operator_outside_parens(expr, " > ") {
        let left = evaluate_variable_expression(expr[..pos].trim(), vars)?;
        let right_str = expr[pos + 3..].trim();
        let right = parse_value(right_str, vars)?;
        return Ok(compare_values(&left, &right)
            .map(|c| c == std::cmp::Ordering::Greater)
            .unwrap_or(false));
    }

    if let Some(pos) = find_operator_outside_parens(expr, " < ") {
        let left = evaluate_variable_expression(expr[..pos].trim(), vars)?;
        let right_str = expr[pos + 3..].trim();
        let right = parse_value(right_str, vars)?;
        return Ok(compare_values(&left, &right)
            .map(|c| c == std::cmp::Ordering::Less)
            .unwrap_or(false));
    }

    // Handle equality operators
    if let Some(pos) = find_operator_outside_parens(expr, " == ") {
        let left = evaluate_variable_expression(expr[..pos].trim(), vars)?;
        let right_str = expr[pos + 4..].trim();
        let right = parse_value(right_str, vars)?;
        return Ok(left == right);
    }

    if let Some(pos) = find_operator_outside_parens(expr, " != ") {
        let left = evaluate_variable_expression(expr[..pos].trim(), vars)?;
        let right_str = expr[pos + 4..].trim();
        let right = parse_value(right_str, vars)?;
        return Ok(left != right);
    }

    // Handle 'is not' tests (must check before 'is')
    if let Some(pos) = find_operator_outside_parens(expr, " is not ") {
        let var_name = expr[..pos].trim();
        let test_expr = expr[pos + 8..].trim();
        let value = evaluate_variable_expression(var_name, vars)?;

        let (test_name, test_arg) = if let Some(paren_pos) = test_expr.find('(') {
            let name = test_expr[..paren_pos].trim();
            let arg_end = test_expr.rfind(')').unwrap_or(test_expr.len());
            let arg = &test_expr[paren_pos + 1..arg_end];
            (name, Some(arg))
        } else {
            (test_expr, None)
        };

        return Ok(!evaluate_jinja_test(&value, test_name, test_arg, vars));
    }

    // Handle 'is' tests
    if let Some(pos) = find_operator_outside_parens(expr, " is ") {
        let var_name = expr[..pos].trim();
        let test_expr = expr[pos + 4..].trim();
        let value = evaluate_variable_expression(var_name, vars)?;

        let (test_name, test_arg) = if let Some(paren_pos) = test_expr.find('(') {
            let name = test_expr[..paren_pos].trim();
            let arg_end = test_expr.rfind(')').unwrap_or(test_expr.len());
            let arg = &test_expr[paren_pos + 1..arg_end];
            (name, Some(arg))
        } else {
            (test_expr, None)
        };

        return Ok(evaluate_jinja_test(&value, test_name, test_arg, vars));
    }

    // Handle 'in' expressions
    if let Some(pos) = find_operator_outside_parens(expr, " in ") {
        let left_str = expr[..pos].trim();
        let right_str = expr[pos + 4..].trim();
        let left = evaluate_variable_expression(left_str, vars)?;
        let right = parse_value(right_str, vars)?;

        return match right {
            JsonValue::Array(arr) => Ok(arr.contains(&left)),
            JsonValue::String(s) => {
                if let JsonValue::String(l) = left {
                    Ok(s.contains(&l))
                } else {
                    Ok(false)
                }
            }
            JsonValue::Object(obj) => {
                if let JsonValue::String(k) = left {
                    Ok(obj.contains_key(&k))
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        };
    }

    // Handle variable truthiness
    let value = evaluate_variable_expression(expr, vars)?;
    Ok(is_truthy(&value))
}

/// Parse a value from string (could be literal or variable)
///
/// # Performance
/// Hot path for expression evaluation - inline for better optimization.
#[inline]
fn parse_value(s: &str, vars: &IndexMap<String, JsonValue>) -> ExecutorResult<JsonValue> {
    let s = s.trim();

    // String literal
    if (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')) {
        return Ok(JsonValue::String(s[1..s.len() - 1].to_string()));
    }

    // Boolean
    if s == "true" || s == "True" {
        return Ok(JsonValue::Bool(true));
    }
    if s == "false" || s == "False" {
        return Ok(JsonValue::Bool(false));
    }

    // Number
    if let Ok(n) = s.parse::<i64>() {
        return Ok(JsonValue::Number(n.into()));
    }
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return Ok(JsonValue::Number(num));
        }
    }

    // Variable reference
    evaluate_variable_expression(s, vars)
}

/// Check if a JSON value is "truthy"
///
/// # Performance
/// Hot path function - called for every condition evaluation.
#[inline]
fn is_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(b) => *b,
        JsonValue::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        JsonValue::String(s) => {
            let value = s.trim();
            if value.is_empty() {
                return false;
            }
            if value.eq_ignore_ascii_case("false")
                || value.eq_ignore_ascii_case("no")
                || value.eq_ignore_ascii_case("off")
                || value.eq_ignore_ascii_case("n")
                || value.eq_ignore_ascii_case("f")
                || value == "0"
            {
                return false;
            }
            true
        }
        JsonValue::Array(arr) => !arr.is_empty(),
        JsonValue::Object(obj) => !obj.is_empty(),
    }
}

/// Module trait for implementing custom modules
#[async_trait]
pub trait Module: Send + Sync {
    /// Module name
    fn name(&self) -> &str;

    /// Execute the module
    async fn execute(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult>;

    /// Validate arguments
    fn validate_args(&self, _args: &IndexMap<String, JsonValue>) -> ExecutorResult<()> {
        Ok(())
    }

    /// Check if module supports check mode
    fn supports_check_mode(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn diligence_task_transport_is_preserved_in_free_and_pinned_strategies() {
        for strategy in [
            crate::executor::ExecutionStrategy::Free,
            crate::executor::ExecutionStrategy::HostPinned,
        ] {
            let mut runtime = RuntimeContext::new();
            runtime.add_host("localhost".into(), None);
            let config = crate::executor::ExecutorConfig {
                strategy,
                ..Default::default()
            };
            let executor = crate::executor::Executor::with_runtime(config, runtime);
            let playbook = crate::executor::Playbook::parse("- hosts: localhost\n  gather_facts: false\n  tasks:\n    - command: /bin/true\n      connection: ssh\n", None).unwrap();
            let results = executor.run_playbook(&playbook).await.unwrap();
            assert!(results["localhost"].unreachable);
        }
    }

    #[test]
    fn diligence_included_task_conversion_preserves_transport_and_scalar_args() {
        let task: crate::playbook::Task =
            serde_yaml::from_str("name: transport\nconnection: ssh\ncommand: /bin/true\n").unwrap();
        let task = Task::from(task);
        assert_eq!(task.module, "command");
        assert_eq!(task.vars["ansible_connection"], "ssh");
        assert_eq!(task.args["_raw_params"], "/bin/true");
    }

    #[tokio::test]
    async fn diligence_local_become_is_rejected_before_execution() {
        let task = Task::new("identity", "command").arg("cmd", "/usr/bin/id -un");
        let mut ctx = ExecutionContext::new("localhost");
        ctx.r#become = true;
        let result = task
            .execute_native(
                "command",
                &task.args,
                &ctx,
                &Arc::new(RwLock::new(RuntimeContext::new())),
                &Arc::new(ModuleRegistry::default()),
            )
            .await
            .unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
        assert!(result.result.is_none());
    }

    #[tokio::test]
    async fn diligence_remote_facts_cannot_authorize_local_execution() {
        let task = Task::new("guard", "command").arg("cmd", "/bin/true");
        let mut runtime = RuntimeContext::new();
        runtime.set_host_var(
            "remote.invalid",
            "ansible_connection".into(),
            serde_json::json!("ssh"),
        );
        runtime.set_host_fact(
            "remote.invalid",
            "ansible_connection".into(),
            serde_json::json!("local"),
        );
        let result = task
            .execute_native(
                "command",
                &task.args,
                &ExecutionContext::new("remote.invalid"),
                &Arc::new(RwLock::new(runtime)),
                &Arc::new(ModuleRegistry::default()),
            )
            .await
            .unwrap();
        assert_eq!(result.status, TaskStatus::Unreachable);
    }

    #[test]
    fn diligence_register_preserves_payload_and_authoritative_status() {
        let result = TaskResult::failed("expected").with_result(serde_json::json!({
            "rc": 7, "stdout": "evidence\n", "stderr": "failure\n",
            "stat": { "exists": false }, "failed": false, "changed": true
        }));
        let registered = result.to_registered(None, None);
        assert!(registered.failed);
        assert!(!registered.changed);
        assert_eq!(registered.rc, Some(7));
        assert_eq!(registered.stdout_lines, Some(vec!["evidence".into()]));
        assert_eq!(registered.data["stat"]["exists"], false);
        assert!(!registered.data.contains_key("failed"));
    }

    #[tokio::test]
    async fn diligence_remote_file_guard_applies_even_with_a_connection() {
        let scratch = tempfile::tempdir().unwrap();
        let sentinel = scratch.path().join("must-not-exist");
        let task = Task::new("guard", "file")
            .arg("path", sentinel.to_str().unwrap())
            .arg("state", "touch");
        let ctx = ExecutionContext::new("remote.invalid")
            .with_connection(Arc::new(crate::connection::local::LocalConnection::new()));
        let runtime = Arc::new(RwLock::new(RuntimeContext::new()));
        let result = task
            .execute_native(
                "file",
                &task.args,
                &ctx,
                &runtime,
                &Arc::new(ModuleRegistry::default()),
            )
            .await
            .unwrap();
        assert_eq!(result.status, TaskStatus::Failed);
        assert!(!sentinel.exists());
    }

    #[tokio::test]
    async fn diligence_explicit_remote_localhost_never_falls_back() {
        let task = Task::new("guard", "command").arg("cmd", "/bin/true");
        let mut runtime = RuntimeContext::new();
        runtime.set_host_var(
            "localhost",
            "ansible_connection".into(),
            serde_json::json!("ssh"),
        );
        let result = task
            .execute_native(
                "command",
                &task.args,
                &ExecutionContext::new("localhost"),
                &Arc::new(RwLock::new(runtime)),
                &Arc::new(ModuleRegistry::default()),
            )
            .await
            .unwrap();
        assert_eq!(result.status, TaskStatus::Unreachable);
    }

    /// Reports whether the module context carried a connection, under any name.
    struct ConnectionProbe(&'static str);

    impl crate::modules::Module for ConnectionProbe {
        fn name(&self) -> &'static str {
            self.0
        }
        fn description(&self) -> &'static str {
            "reports whether the module context carries a connection"
        }
        fn execute(
            &self,
            _params: &crate::modules::ModuleParams,
            context: &crate::modules::ModuleContext,
        ) -> crate::modules::ModuleResult<crate::modules::ModuleOutput> {
            Ok(crate::modules::ModuleOutput::ok(
                match &context.connection {
                    Some(_) => "connection",
                    None => "none",
                },
            ))
        }
    }

    async fn probe_local_target(module: &'static str) -> TaskResult {
        let mut registry = ModuleRegistry::new();
        registry.register(Arc::new(ConnectionProbe(module)));
        let mut runtime = RuntimeContext::new();
        runtime.set_host_var(
            "web1",
            "ansible_connection".into(),
            serde_json::json!("local"),
        );
        let task = Task::new("probe", module);
        task.execute_native(
            module,
            &task.args,
            &ExecutionContext::new("web1"),
            &Arc::new(RwLock::new(runtime)),
            &Arc::new(registry),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn diligence_explicit_local_target_receives_a_local_connection() {
        let result = probe_local_target("connection_probe").await;
        assert_eq!(result.status, TaskStatus::Ok);
        assert_eq!(result.msg.as_deref(), Some("connection"));
    }

    #[tokio::test]
    async fn diligence_local_dispatch_modules_keep_running_without_a_connection() {
        let result = probe_local_target("copy").await;
        assert_eq!(result.status, TaskStatus::Ok);
        assert_eq!(result.msg.as_deref(), Some("none"));
    }

    async fn probe_local_target_via_task(
        host: &str,
        task_module: &str,
        registered: &'static str,
        manager: &Arc<ParallelizationManager>,
    ) -> TaskResult {
        let mut registry = ModuleRegistry::new();
        registry.register(Arc::new(ConnectionProbe(registered)));
        let mut runtime = RuntimeContext::new();
        runtime.set_host_var(
            host,
            "ansible_connection".into(),
            serde_json::json!("local"),
        );
        Task::new("probe", task_module)
            .execute_module(
                &ExecutionContext::new(host),
                &Arc::new(RwLock::new(runtime)),
                &Arc::new(RwLock::new(HashMap::new())),
                &Arc::new(Mutex::new(std::collections::HashSet::new())),
                manager,
                &Arc::new(registry),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn diligence_local_aliases_serialize_default_hint_modules_under_one_lock() {
        // The probe declares no parallelization constraint; on local aliases it
        // must still be serialized under the shared key.
        let manager = Arc::new(ParallelizationManager::new());
        for host in ["web1", "web2"] {
            let result =
                probe_local_target_via_task(host, "connection_probe", "connection_probe", &manager)
                    .await;
            assert_eq!(result.msg.as_deref(), Some("connection"));
        }
        let locks = manager.stats().host_locks;
        assert_eq!(locks.len(), 1, "{locks:?}");
        assert_eq!(locks.get("localhost"), Some(&1));
    }

    #[tokio::test]
    async fn diligence_collection_prefixed_names_follow_the_same_dispatch_rule() {
        let manager = Arc::new(ParallelizationManager::new());
        let result =
            probe_local_target_via_task("web1", "ansible.legacy.copy", "copy", &manager).await;
        assert_eq!(result.msg.as_deref(), Some("none"));
        let result = probe_local_target_via_task(
            "web1",
            "ansible.builtin.connection_probe",
            "connection_probe",
            &manager,
        )
        .await;
        assert_eq!(result.msg.as_deref(), Some("connection"));
    }

    #[tokio::test]
    async fn diligence_prefixed_set_fact_stores_facts_on_the_inventory_host() {
        use crate::executor::batch_processor::{BatchConfig, BatchProcessor};

        let mut task = Task::new("fact", "ansible.legacy.set_fact").arg("greeting", "hello");
        task.delegate_to = Some("delegate.invalid".to_string());
        let mut runtime = RuntimeContext::new();
        runtime.add_host("web1".to_string(), None);
        runtime.add_host("delegate.invalid".to_string(), None);
        let runtime = Arc::new(RwLock::new(runtime));
        let result = task
            .execute(
                &ExecutionContext::new("web1"),
                &runtime,
                &Arc::new(RwLock::new(HashMap::new())),
                &Arc::new(Mutex::new(std::collections::HashSet::new())),
                &Arc::new(ParallelizationManager::new()),
                &Arc::new(ModuleRegistry::default()),
                &Arc::new(BatchProcessor::new(BatchConfig::default())),
                false,
            )
            .await
            .unwrap();
        assert_ne!(result.status, TaskStatus::Failed, "{:?}", result.msg);
        let runtime = runtime.read().await;
        assert_eq!(
            runtime.get_host_fact("web1", "greeting"),
            Some(serde_json::json!("hello"))
        );
        assert_eq!(runtime.get_host_fact("delegate.invalid", "greeting"), None);
    }

    #[tokio::test]
    async fn diligence_local_include_tasks_does_not_deadlock_on_the_shared_lock() {
        let scratch = tempfile::tempdir().unwrap();
        std::fs::write(
            scratch.path().join("included.yml"),
            "- name: included\n  debug:\n    msg: included ran\n",
        )
        .unwrap();
        let mut runtime = RuntimeContext::new();
        runtime.add_host("web1".to_string(), None);
        runtime.set_host_var(
            "web1",
            "ansible_connection".into(),
            serde_json::json!("local"),
        );
        let manager = Arc::new(ParallelizationManager::new());
        let task = Task::new("include", "include_tasks").arg(
            "file",
            serde_json::json!(scratch.path().join("included.yml").to_string_lossy()),
        );
        let ctx = ExecutionContext::new("web1");
        let runtime = Arc::new(RwLock::new(runtime));
        let handlers = Arc::new(RwLock::new(HashMap::new()));
        let notified = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let registry = Arc::new(ModuleRegistry::default());
        let run = task.execute_module(&ctx, &runtime, &handlers, &notified, &manager, &registry);
        let result = tokio::time::timeout(std::time::Duration::from_secs(30), run)
            .await
            .expect("include_tasks must not wait on the shared local lock")
            .unwrap();
        assert_ne!(result.status, TaskStatus::Failed, "{:?}", result.msg);
    }

    #[tokio::test]
    async fn diligence_local_shell_stdin_reaches_the_command() {
        let task = Task::new("stdin", "shell")
            .arg("cmd", "cat")
            .arg("stdin", "payload");
        let result = task
            .execute_native(
                "shell",
                &task.args,
                &ExecutionContext::new("localhost"),
                &Arc::new(RwLock::new(RuntimeContext::new())),
                &Arc::new(ModuleRegistry::default()),
            )
            .await
            .unwrap();
        assert_ne!(result.status, TaskStatus::Failed, "{:?}", result.msg);
        let stdout = result.result.as_ref().and_then(|r| r["stdout"].as_str());
        assert_eq!(stdout.map(str::trim_end), Some("payload"));
    }

    #[test]
    fn test_task_builder() {
        let task = Task::new("Install nginx", "package")
            .arg("name", "nginx")
            .arg("state", "present")
            .when("ansible_os_family == 'Debian'")
            .notify("restart nginx")
            .register("install_result");

        assert_eq!(task.name, "Install nginx");
        assert_eq!(task.module, "package");
        assert_eq!(
            task.args.get("name"),
            Some(&JsonValue::String("nginx".into()))
        );
        assert_eq!(task.when, Some("ansible_os_family == 'Debian'".to_string()));
        assert!(task.notify.contains(&"restart nginx".to_string()));
        assert_eq!(task.register, Some("install_result".to_string()));
    }

    #[test]
    fn test_template_string() {
        let mut vars = IndexMap::new();
        vars.insert("name".to_string(), JsonValue::String("world".to_string()));
        vars.insert("count".to_string(), JsonValue::Number(42.into()));

        let result = template_string("Hello {{ name }}!", &vars).unwrap();
        assert_eq!(result, "Hello world!");

        let result = template_string("Count: {{ count }}", &vars).unwrap();
        assert_eq!(result, "Count: 42");
    }

    #[test]
    fn test_evaluate_expression_boolean() {
        let vars = IndexMap::new();

        assert!(evaluate_expression("true", &vars).unwrap());
        assert!(!evaluate_expression("false", &vars).unwrap());
        assert!(!evaluate_expression("not true", &vars).unwrap());
    }

    #[test]
    fn test_evaluate_expression_comparison() {
        let mut vars = IndexMap::new();
        vars.insert("os".to_string(), JsonValue::String("Debian".to_string()));
        vars.insert("count".to_string(), JsonValue::Number(5.into()));

        assert!(evaluate_expression("os == 'Debian'", &vars).unwrap());
        assert!(!evaluate_expression("os == 'RedHat'", &vars).unwrap());
        assert!(evaluate_expression("os != 'RedHat'", &vars).unwrap());
    }

    #[test]
    fn test_evaluate_expression_defined() {
        let mut vars = IndexMap::new();
        vars.insert(
            "existing".to_string(),
            JsonValue::String("value".to_string()),
        );

        assert!(evaluate_expression("existing is defined", &vars).unwrap());
        assert!(!evaluate_expression("nonexistent is defined", &vars).unwrap());
        assert!(evaluate_expression("nonexistent is not defined", &vars).unwrap());
    }

    #[test]
    fn test_evaluate_expression_in() {
        let mut vars = IndexMap::new();
        vars.insert("items".to_string(), serde_json::json!(["a", "b", "c"]));
        vars.insert("letter".to_string(), JsonValue::String("b".to_string()));

        assert!(evaluate_expression("letter in items", &vars).unwrap());
    }

    #[test]
    fn test_task_result() {
        let result = TaskResult::ok();
        assert_eq!(result.status, TaskStatus::Ok);
        assert!(!result.changed);

        let result = TaskResult::changed();
        assert_eq!(result.status, TaskStatus::Changed);
        assert!(result.changed);

        let result = TaskResult::failed("error message");
        assert_eq!(result.status, TaskStatus::Failed);
        assert_eq!(result.msg, Some("error message".to_string()));
    }

    #[test]
    fn test_is_truthy() {
        assert!(!is_truthy(&JsonValue::Null));
        assert!(!is_truthy(&JsonValue::Bool(false)));
        assert!(is_truthy(&JsonValue::Bool(true)));
        assert!(!is_truthy(&JsonValue::String("".to_string())));
        assert!(!is_truthy(&JsonValue::String("0".to_string())));
        assert!(!is_truthy(&JsonValue::String("false".to_string())));
        assert!(!is_truthy(&JsonValue::String("no".to_string())));
        assert!(!is_truthy(&JsonValue::String("off".to_string())));
        assert!(!is_truthy(&JsonValue::String("n".to_string())));
        assert!(!is_truthy(&JsonValue::String("f".to_string())));
        assert!(is_truthy(&JsonValue::String("hello".to_string())));
        assert!(!is_truthy(&JsonValue::Array(vec![])));
        assert!(is_truthy(&JsonValue::Array(vec![JsonValue::Null])));
    }
}
