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

const FORBIDDEN_MANIFEST_DEPS: [&str; 10] = [
    "tauri-plugin-http",
    "tauri-plugin-shell",
    "tauri-plugin-fs",
    "tauri-plugin-localhost",
    "tauri-plugin-updater",
    "tauri-plugin-store",
    "tauri-plugin-persisted-scope",
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
    assert!(
        csp.contains("learnvault-media: http://learnvault-media.localhost"),
        "a CSP precisa liberar o protocolo de mídia local: {csp}"
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
fn build_registers_the_app_commands_in_the_acl_manifest() {
    let build = read(&crate_root().join("build.rs"));
    assert!(
        build.contains("app_manifest"),
        "o build precisa declarar o AppManifest do ACL"
    );
    for command in [
        "desktop_config",
        "desktop_catalog",
        "desktop_select_library",
        "desktop_rescan",
        "desktop_study_progress",
        "desktop_record_position",
        "desktop_set_completed",
        "desktop_recent_study",
    ] {
        assert!(
            build.contains(command),
            "o comando '{command}' precisa ser declarado no ACL app-defined"
        );
    }
}

#[test]
fn capability_grants_only_the_core_default_and_the_app_commands() {
    let raw = read(&crate_root().join("capabilities/default.json"));
    let capability: serde_json::Value =
        serde_json::from_str(&raw).expect("a capability precisa ser JSON válido");
    let permissions: Vec<&str> = capability["permissions"]
        .as_array()
        .expect("a capability precisa listar permissões")
        .iter()
        .map(|permission| permission.as_str().unwrap_or_default())
        .collect();

    assert!(permissions.contains(&"core:default"));
    assert!(permissions.contains(&"allow-desktop-config"));
    assert!(permissions.contains(&"allow-desktop-catalog"));
    assert!(permissions.contains(&"allow-desktop-select-library"));
    assert!(permissions.contains(&"allow-desktop-rescan"));
    assert!(permissions.contains(&"allow-desktop-study-progress"));
    assert!(permissions.contains(&"allow-desktop-record-position"));
    assert!(permissions.contains(&"allow-desktop-set-completed"));
    assert!(permissions.contains(&"allow-desktop-recent-study"));
    assert_eq!(
        permissions.len(),
        9,
        "a capability não pode ampliar além do núcleo e dos comandos do app: {permissions:?}"
    );
}

#[test]
fn capability_exposes_no_dialog_or_filesystem_permission() {
    let raw = read(&crate_root().join("capabilities/default.json"));
    let capability: serde_json::Value =
        serde_json::from_str(&raw).expect("a capability precisa ser JSON válido");
    for permission in capability["permissions"]
        .as_array()
        .expect("a capability precisa listar permissões")
    {
        let identifier = permission.as_str().unwrap_or_default();
        for forbidden in ["dialog", "fs", "store", "shell", "http", "persisted"] {
            assert!(
                !identifier.contains(forbidden),
                "o seletor nativo é interno ao Rust e não pode ser concedido ao webview: '{identifier}'"
            );
        }
    }
}

#[test]
fn the_native_picker_is_registered_in_rust_only() {
    let lib = read(&crate_root().join("src/lib.rs"));
    assert!(
        lib.contains("tauri_plugin_dialog::init()"),
        "o seletor nativo precisa ser registrado pelo Rust, sem plugin de fs nem store"
    );

    let command = read(&crate_root().join("src/command.rs"));
    let marker = "pub async fn desktop_select_library";
    let start = command
        .find(marker)
        .expect("desktop_select_library precisa existir como comando assíncrono");
    let rest = &command[start..];
    let signature = &rest[..rest
        .find('{')
        .expect("a assinatura do comando precisa fechar")];
    for forbidden in ["String", "Path", "path"] {
        assert!(
            !signature.contains(forbidden),
            "o webview não pode enviar caminho: {signature}"
        );
    }

    let body = &rest[..rest.find("\n}").expect("o comando precisa fechar o corpo")];
    assert!(
        body.contains("state.select_library"),
        "o comando delega a seleção ao estado: {body}"
    );
    for forbidden in ["rescan", "catalog", "database"] {
        assert!(
            !body.contains(forbidden),
            "selecionar a raiz não pode disparar rescan nem tocar no banco: {body}"
        );
    }

    // O rescan é explícito: nem a inicialização do aplicativo nem a seleção da raiz podem
    // disparar uma varredura. Só a invocação do comando dedicado faz isso.
    let setup = setup_block(&lib);
    assert!(
        !setup.contains("rescan"),
        "a inicialização não pode disparar rescan: {setup}"
    );
}

/// Extrai o bloco de `setup` do `lib.rs` até o `invoke_handler`.
fn setup_block(lib: &str) -> &str {
    let start = lib
        .find(".setup(")
        .expect("o lib.rs precisa registrar um setup");
    let end = lib[start..]
        .find(".invoke_handler")
        .map(|offset| start + offset)
        .expect("o setup precisa terminar antes do invoke_handler");
    &lib[start..end]
}

#[test]
fn invoke_handler_registers_only_the_desktop_commands() {
    let lib = read(&crate_root().join("src/lib.rs"));
    let marker = "generate_handler![";
    let start = lib
        .find(marker)
        .expect("o lib.rs precisa registrar um invoke_handler")
        + marker.len();
    let rest = &lib[start..];
    let end = rest
        .find(']')
        .expect("o invoke_handler precisa fechar a lista de comandos");
    let commands = &rest[..end];

    assert!(
        commands.contains("desktop_config"),
        "o handler precisa registrar desktop_config: {commands}"
    );
    assert!(
        commands.contains("desktop_catalog"),
        "o handler precisa registrar desktop_catalog: {commands}"
    );
    assert!(
        commands.contains("desktop_rescan"),
        "o handler precisa registrar o comando de rescan explícito: {commands}"
    );
    for command in [
        "desktop_study_progress",
        "desktop_record_position",
        "desktop_set_completed",
        "desktop_recent_study",
    ] {
        assert!(
            commands.contains(command),
            "o handler precisa registrar o comando de estudo '{command}': {commands}"
        );
    }
}

#[test]
fn study_commands_are_async_blocking_and_never_accept_an_owner() {
    let command = read(&crate_root().join("src/command.rs"));

    let expected = [
        ("desktop_study_progress", "state.study_progress("),
        ("desktop_record_position", "state.record_position("),
        ("desktop_set_completed", "state.set_completed("),
        ("desktop_recent_study", "state.recent_study("),
    ];
    for (name, delegation) in expected {
        assert!(
            !command.contains(&format!("pub fn {name}")),
            "{name} não pode ser síncrono (executaria o SQLite na main thread)"
        );
        let marker = format!("pub async fn {name}");
        let start = command
            .find(&marker)
            .unwrap_or_else(|| panic!("{name} precisa ser um comando assíncrono"));
        let rest = &command[start..];
        let signature = &rest[..rest
            .find('{')
            .unwrap_or_else(|| panic!("{name} precisa fechar a assinatura"))];
        // O dono do progresso é fixo no Rust: nenhum comando de estudo aceita `owner_id`,
        // nem alterna o dono por parâmetro, nem recebe caminho.
        for forbidden in ["owner", "owner_id", "Path", "path"] {
            assert!(
                !signature.contains(forbidden),
                "{name} não pode receber '{forbidden}': {signature}"
            );
        }
        let body = &rest[..rest
            .find("\n}")
            .unwrap_or_else(|| panic!("{name} precisa fechar o corpo"))];
        assert!(
            body.contains("tauri::async_runtime::spawn_blocking"),
            "o SQLite bloqueante de {name} precisa rodar em spawn_blocking do Tauri: {body}"
        );
        assert!(
            body.contains(delegation),
            "{name} precisa delegar ao estado: {body}"
        );
        // O dono é resolvido no adaptador: o comando não nomeia nem repassa o dono local.
        assert!(
            !body.contains("LOCAL_OWNER_ID"),
            "{name} não pode passar o dono local ao estado: {body}"
        );
    }

    // O comando de histórico recebe apenas um limite opcional: nada de caminho, dono ou
    // parâmetro que permita escolher outra biblioteca.
    let start = command
        .find("pub async fn desktop_recent_study")
        .expect("desktop_recent_study precisa existir");
    let rest = &command[start..];
    let signature = &rest[..rest
        .find('{')
        .expect("a assinatura do comando precisa fechar")];
    assert!(
        signature.contains("limit: Option<usize>"),
        "o limite do histórico precisa ser opcional: {signature}"
    );
}

#[test]
fn catalog_command_offloads_blocking_sqlite_off_the_main_thread() {
    let command = read(&crate_root().join("src/command.rs"));

    // O catálogo lê o SQLite de forma bloqueante. Um `#[tauri::command]` síncrono roda na
    // main thread do webview e travaria a UI; o comando precisa ser `async` e executar o
    // trabalho bloqueante em `spawn_blocking` do runtime do Tauri, sem adicionar `tokio`
    // como dependência direta.
    assert!(
        !command.contains("pub fn desktop_catalog"),
        "desktop_catalog não pode continuar síncrono (executaria o SQLite na main thread)"
    );

    let marker = "pub async fn desktop_catalog";
    let start = command
        .find(marker)
        .expect("desktop_catalog precisa ser um comando assíncrono (pub async fn)");
    let rest = &command[start..];
    let end = rest
        .find("\n}")
        .expect("o comando desktop_catalog precisa fechar o corpo");
    let body = &rest[..end];

    assert!(
        body.contains("tauri::async_runtime::spawn_blocking"),
        "o SQLite bloqueante precisa rodar em spawn_blocking do Tauri: {body}"
    );
    assert!(
        body.contains("state.catalog()"),
        "o trabalho bloqueante continua sendo o catálogo do estado: {body}"
    );
}

#[test]
fn rescan_command_offloads_the_blocking_scan_off_the_main_thread() {
    let command = read(&crate_root().join("src/command.rs"));

    // A varredura lê o sistema de arquivos e o SQLite de forma bloqueante. Como o catálogo,
    // o comando precisa ser `async` e executar o trabalho em `spawn_blocking`, sem
    // `tokio` direto. O webview não envia caminho: o comando não tem argumento algum.
    assert!(
        !command.contains("pub fn desktop_rescan"),
        "desktop_rescan não pode ser síncrono (executaria a varredura na main thread)"
    );

    let marker = "pub async fn desktop_rescan";
    let start = command
        .find(marker)
        .expect("desktop_rescan precisa ser um comando assíncrono (pub async fn)");
    let rest = &command[start..];
    let signature = &rest[..rest
        .find('{')
        .expect("a assinatura do comando precisa fechar")];
    for forbidden in ["String", "Path", "path"] {
        assert!(
            !signature.contains(forbidden),
            "o webview não pode enviar caminho ao rescan: {signature}"
        );
    }

    let body = &rest[..rest
        .find("\n}")
        .expect("o comando desktop_rescan precisa fechar o corpo")];
    assert!(
        body.contains("tauri::async_runtime::spawn_blocking"),
        "a varredura bloqueante precisa rodar em spawn_blocking do Tauri: {body}"
    );
    assert!(
        body.contains("state.rescan()"),
        "o trabalho bloqueante continua sendo o rescan do estado: {body}"
    );
}

#[test]
fn rescan_is_never_triggered_at_startup_or_after_selection() {
    let lib = read(&crate_root().join("src/lib.rs"));
    let setup = setup_block(&lib);
    assert!(
        !setup.contains("rescan"),
        "a inicialização do aplicativo não pode disparar rescan: {setup}"
    );

    let command = read(&crate_root().join("src/command.rs"));
    let marker = "pub async fn desktop_select_library";
    let start = command
        .find(marker)
        .expect("desktop_select_library precisa existir como comando assíncrono");
    let rest = &command[start..];
    let body = &rest[..rest.find("\n}").expect("o comando precisa fechar o corpo")];
    assert!(
        !body.contains("rescan"),
        "selecionar a raiz nunca pode disparar um rescan automático: {body}"
    );
}

#[test]
fn media_protocol_is_registered_without_a_new_capability() {
    // O protocolo de mídia é registrado no builder do Tauri, restrito à janela principal
    // por rótulo. Ele não é uma capability: o ACL continua deny-by-default e nenhuma
    // permissão é ampliada.
    let lib = read(&crate_root().join("src/lib.rs"));
    assert!(
        lib.contains("media::register"),
        "o protocolo de mídia precisa ser registrado no builder: {lib}"
    );

    let media = read(&crate_root().join("src/media.rs"));
    assert!(
        media.contains("register_uri_scheme_protocol"),
        "o protocolo precisa ser registrado pela API oficial do Tauri"
    );
    assert!(
        media.contains("MAIN_WEBVIEW_LABEL"),
        "o protocolo precisa restringir a janela principal"
    );

    let raw = read(&crate_root().join("capabilities/default.json"));
    assert!(
        !raw.contains("learnvault-media"),
        "o protocolo de mídia não pode ampliar a capability do webview"
    );
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
