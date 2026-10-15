//! Servidor HTTP do LearnVault.

#![forbid(unsafe_code)]

mod config;
mod auth;
mod health;
mod state;

pub use config::{Command, Config, ConfigError, parse_command};
pub use state::AppState;

use std::{future::Future, time::Duration};

use axum::{BoxError, Router, error_handling::HandleErrorLayer, http::StatusCode, routing::get};
use tokio::net::TcpListener;
use tower::ServiceBuilder;

/// Versão compilada do servidor, usada pelo executável e pelo endpoint de saúde.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Monta as rotas públicas mínimas do servidor.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/healthz", get(health::healthz))
        .with_state(state)
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
}

/// Inicia o servidor e aguarda o futuro de encerramento fornecido pelo processo.
pub async fn serve<F>(config: Config, shutdown: F) -> Result<(), ServeError>
where
    F: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .map_err(ServeError::Bind)?;
    let app = build_router(AppState::new(config)).layer(
        ServiceBuilder::new()
            .layer(HandleErrorLayer::new(|_: BoxError| async {
                (StatusCode::REQUEST_TIMEOUT, "request timeout")
            }))
            .timeout(Duration::from_secs(30)),
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(ServeError::Serve)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tempfile::TempDir;
    use tokio::sync::oneshot;
    use tower::ServiceExt;

    use super::{AppState, Command, Config, VERSION, build_router, parse_command, serve};

    fn config_fixture(temp: &TempDir) -> Config {
        Config::from_args([
            "learnvault-server",
            "--listen-addr",
            "127.0.0.1:18090",
            "--database",
            temp.path().join("learnvault.sqlite").to_str().unwrap(),
            "--library-root",
            temp.path().to_str().unwrap(),
        ])
        .expect("configuração de teste válida")
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
}
