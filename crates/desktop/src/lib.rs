//! Aplicativo desktop do LearnVault (Tauri 2).
//!
//! Esta fatia expõe a fronteira IPC de catálogo: o webview consulta a configuração
//! (`desktop_config`) e o catálogo (`desktop_catalog`). O catálogo é derivado pelo núcleo
//! `learnvault-core` sobre um banco SQLite local, no diretório de dados do aplicativo.
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
pub mod state;

pub use command::{catalog, desktop_catalog, desktop_config};
pub use config::{DesktopConfigDto, config_status};
pub use dto::{CatalogDto, CatalogFileDto, CatalogNodeDto};
pub use error::{IpcError, IpcErrorCode};
pub use state::DesktopState;

use tauri::Manager;

/// Inicializa o webview Tauri apontando para a SPA embutida em `frontend-dist`.
///
/// No `setup`, o estado abre o banco local no diretório de dados do aplicativo e registra
/// apenas os comandos de configuração e catálogo. O deny-by-default do ACL garante que
/// nada fora das permissões declaradas em `capabilities/` seja executável pelo webview.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let state = DesktopState::open(&data_dir).map_err(|_| {
                std::io::Error::other("learnvault desktop: banco de dados local indisponível")
            })?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            command::desktop_config,
            command::desktop_catalog
        ])
        .run(tauri::generate_context!())
        .expect("falha ao executar o aplicativo desktop do LearnVault");
}
