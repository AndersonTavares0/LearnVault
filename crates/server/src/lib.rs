//! Servidor HTTP do LearnVault.

#![forbid(unsafe_code)]

use std::io::Write;

mod auth;
mod catalog;
mod config;
mod courses;
mod health;
mod media;
mod spa;
mod state;
mod stream;
mod study;

pub use auth::{
    AuthError, generate_session_token, hash_password, hash_session_token,
    validate_current_password, validate_password, validate_username, verify_password,
};
pub use config::{Command, Config, ConfigError, parse_command};
pub use learnvault_core::{LegacyImportOptions, LegacyImportReport, import_go_database};
pub use state::{AppState, StateError};

use std::{future::Future, time::Duration};

use axum::{
    BoxError, Router,
    error_handling::HandleErrorLayer,
    extract::{DefaultBodyLimit, Request},
    http::{
        StatusCode,
        header::{CACHE_CONTROL, HeaderName, HeaderValue},
    },
    middleware::{Next, from_fn, map_response},
    response::Response,
    routing::{get, post},
};
use tokio::net::TcpListener;
use tower::ServiceBuilder;

/// Identidade de versão do servidor: o Release Please é o dono da versão de release;
/// o `0.0.0` do manifesto nunca deve ser publicado, e builds empacotados injetam a versão.
pub const VERSION: &str = match option_env!("LEARNVAULT_BUILD_VERSION") {
    Some(version) if !version.is_empty() => version,
    _ => env!("CARGO_PKG_VERSION"),
};

/// Monta as rotas públicas mínimas do servidor.
pub fn build_router(state: AppState) -> Router {
    let inline_script_hashes = state.inline_script_hashes.clone();
    let inline_style_hashes = state.inline_style_hashes.clone();
    Router::new()
        .route("/api/healthz", get(health::healthz))
        .route("/api/login", axum::routing::post(auth::login))
        .route("/api/me", get(auth::me))
        .route("/api/logout", axum::routing::post(auth::logout))
        .route(
            "/api/account/credentials",
            axum::routing::post(auth::credentials),
        )
        .route("/api/catalog", get(catalog::catalog))
        .route("/api/admin/catalog/rescan", post(catalog::rescan))
        .route("/api/admin/courses", post(courses::import))
        .route(
            "/api/admin/courses/{id}",
            axum::routing::delete(courses::remove),
        )
        .route("/api/admin/media/{id}/impact", get(media::impact))
        .route(
            "/api/admin/media/{id}",
            axum::routing::delete(media::remove_media),
        )
        .route("/api/media", get(media::media).head(media::media))
        .route("/api/study/recent", get(study::recent))
        .route("/api/study/progress", get(study::progress))
        .route("/api/study/position", post(study::record_position))
        .route("/api/study/completed", post(study::set_completed))
        .route("/api/study/capabilities", get(study::capabilities))
        .fallback(spa::serve)
        .layer(DefaultBodyLimit::max(8 * 1024))
        .layer(map_response(move |response| {
            security_headers(
                response,
                inline_script_hashes.clone(),
                inline_style_hashes.clone(),
            )
        }))
        .layer(from_fn(api_cache_headers))
        .with_state(state)
}

fn with_request_limits(router: Router, timeout: Duration) -> Router {
    router.layer(
        ServiceBuilder::new()
            .layer(from_fn(api_cache_headers))
            .layer(HandleErrorLayer::new(|_: BoxError| async {
                (StatusCode::REQUEST_TIMEOUT, "request timeout")
            }))
            .timeout(timeout),
    )
}

/// Garante que nenhuma resposta `/api` possa ser armazenada, incluindo as
/// rejeições produzidas antes do handler (`Json` e `DefaultBodyLimit`) e as
/// respostas do healthz ou de rotas desconhecidas. Um handler que já escolheu
/// uma política mais restritiva (`private, no-store`) é preservado.
async fn api_cache_headers(request: Request, next: Next) -> Response {
    let is_api = request.uri().path() == "/api" || request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    if is_api && !response.headers().contains_key(CACHE_CONTROL) {
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

async fn security_headers(
    mut response: Response,
    inline_script_hashes: std::sync::Arc<[String]>,
    inline_style_hashes: std::sync::Arc<[String]>,
) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    let mut csp = "default-src 'self'; script-src 'self'".to_owned();
    for hash in inline_script_hashes.iter() {
        csp.push_str(" '");
        csp.push_str(hash);
        csp.push('\'');
    }
    csp.push_str("; style-src 'self' 'unsafe-hashes'");
    for hash in inline_style_hashes.iter() {
        csp.push_str(" '");
        csp.push_str(hash);
        csp.push('\'');
    }
    csp.push_str("; media-src 'self' data: blob:; frame-ancestors 'none'");
    // Os hashes são base64 por construção, então a conversão é infalível na
    // prática; ainda assim, prefira responder sem CSP a travar o worker.
    if let Ok(csp) = csp.parse() {
        headers.insert(HeaderName::from_static("content-security-policy"), csp);
    }
    response
}

/// Erros públicos do ciclo de vida do servidor.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    /// O listener HTTP não pôde ser criado.
    #[error("could not bind HTTP listener")]
    Bind(#[source] std::io::Error),
    /// O servidor HTTP terminou com erro.
    #[error("HTTP server failed")]
    Serve(#[source] std::io::Error),
    /// A autenticação não pôde ser inicializada com segurança.
    #[error("server startup failed: {0}")]
    Startup(String),
}

/// Inicia o servidor e aguarda o futuro de encerramento fornecido pelo processo.
pub async fn serve<F>(config: Config, shutdown: F) -> Result<(), ServeError>
where
    F: Future<Output = ()> + Send + 'static,
{
    let ephemeral_listener = config.listen_addr.port() == 0;
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .map_err(ServeError::Bind)?;
    let state = AppState::initialize(config)
        .await
        .map_err(|error| ServeError::Startup(error.to_string()))?;
    let app = with_request_limits(build_router(state), Duration::from_secs(30));
    // Stable machine-readable handoff for process-level smoke tests. It is emitted only for
    // an explicitly ephemeral listener, after the OS assigned the address and the application
    // is ready to serve requests; normal fixed-port launches keep their existing output.
    if ephemeral_listener {
        let local_addr = listener.local_addr().map_err(ServeError::Bind)?;
        println!("LEARNVAULT_READY {local_addr}");
        let _ = std::io::stdout().flush();
    }

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
    .map_err(ServeError::Serve)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::{fs, time::Duration};

    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use http_body_util::BodyExt;
    use tempfile::TempDir;
    use tokio::sync::oneshot;
    use tower::ServiceExt;

    use super::{
        AppState, Command, Config, VERSION, build_router, parse_command, serve, with_request_limits,
    };

    async fn media_app() -> (axum::Router, tempfile::TempDir, AppState) {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("lesson.mp4"), b"0123456789").unwrap();
        fs::write(temp.path().join("slow.mp4"), vec![b'x'; 16 * 1024]).unwrap();
        let mut config = config_fixture(&temp);
        config.database_path = temp.path().join("media.sqlite");
        let state = AppState::initialize(config).await.unwrap();
        let outcome = learnvault_core::scan(temp.path()).unwrap();
        {
            let mut database = state.database.try_lock().unwrap();
            learnvault_core::apply_scan(
                &mut database,
                "library-1",
                &outcome,
                learnvault_core::KeyMode::for_host(),
            )
            .unwrap();
        }
        (build_router(state.clone()), temp, state)
    }

    async fn media_cookie(app: &axum::Router) -> String {
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

    fn config_fixture(temp: &TempDir) -> Config {
        let mut config = Config::from_args([
            "learnvault-server",
            "--listen-addr",
            "127.0.0.1:18090",
            "--database",
            temp.path().join("learnvault.sqlite").to_str().unwrap(),
            "--library-root",
            temp.path().to_str().unwrap(),
        ])
        .expect("configuração de teste válida");
        config.admin_username = Some("admin".to_owned());
        config.admin_password = Some("uma senha local muito segura".to_owned());
        config.cookie_secure = false;
        config
    }

    #[test]
    fn version_identity_uses_injected_value_or_manifest_version() {
        assert!(!VERSION.is_empty());
        let expected = match option_env!("LEARNVAULT_BUILD_VERSION") {
            Some(version) if !version.is_empty() => version,
            _ => env!("CARGO_PKG_VERSION"),
        };
        assert_eq!(VERSION, expected);
    }

    #[tokio::test]
    async fn rescan_returns_conflict_while_the_local_lock_is_held() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::initialize(config_fixture(&temp)).await.unwrap();
        let app = build_router(state.clone());
        let cookie = media_cookie(&app).await;
        let _local_lock = state.rescan_lock.lock_owned().await;

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

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );
    }

    #[tokio::test]
    async fn healthz_is_public_and_returns_status_and_version() {
        let temp = tempfile::tempdir().unwrap();
        let app = build_router(AppState::new(config_fixture(&temp)));

        let response = app
            .oneshot(Request::get("/api/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["version"], VERSION);
    }

    #[tokio::test]
    async fn healthz_reports_unavailable_without_database_details() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::new(config_fixture(&temp));
        let app = build_router(state.clone());
        let _database = state.database.clone().lock_owned().await;

        let response = app
            .oneshot(Request::get("/api/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json, serde_json::json!({"status": "unavailable"}));
    }

    #[tokio::test]
    async fn healthz_reports_unavailable_when_database_check_capacity_is_busy() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::new(config_fixture(&temp));
        let app = build_router(state.clone());
        let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();

        let response = app
            .oneshot(Request::get("/api/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }

    #[tokio::test]
    async fn healthz_rejects_missing_old_newer_and_incompatible_schemas() {
        for (name, setup) in [
            ("missing", "PRAGMA user_version = 0"),
            ("older", "PRAGMA user_version = 3"),
            ("newer", "PRAGMA user_version = 5"),
            (
                "missing required column",
                "ALTER TABLE user RENAME COLUMN must_change_credentials TO changed",
            ),
            ("missing required table", "DROP TABLE session"),
            (
                "schema mismatch",
                "DROP TABLE library; CREATE TABLE library (library_id TEXT PRIMARY KEY, available INTEGER NOT NULL)",
            ),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let state = AppState::new(config_fixture(&temp));
            {
                let database = state.database.lock().await;
                database.connection().execute_batch(setup).unwrap();
            }
            let app = build_router(state);

            let response = app
                .oneshot(Request::get("/api/healthz").body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{name}");
            assert_eq!(response.headers()["cache-control"], "no-store", "{name}");
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json, serde_json::json!({"status": "unavailable"}), "{name}");
        }
    }

    #[tokio::test]
    async fn healthz_rejects_adversarial_schema_definitions() {
        let cases = [
            (
                "comment spoof",
                "DROP TABLE library; CREATE TABLE library (library_id TEXT PRIMARY KEY NOT NULL, available INTEGER NOT NULL /* CHECK (available IN (0, 1)) */)",
            ),
            (
                "case-altered check literal",
                "DROP TABLE media; CREATE TABLE media (media_id TEXT PRIMARY KEY NOT NULL, library_id TEXT NOT NULL REFERENCES library (library_id) ON DELETE NO ACTION, relative_path TEXT NOT NULL, relative_path_key TEXT NOT NULL, size INTEGER NOT NULL, mtime INTEGER NOT NULL, availability TEXT NOT NULL CHECK (availability IN ('AVAILABLE', 'missing')), UNIQUE (library_id, relative_path_key))",
            ),
            (
                "wrong foreign key action",
                "DROP TABLE media; CREATE TABLE media (media_id TEXT PRIMARY KEY NOT NULL, library_id TEXT NOT NULL REFERENCES library (library_id) ON DELETE CASCADE, relative_path TEXT NOT NULL, relative_path_key TEXT NOT NULL, size INTEGER NOT NULL, mtime INTEGER NOT NULL, availability TEXT NOT NULL CHECK (availability IN ('available', 'missing')), UNIQUE (library_id, relative_path_key))",
            ),
            (
                "wrong username unique columns",
                "DROP TABLE user; CREATE TABLE user (user_id TEXT PRIMARY KEY NOT NULL, username TEXT NOT NULL, password_hash TEXT NOT NULL UNIQUE, is_admin INTEGER NOT NULL CHECK (is_admin IN (0, 1)), created_at INTEGER NOT NULL, must_change_credentials INTEGER NOT NULL DEFAULT 0 CHECK (must_change_credentials IN (0, 1)))",
            ),
            (
                "wrong session index columns",
                "DROP INDEX session_expires_at_idx; CREATE INDEX session_expires_at_idx ON session (user_id)",
            ),
            (
                "wrong primary key order",
                "DROP TABLE study_state; CREATE TABLE study_state (owner_id TEXT NOT NULL, media_id TEXT NOT NULL REFERENCES media (media_id) ON DELETE NO ACTION, position_seconds INTEGER NOT NULL DEFAULT 0, completed INTEGER NOT NULL CHECK (completed IN (0, 1)), updated_at INTEGER NOT NULL, PRIMARY KEY (media_id, owner_id))",
            ),
            (
                "generated column",
                "ALTER TABLE user ADD COLUMN generated TEXT GENERATED ALWAYS AS (username) VIRTUAL",
            ),
            (
                "without rowid",
                "DROP TABLE library; CREATE TABLE library (library_id TEXT PRIMARY KEY NOT NULL, available INTEGER NOT NULL CHECK (available IN (0, 1))) WITHOUT ROWID",
            ),
            (
                "strict table",
                "DROP TABLE library; CREATE TABLE library (library_id TEXT PRIMARY KEY NOT NULL, available INTEGER NOT NULL CHECK (available IN (0, 1))) STRICT",
            ),
        ];
        for (name, setup) in cases {
            let temp = tempfile::tempdir().unwrap();
            let state = AppState::new(config_fixture(&temp));
            {
                let database = state.database.lock().await;
                database.connection().execute_batch(setup).unwrap();
            }
            let response = build_router(state)
                .oneshot(Request::get("/api/healthz").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{name}");
            assert_eq!(response.headers()["cache-control"], "no-store", "{name}");
        }
    }

    #[tokio::test]
    async fn healthz_rejects_schema_sql_over_byte_bound() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::new(config_fixture(&temp));
        let comment = "é".repeat(40_000);
        let setup = format!(
            "DROP TABLE library; CREATE TABLE library (library_id TEXT PRIMARY KEY NOT NULL, available INTEGER NOT NULL CHECK (available IN (0, 1)) /*{comment}*/)",
        );
        {
            let database = state.database.lock().await;
            database.connection().execute_batch(&setup).unwrap();
        }

        let response = build_router(state)
            .oneshot(Request::get("/api/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }

    #[tokio::test]
    async fn api_timeout_rejection_gets_no_store_cache_header() {
        let temp = tempfile::tempdir().unwrap();
        let router = build_router(AppState::new(config_fixture(&temp))).route(
            "/api/slow",
            get(|| async { std::future::pending::<()>().await }),
        );
        let response = with_request_limits(router, Duration::from_millis(1))
            .oneshot(Request::get("/api/slow").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }

    #[tokio::test]
    async fn media_authenticates_before_validating_a_missing_query() {
        let temp = tempfile::tempdir().unwrap();
        let app = build_router(AppState::new(config_fixture(&temp)));
        let response = app
            .oneshot(Request::get("/api/media").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );
    }

    #[test]
    fn invalid_listen_address_is_rejected_before_startup() {
        let error = Config::from_args([
            "learnvault-server",
            "--listen-addr",
            "not-an-address",
            "--database",
            "data/learnvault.sqlite",
            "--library-root",
            ".",
        ])
        .unwrap_err();

        assert!(error.to_string().contains("listen address"));
        assert!(!error.to_string().contains("not-an-address"));
    }

    #[test]
    fn invalid_library_root_is_rejected_without_exposing_the_path() {
        let secret_path = PathBuf::from("C:/private/course-library");
        let error = Config::from_args([
            "learnvault-server",
            "--database",
            "data/learnvault.sqlite",
            "--library-root",
            secret_path.to_str().unwrap(),
        ])
        .unwrap_err();

        assert!(error.to_string().contains("library root"));
        assert!(!error.to_string().contains("private"));
    }

    #[test]
    fn version_flag_is_parsed_without_starting_the_server() {
        assert_eq!(
            parse_command(["learnvault-server", "--version"]).unwrap(),
            Command::Version
        );
    }

    #[test]
    fn unknown_arguments_are_rejected_safely() {
        let error = parse_command(["learnvault-server", "--secret", "value"]).unwrap_err();

        assert!(error.to_string().contains("unknown argument"));
        assert!(!error.to_string().contains("value"));
    }

    #[tokio::test]
    async fn server_stops_after_the_shutdown_signal() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = config_fixture(&temp);
        config.listen_addr = "127.0.0.1:0".parse().unwrap();
        let (sender, receiver) = oneshot::channel();

        let task = tokio::spawn(serve(config, async {
            let _ = receiver.await;
        }));
        sender.send(()).unwrap();

        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn media_requires_authentication_and_supports_get_head_and_ranges() {
        let (app, _temp, _state) = media_app().await;
        let unauthenticated = app
            .clone()
            .oneshot(
                Request::get("/api/media?course=library-1&path=lesson.mp4")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

        let cookie = media_cookie(&app).await;
        let full = app
            .clone()
            .oneshot(
                Request::get("/api/media?course=library-1&path=lesson.mp4")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(full.status(), StatusCode::OK);
        assert_eq!(full.headers()["content-type"], "video/mp4");
        assert_eq!(full.headers()["accept-ranges"], "bytes");
        assert_eq!(
            full.into_body().collect().await.unwrap().to_bytes(),
            "0123456789"
        );

        let head = app
            .clone()
            .oneshot(
                Request::head("/api/media?course=library-1&path=lesson.mp4")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()["content-length"], "10");
        assert!(
            head.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .is_empty()
        );

        let partial = app
            .clone()
            .oneshot(
                Request::get("/api/media?course=library-1&path=lesson.mp4")
                    .header("cookie", &cookie)
                    .header("range", "bytes=2-5")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(partial.headers()["content-range"], "bytes 2-5/10");
        assert_eq!(
            partial.into_body().collect().await.unwrap().to_bytes(),
            "2345"
        );
    }

    #[tokio::test]
    async fn media_resolves_the_course_requested_when_relative_paths_collide() {
        let temp = tempfile::tempdir().unwrap();
        let configured_root = temp.path().join("configured");
        let other_root = temp.path().join("other");
        fs::create_dir_all(&configured_root).unwrap();
        fs::create_dir_all(&other_root).unwrap();
        fs::write(configured_root.join("lesson.mp4"), b"configured").unwrap();
        fs::write(other_root.join("lesson.mp4"), b"other").unwrap();

        let mut config = Config::from_args([
            "learnvault-server",
            "--database",
            temp.path().join("media.sqlite").to_str().unwrap(),
            "--library-root",
            configured_root.to_str().unwrap(),
        ])
        .unwrap();
        config.admin_username = Some("admin".to_owned());
        config.admin_password = Some("uma senha local muito segura".to_owned());
        config.cookie_secure = false;
        let state = AppState::initialize(config).await.unwrap();
        // O segundo curso entra no registro como a interface faria; as duas pastas têm o
        // mesmo caminho relativo e o parâmetro `course` decide qual mídia é servida.
        let other_id = learnvault_core::course_id_from_path(&other_root.canonicalize().unwrap());
        state
            .courses
            .write()
            .unwrap()
            .push(super::courses::RegisteredCourse {
                id: other_id.clone(),
                title: "Other".to_owned(),
                root: learnvault_core::CourseRoot::resolve(&other_root).unwrap(),
            });
        {
            let mut database = state.database.try_lock().unwrap();
            learnvault_core::apply_scan(
                &mut database,
                &other_id,
                &learnvault_core::scan(&other_root).unwrap(),
                learnvault_core::KeyMode::for_host(),
            )
            .unwrap();
            learnvault_core::apply_scan(
                &mut database,
                "library-1",
                &learnvault_core::scan(&configured_root).unwrap(),
                learnvault_core::KeyMode::for_host(),
            )
            .unwrap();
        }
        let app = build_router(state);
        let cookie = media_cookie(&app).await;
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/media?course=library-1&path=lesson.mp4")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            "configured"
        );

        let other = app
            .oneshot(
                Request::get(format!("/api/media?course={other_id}&path=lesson.mp4"))
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(other.status(), StatusCode::OK);
        assert_eq!(
            other.into_body().collect().await.unwrap().to_bytes(),
            "other"
        );
    }

    #[tokio::test]
    async fn media_rejects_unsatisfiable_ranges_and_path_traversal() {
        let (app, _temp, _state) = media_app().await;
        let cookie = media_cookie(&app).await;
        let range = app
            .clone()
            .oneshot(
                Request::get("/api/media?course=library-1&path=lesson.mp4")
                    .header("cookie", &cookie)
                    .header("range", "bytes=99-")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(range.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(range.headers()["content-range"], "bytes */10");
        assert_eq!(
            range
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );

        let traversal = app
            .oneshot(
                Request::get("/api/media?course=library-1&path=../lesson.mp4")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(traversal.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            traversal
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );
    }

    #[tokio::test]
    async fn media_reports_a_corrupt_stored_size_as_a_sanitized_internal_error() {
        let (app, _temp, state) = media_app().await;
        let cookie = media_cookie(&app).await;
        // Um tamanho negativo é corrupção de catálogo: a resolução precisa falhar
        // alto (500) em vez de tratar a linha como arquivo vazio.
        state
            .database
            .try_lock()
            .unwrap()
            .connection()
            .execute("UPDATE media SET size = -1", [])
            .unwrap();

        let response = app
            .oneshot(
                Request::get("/api/media?course=library-1&path=lesson.mp4")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(body.is_empty(), "erros sanitizados não expõem detalhes");
    }

    #[tokio::test]
    async fn media_rejects_requests_when_blocking_capacity_is_saturated() {
        let (app, _temp, state) = media_app().await;
        let cookie = media_cookie(&app).await;
        let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();

        let response = app
            .oneshot(
                Request::get("/api/media?course=library-1&path=lesson.mp4")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error"],
            "server busy"
        );
    }

    #[tokio::test]
    async fn media_stream_capacity_is_held_until_response_bodies_are_dropped() {
        let (app, _temp, _state) = media_app().await;
        let cookie = media_cookie(&app).await;
        let mut slow_responses = Vec::new();

        for _ in 0..4 {
            let response = app
                .clone()
                .oneshot(
                    Request::get("/api/media?course=library-1&path=slow.mp4")
                        .header("cookie", &cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            slow_responses.push(response);
        }

        let saturated = app
            .clone()
            .oneshot(
                Request::get("/api/media?course=library-1&path=slow.mp4")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(saturated.status(), StatusCode::SERVICE_UNAVAILABLE);

        drop(slow_responses);
        let recovered = app
            .oneshot(
                Request::get("/api/media?course=library-1&path=slow.mp4")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(recovered.status(), StatusCode::OK);
    }

    #[tokio::test(start_paused = true)]
    async fn idle_media_streams_release_capacity_without_reading_the_body() {
        let (app, _temp, _state) = media_app().await;
        let cookie = media_cookie(&app).await;
        let mut idle_responses = Vec::new();

        for _ in 0..4 {
            let response = app
                .clone()
                .oneshot(
                    Request::get("/api/media?course=library-1&path=slow.mp4")
                        .header("cookie", &cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            idle_responses.push(response);
        }

        let saturated = app
            .clone()
            .oneshot(
                Request::get("/api/media?course=library-1&path=slow.mp4")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(saturated.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Avança em passos de 1s para que os produtores sejam escalonados e enfileirem
        // o primeiro chunk antes do deadline de 30s; um único yield + advance(30s)
        // é racy sob carga (os produtores podem expirar sem nunca escrever).
        for _ in 0..30 {
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_secs(1)).await;
        }
        tokio::task::yield_now().await;

        let recovered = app
            .oneshot(
                Request::get("/api/media?course=library-1&path=slow.mp4")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(recovered.status(), StatusCode::OK);

        let mut timed_out = idle_responses.pop().unwrap();
        // O primeiro chunk já lido é sempre entregue (o envio tem prioridade sobre o
        // deadline expirado). Ao retirá-lo, o consumidor libera espaço e o produtor pode
        // enfileirar o chunk seguinte antes de observar o fim do arquivo — comportamento
        // previsto: o deadline mede ociosidade do transporte, não descarta dados lidos.
        // Por isso drenamos até o encerramento em vez de exigir um único frame.
        assert!(timed_out.body_mut().frame().await.unwrap().is_ok());
        while let Some(frame) = timed_out.body_mut().frame().await {
            assert!(
                frame.is_ok(),
                "idle stream delivered an error frame: {frame:?}"
            );
        }
        drop(idle_responses);
    }
}
