//! Endpoints de situação e cancelamento da indexação automática.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_server::{AppState, Config, build_router};
use tempfile::TempDir;
use tower::ServiceExt;

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("indexing.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
        "--no-update-check",
    ])
    .unwrap();
    config.admin_username = Some("admin".to_owned());
    config.admin_password = Some("uma senha local muito segura".to_owned());
    config.cookie_secure = false;
    config
}

async fn login(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"uma senha local muito segura"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

fn assert_private_no_store(response: &Response) {
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
}

#[tokio::test]
async fn indexing_status_requires_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let response = app
        .oneshot(Request::get("/api/indexing").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&response);
}

#[tokio::test]
async fn indexing_status_reports_idle_before_the_first_job() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;

    let response = app
        .oneshot(
            Request::get("/api/indexing")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_private_no_store(&response);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "idle");
    assert_eq!(json["started_at"], serde_json::Value::Null);
    assert_eq!(json["finished_at"], serde_json::Value::Null);
    assert_eq!(json["last_error"], serde_json::Value::Null);
    assert_eq!(json["progress"]["files_seen"], 0);
    assert_eq!(json["progress"]["added"], 0);
    assert_eq!(json["progress"]["changed"], 0);
    assert_eq!(json["progress"]["removed"], 0);
    assert_eq!(json["progress"]["moved"], 0);
}

#[tokio::test]
async fn indexing_cancel_requires_origin_and_an_active_job() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;

    let without_origin = app
        .clone()
        .oneshot(
            Request::post("/api/indexing/cancel")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(without_origin.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&without_origin);

    // Sem job em execução, o cancelamento é um conflito previsível.
    let idle = app
        .oneshot(
            Request::post("/api/indexing/cancel")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(idle.status(), StatusCode::CONFLICT);
    assert_private_no_store(&idle);
}
