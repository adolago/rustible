//! Confirmed progress survives dropping a later, pending fake operation.

use super::*;
use std::sync::atomic::AtomicUsize;
use tempfile::TempDir;
use tokio::sync::Notify;

struct ProgressBackend {
    local: LocalBackend,
    lock: Arc<InMemoryLock>,
    fail_save: bool,
    saves: AtomicUsize,
}

impl ProgressBackend {
    fn new(directory: &TempDir, fail_save: bool) -> Self {
        Self {
            local: LocalBackend::new(directory.path().join("state.json")),
            lock: Arc::new(InMemoryLock::new()),
            fail_save,
            saves: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl StateBackend for ProgressBackend {
    fn name(&self) -> &str {
        "progress-fixture"
    }
    async fn load_raw(&self) -> ProvisioningResult<Option<String>> {
        self.local.load_raw().await
    }
    async fn load(&self) -> ProvisioningResult<Option<ProvisioningState>> {
        self.local.load().await
    }
    async fn save(&self, state: &ProvisioningState) -> ProvisioningResult<()> {
        assert!(self.lock.get_lock().await?.is_some());
        self.saves.fetch_add(1, Ordering::SeqCst);
        if self.fail_save {
            return Err(ProvisioningError::StatePersistenceError(
                "fixture save failure".into(),
            ));
        }
        self.local.save(state).await
    }
    async fn delete(&self) -> ProvisioningResult<()> {
        self.local.delete().await
    }
    async fn exists(&self) -> ProvisioningResult<bool> {
        self.local.exists().await
    }
    fn lock_backend(&self) -> Option<Arc<dyn LockBackend>> {
        Some(self.lock.clone())
    }
}

#[derive(Debug)]
struct ProgressResource {
    ordinary: FixtureResource,
    pending_id: Option<&'static str>,
    entered: Notify,
}

impl ProgressResource {
    fn new(pending_id: Option<&'static str>) -> Self {
        Self {
            ordinary: FixtureResource {
                events: Arc::new(Mutex::new(Vec::new())),
            },
            pending_id,
            entered: Notify::new(),
        }
    }

    async fn wait_if_pending(&self, id: &str) {
        if self.pending_id == Some(id) {
            self.entered.notify_one();
            std::future::pending::<()>().await;
        }
    }
}

#[async_trait]
impl Resource for ProgressResource {
    fn resource_type(&self) -> &str {
        self.ordinary.resource_type()
    }
    fn provider(&self) -> &str {
        self.ordinary.provider()
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
        context: &ProviderContext,
    ) -> ProvisioningResult<ResourceResult> {
        self.wait_if_pending(config["id"].as_str().unwrap()).await;
        self.ordinary.create(config, context).await
    }
    async fn update(
        &self,
        id: &str,
        old: &Value,
        config: &Value,
        context: &ProviderContext,
    ) -> ProvisioningResult<ResourceResult> {
        self.wait_if_pending(id).await;
        self.ordinary.update(id, old, config, context).await
    }
    async fn destroy(
        &self,
        id: &str,
        context: &ProviderContext,
    ) -> ProvisioningResult<ResourceResult> {
        self.wait_if_pending(id).await;
        self.ordinary.destroy(id, context).await
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

async fn drop_when_next_operation_starts(
    executor: &ProvisioningExecutor,
    plan: &ExecutionPlan,
    resource: &ProgressResource,
) {
    // The fake pending operation does no blocking IO. Dropping this future is
    // bounded; the fixture's isolated memory lock is discarded after the test.
    let mut apply = Box::pin(executor.apply(plan));
    tokio::select! {
        result = &mut apply => panic!("expected pending fixture operation: {result:?}"),
        _ = resource.entered.notified() => {},
        _ = tokio::time::sleep(Duration::from_secs(3)) => panic!("fixture did not reach next operation"),
    }
    drop(apply);
}

#[tokio::test]
async fn cancelled_later_create_retains_completed_resource_in_local_state() {
    let directory = TempDir::new().unwrap();
    let backend = Arc::new(ProgressBackend::new(&directory, false));
    let resource = Arc::new(ProgressResource::new(Some("later")));
    let executor = executor_with(
        backend.clone(),
        json!({"first": {"id": "first"}, "later": {"id": "later", "depends_on": ["fixture_item.first"]}}),
        resource.clone(),
    ).await;
    let plan = executor.plan().await.unwrap();
    drop_when_next_operation_starts(&executor, &plan, &resource).await;
    let saved = backend
        .local
        .load()
        .await
        .unwrap()
        .expect("completed create must be saved");
    assert_eq!(saved.resources.len(), 1);
    assert_eq!(
        saved
            .get_resource(&ResourceId::new("fixture_item", "first"))
            .unwrap()
            .cloud_id,
        "first"
    );
    assert_eq!(backend.saves.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancelled_later_update_retains_completed_update_in_local_state() {
    let directory = TempDir::new().unwrap();
    let backend = Arc::new(ProgressBackend::new(&directory, false));
    let mut initial = state_with("first", "first");
    initial.add_resource(
        state_with("later", "later")
            .resources
            .into_values()
            .next()
            .unwrap(),
    );
    initial.prepare_for_save();
    backend.local.save(&initial).await.unwrap();
    let resource = Arc::new(ProgressResource::new(Some("later")));
    let executor = executor_with(
        backend.clone(),
        json!({"first": {"id": "first", "value": "new"}, "later": {"id": "later", "value": "new", "depends_on": ["fixture_item.first"]}}),
        resource.clone(),
    ).await;
    let plan = executor.plan().await.unwrap();
    drop_when_next_operation_starts(&executor, &plan, &resource).await;
    let saved = backend.local.load().await.unwrap().unwrap();
    assert_eq!(
        saved
            .get_resource(&ResourceId::new("fixture_item", "first"))
            .unwrap()
            .config["value"],
        "new"
    );
    assert!(saved
        .get_resource(&ResourceId::new("fixture_item", "later"))
        .unwrap()
        .config
        .get("value")
        .is_none());
    assert_eq!(backend.saves.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancelled_later_destroy_retains_completed_removal_in_local_state() {
    let directory = TempDir::new().unwrap();
    let backend = Arc::new(ProgressBackend::new(&directory, false));
    let mut initial = state_with("first", "first");
    initial
        .get_resource_mut(&ResourceId::new("fixture_item", "first"))
        .unwrap()
        .dependencies = vec![ResourceId::new("fixture_item", "later")];
    initial.add_resource(
        state_with("later", "later")
            .resources
            .into_values()
            .next()
            .unwrap(),
    );
    initial.prepare_for_save();
    backend.local.save(&initial).await.unwrap();
    let resource = Arc::new(ProgressResource::new(Some("later")));
    let executor = executor_with(backend.clone(), json!({}), resource.clone()).await;
    let plan = executor.plan_destroy().await.unwrap();
    drop_when_next_operation_starts(&executor, &plan, &resource).await;
    let saved = backend.local.load().await.unwrap().unwrap();
    assert!(saved
        .get_resource(&ResourceId::new("fixture_item", "first"))
        .is_none());
    assert_eq!(saved.resources.len(), 1);
    assert!(saved
        .get_resource(&ResourceId::new("fixture_item", "later"))
        .is_some());
    assert_eq!(backend.saves.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn save_failure_stops_before_the_next_resource_call() {
    let directory = TempDir::new().unwrap();
    let backend = Arc::new(ProgressBackend::new(&directory, true));
    let resource = Arc::new(ProgressResource::new(None));
    let executor = executor_with(
        backend.clone(),
        json!({"first": {"id": "first"}, "later": {"id": "later", "depends_on": ["fixture_item.first"]}}),
        resource.clone(),
    ).await;
    let plan = executor.plan().await.unwrap();
    assert!(matches!(
        executor.apply(&plan).await,
        Err(ProvisioningError::StatePersistenceError(_))
    ));
    assert_eq!(*resource.ordinary.events.lock(), ["create:first"]);
    assert_eq!(backend.saves.load(Ordering::SeqCst), 1);
    assert!(backend.local.load().await.unwrap().is_none());
    assert!(backend.lock.get_lock().await.unwrap().is_none());
}
