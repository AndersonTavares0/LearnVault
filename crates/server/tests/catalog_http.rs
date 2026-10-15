use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use learnvault_server::{AppState, Config, build_router};
use std::{
    fs::{self, OpenOptions},
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};
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
async fn rescan_requires_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let response = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
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

    let missing_origin = app
        .clone()
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_origin.status(), StatusCode::FORBIDDEN);

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
    assert_eq!(catalog.headers()["cache-control"], "private, no-store");
    let body = catalog.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["categories"][0]["title"], "Categoria");
}

#[tokio::test]
async fn rescan_returns_conflict_while_another_process_holds_its_lock() {
    let temp = tempfile::tempdir().unwrap();
    let lock_path = temp.path().join("catalog.sqlite.rescan.lock");
    let ready_path = temp.path().join("lock-ready");
    let release_path = temp.path().join("lock-release");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("rescan_lock_helper")
        .env("LEARNVAULT_RESCAN_LOCK_PATH", &lock_path)
        .env("LEARNVAULT_RESCAN_LOCK_READY", &ready_path)
        .env("LEARNVAULT_RESCAN_LOCK_RELEASE", &release_path)
        .spawn()
        .unwrap();
    wait_for_file(&ready_path);

    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;
    let response = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    fs::write(release_path, []).unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[test]
fn rescan_lock_helper() {
    let Ok(lock_path) = std::env::var("LEARNVAULT_RESCAN_LOCK_PATH") else {
        return;
    };
    let ready_path = std::env::var("LEARNVAULT_RESCAN_LOCK_READY").unwrap();
    let release_path = std::env::var("LEARNVAULT_RESCAN_LOCK_RELEASE").unwrap();
    let _lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .unwrap();
    _lock.lock().unwrap();
    fs::write(ready_path, []).unwrap();
    wait_for_file(Path::new(&release_path));
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(Instant::now() < deadline, "timed out waiting for {path:?}");
        thread::sleep(Duration::from_millis(10));
    }
}
