use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_core::Database;
use learnvault_server::{AppState, Config, build_router, hash_password};
use std::{
    fs::{self, OpenOptions},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tower::ServiceExt;

/// Generous deadline for the synchronization between processes of the rescan
/// lock test. A short value (seconds) becomes a flake on loaded CI runners.
const RESCAN_LOCK_TIMEOUT: Duration = Duration::from_secs(60);

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

/// The catalog is revalidated by `ETag`, so it may stay in the browser's private cache.
fn assert_private_no_cache(response: &Response) {
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("private, no-cache")
    );
    assert!(
        response.headers().get("etag").is_some(),
        "a resposta do catálogo precisa de um ETag de conteúdo"
    );
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

#[tokio::test]
async fn catalog_requires_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let response = app
        .oneshot(Request::get("/api/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&response);
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
    assert_private_no_store(&response);
}

#[tokio::test]
async fn catalog_and_rescan_reject_invalid_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = "learnvault_session=invalid-token";

    let catalog = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&catalog);

    let rescan = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("cookie", cookie)
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rescan.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&rescan);
}

#[tokio::test]
async fn catalog_and_rescan_reject_requests_when_blocking_capacity_is_saturated() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(config(&temp)).await.unwrap();
    let app = build_router(state.clone());
    let cookie = login(&app).await;
    let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();

    let catalog = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_private_no_store(&catalog);
    let catalog_body = catalog.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&catalog_body).unwrap()["error"],
        "server busy"
    );

    let rescan = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rescan.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_private_no_store(&rescan);
    let rescan_body = rescan.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&rescan_body).unwrap()["error"],
        "server busy"
    );
}

#[tokio::test]
async fn non_admin_cannot_rescan() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(temp.path().join("catalog.sqlite")).unwrap();
    let password_hash = hash_password("senha local longa para aluno").unwrap();
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
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"reader","password":"senha local longa para aluno"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let rescan = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rescan.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&rescan);
}

#[tokio::test]
async fn admin_rescan_populates_the_authenticated_catalog() {
    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    std::fs::create_dir_all(library.join("Categoria/Curso")).unwrap();
    std::fs::write(library.join("Categoria/Curso/lesson.mp4"), b"video").unwrap();
    // Loose file directly in the category: the legacy projection has nowhere to keep it, but
    // it must stay accessible in the canonical `nodes` tree.
    std::fs::write(library.join("Categoria/nota.pdf"), b"nota").unwrap();
    let mut server_config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        library.to_str().unwrap(),
    ])
    .unwrap();
    server_config.admin_username = Some("admin".to_owned());
    server_config.admin_password = Some("uma senha local muito segura".to_owned());
    server_config.cookie_secure = false;
    let app = build_router(AppState::initialize(server_config).await.unwrap());
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
    assert_private_no_store(&missing_origin);

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
    assert_private_no_store(&rescan);
    let rescan_body = rescan.into_body().collect().await.unwrap().to_bytes();
    let rescan_json: serde_json::Value = serde_json::from_slice(&rescan_body).unwrap();
    assert_eq!(rescan_json["complete"], true);
    assert_eq!(rescan_json["courses"], 1);
    assert_eq!(rescan_json["failed_courses"], 0);
    assert_eq!(rescan_json["inserted"], 2);
    assert!(rescan_json["scan_id"].as_i64().unwrap() > 0);
    // Sanitized response: it exposes only the count of ambiguous keys, never the relative
    // paths that make them up. Producing real ambiguity over HTTP is not portable (it depends on
    // per-platform separator normalization); the case with a path is covered deterministically
    // by the unit test of `RescanResponse::from` in `catalog.rs`.
    assert!(
        rescan_json["ambiguous_count"].is_u64(),
        "ambiguous_count precisa ser uma contagem numérica: {rescan_json}"
    );
    assert!(
        rescan_json.get("ambiguous").is_none(),
        "a lista de caminhos ambíguos não pode cruzar a API: {rescan_json}"
    );
    let rescan_text = String::from_utf8_lossy(&rescan_body);
    assert!(
        !rescan_text.contains("Categoria")
            && !rescan_text.contains("lesson.mp4")
            && !rescan_text.contains("nota.pdf"),
        "a resposta do rescan não pode vazar caminhos relativos: {rescan_text}"
    );

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
    assert_private_no_cache(&catalog);
    let body = catalog.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["complete"], true);

    // The library root registers each top-level folder as a course: here `Category`
    // becomes the course, and the top-level node is the tree relative to it.
    let course = &json["nodes"][0];
    assert_eq!(course["title"], "Categoria");
    assert_eq!(course["path"], "");
    assert_eq!(course["role"], "course");
    assert!(
        course["course_id"].is_string(),
        "o curso semeado precisa de identidade opaca: {course}"
    );
    assert_ne!(
        course["course_id"], "library-1",
        "a pasta de topo é o próprio curso"
    );
    assert_eq!(course["source_folder"], "library");
    // `nodes` is the canonical recursive tree, the contract preferred by the SPA: the lesson sits
    // under the module, with media identity, relative paths and verification.
    let module = &course["children"][0];
    assert_eq!(module["title"], "Curso");
    assert_eq!(module["path"], "Curso");
    assert_eq!(module["role"], "module");
    let lesson = &module["children"][0];
    assert_eq!(lesson["title"], "lesson");
    assert_eq!(lesson["path"], "Curso/lesson.mp4");
    assert_eq!(lesson["role"], "lesson");
    let node_file = &lesson["files"][0];
    assert_eq!(node_file["name"], "lesson.mp4");
    assert_eq!(node_file["path"], "Curso/lesson.mp4");
    assert_eq!(node_file["size"], 5);
    assert!(
        node_file["mtime_seconds"]
            .as_i64()
            .is_some_and(|mtime| mtime >= 0)
    );
    assert_eq!(node_file["kind"], "media");
    assert_eq!(node_file["availability"], "available");
    assert_eq!(node_file["unverified"], false);
    assert!(
        node_file["media_id"].is_string(),
        "a identidade de mídia precisa sobreviver em nodes"
    );
    // The course's loose file belongs to no lesson: it stays accessible, with no
    // loss, in the loose files of the course node itself.
    let loose = &course["loose_files"][0];
    assert_eq!(loose["name"], "nota.pdf");
    assert_eq!(loose["path"], "nota.pdf");
    assert!(
        loose["media_id"].is_string(),
        "o arquivo solto também carrega identidade de mídia em nodes"
    );

    // Each top-level node has the `course` role, an opaque `course_id` and the scanned subtree under it.
    assert_eq!(json["nodes"][0]["role"], "course");
    assert_eq!(json["nodes"][0]["course_id"], course["course_id"]);
    let file = find_file(&json["nodes"][0]).expect("lesson file in catalog");
    assert_eq!(file["name"], "lesson.mp4");
    assert_eq!(file["path"], "Curso/lesson.mp4");
    assert_eq!(file["size"], 5);
    assert_eq!(file["kind"], "media");
    assert_eq!(file["availability"], "available");
    assert_eq!(file["unverified"], false);
}

#[tokio::test]
async fn catalog_reports_database_errors_without_caching_or_leaking_paths() {
    let temp = tempfile::tempdir().unwrap();
    let course = temp.path().join("course");
    fs::create_dir(&course).unwrap();
    fs::write(course.join("lesson.mp4"), b"video").unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;
    Database::open(temp.path().join("catalog.sqlite"))
        .unwrap()
        .connection()
        .execute_batch("DROP TABLE media;")
        .unwrap();

    let response = app
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_private_no_store(&response);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "catalog unavailable");
    assert!(
        !body
            .windows(temp.path().to_str().unwrap().len())
            .any(|window| { window == temp.path().to_str().unwrap().as_bytes() })
    );
}

async fn course_rescan(app: &axum::Router, cookie: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_private_no_store(&response);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn scanned_course(
    temp: &TempDir,
) -> (axum::Router, String, std::path::PathBuf, Database, String) {
    let course = temp.path().join("library/Course");
    fs::create_dir_all(&course).unwrap();
    fs::write(course.join("lesson.webm"), b"video fixture").unwrap();
    let mut config = config(temp);
    config.library_root = Some(temp.path().join("library"));
    let app = build_router(AppState::initialize(config).await.unwrap());
    let cookie = login(&app).await;
    let summary = course_rescan(&app, &cookie).await;
    assert_eq!(summary["inserted"], 1);
    let database = Database::open(temp.path().join("catalog.sqlite")).unwrap();
    let media_id = database
        .connection()
        .query_row("SELECT media_id FROM media", [], |row| row.get(0))
        .unwrap();
    (app, cookie, course, database, media_id)
}

fn media_availability(database: &Database, media_id: &str) -> String {
    database
        .connection()
        .query_row(
            "SELECT availability FROM media WHERE media_id = ?1",
            [media_id],
            |row| row.get(0),
        )
        .unwrap()
}

#[tokio::test]
async fn rescan_marks_a_renamed_course_missing_and_restores_its_media_and_progress() {
    let temp = tempfile::tempdir().unwrap();
    let (app, cookie, course, database, media_id) = scanned_course(&temp).await;
    let complete = app
        .clone()
        .oneshot(
            Request::post("/api/study/completed")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "media_id": media_id, "completed": true }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete.status(), StatusCode::OK);

    let moved = course.with_file_name("Moved course");
    fs::rename(&course, &moved).unwrap();
    let summary = course_rescan(&app, &cookie).await;
    assert_eq!(summary["complete"], true);
    assert_eq!(summary["failed_courses"], 0);
    assert_eq!(summary["marked_missing"], 1);
    assert_eq!(media_availability(&database, &media_id), "missing");

    fs::rename(&moved, &course).unwrap();
    let summary = course_rescan(&app, &cookie).await;
    assert_eq!(summary["updated"], 1);
    assert_eq!(summary["inserted"], 0);
    assert_eq!(media_availability(&database, &media_id), "available");

    let progress = app
        .oneshot(
            Request::get(format!("/api/study/progress?media_id={media_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(progress.status(), StatusCode::OK);
    let body = progress.into_body().collect().await.unwrap().to_bytes();
    let progress: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(progress["recorded"], true);
    assert_eq!(progress["completed"], true);
}

#[tokio::test]
async fn rescan_marks_a_removed_course_missing_without_exposing_paths() {
    let temp = tempfile::tempdir().unwrap();
    let (app, cookie, course, database, media_id) = scanned_course(&temp).await;
    fs::remove_dir_all(&course).unwrap();

    let summary = course_rescan(&app, &cookie).await;
    assert_eq!(summary["marked_missing"], 1);
    assert_eq!(summary["failed_courses"], 0);
    assert_eq!(media_availability(&database, &media_id), "missing");
    assert!(!summary.to_string().contains(course.to_str().unwrap()));
}

#[tokio::test]
async fn rescan_preserves_media_if_the_course_parent_is_gone() {
    let temp = tempfile::tempdir().unwrap();
    let (app, cookie, course, database, media_id) = scanned_course(&temp).await;
    fs::rename(
        course.parent().unwrap(),
        temp.path().join("Offline library"),
    )
    .unwrap();

    let summary = course_rescan(&app, &cookie).await;
    assert_eq!(summary["complete"], false);
    assert_eq!(summary["failed_courses"], 1);
    assert_eq!(summary["marked_missing"], 0);
    assert_eq!(media_availability(&database, &media_id), "available");
}

#[cfg(unix)]
#[tokio::test]
async fn rescan_preserves_media_if_the_course_root_has_no_read_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let (app, cookie, course, database, media_id) = scanned_course(&temp).await;
    let original = fs::metadata(&course).unwrap().permissions();
    fs::set_permissions(&course, fs::Permissions::from_mode(0o000)).unwrap();
    let listing = fs::read_dir(&course);
    if listing.is_ok() {
        // Privileged runners can bypass the mode bits.
        eprintln!(
            "Skipping permission-denial assertions because the runner can list a mode-000 directory."
        );
        fs::set_permissions(&course, original).unwrap();
        return;
    }
    let summary = course_rescan(&app, &cookie).await;
    fs::set_permissions(&course, original).unwrap();
    assert_eq!(
        listing.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(summary["complete"], false);
    assert_eq!(summary["failed_courses"], 1);
    assert_eq!(summary["marked_missing"], 0);
    assert_eq!(media_availability(&database, &media_id), "available");
}

#[tokio::test]
async fn rescan_returns_conflict_while_another_process_holds_its_lock() {
    let temp = tempfile::tempdir().unwrap();
    let course = temp.path().join("course");
    fs::create_dir(&course).unwrap();
    fs::write(course.join("lesson.mp4"), b"video").unwrap();
    let lock_path = temp.path().join("catalog.sqlite.rescan.lock");
    let ready_path = temp.path().join("lock-ready");

    // Bootstrap (the admin's Argon2id) and login (Argon2id) are deliberately slow
    // and can take seconds on a loaded CI runner. Prepare them before
    // spawning the child so that the window between `lock-ready` and the release covers
    // only the request, and not the heavy startup work.
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;

    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("rescan_lock_helper")
        .env("LEARNVAULT_RESCAN_LOCK_HELPER", "1")
        .env("LEARNVAULT_RESCAN_LOCK_PATH", &lock_path)
        .env("LEARNVAULT_RESCAN_LOCK_READY", &ready_path)
        // The release is signalled through the child's stdin channel, not by a file
        // with a deadline: a real channel is deterministic and frees the process on
        // close even if the parent fails, without leaving an orphaned helper stuck.
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_file(&ready_path);

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

    // Closing the child's stdin makes `read_line` return EOF and the helper exit.
    // `Stdio::piped()` is a test invariant: if it went away, `wait()` would hang
    // forever, so fail loudly instead of hanging the suite.
    let stdin = child
        .stdin
        .take()
        .expect("o filho deve ter stdin encanado para o release");
    drop(stdin);
    assert!(
        child.wait().unwrap().success(),
        "o processo auxiliar deve sair com sucesso depois do release"
    );
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_private_no_store(&response);
}

#[test]
fn rescan_lock_helper() {
    // Dedicated marker: only the helper process spawned by this file sets it.
    // Without it, the test never runs the body, avoiding a hang in `read_line` when
    // run inside the main suite (a regression the deadline poll used to contain).
    if std::env::var("LEARNVAULT_RESCAN_LOCK_HELPER").as_deref() != Ok("1") {
        return;
    }
    let lock_path = std::env::var("LEARNVAULT_RESCAN_LOCK_PATH").unwrap();
    let ready_path = std::env::var("LEARNVAULT_RESCAN_LOCK_READY").unwrap();
    let _lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .unwrap();
    _lock.lock().unwrap();
    fs::write(ready_path, []).unwrap();

    // Holds the lock until the parent signals the release through the stdin channel. A
    // blocking `read_line` (or EOF) replaces polling with a deadline: the child
    // never `sleep`s and never stays stuck beyond the parent closing.
    let mut release = String::new();
    let _ = std::io::stdin().read_line(&mut release);
}

/// Waits for a synchronization file between processes with a generous deadline:
/// on loaded CI, the helper process can take much longer than a few
/// seconds to be spawned and signal `lock-ready`.
fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + RESCAN_LOCK_TIMEOUT;
    while !path.exists() {
        assert!(Instant::now() < deadline, "timed out waiting for {path:?}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn find_file(value: &serde_json::Value) -> Option<&serde_json::Value> {
    if value.get("name").and_then(serde_json::Value::as_str) == Some("lesson.mp4") {
        return Some(value);
    }
    for key in ["files", "loose_files", "children"] {
        if let Some(values) = value.get(key).and_then(serde_json::Value::as_array) {
            for value in values {
                if let Some(file) = find_file(value) {
                    return Some(file);
                }
            }
        }
    }
    None
}

#[tokio::test]
async fn catalog_revalidates_by_etag_and_answers_not_modified() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;

    let first = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_private_no_cache(&first);
    let etag = first.headers()["etag"].to_str().unwrap().to_owned();
    let body = first.into_body().collect().await.unwrap().to_bytes();
    assert!(!body.is_empty());

    let revalidated = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", &cookie)
                .header("if-none-match", &etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED);
    assert_private_no_cache(&revalidated);
    assert_eq!(revalidated.headers()["etag"], etag.as_str());
    let revalidated_body = revalidated.into_body().collect().await.unwrap().to_bytes();
    assert!(revalidated_body.is_empty(), "um 304 não carrega corpo");

    let changed = app
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", &cookie)
                .header("if-none-match", "\"stale\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_rescan_keeps_healthz_responsive_across_multiple_courses() {
    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    for i in 0..10 {
        let dir = library.join(format!("Course {i:02}")).join("01 Intro");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("01.mp4"), b"video").unwrap();
    }
    let mut cfg = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        library.to_str().unwrap(),
    ])
    .unwrap();
    cfg.admin_username = Some("admin".to_owned());
    cfg.admin_password = Some("uma senha local muito segura".to_owned());
    cfg.cookie_secure = false;

    let app = build_router(AppState::initialize(cfg).await.unwrap());
    let cookie = login(&app).await;

    let rescan_fut = app.clone().oneshot(
        Request::post("/api/admin/catalog/rescan")
            .header("origin", "http://localhost")
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap(),
    );
    tokio::pin!(rescan_fut);

    let mut healthz_ok_during_rescan = 0usize;
    let rescan = loop {
        tokio::select! {
            res = &mut rescan_fut => break res.unwrap(),
            health = app.clone().oneshot(Request::get("/api/healthz").body(Body::empty()).unwrap()) => {
                if health.unwrap().status() == StatusCode::OK {
                    healthz_ok_during_rescan += 1;
                }
                tokio::task::yield_now().await;
            }
        }
    };

    assert_eq!(rescan.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&rescan.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["courses"], 10);
    assert_eq!(body["failed_courses"], 0);
    assert_eq!(body["complete"], true);
    assert!(
        healthz_ok_during_rescan >= 1,
        "expected /api/healthz to succeed while multi-course rescan ran"
    );
}

#[tokio::test]
async fn rescan_discovers_new_library_folders_and_imports_by_relative_name() {
    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    let first = library.join("Existing Course");
    fs::create_dir_all(&first).unwrap();
    fs::write(first.join("01 - Intro.mp4"), b"video").unwrap();

    let mut cfg = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        library.to_str().unwrap(),
    ])
    .unwrap();
    cfg.admin_username = Some("admin".to_owned());
    cfg.admin_password = Some("uma senha local muito segura".to_owned());
    cfg.cookie_secure = false;

    let app = build_router(AppState::initialize(cfg).await.unwrap());
    let cookie = login(&app).await;

    let newcomer = library.join("02 - Fresh Rust");
    fs::create_dir_all(&newcomer).unwrap();
    fs::write(newcomer.join("01 - Basics.mp4"), b"video").unwrap();

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
    let body: serde_json::Value =
        serde_json::from_slice(&rescan.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let discovered = body["discovered"].as_array().unwrap();
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0]["folder"], "02 - Fresh Rust");
    assert_eq!(discovered[0]["title"], "Fresh Rust");

    let import = app
        .clone()
        .oneshot(
            Request::post("/api/admin/courses")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"folder":"02 - Fresh Rust"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import.status(), StatusCode::OK);

    // Traversal outside --library-root via `folder` is rejected.
    let escape = app
        .oneshot(
            Request::post("/api/admin/courses")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"folder":"../outside"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(escape.status(), StatusCode::BAD_REQUEST);
}
