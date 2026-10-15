//! Servidor HTTP do LearnVault.

#![forbid(unsafe_code)]

mod auth;
mod catalog;
mod config;
mod health;
mod media;
mod spa;
mod state;
mod stream;

pub use auth::{
    AuthError, generate_session_token, hash_password, hash_session_token, validate_password,
    validate_username, verify_password,
};
pub use config::{Command, Config, ConfigError, parse_command};
pub use state::{AppState, StateError};

use std::{future::Future, time::Duration};

use axum::{
    BoxError, Router,
    error_handling::HandleErrorLayer,
    extract::DefaultBodyLimit,
    http::{
        StatusCode,
        header::{HeaderName, HeaderValue},
    },
    middleware::map_response,
    response::Response,
    routing::{get, post},
};
use tokio::net::TcpListener;
use tower::ServiceBuilder;

/// Versão compilada do servidor, usada pelo executável e pelo endpoint de saúde.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Monta as rotas públicas mínimas do servidor.
pub fn build_router(state: AppState) -> Router {
    let inline_script_hashes = state.inline_script_hashes.clone();
    let inline_style_hashes = state.inline_style_hashes.clone();
    Router::new()
        .route("/api/healthz", get(health::healthz))
        .route("/api/login", axum::routing::post(auth::login))
        .route("/api/me", get(auth::me))
        .route("/api/logout", axum::routing::post(auth::logout))
        .route("/api/catalog", get(catalog::catalog))
        .route("/api/admin/catalog/rescan", post(catalog::rescan))
        .route("/api/media", get(media::media).head(media::media))
        .fallback(spa::serve)
        .layer(DefaultBodyLimit::max(8 * 1024))
        .layer(map_response(move |response| {
            security_headers(
                response,
                inline_script_hashes.clone(),
                inline_style_hashes.clone(),
            )
        }))
        .with_state(state)
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
    csp.push_str("; media-src 'self' data:; frame-ancestors 'none'");
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
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .map_err(ServeError::Bind)?;
    let state = AppState::initialize(config)
        .await
        .map_err(|error| ServeError::Startup(error.to_string()))?;
    let app = build_router(state).layer(
        ServiceBuilder::new()
            .layer(HandleErrorLayer::new(|_: BoxError| async {
                (StatusCode::REQUEST_TIMEOUT, "request timeout")
            }))
            .timeout(Duration::from_secs(30)),
    );

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
    };
    use http_body_util::BodyExt;
    use tempfile::TempDir;
    use tokio::sync::oneshot;
    use tower::ServiceExt;

    use super::{AppState, Command, Config, VERSION, build_router, parse_command, serve};

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
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["version"], VERSION);
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
                Request::get("/api/media?path=lesson.mp4")
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
                Request::get("/api/media?path=lesson.mp4")
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
                Request::head("/api/media?path=lesson.mp4")
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
                Request::get("/api/media?path=lesson.mp4")
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
    async fn media_rejects_unsatisfiable_ranges_and_path_traversal() {
        let (app, _temp, _state) = media_app().await;
        let cookie = media_cookie(&app).await;
        let range = app
            .clone()
            .oneshot(
                Request::get("/api/media?path=lesson.mp4")
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
                Request::get("/api/media?path=../lesson.mp4")
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
    async fn media_rejects_requests_when_blocking_capacity_is_saturated() {
        let (app, _temp, state) = media_app().await;
        let cookie = media_cookie(&app).await;
        let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();

        let response = app
            .oneshot(
                Request::get("/api/media?path=lesson.mp4")
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
                    Request::get("/api/media?path=slow.mp4")
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
                Request::get("/api/media?path=slow.mp4")
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
                Request::get("/api/media?path=slow.mp4")
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
                    Request::get("/api/media?path=slow.mp4")
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
                Request::get("/api/media?path=slow.mp4")
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
                Request::get("/api/media?path=slow.mp4")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(recovered.status(), StatusCode::OK);

        let mut timed_out = idle_responses.pop().unwrap();
        assert!(timed_out.body_mut().frame().await.unwrap().is_ok());
        assert!(timed_out.body_mut().frame().await.is_none());
        drop(idle_responses);
    }
}
