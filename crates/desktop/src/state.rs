//! Estado compartilhado do aplicativo desktop.
//!
//! O estado guarda o banco SQLite local protegido por um mutex. O mutex serializa o
//! acesso à conexão, que não é `Sync`; o `Arc` permite compartilhar o mesmo estado entre
//! os comandos e uma eventual tarefa em background sem duplicar a conexão.

use std::path::Path;
use std::sync::{Arc, Mutex};

use learnvault_core::Database;

use crate::command::catalog;
use crate::config::database_path;
use crate::dto::CatalogDto;
use crate::error::IpcError;

/// Estado Tauri do desktop: banco local acessado por IPC.
#[derive(Clone)]
pub struct DesktopState {
    database: Arc<Mutex<Database>>,
}

impl DesktopState {
    /// Abre o banco local dentro do diretório de dados, criando o diretório se preciso.
    ///
    /// O diretório de dados pertence ao aplicativo; a biblioteca externa nunca é criada,
    /// lida nem escrita por esta operação.
    pub fn open(data_dir: &Path) -> Result<Self, IpcError> {
        std::fs::create_dir_all(data_dir).map_err(|_| IpcError::database_unavailable())?;
        let database = Database::open(database_path(data_dir))
            .map_err(|_| IpcError::database_unavailable())?;
        Ok(Self {
            database: Arc::new(Mutex::new(database)),
        })
    }

    /// Constrói o catálogo a partir do banco local.
    pub fn catalog(&self) -> Result<CatalogDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        catalog(&database)
    }
}
