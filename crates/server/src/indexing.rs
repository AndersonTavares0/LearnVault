//! Automatic, incremental indexing of the registered libraries.
//!
//! The work runs off the request path, with a single job at a time: file-system events
//! (debounced) trigger a run, and a periodic interval reconciles the catalog even when the
//! system does not deliver events — the case of network mounts, where the watcher only
//! degrades latency and never correctness.
//!
//! Each course is scanned without holding the database and applied in a transaction of its own; between
//! courses the database is released and the heavy-work reservation is returned, so that an
//! ordinary request never waits for the whole library. The write uses the same
//! rescan policy as the domain, so a missing file is only marked as unavailable and no
//! history is erased.
//!
//! The status is kept in memory, not in the database: it is an ephemeral observation of the current run, and
//! the persistent history remains the domain's `scan` table.

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
        scan_registered_course, service_unavailable, status_private_no_store,
    },
    courses::RegisteredCourse,
};

/// Interval of the periodic reconciliation, including when the watcher is active.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(300);

/// Quiet window that groups a burst of events into a single scan.
const DEBOUNCE: Duration = Duration::from_millis(1_500);

/// State of the indexing job.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IndexingStatus {
    /// No job has run since the process started.
    #[default]
    Idle,
    /// The job is running.
    Running,
    /// The job finished walking the courses.
    Done,
    /// The job was interrupted by a cancellation request.
    Cancelled,
    /// The job finished with an error in at least one course.
    Error,
}

/// Count of files classified by the last run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct IndexingProgress {
    /// Files seen in the completed scans.
    pub files_seen: u64,
    /// New files inserted into the catalog.
    pub added: u64,
    /// Files whose content or metadata changed.
    pub changed: u64,
    /// Files marked as missing.
    pub removed: u64,
    /// Files that changed path while keeping their identity.
    pub moved: u64,
}

/// Immutable snapshot of the job, as the API exposes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct IndexingSnapshot {
    pub status: IndexingStatus,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub progress: IndexingProgress,
    pub courses_total: usize,
    pub courses_completed: usize,
    /// Stable code of the last error; it never contains a path nor a system message.
    pub last_error: Option<&'static str>,
}

/// Shared state of the indexing job.
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
    /// Starts the indexing loop in the background.
    pub(crate) fn start(&self, app: AppState) {
        let indexer = self.clone();
        tokio::spawn(async move { indexer.run(app).await });
    }

    /// Current snapshot of the job.
    pub(crate) async fn snapshot(&self) -> IndexingSnapshot {
        self.inner.snapshot.lock().await.clone()
    }

    /// Requests cancellation of the running job; returns `false` when there is no active job.
    pub(crate) async fn cancel(&self) -> bool {
        // The flag is written under the same lock as the completion: either the job is still
        // running and will see the request, or it has already finished and the request is refused. A
        // flag is never left on for the next run.
        let snapshot = self.inner.snapshot.lock().await;
        if snapshot.status != IndexingStatus::Running {
            return false;
        }
        self.inner.cancel.store(true, Ordering::SeqCst);
        true
    }

    /// Single loop: periodic reconciliation and debounced events share the same job.
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

    /// A watcher that cannot be created (inotify limit, unsupported system)
    /// simply leaves the periodic reconciliation as the only trigger.
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

    /// Runs a full scan. A manual rescan in progress postpones this run.
    async fn execute(&self, app: &AppState) {
        // The same lock as the manual rescan guarantees a single catalog writer at a time.
        // A manual rescan in progress only postpones this run: giving up here would discard
        // the event that woke the job and delay the update until the periodic reconciliation.
        let _manual_rescan = loop {
            match app.rescan_lock.clone().try_lock_owned() {
                Ok(guard) => break guard,
                Err(_) => tokio::time::sleep(DEBOUNCE).await,
            }
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
            let Some((root, outcome)) = scan_course(app, course.root.as_path()).await else {
                error = Some("course_unavailable");
                continue;
            };
            progress.files_seen += outcome.files.len() as u64;
            match apply_course(app, &course.id, outcome, &course.root, root).await {
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

        // The completion and the cancellation contend for the same snapshot lock: a cancellation
        // request accepted while the status is `running` is never lost, and the flag
        // is never left over for the next run.
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

    /// Publishes progress without overwriting an already-finalized status.
    async fn publish(&self, progress: IndexingProgress, completed: usize) {
        let mut snapshot = self.inner.snapshot.lock().await;
        if snapshot.status == IndexingStatus::Running {
            snapshot.progress = progress;
            snapshot.courses_completed = completed;
        }
    }
}

/// Roots watched by the file system.
struct CourseWatch {
    watcher: notify::RecommendedWatcher,
    roots: Vec<PathBuf>,
}

impl CourseWatch {
    /// Resubscribes the watcher only when the course registry has changed.
    ///
    /// A root the system refuses (for example a network mount without support for
    /// events) is still covered by the periodic reconciliation.
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

/// Scans a course off the Tokio threads, without holding the database.
async fn scan_course(
    app: &AppState,
    course_path: &Path,
) -> Option<(Option<CourseRoot>, ScanOutcome)> {
    let permit = app.try_acquire_blocking_task()?;
    let course_path = course_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let (root, outcome) = scan_registered_course(&course_path)?;
        Some((root, outcome.ok()?))
    })
    .await
    .ok()
    .flatten()
}

/// Applies a course in a transaction of its own, releasing the database between courses.
async fn apply_course(
    app: &AppState,
    library_id: &str,
    outcome: ScanOutcome,
    registered_root: &CourseRoot,
    root: Option<CourseRoot>,
) -> Option<RescanSummary> {
    let database = app.database.clone().lock_owned().await;
    let permit = app.try_acquire_blocking_task()?;
    let library_id = library_id.to_owned();
    let registered_root = registered_root.clone();
    let covers_dir = crate::covers::covers_dir(&app.config.database_path);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut database = database;
        // The mode comes from the host, the same one the scan's scanner uses.
        let summary = learnvault_core::apply_incremental_scan(
            &mut database,
            &library_id,
            &outcome,
            KeyMode::for_host(),
            root.as_ref().unwrap_or(&registered_root).as_path(),
        )
        .ok()?;
        if let Some(root) = root {
            let _ =
                crate::covers::auto_detect_course_cover(&covers_dir, &library_id, root.as_path());
        }
        Some(summary)
    })
    .await
    .ok()
    .flatten()
}

/// `GET /api/indexing` — state of the job for any authenticated session.
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

/// `POST /api/indexing/cancel` — interrupts the running job (administrator only).
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
    use std::time::Duration;

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

        // Simulates a cancellation accepted while the job was still running.
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
        let missing_root = missing.path().join("library/Curso");
        fs::create_dir_all(&missing_root).unwrap();
        state.courses.write().unwrap().push(RegisteredCourse {
            id: "curso-1".to_owned(),
            title: "Curso".to_owned(),
            root: learnvault_core::CourseRoot::resolve(&missing_root).unwrap(),
        });
        fs::remove_dir_all(missing.path().join("library")).unwrap();
        let indexer = Indexer::default();

        indexer.execute(&state).await;

        let snapshot = indexer.snapshot().await;
        assert_eq!(snapshot.status, IndexingStatus::Error);
        assert_eq!(snapshot.last_error, Some("course_unavailable"));
        assert_eq!(snapshot.courses_completed, 0);
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains(missing.path().to_str().unwrap()));
    }

    #[tokio::test]
    async fn a_renamed_course_root_is_marked_missing_and_restored_with_progress_intact() {
        let root = tempfile::tempdir().unwrap();
        let state = state(&root);
        register(&state, &root);
        let indexer = Indexer::default();
        let course = root.path().join("Curso");
        let moved = root.path().join("Curso movido");
        fs::write(course.join("aula.mp4"), b"conteudo").unwrap();

        indexer.execute(&state).await;
        assert_eq!(indexer.snapshot().await.progress.added, 1);

        let media_id: String = {
            let db = state.database.try_lock().unwrap();
            let media_id: String = db
                .connection()
                .query_row("SELECT media_id FROM media", [], |row| row.get(0))
                .unwrap();
            learnvault_core::record_position(&db, "owner-1", &media_id, 42).unwrap();
            learnvault_core::set_completed(&db, "owner-1", &media_id, true).unwrap();
            media_id
        };

        fs::rename(&course, &moved).unwrap();
        indexer.execute(&state).await;
        let after_rename = indexer.snapshot().await;
        assert_eq!(after_rename.status, IndexingStatus::Done);
        assert_eq!(after_rename.last_error, None);
        assert_eq!(after_rename.courses_completed, 1);
        assert_eq!(after_rename.progress.removed, 1);
        let serialized = serde_json::to_string(&after_rename).unwrap();
        assert!(!serialized.contains(root.path().to_str().unwrap()));
        assert!(!serialized.contains("Curso"));

        {
            let db = state.database.try_lock().unwrap();
            let availability: String = db
                .connection()
                .query_row(
                    "SELECT availability FROM media WHERE media_id = ?1",
                    [&media_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(availability, "missing");
        }

        fs::rename(&moved, &course).unwrap();
        indexer.execute(&state).await;
        let after_restore = indexer.snapshot().await;
        assert_eq!(after_restore.status, IndexingStatus::Done);
        assert_eq!(after_restore.last_error, None);
        assert_eq!(after_restore.courses_completed, 1);
        assert_eq!(after_restore.progress.added, 0);

        {
            let db = state.database.try_lock().unwrap();
            let availability: String = db
                .connection()
                .query_row(
                    "SELECT availability FROM media WHERE media_id = ?1",
                    [&media_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(availability, "available");
            let progress = learnvault_core::study_progress(&db, "owner-1", &media_id)
                .unwrap()
                .expect("study progress must stay intact");
            assert_eq!(progress.position_seconds, 42);
            assert!(progress.completed);
        }
    }

    #[tokio::test]
    async fn a_manual_rescan_defers_the_job_instead_of_dropping_it() {
        let root = tempfile::tempdir().unwrap();
        let state = state(&root);
        register(&state, &root);
        fs::write(root.path().join("Curso/aula.mp4"), b"conteudo").unwrap();
        let indexer = Indexer::default();

        let manual_rescan = state.rescan_lock.clone().lock_owned().await;
        let job = tokio::spawn({
            let state = state.clone();
            let indexer = indexer.clone();
            async move { indexer.execute(&state).await }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            indexer.snapshot().await.status,
            IndexingStatus::Idle,
            "o job espera o rescan manual em vez de desistir do evento"
        );

        drop(manual_rescan);
        job.await.unwrap();

        assert_eq!(indexer.snapshot().await.status, IndexingStatus::Done);
        assert_eq!(media_count(&state), 1);
    }
}
