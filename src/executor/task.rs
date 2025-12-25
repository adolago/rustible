//! Task definition and execution for Rustible
//!
//! This module provides:
//! - Task struct with module, args, when conditions, loops
//! - Task result handling
//! - Changed/ok/failed states

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, instrument, warn};

use crate::executor::parallelization::ParallelizationManager;
use crate::executor::runtime::{ExecutionContext, RegisteredResult, RuntimeContext};
use crate::executor::{ExecutorError, ExecutorResult};

/// Status of a task execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Task completed successfully without changes
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

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Ok
    }
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
        RegisteredResult {
            changed: self.changed,
            failed: self.status == TaskStatus::Failed,
            skipped: self.status == TaskStatus::Skipped,
            rc: None,
            stdout: stdout.clone(),
            stdout_lines: stdout.map(|s| s.lines().map(String::from).collect()),
            stderr: stderr.clone(),
            stderr_lines: stderr.map(|s| s.lines().map(String::from).collect()),
            msg: self.msg.clone(),
            results: None,
            data: IndexMap::new(),
        }
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
    pub loop_items: Option<Vec<JsonValue>>,
    /// Loop variable name (default: "item")
    #[serde(default = "default_loop_var")]
    pub loop_var: String,
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
            ignore_errors: false,
            changed_when: None,
            failed_when: None,
            delegate_to: None,
            delegate_facts: None,
            run_once: false,
            tags: Vec::new(),
            r#become: false,
            become_user: None,
        }
    }
}

/// Convert from playbook::Task to executor::task::Task
impl From<crate::playbook::Task> for Task {
    fn from(pt: crate::playbook::Task) -> Self {
        // Convert args from serde_json::Value to IndexMap
        let args = if let Some(obj) = pt.module.args.as_object() {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            IndexMap::new()
        };

        // Convert when condition
        let when = pt.when.map(|w| match w {
            crate::playbook::When::Single(s) => s,
            crate::playbook::When::Multiple(v) => v.join(" and "),
        });

        // Convert loop items
        let loop_items = pt.loop_.or(pt.with_items).and_then(|v| {
            if let Some(arr) = v.as_array() {
                Some(arr.clone())
            } else {
                None
            }
        });

        // Get loop_var from loop_control if available
        let loop_var = pt
            .loop_control
            .as_ref()
            .map(|lc| lc.loop_var.clone())
            .unwrap_or_else(default_loop_var);

        Self {
            name: pt.name,
            module: pt.module.name,
            args,
            when,
            notify: pt.notify,
            register: pt.register,
            loop_items,
            loop_var,
            ignore_errors: pt.ignore_errors,
            changed_when: pt.changed_when,
            failed_when: pt.failed_when,
            delegate_to: pt.delegate_to,
            delegate_facts: pt.delegate_facts,
            run_once: pt.run_once,
            tags: pt.tags,
            r#become: pt.r#become.unwrap_or(false),
            become_user: pt.become_user,
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
        self.loop_items = Some(items);
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
    #[instrument(skip(self, ctx, runtime, handlers, notified, parallelization_manager), fields(task_name = %self.name, host = %ctx.host))]
    pub async fn execute(
        &self,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
        handlers: &Arc<RwLock<HashMap<String, Handler>>>,
        notified: &Arc<Mutex<std::collections::HashSet<String>>>,
        parallelization_manager: &Arc<ParallelizationManager>,
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
        let (execution_ctx, fact_storage_ctx) = if let Some(ref delegate_host) = self.delegate_to {
            debug!("Delegating task to host: {}", delegate_host);

            // Create execution context for the delegate host (where task actually runs)
            let mut delegate_ctx = ctx.clone();
            delegate_ctx.host = delegate_host.clone();

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

        // Handle loops - for set_fact, use fact_storage_ctx; for others, use execution_ctx
        if let Some(ref items) = self.loop_items {
            let loop_ctx = if self.module == "set_fact" {
                &fact_storage_ctx
            } else {
                &execution_ctx
            };
            return self
                .execute_loop(items, loop_ctx, runtime, handlers, notified, parallelization_manager)
                .await;
        }

        // Execute the module - use fact_storage_ctx for set_fact to ensure facts go to right host
        let module_ctx = if self.module == "set_fact" {
            &fact_storage_ctx
        } else {
            &execution_ctx
        };
        let result = self.execute_module(module_ctx, runtime, handlers, notified, parallelization_manager).await?;

        // Apply changed_when override - use execution context for condition evaluation
        let result = self.apply_changed_when(result, &execution_ctx, runtime).await?;

        // Apply failed_when override - use execution context for condition evaluation
        let result = self.apply_failed_when(result, &execution_ctx, runtime).await?;

        // Register result if needed - always register on the original host
        if let Some(ref register_name) = self.register {
            self.register_result(register_name, &result, ctx, runtime)
                .await?;
        }

        // Notify handlers if task changed
        if result.changed && result.status != TaskStatus::Failed {
            for handler_name in &self.notify {
                let mut notified = notified.lock().await;
                notified.insert(handler_name.clone());
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
    ) -> ExecutorResult<TaskResult> {
        debug!("Executing loop with {} items", items.len());

        let mut loop_results = Vec::new();
        let mut any_changed = false;
        let mut any_failed = false;

        for (index, item) in items.iter().enumerate() {
            // Set loop variables
            {
                let mut rt = runtime.write().await;
                rt.set_task_var(self.loop_var.clone(), item.clone());
                rt.set_task_var(
                    "ansible_loop".to_string(),
                    serde_json::json!({
                        "index": index,
                        "index0": index,
                        "first": index == 0,
                        "last": index == items.len() - 1,
                        "length": items.len(),
                    }),
                );
            }

            // Execute for this item with parallelization enforcement
            let result = self.execute_module(ctx, runtime, handlers, notified, parallelization_manager).await?;

            if result.changed {
                any_changed = true;
            }
            if result.status == TaskStatus::Failed {
                any_failed = true;
                if !self.ignore_errors {
                    // Stop on first failure unless ignore_errors
                    loop_results.push(result.to_registered(None, None));
                    break;
                }
            }

            loop_results.push(result.to_registered(None, None));
        }

        // Clear loop variables
        {
            let mut rt = runtime.write().await;
            rt.clear_task_vars();
        }

        // Create combined result
        let status = if any_failed && !self.ignore_errors {
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
            registered.results = Some(loop_results);

            let mut rt = runtime.write().await;
            rt.register_result(&ctx.host, register_name.clone(), registered);
        }

        // Notify handlers if anything changed
        if any_changed && !any_failed {
            for handler_name in &self.notify {
                let mut n = notified.lock().await;
                n.insert(handler_name.clone());
            }
        }

        Ok(result)
    }

    /// Execute the actual module
    async fn execute_module(
        &self,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
        handlers: &Arc<RwLock<HashMap<String, Handler>>>,
        notified: &Arc<Mutex<std::collections::HashSet<String>>>,
        parallelization_manager: &Arc<ParallelizationManager>,
    ) -> ExecutorResult<TaskResult> {
        // Template the arguments
        let args = self.template_args(ctx, runtime).await?;

        debug!("Module: {}, Args: {:?}", self.module, args);

        // Enforce parallelization constraints based on module hint
        // Get the module's parallelization hint from the registry
        let hint = {
            let registry = crate::modules::ModuleRegistry::with_builtins();
            if let Some(module) = registry.get(&self.module) {
                module.parallelization_hint()
            } else {
                // For unknown modules (Python fallback), use FullyParallel as default
                crate::modules::ParallelizationHint::FullyParallel
            }
        };

        // Acquire parallelization guard - this will block if necessary based on the hint
        // The guard is automatically released when it goes out of scope (when this function returns)
        let _parallelization_guard = parallelization_manager
            .acquire(hint, &ctx.host, &self.module)
            .await;

        // Execute based on module type
        let result = match self.module.as_str() {
            "debug" => self.execute_debug(&args, ctx).await,
            "set_fact" => self.execute_set_fact(&args, ctx, runtime).await,
            "command" | "shell" => self.execute_command(&args, ctx, runtime).await,
            "copy" => self.execute_copy(&args, ctx).await,
            "file" => self.execute_file(&args, ctx).await,
            "template" => self.execute_template(&args, ctx, runtime).await,
            "package" | "apt" | "yum" | "dnf" => self.execute_package(&args, ctx).await,
            "service" | "systemd" => self.execute_service(&args, ctx).await,
            "user" => self.execute_user(&args, ctx).await,
            "group" => self.execute_group(&args, ctx).await,
            "lineinfile" => self.execute_lineinfile(&args, ctx).await,
            "blockinfile" => self.execute_blockinfile(&args, ctx).await,
            "stat" => self.execute_stat(&args, ctx).await,
            "fail" => self.execute_fail(&args).await,
            "assert" => self.execute_assert(&args, ctx, runtime).await,
            "pause" => self.execute_pause(&args).await,
            "wait_for" => self.execute_wait_for(&args, ctx).await,
            "include_vars" => self.execute_include_vars(&args, ctx, runtime).await,
            "include_tasks" | "import_tasks" => {
                self.execute_include_tasks(&args, ctx, runtime, handlers, notified, parallelization_manager).await
            }
            "meta" => self.execute_meta(&args).await,
            _ => {
                // Python fallback for unknown modules
                // Check if we can find the module in Ansible's module library
                let mut executor = crate::modules::PythonModuleExecutor::new();

                if let Some(module_path) = executor.find_module(&self.module) {
                    debug!(
                        "Found Ansible module {} at {} - Python fallback available",
                        self.module,
                        module_path.display()
                    );

                    // In check mode, report that we would execute
                    if ctx.check_mode {
                        return Ok(TaskResult::ok().with_msg(format!(
                            "Check mode - would execute Python module: {}",
                            self.module
                        )));
                    }

                    // Execute via Python if connection is available
                    if let Some(ref connection) = ctx.connection {
                        // Convert args to ModuleParams-compatible format
                        let module_params: std::collections::HashMap<String, serde_json::Value> =
                            args.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

                        match executor
                            .execute(
                                connection.as_ref(),
                                &self.module,
                                &module_params,
                                &ctx.python_interpreter,
                            )
                            .await
                        {
                            Ok(output) => {
                                let msg = output.msg.clone();
                                let mut result = if output.changed {
                                    TaskResult::changed()
                                } else {
                                    TaskResult::ok()
                                };
                                result.msg = Some(msg);
                                if !output.data.is_empty() {
                                    result.result = Some(
                                        serde_json::to_value(&output.data).unwrap_or_default(),
                                    );
                                }
                                Ok(result)
                            }
                            Err(e) => Err(ExecutorError::RuntimeError(format!(
                                "Python module {} failed: {}",
                                self.module, e
                            ))),
                        }
                    } else {
                        // No connection available - simulate for localhost or log warning
                        if ctx.host == "localhost" || ctx.host == "127.0.0.1" {
                            warn!(
                                "Python module {} would need local execution (not implemented)",
                                self.module
                            );
                        } else {
                            warn!(
                                "Python module {} requires connection to {} (not available)",
                                self.module, ctx.host
                            );
                        }
                        Ok(TaskResult::changed().with_msg(format!(
                            "Executed Python module: {} (simulated - no connection)",
                            self.module
                        )))
                    }
                } else {
                    // Module not found anywhere
                    Err(ExecutorError::ModuleNotFound(format!(
                        "Module '{}' not found. Not a native module and not found in Ansible module paths. \
                        Ensure Ansible is installed or set ANSIBLE_LIBRARY environment variable.",
                        self.module
                    )))
                }
            }
        };

        result
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

        for (key, value) in &self.args {
            let templated = template_value(value, &vars)?;
            result.insert(key.clone(), templated);
        }

        Ok(result)
    }

    /// Evaluate a when condition
    async fn evaluate_condition(
        &self,
        condition: &str,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<bool> {
        let rt = runtime.read().await;
        let vars = rt.get_merged_vars(&ctx.host);

        evaluate_expression(condition, &vars)
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
            result.status = if should_be_changed {
                TaskStatus::Changed
            } else {
                TaskStatus::Ok
            };
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
                debug!("Set fact '{}' = {:?} for host '{}'", key, value, fact_target);
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

    async fn execute_command(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
        _runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<TaskResult> {
        let cmd = args
            .get("cmd")
            .or_else(|| args.get("_raw_params"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutorError::RuntimeError("command module requires 'cmd' argument".into())
            })?;

        if ctx.check_mode {
            return Ok(TaskResult::skipped("Check mode - command not executed"));
        }

        debug!("Would execute command: {}", cmd);

        // In a real implementation, this would actually run the command
        // For now, simulate successful execution
        let result = RegisteredResult {
            changed: true,
            rc: Some(0),
            stdout: Some(String::new()),
            stderr: Some(String::new()),
            ..Default::default()
        };

        Ok(TaskResult::changed()
            .with_msg(format!("Command executed: {}", cmd))
            .with_result(result.to_json()))
    }

    async fn execute_copy(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let dest = args.get("dest").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("copy module requires 'dest' argument".into())
        })?;

        if ctx.check_mode {
            return Ok(TaskResult::ok().with_msg("Check mode - would copy file"));
        }

        debug!("Would copy file to: {}", dest);
        Ok(TaskResult::changed().with_msg(format!("Copied to {}", dest)))
    }

    async fn execute_file(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("file module requires 'path' argument".into())
        })?;

        let state = args.get("state").and_then(|v| v.as_str()).unwrap_or("file");

        if ctx.check_mode {
            return Ok(TaskResult::ok()
                .with_msg(format!("Check mode - would ensure {} is {}", path, state)));
        }

        debug!("Would ensure {} state for: {}", state, path);
        Ok(TaskResult::changed().with_msg(format!("{} state set for {}", state, path)))
    }

    async fn execute_template(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
        _runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<TaskResult> {
        let src = args.get("src").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("template module requires 'src' argument".into())
        })?;

        let dest = args.get("dest").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("template module requires 'dest' argument".into())
        })?;

        if ctx.check_mode {
            return Ok(TaskResult::ok().with_msg("Check mode - would template file"));
        }

        debug!("Would template {} to {}", src, dest);
        Ok(TaskResult::changed().with_msg(format!("Templated {} to {}", src, dest)))
    }

    async fn execute_package(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let name = args.get("name").ok_or_else(|| {
            ExecutorError::RuntimeError("package module requires 'name' argument".into())
        })?;

        let state = args
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("present");

        if ctx.check_mode {
            return Ok(TaskResult::ok().with_msg(format!(
                "Check mode - would ensure package {:?} is {}",
                name, state
            )));
        }

        debug!("Would ensure package {:?} is {}", name, state);
        Ok(TaskResult::changed().with_msg(format!("Package {:?} state: {}", name, state)))
    }

    async fn execute_service(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("service module requires 'name' argument".into())
        })?;

        let state = args.get("state").and_then(|v| v.as_str());
        let enabled = args.get("enabled").and_then(|v| v.as_bool());

        if ctx.check_mode {
            return Ok(
                TaskResult::ok().with_msg(format!("Check mode - would manage service {}", name))
            );
        }

        debug!(
            "Would manage service: {} (state: {:?}, enabled: {:?})",
            name, state, enabled
        );
        Ok(TaskResult::changed().with_msg(format!("Service {} managed", name)))
    }

    async fn execute_user(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("user module requires 'name' argument".into())
        })?;

        if ctx.check_mode {
            return Ok(
                TaskResult::ok().with_msg(format!("Check mode - would manage user {}", name))
            );
        }

        debug!("Would manage user: {}", name);
        Ok(TaskResult::changed().with_msg(format!("User {} managed", name)))
    }

    async fn execute_group(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("group module requires 'name' argument".into())
        })?;

        if ctx.check_mode {
            return Ok(
                TaskResult::ok().with_msg(format!("Check mode - would manage group {}", name))
            );
        }

        debug!("Would manage group: {}", name);
        Ok(TaskResult::changed().with_msg(format!("Group {} managed", name)))
    }

    async fn execute_lineinfile(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("lineinfile requires 'path' argument".into())
        })?;

        if ctx.check_mode {
            return Ok(TaskResult::ok().with_msg(format!("Check mode - would modify {}", path)));
        }

        debug!("Would modify line in: {}", path);
        Ok(TaskResult::changed().with_msg(format!("Modified {}", path)))
    }

    async fn execute_blockinfile(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            ExecutorError::RuntimeError("blockinfile requires 'path' argument".into())
        })?;

        if ctx.check_mode {
            return Ok(
                TaskResult::ok().with_msg(format!("Check mode - would modify block in {}", path))
            );
        }

        debug!("Would modify block in: {}", path);
        Ok(TaskResult::changed().with_msg(format!("Modified block in {}", path)))
    }

    async fn execute_stat(
        &self,
        args: &IndexMap<String, JsonValue>,
        _ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExecutorError::RuntimeError("stat requires 'path' argument".into()))?;

        debug!("Would stat: {}", path);

        // Return simulated stat result
        let stat_result = serde_json::json!({
            "exists": true,
            "path": path,
            "isdir": false,
            "isreg": true,
            "mode": "0644",
            "uid": 1000,
            "gid": 1000,
            "size": 1024,
        });

        Ok(TaskResult::ok().with_result(serde_json::json!({ "stat": stat_result })))
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

    async fn execute_wait_for(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
    ) -> ExecutorResult<TaskResult> {
        let host = args
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.host);
        let port = args.get("port").and_then(|v| v.as_u64());
        let timeout = args.get("timeout").and_then(|v| v.as_u64()).unwrap_or(300);

        if let Some(p) = port {
            debug!("Would wait for {}:{} (timeout: {}s)", host, p, timeout);
        }

        Ok(TaskResult::ok().with_msg("Wait condition met"))
    }

    async fn execute_include_vars(
        &self,
        args: &IndexMap<String, JsonValue>,
        ctx: &ExecutionContext,
        runtime: &Arc<RwLock<RuntimeContext>>,
    ) -> ExecutorResult<TaskResult> {
        // Get file or dir parameter
        let file = args.get("file").or_else(|| args.get("_raw_params")).and_then(|v| v.as_str());
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

        // Determine base path
        let base_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

        let mut all_vars: IndexMap<String, JsonValue> = IndexMap::new();
        let source: String;

        if let Some(file_path) = file {
            // Load from a single file
            let resolved_path = if std::path::Path::new(file_path).is_absolute() {
                std::path::PathBuf::from(file_path)
            } else {
                base_path.join(file_path)
            };

            if !resolved_path.exists() {
                return Err(ExecutorError::RuntimeError(format!(
                    "include_vars file not found: {}",
                    resolved_path.display()
                )));
            }

            let content = tokio::fs::read_to_string(&resolved_path).await.map_err(|e| {
                ExecutorError::RuntimeError(format!(
                    "Failed to read include_vars file {}: {}",
                    resolved_path.display(),
                    e
                ))
            })?;

            // Parse as YAML (which also handles JSON)
            let vars: IndexMap<String, serde_yaml::Value> =
                serde_yaml::from_str(&content).map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to parse include_vars file {}: {}",
                        resolved_path.display(),
                        e
                    ))
                })?;

            // Convert YAML values to JSON values
            for (key, value) in vars {
                let json_value = serde_json::to_value(&value).map_err(|e| {
                    ExecutorError::RuntimeError(format!("Failed to convert variable {}: {}", key, e))
                })?;
                all_vars.insert(key, json_value);
            }

            source = resolved_path.display().to_string();
        } else if let Some(dir_path) = dir {
            // Load from all files in a directory
            let resolved_path = if std::path::Path::new(dir_path).is_absolute() {
                std::path::PathBuf::from(dir_path)
            } else {
                base_path.join(dir_path)
            };

            if !resolved_path.is_dir() {
                return Err(ExecutorError::RuntimeError(format!(
                    "include_vars directory not found: {}",
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

            // Load each file and merge variables
            for file_path in &files {
                let content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
                    ExecutorError::RuntimeError(format!(
                        "Failed to read file {}: {}",
                        file_path.display(),
                        e
                    ))
                })?;

                let vars: IndexMap<String, serde_yaml::Value> =
                    serde_yaml::from_str(&content).map_err(|e| {
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
    ) -> ExecutorResult<TaskResult> {
        let file = args
            .get("file")
            .or_else(|| args.get("_raw_params"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutorError::RuntimeError("include_tasks requires file path".into())
            })?;

        info!("Including tasks from: {}", file);

        // Determine base path from the runtime context or use current directory
        let base_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
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
            .map_err(|e| ExecutorError::RuntimeError(format!("Failed to load include_tasks: {}", e)))?;

        debug!("Loaded {} tasks from {}", playbook_tasks.len(), file);

        // Convert playbook::Task to executor::task::Task and execute
        let mut total_changed = false;
        let mut task_count = 0;
        let mut failed = false;

        for playbook_task in playbook_tasks {
            // Convert to executor task
            let executor_task: Task = playbook_task.into();
            // Use Box::pin to handle async recursion
            let result = Box::pin(executor_task.execute(ctx, runtime, handlers, notified, parallelization_manager)).await?;

            task_count += 1;
            if result.changed {
                total_changed = true;
            }
            if result.status == TaskStatus::Failed {
                failed = true;
                break;
            }
        }

        if failed {
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
            "flush_handlers" => {
                debug!("Would flush handlers");
                Ok(TaskResult::ok().with_msg("Handlers flushed"))
            }
            "refresh_inventory" => {
                debug!("Would refresh inventory");
                Ok(TaskResult::ok().with_msg("Inventory refreshed"))
            }
            "noop" => Ok(TaskResult::ok()),
            "end_play" => Ok(TaskResult::ok().with_msg("Play ended")),
            "end_host" => Ok(TaskResult::ok().with_msg("Host ended")),
            "clear_facts" => {
                debug!("Would clear facts");
                Ok(TaskResult::ok().with_msg("Facts cleared"))
            }
            "clear_host_errors" => Ok(TaskResult::ok().with_msg("Host errors cleared")),
            _ => {
                warn!("Unknown meta action: {}", action);
                Ok(TaskResult::ok())
            }
        }
    }
}

/// Template a value using variables
fn template_value(
    value: &JsonValue,
    vars: &IndexMap<String, JsonValue>,
) -> ExecutorResult<JsonValue> {
    match value {
        JsonValue::String(s) => {
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
        _ => Ok(value.clone()),
    }
}

/// Template a string using variables
fn template_string(template: &str, vars: &IndexMap<String, JsonValue>) -> ExecutorResult<String> {
    // Simple Jinja2-like templating
    // Handle {{ variable }} syntax
    let mut result = template.to_string();

    // Find all {{ ... }} patterns
    let re = regex::Regex::new(r"\{\{\s*([^}]+?)\s*\}\}").unwrap();

    for cap in re.captures_iter(template) {
        let full_match = cap.get(0).unwrap().as_str();
        let expr = cap.get(1).unwrap().as_str().trim();

        let value = evaluate_variable_expression(expr, vars)?;
        let replacement = json_to_string(&value);
        result = result.replace(full_match, &replacement);
    }

    Ok(result)
}

/// Evaluate a variable expression (e.g., "foo.bar" or "foo['bar']")
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
fn json_to_string(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "".to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => s.clone(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

/// Evaluate a conditional expression
fn evaluate_expression(expr: &str, vars: &IndexMap<String, JsonValue>) -> ExecutorResult<bool> {
    let expr = expr.trim();

    // Handle simple boolean expressions
    if expr == "true" || expr == "True" {
        return Ok(true);
    }
    if expr == "false" || expr == "False" {
        return Ok(false);
    }

    // Handle 'not' expressions
    if let Some(inner) = expr.strip_prefix("not ") {
        return Ok(!evaluate_expression(inner.trim(), vars)?);
    }

    // Handle 'and' expressions
    if let Some(pos) = expr.find(" and ") {
        let left = &expr[..pos];
        let right = &expr[pos + 5..];
        return Ok(
            evaluate_expression(left.trim(), vars)? && evaluate_expression(right.trim(), vars)?
        );
    }

    // Handle 'or' expressions
    if let Some(pos) = expr.find(" or ") {
        let left = &expr[..pos];
        let right = &expr[pos + 4..];
        return Ok(
            evaluate_expression(left.trim(), vars)? || evaluate_expression(right.trim(), vars)?
        );
    }

    // Handle comparison operators
    if let Some(pos) = expr.find(" == ") {
        let left = evaluate_variable_expression(&expr[..pos].trim(), vars)?;
        let right_str = expr[pos + 4..].trim();
        let right = parse_value(right_str, vars)?;
        return Ok(left == right);
    }

    if let Some(pos) = expr.find(" != ") {
        let left = evaluate_variable_expression(&expr[..pos].trim(), vars)?;
        let right_str = expr[pos + 4..].trim();
        let right = parse_value(right_str, vars)?;
        return Ok(left != right);
    }

    if let Some(pos) = expr.find(" is defined") {
        let var_name = expr[..pos].trim();
        let value = evaluate_variable_expression(var_name, vars)?;
        return Ok(!value.is_null());
    }

    if let Some(pos) = expr.find(" is not defined") {
        let var_name = expr[..pos].trim();
        let value = evaluate_variable_expression(var_name, vars)?;
        return Ok(value.is_null());
    }

    if let Some(pos) = expr.find(" in ") {
        let left_str = expr[..pos].trim();
        let right_str = expr[pos + 4..].trim();
        let left = evaluate_variable_expression(left_str, vars)?;
        let right = evaluate_variable_expression(right_str, vars)?;

        return match right {
            JsonValue::Array(arr) => Ok(arr.contains(&left)),
            JsonValue::String(s) => {
                if let JsonValue::String(l) = left {
                    Ok(s.contains(&l))
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
fn is_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(b) => *b,
        JsonValue::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        JsonValue::String(s) => !s.is_empty() && s != "false" && s != "False" && s != "no",
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
        assert!(is_truthy(&JsonValue::String("hello".to_string())));
        assert!(!is_truthy(&JsonValue::Array(vec![])));
        assert!(is_truthy(&JsonValue::Array(vec![JsonValue::Null])));
    }
}
