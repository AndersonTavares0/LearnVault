//! HTTP tests for the global search endpoint.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use learnvault_core::{Database, course_id_from_path};
use learnvault_server::{AppState, Config, build_router, hash_password};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

const ADMIN: &str = "admin";
const ADMIN_PASSWORD: &str = "senha-admin-longa-e-segura";
const STUDENT: &str = "estudante";
const STUDENT_PASSWORD: &str = "senha-aluno-longa-e-segura";
const STUDENT_ID: &str = "usr-estudante";

struct Fixture {
    temp: TempDir,
    course_id: String,
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let course = temp.path().join("Course Folder");
    std::fs::create_dir(&course).unwrap();
    let course_id = course_id_from_path(&course.canonicalize().unwrap());
    let registry = serde_json::json!({
        "version": 1,
        "courses": [{ "id": course_id.clone(), "path": course }]
    });
    std::fs::write(temp.path().join("courses.json"), registry.to_string()).unwrap();

    let db = Database::open(temp.path().join("search.sqlite")).unwrap();
    db.connection()
        .execute_batch(&format!(
            "INSERT INTO library (library_id, available) VALUES ('{course_id}', 1);
             INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('media-1', '{course_id}', 'Module/01 - Ownership.mp4', 'module/01 - ownership.mp4', 1024, 1760000000, 'available');"
        ))
        .unwrap();
    let student_hash = hash_password(STUDENT_PASSWORD).unwrap();
    db.connection()
        .execute_batch(&format!(
            "INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials)
             VALUES ('{STUDENT_ID}', '{STUDENT}', '{student_hash}', 0, 1, 0);"
        ))
        .unwrap();
    Fixture { temp, course_id }
}

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("search.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
        "--no-update-check",
    ])
    .unwrap();
    config.admin_username = Some(ADMIN.to_owned());
    config.admin_password = Some(ADMIN_PASSWORD.to_owned());
    config.cookie_secure = false;
    config
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

async fn search(app: &axum::Router, cookie: &str, query: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::get(format!("/api/search?q={}", urlencode(query)))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
    (status, json)
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            b' ' => "+".to_owned(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

async fn create_note(app: &axum::Router, cookie: &str, content: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/study/notes")
                .header("cookie", cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"media_id":"media-1","position_seconds":75,"content":"{content}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn search_requires_authentication() {
    let fixture = fixture();
    let state = AppState::initialize(config(&fixture.temp)).await.unwrap();
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::get("/api/search?q=borrow")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
}

#[tokio::test]
async fn search_returns_grouped_results_and_keeps_notes_private() {
    let fixture = fixture();
    let state = AppState::initialize(config(&fixture.temp)).await.unwrap();
    let app = build_router(state);

    let admin = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let student = login(&app, STUDENT, STUDENT_PASSWORD).await;

    let note = create_note(&app, &admin, "Borrow checker rules").await;
    let note_id = note["note_id"].as_str().unwrap().to_owned();

    let (status, body) = search(&app, &admin, "borrow").await;
    assert_eq!(status, StatusCode::OK);
    let notes = body["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["note_id"], note_id);
    assert_eq!(notes[0]["course_id"], fixture.course_id);
    assert_eq!(notes[0]["course_title"], "Course Folder");
    assert_eq!(notes[0]["lesson_title"], "Ownership");
    assert_eq!(notes[0]["lesson_path"], "Module/01 - Ownership.mp4");
    assert_eq!(notes[0]["position_seconds"], 75);
    assert!(notes[0]["snippet"].as_str().unwrap().contains("<mark>"));

    // Another user never sees the note, even with access to the same course.
    let (_, student_body) = search(&app, &student, "borrow").await;
    assert!(student_body["notes"].as_array().unwrap().is_empty());

    let (_, lessons) = search(&app, &admin, "ownership").await;
    assert_eq!(lessons["lessons"].as_array().unwrap().len(), 1);
    assert_eq!(lessons["lessons"][0]["title"], "Ownership");
    assert_eq!(lessons["lessons"][0]["path"], "Module/01 - Ownership.mp4");
    assert_eq!(lessons["lessons"][0]["course_title"], "Course Folder");

    let (_, courses) = search(&app, &admin, "folder").await;
    assert_eq!(courses["courses"].as_array().unwrap().len(), 1);
    assert_eq!(courses["courses"][0]["course_id"], fixture.course_id);
    assert_eq!(courses["courses"][0]["title"], "Course Folder");
}

#[tokio::test]
async fn search_tracks_note_edits_and_deletion() {
    let fixture = fixture();
    let state = AppState::initialize(config(&fixture.temp)).await.unwrap();
    let app = build_router(state);
    let admin = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let note = create_note(&app, &admin, "Borrow checker rules").await;
    let note_id = note["note_id"].as_str().unwrap().to_owned();

    let response = app
        .clone()
        .oneshot(
            Request::put(format!("/api/study/notes/{note_id}"))
                .header("cookie", &admin)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"content":"Lifetimes explained"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    assert!(
        search(&app, &admin, "borrow").await.1["notes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        search(&app, &admin, "lifetimes").await.1["notes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let response = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/study/notes/{note_id}"))
                .header("cookie", &admin)
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        search(&app, &admin, "lifetimes").await.1["notes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn short_and_hostile_queries_are_safe() {
    let fixture = fixture();
    let state = AppState::initialize(config(&fixture.temp)).await.unwrap();
    let app = build_router(state);
    let admin = login(&app, ADMIN, ADMIN_PASSWORD).await;
    create_note(&app, &admin, "NEAR(foo bar) quoted").await;

    let (status, body) = search(&app, &admin, "a").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["notes"].as_array().unwrap().is_empty());
    assert!(body["courses"].as_array().unwrap().is_empty());
    assert!(body["lessons"].as_array().unwrap().is_empty());

    for query in [
        "NEAR(foo",
        "\"quoted\"",
        "*",
        "-",
        "AND OR NOT",
        "\u{1F600}",
    ] {
        let (status, _) = search(&app, &admin, query).await;
        assert_eq!(status, StatusCode::OK, "query {query:?}");
    }
}
