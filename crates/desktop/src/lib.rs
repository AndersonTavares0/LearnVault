//! Aplicativo desktop do LearnVault (Tauri 2).
//!
//! Esta fatia expõe a fronteira IPC de configuração, catálogo, rescan e progresso de
//! estudo: o webview consulta a configuração (`desktop_config`), dispara a seleção da raiz
//! da biblioteca (`desktop_select_library`), lê o catálogo (`desktop_catalog`), aciona a
//! indexação explícita (`desktop_rescan`) e sincroniza o estudo por `desktop_study_progress`,
//! `desktop_record_position`, `desktop_set_completed` e `desktop_recent_study`. O catálogo, o
//! resumo do rescan e o progresso são derivados pelo núcleo `learnvault-core` sobre um banco
//! SQLite local, no diretório de dados do aplicativo.
//!
//! O dono do progresso de estudo é um identificador local fixo escolhido pelo Rust: nenhum
//! comando de estudo aceita `owner_id`, então o webview só lê e grava o progresso do dono
//! local e nunca o de outro dono.
//!
//! A raiz da biblioteca é escolhida por um seletor nativo acionado pelo Rust
//! (`tauri-plugin-dialog`, interno e não concedido ao webview) e persistida de forma
//! atômica e versionada no diretório de configuração do aplicativo — nunca dentro da
//! biblioteca nem no SQLite. O webview nunca envia nem recebe um caminho: só o booleano
//! `configured`.
//!
//! O rescan é sempre explícito: nem a inicialização nem a seleção da raiz o disparam, e
//! chamadas concorrentes são recusadas com um erro estável enquanto uma varredura está em
//! andamento. Não há worker, progresso nem cancelamento nesta fatia.
//!
//! Não há listener de rede, servidor HTTP, plugin de shell, filesystem, http ou updater,
//! nem entrega de mídia ou autenticação nesta fatia. Sem entrega de mídia o player do
//! desktop não tem fonte nem reprodução, então a interface não oferece retomada nem
//! conclusão: o progresso existe por IPC, mas a única fonte de posição real — a
//! reprodução — continua pendente. A biblioteca permanece externa e somente leitura: o
//! rescan apenas a lê e nenhum caminho de biblioteca é escrito.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod command;
pub mod config;
pub mod dto;
pub mod error;
pub mod library;
pub mod state;
pub mod study;

pub use command::{
    catalog, desktop_catalog, desktop_config, desktop_recent_study, desktop_record_position,
    desktop_rescan, desktop_select_library, desktop_set_completed, desktop_study_progress,
};
pub use config::{DATABASE_FILE_NAME, DesktopConfigDto, database_path};
pub use dto::{
    CatalogDto, CatalogFileDto, CatalogNodeDto, RecentStudyDto, RescanSummaryDto, StudyProgressDto,
};
pub use error::{IpcError, IpcErrorCode};
pub use library::{
    CONFIG_FILE_NAME, CONFIG_VERSION, LibraryError, LibraryRoot, load_library_root,
    store_library_root,
};
pub use state::{DesktopState, RescanGuard};
pub use study::{DEFAULT_RECENT_LIMIT, LOCAL_OWNER_ID};

use tauri::Manager;

/// Inicializa o webview Tauri apontando para a SPA embutida em `frontend-dist`.
///
/// No `setup`, o estado abre o banco local no diretório de dados do aplicativo, carrega e
/// revalida a raiz da biblioteca do diretório de configuração, e registra apenas os
/// comandos de configuração, seleção, catálogo, rescan e estudo. O plugin de diálogo é
/// registrado para uso interno do Rust e não é concedido ao webview pelo ACL. O
/// deny-by-default do ACL garante que nada fora das permissões declaradas em
/// `capabilities/` seja executável pelo webview.
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
            command::desktop_select_library,
            command::desktop_rescan,
            command::desktop_study_progress,
            command::desktop_record_position,
            command::desktop_set_completed,
            command::desktop_recent_study
        ])
        .run(tauri::generate_context!())
        .expect("falha ao executar o aplicativo desktop do LearnVault");
}
