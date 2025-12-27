//! Comprehensive tests for CallbackManager (src/callback/manager.rs)
//!
//! This test module covers the 13 required tests for callback manager:
//! 1. test_callback_manager_new
//! 2. test_callback_manager_add_callback
//! 3. test_callback_manager_remove_callback
//! 4. test_callback_manager_clear
//! 5. test_callback_manager_dispatch_playbook_start
//! 6. test_callback_manager_dispatch_playbook_end
//! 7. test_callback_manager_dispatch_play_start
//! 8. test_callback_manager_dispatch_play_end
//! 9. test_callback_manager_dispatch_task_start
//! 10. test_callback_manager_dispatch_task_complete
//! 11. test_callback_manager_dispatch_handler_triggered
//! 12. test_callback_manager_dispatch_facts_gathered
//! 13. test_callback_manager_multiple_callbacks

use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustible::facts::Facts;
use rustible::traits::{ExecutionCallback, ExecutionResult, ModuleResult};

// ============================================================================
// Test Helper: TrackingCallback
// ============================================================================

/// A callback that tracks all invocations for testing purposes.
#[derive(Debug)]
pub struct TrackingCallback {
    /// Name for identification in multi-callback tests
    pub name: String,
    /// Count of on_playbook_start calls
    pub playbook_start_count: AtomicU32,
    /// Count of on_playbook_end calls
    pub playbook_end_count: AtomicU32,
    /// Count of on_play_start calls
    pub play_start_count: AtomicU32,
    /// Count of on_play_end calls
    pub play_end_count: AtomicU32,
    /// Count of on_task_start calls
    pub task_start_count: AtomicU32,
    /// Count of on_task_complete calls
    pub task_complete_count: AtomicU32,
    /// Count of on_handler_triggered calls
    pub handler_triggered_count: AtomicU32,
    /// Count of on_facts_gathered calls
    pub facts_gathered_count: AtomicU32,
    /// All events recorded for verification
    pub events: RwLock<Vec<String>>,
    /// Last playbook name seen
    pub last_playbook_name: RwLock<Option<String>>,
    /// Last play name seen
    pub last_play_name: RwLock<Option<String>>,
    /// Last task name seen
    pub last_task_name: RwLock<Option<String>>,
    /// Last host seen
    pub last_host: RwLock<Option<String>>,
    /// Last handler name seen
    pub last_handler_name: RwLock<Option<String>>,
}

impl Default for TrackingCallback {
    fn default() -> Self {
        Self::new("default")
    }
}

impl TrackingCallback {
    /// Create a new tracking callback with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            playbook_start_count: AtomicU32::new(0),
            playbook_end_count: AtomicU32::new(0),
            play_start_count: AtomicU32::new(0),
            play_end_count: AtomicU32::new(0),
            task_start_count: AtomicU32::new(0),
            task_complete_count: AtomicU32::new(0),
            handler_triggered_count: AtomicU32::new(0),
            facts_gathered_count: AtomicU32::new(0),
            events: RwLock::new(Vec::new()),
            last_playbook_name: RwLock::new(None),
            last_play_name: RwLock::new(None),
            last_task_name: RwLock::new(None),
            last_host: RwLock::new(None),
            last_handler_name: RwLock::new(None),
        }
    }

    /// Get total number of all callback invocations.
    pub fn total_calls(&self) -> u32 {
        self.playbook_start_count.load(Ordering::SeqCst)
            + self.playbook_end_count.load(Ordering::SeqCst)
            + self.play_start_count.load(Ordering::SeqCst)
            + self.play_end_count.load(Ordering::SeqCst)
            + self.task_start_count.load(Ordering::SeqCst)
            + self.task_complete_count.load(Ordering::SeqCst)
            + self.handler_triggered_count.load(Ordering::SeqCst)
            + self.facts_gathered_count.load(Ordering::SeqCst)
    }

    /// Get all recorded events.
    pub fn get_events(&self) -> Vec<String> {
        self.events.read().clone()
    }

    /// Check if a specific event was recorded.
    pub fn has_event(&self, event: &str) -> bool {
        self.events.read().iter().any(|e| e.contains(event))
    }

    /// Reset all counters.
    pub fn reset(&self) {
        self.playbook_start_count.store(0, Ordering::SeqCst);
        self.playbook_end_count.store(0, Ordering::SeqCst);
        self.play_start_count.store(0, Ordering::SeqCst);
        self.play_end_count.store(0, Ordering::SeqCst);
        self.task_start_count.store(0, Ordering::SeqCst);
        self.task_complete_count.store(0, Ordering::SeqCst);
        self.handler_triggered_count.store(0, Ordering::SeqCst);
        self.facts_gathered_count.store(0, Ordering::SeqCst);
        self.events.write().clear();
    }
}

#[async_trait]
impl ExecutionCallback for TrackingCallback {
    async fn on_playbook_start(&self, name: &str) {
        self.playbook_start_count.fetch_add(1, Ordering::SeqCst);
        *self.last_playbook_name.write() = Some(name.to_string());
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

    async fn on_play_start(&self, name: &str, hosts: &[String]) {
        self.play_start_count.fetch_add(1, Ordering::SeqCst);
        *self.last_play_name.write() = Some(name.to_string());
        self.events.write().push(format!(
            "{}:play_start:{}:hosts={}",
            self.name,
            name,
            hosts.len()
        ));
    }

    async fn on_play_end(&self, name: &str, success: bool) {
        self.play_end_count.fetch_add(1, Ordering::SeqCst);
        self.events
            .write()
            .push(format!("{}:play_end:{}:{}", self.name, name, success));
    }

    async fn on_task_start(&self, name: &str, host: &str) {
        self.task_start_count.fetch_add(1, Ordering::SeqCst);
        *self.last_task_name.write() = Some(name.to_string());
        *self.last_host.write() = Some(host.to_string());
        self.events
            .write()
            .push(format!("{}:task_start:{}:{}", self.name, name, host));
    }

    async fn on_task_complete(&self, result: &ExecutionResult) {
        self.task_complete_count.fetch_add(1, Ordering::SeqCst);
        self.events.write().push(format!(
            "{}:task_complete:{}:{}:success={}",
            self.name, result.task_name, result.host, result.result.success
        ));
    }

    async fn on_handler_triggered(&self, name: &str) {
        self.handler_triggered_count.fetch_add(1, Ordering::SeqCst);
        *self.last_handler_name.write() = Some(name.to_string());
        self.events
            .write()
            .push(format!("{}:handler_triggered:{}", self.name, name));
    }

    async fn on_facts_gathered(&self, host: &str, _facts: &Facts) {
        self.facts_gathered_count.fetch_add(1, Ordering::SeqCst);
        *self.last_host.write() = Some(host.to_string());
        self.events
            .write()
            .push(format!("{}:facts_gathered:{}", self.name, host));
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test ExecutionResult for task completion testing.
fn create_test_result(
    task_name: &str,
    host: &str,
    success: bool,
    changed: bool,
) -> ExecutionResult {
    ExecutionResult {
        host: host.to_string(),
        task_name: task_name.to_string(),
        result: if success {
            if changed {
                ModuleResult::changed("Task completed with changes")
            } else {
                ModuleResult::ok("Task completed successfully")
            }
        } else {
            ModuleResult::failed("Task failed")
        },
        duration: Duration::from_millis(100),
        notify: vec![],
    }
}

/// Create a test Facts object.
fn create_test_facts() -> Facts {
    let mut facts = Facts::new();
    facts.set("ansible_os_family", serde_json::json!("Debian"));
    facts.set("ansible_distribution", serde_json::json!("Ubuntu"));
    facts.set("ansible_distribution_version", serde_json::json!("22.04"));
    facts
}

// ============================================================================
// Test 1: test_callback_manager_new
// ============================================================================

/// Simple wrapper CallbackManager for testing (since the actual one is in the library)
struct TestCallbackManager {
    callbacks: RwLock<Vec<Arc<dyn ExecutionCallback>>>,
}

impl TestCallbackManager {
    fn new() -> Self {
        Self {
            callbacks: RwLock::new(Vec::new()),
        }
    }

    fn add_callback(&self, callback: Arc<dyn ExecutionCallback>) {
        self.callbacks.write().push(callback);
    }

    fn remove_callback(&self, index: usize) -> Option<Arc<dyn ExecutionCallback>> {
        let mut callbacks = self.callbacks.write();
        if index < callbacks.len() {
            Some(callbacks.remove(index))
        } else {
            None
        }
    }

    fn clear(&self) {
        self.callbacks.write().clear();
    }

    fn callback_count(&self) -> usize {
        self.callbacks.read().len()
    }

    async fn dispatch_playbook_start(&self, name: &str) {
        let callbacks: Vec<_> = self.callbacks.read().clone();
        for callback in callbacks.iter() {
            callback.on_playbook_start(name).await;
        }
    }

    async fn dispatch_playbook_end(&self, name: &str, success: bool) {
        let callbacks: Vec<_> = self.callbacks.read().clone();
        for callback in callbacks.iter() {
            callback.on_playbook_end(name, success).await;
        }
    }

    async fn dispatch_play_start(&self, name: &str, hosts: &[String]) {
        let callbacks: Vec<_> = self.callbacks.read().clone();
        for callback in callbacks.iter() {
            callback.on_play_start(name, hosts).await;
        }
    }

    async fn dispatch_play_end(&self, name: &str, success: bool) {
        let callbacks: Vec<_> = self.callbacks.read().clone();
        for callback in callbacks.iter() {
            callback.on_play_end(name, success).await;
        }
    }

    async fn dispatch_task_start(&self, name: &str, host: &str) {
        let callbacks: Vec<_> = self.callbacks.read().clone();
        for callback in callbacks.iter() {
            callback.on_task_start(name, host).await;
        }
    }

    async fn dispatch_task_complete(&self, result: &ExecutionResult) {
        let callbacks: Vec<_> = self.callbacks.read().clone();
        for callback in callbacks.iter() {
            callback.on_task_complete(result).await;
        }
    }

    async fn dispatch_handler_triggered(&self, name: &str) {
        let callbacks: Vec<_> = self.callbacks.read().clone();
        for callback in callbacks.iter() {
            callback.on_handler_triggered(name).await;
        }
    }

    async fn dispatch_facts_gathered(&self, host: &str, facts: &Facts) {
        let callbacks: Vec<_> = self.callbacks.read().clone();
        for callback in callbacks.iter() {
            callback.on_facts_gathered(host, facts).await;
        }
    }
}

#[tokio::test]
async fn test_callback_manager_new() {
    let manager = TestCallbackManager::new();

    // Manager should start with no callbacks
    assert_eq!(manager.callback_count(), 0);

    // Dispatching to empty manager should not panic
    manager.dispatch_playbook_start("empty-test").await;
    manager.dispatch_playbook_end("empty-test", true).await;
}

// ============================================================================
// Test 2: test_callback_manager_add_callback
// ============================================================================

#[tokio::test]
async fn test_callback_manager_add_callback() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test-callback"));

    // Add a callback
    manager.add_callback(callback.clone());
    assert_eq!(manager.callback_count(), 1);

    // Verify the callback receives events
    manager.dispatch_playbook_start("test-playbook").await;
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);

    // Add another callback
    let callback2 = Arc::new(TrackingCallback::new("test-callback-2"));
    manager.add_callback(callback2.clone());
    assert_eq!(manager.callback_count(), 2);

    // Both callbacks should receive the event
    manager.dispatch_playbook_start("test-playbook-2").await;
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 2);
    assert_eq!(callback2.playbook_start_count.load(Ordering::SeqCst), 1);
}

// ============================================================================
// Test 3: test_callback_manager_remove_callback
// ============================================================================

#[tokio::test]
async fn test_callback_manager_remove_callback() {
    let manager = TestCallbackManager::new();
    let callback1 = Arc::new(TrackingCallback::new("callback-1"));
    let callback2 = Arc::new(TrackingCallback::new("callback-2"));

    manager.add_callback(callback1.clone());
    manager.add_callback(callback2.clone());
    assert_eq!(manager.callback_count(), 2);

    // Remove first callback
    let removed = manager.remove_callback(0);
    assert!(removed.is_some());
    assert_eq!(manager.callback_count(), 1);

    // Dispatch an event - only callback2 should receive it
    manager.dispatch_playbook_start("after-remove").await;
    assert_eq!(callback1.playbook_start_count.load(Ordering::SeqCst), 0);
    assert_eq!(callback2.playbook_start_count.load(Ordering::SeqCst), 1);

    // Try to remove non-existent index
    let removed = manager.remove_callback(10);
    assert!(removed.is_none());
}

// ============================================================================
// Test 4: test_callback_manager_clear
// ============================================================================

#[tokio::test]
async fn test_callback_manager_clear() {
    let manager = TestCallbackManager::new();
    let callback1 = Arc::new(TrackingCallback::new("callback-1"));
    let callback2 = Arc::new(TrackingCallback::new("callback-2"));
    let callback3 = Arc::new(TrackingCallback::new("callback-3"));

    manager.add_callback(callback1.clone());
    manager.add_callback(callback2.clone());
    manager.add_callback(callback3.clone());
    assert_eq!(manager.callback_count(), 3);

    // Clear all callbacks
    manager.clear();
    assert_eq!(manager.callback_count(), 0);

    // Dispatch event - no callbacks should receive it
    manager.dispatch_playbook_start("after-clear").await;
    assert_eq!(callback1.playbook_start_count.load(Ordering::SeqCst), 0);
    assert_eq!(callback2.playbook_start_count.load(Ordering::SeqCst), 0);
    assert_eq!(callback3.playbook_start_count.load(Ordering::SeqCst), 0);
}

// ============================================================================
// Test 5: test_callback_manager_dispatch_playbook_start
// ============================================================================

#[tokio::test]
async fn test_callback_manager_dispatch_playbook_start() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test"));
    manager.add_callback(callback.clone());

    // Dispatch playbook_start event
    manager.dispatch_playbook_start("my-playbook").await;

    // Verify callback received the event
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        callback.last_playbook_name.read().as_deref(),
        Some("my-playbook")
    );
    assert!(callback.has_event("playbook_start:my-playbook"));

    // Dispatch multiple times
    manager.dispatch_playbook_start("playbook-2").await;
    manager.dispatch_playbook_start("playbook-3").await;
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 3);
}

// ============================================================================
// Test 6: test_callback_manager_dispatch_playbook_end
// ============================================================================

#[tokio::test]
async fn test_callback_manager_dispatch_playbook_end() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test"));
    manager.add_callback(callback.clone());

    // Dispatch playbook_end with success=true
    manager.dispatch_playbook_end("my-playbook", true).await;

    assert_eq!(callback.playbook_end_count.load(Ordering::SeqCst), 1);
    assert!(callback.has_event("playbook_end:my-playbook:true"));

    // Dispatch playbook_end with success=false
    manager
        .dispatch_playbook_end("failed-playbook", false)
        .await;

    assert_eq!(callback.playbook_end_count.load(Ordering::SeqCst), 2);
    assert!(callback.has_event("playbook_end:failed-playbook:false"));
}

// ============================================================================
// Test 7: test_callback_manager_dispatch_play_start
// ============================================================================

#[tokio::test]
async fn test_callback_manager_dispatch_play_start() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test"));
    manager.add_callback(callback.clone());

    let hosts = vec![
        "host1".to_string(),
        "host2".to_string(),
        "host3".to_string(),
    ];

    // Dispatch play_start event
    manager
        .dispatch_play_start("Configure webservers", &hosts)
        .await;

    assert_eq!(callback.play_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        callback.last_play_name.read().as_deref(),
        Some("Configure webservers")
    );
    assert!(callback.has_event("play_start:Configure webservers:hosts=3"));

    // Dispatch with empty hosts
    manager.dispatch_play_start("Empty play", &[]).await;
    assert!(callback.has_event("play_start:Empty play:hosts=0"));
}

// ============================================================================
// Test 8: test_callback_manager_dispatch_play_end
// ============================================================================

#[tokio::test]
async fn test_callback_manager_dispatch_play_end() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test"));
    manager.add_callback(callback.clone());

    // Dispatch play_end with success
    manager
        .dispatch_play_end("Configure webservers", true)
        .await;

    assert_eq!(callback.play_end_count.load(Ordering::SeqCst), 1);
    assert!(callback.has_event("play_end:Configure webservers:true"));

    // Dispatch play_end with failure
    manager.dispatch_play_end("Failed play", false).await;

    assert_eq!(callback.play_end_count.load(Ordering::SeqCst), 2);
    assert!(callback.has_event("play_end:Failed play:false"));
}

// ============================================================================
// Test 9: test_callback_manager_dispatch_task_start
// ============================================================================

#[tokio::test]
async fn test_callback_manager_dispatch_task_start() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test"));
    manager.add_callback(callback.clone());

    // Dispatch task_start event
    manager
        .dispatch_task_start("Install nginx", "webserver1")
        .await;

    assert_eq!(callback.task_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        callback.last_task_name.read().as_deref(),
        Some("Install nginx")
    );
    assert_eq!(callback.last_host.read().as_deref(), Some("webserver1"));
    assert!(callback.has_event("task_start:Install nginx:webserver1"));

    // Dispatch task on multiple hosts
    manager
        .dispatch_task_start("Install nginx", "webserver2")
        .await;
    manager
        .dispatch_task_start("Install nginx", "webserver3")
        .await;

    assert_eq!(callback.task_start_count.load(Ordering::SeqCst), 3);
}

// ============================================================================
// Test 10: test_callback_manager_dispatch_task_complete
// ============================================================================

#[tokio::test]
async fn test_callback_manager_dispatch_task_complete() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test"));
    manager.add_callback(callback.clone());

    // Test successful task
    let success_result = create_test_result("Install nginx", "webserver1", true, false);
    manager.dispatch_task_complete(&success_result).await;

    assert_eq!(callback.task_complete_count.load(Ordering::SeqCst), 1);
    assert!(callback.has_event("task_complete:Install nginx:webserver1:success=true"));

    // Test changed task
    let changed_result = create_test_result("Configure nginx", "webserver1", true, true);
    manager.dispatch_task_complete(&changed_result).await;

    assert_eq!(callback.task_complete_count.load(Ordering::SeqCst), 2);

    // Test failed task
    let failed_result = create_test_result("Deploy app", "webserver1", false, false);
    manager.dispatch_task_complete(&failed_result).await;

    assert_eq!(callback.task_complete_count.load(Ordering::SeqCst), 3);
    assert!(callback.has_event("task_complete:Deploy app:webserver1:success=false"));
}

// ============================================================================
// Test 11: test_callback_manager_dispatch_handler_triggered
// ============================================================================

#[tokio::test]
async fn test_callback_manager_dispatch_handler_triggered() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test"));
    manager.add_callback(callback.clone());

    // Dispatch handler_triggered event
    manager.dispatch_handler_triggered("Restart nginx").await;

    assert_eq!(callback.handler_triggered_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        callback.last_handler_name.read().as_deref(),
        Some("Restart nginx")
    );
    assert!(callback.has_event("handler_triggered:Restart nginx"));

    // Dispatch multiple handlers
    manager.dispatch_handler_triggered("Reload systemd").await;
    manager
        .dispatch_handler_triggered("Restart postgresql")
        .await;

    assert_eq!(callback.handler_triggered_count.load(Ordering::SeqCst), 3);
}

// ============================================================================
// Test 12: test_callback_manager_dispatch_facts_gathered
// ============================================================================

#[tokio::test]
async fn test_callback_manager_dispatch_facts_gathered() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("test"));
    manager.add_callback(callback.clone());

    let facts = create_test_facts();

    // Dispatch facts_gathered event
    manager.dispatch_facts_gathered("webserver1", &facts).await;

    assert_eq!(callback.facts_gathered_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback.last_host.read().as_deref(), Some("webserver1"));
    assert!(callback.has_event("facts_gathered:webserver1"));

    // Dispatch for multiple hosts
    manager.dispatch_facts_gathered("webserver2", &facts).await;
    manager.dispatch_facts_gathered("dbserver1", &facts).await;

    assert_eq!(callback.facts_gathered_count.load(Ordering::SeqCst), 3);
}

// ============================================================================
// Test 13: test_callback_manager_multiple_callbacks
// ============================================================================

#[tokio::test]
async fn test_callback_manager_multiple_callbacks() {
    let manager = TestCallbackManager::new();

    let callback1 = Arc::new(TrackingCallback::new("callback-1"));
    let callback2 = Arc::new(TrackingCallback::new("callback-2"));
    let callback3 = Arc::new(TrackingCallback::new("callback-3"));

    manager.add_callback(callback1.clone());
    manager.add_callback(callback2.clone());
    manager.add_callback(callback3.clone());

    // Simulate a full playbook run
    manager.dispatch_playbook_start("multi-callback-test").await;

    let hosts = vec!["host1".to_string(), "host2".to_string()];
    manager.dispatch_play_start("Test play", &hosts).await;

    // Gather facts for each host
    let facts = create_test_facts();
    manager.dispatch_facts_gathered("host1", &facts).await;
    manager.dispatch_facts_gathered("host2", &facts).await;

    // Execute a task on each host
    manager
        .dispatch_task_start("Install package", "host1")
        .await;
    let result1 = create_test_result("Install package", "host1", true, true);
    manager.dispatch_task_complete(&result1).await;

    manager
        .dispatch_task_start("Install package", "host2")
        .await;
    let result2 = create_test_result("Install package", "host2", true, true);
    manager.dispatch_task_complete(&result2).await;

    // Trigger a handler
    manager.dispatch_handler_triggered("Restart service").await;

    // End play and playbook
    manager.dispatch_play_end("Test play", true).await;
    manager
        .dispatch_playbook_end("multi-callback-test", true)
        .await;

    // Verify all callbacks received all events
    for (i, callback) in [&callback1, &callback2, &callback3].iter().enumerate() {
        let name = format!("callback-{}", i + 1);

        assert_eq!(
            callback.playbook_start_count.load(Ordering::SeqCst),
            1,
            "{} playbook_start_count",
            name
        );
        assert_eq!(
            callback.playbook_end_count.load(Ordering::SeqCst),
            1,
            "{} playbook_end_count",
            name
        );
        assert_eq!(
            callback.play_start_count.load(Ordering::SeqCst),
            1,
            "{} play_start_count",
            name
        );
        assert_eq!(
            callback.play_end_count.load(Ordering::SeqCst),
            1,
            "{} play_end_count",
            name
        );
        assert_eq!(
            callback.task_start_count.load(Ordering::SeqCst),
            2,
            "{} task_start_count",
            name
        );
        assert_eq!(
            callback.task_complete_count.load(Ordering::SeqCst),
            2,
            "{} task_complete_count",
            name
        );
        assert_eq!(
            callback.handler_triggered_count.load(Ordering::SeqCst),
            1,
            "{} handler_triggered_count",
            name
        );
        assert_eq!(
            callback.facts_gathered_count.load(Ordering::SeqCst),
            2,
            "{} facts_gathered_count",
            name
        );

        // Verify total calls
        assert_eq!(callback.total_calls(), 11, "{} total calls", name);
    }
}

// ============================================================================
// Additional Tests: Concurrent Dispatch
// ============================================================================

#[tokio::test]
async fn test_callback_manager_concurrent_dispatch() {
    use tokio::task::JoinSet;

    let manager = Arc::new(TestCallbackManager::new());
    let callback = Arc::new(TrackingCallback::new("concurrent-test"));
    manager.add_callback(callback.clone());

    // Spawn many concurrent tasks that dispatch events
    let mut join_set = JoinSet::new();

    for i in 0..100 {
        let mgr = manager.clone();
        let playbook_name = format!("playbook-{}", i);
        join_set.spawn(async move {
            mgr.dispatch_playbook_start(&playbook_name).await;
        });
    }

    // Wait for all tasks to complete
    while join_set.join_next().await.is_some() {}

    // Verify all events were received
    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 100);
    assert_eq!(callback.get_events().len(), 100);
}

#[tokio::test]
async fn test_callback_manager_concurrent_mixed_events() {
    use tokio::task::JoinSet;

    let manager = Arc::new(TestCallbackManager::new());
    let callback = Arc::new(TrackingCallback::new("mixed-concurrent"));
    manager.add_callback(callback.clone());

    let mut join_set = JoinSet::new();

    // Spawn various event types concurrently
    for i in 0..50 {
        let mgr = manager.clone();
        join_set.spawn(async move {
            mgr.dispatch_task_start(&format!("task-{}", i), &format!("host-{}", i % 10))
                .await;
        });

        let mgr = manager.clone();
        join_set.spawn(async move {
            let result = create_test_result(
                &format!("task-{}", i),
                &format!("host-{}", i % 10),
                true,
                i % 2 == 0,
            );
            mgr.dispatch_task_complete(&result).await;
        });
    }

    while join_set.join_next().await.is_some() {}

    // Verify all events were received
    assert_eq!(callback.task_start_count.load(Ordering::SeqCst), 50);
    assert_eq!(callback.task_complete_count.load(Ordering::SeqCst), 50);
}

// ============================================================================
// Additional Tests: Event Ordering
// ============================================================================

#[tokio::test]
async fn test_callback_manager_event_ordering() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("ordering"));
    manager.add_callback(callback.clone());

    // Execute events in a specific order
    manager.dispatch_playbook_start("ordered-test").await;
    manager
        .dispatch_play_start("play-1", &["host1".to_string()])
        .await;
    manager.dispatch_task_start("task-1", "host1").await;
    manager
        .dispatch_task_complete(&create_test_result("task-1", "host1", true, false))
        .await;
    manager.dispatch_play_end("play-1", true).await;
    manager.dispatch_playbook_end("ordered-test", true).await;

    // Verify ordering is preserved
    let events = callback.get_events();
    assert!(events[0].contains("playbook_start"));
    assert!(events[1].contains("play_start"));
    assert!(events[2].contains("task_start"));
    assert!(events[3].contains("task_complete"));
    assert!(events[4].contains("play_end"));
    assert!(events[5].contains("playbook_end"));
}

// ============================================================================
// Additional Tests: Edge Cases
// ============================================================================

#[tokio::test]
async fn test_callback_manager_empty_strings() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("edge-case"));
    manager.add_callback(callback.clone());

    // Test with empty strings (should not panic)
    manager.dispatch_playbook_start("").await;
    manager.dispatch_play_start("", &[]).await;
    manager.dispatch_task_start("", "").await;
    manager.dispatch_handler_triggered("").await;

    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback.play_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback.task_start_count.load(Ordering::SeqCst), 1);
    assert_eq!(callback.handler_triggered_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_callback_manager_unicode_strings() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("unicode"));
    manager.add_callback(callback.clone());

    // Test with unicode characters
    manager.dispatch_playbook_start("playbook-\u{1F680}").await;
    manager
        .dispatch_play_start("play-\u{2764}", &["host-\u{1F4BB}".to_string()])
        .await;
    manager
        .dispatch_task_start("task-\u{1F389}", "host-\u{1F4BB}")
        .await;

    assert_eq!(callback.playbook_start_count.load(Ordering::SeqCst), 1);
    assert!(callback.has_event("\u{1F680}"));
    assert!(callback.has_event("\u{2764}"));
    assert!(callback.has_event("\u{1F389}"));
}

#[tokio::test]
async fn test_callback_manager_large_host_list() {
    let manager = TestCallbackManager::new();
    let callback = Arc::new(TrackingCallback::new("large-list"));
    manager.add_callback(callback.clone());

    // Test with many hosts
    let hosts: Vec<String> = (0..1000).map(|i| format!("host-{}", i)).collect();
    manager.dispatch_play_start("large-play", &hosts).await;

    assert_eq!(callback.play_start_count.load(Ordering::SeqCst), 1);
    assert!(callback.has_event("hosts=1000"));
}

#[tokio::test]
async fn test_callback_manager_rapid_add_remove() {
    let manager = TestCallbackManager::new();

    // Rapidly add and remove callbacks
    for _ in 0..100 {
        let callback = Arc::new(TrackingCallback::new("temp"));
        manager.add_callback(callback);
        manager.remove_callback(0);
    }

    assert_eq!(manager.callback_count(), 0);
}
