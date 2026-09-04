#![cfg(feature = "api")]
//! Metadata-only fixtures: no background jobs, listeners or commands run.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use futures::FutureExt;
use rustible::api::{types::JobStatus, ApiConfig, ApiServer};
use serde_json::Value;
use tower::ServiceExt;

fn server_with_job(status: JobStatus, kernel: bool) -> (ApiServer, uuid::Uuid) {
    let server = ApiServer::new(ApiConfig::default());
    let state = server.state();
    let id = state.create_job("synthetic.yml".into(), None, None, Default::default());
    state.update_job_status(id, status);
    if kernel {
        state.register_kernel_job(id, &[]);
    }
    (server, id)
}

#[tokio::test]
async fn cancellation_api_refuses_to_report_unconfirmed_execution_stop() {
    for status in [
        JobStatus::Pending,
        JobStatus::Running,
        JobStatus::ActionRequired,
    ] {
        for kernel in [false, true] {
            let (server, id) = server_with_job(status, kernel);
            let state = server.state();
            let before = state.get_job(id).unwrap();
            let mut events = state.subscribe_to_job(id).unwrap();
            let runtime = state.get_kernel_job_runtime(id);
            let mut resume_wait = runtime
                .as_ref()
                .map(|runtime| Box::pin(runtime.resume_notify.notified()));
            if let Some(wait) = resume_wait.as_mut() {
                wait.as_mut().enable();
            }
            let token = state.jwt_auth.generate_token("synthetic-user").unwrap();
            let response = server
                .router()
                .oneshot(
                    Request::post(format!("/api/v1/jobs/{id}/cancel"))
                        .header("authorization", format!("Bearer {token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
            let error: Value = serde_json::from_slice(&bytes).unwrap();
            assert!(error["message"].as_str().unwrap().contains("unsupported"));
            let after = state.get_job(id).unwrap();
            assert_eq!(after.status, before.status);
            assert_eq!(after.finished_at, before.finished_at);
            assert!(matches!(
                events.try_recv(),
                Err(tokio::sync::broadcast::error::TryRecvError::Empty)
            ));
            if let Some(wait) = resume_wait.as_mut() {
                assert!(wait.as_mut().now_or_never().is_none());
            }
        }
    }
}

#[tokio::test]
async fn legacy_cancellation_method_does_not_change_execution_metadata() {
    for status in [
        JobStatus::Pending,
        JobStatus::Running,
        JobStatus::ActionRequired,
        JobStatus::Success,
        JobStatus::Failed,
        JobStatus::Cancelled,
    ] {
        for kernel in [false, true] {
            let (server, id) = server_with_job(status, kernel);
            let state = server.state();
            let before = state.get_job(id).unwrap();
            let mut events = state.subscribe_to_job(id).unwrap();
            let runtime = state.get_kernel_job_runtime(id);
            let mut resume_wait = runtime
                .as_ref()
                .map(|runtime| Box::pin(runtime.resume_notify.notified()));
            if let Some(wait) = resume_wait.as_mut() {
                wait.as_mut().enable();
            }
            assert!(
                !state.cancel_job(id),
                "stopping execution is not implemented"
            );
            let after = state.get_job(id).unwrap();
            assert_eq!(after.status, before.status);
            assert_eq!(after.finished_at, before.finished_at);
            assert!(matches!(
                events.try_recv(),
                Err(tokio::sync::broadcast::error::TryRecvError::Empty)
            ));
            if let Some(wait) = resume_wait.as_mut() {
                assert!(wait.as_mut().now_or_never().is_none());
            }
        }
    }
}

#[tokio::test]
async fn cancellation_reports_completed_and_missing_jobs_without_mutating_them() {
    for status in [JobStatus::Success, JobStatus::Failed, JobStatus::Cancelled] {
        let (server, id) = server_with_job(status, false);
        let state = server.state();
        let before = state.get_job(id).unwrap();
        let token = state.jwt_auth.generate_token("synthetic-user").unwrap();
        let response = server
            .router()
            .oneshot(
                Request::post(format!("/api/v1/jobs/{id}/cancel"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(state.get_job(id).unwrap().finished_at, before.finished_at);
    }
    let server = ApiServer::new(ApiConfig::default());
    let state = server.state();
    let token = state.jwt_auth.generate_token("synthetic-user").unwrap();
    let id = uuid::Uuid::new_v4();
    assert!(!state.cancel_job(id));
    let response = server
        .router()
        .oneshot(
            Request::post(format!("/api/v1/jobs/{id}/cancel"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(state.jobs.read().is_empty());
}
