//! Aplicativo desktop do LearnVault (Tauri 2).
//!
//! Esta fatia expõe a fronteira IPC de configuração e catálogo: o webview consulta a
//! configuração (`desktop_config`), dispara a seleção da raiz da biblioteca
//! (`desktop_select_library`) e lê o catálogo (`desktop_catalog`). O catálogo é derivado
//! pelo núcleo `learnvault-core` sobre um banco SQLite local, no diretório de dados do
//! aplicativo.
//!
//! A raiz da biblioteca é escolhida por um seletor nativo acionado pelo Rust
//! (`tauri-plugin-dialog`, interno e não concedido ao webview) e persistida de forma
//! atômica e versionada no diretório de configuração do aplicativo — nunca dentro da
//! biblioteca nem no SQLite. O webview nunca envia nem recebe um caminho: só o booleano
//! `configured`.
//!
//! Não há listener de rede, servidor HTTP, plugin de shell, filesystem, http ou updater,
//! nem rescan, mídia ou autenticação nesta fatia. A biblioteca permanece externa e somente
//! leitura: nenhuma varredura é disparada e nenhum caminho de biblioteca é escrito.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod command;
pub mod config;
pub mod dto;
pub mod error;
pub mod library;
pub mod state;

pub use command::{catalog, desktop_catalog, desktop_config, desktop_select_library};
pub use config::{DATABASE_FILE_NAME, DesktopConfigDto, database_path};
pub use dto::{CatalogDto, CatalogFileDto, CatalogNodeDto};
pub use error::{IpcError, IpcErrorCode};
pub use library::{
    CONFIG_FILE_NAME, CONFIG_VERSION, LibraryError, LibraryRoot, load_library_root,
    store_library_root,
};
pub use state::DesktopState;

use tauri::Manager;

/// Inicializa o webview Tauri apontando para a SPA embutida em `frontend-dist`.
///
/// No `setup`, o estado abre o banco local no diretório de dados do aplicativo, carrega e
/// revalida a raiz da biblioteca do diretório de configuração, e registra apenas os
/// comandos de configuração, seleção e catálogo. O plugin de diálogo é registrado para uso
/// interno do Rust e não é concedido ao webview pelo ACL. O deny-by-default do ACL garante
/// que nada fora das permissões declaradas em `capabilities/` seja executável pelo webview.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let config_dir = app.path().app_config_dir()?;
            let state = DesktopState::open(&data_dir, &config_dir).map_err(|_| {
                std::io::Error::other("learnvault desktop: banco de dados local indisponível")
            })?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            command::desktop_config,
            command::desktop_catalog,
            command::desktop_select_library
        ])
        .run(tauri::generate_context!())
        .expect("falha ao executar o aplicativo desktop do LearnVault");
}
