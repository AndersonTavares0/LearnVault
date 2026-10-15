//! Testes da resolução segura de mídia.
//!
//! O núcleo recebe `library_id` e `media_id`, valida o caminho persistido, confina o
//! arquivo sob a raiz e devolve o arquivo já aberto em modo somente leitura, com os
//! metadados lidos do próprio descritor.

use std::io::{Read, Write};
use std::path::Path;

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, MediaKind, find_media_by_path, open_media, open_media_reference,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "bib-1";

fn key(path: &str) -> String {
    // Caminhos que o domínio recusa são gravados como estão: os testes que precisam deles
    // verificam justamente que a resolução revalida o que está no banco.
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

/// Manifesto dos arquivos da árvore, usado para provar que a resolução não escreve.
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

#[test]
fn a_file_changed_only_in_mtime_is_flagged() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let db = library_db()?;
    // Tamanho correto e mtime antigo: só o mtime diverge do que a varredura registrou.
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

/// O caminho usado pelo servidor HTTP (`open_media_reference`) recusa um diretório
/// como `MediaNotAFile` nas duas plataformas. No Windows, sem esta classificação
/// pré-abertura, `File::open` de um diretório devolveria `PermissionDenied` e o erro
/// viraria `MediaUnreadable`.
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

/// Um arquivo comum com o atributo somente leitura continua legível: a correção não
/// pode confundir "sem permissão de escrita" com "inacessível".
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

    // Restaura a escrita para que a limpeza do `TempDir` não tropece no Windows,
    // onde apagar um arquivo somente leitura é recusado.
    restore_writable(&path)?;
    Ok(())
}

/// Restaura a permissão de escrita sem `set_readonly(false)`, que o clippy recusa
/// por tornar o arquivo gravável por todos no Unix.
fn restore_writable(path: &Path) -> std::io::Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o644);
    }
    #[cfg(windows)]
    {
        // No Windows, `set_readonly(false)` apenas limpa FILE_ATTRIBUTE_READONLY; a
        // preocupação do lint (tornar o arquivo gravável por todos) é específica do
        // Unix, então o silêncio é local e justificado.
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
    }
    std::fs::set_permissions(path, permissions)
}

/// Um arquivo comum que não pode ser aberto continua sendo um erro operacional
/// (`MediaUnreadable`), nunca `MediaNotAFile`: a classificação pré-abertura olha o
/// tipo do caminho, não o resultado da abertura, então não mascara permissão negada.
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

    // `root` (uid 0) ignora as permissões; sem a pré-condição real, não invente o
    // resultado: restaure o modo e saia sem afirmar um contrato que o ambiente não
    // permite observar. Runners de CI não-root exercitam a asserção abaixo.
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

/// No Windows, junctions são pontos de reanálise criáveis sem privilégio. Uma
/// junction que escapa da biblioteca é recusada, provando que a validação de
/// reparse/canonicalização não é contornada no Windows.
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
