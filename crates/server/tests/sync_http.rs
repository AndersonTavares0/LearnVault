//! Testes HTTP do diário de sincronização.
//!
//! O dono é sempre o usuário da sessão: os casos provam o isolamento por dono, a
//! idempotência do `op_id`, o cursor monotônico e a leitura que ignora o próprio
//! dispositivo. Nenhum caso toca a biblioteca física.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_core::{Database, KeyMode};
use learnvault_server::{AppState, Config, build_router, hash_password};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

const ADMIN: &str = "admin";
const ADMIN_PASSWORD: &str = "uma senha local muito segura";
const READER: &str = "reader";
const READER_PASSWORD: &str = "senha local longa para aluno";

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("sync.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some(ADMIN.to_owned());
    config.admin_password = Some(ADMIN_PASSWORD.to_owned());
    config.cookie_secure = false;
    config
}

fn seed(temp: &TempDir) {
    let db = Database::open(temp.path().join("sync.sqlite")).unwrap();
    db.connection()
        .execute_batch("INSERT INTO library (library_id, available) VALUES ('library-1', 1);")
        .unwrap();
    let path = "curso/m1.mp4";
    let key = learnvault_core::paths::relative_path_key(path, KeyMode::for_host()).unwrap();
    db.connection()
        .execute_batch(&format!(
            "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('m1', 'library-1', '{path}', '{key}', 1024, 1760000000, 'available');"
        ))
        .unwrap();
}

fn seed_user(temp: &TempDir, user_id: &str, username: &str, password: &str) {
    let hash = hash_password(password).unwrap();
    Database::open(temp.path().join("sync.sqlite"))
        .unwrap()
        .connection()
        .execute_batch(&format!(
            "INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials)
             VALUES ('{user_id}', '{username}', '{hash}', 0, 1, 0);"
        ))
        .unwrap();
}

async fn login(app: &axum::Router, username: &str, password: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"username":"{username}","password":"{password}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "login de teste");
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

fn push_request(cookie: &str, body: Value) -> Request<Body> {
    Request::post("/api/sync/push")
        .header("origin", "http://localhost")
        .header("cookie", cookie)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn pull_request(uri: &str, cookie: &str) -> Request<Body> {
    Request::get(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

async fn json_body(response: Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn progress(op_id: &str, position: i64, completed: bool) -> Value {
    json!({
        "op_id": op_id,
        "client_at": 100,
        "payload": { "kind": "progress", "media_id": "m1", "position_seconds": position, "completed": completed }
    })
}

#[tokio::test]
async fn push_and_pull_require_a_session() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let push = app
        .clone()
        .oneshot(
            Request::post("/api/sync/push")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "device_id": "d1", "operations": [] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(push.status(), StatusCode::UNAUTHORIZED);

    let pull = app
        .oneshot(pull_request("/api/sync/pull?device=d1", ""))
        .await
        .unwrap();
    assert_eq!(pull.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn push_requires_the_same_origin() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let response = app
        .oneshot(
            Request::post("/api/sync/push")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "device_id": "d1", "operations": [progress("op-1", 5, false)] })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(response).await["error"], "csrf_failed");
}

#[tokio::test]
async fn push_is_idempotent_and_pull_skips_the_same_device() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let body = json!({ "device_id": "device-a", "operations": [progress("op-1", 90, true)] });
    let first = app
        .clone()
        .oneshot(push_request(&cookie, body.clone()))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let outcome = json_body(first).await;
    assert_eq!(outcome["applied"], 1);
    assert_eq!(outcome["duplicated"], 0);
    assert!(outcome["cursor"].as_i64().unwrap() >= 1);

    let replay = app
        .clone()
        .oneshot(push_request(&cookie, body))
        .await
        .unwrap();
    let replay = json_body(replay).await;
    assert_eq!(replay["applied"], 0);
    assert_eq!(replay["duplicated"], 1);

    let own = app
        .clone()
        .oneshot(pull_request("/api/sync/pull?device=device-a", &cookie))
        .await
        .unwrap();
    assert_eq!(own.status(), StatusCode::OK);
    assert_eq!(json_body(own).await["items"].as_array().unwrap().len(), 0);

    let other = app
        .oneshot(pull_request(
            "/api/sync/pull?device=device-b&cursor=0&limit=10",
            &cookie,
        ))
        .await
        .unwrap();
    let pulled = json_body(other).await;
    let items = pulled["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["op_id"], "op-1");
    assert_eq!(items[0]["device_id"], "device-a");
    assert_eq!(items[0]["payload"]["kind"], "progress");
    assert_eq!(pulled["has_more"], false);
}

#[tokio::test]
async fn operations_are_isolated_per_session_user() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp);
    seed_user(&temp, "usr-reader", READER, READER_PASSWORD);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin_cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let reader_cookie = login(&app, READER, READER_PASSWORD).await;

    let admin_push = app
        .clone()
        .oneshot(push_request(
            &admin_cookie,
            json!({ "device_id": "device-a", "operations": [progress("op-admin", 10, false)] }),
        ))
        .await
        .unwrap();
    assert_eq!(admin_push.status(), StatusCode::OK);
    let reader_push = app
        .clone()
        .oneshot(push_request(
            &reader_cookie,
            json!({ "device_id": "device-b", "operations": [progress("op-reader", 20, false)] }),
        ))
        .await
        .unwrap();
    assert_eq!(reader_push.status(), StatusCode::OK);

    let reader_pull = app
        .clone()
        .oneshot(pull_request(
            "/api/sync/pull?device=device-c",
            &reader_cookie,
        ))
        .await
        .unwrap();
    let items = json_body(reader_pull).await;
    let op_ids: Vec<&str> = items["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["op_id"].as_str().unwrap())
        .collect();
    assert_eq!(op_ids, vec!["op-reader"]);

    let admin_pull = app
        .oneshot(pull_request(
            "/api/sync/pull?device=device-c",
            &admin_cookie,
        ))
        .await
        .unwrap();
    let items = json_body(admin_pull).await;
    let op_ids: Vec<&str> = items["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["op_id"].as_str().unwrap())
        .collect();
    assert_eq!(op_ids, vec!["op-admin"]);
}

#[tokio::test]
async fn an_empty_device_is_refused_on_pull() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let response = app
        .oneshot(pull_request("/api/sync/pull?device=", &cookie))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_body(response).await["error"], "invalid_sync_operation");
}

#[tokio::test]
async fn an_operation_for_unknown_media_is_logged_without_a_progress_row() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let response = app
        .oneshot(push_request(
            &cookie,
            json!({
                "device_id": "device-a",
                "operations": [{
                    "op_id": "op-ghost",
                    "client_at": 100,
                    "payload": { "kind": "progress", "media_id": "ghost", "position_seconds": 5, "completed": true }
                }]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await["applied"], 1);

    let db = Database::open(temp.path().join("sync.sqlite")).unwrap();
    let progress: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM study_state WHERE media_id = 'ghost'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(progress, 0);
    let logged: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM sync_operation", [], |row| row.get(0))
        .unwrap();
    assert_eq!(logged, 1, "a operação continua no diário");
}
