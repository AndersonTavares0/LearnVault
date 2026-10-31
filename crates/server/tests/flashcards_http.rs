//! Testes HTTP dos cartões de estudo pessoais com repetição espaçada (Flashcards).
//!
//! Valida autenticação, CSRF (same-origin), criação de cartões, listagem por vencimento,
//! edição, exclusão, revisão com agendamento espaçado (Again/Good) e isolamento estrito entre usuários.

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
const STUDENT_ID: &str = "usr-estudante-cards";

fn test_config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("flashcards.sqlite").to_str().unwrap(),
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
    let db = Database::open(temp.path().join("flashcards.sqlite")).unwrap();
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
    response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn flashcards_endpoints_require_authentication() {
    let temp = TempDir::new().unwrap();
    let config = test_config(&temp);
    seed_database(&temp);
    let state = AppState::initialize(config).await.unwrap();
    let app = build_router(state);

    // Listagem sem cookie
    let list = app
        .clone()
        .oneshot(
            Request::get("/api/study/flashcards?media_id=media-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::UNAUTHORIZED);

    // Criação sem cookie
    let create = app
        .clone()
        .oneshot(
            Request::post("/api/study/flashcards")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","front":"F","back":"V"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::UNAUTHORIZED);

    // Edição sem cookie
    let update = app
        .clone()
        .oneshot(
            Request::put("/api/study/flashcards/card-123")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"front":"F2","back":"V2"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::UNAUTHORIZED);

    // Exclusão sem cookie
    let delete = app
        .clone()
        .oneshot(
            Request::delete("/api/study/flashcards/card-123")
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::UNAUTHORIZED);

    // Revisão sem cookie
    let review = app
        .clone()
        .oneshot(
            Request::post("/api/study/flashcards/card-123/review")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"rating":"good"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(review.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn flashcards_mutations_require_csrf_same_origin() {
    let temp = TempDir::new().unwrap();
    let config = test_config(&temp);
    seed_database(&temp);
    let state = AppState::initialize(config).await.unwrap();
    let app = build_router(state);
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    // POST sem header origin
    let create = app
        .clone()
        .oneshot(
            Request::post("/api/study/flashcards")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","front":"F","back":"V"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::FORBIDDEN);

    // POST com origin malicioso
    let evil = app
        .clone()
        .oneshot(
            Request::post("/api/study/flashcards")
                .header("cookie", &cookie)
                .header("origin", "http://evil-tracker.com")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","front":"F","back":"V"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(evil.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn flashcards_crud_and_review_flow() {
    let temp = TempDir::new().unwrap();
    let config = test_config(&temp);
    seed_database(&temp);
    let state = AppState::initialize(config).await.unwrap();
    let app = build_router(state);

    let admin_cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let student_cookie = login(&app, STUDENT, STUDENT_PASSWORD).await;

    // 1. Criar cartão para o aluno
    let create_resp = app
        .clone()
        .oneshot(
            Request::post("/api/study/flashcards")
                .header("cookie", &student_cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"media-1","front":"O que é TDD?","back":"Desenvolvimento orientado a testes (Red-Green-Refactor)."}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let body = create_resp.into_body().collect().await.unwrap().to_bytes();
    let card: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let card_id = card["card_id"].as_str().unwrap().to_owned();
    assert_eq!(card["front"], "O que é TDD?");
    assert_eq!(card["interval_days"], 0);

    // 2. Listar cartões do aluno
    let list_resp = app
        .clone()
        .oneshot(
            Request::get("/api/study/flashcards?media_id=media-1")
                .header("cookie", &student_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = list_resp.into_body().collect().await.unwrap().to_bytes();
    let list_json: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    assert_eq!(list_json["items"].as_array().unwrap().len(), 1);

    // 3. Isolamento: Admin não vê os cartões do aluno
    let admin_list_resp = app
        .clone()
        .oneshot(
            Request::get("/api/study/flashcards?media_id=media-1")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_body = admin_list_resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let admin_json: serde_json::Value = serde_json::from_slice(&admin_body).unwrap();
    assert_eq!(admin_json["items"].as_array().unwrap().len(), 0);

    // 4. Revisar cartão (Good): intervalo passa de 0 para 1 dia
    let review_resp = app
        .clone()
        .oneshot(
            Request::post(format!("/api/study/flashcards/{card_id}/review"))
                .header("cookie", &student_cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"rating":"good"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(review_resp.status(), StatusCode::OK);
    let rev_body = review_resp.into_body().collect().await.unwrap().to_bytes();
    let rev_json: serde_json::Value = serde_json::from_slice(&rev_body).unwrap();
    assert_eq!(rev_json["interval_days"], 1);

    // 5. Revisar cartão com 'again': intervalo zera para 0
    let again_resp = app
        .clone()
        .oneshot(
            Request::post(format!("/api/study/flashcards/{card_id}/review"))
                .header("cookie", &student_cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"rating":"again"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(again_resp.status(), StatusCode::OK);
    let again_body = again_resp.into_body().collect().await.unwrap().to_bytes();
    let again_json: serde_json::Value = serde_json::from_slice(&again_body).unwrap();
    assert_eq!(again_json["interval_days"], 0);

    // 6. Atualizar texto do cartão
    let update_resp = app
        .clone()
        .oneshot(
            Request::put(format!("/api/study/flashcards/{card_id}"))
                .header("cookie", &student_cookie)
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"front":"O que é TDD? (atualizado)","back":"Test-Driven Development."}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_resp.status(), StatusCode::OK);
    let up_body = update_resp.into_body().collect().await.unwrap().to_bytes();
    let up_json: serde_json::Value = serde_json::from_slice(&up_body).unwrap();
    assert_eq!(up_json["front"], "O que é TDD? (atualizado)");

    // 7. Excluir cartão
    let del_resp = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/study/flashcards/{card_id}"))
                .header("cookie", &student_cookie)
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del_resp.status(), StatusCode::NO_CONTENT);

    // Confirmar exclusão na listagem
    let list_after = app
        .clone()
        .oneshot(
            Request::get("/api/study/flashcards?media_id=media-1")
                .header("cookie", &student_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let after_body = list_after.into_body().collect().await.unwrap().to_bytes();
    let after_json: serde_json::Value = serde_json::from_slice(&after_body).unwrap();
    assert_eq!(after_json["items"].as_array().unwrap().len(), 0);
}
