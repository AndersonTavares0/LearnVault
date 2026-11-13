//! Estado compartilhado do aplicativo desktop.
//!
//! O estado guarda dois recursos independentes: o banco SQLite local protegido por um
//! mutex e a lista de cursos registrados, protegida por um mutex próprio. Separar os
//! cadeados é deliberado: o seletor nativo e a persistência da configuração não podem
//! segurar o cadeado do SQLite, e a leitura do catálogo continua fora da main thread.
//!
//! Cada curso é apenas um caminho canonicalizado com identidade estável; o estado nunca
//! escreve dentro dele. A configuração é gravada no diretório de configuração do
//! aplicativo, nunca no banco nem nas pastas de curso.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use learnvault_core::{CourseRoot, Database, SyncOperation, SyncOperationInput};

use crate::backup;
use crate::command::catalog_with_orders;
use crate::config::{DesktopConfigDto, database_path};
use crate::courses::{CourseEntry, load_courses, store_courses};
use crate::covers::{
    COVERS_DIR_NAME, CoverError, cover_version, read_picked_cover, remove_cover, store_cover,
};
use crate::dto::{
    CatalogDto, CoverDto, CoverRemovalDto, FlashcardDto, FlashcardReviewsDto, FocusSessionDto,
    FocusSummaryDto, MediaBookmarkDto, MediaRemovalImpactDto, MediaRemovalOutcomeDto,
    NewPdfAnnotationDto, PdfAnnotationDto, PlaybackPreferencesDto, RecentStudyDto,
    RescanSummaryDto, RewardSummaryDto, SearchResultsDto, StudyFavoritesDto, StudyNoteDto,
    StudyProgressDto, StudySummaryDto, SyncResultDto, SyncStateDto,
};
use crate::error::IpcError;
use crate::ordering::{CourseOrders, load_orders, store_orders};
use crate::search;
use crate::study;
use crate::sync::{self, SyncFailure, SyncFlight, SyncSettings};

/// Estado Tauri do desktop: banco local e cursos registrados.
#[derive(Clone)]
pub struct DesktopState {
    database: Arc<Mutex<Database>>,
    config_dir: PathBuf,
    covers_dir: PathBuf,
    orders_dir: PathBuf,
    courses: Arc<Mutex<Vec<CourseEntry>>>,
    orders: Arc<Mutex<CourseOrders>>,
    rescanning: Arc<AtomicBool>,
    sync_file: PathBuf,
    sync: Arc<Mutex<SyncSettings>>,
    sync_flight: Arc<SyncFlight>,
    sync_result: Arc<Mutex<Option<SyncResultDto>>>,
    pending_update: Arc<Mutex<Option<PendingUpdate>>>,
}

/// Signed update bytes downloaded ahead of the restart, held until the person asks to restart.
///
/// The bytes never cross the IPC and are installed only after a fresh check confirms the same
/// version is still offered.
pub(crate) struct PendingUpdate {
    pub(crate) version: String,
    pub(crate) bytes: Vec<u8>,
}

impl DesktopState {
    /// Abre o banco local e carrega os cursos registrados do diretório de configuração.
    ///
    /// O diretório de dados pertence ao aplicativo; as pastas de curso são externas e
    /// nunca são criadas, lidas nem escritas por esta operação. A configuração é
    /// revalidada: cursos com raiz removida ou inválida são descartados, sem falha fatal.
    /// As capas personalizadas vivem em um subdiretório de capas do diretório de dados,
    /// criado sob demanda na primeira gravação.
    pub fn open(data_dir: &Path, config_dir: &Path) -> Result<Self, IpcError> {
        std::fs::create_dir_all(data_dir).map_err(|_| IpcError::database_unavailable())?;
        let database_file = database_path(data_dir);
        // Before the first open on a new version migrates the schema, keep a rollback copy.
        // A failed backup never blocks startup, but it must stay observable: no path, no
        // database content, just the non-fatal warning.
        if backup::backup_before_migration(&database_file).is_err() {
            eprintln!("learnvault desktop: pre-migration database backup unavailable");
        }
        let database =
            Database::open(database_file).map_err(|_| IpcError::database_unavailable())?;
        let courses = load_courses(config_dir);
        let orders = load_orders(data_dir);
        let sync_file = sync::settings_path(data_dir);
        let sync = sync::load_settings(data_dir);

        Ok(Self {
            database: Arc::new(Mutex::new(database)),
            config_dir: config_dir.to_path_buf(),
            covers_dir: data_dir.join(COVERS_DIR_NAME),
            orders_dir: data_dir.to_path_buf(),
            courses: Arc::new(Mutex::new(courses)),
            orders: Arc::new(Mutex::new(orders)),
            rescanning: Arc::new(AtomicBool::new(false)),
            sync_file,
            sync: Arc::new(Mutex::new(sync)),
            sync_flight: Arc::new(SyncFlight::default()),
            sync_result: Arc::new(Mutex::new(None)),
            pending_update: Arc::new(Mutex::new(None)),
        })
    }

    /// Holds the downloaded, signature-verified update bytes until the restart.
    pub(crate) fn hold_update(&self, version: String, bytes: Vec<u8>) -> Result<(), IpcError> {
        let mut guard = self
            .pending_update
            .lock()
            .map_err(|_| IpcError::update_unavailable())?;
        *guard = Some(PendingUpdate { version, bytes });
        Ok(())
    }

    /// Takes the held update bytes, leaving the state empty for the next download.
    pub(crate) fn take_update(&self) -> Result<Option<PendingUpdate>, IpcError> {
        Ok(self
            .pending_update
            .lock()
            .map_err(|_| IpcError::update_unavailable())?
            .take())
    }

    /// Diretório das capas personalizadas. Uso interno do Rust: nunca cruza o IPC.
    pub(crate) fn covers_dir(&self) -> PathBuf {
        self.covers_dir.clone()
    }

    /// Constrói o catálogo a partir do banco local.
    pub fn catalog(&self) -> Result<CatalogDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        catalog_with_orders(
            &database,
            &self.courses(),
            &self.orders_snapshot(),
            &self.covers_dir,
        )
    }

    /// Fotografia imutável das preferências de ordenação. Uso interno do Rust.
    fn orders_snapshot(&self) -> CourseOrders {
        self.orders
            .lock()
            .map(|orders| orders.clone())
            .unwrap_or_default()
    }

    /// Define a ordem manual dos irmãos de um nó do curso.
    ///
    /// A lista precisa ser uma permutação exata dos irmãos do caminho `parent` (vazio para
    /// os módulos diretamente sob o curso): um caminho de fora ou uma lista parcial é
    /// recusado com [`IpcError::invalid_order`]. O curso precisa estar registrado. A ordem
    /// vive no diretório de dados; nenhum arquivo do curso é lido além do catálogo.
    pub fn set_course_order(
        &self,
        course_id: &str,
        parent: &str,
        children: &[String],
    ) -> Result<(), IpcError> {
        let Some(course) = self.course(course_id) else {
            return Err(IpcError::course_not_found());
        };
        let siblings = {
            let database = self
                .database
                .lock()
                .map_err(|_| IpcError::database_unavailable())?;
            let catalog = learnvault_core::build_catalog(&database, &course.id)
                .map_err(|_| IpcError::catalog_unavailable())?;
            let (shaped, _) = learnvault_core::shape_course_catalog(catalog);
            learnvault_core::sibling_paths(&shaped, parent).ok_or_else(IpcError::invalid_order)?
        };
        if siblings.is_empty() || !learnvault_core::validate_siblings(&siblings, children) {
            return Err(IpcError::invalid_order());
        }

        let mut guard = self
            .orders
            .lock()
            .map_err(|_| IpcError::order_unavailable())?;
        let mut next = guard.clone();
        next.entry(course_id.to_owned())
            .or_default()
            .set_children(parent, children.to_vec());
        store_orders(&self.orders_dir, &next).map_err(|_| IpcError::order_unavailable())?;
        *guard = next;
        Ok(())
    }

    /// Restaura a ordem automática de um pai específico (`Some`) ou de todo o curso (`None`).
    ///
    /// É idempotente: restaurar o que já estava automático devolve `false`.
    pub fn reset_course_order(
        &self,
        course_id: &str,
        parent: Option<&str>,
    ) -> Result<bool, IpcError> {
        if self.course(course_id).is_none() {
            return Err(IpcError::course_not_found());
        }
        let mut guard = self
            .orders
            .lock()
            .map_err(|_| IpcError::order_unavailable())?;
        let mut next = guard.clone();
        let removed = match parent {
            Some(parent) => next
                .get_mut(course_id)
                .is_some_and(|order| order.remove(parent)),
            None => next.remove(course_id).is_some(),
        };
        if !removed {
            return Ok(false);
        }
        if next
            .get(course_id)
            .is_some_and(learnvault_core::CatalogOrder::is_empty)
        {
            next.remove(course_id);
        }
        store_orders(&self.orders_dir, &next).map_err(|_| IpcError::order_unavailable())?;
        *guard = next;
        Ok(true)
    }

    /// Executa o seletor nativo de imagens e grava a capa do curso escolhido.
    ///
    /// `pick` devolve o arquivo bruto escolhido pelo usuário ou `None` quando o seletor é
    /// cancelado. O cancelamento preserva a capa anterior e devolve a versão atual. O curso
    /// precisa estar registrado: uma identidade desconhecida ou insegura é recusada sem
    /// tocar o disco nem o seletor. O arquivo é reaberto e revalidado no Rust (tipo pelos
    /// magic bytes e limite de tamanho) antes da gravação atômica; nada é escrito dentro da
    /// pasta do curso.
    pub fn import_course_cover(
        &self,
        course_id: &str,
        pick: impl FnOnce() -> Option<PathBuf>,
    ) -> Result<CoverDto, IpcError> {
        if self.course(course_id).is_none() {
            return Err(IpcError::course_not_found());
        }
        let Some(picked) = pick() else {
            return Ok(self.cover_status(course_id));
        };
        let bytes = read_picked_cover(&picked).map_err(|error| match error {
            CoverError::Rejected => IpcError::invalid_cover(),
            _ => IpcError::cover_unavailable(),
        })?;
        store_cover(&self.covers_dir, course_id, &bytes)
            .map_err(|_| IpcError::cover_unavailable())?;
        Ok(self.cover_status(course_id))
    }

    /// Remove a capa personalizada de um curso registrado.
    ///
    /// A remoção não toca na pasta do curso e é idempotente: um curso sem capa devolve
    /// `removed: false`. Uma identidade desconhecida é recusada com um erro estável.
    pub fn remove_course_cover(&self, course_id: &str) -> Result<CoverRemovalDto, IpcError> {
        if self.course(course_id).is_none() {
            return Err(IpcError::course_not_found());
        }
        let removed =
            remove_cover(&self.covers_dir, course_id).map_err(|_| IpcError::cover_unavailable())?;
        Ok(CoverRemovalDto { removed })
    }

    /// Situação atual da capa de um curso, sem caminho.
    fn cover_status(&self, course_id: &str) -> CoverDto {
        CoverDto {
            cover_version: cover_version(&self.covers_dir, course_id),
        }
    }

    /// Situação de configuração exposta ao webview: só o booleano, nunca um caminho.
    ///
    /// As raízes são revalidadas em cada consulta para que um curso removido, inacessível
    /// ou revogado pelo sandbox não continue sendo anunciado como configurado durante a
    /// vida do processo. A configuração persistida e a lista em memória não são apagadas
    /// aqui: uma falha transitória ou um cancelamento continuam preservando o estado
    /// anterior, enquanto operações que usam as raízes fazem sua própria revalidação antes
    /// de prosseguir.
    pub fn config_status(&self) -> DesktopConfigDto {
        DesktopConfigDto {
            configured: self
                .courses()
                .iter()
                .any(|course| CourseRoot::resolve(course.root.as_path()).is_ok()),
        }
    }

    /// Fotografia imutável dos cursos registrados. Uso interno do Rust.
    pub fn courses(&self) -> Vec<CourseEntry> {
        self.courses
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Curso registrado com a identidade informada, se houver. Uso interno da mídia.
    pub fn course(&self, course_id: &str) -> Option<CourseEntry> {
        self.courses()
            .into_iter()
            .find(|course| course.id == course_id)
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

    /// Executa o seletor nativo e persiste a pasta escolhida como um curso.
    ///
    /// `pick` devolve o caminho bruto escolhido pelo usuário ou `None` quando o seletor é
    /// cancelado. O cancelamento preserva a lista anterior e devolve o estado atual. Uma
    /// escolha inválida ou uma falha de gravação também preserva o estado e devolve um
    /// erro IPC sanitizado. Escolher uma pasta já registrada é idempotente: a lista
    /// permanece com um único registro. Em nenhum caso o caminho é escrito dentro da
    /// pasta de curso, e o cadeado do SQLite nunca é adquirido aqui.
    pub fn import_course(
        &self,
        pick: impl FnOnce() -> Option<PathBuf>,
    ) -> Result<DesktopConfigDto, IpcError> {
        let Some(picked) = pick() else {
            return Ok(self.config_status());
        };

        let root = CourseRoot::resolve(&picked).map_err(|_| IpcError::course_path_invalid())?;
        let entry = CourseEntry::from_root(root);

        // O cadeado da lista cobre a gravação e a atualização em memória: duas importações
        // concorrentes não podem terminar com a memória divergente do arquivo persistido.
        // É o cadeado da configuração, nunca o do SQLite.
        let mut guard = self
            .courses
            .lock()
            .map_err(|_| IpcError::courses_config_unavailable())?;
        if guard.iter().any(|course| course.id == entry.id) {
            return Ok(DesktopConfigDto { configured: true });
        }
        let mut next = guard.clone();
        next.push(entry);
        store_courses(&self.config_dir, &next)
            .map_err(|_| IpcError::courses_config_unavailable())?;
        *guard = next;

        Ok(DesktopConfigDto { configured: true })
    }

    /// Remove um curso do registro, sem tocar nos arquivos.
    ///
    /// Remover uma identidade desconhecida é idempotente e não altera a lista. A gravação
    /// e a atualização em memória acontecem sob o cadeado da lista.
    pub fn remove_course(&self, course_id: &str) -> Result<DesktopConfigDto, IpcError> {
        let mut guard = self
            .courses
            .lock()
            .map_err(|_| IpcError::courses_config_unavailable())?;
        let next: Vec<CourseEntry> = guard
            .iter()
            .filter(|course| course.id != course_id)
            .cloned()
            .collect();
        if next.len() != guard.len() {
            store_courses(&self.config_dir, &next)
                .map_err(|_| IpcError::courses_config_unavailable())?;
            *guard = next;
        }
        drop(guard);
        Ok(self.config_status())
    }

    /// Lê o progresso de estudo do dono local em uma mídia conhecida.
    ///
    /// O dono é o identificador local fixo do desktop: o webview nunca o envia. A leitura
    /// bloqueante do SQLite é serializada pelo cadeado do banco, como as demais operações
    /// do estado, e não consulta nem altera o catálogo.
    pub fn study_progress(&self, media_id: &str) -> Result<StudyProgressDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::progress(&database, media_id)
    }

    /// Registra a posição de retomada do dono local, preservando a conclusão.
    ///
    /// A regra de posição e conclusão independentes pertence ao núcleo; o estado apenas
    /// delega com o dono fixo sob o cadeado do banco.
    pub fn record_position(
        &self,
        media_id: &str,
        position_seconds: i64,
    ) -> Result<StudyProgressDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let result = study::record_position(&database, media_id, position_seconds)?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Marca ou desmarca a conclusão do dono local, preservando a posição de retomada.
    pub fn set_completed(
        &self,
        media_id: &str,
        completed: bool,
    ) -> Result<StudyProgressDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let result = study::set_completed(&database, media_id, completed)?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Lista o progresso de estudo do dono local, do mais recente ao mais antigo.
    ///
    /// O limite é opcional: sem ele, o adaptador aplica o padrão do contrato e o teto do
    /// núcleo continua valendo para qualquer valor recebido.
    pub fn recent_study(&self, limit: Option<usize>) -> Result<RecentStudyDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::recent(&database, limit)
    }

    /// Resume a ofensiva e a semana corrente do dono local.
    pub fn study_summary(&self) -> Result<StudySummaryDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::summary(&database)
    }

    /// Lê as preferências de reprodução persistidas do dono local.
    pub fn playback_preference(&self) -> Result<PlaybackPreferencesDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::playback_preference(&database)
    }

    /// Grava as preferências de reprodução e as enfileira para o servidor.
    pub fn save_playback_preference(
        &self,
        video_speed: i64,
        audio_speed: i64,
        subtitle_language: &str,
    ) -> Result<PlaybackPreferencesDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let result = study::save_playback_preference(
            &database,
            video_speed,
            audio_speed,
            subtitle_language,
        )?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Identidade local do dispositivo e cursor já aplicado.
    pub fn sync_state(&self) -> Result<SyncStateDto, IpcError> {
        let (device_id, pull_cursor, pending) = {
            let database = self
                .database
                .lock()
                .map_err(|_| IpcError::database_unavailable())?;
            let state = sync::state(&database)?;
            let pending = sync::pending_count(&database)?;
            (state.device_id, state.pull_cursor, pending)
        };
        let settings = self
            .sync
            .lock()
            .map_err(|_| IpcError::sync_unavailable())?
            .clone();
        let last_result = self
            .sync_result
            .lock()
            .map_err(|_| IpcError::sync_unavailable())?
            .clone();
        let signed_in = settings.signed_in();
        Ok(SyncStateDto {
            device_id,
            pull_cursor,
            server_url: settings.server_url,
            signed_in,
            pending,
            last_error: last_result.as_ref().and_then(|result| result.error.clone()),
            last_result,
        })
    }

    /// Configura a URL do servidor. Trocar de servidor apaga a sessão guardada, para que
    /// um cookie de outra origem nunca seja enviado ao novo endereço.
    pub fn sync_configure(&self, server_url: &str) -> Result<SyncStateDto, IpcError> {
        let normalized = sync::validate_server_url(server_url)?;
        {
            let mut settings = self.sync.lock().map_err(|_| IpcError::sync_unavailable())?;
            if settings.server_url != normalized {
                settings.server_url = normalized;
                settings.session.clear();
                sync::store_settings(&self.sync_file, &settings)?;
            }
        }
        self.sync_state()
    }

    /// Entra no servidor configurado e guarda a sessão em disco.
    pub fn sync_login(&self, username: &str, password: &str) -> Result<SyncStateDto, IpcError> {
        let settings = self
            .sync
            .lock()
            .map_err(|_| IpcError::sync_unavailable())?
            .clone();
        if !settings.configured() {
            return Err(IpcError::invalid_sync_server());
        }
        let session = sync::login(&settings.server_url, username, password)?;
        {
            let mut guard = self.sync.lock().map_err(|_| IpcError::sync_unavailable())?;
            // A URL pode ter mudado durante a requisição; não grave um cookie de outra origem.
            if guard.server_url != settings.server_url {
                return Err(IpcError::invalid_sync_server());
            }
            guard.session = session;
            sync::store_settings(&self.sync_file, &guard)?;
        }
        self.sync_state()
    }

    /// Encerra a sessão no servidor (melhor esforço) e apaga a cópia local.
    pub fn sync_logout(&self) -> Result<SyncStateDto, IpcError> {
        let settings = self
            .sync
            .lock()
            .map_err(|_| IpcError::sync_unavailable())?
            .clone();
        if settings.configured() && settings.signed_in() {
            sync::logout(&settings.server_url, &settings.session);
        }
        {
            let mut guard = self.sync.lock().map_err(|_| IpcError::sync_unavailable())?;
            guard.session.clear();
            sync::store_settings(&self.sync_file, &guard)?;
        }
        *self
            .sync_result
            .lock()
            .map_err(|_| IpcError::sync_unavailable())? = None;
        self.sync_state()
    }

    /// Executa uma sincronização completa. Nunca sobrepõe outra execução: nesse caso
    /// devolve apenas o resumo com `sync_in_progress`.
    pub fn sync_now(&self) -> Result<SyncResultDto, IpcError> {
        let Some(_flight) = self.sync_flight.try_begin() else {
            return self.summary_with_error("sync_in_progress");
        };
        let settings = self
            .sync
            .lock()
            .map_err(|_| IpcError::sync_unavailable())?
            .clone();
        let result = {
            let database = self
                .database
                .lock()
                .map_err(|_| IpcError::database_unavailable())?;
            sync::run(&database, &settings.server_url, &settings.session)
        };
        let summary = match result {
            Ok(result) => result,
            Err(failure) => {
                if failure == SyncFailure::Unauthorized {
                    self.clear_session_if(&settings.server_url);
                }
                self.summary_with_error(failure.code())?
            }
        };
        *self
            .sync_result
            .lock()
            .map_err(|_| IpcError::sync_unavailable())? = Some(summary.clone());
        Ok(summary)
    }

    fn summary_with_error(&self, code: &str) -> Result<SyncResultDto, IpcError> {
        let pending = {
            let database = self
                .database
                .lock()
                .map_err(|_| IpcError::database_unavailable())?;
            sync::pending_count(&database)?
        };
        Ok(SyncResultDto {
            pushed: 0,
            pulled: 0,
            pending,
            error: Some(code.to_owned()),
        })
    }

    fn clear_session_if(&self, server_url: &str) {
        if let Ok(mut settings) = self.sync.lock()
            && settings.server_url == server_url
        {
            settings.session.clear();
            let _ = sync::store_settings(&self.sync_file, &settings);
        }
    }

    /// Dispara uma sincronização em segundo plano quando há servidor e sessão.
    ///
    /// Chamado depois de cada escrita enfileirada e na inicialização. Se outra execução
    /// já estiver em andamento, a chamada não faz nada.
    pub(crate) fn request_sync(&self) {
        let ready = self
            .sync
            .lock()
            .map(|settings| settings.configured() && settings.signed_in())
            .unwrap_or(false);
        if !ready || self.sync_flight.is_running() {
            return;
        }
        let state = self.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = state.sync_now();
        });
    }

    /// Operações ainda pendentes no outbox local.
    pub fn sync_pending(&self, limit: Option<usize>) -> Result<Vec<SyncOperationInput>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        sync::pending(&database, limit.unwrap_or(learnvault_core::MAX_SYNC_BATCH))
    }

    /// Remove do outbox as operações aceitas pelo servidor.
    pub fn sync_acknowledge(&self, op_ids: &[String]) -> Result<usize, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        sync::acknowledge(&database, op_ids)
    }

    /// Aplica as operações recebidas e grava o cursor na mesma transação.
    pub fn sync_apply(&self, operations: &[SyncOperation], cursor: i64) -> Result<usize, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        sync::apply(&database, operations, cursor)
    }

    /// Lista anotações do dono local, filtrando por mídia ou trazendo todas com caminhos relativos.
    pub fn list_notes(&self, media_id: Option<&str>) -> Result<Vec<StudyNoteDto>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_notes(&database, media_id)
    }

    /// Busca notas e catálogo no banco local do dono fixo.
    ///
    /// Os títulos dos cursos vêm do registro, então o índice é sincronizado a cada busca;
    /// a consulta e o limite são validados pelo núcleo.
    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<SearchResultsDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let courses: Vec<(String, String)> = self
            .courses()
            .iter()
            .map(|course| (course.id.clone(), course.title()))
            .collect();
        search::search(&database, &courses, query, limit)
    }

    /// Cria uma nova anotação de estudo associada a uma mídia.
    pub fn create_note(
        &self,
        media_id: &str,
        position_seconds: i64,
        content: &str,
    ) -> Result<StudyNoteDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let result = study::create_note(&database, media_id, position_seconds, content)?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Atualiza o conteúdo de uma anotação existente.
    pub fn update_note(&self, note_id: &str, content: &str) -> Result<StudyNoteDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let result = study::update_note(&database, note_id, content)?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Remove uma anotação de estudo do usuário local.
    pub fn delete_note(&self, note_id: &str) -> Result<(), IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::delete_note(&database, note_id)?;
        drop(database);
        self.request_sync();
        Ok(())
    }

    /// Lists the local owner's timestamp bookmarks for one lesson.
    pub fn list_bookmarks(&self, media_id: &str) -> Result<Vec<MediaBookmarkDto>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_bookmarks(&database, media_id)
    }

    /// Creates a timestamp bookmark for the local owner.
    pub fn create_bookmark(
        &self,
        media_id: &str,
        position_seconds: i64,
        label: &str,
    ) -> Result<MediaBookmarkDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let result = study::create_bookmark(&database, media_id, position_seconds, label)?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Deletes one of the local owner's timestamp bookmarks.
    pub fn delete_bookmark(&self, bookmark_id: &str) -> Result<(), IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::delete_bookmark(&database, bookmark_id)?;
        drop(database);
        self.request_sync();
        Ok(())
    }

    /// Lists the local owner's annotations on one PDF.
    pub fn list_pdf_annotations(&self, media_id: &str) -> Result<Vec<PdfAnnotationDto>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_pdf_annotations(&database, media_id)
    }

    /// Creates an annotation on a PDF.
    pub fn create_pdf_annotation(
        &self,
        media_id: &str,
        input: NewPdfAnnotationDto,
    ) -> Result<PdfAnnotationDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::create_pdf_annotation(&database, media_id, input)
    }

    /// Changes an annotation's color and/or text.
    pub fn update_pdf_annotation(
        &self,
        annotation_id: &str,
        color: Option<&str>,
        content: Option<&str>,
    ) -> Result<PdfAnnotationDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::update_pdf_annotation(&database, annotation_id, color, content)
    }

    /// Deletes an annotation.
    pub fn delete_pdf_annotation(&self, annotation_id: &str) -> Result<(), IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::delete_pdf_annotation(&database, annotation_id)
    }

    /// Lista os cartões de memorização do dono local para uma mídia específica.
    pub fn list_flashcards(&self, media_id: &str) -> Result<Vec<FlashcardDto>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_flashcards(&database, media_id)
    }

    /// Lists every card of the local owner, for the review hub.
    pub fn list_all_flashcards(&self) -> Result<Vec<FlashcardDto>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_all_flashcards(&database)
    }

    /// Lists the recent review history of one local card with the full log count.
    pub fn list_flashcard_reviews(
        &self,
        card_id: &str,
        limit: Option<usize>,
    ) -> Result<FlashcardReviewsDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_flashcard_reviews(&database, card_id, limit)
    }

    /// Counts the local owner's cards that are due now.
    pub fn count_due_flashcards(&self) -> Result<i64, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::count_due_flashcards(&database)
    }

    /// Cria um novo cartão de memorização associado a uma mídia.
    pub fn create_flashcard(
        &self,
        media_id: &str,
        front: &str,
        back: &str,
        position_seconds: Option<i64>,
    ) -> Result<FlashcardDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::create_flashcard(&database, media_id, front, back, position_seconds)
    }

    /// Atualiza a frente e o verso de um cartão de memorização existente.
    pub fn update_flashcard(
        &self,
        card_id: &str,
        front: &str,
        back: &str,
    ) -> Result<FlashcardDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::update_flashcard(&database, card_id, front, back)
    }

    /// Remove um cartão de memorização pertencente ao usuário local.
    pub fn delete_flashcard(&self, card_id: &str) -> Result<(), IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::delete_flashcard(&database, card_id)
    }

    /// Registra uma revisão em um cartão de memorização pertencente ao usuário local.
    pub fn review_flashcard(&self, card_id: &str, rating: &str) -> Result<FlashcardDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let result = study::review_flashcard(&database, card_id, rating)?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Lista os cursos e aulas favoritados pelo dono local.
    pub fn list_favorites(&self) -> Result<StudyFavoritesDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_favorites(&database)
    }

    /// Marca ou desmarca um curso como favorito para o dono local.
    pub fn set_course_favorite(&self, course_id: &str, favorite: bool) -> Result<bool, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::set_course_favorite(&database, course_id, favorite)
    }

    /// Marca ou desmarca uma aula (mídia) como favorita para o dono local.
    pub fn set_lesson_favorite(&self, media_id: &str, favorite: bool) -> Result<bool, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::set_lesson_favorite(&database, media_id, favorite)
    }

    /// Persiste uma sessão Pomodoro do dono local.
    pub fn record_focus_session(
        &self,
        input: learnvault_core::FocusSessionInput,
    ) -> Result<FocusSessionDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::record_focus_session(&database, input)
    }

    /// Lista as sessões de foco persistidas do dono local, da mais recente para a mais antiga.
    pub fn list_focus_sessions(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<FocusSessionDto>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_focus_sessions(&database, limit)
    }

    /// Lê o resumo de foco persistido do dono local.
    pub fn focus_summary(&self) -> Result<FocusSummaryDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::focus_summary(&database)
    }

    /// Lê o resumo de recompensas persistido do dono local.
    pub fn reward_summary(&self) -> Result<RewardSummaryDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::reward_summary(&database)
    }

    /// Resgata uma missão diária do dono local.
    pub fn claim_daily_mission(&self, mission_id: &str) -> Result<bool, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::claim_daily_mission(&database, mission_id)
    }

    /// Lê a meta semanal configurada do dono local.
    pub fn weekly_goal(&self) -> Result<i64, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::weekly_goal(&database)
    }

    /// Salva a meta semanal configurada do dono local.
    pub fn set_weekly_goal(&self, goal: i64) -> Result<i64, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::set_weekly_goal(&database, goal)
    }

    /// Consulta o impacto da remoção de uma mídia do catálogo local.
    pub fn media_removal_impact(&self, media_id: &str) -> Result<MediaRemovalImpactDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let library_id = learnvault_core::find_media_library_id(&database, media_id)
            .map_err(|_| IpcError::database_unavailable())?
            .ok_or_else(IpcError::media_not_found)?;
        let impact = learnvault_core::media_removal_impact(&database, &library_id, media_id)
            .map_err(|err| match err {
                learnvault_core::Error::MediaNotFound { .. } => IpcError::media_not_found(),
                _ => IpcError::database_unavailable(),
            })?;
        Ok(impact.into())
    }

    /// Remove explicitamente uma mídia do catálogo local (tombstone persistente).
    pub fn remove_media(
        &self,
        media_id: &str,
        confirm_progress: bool,
    ) -> Result<MediaRemovalOutcomeDto, IpcError> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let library_id = learnvault_core::find_media_library_id(&database, media_id)
            .map_err(|_| IpcError::database_unavailable())?
            .ok_or_else(IpcError::media_not_found)?;
        let outcome =
            learnvault_core::remove_media(&mut database, &library_id, media_id, confirm_progress)
                .map_err(|err| match err {
                learnvault_core::Error::MediaRemovalNeedsConfirmation { .. } => {
                    IpcError::media_removal_needs_confirmation()
                }
                learnvault_core::Error::MediaNotFound { .. } => IpcError::media_not_found(),
                _ => IpcError::media_removal_unavailable(),
            })?;
        Ok(outcome.into())
    }

    /// Abre a mídia especificada no reprodutor padrão do sistema operacional.
    pub fn open_in_system_player(&self, media_id: &str) -> Result<bool, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let courses = self.courses();
        crate::command::open_in_system_player(&database, &courses, media_id)
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

    /// Lê as pastas de curso registradas e aplica a varredura a cada catálogo local.
    ///
    /// Só é executado por ação explícita: nem a inicialização nem a importação o chamam.
    /// Sem cursos registrados devolve [`IpcError::no_courses_registered`]; um curso cuja
    /// pasta sumiu, virou link/junction ou ficou inacessível conta como falha na agregação
    /// e não impede os demais. Cada raiz é revalidada imediatamente antes da varredura e
    /// nenhum cadeado da configuração é mantido durante a leitura; o banco permanece fora
    /// das pastas e elas permanecem somente leitura. Chamadas concorrentes devolvem
    /// [`IpcError::rescan_in_progress`] sem tocar o catálogo.
    ///
    /// Limitação conhecida desta fatia: o cadeado do banco SQLite é mantido durante toda a
    /// varredura, então uma leitura de catálogo concorrente aguarda a indexação terminar.
    /// O estado ocupado recusa um segundo rescan, mas não paraleliza a leitura do catálogo.
    pub fn rescan(&self) -> Result<RescanSummaryDto, IpcError> {
        let _guard = self.begin_rescan()?;

        let courses = self.courses();
        if courses.is_empty() {
            return Err(IpcError::no_courses_registered());
        }

        let mut database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;

        let mut summary = RescanSummaryDto {
            complete: true,
            courses: courses.len(),
            failed: 0,
            inserted: 0,
            updated: 0,
            marked_missing: 0,
            preserved: 0,
            ambiguous_count: 0,
        };
        for course in &courses {
            // Revalida a raiz imediatamente antes de varrer: se o diretório foi removido,
            // substituído por um link/junction ou deixou de ser legível desde a importação,
            // a varredura é recusada sem seguir o alvo.
            let Ok(root) = CourseRoot::resolve(course.root.as_path()) else {
                summary.failed += 1;
                summary.complete = false;
                continue;
            };
            match learnvault_core::rescan(&mut database, &course.id, root.as_path()) {
                Ok(course_summary) => {
                    let _ = crate::covers::auto_detect_course_cover(
                        &self.covers_dir,
                        &course.id,
                        root.as_path(),
                    );
                    summary.complete = summary.complete && course_summary.complete;
                    summary.inserted += course_summary.inserted;
                    summary.updated += course_summary.updated;
                    summary.marked_missing += course_summary.marked_missing;
                    summary.preserved += course_summary.preserved;
                    summary.ambiguous_count += course_summary.ambiguous.len();
                }
                Err(_) => {
                    summary.failed += 1;
                    summary.complete = false;
                }
            }
        }
        Ok(summary)
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
