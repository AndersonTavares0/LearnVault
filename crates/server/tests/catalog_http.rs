use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use learnvault_server::{AppState, Config, build_router};
use tempfile::TempDir;
use tower::ServiceExt;

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
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

#[tokio::test]
async fn catalog_requires_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let response = app
        .oneshot(Request::get("/api/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_rescan_populates_the_authenticated_catalog() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("Categoria/Curso")).unwrap();
    std::fs::write(temp.path().join("Categoria/Curso/lesson.mp4"), b"video").unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;

    let rescan = app
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
    assert_eq!(rescan.status(), StatusCode::OK);

    let catalog = app
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let body = catalog.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["categories"][0]["title"], "Categoria");
}
