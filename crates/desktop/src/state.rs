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

use learnvault_core::{CourseRoot, Database};

use crate::command::{catalog, reorder_catalog, reset_catalog_order};
use crate::config::{DesktopConfigDto, database_path};
use crate::courses::{CourseEntry, load_courses, store_courses};
use crate::dto::{
    CatalogDto, MediaRemovalImpactDto, MediaRemovalOutcomeDto, RecentStudyDto, RescanSummaryDto,
    StudyAnalyticsDto, StudyProgressDto,
};
use crate::error::IpcError;
use crate::study;

/// Estado Tauri do desktop: banco local e cursos registrados.
#[derive(Clone)]
pub struct DesktopState {
    database: Arc<Mutex<Database>>,
    config_dir: PathBuf,
    courses: Arc<Mutex<Vec<CourseEntry>>>,
    rescanning: Arc<AtomicBool>,
}

impl DesktopState {
    /// Abre o banco local e carrega os cursos registrados do diretório de configuração.
    ///
    /// O diretório de dados pertence ao aplicativo; as pastas de curso são externas e
    /// nunca são criadas, lidas nem escritas por esta operação. A configuração é
    /// revalidada: cursos com raiz removida ou inválida são descartados, sem falha fatal.
    pub fn open(data_dir: &Path, config_dir: &Path) -> Result<Self, IpcError> {
        std::fs::create_dir_all(data_dir).map_err(|_| IpcError::database_unavailable())?;
        let database = Database::open(database_path(data_dir))
            .map_err(|_| IpcError::database_unavailable())?;
        let courses = load_courses(config_dir);

        Ok(Self {
            database: Arc::new(Mutex::new(database)),
            config_dir: config_dir.to_path_buf(),
            courses: Arc::new(Mutex::new(courses)),
            rescanning: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Constrói o catálogo a partir do banco local.
    pub fn catalog(&self) -> Result<CatalogDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        catalog(&database, &self.courses())
    }

    /// Persiste a ordem dos irmãos selecionados para o dono local do desktop.
    pub fn reorder_catalog(
        &self,
        course_id: &str,
        parent_path: &str,
        child_paths: &[String],
    ) -> Result<(), IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        reorder_catalog(
            &database,
            &self.courses(),
            course_id,
            parent_path,
            child_paths,
        )
    }

    /// Remove a preferência da lista selecionada para o dono local do desktop.
    pub fn reset_catalog_order(&self, course_id: &str, parent_path: &str) -> Result<(), IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        reset_catalog_order(&database, &self.courses(), course_id, parent_path)
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
        study::record_position(&database, media_id, position_seconds)
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
        study::set_completed(&database, media_id, completed)
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

    /// Consulta métricas agregadas do estudo do dono local.
    pub fn study_analytics(&self) -> Result<StudyAnalyticsDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::analytics(&database)
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
