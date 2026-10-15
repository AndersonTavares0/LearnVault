use learnvault_core::{Database, apply_scan, find_media_by_path, remove_media, scan};
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

    let reference = find_media_by_path(&db, "lesson.mp4")?.expect("media should be indexed");
    assert_eq!(reference.library_id, "library-1");
    assert!(find_media_by_path(&db, "../lesson.mp4")?.is_none());
    Ok(())
}

#[test]
fn media_path_lookup_excludes_explicitly_removed_media() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    fs::write(root.path().join("lesson.mp4"), b"0123456789")?;
    let mut db = Database::open_in_memory()?;
    apply_scan(&mut db, "library-1", &scan(root.path())?, learnvault_core::KeyMode::for_host())?;
    let media_id = find_media_by_path(&db, "lesson.mp4")?.expect("media should be indexed").media_id;
    remove_media(&mut db, "library-1", &media_id, false)?;
    assert!(find_media_by_path(&db, "lesson.mp4")?.is_none());
    Ok(())
}
