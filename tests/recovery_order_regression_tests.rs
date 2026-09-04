//! Harmless public-manager regressions for rollback execution order.

use rustible::recovery::{RecoveryConfig, RecoveryManager, StateChange};
use std::sync::Arc;

#[tokio::test]
async fn test_diligence_recovery_repeated_file_changes_restore_original() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("destination");
    let original = directory.path().join("original");
    let intermediate = directory.path().join("intermediate");
    std::fs::write(&original, "A").unwrap();
    std::fs::write(&intermediate, "B").unwrap();
    std::fs::write(&destination, "C").unwrap();
    let manager = RecoveryManager::new(RecoveryConfig::default());
    let context = manager.begin_rollback_tracking().await.unwrap();
    for backup in [original, intermediate] {
        manager
            .record_state_change(
                &context.id,
                StateChange::FileModified {
                    path: destination.clone(),
                    backup_path: backup,
                    original_content_hash: None,
                },
            )
            .await
            .unwrap();
    }
    manager.rollback(&context.id).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(destination).unwrap(),
        "A",
        "rollback restored an intermediate version instead of the original"
    );
}

struct ObserveFileModule {
    path: std::path::PathBuf,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl rustible::modules::Module for ObserveFileModule {
    fn name(&self) -> &'static str {
        "service"
    }
    fn description(&self) -> &'static str {
        "Read-only priority-order fixture"
    }
    fn execute(
        &self,
        _: &rustible::modules::ModuleParams,
        _: &rustible::modules::ModuleContext,
    ) -> rustible::modules::ModuleResult<rustible::modules::ModuleOutput> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            std::fs::read_to_string(&self.path).unwrap(),
            "current",
            "higher-priority action ran after the lower-priority file restoration"
        );
        Ok(rustible::modules::ModuleOutput::ok(
            "observed before file restoration",
        ))
    }
}

#[tokio::test]
async fn test_diligence_recovery_preserves_plan_priority() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("destination");
    let backup = directory.path().join("backup");
    std::fs::write(&destination, "current").unwrap();
    std::fs::write(&backup, "original").unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut registry = rustible::modules::ModuleRegistry::new();
    registry.register(Arc::new(ObserveFileModule {
        path: destination.clone(),
        calls: calls.clone(),
    }));
    let manager = RecoveryManager::new(RecoveryConfig::default());
    manager.set_module_registry(Arc::new(registry));
    let context = manager.begin_rollback_tracking().await.unwrap();
    manager
        .record_state_change(
            &context.id,
            StateChange::FileModified {
                path: destination.clone(),
                backup_path: backup,
                original_content_hash: None,
            },
        )
        .await
        .unwrap();
    manager
        .record_state_change(
            &context.id,
            StateChange::ServiceStateChanged {
                service: "synthetic-service".into(),
                previous_state: "stopped".into(),
                new_state: "running".into(),
            },
        )
        .await
        .unwrap();
    manager.rollback(&context.id).await.unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(std::fs::read_to_string(destination).unwrap(), "original");
}
