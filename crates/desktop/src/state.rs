//! Shared state of the desktop application.
//!
//! The state holds two independent resources: the local SQLite database protected by a
//! mutex and the list of registered courses, protected by a mutex of its own. Separating the
//! locks is deliberate: the native picker and configuration persistence cannot
//! hold the SQLite lock, and reading the catalog stays off the main thread.
//!
//! Each course is just a canonicalized path with a stable identity; the state never
//! writes inside it. The configuration is written to the application's configuration directory,
//! never to the database nor to the course folders.

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
    MediaSliceDto, NewPdfAnnotationDto, PdfAnnotationDto, PlaybackPreferencesDto, RecentStudyDto,
    RescanSummaryDto, RewardSummaryDto, SearchResultsDto, StudyFavoritesDto, StudyNoteDto,
    StudyProgressDto, StudyStateDto, StudySummaryDto, SyncResultDto, SyncStateDto,
};
use crate::error::IpcError;
use crate::ordering::{CourseOrders, load_orders, store_orders};
use crate::search;
use crate::study;
use crate::sync::{self, SyncFailure, SyncFlight, SyncSettings};

/// Tauri state of the desktop: local database and registered courses.
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

impl std::fmt::Debug for PendingUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingUpdate")
            .field("version", &self.version)
            .field("bytes_len", &self.bytes.len())
            .finish()
    }
}

impl DesktopState {
    /// Opens the local database and loads the registered courses from the configuration directory.
    ///
    /// The data directory belongs to the application; the course folders are external and
    /// are never created, read or written by this operation. The configuration is
    /// revalidated: courses with a removed or invalid root are dropped, without a fatal failure.
    /// Custom covers live in a covers subdirectory of the data directory,
    /// created on demand at the first write.
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

    /// Returns the version of the held update without consuming its bytes.
    pub(crate) fn pending_update_version(&self) -> Result<Option<String>, IpcError> {
        let guard = self
            .pending_update
            .lock()
            .map_err(|_| IpcError::update_unavailable())?;
        Ok(guard.as_ref().map(|pending| pending.version.clone()))
    }

    /// Clears the held update only if it still matches `expected_version`.
    pub(crate) fn clear_update_if_version(&self, expected_version: &str) -> Result<(), IpcError> {
        let mut guard = self
            .pending_update
            .lock()
            .map_err(|_| IpcError::update_unavailable())?;
        if guard
            .as_ref()
            .is_some_and(|pending| pending.version == expected_version)
        {
            *guard = None;
        }
        Ok(())
    }

    /// Takes the held update only if it still matches `expected_version`.
    pub(crate) fn take_update_if_version(
        &self,
        expected_version: &str,
    ) -> Result<Option<PendingUpdate>, IpcError> {
        let mut guard = self
            .pending_update
            .lock()
            .map_err(|_| IpcError::update_unavailable())?;
        if guard
            .as_ref()
            .is_some_and(|pending| pending.version == expected_version)
        {
            return Ok(guard.take());
        }
        Ok(None)
    }

    /// Restores a taken update only when no newer download has already replaced it.
    pub(crate) fn restore_update_if_empty(&self, pending: PendingUpdate) -> Result<(), IpcError> {
        let mut guard = self
            .pending_update
            .lock()
            .map_err(|_| IpcError::update_unavailable())?;
        if guard.is_none() {
            *guard = Some(pending);
        }
        Ok(())
    }

    /// Directory of the custom covers. Internal Rust use: it never crosses the IPC.
    pub(crate) fn covers_dir(&self) -> PathBuf {
        self.covers_dir.clone()
    }

    /// Builds the catalog from the local database.
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

    /// Immutable snapshot of the ordering preferences. Internal Rust use.
    fn orders_snapshot(&self) -> CourseOrders {
        self.orders
            .lock()
            .map(|orders| orders.clone())
            .unwrap_or_default()
    }

    /// Sets the manual order of the siblings of a course node.
    ///
    /// The list must be an exact permutation of the siblings at path `parent` (empty for
    /// the modules directly under the course): a path from outside or a partial list is
    /// refused with [`IpcError::invalid_order`]. The course must be registered. The order
    /// lives in the data directory; no course file is read beyond the catalog.
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

    /// Restores the automatic order of a specific parent (`Some`) or of the whole course (`None`).
    ///
    /// It is idempotent: restoring what was already automatic returns `false`.
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

    /// Runs the native image picker and stores the chosen course's cover.
    ///
    /// `pick` returns the raw file chosen by the user or `None` when the picker is
    /// cancelled. Cancelling preserves the previous cover and returns the current version. The course
    /// must be registered: an unknown or unsafe identity is refused without
    /// touching the disk or the picker. The file is reopened and revalidated in Rust (type by
    /// magic bytes and size limit) before the atomic write; nothing is written inside the
    /// course folder.
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

    /// Removes the custom cover of a registered course.
    ///
    /// The removal does not touch the course folder and is idempotent: a course with no cover returns
    /// `removed: false`. An unknown identity is refused with a stable error.
    pub fn remove_course_cover(&self, course_id: &str) -> Result<CoverRemovalDto, IpcError> {
        if self.course(course_id).is_none() {
            return Err(IpcError::course_not_found());
        }
        let removed =
            remove_cover(&self.covers_dir, course_id).map_err(|_| IpcError::cover_unavailable())?;
        Ok(CoverRemovalDto { removed })
    }

    /// Current state of a course's cover, with no path.
    fn cover_status(&self, course_id: &str) -> CoverDto {
        CoverDto {
            cover_version: cover_version(&self.covers_dir, course_id),
        }
    }

    /// Configuration state exposed to the webview: only the boolean, never a path.
    ///
    /// The roots are revalidated on every query so that a course that was removed, became inaccessible
    /// or was revoked by the sandbox is not still announced as configured during the
    /// process's life. The persisted configuration and the in-memory list are not erased
    /// here: a transient failure or a cancellation still preserve the previous
    /// state, while operations that use the roots do their own revalidation before
    /// proceeding.
    pub fn config_status(&self) -> DesktopConfigDto {
        DesktopConfigDto {
            configured: self
                .courses()
                .iter()
                .any(|course| CourseRoot::resolve(course.root.as_path()).is_ok()),
        }
    }

    /// Immutable snapshot of the registered courses. Internal Rust use.
    pub fn courses(&self) -> Vec<CourseEntry> {
        self.courses
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Registered course with the given identity, if there is one. Internal media use.
    pub fn course(&self, course_id: &str) -> Option<CourseEntry> {
        self.courses()
            .into_iter()
            .find(|course| course.id == course_id)
    }

    /// Takes the local database lock for a read. Internal use of media delivery.
    ///
    /// The lock serializes the catalog revalidation with concurrent removals during the
    /// opening of the media, exactly as in the HTTP adapter.
    pub(crate) fn lock_database(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, learnvault_core::Database>, IpcError> {
        self.database
            .lock()
            .map_err(|_| IpcError::database_unavailable())
    }

    /// Runs the native picker and persists the chosen folder as a course.
    ///
    /// `pick` returns the raw path chosen by the user or `None` when the picker is
    /// cancelled. Cancelling preserves the previous list and returns the current state. An
    /// invalid choice or a write failure also preserves the state and returns a
    /// sanitized IPC error. Choosing a folder already registered is idempotent: the list
    /// keeps a single record. In no case is the path written inside the
    /// course folder, and the SQLite lock is never acquired here.
    pub fn import_course(
        &self,
        pick: impl FnOnce() -> Option<PathBuf>,
    ) -> Result<DesktopConfigDto, IpcError> {
        let Some(picked) = pick() else {
            return Ok(self.config_status());
        };

        let root = CourseRoot::resolve(&picked).map_err(|_| IpcError::course_path_invalid())?;
        let entry = CourseEntry::from_root(root);

        // The list lock covers the write and the in-memory update: two concurrent
        // imports cannot end with memory diverging from the persisted file.
        // It is the configuration lock, never SQLite's.
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

    /// Removes a course from the registry, without touching the files.
    ///
    /// Removing an unknown identity is idempotent and does not change the list. The write
    /// and the in-memory update happen under the list lock.
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

    /// Reads the local owner's study progress on a known media.
    ///
    /// The owner is the desktop's fixed local identifier: the webview never sends it. The
    /// blocking SQLite read is serialized by the database lock, like the state's other operations,
    /// and it neither queries nor changes the catalog.
    pub fn study_progress(&self, media_id: &str) -> Result<StudyProgressDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::progress(&database, media_id)
    }

    /// Records the local owner's resume position, preserving completion.
    ///
    /// The rule of independent position and completion belongs to the core; the state only
    /// delegates with the fixed owner under the database lock.
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

    /// Marks or unmarks the local owner's completion, preserving the resume position.
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

    /// Lists the local owner's study progress, from newest to oldest.
    ///
    /// The limit is optional: without it, the adapter applies the contract's default and the
    /// core's ceiling still holds for any value received.
    pub fn recent_study(&self, limit: Option<usize>) -> Result<RecentStudyDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::recent(&database, limit)
    }

    /// Pages through the local owner's whole study state, newest first.
    pub fn study_state(
        &self,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<StudyStateDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::state(&database, limit, cursor)
    }

    /// Summarizes the local owner's streak and current week.
    pub fn study_summary(&self) -> Result<StudySummaryDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::summary(&database)
    }

    /// Reads the local owner's persisted playback preferences.
    pub fn playback_preference(&self) -> Result<PlaybackPreferencesDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::playback_preference(&database)
    }

    /// Writes the playback preferences and queues them for the server.
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

    /// Local identity of the device and the cursor already applied.
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

    /// Sets the server URL. Switching servers erases the stored session, so that
    /// a cookie from another origin is never sent to the new address.
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

    /// Signs in to the configured server and stores the session on disk.
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
            // The URL may have changed during the request; do not store a cookie from another origin.
            if guard.server_url != settings.server_url {
                return Err(IpcError::invalid_sync_server());
            }
            guard.session = session;
            sync::store_settings(&self.sync_file, &guard)?;
        }
        self.sync_state()
    }

    /// Ends the session on the server (best effort) and erases the local copy.
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

    /// Runs a full sync. It never overlaps another run: in that case it
    /// returns only the summary with `sync_in_progress`.
    pub fn sync_now(&self) -> Result<SyncResultDto, IpcError> {
        let Some(_flight) = self.sync_flight.try_begin() else {
            return self.summary_with_error("sync_in_progress");
        };
        let settings = self
            .sync
            .lock()
            .map_err(|_| IpcError::sync_unavailable())?
            .clone();
        let result = sync::run(&self.database, &settings.server_url, &settings.session);
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

    /// Fires a sync in the background when there is a server and a session.
    ///
    /// Called after each queued write and at startup. If another run
    /// is already in progress, the call does nothing.
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

    /// Operations still pending in the local outbox.
    pub fn sync_pending(&self, limit: Option<usize>) -> Result<Vec<SyncOperationInput>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        sync::pending(&database, limit.unwrap_or(learnvault_core::MAX_SYNC_BATCH))
    }

    /// Removes from the outbox the operations accepted by the server.
    pub fn sync_acknowledge(&self, op_ids: &[String]) -> Result<usize, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        sync::acknowledge(&database, op_ids)
    }

    /// Applies the operations received and stores the cursor in the same transaction.
    pub fn sync_apply(&self, operations: &[SyncOperation], cursor: i64) -> Result<usize, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        sync::apply(&database, operations, cursor)
    }

    /// Lists the local owner's notes, filtering by media or bringing all of them with relative paths.
    pub fn list_notes(&self, media_id: Option<&str>) -> Result<Vec<StudyNoteDto>, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_notes(&database, media_id)
    }

    /// Searches notes and catalog in the fixed owner's local database.
    ///
    /// Course titles come from the registry, so the index is synced on every search;
    /// the query and the limit are validated by the core.
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

    /// Creates a new study note associated with a media.
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

    /// Updates the content of an existing note.
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

    /// Removes a study note of the local user.
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

    /// Lists the local owner's memorization cards for a specific media.
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

    /// Creates a new memorization card associated with a media.
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
        let result = study::create_flashcard(&database, media_id, front, back, position_seconds)?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Updates the front and back of an existing memorization card.
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
        let result = study::update_flashcard(&database, card_id, front, back)?;
        drop(database);
        self.request_sync();
        Ok(result)
    }

    /// Removes a memorization card that belongs to the local user.
    pub fn delete_flashcard(&self, card_id: &str) -> Result<(), IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::delete_flashcard(&database, card_id)?;
        drop(database);
        self.request_sync();
        Ok(())
    }

    /// Records a review on a memorization card that belongs to the local user.
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

    /// Lists the courses and lessons favourited by the local owner.
    pub fn list_favorites(&self) -> Result<StudyFavoritesDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::list_favorites(&database)
    }

    /// Marks or unmarks a course as a favourite for the local owner.
    pub fn set_course_favorite(&self, course_id: &str, favorite: bool) -> Result<bool, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::set_course_favorite(&database, course_id, favorite)
    }

    /// Marks or unmarks a lesson (media) as a favourite for the local owner.
    pub fn set_lesson_favorite(&self, media_id: &str, favorite: bool) -> Result<bool, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::set_lesson_favorite(&database, media_id, favorite)
    }

    /// Persists a Pomodoro session of the local owner.
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

    /// Lists the local owner's persisted focus sessions, from newest to oldest.
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

    /// Reads the local owner's persisted focus summary.
    pub fn focus_summary(&self) -> Result<FocusSummaryDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::focus_summary(&database)
    }

    /// Reads the local owner's persisted rewards summary.
    pub fn reward_summary(&self) -> Result<RewardSummaryDto, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::reward_summary(&database)
    }

    /// Redeems a daily mission of the local owner.
    pub fn claim_daily_mission(&self, mission_id: &str) -> Result<bool, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::claim_daily_mission(&database, mission_id)
    }

    /// Reads the local owner's configured weekly goal.
    pub fn weekly_goal(&self) -> Result<i64, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::weekly_goal(&database)
    }

    /// Saves the local owner's configured weekly goal.
    pub fn set_weekly_goal(&self, goal: i64) -> Result<i64, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        study::set_weekly_goal(&database, goal)
    }

    /// Queries the impact of removing a media from the local catalog.
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

    /// Explicitly removes a media from the local catalog (persistent tombstone).
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

    /// Opens the specified media in the operating system's default player.
    pub fn open_in_system_player(&self, media_id: &str) -> Result<bool, IpcError> {
        let database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        let courses = self.courses();
        crate::command::open_in_system_player(&database, &courses, media_id)
    }

    /// Reads a bounded slice from an authorized local media or material file without HTTP `fetch`.
    pub fn read_media_slice(
        &self,
        media_id: &str,
        start: u64,
        length: usize,
    ) -> Result<MediaSliceDto, IpcError> {
        use std::io::{Read, Seek, SeekFrom};

        const MAX_SLICE_BYTES: usize = 512 * 1024;
        let requested = length.min(MAX_SLICE_BYTES);
        let opened = {
            let database = self
                .database
                .lock()
                .map_err(|_| IpcError::database_unavailable())?;
            let Some(library_id) = learnvault_core::find_media_library_id(&database, media_id)
                .map_err(|_| IpcError::study_unavailable())?
            else {
                return Err(IpcError::media_not_found());
            };
            let Some(course) = self.course(&library_id) else {
                return Err(IpcError::media_not_found());
            };
            let root = CourseRoot::resolve(course.root.as_path())
                .map_err(|_| IpcError::media_not_found())?;
            learnvault_core::open_media(&database, &library_id, media_id, root.as_path())
                .map_err(|_| IpcError::media_not_found())?
        };
        let total = opened.size;
        if requested == 0 || start >= total {
            return Ok(MediaSliceDto {
                bytes: Vec::new(),
                total,
            });
        }
        let to_read = (total - start).min(requested as u64) as usize;
        let mut file = opened.file;
        file.seek(SeekFrom::Start(start))
            .map_err(|_| IpcError::media_not_found())?;
        let mut buf = vec![0u8; to_read];
        let mut filled = 0;
        while filled < to_read {
            match file.read(&mut buf[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(IpcError::media_not_found()),
            }
        }
        buf.truncate(filled);
        Ok(MediaSliceDto { bytes: buf, total })
    }

    /// Reserves the rescan state, returning a guard that releases it on `Drop`.
    ///
    /// The swap is atomic: the first call wins and the concurrent ones receive
    /// [`IpcError::rescan_in_progress`], without waiting or running in parallel. While the
    /// guard exists, no other scan can start — the state is neither cancelled nor
    /// corrupted, only refused. It is the rescan's busy state, never the SQLite lock.
    pub fn begin_rescan(&self) -> Result<RescanGuard, IpcError> {
        self.rescanning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| IpcError::rescan_in_progress())?;
        Ok(RescanGuard {
            rescanning: Arc::clone(&self.rescanning),
        })
    }

    /// Reads the registered course folders and applies the scan to each local catalog.
    ///
    /// It only runs by explicit action: neither startup nor import calls it.
    /// With no registered courses it returns [`IpcError::no_courses_registered`]; a course whose
    /// folder vanished, became a link/junction or became inaccessible counts as a failure in the aggregation
    /// and does not stop the others. Each root is revalidated immediately before the scan and
    /// no configuration lock is held during the read; the database stays outside
    /// the folders and they stay read-only. Concurrent calls return
    /// [`IpcError::rescan_in_progress`] without touching the catalog.
    ///
    /// Known limitation of this slice: the SQLite database lock is held during the whole
    /// scan, so a concurrent catalog read waits for the indexing to finish.
    /// The busy state refuses a second rescan, but does not parallelize the catalog read.
    pub fn rescan(&self) -> Result<RescanSummaryDto, IpcError> {
        let _guard = self.begin_rescan()?;

        let courses = self.courses();
        if courses.is_empty() {
            return Err(IpcError::no_courses_registered());
        }

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
        let mut scanned = Vec::with_capacity(courses.len());
        for course in &courses {
            // Revalidates the root immediately before scanning outside the SQLite lock:
            // if the directory was removed or turned into a link, the scan is refused.
            let Ok(root) = CourseRoot::resolve(course.root.as_path()) else {
                summary.failed += 1;
                summary.complete = false;
                continue;
            };
            let outcome = learnvault_core::scan(root.as_path());
            if outcome.is_ok() {
                let _ = crate::covers::auto_detect_course_cover(
                    &self.covers_dir,
                    &course.id,
                    root.as_path(),
                );
            }
            scanned.push((course.id.clone(), root, outcome));
        }

        let mut database = self
            .database
            .lock()
            .map_err(|_| IpcError::database_unavailable())?;
        for (course_id, root, outcome) in scanned {
            match learnvault_core::apply_rescan_result(
                &mut database,
                &course_id,
                outcome,
                root.as_path(),
            ) {
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

/// Guard of the rescan's busy state.
///
/// It releases the state when dropped, including on error or panic. It does not expose the
/// path nor the database: it is only the lifecycle of the rescan's mutual exclusion.
pub struct RescanGuard {
    rescanning: Arc<AtomicBool>,
}

impl Drop for RescanGuard {
    fn drop(&mut self) {
        self.rescanning.store(false, Ordering::Release);
    }
}
