use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_core::Database;
use learnvault_server::{AppState, Config, build_router, hash_password};
use tempfile::TempDir;
use tower::ServiceExt;

fn config(temp: &TempDir) -> Config {
    let course_root = temp.path().join("courses");
    std::fs::create_dir_all(&course_root).unwrap();
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        course_root.to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some("admin".to_owned());
    config.admin_password = Some("uma senha local muito segura".to_owned());
    config.cookie_secure = false;
    config
}

async fn login(app: &axum::Router) -> String {
    login_as(app, "admin", "uma senha local muito segura").await
}

async fn login_as(app: &axum::Router, username: &str, password: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "username": username, "password": password }).to_string(),
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

async fn json_body(response: Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn request_catalog(app: &axum::Router, cookie: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

#[tokio::test]
async fn authenticated_same_origin_order_updates_the_catalog_and_rejects_invalid_siblings() {
    let temp = tempfile::tempdir().unwrap();
    let course_path = temp.path().join("courses").join("Curso");
    for module in ["Módulo 1", "Módulo 2"] {
        std::fs::create_dir_all(course_path.join(module).join("Aula 1")).unwrap();
        std::fs::write(
            course_path.join(module).join("Aula 1").join("video.mp4"),
            b"video",
        )
        .unwrap();
    }

    let database = Database::open(temp.path().join("catalog.sqlite")).unwrap();
    let reader_password = "uma senha de aluno longa";
    let password_hash = hash_password(reader_password).unwrap();
    database
        .connection()
        .execute(
            "INSERT INTO user (user_id, username, password_hash, is_admin, created_at)
             VALUES ('usr-reader', 'reader', ?1, 0, 1)",
            [&password_hash],
        )
        .unwrap();
    drop(database);

    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;
    let imported = app
        .clone()
        .oneshot(
            Request::post("/api/admin/courses")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "path": course_path.to_str().unwrap() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported.status(), StatusCode::OK);
    let course_id = json_body(imported).await["course"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let scanned = app
        .clone()
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(scanned.status(), StatusCode::OK);

    let reorder = app
        .clone()
        .oneshot(
            Request::put("/api/catalog/order")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "course_id": course_id,
                        "parent_path": "",
                        "child_paths": ["Módulo 2", "Módulo 1"],
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reorder.status(), StatusCode::OK);
    assert_eq!(
        request_catalog(&app, &cookie).await["categories"][0]["children"][0]["path"],
        "Módulo 2"
    );

    let reader_cookie = login_as(&app, "reader", reader_password).await;
    assert_eq!(
        request_catalog(&app, &reader_cookie).await["categories"][0]["children"][0]["path"],
        "Módulo 1",
        "a preferência de outro usuário não pode alterar a ordenação natural"
    );

    let invalid = app
        .clone()
        .oneshot(
            Request::put("/api/catalog/order")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "course_id": course_id,
                        "parent_path": "",
                        "child_paths": ["not-a-sibling"],
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        request_catalog(&app, &cookie).await["categories"][0]["children"][0]["path"],
        "Módulo 2",
        "uma lista inválida não deve remover a ordem já salva"
    );

    let large_invalid_order = app
        .clone()
        .oneshot(
            Request::put("/api/catalog/order")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "course_id": course_id,
                        "parent_path": "",
                        "child_paths": (0..1_000).map(|index| format!("missing-{index:04}")).collect::<Vec<_>>(),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        large_invalid_order.status(),
        StatusCode::BAD_REQUEST,
        "a valid-sized sibling list should reach sibling validation instead of the global 8 KiB body limit"
    );

    let reset = app
        .clone()
        .oneshot(
            Request::delete("/api/catalog/order")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "course_id": course_id, "parent_path": "" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reset.status(), StatusCode::OK);
    assert_eq!(
        request_catalog(&app, &cookie).await["categories"][0]["children"][0]["path"],
        "Módulo 1",
        "reset deve restaurar a ordenação natural"
    );
}

#[tokio::test]
async fn order_mutations_require_authentication_and_same_origin() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let body = serde_json::json!({
        "course_id": "course-opaque",
        "parent_path": "",
        "child_paths": ["Módulo 1"],
    })
    .to_string();

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::put("/api/catalog/order")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let cookie = login(&app).await;
    let cross_origin = app
        .oneshot(
            Request::put("/api/catalog/order")
                .header("origin", "https://attacker.example")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn import_preview_is_read_only_and_reports_physical_tree_and_suggestions() {
    let temp = tempfile::tempdir().unwrap();
    let course_path = temp.path().join("courses").join("Curso Prévia");
    std::fs::create_dir_all(course_path.join("01 - Módulo").join("Aula 02")).unwrap();
    std::fs::create_dir_all(course_path.join("temp_cache")).unwrap();
    std::fs::write(
        course_path
            .join("01 - Módulo")
            .join("Aula 02")
            .join("video.mp4"),
        b"video",
    )
    .unwrap();
    std::fs::write(course_path.join("01 - Módulo").join("nota.pdf"), b"pdf").unwrap();
    std::fs::write(course_path.join("README.xyz"), b"unknown").unwrap();

    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;
    let catalog_before = request_catalog(&app, &cookie).await;
    let preview = app
        .clone()
        .oneshot(
            Request::post("/api/admin/courses/preview")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "path": course_path }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let body = json_body(preview).await;
    assert_eq!(body["discovery"]["summary"]["found"], 2);
    assert_eq!(body["discovery"]["summary"]["ignored"], 1);
    assert_eq!(body["discovery"]["summary"]["unsupported"], 1);
    assert_eq!(body["discovery"]["root"]["path"], ".");
    assert!(
        body["interpretation"]["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["ambiguous"] == true)
    );

    let catalog_after = request_catalog(&app, &cookie).await;
    assert_eq!(
        catalog_after, catalog_before,
        "a prévia não pode alterar o catálogo"
    );
}
