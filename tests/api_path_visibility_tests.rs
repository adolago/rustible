#![cfg(feature = "api")]
//! In-process requests against private temporary paths; no jobs or listeners.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use rustible::api::{ApiConfig, ApiServer};
use serde_json::json;
use tower::ServiceExt;

async fn submit_path(server: &ApiServer, path: &std::path::Path) -> StatusCode {
    let token = server
        .state()
        .jwt_auth
        .generate_token("synthetic-user")
        .unwrap();
    server
        .router()
        .oneshot(
            Request::post("/api/v1/playbooks/execute")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"playbook": path.to_str().unwrap()}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn outside_absolute_path_does_not_disclose_whether_it_exists() {
    let directory = tempfile::tempdir().unwrap();
    let allowed = directory.path().join("playbooks");
    std::fs::create_dir(&allowed).unwrap();
    let outside = directory.path().join("playbooks-sibling.yml");
    std::fs::write(&outside, "[]\n").unwrap();
    let server = ApiServer::new(ApiConfig {
        playbook_paths: vec![allowed.to_str().unwrap().into()],
        ..Default::default()
    });
    let present = submit_path(&server, &outside).await;
    std::fs::remove_file(&outside).unwrap();
    let absent = submit_path(&server, &outside).await;
    assert_eq!(present, absent, "outside paths must have the same response");
    assert_eq!(present, StatusCode::NOT_FOUND);
    assert!(server.state().jobs.read().is_empty());
}

#[tokio::test]
async fn no_allowed_roots_does_not_disclose_absolute_path_existence() {
    let directory = tempfile::tempdir().unwrap();
    let outside = directory.path().join("synthetic.yml");
    std::fs::write(&outside, "[]\n").unwrap();
    let server = ApiServer::new(ApiConfig {
        playbook_paths: Vec::new(),
        ..Default::default()
    });
    assert_eq!(submit_path(&server, &outside).await, StatusCode::NOT_FOUND);
    assert!(server.state().jobs.read().is_empty());
}
