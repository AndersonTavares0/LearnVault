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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use learnvault_core::Database;

use crate::command::{LIBRARY_ID, catalog};
use crate::config::{DesktopConfigDto, database_path};
use crate::dto::{CatalogDto, RescanSummaryDto};
use crate::error::IpcError;
use crate::library::{LibraryRoot, load_library_root, store_library_root};

/// Estado Tauri do desktop: banco local e raiz da biblioteca.
#[derive(Clone)]
pub struct DesktopState {
    database: Arc<Mutex<Database>>,
    config_dir: PathBuf,
    library_root: Arc<Mutex<Option<LibraryRoot>>>,
    rescanning: Arc<AtomicBool>,
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
            rescanning: Arc::new(AtomicBool::new(false)),
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

    /// Toma o cadeado do banco local para uma leitura. Uso interno da entrega de mídia.
    ///
    /// O cadeado serializa a revalidação do catálogo com remoções concorrentes durante a
    /// abertura da mídia, exatamente como no adaptador HTTP.
    pub(crate) fn lock_database(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, learnvault_core::Database>, IpcError> {
        self.database
            .lock()
            .map_err(|_| IpcError::database_unavailable())
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

        // O cadeado da raiz cobre a gravação e a atualização em memória: duas seleções
        // concorrentes não podem terminar com a memória apontando para uma raiz diferente
        // da persistida em disco. É o cadeado da raiz, nunca o do SQLite.
        let mut guard = self
            .library_root
            .lock()
            .map_err(|_| IpcError::library_config_unavailable())?;
        store_library_root(&self.config_dir, &root)
            .map_err(|_| IpcError::library_config_unavailable())?;
        *guard = Some(root);

        Ok(DesktopConfigDto { configured: true })
    }

    /// Reserva o estado de rescan, devolvendo um guarda que o libera no `Drop`.
    ///
    /// A troca é atômica: a primeira chamada vence e as concorrentes recebem
    /// [`IpcError::rescan_in_progress`], sem esperar nem entrar em paralelo. Enquanto o
    /// guarda existe, nenhuma outra varredura pode começar — o estado não é cancelado nem
    /// corrompido, apenas recusado. É o estado ocupado do rescan, nunca o cadeado do SQLite.
    pub fn begin_rescan(&self) -> Result<RescanGuard, IpcError> {
        self.rescanning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| IpcError::rescan_in_progress())?;
        Ok(RescanGuard {
            rescanning: Arc::clone(&self.rescanning),
        })
    }

    /// Lê a biblioteca configurada e aplica a varredura ao catálogo local.
    ///
    /// Só é executado por ação explícita: nem a inicialização nem a seleção da raiz o
    /// chamam. Sem raiz configurada devolve [`IpcError::library_not_configured`]; se a raiz
    /// registrada sumiu, virou link/junction ou ficou inacessível, a revalidação imediata
    /// devolve [`IpcError::rescan_unavailable`] e a política do núcleo cobre a raiz que já
    /// não pode ser lida. A raiz é revalidada e copiada antes da varredura e nenhum cadeado
    /// da configuração é mantido durante a leitura; o banco permanece fora da biblioteca e
    /// a biblioteca permanece somente leitura. Chamadas concorrentes devolvem
    /// [`IpcError::rescan_in_progress`] sem tocar o catálogo.
    ///
    /// Limitação conhecida desta fatia: o cadeado do banco SQLite é mantido durante toda a
    /// varredura, então uma leitura de catálogo concorrente aguarda a indexação terminar.
    /// O estado ocupado recusa um segundo rescan, mas não paraleliza a leitura do catálogo.
    pub fn rescan(&self) -> Result<RescanSummaryDto, IpcError> {
        let _guard = self.begin_rescan()?;

        let root = self
            .library_root()
            .ok_or_else(IpcError::library_not_configured)?;

        // Revalida a raiz imediatamente antes de varrer: se o diretório foi removido,
        // substituído por um link/junction ou deixou de ser legível desde a seleção, a
        // varredura é recusada sem seguir o alvo. A varredura usa a forma revalidada.
        let root =
            LibraryRoot::resolve(root.as_path()).map_err(|_| IpcError::rescan_unavailable())?;

        let mut database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;

        learnvault_core::rescan(&mut database, LIBRARY_ID, root.as_path())
            .map(RescanSummaryDto::from)
            .map_err(|_| IpcError::rescan_unavailable())
    }
}

/// Guarda do estado ocupado do rescan.
///
/// Libera o estado ao ser descartado, inclusive em caso de erro ou pânico. Não expõe o
/// caminho nem o banco: é apenas o ciclo de vida da exclusão mútua do rescan.
pub struct RescanGuard {
    rescanning: Arc<AtomicBool>,
}

impl Drop for RescanGuard {
    fn drop(&mut self) {
        self.rescanning.store(false, Ordering::Release);
    }
}
