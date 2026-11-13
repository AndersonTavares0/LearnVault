//! Indexação automática e incremental das bibliotecas registradas.
//!
//! O trabalho roda fora do caminho de requisição, com um único job por vez: eventos do
//! sistema de arquivos (com debounce) disparam uma execução, e um intervalo periódico
//! reconcilia o catálogo mesmo quando o sistema não entrega eventos — o caso de montagens
//! de rede, onde o observador só degrada a latência e nunca a correção.
//!
//! Cada curso é varrido sem segurar o banco e aplicado em uma transação própria; entre
//! cursos o banco é liberado e a reserva de trabalho pesado é devolvida, para que uma
//! requisição normal nunca espere a biblioteca inteira. A gravação usa a mesma política de
//! rescan do domínio, então um arquivo ausente é apenas marcado como indisponível e nenhum
//! histórico é apagado.
//!
//! O status é mantido em memória, não no banco: é observação efêmera da execução atual, e
//! o histórico persistente continua sendo a tabela `scan` do domínio.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{CourseRoot, KeyMode, RescanSummary, ScanOutcome};
use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use tokio::sync::{Mutex, Notify};

use crate::{
    AppState, StateError,
    auth::same_origin,
    catalog::{
        authenticated_user, credentials_required, csrf_failed, private_no_store,
        service_unavailable, status_private_no_store,
    },
    courses::RegisteredCourse,
};

/// Intervalo da reconciliação periódica, inclusive quando o observador está ativo.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(300);

/// Janela de silêncio que agrupa uma rajada de eventos em uma única varredura.
const DEBOUNCE: Duration = Duration::from_millis(1_500);

/// Situação do job de indexação.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IndexingStatus {
    /// Nenhum job executou desde o início do processo.
    #[default]
    Idle,
    /// O job está em execução.
    Running,
    /// O job terminou de percorrer os cursos.
    Done,
    /// O job foi interrompido por um pedido de cancelamento.
    Cancelled,
    /// O job terminou com erro em pelo menos um curso.
    Error,
}

/// Contagem de arquivos classificada pela última execução.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct IndexingProgress {
    /// Arquivos vistos nas varreduras concluídas.
    pub files_seen: u64,
    /// Arquivos novos inseridos no catálogo.
    pub added: u64,
    /// Arquivos cujo conteúdo ou metadados mudou.
    pub changed: u64,
    /// Arquivos marcados como ausentes.
    pub removed: u64,
    /// Arquivos que mudaram de caminho mantendo a identidade.
    pub moved: u64,
}

/// Retrato imutável do job, como a API o expõe.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct IndexingSnapshot {
    pub status: IndexingStatus,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub progress: IndexingProgress,
    pub courses_total: usize,
    pub courses_completed: usize,
    /// Código estável do último erro; nunca contém caminho nem mensagem do sistema.
    pub last_error: Option<&'static str>,
}

/// Estado compartilhado do job de indexação.
#[derive(Clone, Debug, Default)]
pub(crate) struct Indexer {
    inner: Arc<IndexerState>,
}

#[derive(Debug, Default)]
struct IndexerState {
    snapshot: Mutex<IndexingSnapshot>,
    cancel: AtomicBool,
    wake: Arc<Notify>,
}

impl Indexer {
    /// Inicia o laço de indexação em segundo plano.
    pub(crate) fn start(&self, app: AppState) {
        let indexer = self.clone();
        tokio::spawn(async move { indexer.run(app).await });
    }

    /// Retrato atual do job.
    pub(crate) async fn snapshot(&self) -> IndexingSnapshot {
        self.inner.snapshot.lock().await.clone()
    }

    /// Pede o cancelamento do job em execução; devolve `false` quando não há job ativo.
    pub(crate) async fn cancel(&self) -> bool {
        // A bandeira é gravada sob o mesmo cadeado da conclusão: ou o job ainda está em
        // execução e verá o pedido, ou já terminou e o pedido é recusado. Nunca sobra uma
        // bandeira ligada para a execução seguinte.
        let snapshot = self.inner.snapshot.lock().await;
        if snapshot.status != IndexingStatus::Running {
            return false;
        }
        self.inner.cancel.store(true, Ordering::SeqCst);
        true
    }

    /// Laço único: reconciliação periódica e eventos debounced compartilham o mesmo job.
    async fn run(self, app: AppState) {
        let mut watch = self.watch(&app);
        let mut timer = tokio::time::interval(RECONCILE_INTERVAL);
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = self.inner.wake.notified() => tokio::time::sleep(DEBOUNCE).await,
                _ = timer.tick() => {}
            }
            if let Some(watch) = watch.as_mut() {
                watch.sync(&app.courses_snapshot());
            }
            self.execute(&app).await;
        }
    }

    /// Um observador que não pode ser criado (limite do inotify, sistema sem suporte)
    /// simplesmente deixa a reconciliação periódica como único gatilho.
    fn watch(&self, app: &AppState) -> Option<CourseWatch> {
        let wake = self.inner.wake.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if event.is_ok() {
                wake.notify_one();
            }
        })
        .ok()?;
        let mut watch = CourseWatch {
            watcher,
            roots: Vec::new(),
        };
        watch.sync(&app.courses_snapshot());
        Some(watch)
    }

    /// Executa uma varredura completa. Um rescan manual em andamento adia esta execução.
    async fn execute(&self, app: &AppState) {
        // O mesmo cadeado do rescan manual garante um único escritor do catálogo por vez.
        let Ok(_manual_rescan) = app.rescan_lock.clone().try_lock_owned() else {
            return;
        };

        let courses = app.courses_snapshot();
        {
            let mut snapshot = self.inner.snapshot.lock().await;
            *snapshot = IndexingSnapshot {
                status: IndexingStatus::Running,
                started_at: Some(unix_now()),
                courses_total: courses.len(),
                ..IndexingSnapshot::default()
            };
        }

        let mut progress = IndexingProgress::default();
        let mut completed = 0usize;
        let mut error: Option<&'static str> = None;

        for course in &courses {
            if self.inner.cancel.load(Ordering::SeqCst) {
                break;
            }
            let Ok(root) = CourseRoot::resolve(course.root.as_path()) else {
                error = Some("course_unavailable");
                continue;
            };
            let Some(outcome) = scan_course(app, root.as_path()).await else {
                error = Some("course_unavailable");
                continue;
            };
            progress.files_seen += outcome.files.len() as u64;
            match apply_course(app, &course.id, outcome, root.as_path()).await {
                Some(summary) => {
                    progress.added += summary.inserted as u64;
                    progress.changed += summary.changed as u64;
                    progress.removed += summary.marked_missing as u64;
                    progress.moved += summary.moved as u64;
                    completed += 1;
                }
                None => error = Some("indexing_failed"),
            }
            self.publish(progress.clone(), completed).await;
        }

        // A conclusão e o cancelamento disputam o mesmo cadeado do retrato: um pedido de
        // cancelamento aceito enquanto o status é `running` nunca se perde, e a bandeira
        // nunca sobra para a execução seguinte.
        let mut snapshot = self.inner.snapshot.lock().await;
        let cancel_requested = self.inner.cancel.swap(false, Ordering::SeqCst);
        let cancelled = cancel_requested && completed < courses.len();
        snapshot.status = if cancelled {
            IndexingStatus::Cancelled
        } else if error.is_some() {
            IndexingStatus::Error
        } else {
            IndexingStatus::Done
        };
        snapshot.finished_at = Some(unix_now());
        snapshot.last_error = if cancelled { None } else { error };
    }

    /// Publica o progresso sem sobrescrever um status já finalizado.
    async fn publish(&self, progress: IndexingProgress, completed: usize) {
        let mut snapshot = self.inner.snapshot.lock().await;
        if snapshot.status == IndexingStatus::Running {
            snapshot.progress = progress;
            snapshot.courses_completed = completed;
        }
    }
}

/// Raízes observadas pelo sistema de arquivos.
struct CourseWatch {
    watcher: notify::RecommendedWatcher,
    roots: Vec<PathBuf>,
}

impl CourseWatch {
    /// Reassina o observador apenas quando o registro de cursos mudou.
    ///
    /// Uma raiz que o sistema recusa (por exemplo uma montagem de rede sem suporte a
    /// eventos) continua coberta pela reconciliação periódica.
    fn sync(&mut self, courses: &[RegisteredCourse]) {
        let roots: Vec<PathBuf> = courses
            .iter()
            .map(|course| course.root.as_path().to_path_buf())
            .collect();
        if roots == self.roots {
            return;
        }
        for root in &self.roots {
            if !roots.contains(root) {
                let _ = self.watcher.unwatch(root);
            }
        }
        for root in &roots {
            if !self.roots.contains(root) {
                let _ = self.watcher.watch(root, RecursiveMode::Recursive);
            }
        }
        self.roots = roots;
    }
}

/// Varre um curso fora das threads Tokio, sem segurar o banco.
async fn scan_course(app: &AppState, root: &Path) -> Option<ScanOutcome> {
    let permit = app.try_acquire_blocking_task()?;
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        learnvault_core::scan(&root).ok()
    })
    .await
    .ok()
    .flatten()
}

/// Aplica um curso em uma transação própria, liberando o banco entre cursos.
async fn apply_course(
    app: &AppState,
    library_id: &str,
    outcome: ScanOutcome,
    root: &Path,
) -> Option<RescanSummary> {
    let database = app.database.clone().lock_owned().await;
    let permit = app.try_acquire_blocking_task()?;
    let library_id = library_id.to_owned();
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut database = database;
        // O modo vem do host, o mesmo usado pelo scanner da varredura.
        learnvault_core::apply_incremental_scan(
            &mut database,
            &library_id,
            &outcome,
            KeyMode::for_host(),
            &root,
        )
        .ok()
    })
    .await
    .ok()
    .flatten()
}

/// `GET /api/indexing` — situação do job para qualquer sessão autenticada.
pub(crate) async fn status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => {
            private_no_store(Json(state.indexer.snapshot().await).into_response())
        }
        Ok(Some(_)) => credentials_required(),
        Err(StateError::Capacity) => service_unavailable(),
        Ok(None) | Err(_) => status_private_no_store(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Serialize)]
struct CancelResponse {
    cancelled: bool,
}

/// `POST /api/indexing/cancel` — interrompe o job em execução (somente administrador).
pub(crate) async fn cancel(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !user.is_admin {
        return status_private_no_store(StatusCode::FORBIDDEN);
    }
    if user.must_change_credentials {
        return credentials_required();
    }
    if state.indexer.cancel().await {
        private_no_store(Json(CancelResponse { cancelled: true }).into_response())
    } else {
        status_private_no_store(StatusCode::CONFLICT)
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{Indexer, IndexingProgress, IndexingStatus};
    use crate::{AppState, Config, courses::RegisteredCourse};

    fn state(root: &TempDir) -> AppState {
        let config = Config {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            database_path: root.path().join("data").join("learnvault.sqlite"),
            library_root: None,
            web_root: None,
            admin_username: None,
            admin_password: None,
            cookie_secure: false,
            update_check: false,
        };
        AppState::new(config)
    }

    fn register(state: &AppState, root: &TempDir) {
        fs::create_dir_all(root.path().join("Curso")).unwrap();
        let course_root = learnvault_core::CourseRoot::resolve(&root.path().join("Curso")).unwrap();
        state.courses.write().unwrap().push(RegisteredCourse {
            id: "curso-1".to_owned(),
            title: "Curso".to_owned(),
            root: course_root,
        });
    }

    fn media_count(state: &AppState) -> i64 {
        state
            .database
            .try_lock()
            .unwrap()
            .connection()
            .query_row("SELECT COUNT(*) FROM media", [], |row| row.get(0))
            .unwrap()
    }

    #[tokio::test]
    async fn an_empty_library_finishes_with_no_progress() {
        let root = tempfile::tempdir().unwrap();
        let state = state(&root);
        let indexer = Indexer::default();

        indexer.execute(&state).await;

        let snapshot = indexer.snapshot().await;
        assert_eq!(snapshot.status, IndexingStatus::Done);
        assert_eq!(snapshot.courses_total, 0);
        assert!(snapshot.started_at.is_some() && snapshot.finished_at.is_some());
        assert_eq!(snapshot.progress, IndexingProgress::default());
    }

    #[tokio::test]
    async fn new_changed_and_removed_files_are_counted_across_runs() {
        let root = tempfile::tempdir().unwrap();
        let state = state(&root);
        register(&state, &root);
        let indexer = Indexer::default();
        let lesson = root.path().join("Curso/aula.mp4");

        fs::write(&lesson, b"primeira versao").unwrap();
        indexer.execute(&state).await;
        assert_eq!(indexer.snapshot().await.progress.added, 1);
        assert_eq!(media_count(&state), 1);

        fs::write(&lesson, b"segunda versao, maior").unwrap();
        indexer.execute(&state).await;
        let changed = indexer.snapshot().await;
        assert_eq!(changed.progress.changed, 1, "o job anterior é substituído");
        assert_eq!(changed.progress.added, 0);

        fs::remove_file(&lesson).unwrap();
        indexer.execute(&state).await;
        assert_eq!(indexer.snapshot().await.progress.removed, 1);
        assert_eq!(media_count(&state), 1, "nenhuma linha é apagada");
    }

    #[tokio::test]
    async fn cancelling_before_the_job_keeps_the_catalogue_untouched() {
        let root = tempfile::tempdir().unwrap();
        let state = state(&root);
        register(&state, &root);
        fs::write(root.path().join("Curso/aula.mp4"), b"conteudo").unwrap();
        let indexer = Indexer::default();

        // Simula um cancelamento aceito enquanto o job ainda estava em execução.
        indexer.inner.snapshot.lock().await.status = IndexingStatus::Running;
        assert!(indexer.cancel().await);
        indexer.execute(&state).await;

        assert_eq!(indexer.snapshot().await.status, IndexingStatus::Cancelled);
        assert_eq!(
            media_count(&state),
            0,
            "o cancelamento não aplica a varredura"
        );
    }

    #[tokio::test]
    async fn a_cancelled_flag_does_not_leak_into_the_next_run() {
        let root = tempfile::tempdir().unwrap();
        let state = state(&root);
        register(&state, &root);
        fs::write(root.path().join("Curso/aula.mp4"), b"conteudo").unwrap();
        let indexer = Indexer::default();

        indexer
            .inner
            .cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
        indexer.execute(&state).await;
        assert_eq!(indexer.snapshot().await.status, IndexingStatus::Cancelled);

        indexer.execute(&state).await;
        let snapshot = indexer.snapshot().await;
        assert_eq!(snapshot.status, IndexingStatus::Done);
        assert_eq!(snapshot.progress.added, 1);
    }

    #[tokio::test]
    async fn an_unavailable_course_marks_the_job_as_failed() {
        let root = tempfile::tempdir().unwrap();
        let state = state(&root);
        let missing = tempfile::tempdir().unwrap();
        let missing_root = missing.path().join("Curso");
        fs::create_dir_all(&missing_root).unwrap();
        state.courses.write().unwrap().push(RegisteredCourse {
            id: "curso-1".to_owned(),
            title: "Curso".to_owned(),
            root: learnvault_core::CourseRoot::resolve(&missing_root).unwrap(),
        });
        fs::remove_dir_all(&missing_root).unwrap();
        let indexer = Indexer::default();

        indexer.execute(&state).await;

        let snapshot = indexer.snapshot().await;
        assert_eq!(snapshot.status, IndexingStatus::Error);
        assert_eq!(snapshot.last_error, Some("course_unavailable"));
        assert_eq!(snapshot.courses_completed, 0);
    }
}
