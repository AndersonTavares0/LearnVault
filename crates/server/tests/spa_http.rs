use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use http_body_util::BodyExt;
use learnvault_server::{AppState, Config, build_router};
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::TempDir;
use tower::ServiceExt;

// A representative inline bootstrap fixture used to exercise CSP hashing.
const SPA_BOOTSTRAP: &str = r#"
				{
					learnvault_bootstrap = {
						base: "",
						version: "1792074284471"
					};

					const element = document.currentScript.parentElement;

					import("/_app/immutable/entry/start.js").then(async (runtime) => {
						runtime.init(learnvault_bootstrap);
						const app = await import("/_app/immutable/entry/app.js");
						runtime.start(app, element);
					});
				}
			"#;
const CSS: &str = "body { color: rebeccapurple; }";
const ALTERNATE_BOOTSTRAP: &str = "window.learnvaultAlternateShell = true;";
const MAX_SPA_FILE_SIZE: u64 = 16 * 1024 * 1024;

fn shell() -> String {
    format!(
        "<!doctype html><html><body><div style=\"display: contents\"><script>{SPA_BOOTSTRAP}</script></div></body></html>"
    )
}

fn web_root(temp: &TempDir) -> std::path::PathBuf {
    let root = temp.path().join("web");
    fs::create_dir_all(root.join("_app/immutable/assets")).unwrap();
    fs::write(root.join("200.html"), shell()).unwrap();
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
    let style_hash = STANDARD.encode(Sha256::digest(b"display: contents"));
    assert!(csp.contains(&format!(
        "style-src 'self' 'unsafe-hashes' 'sha256-{style_hash}'"
    )));
    assert!(csp.contains("img-src 'self' data: blob:"));
    assert!(csp.contains("media-src 'self' data: blob:"));
    assert!(!csp.contains("unsafe-inline"));
    assert!(!csp.contains("unsafe-eval"));
    assert!(csp.contains("frame-ancestors 'none'"));
    assert!(!csp.contains("'unsafe-inline'"));
    assert!(!csp.contains("'unsafe-eval'"));
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        CSS
    );
}

#[tokio::test]
async fn spa_does_not_mark_assets_immutable_based_on_the_web_root_absolute_path() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("_app/immutable/web");
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("200.html"), shell()).unwrap();
    fs::write(root.join("assets/app.css"), CSS).unwrap();

    let response = app(&temp, &root)
        .await
        .oneshot(Request::get("/assets/app.css").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-cache");
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
    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    let bootstrap_hash = STANDARD.encode(Sha256::digest(SPA_BOOTSTRAP.as_bytes()));
    assert!(csp.contains(&format!("script-src 'self' 'sha256-{bootstrap_hash}'")));
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        shell().as_bytes()
    );
}

#[tokio::test]
async fn csp_hashes_the_inline_scripts_from_the_configured_shell() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    fs::write(
        root.join("200.html"),
        format!(
            "<html><body><script>{ALTERNATE_BOOTSTRAP}</script><script src=\"/app.js\"></script></body></html>"
        ),
    )
    .unwrap();
    let response = app(&temp, &root)
        .await
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    let alternate_hash = STANDARD.encode(Sha256::digest(ALTERNATE_BOOTSTRAP.as_bytes()));
    let original_hash = STANDARD.encode(Sha256::digest(SPA_BOOTSTRAP.as_bytes()));
    assert!(csp.contains(&format!("script-src 'self' 'sha256-{alternate_hash}'")));
    assert!(!csp.contains(&original_hash));
    assert!(!csp.contains("app.js"));
    assert!(!csp.contains("'unsafe-inline'"));
    assert!(!csp.contains("'unsafe-eval'"));
}

#[tokio::test]
async fn csp_hashes_all_inline_styles_from_the_configured_shell() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let first_style = "display: grid";
    let second_style = "color: rebeccapurple";
    fs::write(
        root.join("200.html"),
        format!(
            "<html><body><main style=\"{first_style}\"><span STYLE='{second_style}'></span></main><script>{ALTERNATE_BOOTSTRAP}</script></body></html>"
        ),
    )
    .unwrap();
    let response = app(&temp, &root)
        .await
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    let first_hash = STANDARD.encode(Sha256::digest(first_style.as_bytes()));
    let second_hash = STANDARD.encode(Sha256::digest(second_style.as_bytes()));
    assert!(csp.contains(&format!(
        "style-src 'self' 'unsafe-hashes' 'sha256-{first_hash}'"
    )));
    assert!(csp.contains(&format!("'sha256-{second_hash}'")));
    assert!(!csp.contains("sha256-tcbDxjMo+xKqM21aCGYbs/QAJqB7yUXC06oPWDapBgc="));
    assert!(!csp.contains("'unsafe-inline'"));
    assert!(!csp.contains("'unsafe-eval'"));
}

#[tokio::test]
async fn csp_hashes_inline_style_blocks_without_enabling_unsafe_inline() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let style = "main { display: grid; }";
    fs::write(
        root.join("200.html"),
        format!("<html><head><style>{style}</style></head><body></body></html>"),
    )
    .unwrap();
    let response = app(&temp, &root)
        .await
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    let hash = STANDARD.encode(Sha256::digest(style.as_bytes()));
    assert!(csp.contains(&format!("style-src 'self' 'unsafe-hashes' 'sha256-{hash}'")));
    assert!(!csp.contains("'unsafe-inline'"));
    assert!(!csp.contains("'unsafe-eval'"));
}

#[tokio::test]
async fn csp_does_not_truncate_scripts_at_longer_closing_tag_names() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let script = "const marker = '</scripture>'; window.learnvault = marker;";
    fs::write(
        root.join("200.html"),
        format!("<html><body><script>{script}</script></body></html>"),
    )
    .unwrap();
    let response = app(&temp, &root)
        .await
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    let hash = STANDARD.encode(Sha256::digest(script.as_bytes()));
    assert!(csp.contains(&format!("script-src 'self' 'sha256-{hash}'")));
}

#[tokio::test]
async fn changed_shell_is_not_served_with_the_startup_csp() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let app = app(&temp, &root).await;
    fs::write(
        root.join("200.html"),
        format!("<html><script>{ALTERNATE_BOOTSTRAP}</script></html>"),
    )
    .unwrap();

    let response = app
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn csp_allows_the_downloaded_canonical_vite_shell() {
    let canonical_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("crates/server/web/build");
    let shell_path = canonical_root.join("200.html");
    assert!(
        shell_path.is_file(),
        "o shell canônico da SPA deve existir antes do teste CSP"
    );
    let shell = fs::read_to_string(&shell_path).unwrap();
    let mut inline_scripts = Vec::new();
    let mut search_from = 0;
    while let Some(relative_start) = shell[search_from..].find("<script") {
        let script_start = search_from + relative_start;
        let tag_end = shell[script_start..].find('>').unwrap() + script_start;
        let attributes = &shell[script_start..tag_end];
        let content_start = tag_end + 1;
        let content_end = shell[content_start..].find("</script>").unwrap() + content_start;
        if !attributes.contains(" src=") {
            inline_scripts.push(&shell.as_bytes()[content_start..content_end]);
        }
        search_from = content_end + "</script>".len();
    }

    let temp = tempfile::tempdir().unwrap();
    let response = app(&temp, &canonical_root)
        .await
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let csp = response.headers()["content-security-policy"]
        .to_str()
        .unwrap();
    assert!(csp.contains("script-src 'self"));
    assert!(!csp.contains("script-src https://"));
    for script in inline_scripts {
        let hash = STANDARD.encode(Sha256::digest(script));
        assert!(csp.contains(&format!("'sha256-{hash}'")));
    }
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
    assert_eq!(
        missing_asset
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );

    let unknown_api = app
        .oneshot(Request::get("/api/unknown").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unknown_api.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        unknown_api
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
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
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
}

#[tokio::test]
async fn spa_rejects_requests_when_blocking_capacity_is_saturated() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let state = AppState::initialize(config(&temp, &root)).await.unwrap();
    let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();
    let response = build_router(state)
        .oneshot(
            Request::get("/_app/immutable/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn slow_spa_assets_cannot_exhaust_the_shell_stream_reservation() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let app = app(&temp, &root).await;
    let mut slow_responses = Vec::new();

    for _ in 0..3 {
        let response = app
            .clone()
            .oneshot(
                Request::get("/_app/immutable/assets/app.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        slow_responses.push(response);
    }

    let saturated_asset = app
        .clone()
        .oneshot(
            Request::get("/_app/immutable/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(saturated_asset.status(), StatusCode::SERVICE_UNAVAILABLE);

    let shell = app
        .clone()
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(shell.status(), StatusCode::OK);
    drop(shell);

    drop(slow_responses);
    let recovered = app
        .oneshot(
            Request::get("/_app/immutable/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(recovered.status(), StatusCode::OK);
}

#[tokio::test]
async fn retained_shell_body_uses_its_own_stream_reservation() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let app = app(&temp, &root).await;
    let retained_shell = app
        .clone()
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(retained_shell.status(), StatusCode::OK);

    let saturated = app
        .clone()
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(saturated.status(), StatusCode::SERVICE_UNAVAILABLE);

    drop(retained_shell);
    let recovered = app
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(recovered.status(), StatusCode::OK);
}

#[tokio::test(start_paused = true)]
async fn idle_spa_streams_release_asset_capacity_without_reading_the_body() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    fs::write(
        root.join("_app/immutable/assets/app.css"),
        vec![b'x'; 16 * 1024],
    )
    .unwrap();
    let state = AppState::initialize(config(&temp, &root)).await.unwrap();
    let app = build_router(state.clone());
    let mut idle_responses = Vec::new();

    for _ in 0..3 {
        let response = app
            .clone()
            .oneshot(
                Request::get("/_app/immutable/assets/app.css")
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
            Request::get("/_app/immutable/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(saturated.status(), StatusCode::SERVICE_UNAVAILABLE);

    // O deadline ocioso de 30s só é observado pelo produtor depois que o IO de
    // leitura real termina; avançar o relógio virtual e afirmar logo em seguida
    // era racy sob carga (issue #154: `left: 503, right: 200`, os produtores
    // podiam não ter alcançado o `select` de envio a tempo). Em vez de apenas
    // avançar o tempo, aguardamos o *evento de liberação*: cada `acquire` resolve
    // exatamente quando o watchdog devolve uma reserva — com o relógio virtual
    // avançando sozinho até o deadline enquanto o executor fica ocioso — sem que
    // ninguém leia os bodies. Aguardar as três reservas torna a saturação e a
    // recuperação determinísticas; uma saturação genuína (sem watchdog) jamais
    // resolveria o `acquire`.
    let released = [
        state.acquire_spa_asset_stream_permit().await,
        state.acquire_spa_asset_stream_permit().await,
        state.acquire_spa_asset_stream_permit().await,
    ];
    drop(released);

    let recovered = app
        .oneshot(
            Request::get("/_app/immutable/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(recovered.status(), StatusCode::OK);

    // Um stream encerrado pelo watchdog termina de forma limpa: nenhum chunk
    // pode chegar como erro de I/O. A *quantidade* de chunks entregues antes do
    // encerramento depende do agendamento do thread produtor e não é observável
    // de forma determinística, então o teste trava o contrato observável — nunca
    // um erro, e o body fecha — em vez de afirmar uma contagem que só existe por
    // acaso (era a origem do flake que derrubava a CI sob carga).
    for mut idle in idle_responses {
        while let Some(frame) = idle.body_mut().frame().await {
            assert!(
                frame.is_ok(),
                "o watchdog não pode converter ociosidade em erro de I/O"
            );
        }
    }
}

#[tokio::test]
async fn completed_spa_streams_release_capacity_when_eof_is_observed() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    fs::write(
        root.join("_app/immutable/assets/app.css"),
        vec![b'x'; 16 * 1024],
    )
    .unwrap();
    let app = app(&temp, &root).await;

    for _ in 0..3 {
        let response = app
            .clone()
            .oneshot(
                Request::get("/_app/immutable/assets/app.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .len(),
            16 * 1024
        );
    }

    let recovered = app
        .oneshot(
            Request::get("/_app/immutable/assets/app.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(recovered.status(), StatusCode::OK);
}

#[tokio::test]
async fn spa_rejects_assets_larger_than_the_explicit_limit() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let large = root.join("_app/immutable/assets/large.js");
    fs::File::create(&large)
        .unwrap()
        .set_len(MAX_SPA_FILE_SIZE + 1)
        .unwrap();
    let response = app(&temp, &root)
        .await
        .oneshot(
            Request::get("/_app/immutable/assets/large.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn spa_rejects_a_shell_larger_than_the_explicit_limit() {
    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    fs::File::create(root.join("200.html"))
        .unwrap()
        .set_len(MAX_SPA_FILE_SIZE + 1)
        .unwrap();
    let state = AppState::initialize(config(&temp, &root)).await.unwrap();
    let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();
    let response = build_router(state)
        .oneshot(Request::get("/catalog/rust").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[cfg(unix)]
#[tokio::test]
async fn spa_rejects_symlinked_assets() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let root = web_root(&temp);
    let outside = temp.path().join("outside.css");
    fs::write(&outside, CSS).unwrap();
    symlink(&outside, root.join("_app/immutable/assets/linked.css")).unwrap();
    let response = app(&temp, &root)
        .await
        .oneshot(
            Request::get("/_app/immutable/assets/linked.css")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
