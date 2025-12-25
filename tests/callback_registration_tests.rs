//! Registration tests for CallbackManager plugin system.
//!
//! This test module covers:
//! 1. Register single plugin
//! 2. Register multiple plugins
//! 3. Deregister plugin
//! 4. Replace existing plugin
//! 5. Plugin name conflicts

use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustible::facts::Facts;
use rustible::traits::{ExecutionCallback, ExecutionResult, ModuleResult};

// ============================================================================
// CallbackManager Implementation for Testing
// ============================================================================

/// Error types for callback manager operations.
#[derive(Debug, Clone, PartialEq)]
pub enum CallbackError {
    /// Plugin with this name already exists
    PluginAlreadyExists(String),
    /// Plugin not found
    PluginNotFound(String),
    /// Invalid plugin name
    InvalidPluginName(String),
}

impl std::fmt::Display for CallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallbackError::PluginAlreadyExists(name) => {
                write!(f, "Plugin '{}' already exists", name)
            }
            CallbackError::PluginNotFound(name) => {
                write!(f, "Plugin '{}' not found", name)
            }
            CallbackError::InvalidPluginName(name) => {
                write!(f, "Invalid plugin name: '{}'", name)
            }
        }
    }
}

impl std::error::Error for CallbackError {}

/// Result type for callback manager operations.
pub type CallbackResult<T> = Result<T, CallbackError>;

/// Options for registering a callback plugin.
#[derive(Debug, Clone)]
pub struct RegisterOptions {
    /// Whether to replace an existing plugin with the same name
    pub replace_existing: bool,
    /// Priority for callback execution (lower = earlier)
    pub priority: i32,
    /// Whether the plugin is enabled
    pub enabled: bool,
}

impl Default for RegisterOptions {
    fn default() -> Self {
        Self {
            replace_existing: false,
            priority: 0,
            enabled: true, // Enabled by default
        }
    }
}

impl RegisterOptions {
    /// Create new options with replace_existing set to true.
    pub fn replace() -> Self {
        Self {
            replace_existing: true,
            ..Default::default()
        }
    }

    /// Create new options with a specific priority.
    pub fn with_priority(priority: i32) -> Self {
        Self {
            priority,
            enabled: true,
            ..Default::default()
        }
    }
}

/// Metadata about a registered plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin name
    pub name: String,
    /// Priority for execution order
    pub priority: i32,
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Number of times the plugin has been invoked
    pub invocation_count: u32,
}

/// Manages registered callback plugins.
///
/// The CallbackManager allows registering, deregistering, and invoking
/// multiple callback plugins. Plugins are executed in priority order.
#[derive(Debug)]
pub struct CallbackManager {
    /// Registered plugins keyed by name
    plugins: RwLock<HashMap<String, PluginEntry>>,
}

/// Internal entry for a registered plugin.
struct PluginEntry {
    /// The callback implementation
    callback: Arc<dyn ExecutionCallback>,
    /// Plugin priority (lower = earlier)
    priority: i32,
    /// Whether the plugin is enabled
    enabled: bool,
    /// Invocation counter
    invocation_count: AtomicU32,
}

impl std::fmt::Debug for PluginEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginEntry")
            .field("priority", &self.priority)
            .field("enabled", &self.enabled)
            .field(
                "invocation_count",
                &self.invocation_count.load(Ordering::SeqCst),
            )
            .finish()
    }
}

impl CallbackManager {
    /// Creates a new empty callback manager.
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }

    /// Returns the number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.read().len()
    }

    /// Returns whether a plugin with the given name exists.
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.read().contains_key(name)
    }

    /// Returns a list of all registered plugin names.
    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.read().keys().cloned().collect()
    }

    /// Returns information about a specific plugin.
    pub fn plugin_info(&self, name: &str) -> Option<PluginInfo> {
        self.plugins.read().get(name).map(|entry| PluginInfo {
            name: name.to_string(),
            priority: entry.priority,
            enabled: entry.enabled,
            invocation_count: entry.invocation_count.load(Ordering::SeqCst),
        })
    }

    /// Validates a plugin name.
    fn validate_name(name: &str) -> CallbackResult<()> {
        if name.is_empty() {
            return Err(CallbackError::InvalidPluginName(
                "Plugin name cannot be empty".to_string(),
            ));
        }
        if name.len() > 64 {
            return Err(CallbackError::InvalidPluginName(
                "Plugin name too long (max 64 chars)".to_string(),
            ));
        }
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(CallbackError::InvalidPluginName(
                "Plugin name must contain only alphanumeric, underscore, or hyphen".to_string(),
            ));
        }
        Ok(())
    }

    /// Registers a callback plugin with the given name.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the plugin
    /// * `callback` - The callback implementation
    ///
    /// # Errors
    ///
    /// Returns an error if a plugin with the same name already exists.
    pub fn register(
        &self,
        name: impl Into<String>,
        callback: Arc<dyn ExecutionCallback>,
    ) -> CallbackResult<()> {
        self.register_with_options(name, callback, RegisterOptions::default())
    }

    /// Registers a callback plugin with custom options.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the plugin
    /// * `callback` - The callback implementation
    /// * `options` - Registration options
    ///
    /// # Errors
    ///
    /// Returns an error if a plugin with the same name exists and
    /// `replace_existing` is false.
    pub fn register_with_options(
        &self,
        name: impl Into<String>,
        callback: Arc<dyn ExecutionCallback>,
        options: RegisterOptions,
    ) -> CallbackResult<()> {
        let name = name.into();
        Self::validate_name(&name)?;

        let mut plugins = self.plugins.write();

        if plugins.contains_key(&name) && !options.replace_existing {
            return Err(CallbackError::PluginAlreadyExists(name));
        }

        plugins.insert(
            name,
            PluginEntry {
                callback,
                priority: options.priority,
                enabled: options.enabled,
                invocation_count: AtomicU32::new(0),
            },
        );

        Ok(())
    }

    /// Deregisters a callback plugin by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin to remove
    ///
    /// # Errors
    ///
    /// Returns an error if no plugin with the given name exists.
    pub fn deregister(&self, name: &str) -> CallbackResult<Arc<dyn ExecutionCallback>> {
        let mut plugins = self.plugins.write();

        match plugins.remove(name) {
            Some(entry) => Ok(entry.callback),
            None => Err(CallbackError::PluginNotFound(name.to_string())),
        }
    }

    /// Replaces an existing plugin with a new one.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the plugin to replace
    /// * `callback` - The new callback implementation
    ///
    /// # Errors
    ///
    /// Returns an error if no plugin with the given name exists.
    pub fn replace(
        &self,
        name: &str,
        callback: Arc<dyn ExecutionCallback>,
    ) -> CallbackResult<Arc<dyn ExecutionCallback>> {
        let mut plugins = self.plugins.write();

        if let Some(entry) = plugins.get_mut(name) {
            let old_callback = std::mem::replace(&mut entry.callback, callback);
            entry.invocation_count.store(0, Ordering::SeqCst);
            Ok(old_callback)
        } else {
            Err(CallbackError::PluginNotFound(name.to_string()))
        }
    }

    /// Enables or disables a plugin.
    pub fn set_enabled(&self, name: &str, enabled: bool) -> CallbackResult<()> {
        let mut plugins = self.plugins.write();

        if let Some(entry) = plugins.get_mut(name) {
            entry.enabled = enabled;
            Ok(())
        } else {
            Err(CallbackError::PluginNotFound(name.to_string()))
        }
    }

    /// Returns plugins sorted by priority for execution.
    fn get_sorted_plugins(&self) -> Vec<(String, Arc<dyn ExecutionCallback>)> {
        let plugins = self.plugins.read();
        let mut entries: Vec<_> = plugins
            .iter()
            .filter(|(_, entry)| entry.enabled)
            .map(|(name, entry)| (name.clone(), entry.callback.clone(), entry.priority))
            .collect();

        entries.sort_by_key(|(_, _, priority)| *priority);
        entries
            .into_iter()
            .map(|(name, callback, _)| (name, callback))
            .collect()
    }

    /// Increments invocation count for a plugin.
    fn increment_invocation(&self, name: &str) {
        if let Some(entry) = self.plugins.read().get(name) {
            entry.invocation_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Clears all registered plugins.
    pub fn clear(&self) {
        self.plugins.write().clear();
    }
}

impl Default for CallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionCallback for CallbackManager {
    async fn on_playbook_start(&self, name: &str) {
        for (plugin_name, callback) in self.get_sorted_plugins() {
            callback.on_playbook_start(name).await;
            self.increment_invocation(&plugin_name);
        }
    }

    async fn on_playbook_end(&self, name: &str, success: bool) {
        for (plugin_name, callback) in self.get_sorted_plugins() {
            callback.on_playbook_end(name, success).await;
            self.increment_invocation(&plugin_name);
        }
    }

    async fn on_play_start(&self, name: &str, hosts: &[String]) {
        for (plugin_name, callback) in self.get_sorted_plugins() {
            callback.on_play_start(name, hosts).await;
            self.increment_invocation(&plugin_name);
        }
    }

    async fn on_play_end(&self, name: &str, success: bool) {
        for (plugin_name, callback) in self.get_sorted_plugins() {
            callback.on_play_end(name, success).await;
            self.increment_invocation(&plugin_name);
        }
    }

    async fn on_task_start(&self, name: &str, host: &str) {
        for (plugin_name, callback) in self.get_sorted_plugins() {
            callback.on_task_start(name, host).await;
            self.increment_invocation(&plugin_name);
        }
    }

    async fn on_task_complete(&self, result: &ExecutionResult) {
        for (plugin_name, callback) in self.get_sorted_plugins() {
            callback.on_task_complete(result).await;
            self.increment_invocation(&plugin_name);
        }
    }

    async fn on_handler_triggered(&self, name: &str) {
        for (plugin_name, callback) in self.get_sorted_plugins() {
            callback.on_handler_triggered(name).await;
            self.increment_invocation(&plugin_name);
        }
    }

    async fn on_facts_gathered(&self, host: &str, facts: &Facts) {
        for (plugin_name, callback) in self.get_sorted_plugins() {
            callback.on_facts_gathered(host, facts).await;
            self.increment_invocation(&plugin_name);
        }
    }
}

// ============================================================================
// Mock Callback for Testing
// ============================================================================

/// A mock callback that tracks invocations for testing.
#[derive(Debug, Default)]
pub struct TestCallback {
    pub name: String,
    pub playbook_start_count: AtomicU32,
    pub playbook_end_count: AtomicU32,
    pub task_complete_count: AtomicU32,
    pub events: RwLock<Vec<String>>,
}

impl TestCallback {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    pub fn event_count(&self) -> usize {
        self.events.read().len()
    }
}

#[async_trait]
impl ExecutionCallback for TestCallback {
    async fn on_playbook_start(&self, name: &str) {
        self.playbook_start_count.fetch_add(1, Ordering::SeqCst);
        self.events
            .write()
            .push(format!("{}:playbook_start:{}", self.name, name));
    }

    async fn on_playbook_end(&self, name: &str, success: bool) {
        self.playbook_end_count.fetch_add(1, Ordering::SeqCst);
        self.events
            .write()
            .push(format!("{}:playbook_end:{}:{}", self.name, name, success));
    }

    async fn on_task_complete(&self, result: &ExecutionResult) {
        self.task_complete_count.fetch_add(1, Ordering::SeqCst);
        self.events.write().push(format!(
            "{}:task_complete:{}:{}",
            self.name, result.task_name, result.host
        ));
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn create_test_result(task_name: &str, host: &str) -> ExecutionResult {
    ExecutionResult {
        host: host.to_string(),
        task_name: task_name.to_string(),
        result: ModuleResult::ok("OK"),
        duration: Duration::from_millis(100),
        notify: vec![],
    }
}

// ============================================================================
// Test 1: Register Single Plugin
// ============================================================================

#[tokio::test]
async fn test_register_single_plugin() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("test-plugin"));

    let result = manager.register("test-plugin", callback.clone());

    assert!(result.is_ok());
    assert_eq!(manager.plugin_count(), 1);
    assert!(manager.has_plugin("test-plugin"));
}

#[tokio::test]
async fn test_register_single_plugin_with_options() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("priority-plugin"));

    let options = RegisterOptions {
        priority: 10,
        enabled: true,
        replace_existing: false,
    };

    let result = manager.register_with_options("priority-plugin", callback.clone(), options);

    assert!(result.is_ok());
    let info = manager.plugin_info("priority-plugin").unwrap();
    assert_eq!(info.priority, 10);
    assert!(info.enabled);
}

#[tokio::test]
async fn test_register_single_plugin_invokes_correctly() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("invoke-test"));

    manager.register("invoke-test", callback.clone()).unwrap();

    manager.on_playbook_start("test-playbook").await;

    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);
    assert!(callback
        .events
        .read()
        .contains(&"invoke-test:playbook_start:test-playbook".to_string()));
}

#[tokio::test]
async fn test_register_plugin_validates_name() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("test"));

    // Empty name
    let result = manager.register("", callback.clone());
    assert!(matches!(result, Err(CallbackError::InvalidPluginName(_))));

    // Name with invalid characters
    let result = manager.register("test plugin!", callback.clone());
    assert!(matches!(result, Err(CallbackError::InvalidPluginName(_))));

    // Name too long
    let long_name = "a".repeat(65);
    let result = manager.register(long_name, callback.clone());
    assert!(matches!(result, Err(CallbackError::InvalidPluginName(_))));

    // Valid names
    assert!(manager.register("valid_name", callback.clone()).is_ok());
    assert!(manager.register("valid-name-2", callback.clone()).is_ok());
    assert!(manager.register("ValidName123", callback.clone()).is_ok());
}

// ============================================================================
// Test 2: Register Multiple Plugins
// ============================================================================

#[tokio::test]
async fn test_register_multiple_plugins() {
    let manager = CallbackManager::new();

    let callback1 = Arc::new(TestCallback::new("plugin-1"));
    let callback2 = Arc::new(TestCallback::new("plugin-2"));
    let callback3 = Arc::new(TestCallback::new("plugin-3"));

    manager.register("plugin-1", callback1.clone()).unwrap();
    manager.register("plugin-2", callback2.clone()).unwrap();
    manager.register("plugin-3", callback3.clone()).unwrap();

    assert_eq!(manager.plugin_count(), 3);
    assert!(manager.has_plugin("plugin-1"));
    assert!(manager.has_plugin("plugin-2"));
    assert!(manager.has_plugin("plugin-3"));
}

#[tokio::test]
async fn test_multiple_plugins_all_invoked() {
    let manager = CallbackManager::new();

    let callback1 = Arc::new(TestCallback::new("plugin-1"));
    let callback2 = Arc::new(TestCallback::new("plugin-2"));
    let callback3 = Arc::new(TestCallback::new("plugin-3"));

    manager.register("plugin-1", callback1.clone()).unwrap();
    manager.register("plugin-2", callback2.clone()).unwrap();
    manager.register("plugin-3", callback3.clone()).unwrap();

    manager.on_playbook_start("multi-test").await;

    assert_eq!(callback1.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback2.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback3.playbook_start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_multiple_plugins_priority_order() {
    let manager = CallbackManager::new();

    let callback_low = Arc::new(TestCallback::new("low-priority"));
    let callback_high = Arc::new(TestCallback::new("high-priority"));
    let callback_medium = Arc::new(TestCallback::new("medium-priority"));

    // Register in non-priority order
    manager
        .register_with_options(
            "medium-priority",
            callback_medium.clone(),
            RegisterOptions::with_priority(50),
        )
        .unwrap();
    manager
        .register_with_options(
            "high-priority",
            callback_high.clone(),
            RegisterOptions::with_priority(10),
        )
        .unwrap();
    manager
        .register_with_options(
            "low-priority",
            callback_low.clone(),
            RegisterOptions::with_priority(100),
        )
        .unwrap();

    manager.on_playbook_start("priority-test").await;

    // All callbacks should be invoked
    assert_eq!(
        callback_high.playbook_start_count.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        callback_medium.playbook_start_count.load(Ordering::SeqCst),
        1
    );
    assert_eq!(callback_low.playbook_start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_multiple_plugins_task_complete() {
    let manager = CallbackManager::new();

    let callback1 = Arc::new(TestCallback::new("plugin-1"));
    let callback2 = Arc::new(TestCallback::new("plugin-2"));

    manager.register("plugin-1", callback1.clone()).unwrap();
    manager.register("plugin-2", callback2.clone()).unwrap();

    let result = create_test_result("install-nginx", "webserver1");
    manager.on_task_complete(&result).await;

    assert_eq!(callback1.task_complete_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback2.task_complete_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_plugin_names_list() {
    let manager = CallbackManager::new();

    manager
        .register("alpha", Arc::new(TestCallback::new("alpha")))
        .unwrap();
    manager
        .register("beta", Arc::new(TestCallback::new("beta")))
        .unwrap();
    manager
        .register("gamma", Arc::new(TestCallback::new("gamma")))
        .unwrap();

    let names = manager.plugin_names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"alpha".to_string()));
    assert!(names.contains(&"beta".to_string()));
    assert!(names.contains(&"gamma".to_string()));
}

// ============================================================================
// Test 3: Deregister Plugin
// ============================================================================

#[tokio::test]
async fn test_deregister_plugin() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("to-remove"));

    manager.register("to-remove", callback.clone()).unwrap();
    assert_eq!(manager.plugin_count(), 1);

    let removed = manager.deregister("to-remove");

    assert!(removed.is_ok());
    assert_eq!(manager.plugin_count(), 0);
    assert!(!manager.has_plugin("to-remove"));
}

#[tokio::test]
async fn test_deregister_nonexistent_plugin() {
    let manager = CallbackManager::new();

    let result = manager.deregister("does-not-exist");

    assert!(matches!(result, Err(CallbackError::PluginNotFound(_))));
}

#[tokio::test]
async fn test_deregister_stops_invocations() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("removable"));

    manager.register("removable", callback.clone()).unwrap();

    // First invocation
    manager.on_playbook_start("test1").await;
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);

    // Deregister
    manager.deregister("removable").unwrap();

    // Second invocation after deregistration
    manager.on_playbook_start("test2").await;

    // Count should still be 1 (no additional invocations)
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_deregister_one_of_many() {
    let manager = CallbackManager::new();

    let callback1 = Arc::new(TestCallback::new("keep-1"));
    let callback2 = Arc::new(TestCallback::new("remove"));
    let callback3 = Arc::new(TestCallback::new("keep-2"));

    manager.register("keep-1", callback1.clone()).unwrap();
    manager.register("remove", callback2.clone()).unwrap();
    manager.register("keep-2", callback3.clone()).unwrap();

    manager.deregister("remove").unwrap();

    assert_eq!(manager.plugin_count(), 2);
    assert!(manager.has_plugin("keep-1"));
    assert!(!manager.has_plugin("remove"));
    assert!(manager.has_plugin("keep-2"));

    // Verify remaining plugins still work
    manager.on_playbook_start("test").await;
    assert_eq!(callback1.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback2.playbook_start_count.load(Ordering::SeqCst), 0);
    assert_eq!(callback3.playbook_start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_clear_all_plugins() {
    let manager = CallbackManager::new();

    manager
        .register("plugin-1", Arc::new(TestCallback::new("p1")))
        .unwrap();
    manager
        .register("plugin-2", Arc::new(TestCallback::new("p2")))
        .unwrap();
    manager
        .register("plugin-3", Arc::new(TestCallback::new("p3")))
        .unwrap();

    assert_eq!(manager.plugin_count(), 3);

    manager.clear();

    assert_eq!(manager.plugin_count(), 0);
    assert!(!manager.has_plugin("plugin-1"));
    assert!(!manager.has_plugin("plugin-2"));
    assert!(!manager.has_plugin("plugin-3"));
}

// ============================================================================
// Test 4: Replace Existing Plugin
// ============================================================================

#[tokio::test]
async fn test_replace_existing_plugin() {
    let manager = CallbackManager::new();

    let original = Arc::new(TestCallback::new("original"));
    let replacement = Arc::new(TestCallback::new("replacement"));

    manager.register("my-plugin", original.clone()).unwrap();

    let old = manager.replace("my-plugin", replacement.clone());

    assert!(old.is_ok());
    assert_eq!(manager.plugin_count(), 1);
}

#[tokio::test]
async fn test_replace_nonexistent_plugin() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("new"));

    let result = manager.replace("nonexistent", callback);

    assert!(matches!(result, Err(CallbackError::PluginNotFound(_))));
}

#[tokio::test]
async fn test_replace_invokes_new_callback() {
    let manager = CallbackManager::new();

    let original = Arc::new(TestCallback::new("original"));
    let replacement = Arc::new(TestCallback::new("replacement"));

    manager.register("my-plugin", original.clone()).unwrap();

    // Invoke before replacement
    manager.on_playbook_start("before-replace").await;
    assert_eq!(original.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(replacement.playbook_start_count.load(Ordering::SeqCst), 0);

    // Replace
    manager.replace("my-plugin", replacement.clone()).unwrap();

    // Invoke after replacement
    manager.on_playbook_start("after-replace").await;
    assert_eq!(original.playbook_start_count.load(Ordering::SeqCst), 1); // Still 1
    assert_eq!(replacement.playbook_start_count.load(Ordering::SeqCst), 1); // Now 1
}

#[tokio::test]
async fn test_register_with_replace_option() {
    let manager = CallbackManager::new();

    let original = Arc::new(TestCallback::new("original"));
    let replacement = Arc::new(TestCallback::new("replacement"));

    manager.register("my-plugin", original.clone()).unwrap();

    // Try without replace option - should fail
    let result = manager.register("my-plugin", replacement.clone());
    assert!(matches!(result, Err(CallbackError::PluginAlreadyExists(_))));

    // Try with replace option - should succeed
    let result =
        manager.register_with_options("my-plugin", replacement.clone(), RegisterOptions::replace());
    assert!(result.is_ok());

    // Verify replacement is active
    manager.on_playbook_start("test").await;
    assert_eq!(original.playbook_start_count.load(Ordering::SeqCst), 0);
    assert_eq!(replacement.playbook_start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_replace_resets_invocation_count() {
    let manager = CallbackManager::new();

    let original = Arc::new(TestCallback::new("original"));
    let replacement = Arc::new(TestCallback::new("replacement"));

    manager.register("my-plugin", original.clone()).unwrap();

    // Invoke multiple times
    manager.on_playbook_start("test1").await;
    manager.on_playbook_start("test2").await;
    manager.on_playbook_start("test3").await;

    let info_before = manager.plugin_info("my-plugin").unwrap();
    assert_eq!(info_before.invocation_count, 3);

    // Replace
    manager.replace("my-plugin", replacement.clone()).unwrap();

    // Invocation count should be reset
    let info_after = manager.plugin_info("my-plugin").unwrap();
    assert_eq!(info_after.invocation_count, 0);
}

// ============================================================================
// Test 5: Plugin Name Conflicts
// ============================================================================

#[tokio::test]
async fn test_duplicate_name_rejected() {
    let manager = CallbackManager::new();

    let callback1 = Arc::new(TestCallback::new("first"));
    let callback2 = Arc::new(TestCallback::new("second"));

    manager.register("same-name", callback1.clone()).unwrap();

    let result = manager.register("same-name", callback2.clone());

    assert!(
        matches!(result, Err(CallbackError::PluginAlreadyExists(name)) if name == "same-name")
    );
    assert_eq!(manager.plugin_count(), 1);
}

#[tokio::test]
async fn test_name_conflict_preserves_original() {
    let manager = CallbackManager::new();

    let original = Arc::new(TestCallback::new("original"));
    let duplicate = Arc::new(TestCallback::new("duplicate"));

    manager.register("my-plugin", original.clone()).unwrap();

    // Attempt duplicate registration
    let _ = manager.register("my-plugin", duplicate.clone());

    // Invoke - only original should be called
    manager.on_playbook_start("conflict-test").await;

    assert_eq!(original.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(duplicate.playbook_start_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_name_case_sensitivity() {
    let manager = CallbackManager::new();

    let callback1 = Arc::new(TestCallback::new("lower"));
    let callback2 = Arc::new(TestCallback::new("upper"));
    let callback3 = Arc::new(TestCallback::new("mixed"));

    // Names are case-sensitive
    manager.register("myPlugin", callback1.clone()).unwrap();
    manager.register("MYPLUGIN", callback2.clone()).unwrap();
    manager.register("MyPlugin", callback3.clone()).unwrap();

    assert_eq!(manager.plugin_count(), 3);
    assert!(manager.has_plugin("myPlugin"));
    assert!(manager.has_plugin("MYPLUGIN"));
    assert!(manager.has_plugin("MyPlugin"));
}

#[tokio::test]
async fn test_reregister_after_deregister() {
    let manager = CallbackManager::new();

    let callback1 = Arc::new(TestCallback::new("version-1"));
    let callback2 = Arc::new(TestCallback::new("version-2"));

    // Register first version
    manager.register("plugin", callback1.clone()).unwrap();
    manager.on_playbook_start("test1").await;
    assert_eq!(callback1.playbook_start_count.load(Ordering::SeqCst), 1);

    // Deregister
    manager.deregister("plugin").unwrap();

    // Register second version with same name
    manager.register("plugin", callback2.clone()).unwrap();
    manager.on_playbook_start("test2").await;

    // Only new callback should be invoked
    assert_eq!(callback1.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback2.playbook_start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_similar_names_distinct() {
    let manager = CallbackManager::new();

    let callback1 = Arc::new(TestCallback::new("plugin"));
    let callback2 = Arc::new(TestCallback::new("plugin-1"));
    let callback3 = Arc::new(TestCallback::new("plugin_1"));

    manager.register("plugin", callback1.clone()).unwrap();
    manager.register("plugin-1", callback2.clone()).unwrap();
    manager.register("plugin_1", callback3.clone()).unwrap();

    assert_eq!(manager.plugin_count(), 3);

    manager.on_playbook_start("test").await;

    assert_eq!(callback1.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback2.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback3.playbook_start_count.load(Ordering::SeqCst), 1);
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_enable_disable_plugin() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("toggle-plugin"));

    let options = RegisterOptions {
        enabled: true,
        ..Default::default()
    };
    manager
        .register_with_options("toggle-plugin", callback.clone(), options)
        .unwrap();

    // Invoke while enabled
    manager.on_playbook_start("enabled-test").await;
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);

    // Disable
    manager.set_enabled("toggle-plugin", false).unwrap();

    // Invoke while disabled
    manager.on_playbook_start("disabled-test").await;
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1); // Still 1

    // Re-enable
    manager.set_enabled("toggle-plugin", true).unwrap();

    // Invoke after re-enable
    manager.on_playbook_start("reenabled-test").await;
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_plugin_invocation_count_tracking() {
    let manager = CallbackManager::new();
    let callback = Arc::new(TestCallback::new("count-test"));

    manager.register("count-test", callback.clone()).unwrap();

    // Multiple invocations
    for _ in 0..5 {
        manager.on_playbook_start("test").await;
    }

    let info = manager.plugin_info("count-test").unwrap();
    assert_eq!(info.invocation_count, 5);
}

#[tokio::test]
async fn test_empty_manager_invocations() {
    let manager = CallbackManager::new();

    // These should not panic even with no plugins
    manager.on_playbook_start("empty-test").await;
    manager.on_playbook_end("empty-test", true).await;
    manager
        .on_play_start("play", &["host1".to_string()])
        .await;
    manager.on_play_end("play", true).await;
    manager.on_task_start("task", "host").await;
    manager
        .on_task_complete(&create_test_result("task", "host"))
        .await;
    manager.on_handler_triggered("handler").await;
    manager.on_facts_gathered("host", &Facts::new()).await;

    assert_eq!(manager.plugin_count(), 0);
}

#[tokio::test]
async fn test_concurrent_plugin_access() {
    use tokio::task::JoinSet;

    let manager = Arc::new(CallbackManager::new());

    // Register a callback
    let callback = Arc::new(TestCallback::new("concurrent"));
    manager.register("concurrent", callback.clone()).unwrap();

    // Spawn multiple tasks that invoke callbacks concurrently
    let mut join_set = JoinSet::new();
    for i in 0..100 {
        let mgr = manager.clone();
        join_set.spawn(async move {
            mgr.on_playbook_start(&format!("test-{}", i)).await;
        });
    }

    // Wait for all tasks
    while join_set.join_next().await.is_some() {}

    // All invocations should have been recorded
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 100);
}

#[tokio::test]
async fn test_default_trait_implementation() {
    let manager = CallbackManager::default();
    assert_eq!(manager.plugin_count(), 0);
}
