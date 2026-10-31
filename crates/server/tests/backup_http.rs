use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use learnvault_core::Database;
use learnvault_server::{
    AppState, BackupOptions, Command, Config, RestoreOptions, build_router, execute_backup,
    execute_restore, parse_command,
};
use tempfile::TempDir;
use tower::ServiceExt;

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("learnvault.sqlite").to_str().unwrap(),
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
async fn backup_command_is_parsed_from_cli_args() {
    let cmd = parse_command([
        "learnvault-server",
        "backup",
        "--database",
        "custom/path.sqlite",
        "--output",
        "backups/my-backup.sqlite",
    ])
    .unwrap();

    assert_eq!(
        cmd,
        Command::Backup(BackupOptions {
            database_path: "custom/path.sqlite".into(),
            output_path: "backups/my-backup.sqlite".into(),
        })
    );
}

#[tokio::test]
async fn restore_command_is_parsed_from_cli_args() {
    let cmd = parse_command([
        "learnvault-server",
        "restore",
        "--backup",
        "backups/my-backup.sqlite",
        "--database",
        "custom/path.sqlite",
    ])
    .unwrap();

    assert_eq!(
        cmd,
        Command::Restore(RestoreOptions {
            backup_path: "backups/my-backup.sqlite".into(),
            database_path: "custom/path.sqlite".into(),
        })
    );
}

#[tokio::test]
async fn execute_backup_and_restore_via_cli_helpers() {
    let temp = TempDir::new().unwrap();
    let original_db = temp.path().join("original.sqlite");
    let backup_file = temp.path().join("backup.sqlite");
    let restored_db = temp.path().join("restored.sqlite");

    let db = Database::open(&original_db).unwrap();
    db.connection()
        .execute(
            "INSERT INTO library (library_id, available) VALUES ('lib-cli-test', 1)",
            [],
        )
        .unwrap();

    execute_backup(&BackupOptions {
        database_path: original_db.clone(),
        output_path: backup_file.clone(),
    })
    .unwrap();

    assert!(backup_file.is_file());

    execute_restore(&RestoreOptions {
        backup_path: backup_file,
        database_path: restored_db.clone(),
    })
    .unwrap();

    let r_db = Database::open(&restored_db).unwrap();
    let count: i64 = r_db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM library WHERE library_id = 'lib-cli-test'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn backup_endpoint_requires_admin_and_serves_valid_sqlite_stream() {
    let temp = TempDir::new().unwrap();
    let cfg = config(&temp);
    let state = AppState::initialize(cfg).await.unwrap();
    let app = build_router(state);

    // Sem autenticação -> 401
    let unauth = app
        .clone()
        .oneshot(
            Request::get("/api/admin/backup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

    // Com login admin -> 200 OK e download válido
    let cookie = login(&app).await;
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/admin/backup")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/x-sqlite3"
    );
    assert_eq!(
        response.headers().get("cache-control").unwrap(),
        "private, no-store"
    );
    let disposition = response
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(disposition.starts_with("attachment; filename=\"learnvault-backup-"));

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(body.starts_with(b"SQLite format 3\0"));
}
