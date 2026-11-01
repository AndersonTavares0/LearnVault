//! Testes HTTP das anotações de estudo (Study Notes).
//!
//! Valida autenticação, CSRF (same-origin), criação de notas na linha do tempo,
//! listagem cronológica, edição, exclusão e isolamento estrito entre usuários.

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
        temp.path().join("notes.sqlite").to_str().unwrap(),
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
    let db = Database::open(temp.path().join("notes.sqlite")).unwrap();
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

#[tokio::test]
async fn notes_endpoints_require_authentication() {
    let temp = tempfile::tempdir().unwrap();
    seed_database(&temp);
    let state = AppState::initialize(test_config(&temp)).await.unwrap();
    let app = build_router(state);

    let get_res = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes?media_id=media-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_res.status(), StatusCode::UNAUTHORIZED);

    let post_res = app
        .clone()
        .oneshot(
            Request::post("/api/study/notes")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","position_seconds":15,"content":"nota"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn notes_mutations_require_csrf_same_origin() {
    let temp = tempfile::tempdir().unwrap();
    seed_database(&temp);
    let state = AppState::initialize(test_config(&temp)).await.unwrap();
    let app = build_router(state);
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/study/notes")
                .header("cookie", &cookie)
                .header("origin", "http://attacker.com")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","position_seconds":15,"content":"nota"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn notes_crud_and_isolation_flow() {
    let temp = tempfile::tempdir().unwrap();
    seed_database(&temp);
    let state = AppState::initialize(test_config(&temp)).await.unwrap();
    let app = build_router(state);

    let admin_cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let student_cookie = login(&app, STUDENT, STUDENT_PASSWORD).await;

    // 1. Cria duas notas com o admin
    let create_res1 = app
        .clone()
        .oneshot(
            Request::post("/api/study/notes")
                .header("cookie", &admin_cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","position_seconds":60,"content":"Nota em 1m"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_res1.status(), StatusCode::CREATED);
    let note1_bytes = create_res1.into_body().collect().await.unwrap().to_bytes();
    let note1: serde_json::Value = serde_json::from_slice(&note1_bytes).unwrap();
    let note1_id = note1["note_id"].as_str().unwrap();

    let create_res2 = app
        .clone()
        .oneshot(
            Request::post("/api/study/notes")
                .header("cookie", &admin_cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","position_seconds":15,"content":"Nota em 15s"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_res2.status(), StatusCode::CREATED);

    // 2. Lista notas do admin: devem vir ordenadas por position_seconds (15s antes de 60s)
    let list_res = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes?media_id=media-1")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_res.status(), StatusCode::OK);
    let list_bytes = list_res.into_body().collect().await.unwrap().to_bytes();
    let list: serde_json::Value = serde_json::from_slice(&list_bytes).unwrap();
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["position_seconds"], 15);
    assert_eq!(items[1]["position_seconds"], 60);

    // 3. O estudante consulta as notas: não deve ver nenhuma do admin
    let student_list_res = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes?media_id=media-1")
                .header("cookie", &student_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let student_bytes = student_list_res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let student_list: serde_json::Value = serde_json::from_slice(&student_bytes).unwrap();
    assert_eq!(student_list["items"].as_array().unwrap().len(), 0);

    // 4. Estudante tenta editar a nota do admin: 404
    let fail_edit = app
        .clone()
        .oneshot(
            Request::put(format!("/api/study/notes/{note1_id}"))
                .header("cookie", &student_cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"content":"Hack"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fail_edit.status(), StatusCode::NOT_FOUND);

    // 5. Admin edita a própria nota: 200
    let ok_edit = app
        .clone()
        .oneshot(
            Request::put(format!("/api/study/notes/{note1_id}"))
                .header("cookie", &admin_cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"content":"Nota em 1m atualizada"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok_edit.status(), StatusCode::OK);

    // 6. Admin exclui a nota: 204
    let delete_res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/study/notes/{note1_id}"))
                .header("cookie", &admin_cookie)
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    // 7. Lista novamente: apenas 1 nota remanescente
    let final_list = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes?media_id=media-1")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let final_bytes = final_list.into_body().collect().await.unwrap().to_bytes();
    let final_json: serde_json::Value = serde_json::from_slice(&final_bytes).unwrap();
    assert_eq!(final_json["items"].as_array().unwrap().len(), 1);

    // 8. Lista global de notas (sem media_id): deve retornar com relative_path
    let global_list = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(global_list.status(), StatusCode::OK);
    let global_bytes = global_list.into_body().collect().await.unwrap().to_bytes();
    let global_json: serde_json::Value = serde_json::from_slice(&global_bytes).unwrap();
    let global_items = global_json["items"].as_array().unwrap();
    assert_eq!(global_items.len(), 1);
    assert_eq!(global_items[0]["relative_path"], "curso/aula1.mp4");

    // 9. Lista global do estudante continua isolada
    let student_global = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes")
                .header("cookie", &student_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let student_global_bytes = student_global
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let student_global_json: serde_json::Value =
        serde_json::from_slice(&student_global_bytes).unwrap();
    assert_eq!(student_global_json["items"].as_array().unwrap().len(), 0);
}
