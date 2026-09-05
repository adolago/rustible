#![cfg(feature = "api")]
//! In-process HTTP fixtures. No listener, transport or executable task is used.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use rustible::api::{ApiConfig, ApiServer};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn login_status(limit: usize, body: impl Into<Body>) -> StatusCode {
    let server = ApiServer::new(ApiConfig {
        max_body_size: limit,
        ..Default::default()
    });
    server
        .router()
        .oneshot(
            Request::post("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(body.into())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn configured_body_limit_accepts_exact_and_smaller_json() {
    let body = json!({"username": "synthetic", "password": "synthetic"}).to_string();
    // Authentication failure proves the bounded JSON reached the handler.
    for limit in [body.len(), body.len() + 1] {
        assert_eq!(
            login_status(limit, body.clone()).await,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
async fn configured_body_limit_rejects_one_byte_over_without_content_length() {
    let body = json!({"username": "synthetic", "password": "synthetic"}).to_string();
    assert_eq!(
        login_status(body.len() - 1, body).await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn configured_body_limit_rejects_streamed_bytes_over_the_limit() {
    let body = json!({"username": "synthetic", "password": "synthetic"}).to_string();
    let limit = body.len() - 1;
    let split = body.len() / 2;
    let chunks = [
        Ok::<_, std::convert::Infallible>(body[..split].to_owned()),
        Ok(body[split..].to_owned()),
    ];
    let streamed = Body::from_stream(futures_util::stream::iter(chunks));
    assert_eq!(
        login_status(limit, streamed).await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn configured_body_limit_can_exceed_axums_default() {
    let body =
        json!({"username": "x".repeat(2 * 1024 * 1024), "password": "synthetic"}).to_string();
    assert_eq!(
        login_status(body.len(), body).await,
        StatusCode::UNAUTHORIZED
    );
}

async fn submit_restriction(field: &str, value: Value) {
    let directory = tempfile::tempdir().unwrap();
    // Even the baseline can only enqueue an empty playbook; no tasks or hosts.
    std::fs::write(directory.path().join("empty.yml"), "[]\n").unwrap();
    let server = ApiServer::new(ApiConfig {
        playbook_paths: vec![directory.path().to_str().unwrap().into()],
        ..Default::default()
    });
    let state = server.state();
    let token = state.jwt_auth.generate_token("synthetic-user").unwrap();
    let mut request = json!({"playbook": "empty.yml"});
    request[field] = value;
    let response = server
        .router()
        .oneshot(
            Request::post("/api/v1/playbooks/execute")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "{field}"
    );
    assert!(
        state.jobs.read().is_empty(),
        "rejected request must not create a job"
    );
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    let error: Value = serde_json::from_slice(&bytes).unwrap();
    let message = error["message"].as_str().unwrap();
    assert!(message.contains("unsupported") && message.contains(field));
}

#[tokio::test]
async fn requested_host_limit_is_rejected_before_job_creation() {
    submit_restriction("limit", json!("synthetic-host")).await;
    submit_restriction("limit", json!("")).await;
}

#[tokio::test]
async fn requested_tags_are_rejected_before_job_creation() {
    submit_restriction("tags", json!(["synthetic-tag"])).await;
}

#[tokio::test]
async fn requested_skip_tags_are_rejected_before_job_creation() {
    submit_restriction("skip_tags", json!(["synthetic-tag"])).await;
}

#[tokio::test]
async fn requested_start_task_is_rejected_before_job_creation() {
    submit_restriction("start_at_task", json!("synthetic-task")).await;
    submit_restriction("start_at_task", json!("")).await;
}

#[tokio::test]
async fn omitted_restrictions_retain_existing_path_validation() {
    let server = ApiServer::new(ApiConfig {
        playbook_paths: Vec::new(),
        ..Default::default()
    });
    let state = server.state();
    let token = state.jwt_auth.generate_token("synthetic-user").unwrap();
    let response = server
        .router()
        .oneshot(
            Request::post("/api/v1/playbooks/execute")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"playbook": "missing.yml", "tags": [], "skip_tags": []}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(state.jobs.read().is_empty());
}
