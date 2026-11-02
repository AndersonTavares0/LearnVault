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

use crate::command::catalog_with_orders;
use crate::config::{DesktopConfigDto, database_path};
use crate::courses::{CourseEntry, load_courses, store_courses};
use crate::covers::{
    COVERS_DIR_NAME, CoverError, cover_version, read_picked_cover, remove_cover, store_cover,
};
use crate::dto::{
    CatalogDto, CoverDto, CoverRemovalDto, FocusSessionDto, FocusSummaryDto, MediaRemovalImpactDto,
    MediaRemovalOutcomeDto, RecentStudyDto, RescanSummaryDto, RewardSummaryDto, StudyProgressDto,
    StudySummaryDto,
};
use crate::error::IpcError;
use crate::ordering::{CourseOrders, load_orders, store_orders};
use crate::study;

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
        let database = Database::open(database_path(data_dir))
            .map_err(|_| IpcError::database_unavailable())?;
        let courses = load_courses(config_dir);
        let orders = load_orders(data_dir);

        Ok(Self {
            database: Arc::new(Mutex::new(database)),
            config_dir: config_dir.to_path_buf(),
            covers_dir: data_dir.join(COVERS_DIR_NAME),
            orders_dir: data_dir.to_path_buf(),
            courses: Arc::new(Mutex::new(courses)),
            orders: Arc::new(Mutex::new(orders)),
            rescanning: Arc::new(AtomicBool::new(false)),
        })
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

    /// Resume a ofensiva e a semana corrente do dono local.
    pub fn study_summary(&self) -> Result<StudySummaryDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::summary(&database)
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
