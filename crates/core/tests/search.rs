//! Tests of the FTS5 index and of global search.

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
    insert_media_with_size(db, media_id, path, 1024)
}

fn insert_media_with_size(
    db: &Database,
    media_id: &str,
    path: &str,
    size: i64,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, ?2, ?3, ?3, ?4, 1760000000, 'available')",
        rusqlite::params![media_id, LIB, path, size],
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
fn a_restricted_owner_keeps_matches_that_rank_below_the_limit() -> TestResult {
    let mut db = database()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('lib-2', 1)",
        [],
    )?;
    for index in 0..12 {
        let media_id = format!("hidden-{index}");
        db.connection().execute(
            "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES (?1, 'lib-2', ?2, ?2, 1024, 1760000000, 'available')",
            rusqlite::params![media_id, format!("Hidden/{index} - Borrow.mp4")],
        )?;
        create_study_note(&db, "usr-a", &media_id, 10, "borrow note")?;
    }
    insert_media(&db, "media-1", "Course/02 - Borrow.mp4")?;
    insert_user(&db, "usr-a")?;
    set_user_course_access(&mut db, "usr-a", Some(&[LIB.to_owned()]))?;
    create_study_note(&db, "usr-a", "media-1", 10, "borrow note")?;

    // Twelve denied notes share the ranking with the allowed one; a small limit must
    // still surface the note the owner can open instead of an empty group.
    let results = search(&db, "usr-a", false, "borrow", 2)?;
    assert_eq!(results.notes.len(), 1);
    assert_eq!(results.notes[0].media_id, "media-1");
    assert_eq!(results.lessons.len(), 1);
    assert_eq!(results.lessons[0].media_id, "media-1");
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
fn course_index_identical_snapshot_does_not_write_rows() -> TestResult {
    let db = database()?;
    let courses = [(LIB.to_owned(), "Rust Deep Dive".to_owned())];
    sync_course_index(&db, &courses)?;
    let writes = db.connection().total_changes();

    sync_course_index(&db, &courses)?;

    assert_eq!(db.connection().total_changes(), writes);
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

#[test]
fn lesson_search_excludes_markdown_and_keeps_video() -> TestResult {
    let db = database()?;
    insert_media(&db, "media-mp4", "Course/01 - Rust Basics.mp4")?;
    insert_media(&db, "media-md", "Course/01 - Rust Notes.md")?;

    let results = search(&db, "teacher", false, "rust", 20)?;
    assert_eq!(results.lessons.len(), 1);
    assert_eq!(results.lessons[0].media_id, "media-mp4");
    Ok(())
}

#[test]
fn lesson_search_returns_readable_text_lessons_at_their_catalog_path() -> TestResult {
    let mut db = database()?;
    insert_media(&db, "text", "Module/01 - Reading/01 - Ownership.md")?;
    insert_media(&db, "extra", "Module/01 - Reading/02 - Ownership.md")?;
    insert_media(&db, "loose", "Module/Ownership.md")?;
    insert_media(&db, "mixed", "Module/02 - Video/Ownership.md")?;
    insert_media(&db, "video", "Module/02 - Video/Lesson.mp4")?;
    insert_media(&db, "pdf-text", "Module/03 - PDF/Ownership.md")?;
    insert_media(&db, "pdf", "Module/03 - PDF/Handout.pdf")?;

    let results = search(&db, "teacher", false, "ownership", 20)?;
    assert_eq!(results.lessons.len(), 1);
    assert_eq!(results.lessons[0].media_id, "text");
    assert_eq!(
        results.lessons[0].lesson_path.as_deref(),
        Some("Module/01 - Reading")
    );

    insert_user(&db, "restricted")?;
    set_user_course_access(&mut db, "restricted", Some(&[]))?;
    assert!(
        search(&db, "restricted", false, "ownership", 20)?
            .lessons
            .is_empty()
    );
    set_user_course_access(&mut db, "restricted", Some(&[LIB.to_owned()]))?;
    assert_eq!(
        search(&db, "restricted", false, "ownership", 20)?
            .lessons
            .len(),
        1
    );
    Ok(())
}

#[test]
fn text_lesson_search_uses_the_catalog_path_below_a_remote_course_root() -> TestResult {
    let db = database()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('source', 1)",
        [],
    )?;
    db.connection().execute(
        "INSERT INTO library_root (library_id, source_id, prefix, prefix_key)
         VALUES (?1, 'source', 'Store/Course', 'Store/Course')",
        [LIB],
    )?;
    insert_media(&db, "text", "Store/Course/Module/Reading/Ownership.MD")?;
    insert_media(&db, "loose", "Store/Course/Module/Ownership.md")?;

    let result = search(&db, "teacher", false, "ownership", 20)?;
    assert_eq!(result.lessons.len(), 1);
    assert_eq!(
        result.lessons[0].lesson_path.as_deref(),
        Some("Module/Reading")
    );
    Ok(())
}

#[test]
fn lesson_search_excludes_small_ts_files() -> TestResult {
    let db = database()?;
    insert_media_with_size(&db, "ts-small", "Course/01 - Ownership.ts", 1024)?;
    insert_media_with_size(&db, "ts-large", "Course/02 - Ownership.ts", 2 * 1024 * 1024)?;

    let results = search(&db, "teacher", false, "ownership", 20)?;
    assert_eq!(results.lessons.len(), 1);
    assert_eq!(results.lessons[0].media_id, "ts-large");
    Ok(())
}

#[test]
fn lesson_search_excludes_showcase_videos() -> TestResult {
    let db = database()?;
    insert_media(&db, "showcase", "Showcase/Trailer.mp4")?;
    insert_media(&db, "lesson", "Showcase/01 - Intro.mp4")?;

    let results = search(&db, "teacher", false, "showcase", 20)?;
    assert_eq!(results.lessons.len(), 1);
    assert_eq!(results.lessons[0].media_id, "lesson");
    Ok(())
}

#[test]
fn lesson_search_respects_limit_after_filtering_non_media() -> TestResult {
    let db = database()?;
    for index in 0..6 {
        insert_media(
            &db,
            &format!("doc-{index}"),
            &format!("Course/{index:02} - Guide.md"),
        )?;
    }
    for index in 0..3 {
        insert_media(
            &db,
            &format!("vid-{index}"),
            &format!("Course/{index:02} - Guide.mp4"),
        )?;
    }

    let results = search(&db, "teacher", false, "guide", 2)?;
    assert_eq!(results.lessons.len(), 2);
    assert!(
        results
            .lessons
            .iter()
            .all(|hit| hit.relative_path.ends_with(".mp4"))
    );
    Ok(())
}

#[test]
fn lesson_search_finds_video_beyond_higher_ranked_markdown() -> TestResult {
    let db = database()?;
    insert_media(&db, "lesson", "Course/01 - Rust Basics.mp4")?;
    for index in 0..201 {
        insert_media(
            &db,
            &format!("doc-{index}"),
            &format!("Rust Rust/{index}.md"),
        )?;
    }

    let results = search(&db, "teacher", false, "rust", 1)?;
    assert_eq!(results.lessons.len(), 1);
    assert_eq!(results.lessons[0].media_id, "lesson");
    Ok(())
}

#[test]
fn removed_media_notes_are_excluded_from_search_and_global_list() -> TestResult {
    let db = database()?;
    insert_media(&db, "kept", "Course/01 - Kept.mp4")?;
    insert_media(&db, "gone", "Course/02 - Gone.mp4")?;

    create_study_note(&db, "teacher", "kept", 5, "unique token keptnote")?;
    create_study_note(&db, "teacher", "gone", 5, "unique token gonenote")?;
    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at)
         VALUES (?1, 'gone', 1760000010)",
        [LIB],
    )?;

    let found = search(&db, "teacher", false, "unique", 20)?;
    assert_eq!(found.notes.len(), 1);
    assert_eq!(found.notes[0].media_id, "kept");

    let all = learnvault_core::list_all_study_notes(&db, "teacher")?;
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].media_id, "kept");
    Ok(())
}

#[test]
fn study_notes_count_characters_not_bytes_near_limit() -> TestResult {
    let db = database()?;
    insert_media(&db, "kept", "Course/01 - Kept.mp4")?;
    let multibyte = "á".repeat(9000);
    assert!(multibyte.len() > 10_000);
    let note = create_study_note(&db, "teacher", "kept", 0, &multibyte)?;
    assert_eq!(note.content.chars().count(), 9000);
    Ok(())
}

#[test]
fn flashcards_bookmarks_and_pdf_annotations_are_indexed_with_owner_isolation() -> TestResult {
    let db = database()?;
    insert_media(&db, "vid", "Course/01 - Ownership.mp4")?;
    insert_media(&db, "doc", "Course/02 - Handbook.pdf")?;

    learnvault_core::create_flashcard(
        &db,
        "owner-a",
        "vid",
        "What is borrow splitting?",
        "Disjoint struct fields can be borrowed mutably",
        Some(42),
    )?;
    learnvault_core::create_flashcard(
        &db,
        "owner-b",
        "vid",
        "Private splitting",
        "Secret answer",
        None,
    )?;
    learnvault_core::create_media_bookmark(&db, "owner-a", "vid", 90, "Splitting borrows demo")?;
    learnvault_core::create_pdf_annotation(
        &db,
        "owner-a",
        "doc",
        learnvault_core::NewPdfAnnotation {
            page: 7,
            kind: learnvault_core::PdfAnnotationKind::Note,
            color: learnvault_core::PdfAnnotationColor::Yellow,
            rects: vec![learnvault_core::PdfRect {
                x: 0.1,
                y: 0.2,
                width: 0.3,
                height: 0.1,
            }],
            quote: "Splitting borrows across fields".to_owned(),
            content: "Key rule on page 7".to_owned(),
        },
    )?;

    let found = search(&db, "owner-a", false, "splitting", 20)?;
    assert_eq!(found.flashcards.len(), 1);
    assert_eq!(found.flashcards[0].position_seconds, Some(42));
    assert_eq!(found.bookmarks.len(), 2);
    assert!(
        found
            .bookmarks
            .iter()
            .any(|b| b.kind == "bookmark" && b.position_seconds == 90)
    );
    assert!(
        found
            .bookmarks
            .iter()
            .any(|b| b.kind == "pdf" && b.position_seconds == 7)
    );
    Ok(())
}
