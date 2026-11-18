//! Tests of safe media resolution.
//!
//! The core receives `library_id` and `media_id`, validates the persisted path, confines the
//! file under the root and returns the file already open read-only, with the
//! metadata read from the descriptor itself.

use std::io::{Read, Write};
use std::path::Path;

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, MediaKind, find_media_by_path, open_media, open_media_reference,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "bib-1";

fn key(path: &str) -> String {
    // Paths the domain refuses are stored as they are: the tests that need them
    // check precisely that the resolution revalidates what is in the database.
    relative_path_key(path, KeyMode::for_host()).unwrap_or_else(|_| path.to_owned())
}

fn library_db() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    Ok(db)
}

fn insert_media(
    db: &Database,
    library_id: &str,
    media_id: &str,
    path: &str,
    size: i64,
    mtime: i64,
    availability: &str,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![media_id, library_id, path, key(path), size, mtime, availability],
    )
}

fn write_file(root: &Path, relative: &str, bytes: &[u8]) -> std::io::Result<()> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

fn read_all(mut file: std::fs::File) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn mtime_seconds(metadata: &std::fs::Metadata) -> i64 {
    use std::time::UNIX_EPOCH;

    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Manifest of the files in the tree, used to prove that the resolution does not write.
fn manifest(root: &Path) -> Vec<(String, u64)> {
    let mut entries = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).expect("leitura da fixture") {
            let entry = entry.expect("entrada da fixture");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let size = entry.metadata().expect("metadados da fixture").len();
                let relative = path
                    .strip_prefix(root)
                    .expect("caminho relativo")
                    .to_string_lossy()
                    .replace('\\', "/");
                entries.push((relative, size));
            }
        }
    }
    entries.sort();
    entries
}

#[test]
fn an_existing_file_is_opened_with_metadata_from_the_handle() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let bytes = b"conteudo da aula";
    write_file(root, "Curso/Aula/aula.mp4", bytes)?;

    let db = library_db()?;
    let stored = std::fs::metadata(root.join("Curso/Aula/aula.mp4"))?;
    let mtime = mtime_seconds(&stored);
    insert_media(
        &db,
        LIB,
        "m1",
        "Curso/Aula/aula.mp4",
        bytes.len() as i64,
        mtime,
        "available",
    )?;

    let opened = open_media(&db, LIB, "m1", root)?;

    assert_eq!(opened.media_id, "m1");
    assert_eq!(opened.size, bytes.len() as u64);
    assert_eq!(opened.mtime, mtime);
    assert_eq!(opened.kind, MediaKind::Media);
    assert!(
        !opened.changed,
        "os metadados do arquivo conferem com a última varredura"
    );
    assert_eq!(
        read_all(opened.file)?,
        bytes,
        "deve ser possível ler o arquivo aberto"
    );
    Ok(())
}

#[test]
fn a_media_id_that_does_not_exist_is_reported() -> TestResult {
    let directory = tempfile::tempdir()?;
    let db = library_db()?;

    match open_media(&db, LIB, "m-inexistente", directory.path()) {
        Err(Error::MediaNotFound { media_id }) => assert_eq!(media_id, "m-inexistente"),
        other => panic!("esperava Error::MediaNotFound, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_row_marked_missing_is_reported() -> TestResult {
    let directory = tempfile::tempdir()?;
    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 10, 10, "missing")?;

    match open_media(&db, LIB, "m1", directory.path()) {
        Err(Error::MediaUnavailable { media_id }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaUnavailable, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_file_removed_after_the_scan_is_reported() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 10, 10, "available")?;

    match open_media(&db, LIB, "m1", root) {
        Err(Error::MediaMissingOnDisk { media_id }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaMissingOnDisk, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_modified_file_is_opened_and_flagged_as_changed() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let bytes = b"conteudo novo, maior que o registrado";
    write_file(root, "Curso/Aula/aula.mp4", bytes)?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 1, 1, "available")?;

    let opened = open_media(&db, LIB, "m1", root)?;

    assert!(
        opened.changed,
        "o arquivo mudou desde a última varredura e isso precisa ser visível"
    );
    assert_eq!(
        opened.size,
        bytes.len() as u64,
        "o tamanho vem do arquivo aberto, não da linha persistida"
    );
    assert_eq!(read_all(opened.file)?, bytes);
    Ok(())
}

#[test]
fn a_stored_path_that_escapes_the_library_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let db = library_db()?;
    insert_media(&db, LIB, "m1", "../fora.mp4", 10, 10, "available")?;

    match open_media(&db, LIB, "m1", directory.path()) {
        Err(Error::InvalidRelativePath { .. }) => {}
        other => panic!("esperava Error::InvalidRelativePath, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn an_absolute_stored_path_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let db = library_db()?;
    insert_media(&db, LIB, "m1", "/etc/passwd", 10, 10, "available")?;

    match open_media(&db, LIB, "m1", directory.path()) {
        Err(Error::InvalidRelativePath { .. }) => {}
        other => panic!("esperava Error::InvalidRelativePath, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_directory_in_place_of_a_file_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    std::fs::create_dir_all(root.join("Curso/Aula"))?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula", 10, 10, "available")?;

    match open_media(&db, LIB, "m1", root) {
        Err(Error::MediaNotAFile { media_id }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaNotAFile, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn another_library_does_not_see_the_media() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let db = library_db()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        ["bib-2"],
    )?;
    insert_media(
        &db,
        "bib-2",
        "m1",
        "Curso/Aula/aula.mp4",
        8,
        10,
        "available",
    )?;

    match open_media(&db, LIB, "m1", root) {
        Err(Error::MediaNotFound { .. }) => {}
        other => panic!("esperava Error::MediaNotFound, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_missing_root_is_reported() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("ausente");
    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 10, 10, "available")?;

    match open_media(&db, LIB, "m1", &root) {
        Err(Error::LibraryRootMissing { .. }) => {}
        other => panic!("esperava Error::LibraryRootMissing, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_root_that_is_a_file_is_reported() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("arquivo");
    std::fs::write(&root, b"nao e um diretorio")?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 10, 10, "available")?;

    match open_media(&db, LIB, "m1", &root) {
        Err(Error::LibraryRootNotDirectory { .. }) => {}
        other => panic!("esperava Error::LibraryRootNotDirectory, obtive {other:?}"),
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_symlink_in_place_of_the_file_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let outside = directory.path().join("fora.mp4");
    std::fs::write(&outside, b"fora da biblioteca")?;
    std::fs::create_dir_all(root.join("Curso/Aula"))?;
    std::os::unix::fs::symlink(&outside, root.join("Curso/Aula/aula.mp4"))?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 18, 10, "available")?;

    match open_media(&db, LIB, "m1", root) {
        Err(Error::MediaIsSymlink { media_id }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaIsSymlink, obtive {other:?}"),
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_symlink_inside_the_library_is_refused_as_well() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Curso/Real/aula.mp4", b"conteudo real")?;
    std::os::unix::fs::symlink(
        root.join("Curso/Real/aula.mp4"),
        root.join("Curso/Atalho.mp4"),
    )?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Atalho.mp4", 13, 10, "available")?;

    match open_media(&db, LIB, "m1", root) {
        Err(Error::MediaIsSymlink { .. }) => {}
        other => panic!("esperava Error::MediaIsSymlink, obtive {other:?}"),
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_directory_symlink_that_escapes_the_library_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("biblioteca");
    let outside = directory.path().join("fora");
    std::fs::create_dir_all(&outside)?;
    std::fs::write(outside.join("aula.mp4"), b"fora da biblioteca")?;
    std::fs::create_dir_all(&root)?;
    std::os::unix::fs::symlink(&outside, root.join("Curso"))?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/aula.mp4", 18, 10, "available")?;

    match open_media(&db, LIB, "m1", &root) {
        Err(Error::MediaOutsideLibrary { .. }) => {}
        other => panic!("esperava Error::MediaOutsideLibrary, obtive {other:?}"),
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_symlinked_intermediate_directory_is_not_followed() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Course/Real/Lesson/video.mp4", b"synthetic video")?;
    std::os::unix::fs::symlink(root.join("Course/Real"), root.join("Course/Alias"))?;

    let db = library_db()?;
    insert_media(
        &db,
        LIB,
        "m1",
        "Course/Alias/Lesson/video.mp4",
        15,
        10,
        "available",
    )?;

    assert!(open_media(&db, LIB, "m1", root).is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_root_replaced_by_a_symlink_does_not_authorize_outside_files() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("biblioteca");
    let outside = directory.path().join("fora");
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(&outside)?;
    std::fs::write(outside.join("segredo.png"), b"fora da biblioteca")?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "segredo.png", 18, 10, "available")?;

    // The root is replaced by a link to the external folder after it was resolved.
    std::fs::remove_dir(&root)?;
    std::os::unix::fs::symlink(&outside, &root)?;

    match open_media(&db, LIB, "m1", &root) {
        Err(Error::LibraryRootIsLink { .. }) => {}
        other => panic!("a raiz substituída por link precisa ser recusada, obtive {other:?}"),
    }
    Ok(())
}

#[cfg(windows)]
#[test]
fn a_root_replaced_by_a_directory_link_is_refused_on_windows() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("biblioteca");
    let outside = directory.path().join("fora");
    std::fs::create_dir_all(&root)?;
    std::fs::create_dir_all(&outside)?;
    std::fs::write(outside.join("segredo.png"), b"fora da biblioteca")?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "segredo.png", 18, 10, "available")?;

    std::fs::remove_dir(&root)?;
    if std::os::windows::fs::symlink_dir(&outside, &root).is_err() {
        eprintln!("pulado: criar link de diretório exige privilégio no Windows");
        return Ok(());
    }

    match open_media(&db, LIB, "m1", &root) {
        Err(Error::LibraryRootIsLink { .. }) => {}
        other => panic!("a raiz substituída por link precisa ser recusada, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_file_changed_only_in_mtime_is_flagged() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let db = library_db()?;
    // Correct size and old mtime: only the mtime diverges from what the scan recorded.
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 8, 1, "available")?;

    let media = open_media(&db, LIB, "m1", root)?;

    assert!(media.changed, "o mtime divergente deve ser sinalizado");
    assert_eq!(media.size, 8, "o tamanho vem do arquivo aberto");
    Ok(())
}

#[test]
fn a_negative_stored_size_is_rejected_as_corrupt() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", -5, 10, "available")?;

    assert!(
        matches!(
            open_media(&db, LIB, "m1", root),
            Err(Error::MediaCorruptSize { .. })
        ),
        "um tamanho negativo na linha indica corrupção, não um arquivo vazio"
    );
    Ok(())
}

#[test]
fn the_returned_descriptor_is_read_only() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 8, 10, "available")?;

    let mut media = open_media(&db, LIB, "m1", root)?;

    assert!(
        media.file.write_all(b"x").is_err(),
        "o arquivo é aberto somente para leitura"
    );
    Ok(())
}

#[test]
fn an_opened_file_exposes_only_its_name_for_content_type_decisions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Curso/Aula/aula.png", b"conteudo")?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.png", 8, 10, "available")?;

    let opened = open_media(&db, LIB, "m1", root)?;

    assert_eq!(
        opened.name, "aula.png",
        "o adaptador precisa do nome do arquivo para decidir o tipo de conteúdo"
    );
    assert!(
        !opened.name.contains('/') && !opened.name.contains('\\'),
        "o nome exposto é o do arquivo, nunca um caminho: {}",
        opened.name
    );
    Ok(())
}

#[test]
fn opening_does_not_modify_the_library() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo da aula")?;
    let before = manifest(root);

    let db = library_db()?;
    let stored = std::fs::metadata(root.join("Curso/Aula/aula.mp4"))?;
    insert_media(
        &db,
        LIB,
        "m1",
        "Curso/Aula/aula.mp4",
        stored.len() as i64,
        mtime_seconds(&stored),
        "available",
    )?;

    let opened = open_media(&db, LIB, "m1", root)?;
    drop(opened);

    assert_eq!(
        before,
        manifest(root),
        "a resolução não altera a biblioteca"
    );
    Ok(())
}

/// The path used by the HTTP server (`open_media_reference`) refuses a directory
/// as `MediaNotAFile` on both platforms. On Windows, without this pre-open
/// classification, `File::open` of a directory would return `PermissionDenied` and the error
/// would become `MediaUnreadable`.
#[test]
fn a_directory_reference_in_place_of_a_file_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    std::fs::create_dir_all(root.join("Curso/Aula"))?;

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula", 10, 10, "available")?;

    let reference = find_media_by_path(&db, LIB, "Curso/Aula")?
        .expect("a linha indexada deve produzir uma referência");

    match open_media_reference(&db, &reference, root) {
        Err(Error::MediaNotAFile { media_id }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaNotAFile, obtive {other:?}"),
    }
    Ok(())
}

/// A regular file with the read-only attribute is still readable: the fix must
/// not confuse "no write permission" with "inaccessible".
#[test]
fn a_read_only_regular_file_is_still_opened() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let bytes = b"material somente leitura";
    write_file(root, "Curso/Aula/apoio.pdf", bytes)?;
    let path = root.join("Curso/Aula/apoio.pdf");

    let mut permissions = std::fs::metadata(&path)?.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions)?;

    let db = library_db()?;
    insert_media(
        &db,
        LIB,
        "m1",
        "Curso/Aula/apoio.pdf",
        bytes.len() as i64,
        10,
        "available",
    )?;

    let opened = open_media(&db, LIB, "m1", root)?;
    assert_eq!(opened.size, bytes.len() as u64);
    assert_eq!(read_all(opened.file)?, bytes);

    // Restores write access so that cleaning up the `TempDir` does not trip on Windows,
    // where deleting a read-only file is refused.
    restore_writable(&path)?;
    Ok(())
}

/// Restores write permission without `set_readonly(false)`, which clippy refuses
/// because it makes the file writable by everyone on Unix.
fn restore_writable(path: &Path) -> std::io::Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o644);
    }
    #[cfg(windows)]
    {
        // On Windows, `set_readonly(false)` only clears FILE_ATTRIBUTE_READONLY; the
        // lint's concern (making the file writable by everyone) is Unix-specific,
        // so the silencing is local and justified.
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
    }
    std::fs::set_permissions(path, permissions)
}

/// A regular file that cannot be opened is still an operational error
/// (`MediaUnreadable`), never `MediaNotAFile`: the pre-open classification looks at the
/// path's type, not at the result of the open, so it does not mask a permission denial.
#[cfg(unix)]
#[test]
fn an_unreadable_regular_file_is_reported_as_unreadable() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let bytes = b"conteudo protegido";
    write_file(root, "Curso/Aula/aula.mp4", bytes)?;
    let path = root.join("Curso/Aula/aula.mp4");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))?;

    // `root` (uid 0) ignores permissions; without the real precondition, do not invent the
    // result: restore the mode and leave without asserting a contract the environment does not
    // let us observe. Non-root CI runners exercise the assertion below.
    if std::fs::File::open(&path).is_ok() {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        return Ok(());
    }

    let db = library_db()?;
    insert_media(
        &db,
        LIB,
        "m1",
        "Curso/Aula/aula.mp4",
        bytes.len() as i64,
        10,
        "available",
    )?;

    let result = open_media(&db, LIB, "m1", root);

    let mut permissions = std::fs::metadata(&path)?.permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(&path, permissions)?;

    match result {
        Err(Error::MediaUnreadable { media_id, .. }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaUnreadable, obtive {other:?}"),
    }
    Ok(())
}

/// On Windows, junctions are reparse points that can be created without privilege. A
/// junction that escapes the library is refused, proving that the reparse/
/// canonicalization validation is not bypassed on Windows.
#[cfg(windows)]
#[test]
fn a_junction_that_escapes_the_library_is_refused() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("biblioteca");
    let outside = directory.path().join("fora");
    std::fs::create_dir_all(&outside)?;
    std::fs::write(outside.join("aula.mp4"), b"fora da biblioteca")?;
    std::fs::create_dir_all(&root)?;

    let link = root.join("Curso");
    let output = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&link)
        .arg(&outside)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "não foi possível criar a junction: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/aula.mp4", 18, 10, "available")?;

    match open_media(&db, LIB, "m1", &root) {
        Err(Error::MediaOutsideLibrary { .. }) => {}
        other => panic!("esperava Error::MediaOutsideLibrary, obtive {other:?}"),
    }
    Ok(())
}
