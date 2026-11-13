//! Testes da detecção incremental: novo, alterado, movido e ausente.
//!
//! A identidade da aula é o `media_id`. Um arquivo que muda de caminho continua sendo a
//! mesma aula, com as mesmas notas, marcadores e progresso; um arquivo que desaparece é
//! marcado como ausente sem apagar histórico. A biblioteca física permanece intocada.

use std::{
    fs::{self, FileTimes},
    path::Path,
    time::{Duration, SystemTime},
};

use learnvault_core::{Database, create_media_bookmark, create_study_note, rescan, search};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "curso-1";
const OWNER: &str = "dono-1";

fn write(root: &Path, relative: &str, contents: &[u8]) -> std::io::Result<()> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn media_id(db: &Database, key: &str) -> Option<String> {
    db.connection()
        .query_row(
            "SELECT media_id FROM media WHERE library_id = ?1 AND relative_path_key = ?2",
            rusqlite::params![LIB, key],
            |row| row.get(0),
        )
        .ok()
}

fn availability(db: &Database, media_id: &str) -> Option<String> {
    db.connection()
        .query_row(
            "SELECT availability FROM media WHERE media_id = ?1",
            rusqlite::params![media_id],
            |row| row.get(0),
        )
        .ok()
}

fn count(db: &Database, table: &str) -> i64 {
    db.connection()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("a contagem deve ser consultável")
}

/// Progresso, notas e marcadores do dono, para provar que a identidade preserva histórico.
fn history(db: &Database, media_id: &str) -> (i64, usize, usize) {
    let position = db
        .connection()
        .query_row(
            "SELECT position_seconds FROM study_state WHERE owner_id = ?1 AND media_id = ?2",
            rusqlite::params![OWNER, media_id],
            |row| row.get(0),
        )
        .expect("o progresso deve existir");
    let notes = count_where(db, "study_note", media_id);
    let bookmarks = count_where(db, "media_bookmark", media_id);
    (position, notes, bookmarks)
}

fn count_where(db: &Database, table: &str, media_id: &str) -> usize {
    let count: i64 = db
        .connection()
        .query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE owner_id = ?1 AND media_id = ?2"),
            rusqlite::params![OWNER, media_id],
            |row| row.get(0),
        )
        .expect("a contagem deve ser consultável");
    usize::try_from(count).unwrap_or(0)
}

fn seed_history(db: &Database, media_id: &str) -> TestResult {
    db.connection().execute(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES (?1, ?2, 42, 0, 1760000000)",
        rusqlite::params![OWNER, media_id],
    )?;
    create_study_note(db, OWNER, media_id, 42, "preserve esta nota")?;
    create_media_bookmark(db, OWNER, media_id, 42, "marcador preservado")?;
    Ok(())
}

#[test]
fn a_moved_file_keeps_identity_notes_and_progress_even_when_mtime_changes() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let library = tempfile::tempdir()?;
    let root = library.path();

    write(root, "Curso/antigo-nome.mp4", b"conteudo exclusivo da aula")?;
    rescan(&mut db, LIB, root)?;
    let id = media_id(&db, "Curso/antigo-nome.mp4").expect("a aula deve estar catalogada");
    seed_history(&db, &id)?;

    // O arquivo muda de caminho e de mtime, mas o conteúdo é o mesmo.
    let old_path = root.join("Curso/antigo-nome.mp4");
    fs::remove_file(&old_path)?;
    write(root, "Curso/novo-nome.mp4", b"conteudo exclusivo da aula")?;
    set_mtime(
        &root.join("Curso/novo-nome.mp4"),
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
    )?;

    let summary = rescan(&mut db, LIB, root)?;

    assert_eq!(summary.moved, 1, "o hash amostral confirma o mesmo arquivo");
    assert_eq!(summary.inserted, 0);
    assert_eq!(summary.marked_missing, 0);
    assert_eq!(
        media_id(&db, "Curso/novo-nome.mp4").as_deref(),
        Some(id.as_str())
    );
    assert_eq!(media_id(&db, "Curso/antigo-nome.mp4"), None);
    assert_eq!(history(&db, &id), (42, 1, 1));
    assert_eq!(availability(&db, &id).as_deref(), Some("available"));

    // O índice FTS acompanha o novo caminho: a busca antiga não encontra aula e a nova
    // devolve a mesma identidade.
    assert!(search(&db, OWNER, false, "antigo", 10)?.lessons.is_empty());
    let found = search(&db, OWNER, false, "novo", 10)?;
    assert_eq!(found.lessons.len(), 1);
    assert_eq!(found.lessons[0].media_id, id);
    assert_eq!(found.lessons[0].relative_path, "Curso/novo-nome.mp4");
    Ok(())
}

#[test]
fn an_ambiguous_move_is_not_applied_to_any_row() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let library = tempfile::tempdir()?;
    let root = library.path();

    // Duas aulas com o mesmo conteúdo e o mesmo mtime: nenhuma pista distingue qual delas
    // foi movida.
    write(root, "Curso/a.mp4", b"conteudo repetido")?;
    write(root, "Curso/b.mp4", b"conteudo repetido")?;
    set_mtime(
        &root.join("Curso/a.mp4"),
        SystemTime::UNIX_EPOCH + Duration::from_secs(500),
    )?;
    set_mtime(
        &root.join("Curso/b.mp4"),
        SystemTime::UNIX_EPOCH + Duration::from_secs(500),
    )?;
    rescan(&mut db, LIB, root)?;
    let first = media_id(&db, "Curso/a.mp4").expect("a aula deve estar catalogada");
    let second = media_id(&db, "Curso/b.mp4").expect("a aula deve estar catalogada");

    fs::remove_file(root.join("Curso/a.mp4"))?;
    fs::remove_file(root.join("Curso/b.mp4"))?;
    write(root, "Curso/c.mp4", b"conteudo repetido")?;
    set_mtime(
        &root.join("Curso/c.mp4"),
        SystemTime::UNIX_EPOCH + Duration::from_secs(500),
    )?;

    let summary = rescan(&mut db, LIB, root)?;

    assert_eq!(
        summary.moved, 0,
        "uma escolha ambígua não pode realocar nenhuma linha"
    );
    assert_eq!(summary.inserted, 1);
    assert_eq!(summary.marked_missing, 2);
    assert_ne!(media_id(&db, "Curso/c.mp4"), Some(first.clone()));
    assert_ne!(media_id(&db, "Curso/c.mp4"), Some(second));
    Ok(())
}

#[test]
fn a_changed_file_keeps_identity_and_counts_as_changed() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let library = tempfile::tempdir()?;
    let root = library.path();

    write(root, "Curso/aula.mp4", b"primeira versao")?;
    rescan(&mut db, LIB, root)?;
    let id = media_id(&db, "Curso/aula.mp4").expect("a aula deve estar catalogada");

    write(root, "Curso/aula.mp4", b"segunda versao, maior")?;
    let summary = rescan(&mut db, LIB, root)?;

    assert_eq!(summary.changed, 1);
    assert_eq!(summary.inserted, 0);
    assert_eq!(summary.moved, 0);
    assert_eq!(
        media_id(&db, "Curso/aula.mp4").as_deref(),
        Some(id.as_str())
    );
    Ok(())
}

#[test]
fn a_metadata_only_touch_is_not_a_content_change() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let library = tempfile::tempdir()?;
    let root = library.path();

    write(root, "Curso/aula.mp4", b"conteudo estavel")?;
    rescan(&mut db, LIB, root)?;

    // Mesmo conteúdo e mesmo tamanho, mas outro mtime: é um toque, não uma alteração.
    set_mtime(
        &root.join("Curso/aula.mp4"),
        SystemTime::UNIX_EPOCH + Duration::from_secs(2_000),
    )?;
    let summary = rescan(&mut db, LIB, root)?;

    assert_eq!(summary.changed, 0);
    assert_eq!(summary.updated, 1);
    assert_eq!(summary.inserted, 0);
    Ok(())
}

#[test]
fn a_missing_file_is_marked_unavailable_without_deleting_history() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let library = tempfile::tempdir()?;
    let root = library.path();

    write(root, "Curso/aula.mp4", b"conteudo da aula")?;
    rescan(&mut db, LIB, root)?;
    let id = media_id(&db, "Curso/aula.mp4").expect("a aula deve estar catalogada");
    seed_history(&db, &id)?;

    fs::remove_file(root.join("Curso/aula.mp4"))?;
    let summary = rescan(&mut db, LIB, root)?;

    assert_eq!(summary.marked_missing, 1);
    assert_eq!(availability(&db, &id).as_deref(), Some("missing"));
    assert_eq!(history(&db, &id), (42, 1, 1), "o histórico nunca é apagado");
    assert_eq!(count(&db, "media"), 1, "a linha continua no catálogo");
    Ok(())
}

#[test]
fn rescanning_with_hashes_never_writes_to_the_library() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let library = tempfile::tempdir()?;
    let root = library.path();

    write(root, "Curso/aula.mp4", b"conteudo da aula")?;
    write(root, "Curso/apoio.pdf", b"material")?;
    let before = snapshot(root);

    rescan(&mut db, LIB, root)?;
    // Uma segunda varredura exerce o caminho de hash sobre arquivos já conhecidos.
    write(root, "Curso/outra.mp4", b"outra aula")?;
    let expected = snapshot(root);
    assert_ne!(before, expected, "o teste precisa de uma árvore diferente");
    rescan(&mut db, LIB, root)?;

    assert_eq!(
        snapshot(root),
        expected,
        "a varredura não pode alterar a biblioteca"
    );
    Ok(())
}

fn set_mtime(path: &Path, time: SystemTime) -> std::io::Result<()> {
    let file = fs::OpenOptions::new().write(true).open(path)?;
    file.set_times(FileTimes::new().set_modified(time))
}

/// Caminho, tamanho e bytes de cada arquivo da árvore.
fn snapshot(root: &Path) -> Vec<(String, u64, Vec<u8>)> {
    fn walk(base: &Path, current: &Path, entries: &mut Vec<(String, u64, Vec<u8>)>) {
        let mut children: Vec<_> = fs::read_dir(current)
            .expect("o diretório deve ser legível")
            .map(|entry| entry.expect("entrada"))
            .collect();
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let path = child.path();
            let relative = path
                .strip_prefix(base)
                .expect("o caminho pertence à raiz")
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = child.metadata().expect("metadados");
            if metadata.is_dir() {
                walk(base, &path, entries);
            } else {
                entries.push((relative, metadata.len(), fs::read(&path).expect("bytes")));
            }
        }
    }

    let mut entries = Vec::new();
    walk(root, root, &mut entries);
    entries
}
