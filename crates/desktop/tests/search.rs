//! Tests of the desktop's global search at the IPC boundary.
//!
//! They do not bring up the webview: they exercise `DesktopState::search`, which the
//! `#[tauri::command]` command only wraps. The target is the contract observable by the frontend —
//! snake_case shape identical to the server's, fixed local owner, another owner's notes left out of the
//! result and no influence on the physical library.

use std::fs;
use std::path::Path;

use learnvault_desktop::{DesktopState, SearchResultsDto};

/// Desktop state with an external course imported and scanned.
fn configured(root: &Path) -> DesktopState {
    let state = DesktopState::open(&root.join("data"), &root.join("config")).unwrap();
    let course = root.join("Acervo privado");
    fs::create_dir_all(&course).unwrap();
    fs::write(course.join("01 - Ownership.mp4"), b"video").unwrap();
    state.import_course(|| Some(course)).unwrap();
    state.rescan().unwrap();
    state
}

/// Identity of the only media scanned, read straight from the local database.
fn media_id(root: &Path) -> String {
    let database =
        learnvault_core::Database::open(root.join("data").join("learnvault.sqlite")).unwrap();
    database
        .connection()
        .query_row("SELECT media_id FROM media", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn search_finds_the_local_owner_notes_and_catalog_titles() {
    let root = tempfile::tempdir().unwrap();
    let state = configured(root.path());
    let media = media_id(root.path());
    state
        .create_note(&media, 42, "Ownership borrow checker notes")
        .unwrap();

    let found: SearchResultsDto = state.search("borrow", None).unwrap();
    assert_eq!(found.notes.len(), 1);
    assert_eq!(found.notes[0].media_id, media);
    assert_eq!(found.notes[0].lesson_title, "Ownership");
    assert_eq!(found.notes[0].position_seconds, 42);
    assert!(found.notes[0].snippet.contains("<mark>"));
    assert!(found.notes[0].updated_at > 0);

    let lessons = state.search("ownership", None).unwrap();
    assert_eq!(lessons.lessons.len(), 1);
    assert_eq!(lessons.lessons[0].title, "Ownership");
    assert_eq!(lessons.lessons[0].media_id, media);
}

#[test]
fn search_never_returns_notes_from_another_owner() {
    let root = tempfile::tempdir().unwrap();
    let state = configured(root.path());
    let media = media_id(root.path());
    let database =
        learnvault_core::Database::open(root.path().join("data").join("learnvault.sqlite"))
            .unwrap();
    learnvault_core::create_study_note(&database, "another-owner", &media, 0, "borrow secret")
        .unwrap();

    let found = state.search("borrow", None).unwrap();
    assert!(
        found.notes.is_empty(),
        "notas de outro dono não podem aparecer"
    );
}

#[test]
fn short_queries_and_missing_terms_return_empty_groups() {
    let root = tempfile::tempdir().unwrap();
    let state = configured(root.path());
    let media = media_id(root.path());
    state.create_note(&media, 0, "ownership").unwrap();

    let short: SearchResultsDto = state.search("o", None).unwrap();
    assert_eq!(short, SearchResultsDto::default());
    let absent = state.search("nothing-matches-this", None).unwrap();
    assert!(absent.notes.is_empty() && absent.courses.is_empty() && absent.lessons.is_empty());
}

#[test]
fn a_limit_above_the_ceiling_is_capped_by_the_core() {
    let root = tempfile::tempdir().unwrap();
    let state = configured(root.path());
    let found = state.search("ownership", Some(usize::MAX)).unwrap();
    assert_eq!(found.lessons.len(), 1);
}
