//! Estado compartilhado do aplicativo desktop.
//!
//! O estado guarda dois recursos independentes: o banco SQLite local protegido por um
//! mutex e a raiz da biblioteca escolhida, protegida por um mutex próprio. Separar os
//! cadeados é deliberado: o seletor nativo e a persistência da configuração não podem
//! segurar o cadeado do SQLite, e a leitura do catálogo continua fora da main thread.
//!
//! A raiz da biblioteca é apenas um caminho canonicalizado; o estado nunca escreve dentro
//! dela. A configuração é gravada no diretório de configuração do aplicativo, nunca no
//! banco nem na biblioteca.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use learnvault_core::Database;

use crate::command::catalog;
use crate::config::{DesktopConfigDto, database_path};
use crate::dto::CatalogDto;
use crate::error::IpcError;
use crate::library::{LibraryRoot, load_library_root, store_library_root};

/// Estado Tauri do desktop: banco local e raiz da biblioteca.
#[derive(Clone)]
pub struct DesktopState {
    database: Arc<Mutex<Database>>,
    config_dir: PathBuf,
    library_root: Arc<Mutex<Option<LibraryRoot>>>,
}

impl DesktopState {
    /// Abre o banco local e carrega a raiz da biblioteca do diretório de configuração.
    ///
    /// O diretório de dados pertence ao aplicativo; a biblioteca externa nunca é criada,
    /// lida nem escrita por esta operação. A configuração é revalidada: uma raiz removida
    /// ou inválida rebaixa o estado para não configurado, sem falha fatal.
    pub fn open(data_dir: &Path, config_dir: &Path) -> Result<Self, IpcError> {
        std::fs::create_dir_all(data_dir).map_err(|_| IpcError::database_unavailable())?;
        let database = Database::open(database_path(data_dir))
            .map_err(|_| IpcError::database_unavailable())?;
        let library_root = load_library_root(config_dir);

        Ok(Self {
            database: Arc::new(Mutex::new(database)),
            config_dir: config_dir.to_path_buf(),
            library_root: Arc::new(Mutex::new(library_root)),
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

    /// Situação de configuração exposta ao webview: só o booleano, nunca o caminho.
    pub fn config_status(&self) -> DesktopConfigDto {
        DesktopConfigDto {
            configured: self.library_root().is_some(),
        }
    }

    /// Raiz da biblioteca validada, se houver uma configurada. Uso interno do Rust.
    pub fn library_root(&self) -> Option<LibraryRoot> {
        self.library_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Executa o seletor nativo e persiste a raiz escolhida com segurança.
    ///
    /// `pick` devolve o caminho bruto escolhido pelo usuário ou `None` quando o seletor é
    /// cancelado. O cancelamento preserva a configuração anterior e devolve o estado
    /// atual. Uma escolha inválida ou uma falha de gravação também preserva o estado e
    /// devolve um erro IPC sanitizado. Em nenhum caso o caminho é escrito dentro da
    /// biblioteca, e o cadeado do SQLite nunca é adquirido aqui.
    pub fn select_library(
        &self,
        pick: impl FnOnce() -> Option<PathBuf>,
    ) -> Result<DesktopConfigDto, IpcError> {
        let Some(picked) = pick() else {
            return Ok(self.config_status());
        };

        let root = LibraryRoot::resolve(&picked).map_err(|_| IpcError::library_root_invalid())?;
        store_library_root(&self.config_dir, &root)
            .map_err(|_| IpcError::library_config_unavailable())?;

        let mut guard = self
            .library_root
            .lock()
            .map_err(|_| IpcError::library_config_unavailable())?;
        *guard = Some(root);

        Ok(DesktopConfigDto { configured: true })
    }
}
