//! HTTP tests for PDF annotations.
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

/// Ordinary user created by the test itself, to prove isolation by owner.
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

/// Prepares the data directory, known media and, optionally, initial study state.
///
/// The database is written before the `AppState` opens: the media rows must exist so
/// that progress is accepted by the schema's foreign key.
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

/// Creates an ordinary user before the `AppState` opens the database.
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

fn put_json(uri: &str, cookie: &str, body: &str) -> Request<Body> {
    Request::put(uri)
        .header("origin", "http://localhost")
        .header("cookie", cookie)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn delete(uri: &str, cookie: &str) -> Request<Body> {
    Request::delete(uri)
        .header("origin", "http://localhost")
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

const HIGHLIGHT: &str = r#"{"media_id":"m1","page":12,"kind":"highlight","color":"yellow","rects":[{"x":0.1,"y":0.2,"width":0.5,"height":0.03}],"quote":"A reference cannot outlive its owner."}"#;

#[tokio::test]
async fn annotations_are_created_listed_edited_and_deleted() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let created = app
        .clone()
        .oneshot(post_json("/api/study/pdf-annotations", &cookie, HIGHLIGHT))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_private_no_store(&created);
    let created = json_body(created).await;
    assert_eq!(created["page"], 12);
    assert_eq!(created["kind"], "highlight");
    assert_eq!(created["color"], "yellow");
    assert_eq!(created["rects"][0]["width"], 0.5);
    let id = created["annotation_id"].as_str().unwrap().to_owned();

    let listed = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=m1",
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    assert_private_no_store(&listed);
    assert_eq!(
        json_body(listed).await["items"].as_array().unwrap().len(),
        1
    );

    let edited = app
        .clone()
        .oneshot(put_json(
            &format!("/api/study/pdf-annotations/{id}"),
            &cookie,
            r#"{"color":"green","content":"key idea"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(edited.status(), StatusCode::OK);
    let edited = json_body(edited).await;
    assert_eq!(edited["color"], "green");
    assert_eq!(edited["content"], "key idea");
    assert_eq!(edited["quote"], "A reference cannot outlive its owner.");

    let deleted = app
        .clone()
        .oneshot(delete(&format!("/api/study/pdf-annotations/{id}"), &cookie))
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let again = app
        .clone()
        .oneshot(delete(&format!("/api/study/pdf-annotations/{id}"), &cookie))
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(again).await["error"], "pdf_annotation_not_found");
}

#[tokio::test]
async fn annotations_belong_to_the_session_user() {
    let temp = tempfile::tempdir().unwrap();
    seed_user(&temp, READER_ID, READER, READER_PASSWORD);
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let reader = login(&app, READER, READER_PASSWORD).await;

    let created = app
        .clone()
        .oneshot(post_json("/api/study/pdf-annotations", &admin, HIGHLIGHT))
        .await
        .unwrap();
    let id = json_body(created).await["annotation_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let seen = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=m1",
            &reader,
        ))
        .await
        .unwrap();
    assert!(
        json_body(seen).await["items"]
            .as_array()
            .unwrap()
            .is_empty(),
        "another reader sees nothing"
    );

    let edit = app
        .clone()
        .oneshot(put_json(
            &format!("/api/study/pdf-annotations/{id}"),
            &reader,
            r#"{"color":"pink"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(edit.status(), StatusCode::NOT_FOUND);
    let remove = app
        .clone()
        .oneshot(delete(&format!("/api/study/pdf-annotations/{id}"), &reader))
        .await
        .unwrap();
    assert_eq!(remove.status(), StatusCode::NOT_FOUND);

    let still = app
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=m1",
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(json_body(still).await["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_annotations_are_refused_and_not_stored() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let bodies = [
        // unknown kind and color
        r#"{"media_id":"m1","page":1,"kind":"strike","color":"yellow","rects":[{"x":0.1,"y":0.1,"width":0.2,"height":0.1}]}"#,
        r#"{"media_id":"m1","page":1,"kind":"highlight","color":"red","rects":[{"x":0.1,"y":0.1,"width":0.2,"height":0.1}]}"#,
        // page and rectangles out of range, and none at all
        r#"{"media_id":"m1","page":0,"kind":"highlight","color":"yellow","rects":[{"x":0.1,"y":0.1,"width":0.2,"height":0.1}]}"#,
        r#"{"media_id":"m1","page":1,"kind":"highlight","color":"yellow","rects":[{"x":0.9,"y":0.1,"width":0.5,"height":0.1}]}"#,
        r#"{"media_id":"m1","page":1,"kind":"highlight","color":"yellow","rects":[]}"#,
        // a note without text
        r#"{"media_id":"m1","page":1,"kind":"note","color":"yellow","rects":[{"x":0.1,"y":0.1,"width":0.2,"height":0.1}]}"#,
    ];
    for body in bodies {
        let response = app
            .clone()
            .oneshot(post_json("/api/study/pdf-annotations", &cookie, body))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{body}"
        );
        assert_eq!(json_body(response).await["error"], "invalid_pdf_annotation");
    }

    let unknown_media = app
        .clone()
        .oneshot(post_json(
            "/api/study/pdf-annotations",
            &cookie,
            &HIGHLIGHT.replace("m1", "ghost"),
        ))
        .await
        .unwrap();
    assert_eq!(unknown_media.status(), StatusCode::NOT_FOUND);

    let listed = app
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=m1",
            &cookie,
        ))
        .await
        .unwrap();
    assert!(
        json_body(listed).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn annotations_need_a_session_and_the_same_origin() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let anonymous = app
        .clone()
        .oneshot(
            Request::get("/api/study/pdf-annotations?media_id=m1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let cross_origin = app
        .clone()
        .oneshot(
            Request::post("/api/study/pdf-annotations")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(HIGHLIGHT))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);

    let malformed = app
        .oneshot(get_with_cookie("/api/study/pdf-annotations", &cookie))
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
}

fn seed_course_media(temp: &TempDir, library_id: &str, media: &[&str]) {
    let db = Database::open(database_path(temp)).unwrap();
    db.connection()
        .execute_batch(&format!(
            "INSERT INTO library (library_id, available) VALUES ('{library_id}', 1);"
        ))
        .unwrap();
    for media_id in media {
        let path = format!("{library_id}/{media_id}.pdf");
        let key = learnvault_core::paths::relative_path_key(&path, KeyMode::for_host()).unwrap();
        db.connection()
            .execute_batch(&format!(
                "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
                 VALUES ('{media_id}', '{library_id}', '{path}', '{key}', 1024, 1760000000, 'available');"
            ))
            .unwrap();
    }
}

#[tokio::test]
async fn annotations_enforce_course_access_grants() {
    let temp = tempfile::tempdir().unwrap();
    seed_user(&temp, READER_ID, READER, READER_PASSWORD);
    seed_course_media(&temp, "course-1", &["m1"]);
    seed_course_media(&temp, "course-2", &["m2"]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let reader = login(&app, READER, READER_PASSWORD).await;

    // Restrict reader to course-1 only.
    let grant = app
        .clone()
        .oneshot(put_json(
            &format!("/api/admin/users/{READER_ID}/courses"),
            &admin,
            r#"{"allowed_course_ids":["course-1"]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(grant.status(), StatusCode::OK);

    // Admin retains full access to course-2.
    let admin_m2 = app
        .clone()
        .oneshot(post_json(
            "/api/study/pdf-annotations",
            &admin,
            &HIGHLIGHT.replace("m1", "m2"),
        ))
        .await
        .unwrap();
    assert_eq!(admin_m2.status(), StatusCode::CREATED);
    let admin_m2_id = json_body(admin_m2).await["annotation_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Granted course (course-1) works for reader.
    let reader_m1 = app
        .clone()
        .oneshot(post_json("/api/study/pdf-annotations", &reader, HIGHLIGHT))
        .await
        .unwrap();
    assert_eq!(reader_m1.status(), StatusCode::CREATED);
    let reader_m1_id = json_body(reader_m1).await["annotation_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let listed_m1 = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=m1",
            &reader,
        ))
        .await
        .unwrap();
    assert_eq!(listed_m1.status(), StatusCode::OK);
    assert_eq!(
        json_body(listed_m1).await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let edited_m1 = app
        .clone()
        .oneshot(put_json(
            &format!("/api/study/pdf-annotations/{reader_m1_id}"),
            &reader,
            r#"{"color":"blue"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(edited_m1.status(), StatusCode::OK);

    let deleted_m1 = app
        .clone()
        .oneshot(delete(
            &format!("/api/study/pdf-annotations/{reader_m1_id}"),
            &reader,
        ))
        .await
        .unwrap();
    assert_eq!(deleted_m1.status(), StatusCode::NO_CONTENT);

    // Ungranted course (course-2) is denied with 404 media_not_found,
    // indistinguishable from a nonexistent media_id.
    let list_ungranted = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=m2",
            &reader,
        ))
        .await
        .unwrap();
    assert_eq!(list_ungranted.status(), StatusCode::NOT_FOUND);
    let list_ungranted_body = json_body(list_ungranted).await;

    let list_missing = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=ghost",
            &reader,
        ))
        .await
        .unwrap();
    assert_eq!(list_missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(list_ungranted_body, json_body(list_missing).await);

    let create_ungranted = app
        .clone()
        .oneshot(post_json(
            "/api/study/pdf-annotations",
            &reader,
            &HIGHLIGHT.replace("m1", "m2"),
        ))
        .await
        .unwrap();
    assert_eq!(create_ungranted.status(), StatusCode::NOT_FOUND);
    let create_ungranted_body = json_body(create_ungranted).await;

    let create_missing = app
        .clone()
        .oneshot(post_json(
            "/api/study/pdf-annotations",
            &reader,
            &HIGHLIGHT.replace("m1", "ghost"),
        ))
        .await
        .unwrap();
    assert_eq!(create_missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(create_ungranted_body, json_body(create_missing).await);

    let update_ungranted = app
        .clone()
        .oneshot(put_json(
            &format!("/api/study/pdf-annotations/{admin_m2_id}"),
            &reader,
            r#"{"color":"pink"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(update_ungranted.status(), StatusCode::NOT_FOUND);

    let delete_ungranted = app
        .oneshot(delete(
            &format!("/api/study/pdf-annotations/{admin_m2_id}"),
            &reader,
        ))
        .await
        .unwrap();
    assert_eq!(delete_ungranted.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn annotations_become_inaccessible_after_course_revocation() {
    let temp = tempfile::tempdir().unwrap();
    seed_user(&temp, READER_ID, READER, READER_PASSWORD);
    seed_course_media(&temp, "course-1", &["m1"]);
    seed_course_media(&temp, "course-2", &["m2"]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let reader = login(&app, READER, READER_PASSWORD).await;

    // Grant both courses initially so reader creates an annotation on course-2.
    let grant = app
        .clone()
        .oneshot(put_json(
            &format!("/api/admin/users/{READER_ID}/courses"),
            &admin,
            r#"{"allowed_course_ids":["course-1","course-2"]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(grant.status(), StatusCode::OK);

    let created = app
        .clone()
        .oneshot(post_json(
            "/api/study/pdf-annotations",
            &reader,
            &HIGHLIGHT.replace("m1", "m2"),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let annotation_id = json_body(created).await["annotation_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Admin revokes course-2.
    let revoke = app
        .clone()
        .oneshot(put_json(
            &format!("/api/admin/users/{READER_ID}/courses"),
            &admin,
            r#"{"allowed_course_ids":["course-1"]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::OK);

    // Reader can no longer list, create, update, or delete annotations on m2.
    let listed = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=m2",
            &reader,
        ))
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::NOT_FOUND);

    let created_again = app
        .clone()
        .oneshot(post_json(
            "/api/study/pdf-annotations",
            &reader,
            &HIGHLIGHT.replace("m1", "m2"),
        ))
        .await
        .unwrap();
    assert_eq!(created_again.status(), StatusCode::NOT_FOUND);

    let updated = app
        .clone()
        .oneshot(put_json(
            &format!("/api/study/pdf-annotations/{annotation_id}"),
            &reader,
            r#"{"color":"green","content":"changed while revoked"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::NOT_FOUND);

    let deleted = app
        .clone()
        .oneshot(delete(
            &format!("/api/study/pdf-annotations/{annotation_id}"),
            &reader,
        ))
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NOT_FOUND);

    // Re-granting course-2 confirms the original annotation was untouched.
    let regrant = app
        .clone()
        .oneshot(put_json(
            &format!("/api/admin/users/{READER_ID}/courses"),
            &admin,
            r#"{"allowed_course_ids":["course-1","course-2"]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(regrant.status(), StatusCode::OK);

    let restored = app
        .oneshot(get_with_cookie(
            "/api/study/pdf-annotations?media_id=m2",
            &reader,
        ))
        .await
        .unwrap();
    assert_eq!(restored.status(), StatusCode::OK);
    let items = json_body(restored).await["items"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["annotation_id"], annotation_id);
    assert_eq!(items[0]["color"], "yellow");
}
