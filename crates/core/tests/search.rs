//! Testes do índice FTS5 e da busca global.

use learnvault_core::{
    Database, MAX_SEARCH_QUERY_CHARS, create_study_note, delete_study_note, search,
    set_user_course_access, sync_course_index, update_study_note,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "lib-1";

fn database() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    Ok(db)
}

fn insert_media(db: &Database, media_id: &str, path: &str) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, ?2, ?3, ?3, 1024, 1760000000, 'available')",
        rusqlite::params![media_id, LIB, path],
    )
}

fn insert_user(db: &Database, user_id: &str) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO user (user_id, username, password_hash, is_admin, created_at)
         VALUES (?1, ?1, 'hash', 0, 1760000000)",
        [user_id],
    )
}

#[test]
fn note_index_follows_create_update_and_delete() -> TestResult {
    let db = database()?;
    insert_media(&db, "media-1", "Course/01 - Ownership.mp4")?;

    let note = create_study_note(&db, "owner-a", "media-1", 75, "Borrow checker rules")?;
    let found = search(&db, "owner-a", false, "borrow", 20)?;
    assert_eq!(found.notes.len(), 1);
    assert_eq!(found.notes[0].note_id, note.note_id);
    assert_eq!(found.notes[0].relative_path, "Course/01 - Ownership.mp4");
    assert_eq!(found.notes[0].position_seconds, 75);
    assert!(found.notes[0].snippet.contains("<mark>"));
    assert!(found.notes[0].snippet.contains("</mark>"));

    update_study_note(&db, "owner-a", &note.note_id, "Lifetimes explained")?;
    assert!(
        search(&db, "owner-a", false, "borrow", 20)?
            .notes
            .is_empty()
    );
    assert_eq!(
        search(&db, "owner-a", false, "lifetimes", 20)?.notes.len(),
        1
    );

    delete_study_note(&db, "owner-a", &note.note_id)?;
    assert!(
        search(&db, "owner-a", false, "lifetimes", 20)?
            .notes
            .is_empty()
    );
    Ok(())
}

#[test]
fn notes_are_searchable_only_by_their_owner() -> TestResult {
    let db = database()?;
    insert_media(&db, "media-1", "Course/01 - Ownership.mp4")?;
    create_study_note(&db, "owner-a", "media-1", 10, "private borrow note")?;
    create_study_note(&db, "owner-b", "media-1", 20, "another borrow note")?;

    let a = search(&db, "owner-a", false, "borrow", 20)?;
    let b = search(&db, "owner-b", false, "borrow", 20)?;
    assert_eq!(a.notes.len(), 1);
    assert_eq!(b.notes.len(), 1);
    assert_ne!(a.notes[0].note_id, b.notes[0].note_id);
    Ok(())
}

#[test]
fn course_access_filters_notes_and_catalog_results() -> TestResult {
    let mut db = database()?;
    insert_media(&db, "media-1", "Course/01 - Ownership.mp4")?;
    insert_user(&db, "usr-a")?;
    set_user_course_access(&mut db, "usr-a", Some(&["other-course".to_owned()]))?;
    create_study_note(&db, "usr-a", "media-1", 10, "borrow checker")?;
    sync_course_index(&db, &[(LIB.to_owned(), "Rust Course".to_owned())])?;

    let denied = search(&db, "usr-a", false, "borrow", 20)?;
    assert!(denied.notes.is_empty());
    assert!(denied.lessons.is_empty());
    assert!(
        search(&db, "usr-a", false, "rust", 20)?
            .course_ids
            .is_empty()
    );

    let admin = search(&db, "usr-a", true, "borrow", 20)?;
    assert_eq!(admin.notes.len(), 1);
    assert_eq!(
        search(&db, "usr-a", true, "ownership", 20)?.lessons.len(),
        1
    );
    Ok(())
}

#[test]
fn media_index_tracks_renames_and_explicit_removal() -> TestResult {
    let db = database()?;
    insert_media(&db, "media-1", "Course/01 - Ownership.mp4")?;

    let found = search(&db, "teacher", false, "ownership", 20)?;
    assert_eq!(found.lessons.len(), 1);
    assert_eq!(found.lessons[0].media_id, "media-1");

    db.connection().execute(
        "UPDATE media SET relative_path = 'Course/03 - Renamed.mp4' WHERE media_id = 'media-1'",
        [],
    )?;
    assert!(
        search(&db, "teacher", false, "ownership", 20)?
            .lessons
            .is_empty()
    );
    assert_eq!(
        search(&db, "teacher", false, "renamed", 20)?.lessons.len(),
        1
    );

    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES (?1, 'media-1', 1)",
        [LIB],
    )?;
    assert!(
        search(&db, "teacher", false, "renamed", 20)?
            .lessons
            .is_empty()
    );
    Ok(())
}

#[test]
fn course_index_is_replaced_without_stale_rows() -> TestResult {
    let db = database()?;
    sync_course_index(&db, &[(LIB.to_owned(), "Rust Deep Dive".to_owned())])?;
    assert_eq!(
        search(&db, "teacher", false, "rust", 20)?.course_ids,
        vec![LIB.to_owned()]
    );

    sync_course_index(&db, &[(LIB.to_owned(), "Go Essentials".to_owned())])?;
    assert!(
        search(&db, "teacher", false, "rust", 20)?
            .course_ids
            .is_empty()
    );
    assert_eq!(
        search(&db, "teacher", false, "essentials", 20)?.course_ids,
        vec![LIB.to_owned()]
    );
    Ok(())
}

#[test]
fn user_query_syntax_is_searched_literally() -> TestResult {
    let db = database()?;
    insert_media(&db, "media-1", "Course/01 - Ownership.mp4")?;
    create_study_note(
        &db,
        "owner-a",
        "media-1",
        10,
        "NEAR(foo bar) 100% \"quoted\" -dash",
    )?;

    // FTS5 operators and unbalanced quotes never turn into syntax errors.
    for query in [
        "NEAR(foo",
        "\"quoted\"",
        "-dash",
        "100%",
        "\"",
        "AND OR NOT",
    ] {
        let results = search(&db, "owner-a", false, query, 20)?;
        assert!(results.notes.len() <= 1, "query {query:?}");
    }
    assert!(!search(&db, "owner-a", false, "foo", 20)?.notes.is_empty());
    Ok(())
}

#[test]
fn short_or_empty_queries_return_empty_results() -> TestResult {
    let db = database()?;
    insert_media(&db, "media-1", "Course/01 - Ownership.mp4")?;
    create_study_note(&db, "owner-a", "media-1", 10, "ownership")?;

    for query in ["", " ", "a", "  b  "] {
        assert!(search(&db, "owner-a", false, query, 20)?.notes.is_empty());
    }
    let long = "a".repeat(MAX_SEARCH_QUERY_CHARS + 50);
    assert!(search(&db, "owner-a", false, &long, 20)?.notes.is_empty());
    Ok(())
}

#[test]
fn rebuild_backfills_rows_that_predate_the_index() -> TestResult {
    let db = database()?;
    insert_media(&db, "media-1", "Course/01 - Ownership.mp4")?;
    create_study_note(&db, "owner-a", "media-1", 10, "borrow checker")?;

    // The migration backfills through the FTS5 `rebuild` command; clearing the index and
    // rebuilding proves the same path indexes existing content.
    db.connection()
        .execute_batch("DELETE FROM study_note_fts; DELETE FROM media_fts;")?;
    assert!(
        search(&db, "owner-a", false, "borrow", 20)?
            .notes
            .is_empty()
    );

    db.connection().execute_batch(
        "INSERT INTO study_note_fts (study_note_fts) VALUES ('rebuild');
         INSERT INTO media_fts (media_fts) VALUES ('rebuild');",
    )?;
    assert_eq!(search(&db, "owner-a", false, "borrow", 20)?.notes.len(), 1);
    assert_eq!(
        search(&db, "owner-a", false, "ownership", 20)?
            .lessons
            .len(),
        1
    );
    Ok(())
}
