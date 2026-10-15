//! Guarda de construção do crate desktop.
//!
//! Estes testes não sobem o webview nem dependem de display: eles afirmam, por
//! construção, que a fundação do desktop embute o frontend em produção, mantém o ACL
//! em deny-by-default e não introduz listener de rede, servidor HTTP nem plugin
//! privilegiado. O alvo é detectar regressões de configuração em CI, não exercitar a
//! UI do Tauri.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_SOURCE_TOKENS: [&str; 13] = [
    "TcpListener",
    "TcpStream",
    "UdpSocket",
    "std::net",
    "tokio::net",
    "HttpServer",
    "reqwest",
    "hyper",
    "axum",
    "tauri_plugin_http",
    "tauri_plugin_shell",
    "tauri_plugin_localhost",
    "tauri_plugin_fs",
];

const FORBIDDEN_MANIFEST_DEPS: [&str; 8] = [
    "tauri-plugin-http",
    "tauri-plugin-shell",
    "tauri-plugin-fs",
    "tauri-plugin-localhost",
    "tauri-plugin-updater",
    "tokio",
    "reqwest",
    "axum",
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("não foi possível ler {}: {error}", path.display()))
}

fn rust_sources() -> Vec<PathBuf> {
    let root = crate_root();
    let mut sources = vec![root.join("build.rs")];
    let mut entries = fs::read_dir(root.join("src"))
        .expect("o crate desktop precisa manter o diretório src/")
        .map(|entry| entry.expect("entrada de src/ ilegível").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect::<Vec<_>>();
    entries.sort();
    sources.append(&mut entries);
    sources
}

#[test]
fn config_embeds_frontend_for_production_without_dev_server() {
    let raw = read(&crate_root().join("tauri.conf.json"));
    let config: serde_json::Value =
        serde_json::from_str(&raw).expect("tauri.conf.json precisa ser JSON válido");

    assert_eq!(
        config["build"]["frontendDist"].as_str(),
        Some("frontend-dist"),
        "o frontend precisa ser embutido a partir de frontend-dist"
    );
    assert!(
        config["build"].get("devUrl").is_none(),
        "devUrl não é necessário e não deve abrir porta em dev"
    );

    let plugins_empty = config
        .get("plugins")
        .and_then(|value| value.as_object())
        .map(|map| map.is_empty())
        .unwrap_or(true);
    assert!(plugins_empty, "nenhum plugin Tauri deve ser configurado");

    let csp = config["app"]["security"]["csp"]
        .as_str()
        .expect("a CSP precisa ser explícita");
    assert!(
        csp.contains("connect-src ipc: http://ipc.localhost"),
        "a CSP precisa liberar o IPC do Tauri: {csp}"
    );
    assert!(
        csp.contains("object-src 'none'"),
        "a CSP precisa bloquear object/embed: {csp}"
    );
    assert!(
        !csp.contains("unsafe-eval"),
        "a CSP não pode liberar unsafe-eval: {csp}"
    );
    assert!(
        csp.contains("asset: http://asset.localhost"),
        "a CSP precisa liberar o protocolo de asset para mídia: {csp}"
    );
    assert!(
        csp.contains("blob:"),
        "a CSP precisa liberar blob: para mídia: {csp}"
    );

    let window = &config["app"]["windows"][0];
    assert_eq!(
        window["label"].as_str(),
        Some("main"),
        "a janela principal precisa ter o rótulo main"
    );
    assert_eq!(
        window["url"].as_str(),
        Some("200.html"),
        "a janela precisa carregar o shell embutido"
    );
}

#[test]
fn capability_is_deny_by_default_without_remote_sources() {
    let raw = read(&crate_root().join("capabilities/default.json"));
    let capability: serde_json::Value =
        serde_json::from_str(&raw).expect("a capability precisa ser JSON válido");

    assert!(
        capability.get("remote").is_none(),
        "capabilities remotas não são permitidas"
    );
    assert!(
        capability["identifier"].as_str().is_some(),
        "a capability precisa de um identificador"
    );

    let windows = capability["windows"]
        .as_array()
        .expect("a capability precisa listar janelas");
    assert!(
        windows.iter().any(|window| window == "main"),
        "a capability precisa cobrir a janela main"
    );

    let permissions = capability["permissions"]
        .as_array()
        .expect("a capability precisa listar permissões");
    assert!(
        !permissions.is_empty(),
        "a capability precisa conceder ao menos o conjunto padrão do núcleo"
    );
    for permission in permissions {
        let identifier = permission.as_str().unwrap_or_default();
        assert_ne!(identifier, "*", "curinga de permissão é proibido");
        for forbidden in [
            "shell", "process", "http", "updater", "fs:", ":all", "scope",
        ] {
            assert!(
                !identifier.contains(forbidden),
                "permissão privilegiada '{identifier}' é proibida no desktop"
            );
        }
    }
}

#[test]
fn desktop_source_has_no_network_listener_or_http_server() {
    for source in rust_sources() {
        let content = read(&source);
        for token in FORBIDDEN_SOURCE_TOKENS {
            assert!(
                !content.contains(token),
                "{} não pode conter '{token}'",
                source.display()
            );
        }
    }
}

#[test]
fn manifest_declares_no_privileged_plugin_or_network_stack() {
    let manifest = read(&crate_root().join("Cargo.toml"));
    for line in manifest.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        for dependency in FORBIDDEN_MANIFEST_DEPS {
            assert!(
                !trimmed.starts_with(dependency),
                "a dependência '{dependency}' não pertence ao desktop"
            );
        }
    }
}
