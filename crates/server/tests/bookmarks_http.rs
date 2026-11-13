//! HTTP tests for timestamp bookmarks: authentication, CSRF, ordering, validation and
//! strict isolation between users.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use learnvault_core::Database;
use learnvault_server::{AppState, Config, build_router, hash_password};
use tempfile::TempDir;
use tower::ServiceExt;

const ADMIN: &str = "admin";
const ADMIN_PASSWORD: &str = "senha-admin-longa-e-segura";
const STUDENT: &str = "estudante";
const STUDENT_PASSWORD: &str = "senha-aluno-longa-e-segura";
const STUDENT_ID: &str = "usr-estudante";

fn test_config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("bookmarks.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some(ADMIN.to_owned());
    config.admin_password = Some(ADMIN_PASSWORD.to_owned());
    config.cookie_secure = false;
    config
}

fn seed_database(temp: &TempDir) {
    let db = Database::open(temp.path().join("bookmarks.sqlite")).unwrap();
    db.connection()
        .execute_batch(
            "INSERT INTO library (library_id, available) VALUES ('lib-1', 1);
             INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('media-1', 'lib-1', 'curso/aula1.mp4', 'curso/aula1.mp4', 1024, 1760000000, 'available');",
        )
        .unwrap();

    let student_hash = hash_password(STUDENT_PASSWORD).unwrap();
    db.connection()
        .execute_batch(&format!(
            "INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials)
             VALUES ('{STUDENT_ID}', '{STUDENT}', '{student_hash}', 0, 1, 0);"
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
    assert_eq!(response.status(), StatusCode::OK);
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

fn post(cookie: &str, body: &str) -> Request<Body> {
    Request::post("/api/study/bookmarks")
        .header("cookie", cookie)
        .header("origin", "http://localhost")
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn bookmarks_require_authentication_and_csrf() {
    let temp = tempfile::tempdir().unwrap();
    seed_database(&temp);
    let app = build_router(AppState::initialize(test_config(&temp)).await.unwrap());

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::get("/api/study/bookmarks?media_id=media-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let cross_origin = app
        .clone()
        .oneshot(
            Request::post("/api/study/bookmarks")
                .header("cookie", &cookie)
                .header("origin", "http://attacker.com")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","position_seconds":5,"label":"x"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn bookmarks_are_ordered_validated_and_isolated() {
    let temp = tempfile::tempdir().unwrap();
    seed_database(&temp);
    let app = build_router(AppState::initialize(test_config(&temp)).await.unwrap());
    let admin_cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let student_cookie = login(&app, STUDENT, STUDENT_PASSWORD).await;

    let late = app
        .clone()
        .oneshot(post(
            &admin_cookie,
            r#"{"media_id":"media-1","position_seconds":90,"label":"Segundo"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(late.status(), StatusCode::CREATED);
    let late_id = json(late).await["bookmark_id"].as_str().unwrap().to_owned();

    let early = app
        .clone()
        .oneshot(post(
            &admin_cookie,
            r#"{"media_id":"media-1","position_seconds":10,"label":"Primeiro"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(early.status(), StatusCode::CREATED);

    let list = app
        .clone()
        .oneshot(
            Request::get("/api/study/bookmarks?media_id=media-1")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let items = json(list).await;
    let items = items["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["position_seconds"], 10);
    assert_eq!(items[1]["position_seconds"], 90);

    // Another user sees nothing and cannot delete the admin's bookmark.
    let student_list = app
        .clone()
        .oneshot(
            Request::get("/api/study/bookmarks?media_id=media-1")
                .header("cookie", &student_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        json(student_list).await["items"].as_array().unwrap().len(),
        0
    );

    let foreign_delete = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/study/bookmarks/{late_id}"))
                .header("cookie", &student_cookie)
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_delete.status(), StatusCode::NOT_FOUND);

    let own_delete = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/study/bookmarks/{late_id}"))
                .header("cookie", &admin_cookie)
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(own_delete.status(), StatusCode::NO_CONTENT);

    let invalid_label = app
        .clone()
        .oneshot(post(
            &admin_cookie,
            &format!(
                r#"{{"media_id":"media-1","position_seconds":5,"label":"{}"}}"#,
                "a".repeat(201)
            ),
        ))
        .await
        .unwrap();
    assert_eq!(invalid_label.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let invalid_position = app
        .clone()
        .oneshot(post(
            &admin_cookie,
            r#"{"media_id":"media-1","position_seconds":-1,"label":"x"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(invalid_position.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let unknown_media = app
        .clone()
        .oneshot(post(
            &admin_cookie,
            r#"{"media_id":"media-404","position_seconds":5,"label":"x"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(unknown_media.status(), StatusCode::NOT_FOUND);
}
