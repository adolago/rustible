//! Core execution engine for Rustible
//!
//! This module provides the main task execution engine with:
//! - Async task runner using tokio
//! - Parallel execution across hosts
//! - Task dependency resolution
//! - Handler triggering system
//! - Dry-run support

pub mod playbook;
pub mod runtime;
pub mod task;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures::future::join_all;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error, info, instrument, warn};

use crate::executor::playbook::{Play, Playbook};
use crate::executor::runtime::{ExecutionContext, RuntimeContext};
use crate::executor::task::{Handler, Task, TaskResult, TaskStatus};

/// Errors that can occur during execution
#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Task execution failed: {0}")]
    TaskFailed(String),

    #[error("Host unreachable: {0}")]
    HostUnreachable(String),

    #[error("Dependency cycle detected: {0}")]
    DependencyCycle(String),

    #[error("Handler not found: {0}")]
    HandlerNotFound(String),

    #[error("Variable not found: {0}")]
    VariableNotFound(String),

    #[error("Condition evaluation failed: {0}")]
    ConditionError(String),

    #[error("Module not found: {0}")]
    ModuleNotFound(String),

    #[error("Playbook parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Runtime error: {0}")]
    RuntimeError(String),
}

/// Result type for executor operations
pub type ExecutorResult<T> = Result<T, ExecutorError>;

/// Configuration for the executor
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum number of parallel host executions
    pub forks: usize,
    /// Enable dry-run mode (no actual changes)
    pub check_mode: bool,
    /// Enable diff mode (show changes)
    pub diff_mode: bool,
    /// Verbosity level (0-4)
    pub verbosity: u8,
    /// Strategy: "linear", "free", or "host_pinned"
    pub strategy: ExecutionStrategy,
    /// Timeout for task execution in seconds
    pub task_timeout: u64,
    /// Whether to gather facts automatically
    pub gather_facts: bool,
    /// Any extra variables passed via command line
    pub extra_vars: HashMap<String, serde_json::Value>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            forks: 5,
            check_mode: false,
            diff_mode: false,
            verbosity: 0,
            strategy: ExecutionStrategy::Linear,
            task_timeout: 300,
            gather_facts: true,
            extra_vars: HashMap::new(),
        }
    }
}

/// Execution strategy determining how tasks are run across hosts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStrategy {
    /// Run each task on all hosts before moving to next task
    Linear,
    /// Run all tasks on each host as fast as possible
    Free,
    /// Pin tasks to specific hosts
    HostPinned,
}

/// Statistics collected during execution
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    pub ok: usize,
    pub changed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub unreachable: usize,
}

impl ExecutionStats {
    pub fn merge(&mut self, other: &ExecutionStats) {
        self.ok += other.ok;
        self.changed += other.changed;
        self.failed += other.failed;
        self.skipped += other.skipped;
        self.unreachable += other.unreachable;
    }
}

/// Host execution result containing stats and state
#[derive(Debug, Clone)]
pub struct HostResult {
    pub host: String,
    pub stats: ExecutionStats,
    pub failed: bool,
    pub unreachable: bool,
}

/// The main executor engine
pub struct Executor {
    config: ExecutorConfig,
    runtime: Arc<RwLock<RuntimeContext>>,
    handlers: Arc<RwLock<HashMap<String, Handler>>>,
    notified_handlers: Arc<Mutex<HashSet<String>>>,
    semaphore: Arc<Semaphore>,
}

impl Executor {
    /// Create a new executor with the given configuration
    pub fn new(config: ExecutorConfig) -> Self {
        let forks = config.forks;
        Self {
            config,
            runtime: Arc::new(RwLock::new(RuntimeContext::new())),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            notified_handlers: Arc::new(Mutex::new(HashSet::new())),
            semaphore: Arc::new(Semaphore::new(forks)),
        }
    }

    /// Create executor with a pre-existing runtime context
    pub fn with_runtime(config: ExecutorConfig, runtime: RuntimeContext) -> Self {
        let forks = config.forks;
        Self {
            config,
            runtime: Arc::new(RwLock::new(runtime)),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            notified_handlers: Arc::new(Mutex::new(HashSet::new())),
            semaphore: Arc::new(Semaphore::new(forks)),
        }
    }

    /// Run a complete playbook
    #[instrument(skip(self, playbook), fields(playbook_name = %playbook.name))]
    pub async fn run_playbook(&self, playbook: &Playbook) -> ExecutorResult<HashMap<String, HostResult>> {
        info!("Starting playbook: {}", playbook.name);

        let mut all_results: HashMap<String, HostResult> = HashMap::new();

        // Set playbook-level variables
        {
            let mut runtime = self.runtime.write().await;
            for (key, value) in &playbook.vars {
                runtime.set_global_var(key.clone(), value.clone());
            }
            // Add extra vars (highest precedence)
            for (key, value) in &self.config.extra_vars {
                runtime.set_global_var(key.clone(), value.clone());
            }
        }

        // Execute each play in sequence
        for play in &playbook.plays {
            let play_results = self.run_play(play).await?;

            // Merge results
            for (host, result) in play_results {
                all_results
                    .entry(host)
                    .and_modify(|existing| {
                        existing.stats.merge(&result.stats);
                        existing.failed = existing.failed || result.failed;
                        existing.unreachable = existing.unreachable || result.unreachable;
                    })
                    .or_insert(result);
            }
        }

        // Run any remaining notified handlers
        self.flush_handlers().await?;

        info!("Playbook completed: {}", playbook.name);
        Ok(all_results)
    }

    /// Run a single play
    #[instrument(skip(self, play), fields(play_name = %play.name))]
    pub async fn run_play(&self, play: &Play) -> ExecutorResult<HashMap<String, HostResult>> {
        info!("Starting play: {}", play.name);

        // Register handlers for this play
        {
            let mut handlers = self.handlers.write().await;
            for handler in &play.handlers {
                handlers.insert(handler.name.clone(), handler.clone());
            }
        }

        // Set play-level variables
        {
            let mut runtime = self.runtime.write().await;
            for (key, value) in &play.vars {
                runtime.set_play_var(key.clone(), value.clone());
            }
        }

        // Resolve hosts for this play
        let hosts = self.resolve_hosts(&play.hosts).await?;

        if hosts.is_empty() {
            warn!("No hosts matched for play: {}", play.name);
            return Ok(HashMap::new());
        }

        debug!("Executing on {} hosts", hosts.len());

        // Execute based on strategy
        let results = match self.config.strategy {
            ExecutionStrategy::Linear => self.run_linear(&hosts, &play.tasks).await?,
            ExecutionStrategy::Free => self.run_free(&hosts, &play.tasks).await?,
            ExecutionStrategy::HostPinned => self.run_host_pinned(&hosts, &play.tasks).await?,
        };

        // Flush handlers at end of play
        self.flush_handlers().await?;

        info!("Play completed: {}", play.name);
        Ok(results)
    }

    /// Run tasks in linear strategy (all hosts per task before next task)
    async fn run_linear(
        &self,
        hosts: &[String],
        tasks: &[Task],
    ) -> ExecutorResult<HashMap<String, HostResult>> {
        let mut results: HashMap<String, HostResult> = hosts
            .iter()
            .map(|h| {
                (
                    h.clone(),
                    HostResult {
                        host: h.clone(),
                        stats: ExecutionStats::default(),
                        failed: false,
                        unreachable: false,
                    },
                )
            })
            .collect();

        for task in tasks {
            // Filter hosts that haven't failed
            let active_hosts: Vec<_> = hosts
                .iter()
                .filter(|h| !results.get(*h).map(|r| r.failed || r.unreachable).unwrap_or(false))
                .cloned()
                .collect();

            if active_hosts.is_empty() {
                warn!("All hosts have failed, stopping execution");
                break;
            }

            // Run task on all active hosts in parallel (limited by semaphore)
            let task_results = self.run_task_on_hosts(&active_hosts, task).await?;

            // Update results
            for (host, task_result) in task_results {
                if let Some(host_result) = results.get_mut(&host) {
                    self.update_host_stats(host_result, &task_result);
                }
            }
        }

        Ok(results)
    }

    /// Run tasks in free strategy (each host runs independently)
    async fn run_free(
        &self,
        hosts: &[String],
        tasks: &[Task],
    ) -> ExecutorResult<HashMap<String, HostResult>> {
        let tasks = Arc::new(tasks.to_vec());
        let results = Arc::new(Mutex::new(HashMap::new()));

        let handles: Vec<_> = hosts
            .iter()
            .map(|host| {
                let host = host.clone();
                let tasks = Arc::clone(&tasks);
                let results = Arc::clone(&results);
                let semaphore = Arc::clone(&self.semaphore);
                let runtime = Arc::clone(&self.runtime);
                let config = self.config.clone();
                let handlers = Arc::clone(&self.handlers);
                let notified = Arc::clone(&self.notified_handlers);

                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();

                    let mut host_result = HostResult {
                        host: host.clone(),
                        stats: ExecutionStats::default(),
                        failed: false,
                        unreachable: false,
                    };

                    for task in tasks.iter() {
                        if host_result.failed || host_result.unreachable {
                            break;
                        }

                        let ctx = ExecutionContext {
                            host: host.clone(),
                            check_mode: config.check_mode,
                            diff_mode: config.diff_mode,
                        };

                        let task_result = task.execute(&ctx, &runtime, &handlers, &notified).await;

                        match task_result {
                            Ok(result) => {
                                update_stats(&mut host_result.stats, &result);
                                if result.status == TaskStatus::Failed {
                                    host_result.failed = true;
                                }
                            }
                            Err(_) => {
                                host_result.failed = true;
                                host_result.stats.failed += 1;
                            }
                        }
                    }

                    results.lock().await.insert(host, host_result);
                })
            })
            .collect();

        join_all(handles).await;

        let results = Arc::try_unwrap(results)
            .map_err(|_| ExecutorError::RuntimeError("Failed to unwrap results".into()))?
            .into_inner();

        Ok(results)
    }

    /// Run tasks in host_pinned strategy (dedicated worker per host)
    async fn run_host_pinned(
        &self,
        hosts: &[String],
        tasks: &[Task],
    ) -> ExecutorResult<HashMap<String, HostResult>> {
        // For now, host_pinned behaves like free strategy
        // In a full implementation, this would pin workers to specific hosts
        self.run_free(hosts, tasks).await
    }

    /// Run a single task on multiple hosts in parallel
    async fn run_task_on_hosts(
        &self,
        hosts: &[String],
        task: &Task,
    ) -> ExecutorResult<HashMap<String, TaskResult>> {
        debug!("Running task '{}' on {} hosts", task.name, hosts.len());

        let results = Arc::new(Mutex::new(HashMap::new()));

        let handles: Vec<_> = hosts
            .iter()
            .map(|host| {
                let host = host.clone();
                let task = task.clone();
                let results = Arc::clone(&results);
                let semaphore = Arc::clone(&self.semaphore);
                let runtime = Arc::clone(&self.runtime);
                let config = self.config.clone();
                let handlers = Arc::clone(&self.handlers);
                let notified = Arc::clone(&self.notified_handlers);

                tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();

                    let ctx = ExecutionContext {
                        host: host.clone(),
                        check_mode: config.check_mode,
                        diff_mode: config.diff_mode,
                    };

                    let result = task.execute(&ctx, &runtime, &handlers, &notified).await;

                    match result {
                        Ok(task_result) => {
                            results.lock().await.insert(host, task_result);
                        }
                        Err(e) => {
                            error!("Task failed on host {}: {}", host, e);
                            results.lock().await.insert(
                                host,
                                TaskResult {
                                    status: TaskStatus::Failed,
                                    changed: false,
                                    msg: Some(e.to_string()),
                                    result: None,
                                    diff: None,
                                },
                            );
                        }
                    }
                })
            })
            .collect();

        join_all(handles).await;

        let results = Arc::try_unwrap(results)
            .map_err(|_| ExecutorError::RuntimeError("Failed to unwrap results".into()))?
            .into_inner();

        Ok(results)
    }

    /// Update host statistics based on task result
    fn update_host_stats(&self, host_result: &mut HostResult, task_result: &TaskResult) {
        update_stats(&mut host_result.stats, task_result);
        if task_result.status == TaskStatus::Failed {
            host_result.failed = true;
        } else if task_result.status == TaskStatus::Unreachable {
            host_result.unreachable = true;
        }
    }

    /// Resolve host pattern to list of hosts
    async fn resolve_hosts(&self, pattern: &str) -> ExecutorResult<Vec<String>> {
        let runtime = self.runtime.read().await;

        // Handle special patterns
        if pattern == "all" {
            return Ok(runtime.get_all_hosts());
        }

        if pattern == "localhost" {
            return Ok(vec!["localhost".to_string()]);
        }

        // Check for group name
        if let Some(hosts) = runtime.get_group_hosts(pattern) {
            return Ok(hosts);
        }

        // Check for regex pattern (starts with ~)
        if let Some(regex_pattern) = pattern.strip_prefix('~') {
            let re = regex::Regex::new(regex_pattern)
                .map_err(|e| ExecutorError::ParseError(format!("Invalid regex: {}", e)))?;

            let all_hosts = runtime.get_all_hosts();
            let matched: Vec<_> = all_hosts
                .into_iter()
                .filter(|h| re.is_match(h))
                .collect();

            return Ok(matched);
        }

        // Treat as single host or comma-separated list
        let hosts: Vec<String> = pattern.split(',').map(|s| s.trim().to_string()).collect();
        Ok(hosts)
    }

    /// Flush all notified handlers
    async fn flush_handlers(&self) -> ExecutorResult<()> {
        let notified: Vec<String> = {
            let mut notified = self.notified_handlers.lock().await;
            let handlers: Vec<_> = notified.drain().collect();
            handlers
        };

        if notified.is_empty() {
            return Ok(());
        }

        info!("Running {} notified handlers", notified.len());

        let handlers = self.handlers.read().await;

        for handler_name in notified {
            if let Some(handler) = handlers.get(&handler_name) {
                debug!("Running handler: {}", handler_name);

                // Create task from handler
                let task = Task {
                    name: handler.name.clone(),
                    module: handler.module.clone(),
                    args: handler.args.clone(),
                    when: handler.when.clone(),
                    notify: Vec::new(),
                    register: None,
                    loop_items: None,
                    loop_var: "item".to_string(),
                    ignore_errors: false,
                    changed_when: None,
                    failed_when: None,
                    delegate_to: None,
                    run_once: false,
                    tags: Vec::new(),
                    r#become: false,
                    become_user: None,
                };

                // Get all active hosts from runtime
                let hosts = {
                    let runtime = self.runtime.read().await;
                    runtime.get_all_hosts()
                };

                // Run handler on all hosts
                let _ = self.run_task_on_hosts(&hosts, &task).await?;
            } else {
                warn!("Handler not found: {}", handler_name);
            }
        }

        Ok(())
    }

    /// Notify a handler to be run at end of play
    pub async fn notify_handler(&self, handler_name: &str) {
        let mut notified = self.notified_handlers.lock().await;
        notified.insert(handler_name.to_string());
        debug!("Handler notified: {}", handler_name);
    }

    /// Check if running in dry-run mode
    pub fn is_check_mode(&self) -> bool {
        self.config.check_mode
    }

    /// Get reference to runtime context
    pub fn runtime(&self) -> Arc<RwLock<RuntimeContext>> {
        Arc::clone(&self.runtime)
    }

    /// Get execution statistics summary
    pub fn summarize_results(results: &HashMap<String, HostResult>) -> ExecutionStats {
        let mut summary = ExecutionStats::default();
        for result in results.values() {
            summary.merge(&result.stats);
        }
        summary
    }
}

/// Helper function to update statistics
fn update_stats(stats: &mut ExecutionStats, result: &TaskResult) {
    match result.status {
        TaskStatus::Ok => {
            if result.changed {
                stats.changed += 1;
            } else {
                stats.ok += 1;
            }
        }
        TaskStatus::Changed => stats.changed += 1,
        TaskStatus::Failed => stats.failed += 1,
        TaskStatus::Skipped => stats.skipped += 1,
        TaskStatus::Unreachable => stats.unreachable += 1,
    }
}

/// Dependency graph for task ordering
pub struct DependencyGraph {
    nodes: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Add a dependency: task depends on dependency
    pub fn add_dependency(&mut self, task: &str, dependency: &str) {
        self.nodes
            .entry(task.to_string())
            .or_default()
            .push(dependency.to_string());
    }

    /// Get topologically sorted task order
    pub fn topological_sort(&self) -> ExecutorResult<Vec<String>> {
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();
        let mut result = Vec::new();

        for node in self.nodes.keys() {
            if !visited.contains(node) {
                self.visit(node, &mut visited, &mut temp_visited, &mut result)?;
            }
        }

        result.reverse();
        Ok(result)
    }

    fn visit(
        &self,
        node: &str,
        visited: &mut HashSet<String>,
        temp_visited: &mut HashSet<String>,
        result: &mut Vec<String>,
    ) -> ExecutorResult<()> {
        if temp_visited.contains(node) {
            return Err(ExecutorError::DependencyCycle(node.to_string()));
        }

        if !visited.contains(node) {
            temp_visited.insert(node.to_string());

            if let Some(deps) = self.nodes.get(node) {
                for dep in deps {
                    self.visit(dep, visited, temp_visited, result)?;
                }
            }

            temp_visited.remove(node);
            visited.insert(node.to_string());
            result.push(node.to_string());
        }

        Ok(())
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for PlaybookExecutor (same as Executor)
/// Used for API compatibility and clarity
pub type PlaybookExecutor = Executor;

/// Type alias for TaskExecutor functionality
/// In a more complex implementation, this could be a separate struct
pub type TaskExecutor = Executor;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_graph_no_cycle() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("task3", "task2");
        graph.add_dependency("task2", "task1");

        let order = graph.topological_sort().unwrap();
        assert_eq!(order, vec!["task1", "task2", "task3"]);
    }

    #[test]
    fn test_dependency_graph_cycle_detection() {
        let mut graph = DependencyGraph::new();
        graph.add_dependency("task1", "task2");
        graph.add_dependency("task2", "task3");
        graph.add_dependency("task3", "task1");

        let result = graph.topological_sort();
        assert!(matches!(result, Err(ExecutorError::DependencyCycle(_))));
    }

    #[test]
    fn test_execution_stats_merge() {
        let mut stats1 = ExecutionStats {
            ok: 1,
            changed: 2,
            failed: 0,
            skipped: 1,
            unreachable: 0,
        };

        let stats2 = ExecutionStats {
            ok: 2,
            changed: 1,
            failed: 1,
            skipped: 0,
            unreachable: 1,
        };

        stats1.merge(&stats2);

        assert_eq!(stats1.ok, 3);
        assert_eq!(stats1.changed, 3);
        assert_eq!(stats1.failed, 1);
        assert_eq!(stats1.skipped, 1);
        assert_eq!(stats1.unreachable, 1);
    }
}
