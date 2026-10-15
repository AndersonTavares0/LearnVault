use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use learnvault_server::{AppState, Config, build_router};
use std::fs;
use tempfile::TempDir;
use tower::ServiceExt;

const SHELL: &str = "<!doctype html><html><body>LearnVault SPA</body></html>";
const CSS: &str = "body { color: rebeccapurple; }";

fn web_root(temp: &TempDir) -> std::path::PathBuf {
    let root = temp.path().join("web");
    fs::create_dir_all(root.join("_app/immutable/assets")).unwrap();
    fs::write(root.join("200.html"), SHELL).unwrap();
    fs::write(root.join("_app/immutable/assets/app.css"), CSS).unwrap();
    root
}

fn config(temp: &TempDir, web_root: &std::path::Path) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("learnvault.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
        "--web-root",
        web_root.to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some("admin".to_owned());
    config.admin_password = Some("uma senha local muito segura".to_owned());
    config.cookie_secure = false;
    config
}

async fn app(temp: &TempDir, web_root: &std::path::Path) -> axum::Router {
    build_router(AppState::initialize(config(temp, web_root)).await.unwrap())
}

#[test]
fn web_root_argument_accepts_a_directory_and_rejects_non_directories() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);

    assert!(
        Config::from_args([
            "learnvault-server",
            "--database",
            temp.path().join("learnvault.sqlite").to_str().unwrap(),
            "--library-root",
            temp.path().to_str().unwrap(),
            "--web-root",
            root.to_str().unwrap(),
        ])
        .is_ok()
    );

    let file = temp.path().join("not-a-directory");
    fs::write(&file, "not a web root").unwrap();
    let error = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("learnvault.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
        "--web-root",
        file.to_str().unwrap(),
    ])
    .unwrap_err();

    assert!(error.to_string().contains("web root"));
    assert!(!error.to_string().contains("not-a-directory"));
}

#[tokio::test]
async fn spa_serves_existing_assets_with_mime_security_and_immutable_cache_headers() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let response = app(&temp, &root)
        .await
        .oneshot(
            Request::get("/_app/immutable/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/css");
    assert_eq!(
        response.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["x-frame-options"], "DENY");
    assert_eq!(response.headers()["referrer-policy"], "no-referrer");
    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    assert!(csp.contains("default-src 'self'"));
    assert!(csp.contains("frame-ancestors 'none'"));
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        CSS
    );
}

#[tokio::test]
async fn extensionless_spa_routes_fall_back_to_the_shell_without_caching_it() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let response = app(&temp, &root)
        .await
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(response.headers()["cache-control"], "no-cache");
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        SHELL
    );
}

#[tokio::test]
async fn missing_assets_and_unknown_api_routes_remain_not_found() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let app = app(&temp, &root).await;

    let missing_asset = app
        .clone()
        .oneshot(
            Request::get("/_app/immutable/assets/missing.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_asset.status(), StatusCode::NOT_FOUND);

    let unknown_api = app
        .oneshot(Request::get("/api/unknown").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unknown_api.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn missing_spa_shell_returns_service_unavailable() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let config = config(&temp, &root);
    fs::remove_file(root.join("200.html")).unwrap();
    let app = build_router(AppState::initialize(config).await.unwrap());

    let response = app
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
