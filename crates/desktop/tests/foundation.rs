//! Build guard of the desktop crate.
//!
//! These tests do not start the webview nor depend on a display: they assert, by
//! construction, that the desktop foundation bundles the frontend in production, keeps the
//! ACL deny-by-default and introduces no network listener, HTTP server or privileged
//! plugin. The goal is to catch configuration regressions in CI, not to exercise the Tauri
//! UI. `reqwest` is the only exception, restricted to the sync client: the desktop talks
//! to a user-configured server, it never listens.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_SOURCE_TOKENS: [&str; 12] = [
    "TcpListener",
    "TcpStream",
    "UdpSocket",
    "std::net",
    "tokio::net",
    "HttpServer",
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
    "tauri-plugin-store",
    "tauri-plugin-persisted-scope",
    "tokio",
    "axum",
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

fn rust_sources() -> Vec<PathBuf> {
    let root = crate_root();
    let mut sources = vec![root.join("build.rs")];
    let mut entries = fs::read_dir(root.join("src"))
        .expect("the desktop crate must keep the src/ directory")
        .map(|entry| entry.expect("unreadable src/ entry").path())
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
        serde_json::from_str(&raw).expect("tauri.conf.json must be valid JSON");

    assert_eq!(
        config["build"]["frontendDist"].as_str(),
        Some("frontend-dist"),
        "the frontend must be embedded from frontend-dist"
    );
    assert!(
        config["build"].get("devUrl").is_none(),
        "devUrl is not required and must not open a port in development"
    );
    assert_eq!(
        config["app"]["windows"][0]["decorations"].as_bool(),
        Some(false),
        "the undecorated window requires the custom titlebar"
    );

    let plugins = config.get("plugins").and_then(|value| value.as_object());
    if let Some(plugins) = plugins {
        for key in plugins.keys() {
            assert_eq!(
                key, "updater",
                "only the official updater plugin may be configured"
            );
        }
    }

    let csp = config["app"]["security"]["csp"]
        .as_str()
        .expect("the CSP must be explicit");
    assert!(
        csp.contains("connect-src ipc: http://ipc.localhost"),
        "the CSP must allow the Tauri IPC: {csp}"
    );
    assert!(
        csp.contains("object-src 'none'"),
        "the CSP must block object/embed: {csp}"
    );
    assert!(
        !csp.contains("unsafe-eval"),
        "the CSP must not allow unsafe-eval: {csp}"
    );
    assert!(
        csp.contains("asset: http://asset.localhost"),
        "the CSP must allow the asset protocol for media: {csp}"
    );
    assert!(
        csp.contains("blob:"),
        "the CSP must allow blob: for media: {csp}"
    );
    assert!(
        csp.contains("learnvault-media: http://learnvault-media.localhost"),
        "the CSP must allow the local media protocol: {csp}"
    );
    assert!(
        csp.contains("learnvault-cover: http://learnvault-cover.localhost"),
        "the CSP must allow the covers protocol as an image source: {csp}"
    );

    let window = &config["app"]["windows"][0];
    assert_eq!(
        window["label"].as_str(),
        Some("main"),
        "the main window must have the label main"
    );
    assert_eq!(
        window["url"].as_str(),
        Some("200.html"),
        "the window must load the embedded shell"
    );
}

#[test]
fn windows_bundle_is_nsis_per_user_with_pt_br_and_english() {
    let raw = read(&crate_root().join("tauri.conf.json"));
    let config: serde_json::Value =
        serde_json::from_str(&raw).expect("tauri.conf.json must be valid JSON");

    assert_eq!(
        config["bundle"]["active"].as_bool(),
        Some(true),
        "the bundle must be active to produce the installer"
    );
    assert_eq!(
        config["bundle"]["createUpdaterArtifacts"].as_str(),
        Some("v1Compatible"),
        "the bundle must enable createUpdaterArtifacts to produce .nsis.zip and .sig"
    );
    let targets = config["bundle"]["targets"]
        .as_array()
        .expect("the bundle must declare targets");
    assert!(
        targets.iter().any(|target| target == "nsis"),
        "the Windows target is the NSIS installer: {targets:?}"
    );
    let icons = config["bundle"]["icon"]
        .as_array()
        .expect("the bundle must list icons");
    assert!(
        icons.iter().any(|icon| icon == "icons/icon.ico"),
        "the Windows installer requires the .ico icon: {icons:?}"
    );

    let nsis = &config["bundle"]["windows"]["nsis"];
    assert_eq!(
        nsis["installMode"].as_str(),
        Some("currentUser"),
        "the installation is per user, without an administrator password"
    );
    assert_eq!(
        nsis["displayLanguageSelector"].as_bool(),
        Some(true),
        "the installer must offer the language choice"
    );
    let languages = nsis["languages"]
        .as_array()
        .expect("the installer must list languages");
    for language in ["PortugueseBR", "English"] {
        assert!(
            languages.iter().any(|candidate| candidate == language),
            "the language '{language}' must be in the installer: {languages:?}"
        );
    }
    for key in ["installerIcon", "uninstallerIcon"] {
        assert_eq!(
            nsis[key].as_str(),
            Some("icons/icon.ico"),
            "the installer and uninstaller must use the application icon: {key}"
        );
    }

    assert_eq!(
        config["bundle"]["windows"]["webviewInstallMode"]["type"].as_str(),
        Some("downloadBootstrapper"),
        "the installer downloads WebView2 when it is missing"
    );
}

#[test]
fn capability_is_deny_by_default_without_remote_sources() {
    let raw = read(&crate_root().join("capabilities/default.json"));
    let capability: serde_json::Value =
        serde_json::from_str(&raw).expect("the capability must be valid JSON");

    assert!(
        capability.get("remote").is_none(),
        "remote capabilities are not allowed"
    );
    assert!(
        capability["identifier"].as_str().is_some(),
        "the capability requires an identifier"
    );

    let windows = capability["windows"]
        .as_array()
        .expect("the capability must list windows");
    assert!(
        windows.iter().any(|window| window == "main"),
        "the capability must cover the main window"
    );

    let permissions = capability["permissions"]
        .as_array()
        .expect("the capability must list permissions");
    assert!(
        !permissions.is_empty(),
        "the capability must grant at least the core default set"
    );
    for permission in permissions {
        let identifier = permission.as_str().unwrap_or_default();
        assert_ne!(identifier, "*", "a permission wildcard is forbidden");
        assert!(!identifier.ends_with(":all"), "broad permission forbidden");
        for forbidden in ["shell", "process", "http", "updater", "fs:", "scope"] {
            assert!(
                !identifier.contains(forbidden),
                "privileged permission '{identifier}' is forbidden in the desktop"
            );
        }
    }
}

#[test]
fn build_registers_the_app_commands_in_the_acl_manifest() {
    let build = read(&crate_root().join("build.rs"));
    assert!(
        build.contains("app_manifest"),
        "the build must declare the ACL AppManifest"
    );
    for command in [
        "desktop_catalog",
        "desktop_import_course",
        "desktop_remove_course",
        "desktop_import_course_cover",
        "desktop_remove_course_cover",
        "desktop_rescan",
        "desktop_search",
        "desktop_study_progress",
        "desktop_record_position",
        "desktop_set_completed",
        "desktop_recent_study",
        "desktop_study_state",
        "desktop_study_summary",
        "desktop_list_notes",
        "desktop_create_note",
        "desktop_update_note",
        "desktop_delete_note",
        "desktop_list_pdf_annotations",
        "desktop_create_pdf_annotation",
        "desktop_update_pdf_annotation",
        "desktop_delete_pdf_annotation",
        "desktop_list_flashcards",
        "desktop_list_all_flashcards",
        "desktop_create_flashcard",
        "desktop_update_flashcard",
        "desktop_delete_flashcard",
        "desktop_review_flashcard",
        "desktop_list_favorites",
        "desktop_set_course_favorite",
        "desktop_set_lesson_favorite",
        "desktop_record_focus_session",
        "desktop_list_focus_sessions",
        "desktop_focus_summary",
        "desktop_reward_summary",
        "desktop_claim_daily_mission",
        "desktop_weekly_goal",
        "desktop_set_weekly_goal",
        "desktop_media_removal_impact",
        "desktop_remove_media",
        "desktop_set_course_order",
        "desktop_reset_course_order",
        "desktop_check_update",
        "desktop_download_update",
        "desktop_restart_to_update",
        "desktop_media_capabilities",
        "desktop_open_in_system_player",
        "desktop_playback_preferences",
        "desktop_save_playback_preferences",
        "desktop_sync_state",
        "desktop_sync_configure",
        "desktop_sync_login",
        "desktop_sync_logout",
        "desktop_sync_now",
    ] {
        assert!(
            build.contains(command),
            "the command '{command}' must be declared in the app-defined ACL"
        );
    }
}

#[test]
fn capability_grants_only_the_core_default_and_the_app_commands() {
    let raw = read(&crate_root().join("capabilities/default.json"));
    let capability: serde_json::Value =
        serde_json::from_str(&raw).expect("the capability must be valid JSON");
    let permissions: Vec<&str> = capability["permissions"]
        .as_array()
        .expect("the capability must list permissions")
        .iter()
        .map(|permission| permission.as_str().unwrap_or_default())
        .collect();

    assert!(permissions.contains(&"core:window:allow-minimize"));
    assert!(permissions.contains(&"core:window:allow-toggle-maximize"));
    assert!(permissions.contains(&"core:window:allow-close"));
    assert!(permissions.contains(&"core:window:allow-start-dragging"));
    assert!(permissions.contains(&"core:window:allow-is-maximized"));
    assert!(permissions.contains(&"core:default"));
    assert!(permissions.contains(&"allow-desktop-import-course"));
    assert!(permissions.contains(&"allow-desktop-catalog"));
    assert!(permissions.contains(&"allow-desktop-remove-course"));
    assert!(permissions.contains(&"allow-desktop-import-course-cover"));
    assert!(permissions.contains(&"allow-desktop-remove-course-cover"));
    assert!(permissions.contains(&"allow-desktop-rescan"));
    assert!(permissions.contains(&"allow-desktop-study-progress"));
    assert!(permissions.contains(&"allow-desktop-record-position"));
    assert!(permissions.contains(&"allow-desktop-set-completed"));
    assert!(permissions.contains(&"allow-desktop-recent-study"));
    assert!(permissions.contains(&"allow-desktop-study-state"));
    assert!(permissions.contains(&"allow-desktop-study-summary"));
    assert!(permissions.contains(&"allow-desktop-list-notes"));
    assert!(permissions.contains(&"allow-desktop-create-note"));
    assert!(permissions.contains(&"allow-desktop-update-note"));
    assert!(permissions.contains(&"allow-desktop-delete-note"));
    assert!(permissions.contains(&"allow-desktop-list-pdf-annotations"));
    assert!(permissions.contains(&"allow-desktop-create-pdf-annotation"));
    assert!(permissions.contains(&"allow-desktop-update-pdf-annotation"));
    assert!(permissions.contains(&"allow-desktop-delete-pdf-annotation"));
    assert!(permissions.contains(&"allow-desktop-list-flashcards"));
    assert!(permissions.contains(&"allow-desktop-list-all-flashcards"));
    assert!(permissions.contains(&"allow-desktop-create-flashcard"));
    assert!(permissions.contains(&"allow-desktop-update-flashcard"));
    assert!(permissions.contains(&"allow-desktop-delete-flashcard"));
    assert!(permissions.contains(&"allow-desktop-review-flashcard"));
    assert!(permissions.contains(&"allow-desktop-list-favorites"));
    assert!(permissions.contains(&"allow-desktop-set-course-favorite"));
    assert!(permissions.contains(&"allow-desktop-set-lesson-favorite"));
    assert!(permissions.contains(&"allow-desktop-record-focus-session"));
    assert!(permissions.contains(&"allow-desktop-list-focus-sessions"));
    assert!(permissions.contains(&"allow-desktop-focus-summary"));
    assert!(permissions.contains(&"allow-desktop-reward-summary"));
    assert!(permissions.contains(&"allow-desktop-claim-daily-mission"));
    assert!(permissions.contains(&"allow-desktop-weekly-goal"));
    assert!(permissions.contains(&"allow-desktop-set-weekly-goal"));
    assert!(permissions.contains(&"allow-desktop-media-removal-impact"));
    assert!(permissions.contains(&"allow-desktop-remove-media"));
    assert!(permissions.contains(&"allow-desktop-set-course-order"));
    assert!(permissions.contains(&"allow-desktop-reset-course-order"));
    assert!(permissions.contains(&"allow-desktop-check-update"));
    assert!(permissions.contains(&"allow-desktop-download-update"));
    assert!(permissions.contains(&"allow-desktop-restart-to-update"));
    assert!(permissions.contains(&"allow-desktop-media-capabilities"));
    assert!(permissions.contains(&"allow-desktop-open-in-system-player"));
    assert!(permissions.contains(&"allow-desktop-playback-preferences"));
    assert!(permissions.contains(&"allow-desktop-save-playback-preferences"));
    assert!(permissions.contains(&"allow-desktop-sync-state"));
    assert!(permissions.contains(&"allow-desktop-sync-configure"));
    assert!(permissions.contains(&"allow-desktop-sync-login"));
    assert!(permissions.contains(&"allow-desktop-sync-logout"));
    assert!(permissions.contains(&"allow-desktop-sync-now"));
    assert_eq!(
        permissions.len(),
        58,
        "the capability must not extend beyond the core, the titlebar controls and the app commands: {permissions:?}"
    );
}

#[test]
fn capability_exposes_no_dialog_or_filesystem_permission() {
    let raw = read(&crate_root().join("capabilities/default.json"));
    let capability: serde_json::Value =
        serde_json::from_str(&raw).expect("the capability must be valid JSON");
    for permission in capability["permissions"]
        .as_array()
        .expect("the capability must list permissions")
    {
        let identifier = permission.as_str().unwrap_or_default();
        for forbidden in ["dialog", "fs", "store", "shell", "http", "persisted"] {
            assert!(
                !identifier.contains(forbidden),
                "the native picker is internal to Rust and must not be granted to the webview: '{identifier}'"
            );
        }
    }
}

#[test]
fn dialog_uses_the_xdg_portal_backend_only_on_linux() {
    let manifest = read(&crate_root().join("Cargo.toml"));

    assert!(
        manifest.contains("target.'cfg(target_os = \"linux\")'.dependencies"),
        "the desktop must declare a Linux-specific configuration"
    );
    assert!(
        manifest.contains("features = [\"xdg-portal\"]"),
        "Linux must use the official xdg-portal backend"
    );
    assert!(
        manifest.contains(
            "target.'cfg(any(target_os = \"windows\", target_os = \"macos\"))'.dependencies",
        ),
        "Windows and macOS must keep the native dependency"
    );
}

#[test]
fn flatpak_manifest_does_not_grant_broad_home_or_host_filesystem_access() {
    let manifest = read(&crate_root().join("../../deploy/packages/app.learnvault.desktop.yml"));

    for entry in manifest.lines().map(str::trim) {
        assert!(
            !entry.starts_with("- --filesystem=home") && !entry.starts_with("- --filesystem=host"),
            "the manifest must not grant broad filesystem access: {entry}"
        );
    }
}

#[test]
fn the_native_picker_is_registered_in_rust_only() {
    let lib = read(&crate_root().join("src/lib.rs"));
    assert!(
        lib.contains("tauri_plugin_dialog::init()"),
        "the native picker must be registered by Rust, with no fs or store plugin"
    );

    let command = read(&crate_root().join("src/command.rs"));
    let marker = "pub async fn desktop_import_course";
    let start = command
        .find(marker)
        .expect("desktop_import_course must exist as an async command");
    let rest = &command[start..];
    let signature = &rest[..rest
        .find('{')
        .expect("the command signature must be followed by an opening brace")];
    for forbidden in ["String", "Path", "path"] {
        assert!(
            !signature.contains(forbidden),
            "the webview must not send a path: {signature}"
        );
    }

    let body = &rest[..rest.find("\n}").expect("the command must close its body")];
    assert!(
        body.contains("state.import_course"),
        "the command delegates the import to the state: {body}"
    );
    for forbidden in ["rescan", "catalog", "database"] {
        assert!(
            !body.contains(forbidden),
            "importing a course must not trigger a rescan or touch the database: {body}"
        );
    }

    // The rescan is explicit: neither the application's startup nor the root selection may
    // trigger a scan. Only invoking the dedicated command does that.
    let setup = setup_block(&lib);
    assert!(
        !setup.contains("rescan"),
        "startup must not trigger a rescan: {setup}"
    );
}

/// Extracts the `setup` block of `lib.rs` up to the `invoke_handler`.
fn setup_block(lib: &str) -> &str {
    let start = lib
        .find(".setup(")
        .expect("lib.rs must register a setup block to start the application");
    let end = lib[start..]
        .find(".invoke_handler")
        .map(|offset| start + offset)
        .expect("the setup must end before the invoke_handler");
    &lib[start..end]
}

#[test]
fn invoke_handler_registers_only_the_desktop_commands() {
    let lib = read(&crate_root().join("src/lib.rs"));
    let marker = "generate_handler![";
    let start = lib
        .find(marker)
        .expect("lib.rs must register an invoke_handler")
        + marker.len();
    let rest = &lib[start..];
    let end = rest
        .find(']')
        .expect("the invoke_handler must close the command list");
    let commands = &rest[..end];

    assert!(
        commands.contains("desktop_import_course"),
        "the handler must register desktop_import_course: {commands}"
    );
    assert!(
        commands.contains("desktop_remove_course"),
        "the handler must register desktop_remove_course: {commands}"
    );
    assert!(
        commands.contains("desktop_catalog"),
        "the handler must register desktop_catalog: {commands}"
    );
    assert!(
        commands.contains("desktop_rescan"),
        "the handler must register the explicit rescan command: {commands}"
    );
    for command in [
        "desktop_study_progress",
        "desktop_record_position",
        "desktop_set_completed",
        "desktop_recent_study",
        "desktop_study_state",
        "desktop_list_notes",
        "desktop_create_note",
        "desktop_update_note",
        "desktop_delete_note",
        "desktop_list_pdf_annotations",
        "desktop_create_pdf_annotation",
        "desktop_update_pdf_annotation",
        "desktop_delete_pdf_annotation",
        "desktop_list_flashcards",
        "desktop_list_all_flashcards",
        "desktop_create_flashcard",
        "desktop_update_flashcard",
        "desktop_delete_flashcard",
        "desktop_review_flashcard",
        "desktop_list_favorites",
        "desktop_set_course_favorite",
        "desktop_set_lesson_favorite",
        "desktop_list_focus_sessions",
        "desktop_media_removal_impact",
        "desktop_remove_media",
        "desktop_import_course_cover",
        "desktop_remove_course_cover",
        "desktop_set_course_order",
        "desktop_reset_course_order",
        "desktop_check_update",
        "desktop_download_update",
        "desktop_restart_to_update",
    ] {
        assert!(
            commands.contains(command),
            "the handler must register the command '{command}': {commands}"
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
        ("desktop_study_state", "state.study_state("),
        ("desktop_list_notes", "state.list_notes("),
        ("desktop_create_note", "state.create_note("),
        ("desktop_update_note", "state.update_note("),
        ("desktop_delete_note", "state.delete_note("),
        (
            "desktop_list_pdf_annotations",
            "state.list_pdf_annotations(",
        ),
        (
            "desktop_create_pdf_annotation",
            "state.create_pdf_annotation(",
        ),
        (
            "desktop_update_pdf_annotation",
            "state.update_pdf_annotation(",
        ),
        (
            "desktop_delete_pdf_annotation",
            "state.delete_pdf_annotation(",
        ),
        ("desktop_list_flashcards", "state.list_flashcards("),
        ("desktop_list_all_flashcards", "state.list_all_flashcards("),
        ("desktop_create_flashcard", "state.create_flashcard("),
        ("desktop_update_flashcard", "state.update_flashcard("),
        ("desktop_delete_flashcard", "state.delete_flashcard("),
        ("desktop_review_flashcard", "state.review_flashcard("),
        ("desktop_list_favorites", "state.list_favorites("),
        ("desktop_set_course_favorite", "state.set_course_favorite("),
        ("desktop_set_lesson_favorite", "state.set_lesson_favorite("),
        ("desktop_list_focus_sessions", "state.list_focus_sessions("),
    ];
    for (name, delegation) in expected {
        assert!(
            !command.contains(&format!("pub fn {name}")),
            "{name} must not be synchronous (it would run SQLite on the main thread)"
        );
        let marker = format!("pub async fn {name}");
        let start = command
            .find(&marker)
            .unwrap_or_else(|| panic!("{name} must be an async command"));
        let rest = &command[start..];
        let signature = &rest[..rest
            .find('{')
            .unwrap_or_else(|| panic!("{name} must close the signature"))];
        // The owner of the progress is fixed in Rust: no study command accepts `owner_id`,
        // nor switches the owner by parameter, nor receives a path.
        for forbidden in ["owner", "owner_id", "Path", "path"] {
            assert!(
                !signature.contains(forbidden),
                "{name} must not accept '{forbidden}': {signature}"
            );
        }
        let body = &rest[..rest
            .find("\n}")
            .unwrap_or_else(|| panic!("{name} must close the body"))];
        assert!(
            body.contains("tauri::async_runtime::spawn_blocking"),
            "the blocking SQLite of {name} must run in Tauri spawn_blocking: {body}"
        );
        assert!(
            body.contains(delegation),
            "{name} must delegate to the state: {body}"
        );
        // The owner is resolved in the adapter: the command neither names nor passes on the local owner.
        assert!(
            !body.contains("LOCAL_OWNER_ID"),
            "{name} must not pass the local owner to the state: {body}"
        );
    }

    // The history command receives only an optional limit: no path, owner or
    // parameter that would let it choose another library.
    let start = command
        .find("pub async fn desktop_recent_study")
        .expect("desktop_recent_study must exist");
    let rest = &command[start..];
    let signature = &rest[..rest
        .find('{')
        .expect("the command signature must be followed by an opening brace")];
    assert!(
        signature.contains("limit: Option<usize>"),
        "the history limit must be optional: {signature}"
    );
}

#[test]
fn catalog_command_offloads_blocking_sqlite_off_the_main_thread() {
    let command = read(&crate_root().join("src/command.rs"));

    // The catalog reads SQLite in a blocking way. A synchronous `#[tauri::command]` runs on the
    // webview's main thread and would freeze the UI; the command must be `async` and run the
    // blocking work in `spawn_blocking` of the Tauri runtime, without adding `tokio`
    // as a direct dependency.
    assert!(
        !command.contains("pub fn desktop_catalog"),
        "desktop_catalog must not stay synchronous (it would run SQLite on the main thread)"
    );

    let marker = "pub async fn desktop_catalog";
    let start = command
        .find(marker)
        .expect("desktop_catalog must be an async command (pub async fn)");
    let rest = &command[start..];
    let end = rest
        .find("\n}")
        .expect("the desktop_catalog command must close its body");
    let body = &rest[..end];

    assert!(
        body.contains("tauri::async_runtime::spawn_blocking"),
        "the blocking SQLite must run in Tauri spawn_blocking: {body}"
    );
    assert!(
        body.contains("state.catalog()"),
        "the blocking work is still the state catalog: {body}"
    );
}

#[test]
fn rescan_command_offloads_the_blocking_scan_off_the_main_thread() {
    let command = read(&crate_root().join("src/command.rs"));

    // The scan reads the file system and SQLite in a blocking way. Like the catalog,
    // the command must be `async` and run the work in `spawn_blocking`, with no direct
    // `tokio`. The webview sends no path: the command has no argument at all.
    assert!(
        !command.contains("pub fn desktop_rescan"),
        "desktop_rescan must not be synchronous (it would run the scan on the main thread)"
    );

    let marker = "pub async fn desktop_rescan";
    let start = command
        .find(marker)
        .expect("desktop_rescan must be an async command (pub async fn)");
    let rest = &command[start..];
    let signature = &rest[..rest
        .find('{')
        .expect("the command signature must be followed by an opening brace")];
    for forbidden in ["String", "Path", "path"] {
        assert!(
            !signature.contains(forbidden),
            "the webview must not send a path to the rescan: {signature}"
        );
    }

    let body = &rest[..rest
        .find("\n}")
        .expect("the desktop_rescan command must close its body")];
    assert!(
        body.contains("tauri::async_runtime::spawn_blocking"),
        "the blocking scan must run in Tauri spawn_blocking: {body}"
    );
    assert!(
        body.contains("state.rescan()"),
        "the blocking work is still the state rescan: {body}"
    );
}

#[test]
fn rescan_is_never_triggered_at_startup_or_after_selection() {
    let lib = read(&crate_root().join("src/lib.rs"));
    let setup = setup_block(&lib);
    assert!(
        !setup.contains("rescan"),
        "application startup must not trigger a rescan: {setup}"
    );

    let command = read(&crate_root().join("src/command.rs"));
    let marker = "pub async fn desktop_import_course";
    let start = command
        .find(marker)
        .expect("desktop_import_course must exist as an async command");
    let rest = &command[start..];
    let body = &rest[..rest.find("\n}").expect("the command must close its body")];
    assert!(
        !body.contains("rescan"),
        "importing a course must never trigger an automatic rescan: {body}"
    );
}

#[test]
fn media_protocol_is_registered_without_a_new_capability() {
    // The media protocol is registered on the Tauri builder, restricted to the main window
    // by label. It is not a capability: the ACL stays deny-by-default and no
    // permission is widened.
    let lib = read(&crate_root().join("src/lib.rs"));
    assert!(
        lib.contains("media::register"),
        "the media protocol must be registered on the builder: {lib}"
    );

    let media = read(&crate_root().join("src/media.rs"));
    assert!(
        media.contains("register_uri_scheme_protocol"),
        "the protocol must be registered through the official Tauri API"
    );
    assert!(
        media.contains("MAIN_WEBVIEW_LABEL"),
        "the protocol must restrict the main window"
    );

    let raw = read(&crate_root().join("capabilities/default.json"));
    assert!(
        !raw.contains("learnvault-media"),
        "the media protocol must not widen the webview capability"
    );
}

#[test]
fn cover_protocol_is_registered_without_a_new_capability() {
    // The covers protocol is registered on the Tauri builder, restricted to the main window by
    // label, and reuses media's exact-origin CORS. It is not a capability: the ACL
    // stays deny-by-default and no plugin permission is widened.
    let lib = read(&crate_root().join("src/lib.rs"));
    assert!(
        lib.contains("covers::register"),
        "the covers protocol must be registered on the builder: {lib}"
    );

    let covers = read(&crate_root().join("src/covers.rs"));
    for token in ["register_uri_scheme_protocol", "MAIN_WEBVIEW_LABEL"] {
        assert!(
            covers.contains(token),
            "the covers protocol must use the official Tauri API and restrict the main window: {token}"
        );
    }
    assert!(
        covers.contains("apply_media_cors"),
        "the covers protocol must reuse the exact-origin CORS from media"
    );

    let raw = read(&crate_root().join("capabilities/default.json"));
    assert!(
        !raw.contains("learnvault-cover"),
        "the covers protocol must not widen the webview capability"
    );
}

#[test]
fn desktop_source_has_no_network_listener_or_http_server() {
    for source in rust_sources() {
        let content = read(&source);
        for token in FORBIDDEN_SOURCE_TOKENS {
            assert!(
                !content.contains(token),
                "{} must not contain '{token}'",
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
                "the dependency '{dependency}' does not belong in the desktop"
            );
        }
    }
}
