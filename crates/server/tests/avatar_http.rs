use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt as _;
use learnvault_core::Database;
use learnvault_server::{AppState, Config, build_router, hash_password};
use tempfile::TempDir;
use tower::ServiceExt as _;

const ADMIN: &str = "admin";
const ADMIN_PASSWORD: &str = "admin-password-123";
const STUDENT: &str = "estudante";
const STUDENT_PASSWORD: &str = "estudante-password-123";
const STUDENT_ID: &str = "usr-estudante";

fn sample_png() -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    bytes.extend_from_slice(&[0; 32]);
    bytes
}

fn test_config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("avatar.sqlite").to_str().unwrap(),
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
    let db = Database::open(temp.path().join("avatar.sqlite")).unwrap();
    let student_hash = hash_password(STUDENT_PASSWORD).unwrap();
    db.connection()
        .execute_batch(&format!(
            "INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials)
             VALUES ('{STUDENT_ID}', '{STUDENT}', '{student_hash}', 0, 1, 0);"
        ))
        .unwrap();
}

async fn login(app: &axum::Router, user: &str, pass: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"username":"{user}","password":"{pass}"}}"#
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
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn avatar_endpoints_require_authentication_and_csrf() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(test_config(&temp)).await.unwrap();
    let app = build_router(state);

    // GET sem autenticação: 401
    let get_res = app
        .clone()
        .oneshot(
            Request::get("/api/user/avatar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_res.status(), StatusCode::UNAUTHORIZED);

    // PUT sem autenticação: 401
    let put_res = app
        .clone()
        .oneshot(
            Request::put("/api/user/avatar")
                .header("origin", "http://localhost")
                .body(Body::from(sample_png()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_res.status(), StatusCode::UNAUTHORIZED);

    // PUT autenticado sem origin (CSRF): 403
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let csrf_res = app
        .clone()
        .oneshot(
            Request::put("/api/user/avatar")
                .header("cookie", &cookie)
                .body(Body::from(sample_png()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(csrf_res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn avatar_crud_isolation_and_type_validation_flow() {
    let temp = tempfile::tempdir().unwrap();
    seed_database(&temp);
    let state = AppState::initialize(test_config(&temp)).await.unwrap();
    let app = build_router(state);
    let admin_cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let student_cookie = login(&app, STUDENT, STUDENT_PASSWORD).await;

    // 1. Inicialmente 404 (sem avatar cadastrado)
    let initial = app
        .clone()
        .oneshot(
            Request::get("/api/user/avatar")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(initial.status(), StatusCode::NOT_FOUND);

    // 2. Rejeição de arquivo com tipo não suportado
    let invalid_type = app
        .clone()
        .oneshot(
            Request::put("/api/user/avatar")
                .header("cookie", &admin_cookie)
                .header("origin", "http://localhost")
                .body(Body::from(b"texto comum sem magic bytes".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_type.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    // 3. Upload bem-sucedido de PNG para o admin
    let png = sample_png();
    let put_ok = app
        .clone()
        .oneshot(
            Request::put("/api/user/avatar")
                .header("cookie", &admin_cookie)
                .header("origin", "http://localhost")
                .body(Body::from(png.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put_ok.status(), StatusCode::OK);
    let put_bytes = put_ok.into_body().collect().await.unwrap().to_bytes();
    let put_json: serde_json::Value = serde_json::from_slice(&put_bytes).unwrap();
    assert!(put_json["version"].as_i64().is_some());

    // 4. Download do avatar do admin
    let get_admin = app
        .clone()
        .oneshot(
            Request::get("/api/user/avatar")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_admin.status(), StatusCode::OK);
    assert_eq!(
        get_admin.headers().get("content-type").unwrap(),
        "image/png"
    );
    assert_eq!(
        get_admin.headers().get("cache-control").unwrap(),
        "private, max-age=3600"
    );
    let etag = get_admin
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let downloaded = get_admin.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(downloaded.as_ref(), png.as_slice());

    // 4b. Revalidação condicional (304 Not Modified) quando If-None-Match coincide
    let get_not_modified = app
        .clone()
        .oneshot(
            Request::get("/api/user/avatar")
                .header("cookie", &admin_cookie)
                .header("if-none-match", &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_not_modified.status(), StatusCode::NOT_MODIFIED);

    // 5. Isolamento: o estudante continua sem avatar (404)
    let get_student = app
        .clone()
        .oneshot(
            Request::get("/api/user/avatar")
                .header("cookie", &student_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_student.status(), StatusCode::NOT_FOUND);

    // 6. Remoção do avatar do admin
    let delete_res = app
        .clone()
        .oneshot(
            Request::delete("/api/user/avatar")
                .header("cookie", &admin_cookie)
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_res.status(), StatusCode::NO_CONTENT);

    // 7. Após remoção, volta a responder 404
    let after_delete = app
        .clone()
        .oneshot(
            Request::get("/api/user/avatar")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after_delete.status(), StatusCode::NOT_FOUND);
}
