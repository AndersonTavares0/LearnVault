//! HTTP tests for the review hub: every card of the session user, across lessons.
//!
//!
//! The owner is always the session user: each case proves the adapter derives it from the
//! session, validates the typed body, maps core errors and persists to the real SQLite file.
//! No case reads or writes the physical library.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_core::{Database, KeyMode};
use learnvault_server::{AppState, Config, build_router, hash_password};
use tempfile::TempDir;
use tower::ServiceExt;

const ADMIN: &str = "admin";
const ADMIN_PASSWORD: &str = "uma senha local muito segura";
const READER: &str = "reader";
const READER_PASSWORD: &str = "senha local longa para aluno";

/// Usuário comum criado pelo próprio teste, para provar o isolamento por dono.
const READER_ID: &str = "usr-reader";

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("study.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some(ADMIN.to_owned());
    config.admin_password = Some(ADMIN_PASSWORD.to_owned());
    config.cookie_secure = false;
    config
}

fn database_path(temp: &TempDir) -> std::path::PathBuf {
    temp.path().join("study.sqlite")
}

/// Prepara diretório de dados, mídia conhecida e, opcionalmente, estado de estudo inicial.
///
/// O banco é gravado antes do `AppState` abrir: as linhas de mídia precisam existir para
/// que o progresso seja aceito pela chave estrangeira do schema.
fn seed(temp: &TempDir, media: &[&str], states: &[(&str, &str, i64, bool, i64)]) {
    let db = Database::open(database_path(temp)).unwrap();
    db.connection()
        .execute_batch("INSERT INTO library (library_id, available) VALUES ('library-1', 1);")
        .unwrap();
    for media_id in media {
        let path = format!("curso/{media_id}.mp4");
        let key = learnvault_core::paths::relative_path_key(&path, KeyMode::for_host()).unwrap();
        db.connection()
            .execute_batch(&format!(
                "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
                 VALUES ('{media_id}', 'library-1', '{path}', '{key}', 1024, 1760000000, 'available');"
            ))
            .unwrap();
    }
    for (owner, media_id, position, completed, updated_at) in states {
        db.connection()
            .execute_batch(&format!(
                "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
                 VALUES ('{owner}', '{media_id}', {position}, {}, {updated_at});",
                i64::from(*completed)
            ))
            .unwrap();
    }
}

/// Cria um usuário comum antes do `AppState` abrir o banco.
fn seed_user(temp: &TempDir, user_id: &str, username: &str, password: &str) {
    let hash = hash_password(password).unwrap();
    Database::open(database_path(temp))
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

fn get_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::get(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

fn post_json(uri: &str, cookie: &str, body: &str) -> Request<Body> {
    Request::post(uri)
        .header("origin", "http://localhost")
        .header("cookie", cookie)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn json_body(response: Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
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
async fn cards_of_every_lesson_are_listed_for_the_session_user_only() {
    let temp = tempfile::tempdir().unwrap();
    seed_user(&temp, READER_ID, READER, READER_PASSWORD);
    seed(&temp, &["m1", "m2"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let reader = login(&app, READER, READER_PASSWORD).await;
    for (cookie, media, front) in [
        (&admin, "m1", "a"),
        (&admin, "m2", "b"),
        (&reader, "m1", "c"),
    ] {
        let created = app
            .clone()
            .oneshot(post_json(
                "/api/study/flashcards",
                cookie,
                &format!(r#"{{"media_id":"{media}","front":"{front}","back":"x"}}"#),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
    }

    let all = app
        .clone()
        .oneshot(get_with_cookie("/api/study/flashcards", &admin))
        .await
        .unwrap();
    assert_eq!(all.status(), StatusCode::OK);
    assert_private_no_store(&all);
    let all = json_body(all).await;
    let fronts: Vec<&str> = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| card["front"].as_str().unwrap())
        .collect();
    assert_eq!(
        fronts,
        ["a", "b"],
        "both lessons, and not the other reader's card"
    );

    let one = app
        .oneshot(get_with_cookie("/api/study/flashcards?media_id=m2", &admin))
        .await
        .unwrap();
    assert_eq!(json_body(one).await["items"].as_array().unwrap().len(), 1);
}
