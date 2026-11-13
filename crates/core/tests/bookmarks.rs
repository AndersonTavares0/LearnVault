//! Timestamp bookmarks: owner isolation, chronological order and input limits.

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, create_media_bookmark, delete_media_bookmark, list_media_bookmarks,
    media_bookmark_media_id,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "bib-1";
const OWNER: &str = "aluno-1";
const OTHER: &str = "aluno-2";

fn library_db() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    for media_id in ["m1", "m2"] {
        let path = format!("curso/{media_id}.mp4");
        db.connection().execute(
            "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'available')",
            rusqlite::params![
                media_id,
                LIB,
                path,
                relative_path_key(&path, KeyMode::for_host())?,
                1024,
                1_760_000_000
            ],
        )?;
    }
    Ok(db)
}

#[test]
fn bookmarks_round_trip_in_time_order_and_isolate_owners() -> TestResult {
    let db = library_db()?;

    let late = create_media_bookmark(&db, OWNER, "m1", 120, "Segundo ponto")?;
    let early = create_media_bookmark(&db, OWNER, "m1", 15, "Primeiro ponto")?;
    create_media_bookmark(&db, OTHER, "m1", 45, "Marcador do outro aluno")?;
    create_media_bookmark(&db, OWNER, "m2", 10, "Outra aula")?;

    let bookmarks = list_media_bookmarks(&db, OWNER, "m1")?;
    assert_eq!(bookmarks.len(), 2);
    assert_eq!(bookmarks[0].bookmark_id, early.bookmark_id);
    assert_eq!(bookmarks[0].label, "Primeiro ponto");
    assert_eq!(bookmarks[1].bookmark_id, late.bookmark_id);

    assert_eq!(
        media_bookmark_media_id(&db, &late.bookmark_id)?,
        Some("m1".to_owned())
    );

    // Another owner cannot delete or even see the bookmark.
    assert!(!delete_media_bookmark(&db, OTHER, &early.bookmark_id)?);
    assert_eq!(list_media_bookmarks(&db, OWNER, "m1")?.len(), 2);
    assert!(delete_media_bookmark(&db, OWNER, &early.bookmark_id)?);
    let remaining = list_media_bookmarks(&db, OWNER, "m1")?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].bookmark_id, late.bookmark_id);
    assert_eq!(list_media_bookmarks(&db, OTHER, "m1")?.len(), 1);

    Ok(())
}

#[test]
fn bookmarks_validate_owner_position_label_and_media() -> TestResult {
    let db = library_db()?;

    assert!(matches!(
        create_media_bookmark(&db, OWNER, "m1", -1, "Rótulo"),
        Err(Error::InvalidStudyPosition { .. })
    ));
    assert!(matches!(
        create_media_bookmark(&db, OWNER, "m1", 10, "   "),
        Err(Error::InvalidBookmarkLabel)
    ));
    assert!(matches!(
        create_media_bookmark(&db, OWNER, "m1", 10, &"a".repeat(201)),
        Err(Error::InvalidBookmarkLabel)
    ));
    assert!(matches!(
        create_media_bookmark(&db, "   ", "m1", 10, "Rótulo"),
        Err(Error::InvalidOwnerId)
    ));
    assert!(matches!(
        create_media_bookmark(&db, OWNER, "m-inexistente", 10, "Rótulo"),
        Err(Error::MediaNotFound { .. })
    ));

    // Exactly at the limit is accepted, and the label is trimmed.
    let saved = create_media_bookmark(&db, OWNER, "m1", 10, &format!("  {}  ", "a".repeat(200)))?;
    assert_eq!(saved.label.chars().count(), 200);
    Ok(())
}
