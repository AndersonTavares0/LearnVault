use learnvault_core::{
    Database, apply_scan, find_media_by_path, open_media_reference, remove_media, scan,
};
use std::fs;

#[test]
fn media_path_lookup_returns_the_indexed_identity() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    fs::write(root.path().join("lesson.mp4"), b"0123456789")?;
    let mut db = Database::open_in_memory()?;
    apply_scan(
        &mut db,
        "library-1",
        &scan(root.path())?,
        learnvault_core::KeyMode::for_host(),
    )?;

    let reference =
        find_media_by_path(&db, "library-1", "lesson.mp4")?.expect("media should be indexed");
    assert_eq!(reference.library_id, "library-1");
    assert!(find_media_by_path(&db, "library-1", "../lesson.mp4")?.is_none());
    Ok(())
}

#[test]
fn media_path_lookup_excludes_explicitly_removed_media() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    fs::write(root.path().join("lesson.mp4"), b"0123456789")?;
    let mut db = Database::open_in_memory()?;
    apply_scan(
        &mut db,
        "library-1",
        &scan(root.path())?,
        learnvault_core::KeyMode::for_host(),
    )?;
    let media_id = find_media_by_path(&db, "library-1", "lesson.mp4")?
        .expect("media should be indexed")
        .media_id;
    remove_media(&mut db, "library-1", &media_id, false)?;
    assert!(find_media_by_path(&db, "library-1", "lesson.mp4")?.is_none());
    Ok(())
}

#[test]
fn media_path_lookup_is_scoped_to_the_requested_library() -> Result<(), Box<dyn std::error::Error>>
{
    let first_root = tempfile::tempdir()?;
    let second_root = tempfile::tempdir()?;
    fs::write(first_root.path().join("lesson.mp4"), b"first")?;
    fs::write(second_root.path().join("lesson.mp4"), b"second")?;
    let mut db = Database::open_in_memory()?;
    apply_scan(
        &mut db,
        "library-2",
        &scan(second_root.path())?,
        learnvault_core::KeyMode::for_host(),
    )?;
    apply_scan(
        &mut db,
        "library-1",
        &scan(first_root.path())?,
        learnvault_core::KeyMode::for_host(),
    )?;

    let first = find_media_by_path(&db, "library-1", "lesson.mp4")?.expect("first library media");
    let second = find_media_by_path(&db, "library-2", "lesson.mp4")?.expect("second library media");
    assert_eq!(first.library_id, "library-1");
    assert_eq!(second.library_id, "library-2");
    assert_ne!(first.media_id, second.media_id);
    Ok(())
}

#[test]
fn opening_a_stale_reference_fails_after_media_removal() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    fs::write(root.path().join("lesson.mp4"), b"0123456789")?;
    let mut db = Database::open_in_memory()?;
    apply_scan(
        &mut db,
        "library-1",
        &scan(root.path())?,
        learnvault_core::KeyMode::for_host(),
    )?;
    let reference =
        find_media_by_path(&db, "library-1", "lesson.mp4")?.expect("media should be indexed");
    remove_media(&mut db, "library-1", &reference.media_id, false)?;

    let error = open_media_reference(&db, &reference, root.path()).unwrap_err();
    assert!(matches!(error, learnvault_core::Error::MediaRemoved { .. }));
    Ok(())
}
