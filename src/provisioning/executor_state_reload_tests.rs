//! Public apply-path regressions with memory-only state, locks and resources.

use super::*;
use crate::provisioning::state_lock::{InMemoryLock, LockBackend};
use crate::provisioning::traits::*;
use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
struct MemoryBackend {
    saved: Mutex<Option<ProvisioningState>>,
    lock: Arc<InMemoryLock>,
    events: Arc<Mutex<Vec<String>>>,
    fail_load: AtomicBool,
}

#[async_trait]
impl StateBackend for MemoryBackend {
    fn name(&self) -> &str {
        "fixture"
    }
    async fn load_raw(&self) -> ProvisioningResult<Option<String>> {
        panic!("unused fixture path")
    }
    async fn load(&self) -> ProvisioningResult<Option<ProvisioningState>> {
        let locked = self.lock.get_lock().await?.is_some();
        self.events.lock().push(if locked {
            "load_locked".into()
        } else {
            "load_unlocked".into()
        });
        if self.fail_load.load(Ordering::SeqCst) {
            return Err(ProvisioningError::StatePersistenceError(
                "fixture load failure".into(),
            ));
        }
        Ok(self.saved.lock().clone())
    }
    async fn save(&self, state: &ProvisioningState) -> ProvisioningResult<()> {
        assert!(
            self.lock.get_lock().await?.is_some(),
            "save must remain locked"
        );
        self.events.lock().push("save_locked".into());
        *self.saved.lock() = Some(state.clone());
        Ok(())
    }
    async fn delete(&self) -> ProvisioningResult<()> {
        panic!("unused fixture path")
    }
    async fn exists(&self) -> ProvisioningResult<bool> {
        Ok(self.saved.lock().is_some())
    }
    fn lock_backend(&self) -> Option<Arc<dyn LockBackend>> {
        Some(self.lock.clone())
    }
}

#[derive(Debug)]
struct FixtureResource {
    events: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Resource for FixtureResource {
    fn resource_type(&self) -> &str {
        "fixture_item"
    }
    fn provider(&self) -> &str {
        "fixture"
    }
    fn schema(&self) -> ResourceSchema {
        panic!("unused fixture path")
    }
    async fn read(&self, _: &str, _: &ProviderContext) -> ProvisioningResult<ResourceReadResult> {
        panic!("unused fixture path")
    }
    async fn plan(
        &self,
        _: &Value,
        _: Option<&Value>,
        _: &ProviderContext,
    ) -> ProvisioningResult<ResourceDiff> {
        panic!("unused fixture path")
    }
    async fn create(
        &self,
        config: &Value,
        _: &ProviderContext,
    ) -> ProvisioningResult<ResourceResult> {
        let id = config["id"].as_str().expect("fixture id");
        self.events.lock().push(format!("create:{id}"));
        Ok(ResourceResult::success(id, config.clone()))
    }
    async fn update(
        &self,
        id: &str,
        _: &Value,
        config: &Value,
        _: &ProviderContext,
    ) -> ProvisioningResult<ResourceResult> {
        self.events.lock().push(format!("update:{id}"));
        Ok(ResourceResult::success(id, config.clone()))
    }
    async fn destroy(&self, id: &str, _: &ProviderContext) -> ProvisioningResult<ResourceResult> {
        self.events.lock().push(format!("destroy:{id}"));
        Ok(ResourceResult::success(id, json!({})))
    }
    async fn import(&self, _: &str, _: &ProviderContext) -> ProvisioningResult<ResourceResult> {
        panic!("unused fixture path")
    }
    fn dependencies(&self, _: &Value) -> Vec<ResourceDependency> {
        Vec::new()
    }
    fn forces_replacement(&self) -> Vec<String> {
        Vec::new()
    }
    fn validate(&self, _: &Value) -> ProvisioningResult<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct FixtureProvider {
    resource: Arc<dyn Resource>,
}

#[async_trait]
impl Provider for FixtureProvider {
    fn name(&self) -> &str {
        "fixture"
    }
    fn version(&self) -> &str {
        "0"
    }
    fn config_schema(&self) -> ProviderSchema {
        panic!("unused fixture path")
    }
    async fn configure(&mut self, _: ProviderConfig) -> ProvisioningResult<()> {
        Ok(())
    }
    fn resource(&self, _: &str) -> ProvisioningResult<Arc<dyn Resource>> {
        Ok(self.resource.clone())
    }
    fn data_source(&self, _: &str) -> ProvisioningResult<Arc<dyn DataSource>> {
        panic!("unused fixture path")
    }
    fn resource_types(&self) -> Vec<String> {
        vec!["fixture_item".into()]
    }
    fn data_source_types(&self) -> Vec<String> {
        Vec::new()
    }
    fn validate_config(&self, _: &Value) -> ProvisioningResult<()> {
        Ok(())
    }
    fn context(&self) -> ProvisioningResult<ProviderContext> {
        Ok(ProviderContext {
            provider: "fixture".into(),
            region: None,
            config: json!({}),
            credentials: Arc::new(DebugCredentials::new("fixture")),
            timeout_seconds: 1,
            retry_config: RetryConfig::default(),
            default_tags: HashMap::new(),
        })
    }
}

// Inject only memory fixtures; the production apply, save, resolver and lock
// manager implementations run unchanged. No built-in provider is constructed.
async fn executor(backend: Arc<MemoryBackend>, resources: Value) -> ProvisioningExecutor {
    let resource = Arc::new(FixtureResource {
        events: backend.events.clone(),
    });
    executor_with(backend, resources, resource).await
}

async fn executor_with(
    backend: Arc<dyn StateBackend>,
    resources: Value,
    resource: Arc<dyn Resource>,
) -> ProvisioningExecutor {
    let mut config = InfrastructureConfig::new();
    config.resources = serde_json::from_value(json!({"fixture_item": resources})).unwrap();
    let loaded = backend.load().await.unwrap();
    let state_exists = loaded.is_some();
    let cached = loaded.unwrap_or_default();
    let mut providers = ProviderRegistry::new();
    providers.register_factory("fixture", move || {
        Box::new(FixtureProvider {
            resource: resource.clone(),
        })
    });
    providers
        .initialize_provider(ProviderConfig {
            name: "fixture".into(),
            region: None,
            settings: json!({}),
        })
        .await
        .unwrap();
    let providers = Arc::new(providers);
    let context = ResolverContext::from_config_and_state(&config, &cached);
    ProvisioningExecutor {
        config,
        executor_config: ExecutorConfig {
            backup_state: false,
            refresh_before_plan: false,
            ..Default::default()
        },
        provider_registry: providers.clone(),
        resource_registry: Arc::new(ResourceRegistry::new(providers)),
        state: RwLock::new(cached),
        state_exists: AtomicBool::new(state_exists),
        plan_binding: RwLock::new(None),
        state_backend: backend.clone(),
        lock_manager: backend
            .lock_backend()
            .map(|lock| Arc::new(StateLockManager::from_arc(lock))),
        semaphore: Arc::new(Semaphore::new(1)),
        resolver: TemplateResolver::new(),
        resolver_context: RwLock::new(context),
    }
}

#[path = "executor_progress_tests.rs"]
mod progress_tests;

fn state_with(name: &str, cloud_id: &str) -> ProvisioningState {
    let mut state = ProvisioningState::new();
    state.add_resource(ResourceState::new(
        ResourceId::new("fixture_item", name),
        cloud_id,
        "fixture",
        json!({"id": cloud_id}),
        json!({}),
    ));
    state
}

fn create_plan(name: &str) -> ExecutionPlan {
    let mut plan = ExecutionPlan::empty();
    plan.actions.push(PlannedAction::create(
        ResourceId::new("fixture_item", name),
        "fixture",
        ResourceDiff::create(json!({})),
    ));
    plan
}

fn destroy_plan(name: &str) -> ExecutionPlan {
    let mut plan = ExecutionPlan::empty();
    plan.is_destroy = true;
    plan.actions.push(PlannedAction::destroy(
        ResourceId::new("fixture_item", name),
        "fixture",
    ));
    plan
}

#[tokio::test]
async fn serialized_applies_preserve_the_preceding_writer() {
    let backend = Arc::new(MemoryBackend::default());
    *backend.saved.lock() = Some(ProvisioningState::new());
    let first = executor(backend.clone(), json!({"first": {"id": "created-first"}})).await;
    let second = executor(
        backend.clone(),
        json!({"first": {"id": "created-first"}, "second": {"id": "created-second"}}),
    )
    .await;
    let lineage = first.state().lineage;
    let first_plan = first.plan().await.unwrap();
    let stale_plan = second.plan().await.unwrap();
    assert!(first.apply(&first_plan).await.unwrap().success);
    backend.events.lock().clear();
    for _ in 0..2 {
        assert!(second.apply(&stale_plan).await.is_err());
    }
    assert_eq!(*backend.events.lock(), ["load_locked", "load_locked"]);
    assert_eq!(backend.saved.lock().as_ref().unwrap().resources.len(), 1);
    let fresh_plan = second.plan().await.unwrap();
    backend.events.lock().clear();
    assert!(second.apply(&fresh_plan).await.unwrap().success);
    let saved = backend.saved.lock().clone().unwrap();
    assert_eq!(
        saved.resources.len(),
        2,
        "the second writer must retain the first resource"
    );
    assert_eq!(saved.lineage, lineage);
    assert_eq!(saved.serial, 2);
    assert_eq!(
        *backend.events.lock(),
        ["load_locked", "create:created-second", "save_locked"]
    );
    assert!(backend.lock.get_lock().await.unwrap().is_none());
}

#[tokio::test]
async fn apply_rebuilds_resolver_context_from_reloaded_state() {
    let backend = Arc::new(MemoryBackend::default());
    let executor = executor(
        backend.clone(),
        json!({"first": {"id": "fresh-reference"}, "second": {"id": "{{ resources.fixture_item.first.id }}"}}),
    )
    .await;
    let stale_plan = executor.plan().await.unwrap();
    *backend.saved.lock() = Some(state_with("first", "fresh-reference"));
    backend.events.lock().clear();
    assert!(executor.apply(&stale_plan).await.is_err());
    assert_eq!(*backend.events.lock(), ["load_locked"]);
    let fresh_plan = executor.plan().await.unwrap();
    let result = executor.apply(&fresh_plan).await.unwrap();
    assert!(result.success, "{:?}", result.errors);
    assert!(backend
        .events
        .lock()
        .contains(&"create:fresh-reference".into()));
    assert_eq!(backend.saved.lock().as_ref().unwrap().resources.len(), 2);
}

#[tokio::test]
async fn stale_destroy_rejects_replacement_identity_until_replanned() {
    let backend = Arc::new(MemoryBackend::default());
    *backend.saved.lock() = Some(state_with("target", "old-target"));
    let mut executor = executor(backend.clone(), json!({})).await;
    executor.executor_config.targets = vec!["fixture_item.target".into()];
    let stale_plan = executor.plan_destroy().await.unwrap();
    let mut fresh = executor.state();
    // Preserve serial and lineage: a version-only comparison misses this change.
    fresh
        .get_resource_mut(&ResourceId::new("fixture_item", "target"))
        .unwrap()
        .cloud_id = "fresh-target".into();
    fresh.add_resource(
        state_with("other", "preserved")
            .resources
            .into_values()
            .next()
            .unwrap(),
    );
    *backend.saved.lock() = Some(fresh);
    backend.events.lock().clear();
    for _ in 0..2 {
        assert!(executor.apply(&stale_plan).await.is_err());
    }
    assert_eq!(*backend.events.lock(), ["load_locked", "load_locked"]);
    assert_eq!(backend.saved.lock().as_ref().unwrap().resources.len(), 2);
    let fresh_plan = executor.plan_destroy().await.unwrap();
    backend.events.lock().clear();
    assert!(executor.apply(&fresh_plan).await.unwrap().success);
    assert_eq!(
        *backend.events.lock(),
        ["load_locked", "destroy:fresh-target", "save_locked"]
    );
    let saved = backend.saved.lock();
    assert!(saved
        .as_ref()
        .unwrap()
        .get_resource(&ResourceId::new("fixture_item", "target"))
        .is_none());
    assert!(saved
        .as_ref()
        .unwrap()
        .get_resource(&ResourceId::new("fixture_item", "other"))
        .is_some());
}

#[tokio::test]
async fn reload_failure_prevents_actions_and_releases_the_lock() {
    for plan in [
        create_plan("target"),
        destroy_plan("target"),
        ExecutionPlan::empty(),
    ] {
        let backend = Arc::new(MemoryBackend::default());
        *backend.saved.lock() = Some(state_with("target", "fixture-target"));
        let executor = executor(backend.clone(), json!({"target": {"id": "created-target"}})).await;
        backend.fail_load.store(true, Ordering::SeqCst);
        backend.events.lock().clear();
        let result = executor.apply(&plan).await;
        assert!(
            matches!(result, Err(ProvisioningError::StatePersistenceError(_))),
            "{result:?}"
        );
        assert_eq!(*backend.events.lock(), ["load_locked"]);
        assert!(backend.lock.get_lock().await.unwrap().is_none());
    }
}

#[tokio::test]
async fn no_change_apply_adopts_current_state_without_saving() {
    let backend = Arc::new(MemoryBackend::default());
    let executor = executor(backend.clone(), json!({})).await;
    let fresh = state_with("other", "fresh-other");
    let lineage = fresh.lineage.clone();
    *backend.saved.lock() = Some(fresh);
    backend.events.lock().clear();
    assert!(
        executor
            .apply(&ExecutionPlan::empty())
            .await
            .unwrap()
            .success
    );
    assert_eq!(executor.state().lineage, lineage);
    assert_eq!(executor.state().resources.len(), 1);
    assert_eq!(*backend.events.lock(), ["load_locked"]);
}

#[tokio::test]
async fn missing_backend_state_does_not_reuse_cached_resources() {
    let backend = Arc::new(MemoryBackend::default());
    *backend.saved.lock() = Some(state_with("removed", "removed-id"));
    let executor = executor(backend.clone(), json!({})).await;
    *backend.saved.lock() = None;
    backend.events.lock().clear();
    assert!(
        executor
            .apply(&ExecutionPlan::empty())
            .await
            .unwrap()
            .success
    );
    assert!(executor.state().resources.is_empty());
    assert_eq!(*backend.events.lock(), ["load_locked"]);
}

#[tokio::test]
async fn unchanged_backend_allows_a_normal_create() {
    let backend = Arc::new(MemoryBackend::default());
    let executor = executor(backend.clone(), json!({"target": {"id": "created-target"}})).await;
    let plan = executor.plan().await.unwrap();
    assert!(executor.apply(&plan).await.unwrap().success);
    assert_eq!(backend.saved.lock().as_ref().unwrap().resources.len(), 1);
    assert!(backend.lock.get_lock().await.unwrap().is_none());
}

#[tokio::test]
async fn manual_and_cross_executor_plans_reject_before_actions() {
    let backend = Arc::new(MemoryBackend::default());
    let first = executor(backend.clone(), json!({"target": {"id": "created-target"}})).await;
    let second = executor(backend.clone(), json!({"target": {"id": "created-target"}})).await;
    let foreign_plan = first.plan().await.unwrap();
    second.plan().await.unwrap();
    backend.events.lock().clear();
    assert!(second.apply(&foreign_plan).await.is_err());
    assert!(second.apply(&create_plan("target")).await.is_err());
    assert_eq!(*backend.events.lock(), ["load_locked", "load_locked"]);
    assert!(backend.saved.lock().is_none());
}

#[tokio::test]
async fn generating_another_plan_invalidates_the_previous_plan() {
    let backend = Arc::new(MemoryBackend::default());
    let executor = executor(backend.clone(), json!({"target": {"id": "created-target"}})).await;
    let previous = executor.plan().await.unwrap();
    let current = executor.plan().await.unwrap();
    backend.events.lock().clear();
    assert!(executor.apply(&previous).await.is_err());
    assert_eq!(*backend.events.lock(), ["load_locked"]);
    assert!(executor.apply(&current).await.unwrap().success);
}

mod dependency_tests {
    use super::*;

    #[tokio::test]
    async fn successful_creates_persist_forward_dependencies_for_later_destroy() {
        let backend = Arc::new(MemoryBackend::default());
        let executor = executor(
            backend.clone(),
            json!({
                "base": {"id": "created-base"},
                "middle": {"id": "created-middle", "depends_on": ["fixture_item.base"]},
                "leaf": {"id": "created-leaf", "depends_on": ["fixture_item.middle"]}
            }),
        )
        .await;
        let plan = executor.plan().await.unwrap();
        assert!(executor.apply(&plan).await.unwrap().success);
        let saved = backend.saved.lock().clone().unwrap();
        // Exercise the production state serializer/reader as well as the mock backend.
        let restored =
            ProvisioningState::from_json_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(
            restored
                .get_resource(&ResourceId::new("fixture_item", "middle"))
                .unwrap()
                .dependencies,
            [ResourceId::new("fixture_item", "base")]
        );
        assert_eq!(
            restored
                .get_resource(&ResourceId::new("fixture_item", "leaf"))
                .unwrap()
                .dependencies,
            [ResourceId::new("fixture_item", "middle")]
        );
        *backend.saved.lock() = Some(restored);
        let destroyer = super::executor(backend.clone(), json!({})).await;
        let plan = destroyer.plan_destroy().await.unwrap();
        backend.events.lock().clear();
        assert!(destroyer.apply(&plan).await.unwrap().success);
        let destroys: Vec<_> = backend
            .events
            .lock()
            .iter()
            .filter(|event| event.starts_with("destroy:"))
            .cloned()
            .collect();
        assert_eq!(
            destroys,
            [
                "destroy:created-leaf",
                "destroy:created-middle",
                "destroy:created-base"
            ]
        );
        assert!(backend.saved.lock().as_ref().unwrap().resources.is_empty());
    }

    #[tokio::test]
    async fn successful_update_replaces_stale_forward_dependencies() {
        let backend = Arc::new(MemoryBackend::default());
        let mut saved = state_with("first", "first");
        saved.add_resource(
            state_with("second", "second")
                .resources
                .into_values()
                .next()
                .unwrap(),
        );
        let mut leaf = state_with("leaf", "leaf")
            .resources
            .into_values()
            .next()
            .unwrap();
        leaf.config = json!({"id": "leaf", "depends_on": ["fixture_item.first"]});
        leaf.dependencies = vec![ResourceId::new("fixture_item", "first")];
        saved.add_resource(leaf);
        *backend.saved.lock() = Some(saved);
        let executor = executor(
            backend.clone(),
            json!({
                "first": {"id": "first"}, "second": {"id": "second"},
                "leaf": {"id": "leaf", "depends_on": ["fixture_item.second"]}
            }),
        )
        .await;
        let plan = executor.plan().await.unwrap();
        assert_eq!(plan.to_update, [ResourceId::new("fixture_item", "leaf")]);
        assert!(executor.apply(&plan).await.unwrap().success);
        let saved = backend.saved.lock();
        assert_eq!(
            saved
                .as_ref()
                .unwrap()
                .get_resource(&ResourceId::new("fixture_item", "leaf"))
                .unwrap()
                .dependencies,
            [ResourceId::new("fixture_item", "second")]
        );
    }
}
