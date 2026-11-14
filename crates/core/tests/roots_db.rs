//! Persistence of course roots (ADR 0004): promoting a folder of a source to its own
//! library and undoing it must never lose study data, access or favorites.

use learnvault_core::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, MediaKind, RootError, ScanOutcome, ScannedFile, apply_scan,
    create_flashcard, create_media_bookmark, create_study_note, create_user, demote_root,
    list_favorites, list_flashcards, list_media_bookmarks, list_roots, list_study_notes,
    list_user_course_access, partition_scan, promote_root, record_position, set_course_favorite,
    study_progress,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SRC: &str = "src";
const OWNER: &str = "ana";
const MODE: KeyMode = KeyMode::Sensitive;

fn outcome(paths: &[&str], inaccessible: &[&str]) -> ScanOutcome {
    ScanOutcome {
        files: paths
            .iter()
            .map(|path| ScannedFile {
                relative_path: (*path).to_owned(),
                relative_path_key: relative_path_key(path, MODE).unwrap(),
                kind: MediaKind::Media,
                size: 10,
                mtime: 1,
            })
            .collect(),
        inaccessible: inaccessible.iter().map(|path| (*path).to_owned()).collect(),
    }
}

fn scanned(paths: &[&str]) -> Result<Database, Box<dyn std::error::Error>> {
    scanned_with(paths, &[])
}

fn scanned_with(
    paths: &[&str],
    inaccessible: &[&str],
) -> Result<Database, Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;
    apply_scan(&mut db, SRC, &outcome(paths, inaccessible), MODE)?;
    Ok(db)
}

fn media_id(db: &Database, path: &str) -> String {
    db.connection()
        .query_row(
            "SELECT media_id FROM media WHERE relative_path = ?1",
            [path],
            |row| row.get(0),
        )
        .unwrap()
}

fn library_of(db: &Database, path: &str) -> String {
    db.connection()
        .query_row(
            "SELECT library_id FROM media WHERE relative_path = ?1",
            [path],
            |row| row.get(0),
        )
        .unwrap()
}

fn count(db: &Database, sql: &str) -> i64 {
    db.connection()
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

const TREE: &[&str] = &[
    "Cursos/Rust/01 Intro/1.mp4",
    "Cursos/Rust/02 Types/1.mp4",
    "Cursos/Go/1.mp4",
    "Solto.mp4",
];

#[test]
fn promoting_moves_only_the_media_under_the_prefix_and_keeps_identity() -> TestResult {
    let mut db = scanned(TREE)?;
    let ids: Vec<String> = TREE.iter().map(|path| media_id(&db, path)).collect();

    let moved = promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;

    assert_eq!(moved, 2);
    assert_eq!(library_of(&db, "Cursos/Rust/01 Intro/1.mp4"), "rust");
    assert_eq!(library_of(&db, "Cursos/Rust/02 Types/1.mp4"), "rust");
    assert_eq!(library_of(&db, "Cursos/Go/1.mp4"), SRC);
    assert_eq!(library_of(&db, "Solto.mp4"), SRC);
    // media_id and the path relative to the source do not change.
    let after: Vec<String> = TREE.iter().map(|path| media_id(&db, path)).collect();
    assert_eq!(ids, after);
    assert_eq!(list_roots(&db, SRC)?.len(), 1);
    Ok(())
}

#[test]
fn study_data_follows_the_media_through_promote_and_demote() -> TestResult {
    let mut db = scanned(TREE)?;
    let lesson = media_id(&db, "Cursos/Rust/01 Intro/1.mp4");
    record_position(&db, OWNER, &lesson, 42)?;
    create_study_note(&db, OWNER, &lesson, 5, "note")?;
    create_flashcard(&db, OWNER, &lesson, "front", "back", None)?;
    create_media_bookmark(&db, OWNER, &lesson, 7, "mark")?;

    let check = |db: &Database| -> TestResult {
        assert_eq!(
            study_progress(db, OWNER, &lesson)?
                .unwrap()
                .position_seconds,
            42
        );
        assert_eq!(list_study_notes(db, OWNER, &lesson)?.len(), 1);
        assert_eq!(list_flashcards(db, OWNER, &lesson)?.len(), 1);
        assert_eq!(list_media_bookmarks(db, OWNER, &lesson)?.len(), 1);
        Ok(())
    };
    check(&db)?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    check(&db)?;
    demote_root(&mut db, "rust")?;
    check(&db)?;
    assert_eq!(library_of(&db, "Cursos/Rust/01 Intro/1.mp4"), SRC);
    Ok(())
}

#[test]
fn a_prefix_matches_whole_folders_and_treats_sql_wildcards_as_text() -> TestResult {
    let mut db = scanned(&[
        "A/x.mp4",
        "AB/y.mp4",
        "a_b/1.mp4",
        "axb/2.mp4",
        "p%/1.mp4",
        "pzz/2.mp4",
    ])?;
    promote_root(&mut db, SRC, "a", "A", MODE)?;
    promote_root(&mut db, SRC, "under", "a_b", MODE)?;
    promote_root(&mut db, SRC, "pct", "p%", MODE)?;
    assert_eq!(library_of(&db, "A/x.mp4"), "a");
    assert_eq!(library_of(&db, "AB/y.mp4"), SRC);
    assert_eq!(library_of(&db, "a_b/1.mp4"), "under");
    assert_eq!(library_of(&db, "axb/2.mp4"), SRC);
    assert_eq!(library_of(&db, "p%/1.mp4"), "pct");
    assert_eq!(library_of(&db, "pzz/2.mp4"), SRC);
    Ok(())
}

#[test]
fn an_overlapping_or_invalid_root_is_rejected_and_changes_nothing() -> TestResult {
    let mut db = scanned(TREE)?;
    promote_root(&mut db, SRC, "cursos", "Cursos", MODE)?;
    let before = count(
        &db,
        "SELECT COUNT(*) FROM media WHERE library_id = 'cursos'",
    );

    for (id, prefix) in [("inner", "Cursos/Rust"), ("same", "Cursos"), ("bad", "..")] {
        let error = promote_root(&mut db, SRC, id, prefix, MODE).unwrap_err();
        assert!(matches!(error, Error::SourceRoot(_)), "{prefix}: {error:?}");
    }
    assert!(matches!(
        promote_root(&mut db, SRC, "cursos", "Solto", MODE).unwrap_err(),
        Error::SourceRoot(RootError::LibraryExists(_))
    ));
    assert!(matches!(
        promote_root(&mut db, SRC, SRC, "Solto", MODE).unwrap_err(),
        Error::SourceRoot(RootError::LibraryExists(_))
    ));
    assert_eq!(count(&db, "SELECT COUNT(*) FROM library_root"), 1);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM media WHERE library_id = 'cursos'"
        ),
        before
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM library WHERE library_id IN ('inner','same','bad')"
        ),
        0
    );
    Ok(())
}

#[test]
fn explicitly_removed_media_moves_with_its_library_and_stays_removed() -> TestResult {
    let mut db = scanned(TREE)?;
    let id = media_id(&db, "Cursos/Rust/01 Intro/1.mp4");
    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES (?1, ?2, 1)",
        [SRC, &id],
    )?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM media_removal WHERE library_id = 'rust'"
        ),
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM media_removal WHERE library_id = 'src'"
        ),
        0
    );
    demote_root(&mut db, "rust")?;
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM media_removal WHERE library_id = 'src'"
        ),
        1
    );
    Ok(())
}

#[test]
fn a_root_inherits_the_last_scan_narrowed_to_its_prefix() -> TestResult {
    let mut db = scanned_with(TREE, &["Cursos/Rust/private", "Cursos/Go/locked"])?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    promote_root(&mut db, SRC, "go", "Cursos/Go", MODE)?;
    let areas = |library: &str| -> Vec<String> {
        let mut statement = db
            .connection()
            .prepare(
                "SELECT i.relative_path FROM scan_inaccessible i JOIN scan s ON s.scan_id = i.scan_id
                  WHERE s.library_id = ?1 ORDER BY s.scan_id DESC, i.relative_path",
            )
            .unwrap();
        statement
            .query_map([library], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(areas("rust"), ["Cursos/Rust/private"]);
    assert_eq!(areas("go"), ["Cursos/Go/locked"]);
    assert_eq!(
        count(
            &db,
            "SELECT complete FROM scan WHERE library_id = 'rust' ORDER BY scan_id DESC LIMIT 1"
        ),
        0
    );
    Ok(())
}

#[test]
fn a_clean_root_is_complete_and_a_root_below_an_unreadable_folder_is_not() -> TestResult {
    let mut db = scanned_with(TREE, &["Cursos/Go/locked"])?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    assert_eq!(
        count(&db, "SELECT complete FROM scan WHERE library_id = 'rust'"),
        1
    );
    promote_root(&mut db, SRC, "cursos-go", "Cursos/Go/locked", MODE)?;
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM scan_inaccessible i JOIN scan s ON s.scan_id = i.scan_id WHERE s.library_id = 'cursos-go'"
        ),
        1
    );

    let mut db = scanned_with(TREE, &["Cursos"])?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    let area: String = db.connection().query_row(
        "SELECT i.relative_path FROM scan_inaccessible i JOIN scan s ON s.scan_id = i.scan_id WHERE s.library_id = 'rust'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(area, ".");
    Ok(())
}

#[test]
fn a_user_restricted_to_the_source_gets_the_root_and_demote_takes_it_back() -> TestResult {
    let mut db = scanned(TREE)?;
    create_user(&mut db, "u1", "ana", "hash", 1, Some(&[SRC.to_owned()]))?;
    create_user(&mut db, "u2", "bia", "hash", 1, Some(&["other".to_owned()]))?;
    create_user(&mut db, "u3", "cia", "hash", 1, None)?;

    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    assert_eq!(
        list_user_course_access(&db, "u1")?,
        Some(vec!["rust".to_owned(), SRC.to_owned()])
    );
    assert_eq!(
        list_user_course_access(&db, "u2")?,
        Some(vec!["other".to_owned()])
    );
    assert_eq!(list_user_course_access(&db, "u3")?, None);

    demote_root(&mut db, "rust")?;
    assert_eq!(
        list_user_course_access(&db, "u1")?,
        Some(vec![SRC.to_owned()])
    );
    Ok(())
}

#[test]
fn demote_moves_course_favorites_to_the_source_and_removes_the_root() -> TestResult {
    let mut db = scanned(TREE)?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    set_course_favorite(&db, OWNER, "rust", true)?;

    demote_root(&mut db, "rust")?;

    assert_eq!(list_favorites(&db, OWNER)?.course_ids, [SRC]);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM library WHERE library_id = 'rust'"
        ),
        0
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM scan WHERE library_id = 'rust'"),
        0
    );
    assert!(list_roots(&db, SRC)?.is_empty());
    assert!(matches!(
        demote_root(&mut db, "rust").unwrap_err(),
        Error::SourceRoot(RootError::UnknownRoot(_))
    ));
    Ok(())
}

#[test]
fn a_rescan_split_by_the_roots_keeps_every_identity_and_marks_nothing_missing() -> TestResult {
    let mut db = scanned(TREE)?;
    let lesson = media_id(&db, "Cursos/Rust/01 Intro/1.mp4");
    record_position(&db, OWNER, &lesson, 99)?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;

    // The next scan of the source is split by the roots and applied library by library.
    let roots = list_roots(&db, SRC)?;
    let part = partition_scan(outcome(TREE, &[]), &roots, MODE)?;
    for (library, scan) in &part.roots {
        apply_scan(&mut db, library, scan, MODE)?;
    }
    apply_scan(&mut db, SRC, &part.remainder, MODE)?;

    assert_eq!(media_id(&db, "Cursos/Rust/01 Intro/1.mp4"), lesson);
    assert_eq!(library_of(&db, "Cursos/Rust/01 Intro/1.mp4"), "rust");
    assert_eq!(
        study_progress(&db, OWNER, &lesson)?
            .unwrap()
            .position_seconds,
        99
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM media WHERE availability <> 'available'"
        ),
        0
    );
    Ok(())
}

#[test]
fn promoting_before_the_first_scan_creates_an_empty_root_that_a_later_scan_fills() -> TestResult {
    let mut db = Database::open_in_memory()?;
    assert_eq!(promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?, 0);

    let roots = list_roots(&db, SRC)?;
    let part = partition_scan(outcome(TREE, &[]), &roots, MODE)?;
    for (library, scan) in &part.roots {
        apply_scan(&mut db, library, scan, MODE)?;
    }
    apply_scan(&mut db, SRC, &part.remainder, MODE)?;
    assert_eq!(library_of(&db, "Cursos/Rust/02 Types/1.mp4"), "rust");
    assert_eq!(library_of(&db, "Cursos/Go/1.mp4"), SRC);
    Ok(())
}

fn root_scan(db: &Database, library: &str) -> (i64, String, Vec<String>) {
    let (id, status, complete): (i64, String, i64) = db
        .connection()
        .query_row(
            "SELECT scan_id, status, complete FROM scan WHERE library_id = ?1 ORDER BY scan_id DESC LIMIT 1",
            [library],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let mut statement = db
        .connection()
        .prepare("SELECT relative_path FROM scan_inaccessible WHERE scan_id = ?1")
        .unwrap();
    let areas = statement
        .query_map([id], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    (complete, status, areas)
}

#[test]
fn an_unreadable_source_root_never_leaves_a_root_looking_verified() -> TestResult {
    let mut db = scanned_with(TREE, &["."])?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    let (complete, status, areas) = root_scan(&db, "rust");
    assert_eq!(
        (complete, status.as_str(), areas),
        (0, "completed", vec![".".to_owned()])
    );
    Ok(())
}

#[test]
fn a_failed_source_scan_gives_the_root_a_failed_scan() -> TestResult {
    let mut db = scanned(TREE)?;
    db.connection().execute(
        "INSERT INTO scan (library_id, status, completed_at, complete) VALUES (?1, 'failed', 5, 0)",
        [SRC],
    )?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    let (complete, status, _) = root_scan(&db, "rust");
    assert_eq!((complete, status.as_str()), (0, "failed"));
    Ok(())
}

#[test]
fn demoting_never_turns_a_user_restricted_to_the_root_into_an_unrestricted_one() -> TestResult {
    let mut db = scanned(TREE)?;
    create_user(&mut db, "u1", "ana", "hash", 1, Some(&[SRC.to_owned()]))?;
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    // An administrator narrows the user to the root only.
    learnvault_core::set_user_course_access(&mut db, "u1", Some(&["rust".to_owned()]))?;
    assert_eq!(
        list_user_course_access(&db, "u1")?,
        Some(vec!["rust".to_owned()])
    );

    demote_root(&mut db, "rust")?;

    // Still restricted (to nothing), not unrestricted.
    assert_eq!(list_user_course_access(&db, "u1")?, Some(vec![]));
    Ok(())
}

#[test]
fn an_id_the_access_tables_would_reject_is_refused_up_front() -> TestResult {
    let mut db = scanned(TREE)?;
    create_user(&mut db, "u1", "ana", "hash", 1, Some(&[SRC.to_owned()]))?;
    let long = "x".repeat(257);
    assert!(matches!(
        promote_root(&mut db, SRC, &long, "Cursos/Rust", MODE).unwrap_err(),
        Error::SourceRoot(RootError::LibraryIdTooLong(257))
    ));
    assert_eq!(count(&db, "SELECT COUNT(*) FROM library_root"), 0);
    // 256 is the limit and still works, and the restricted user receives it.
    let edge = "y".repeat(256);
    promote_root(&mut db, SRC, &edge, "Cursos/Rust", MODE)?;
    assert!(list_user_course_access(&db, "u1")?.unwrap().contains(&edge));
    Ok(())
}

#[test]
fn a_failure_halfway_through_demote_changes_nothing() -> TestResult {
    let mut db = scanned(TREE)?;
    let lesson = media_id(&db, "Cursos/Rust/01 Intro/1.mp4");
    promote_root(&mut db, SRC, "rust", "Cursos/Rust", MODE)?;
    // A stale removal row for the same media under the source makes the second move collide
    // after the first one (the media rows) already ran.
    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES (?1, ?2, 1)",
        [SRC, &lesson],
    )?;
    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES ('rust', ?1, 2)",
        [&lesson],
    )?;

    assert!(demote_root(&mut db, "rust").is_err());

    assert_eq!(library_of(&db, "Cursos/Rust/01 Intro/1.mp4"), "rust");
    assert_eq!(list_roots(&db, SRC)?.len(), 1);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM library WHERE library_id = 'rust'"
        ),
        1
    );
    Ok(())
}

#[test]
fn a_source_that_is_a_root_and_blank_ids_are_rejected() -> TestResult {
    let mut db = scanned(TREE)?;
    promote_root(&mut db, SRC, "cursos", "Cursos", MODE)?;
    assert!(matches!(
        promote_root(&mut db, "cursos", "nested", "Cursos/Rust", MODE).unwrap_err(),
        Error::SourceRoot(RootError::SourceIsRoot(_))
    ));
    for (source, library) in [("", "a"), ("  ", "a"), (SRC, ""), (SRC, "  ")] {
        assert!(matches!(
            promote_root(&mut db, source, library, "Solto", MODE).unwrap_err(),
            Error::SourceRoot(RootError::EmptyLibraryId)
        ));
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM library_root"), 1);
    Ok(())
}
