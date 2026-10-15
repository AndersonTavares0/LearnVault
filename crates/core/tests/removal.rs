//! Testes da remoção explícita de mídia.
//!
//! A remoção é uma ação deliberada do usuário sobre um par `library_id` + `media_id`. Ela
//! registra um fato separado da existência do registro: nada é apagado, o progresso de
//! estudo é preservado e nenhum arquivo da biblioteca é tocado. A remoção persiste entre
//! varreduras e não é desfeita por reencontrar o arquivo no disco.
//!
//! Os casos que envolvem a biblioteca física usam diretórios temporários.

use std::path::Path;

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Availability, Catalog, CatalogFile, Database, Error, KeyMode, MediaRemovalImpact,
    MediaRemovalOutcome, build_catalog, media_removal_impact, open_media, recent_study,
    record_position, remove_media, rescan, set_completed, study_progress,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "bib-1";
const OTHER_LIB: &str = "bib-2";
const OWNER: &str = "aluno-1";
const OTHER_OWNER: &str = "aluno-2";

/// Instante conhecido gravado direto no banco, para provar preservação sem depender de espera.
const SEEDED_AT: i64 = 1_000;

fn key(path: &str) -> String {
    relative_path_key(path, KeyMode::for_host()).expect("caminho de teste válido")
}

fn library_db() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    insert_library(&db, LIB)?;
    Ok(db)
}

fn insert_library(db: &Database, library_id: &str) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [library_id],
    )
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

/// Insere uma mídia disponível da biblioteca padrão.
fn media(db: &Database, media_id: &str, path: &str) -> rusqlite::Result<usize> {
    insert_media(db, LIB, media_id, path, 1024, 1_760_000_000, "available")
}

/// Grava um estado de estudo com instante controlado, sem passar pela API.
fn seed_progress(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    completed: bool,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            owner_id,
            media_id,
            position_seconds,
            i64::from(completed),
            SEEDED_AT
        ],
    )
}

/// Lê `(position_seconds, completed, updated_at)` do banco.
fn progress_row(db: &Database, owner_id: &str, media_id: &str) -> Option<(i64, i64, i64)> {
    db.connection()
        .query_row(
            "SELECT position_seconds, completed, updated_at FROM study_state
             WHERE owner_id = ?1 AND media_id = ?2",
            rusqlite::params![owner_id, media_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()
}

fn removal_count(db: &Database, library_id: &str) -> i64 {
    db.connection()
        .query_row(
            "SELECT COUNT(*) FROM media_removal WHERE library_id = ?1",
            [library_id],
            |row| row.get(0),
        )
        .expect("media_removal deve ser consultável")
}

fn media_count(db: &Database, library_id: &str) -> i64 {
    db.connection()
        .query_row(
            "SELECT COUNT(*) FROM media WHERE library_id = ?1",
            [library_id],
            |row| row.get(0),
        )
        .expect("media deve ser consultável")
}

fn availability_of(db: &Database, media_id: &str) -> String {
    db.connection()
        .query_row(
            "SELECT availability FROM media WHERE media_id = ?1",
            [media_id],
            |row| row.get(0),
        )
        .expect("media deve ser consultável")
}

fn only_media_id(db: &Database, library_id: &str) -> String {
    db.connection()
        .query_row(
            "SELECT media_id FROM media WHERE library_id = ?1 ORDER BY media_id LIMIT 1",
            [library_id],
            |row| row.get(0),
        )
        .expect("media deve ser consultável")
}

/// Coleta todos os arquivos do catálogo, em qualquer nível.
fn all_files(catalog: &Catalog) -> Vec<&CatalogFile> {
    fn collect<'a>(nodes: &'a [learnvault_core::CatalogNode], out: &mut Vec<&'a CatalogFile>) {
        for node in nodes {
            out.extend(node.files.iter());
            out.extend(node.loose_files.iter());
            collect(&node.children, out);
        }
    }

    let mut files: Vec<&CatalogFile> = catalog.loose_files.iter().collect();
    collect(&catalog.categories, &mut files);
    files
}

fn write_file(root: &Path, relative: &str, bytes: &[u8]) -> std::io::Result<()> {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

/// Manifesto recursivo da árvore: caminho relativo e tamanho.
fn manifest(root: &Path) -> Vec<(String, u64)> {
    fn walk(base: &Path, current: &Path, out: &mut Vec<(String, u64)>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                walk(base, &path, out)?;
            } else {
                let relative = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, entry.metadata()?.len()));
            }
        }
        Ok(())
    }

    let mut entries = Vec::new();
    walk(root, root, &mut entries).expect("a árvore de teste deve ser legível");
    entries.sort();
    entries
}

#[test]
fn impact_of_a_media_without_progress_reports_no_owners() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;

    let impact: MediaRemovalImpact = media_removal_impact(&db, LIB, "m1")?;

    assert_eq!(impact.media_id, "m1");
    assert_eq!(impact.relative_path, "Curso/Aula/aula.mp4");
    assert_eq!(impact.availability, Availability::Available);
    assert_eq!(impact.progress_owners, 0);
    assert!(!impact.already_removed, "a mídia ainda não foi removida");
    Ok(())
}

#[test]
fn impact_reports_every_owner_with_progress() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 10, false)?;
    seed_progress(&db, OTHER_OWNER, "m1", 20, true)?;

    let impact = media_removal_impact(&db, LIB, "m1")?;

    assert_eq!(impact.progress_owners, 2, "cada dono conta uma vez");
    Ok(())
}

#[test]
fn impact_of_an_unknown_media_is_reported() -> TestResult {
    let db = library_db()?;

    match media_removal_impact(&db, LIB, "m-inexistente") {
        Err(Error::MediaNotFound { media_id }) => assert_eq!(media_id, "m-inexistente"),
        other => panic!("esperava Error::MediaNotFound, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn removing_a_media_without_progress_does_not_require_confirmation() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;

    let outcome: MediaRemovalOutcome = remove_media(&mut db, LIB, "m1", false)?;

    assert!(outcome.removed_now);
    assert_eq!(outcome.preserved_owners, 0);
    assert_eq!(removal_count(&db, LIB), 1);
    Ok(())
}

#[test]
fn removing_a_media_with_progress_requires_confirmation() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 42, true)?;

    match remove_media(&mut db, LIB, "m1", false) {
        Err(Error::MediaRemovalNeedsConfirmation {
            media_id,
            progress_owners,
        }) => {
            assert_eq!(media_id, "m1");
            assert_eq!(progress_owners, 1);
        }
        other => panic!("esperava Error::MediaRemovalNeedsConfirmation, obtive {other:?}"),
    }

    assert_eq!(
        removal_count(&db, LIB),
        0,
        "a recusa não pode gravar a remoção"
    );
    assert_eq!(progress_row(&db, OWNER, "m1"), Some((42, 1, SEEDED_AT)));
    Ok(())
}

#[test]
fn confirming_the_removal_preserves_the_progress() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 42, true)?;

    let outcome = remove_media(&mut db, LIB, "m1", true)?;

    assert!(outcome.removed_now);
    assert_eq!(outcome.preserved_owners, 1);
    assert_eq!(
        progress_row(&db, OWNER, "m1"),
        Some((42, 1, SEEDED_AT)),
        "o progresso não pode ser apagado nem alterado"
    );
    Ok(())
}

#[test]
fn removing_the_same_media_again_is_idempotent() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;

    let first = remove_media(&mut db, LIB, "m1", false)?;
    let again = remove_media(&mut db, LIB, "m1", false)?;

    assert!(first.removed_now);
    assert!(!again.removed_now, "a segunda remoção não altera nada");
    assert_eq!(removal_count(&db, LIB), 1);
    Ok(())
}

#[test]
fn removal_is_isolated_between_libraries() -> TestResult {
    let mut db = library_db()?;
    insert_library(&db, OTHER_LIB)?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    insert_media(
        &db,
        OTHER_LIB,
        "m2",
        "Curso/Aula/aula.mp4",
        1024,
        1_760_000_000,
        "available",
    )?;

    remove_media(&mut db, LIB, "m1", false)?;

    assert_eq!(removal_count(&db, OTHER_LIB), 0, "outra biblioteca intacta");
    assert_eq!(media_count(&db, OTHER_LIB), 1);

    // A mídia da outra biblioteca continua visível no catálogo dela.
    let other = build_catalog(&db, OTHER_LIB)?;
    assert!(all_files(&other).iter().any(|file| file.media_id == "m2"));

    // E a mídia removida não é alcançável a partir da outra biblioteca.
    match remove_media(&mut db, OTHER_LIB, "m1", false) {
        Err(Error::MediaNotFound { media_id }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaNotFound, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_removed_media_disappears_from_the_catalog() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    media(&db, "m2", "Curso/Aula/outra.mp4")?;

    remove_media(&mut db, LIB, "m1", false)?;

    let catalog = build_catalog(&db, LIB)?;
    let files = all_files(&catalog);

    assert!(
        !files.iter().any(|file| file.media_id == "m1"),
        "a mídia removida não pode aparecer no catálogo"
    );
    assert!(
        files.iter().any(|file| file.media_id == "m2"),
        "as demais mídias continuam no catálogo"
    );
    assert_eq!(media_count(&db, LIB), 2, "o registro da mídia permanece");
    Ok(())
}

#[test]
fn a_removed_media_is_refused_by_open_media() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let mut db = library_db()?;
    insert_media(&db, LIB, "m1", "Curso/Aula/aula.mp4", 8, 10, "available")?;
    remove_media(&mut db, LIB, "m1", false)?;

    match open_media(&db, LIB, "m1", root) {
        Err(Error::MediaRemoved { media_id }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaRemoved, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn recent_study_hides_removed_media_without_erasing_progress() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    media(&db, "m2", "Curso/Aula/outra.mp4")?;
    seed_progress(&db, OWNER, "m1", 10, false)?;
    seed_progress(&db, OWNER, "m2", 20, false)?;

    // A mídia tem progresso, então a remoção exige confirmação: é justamente esse progresso
    // que precisa sobreviver à remoção.
    remove_media(&mut db, LIB, "m1", true)?;

    let recent = recent_study(&db, OWNER, 10)?;
    assert_eq!(recent.len(), 1, "só a mídia não removida aparece");
    assert_eq!(recent[0].media_id, "m2");
    assert_eq!(
        progress_row(&db, OWNER, "m1"),
        Some((10, 0, SEEDED_AT)),
        "o progresso da mídia removida continua gravado"
    );
    Ok(())
}

#[test]
fn a_rescan_with_the_file_on_disk_keeps_the_removal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let mut db = library_db()?;
    rescan(&mut db, LIB, root)?;
    let media_id = only_media_id(&db, LIB);

    remove_media(&mut db, LIB, &media_id, false)?;
    rescan(&mut db, LIB, root)?;

    assert_eq!(
        removal_count(&db, LIB),
        1,
        "a varredura não desfaz a remoção"
    );
    assert!(
        root.join("Curso/Aula/aula.mp4").exists(),
        "o arquivo continua no disco"
    );
    let catalog = build_catalog(&db, LIB)?;
    assert!(
        all_files(&catalog).is_empty(),
        "a mídia removida continua fora do catálogo"
    );
    Ok(())
}

#[test]
fn a_rescan_after_the_file_disappears_keeps_the_removal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let mut db = library_db()?;
    rescan(&mut db, LIB, root)?;
    let media_id = only_media_id(&db, LIB);

    remove_media(&mut db, LIB, &media_id, false)?;
    std::fs::remove_file(root.join("Curso/Aula/aula.mp4"))?;
    rescan(&mut db, LIB, root)?;

    assert_eq!(removal_count(&db, LIB), 1);
    assert_eq!(
        availability_of(&db, &media_id),
        "missing",
        "a ausência é registrada, mas a remoção permanece"
    );
    let catalog = build_catalog(&db, LIB)?;
    assert!(all_files(&catalog).is_empty());
    Ok(())
}

#[test]
fn a_rescan_does_not_change_the_removal_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    write_file(root, "Curso/Aula/aula.mp4", b"conteudo")?;

    let mut db = library_db()?;
    rescan(&mut db, LIB, root)?;
    let media_id = only_media_id(&db, LIB);
    remove_media(&mut db, LIB, &media_id, false)?;

    let removed_at: i64 = db.connection().query_row(
        "SELECT removed_at FROM media_removal WHERE library_id = ?1",
        [LIB],
        |row| row.get(0),
    )?;

    rescan(&mut db, LIB, root)?;

    let after: i64 = db.connection().query_row(
        "SELECT removed_at FROM media_removal WHERE library_id = ?1",
        [LIB],
        |row| row.get(0),
    )?;
    assert_eq!(after, removed_at, "a remoção gravada não é reescrita");
    Ok(())
}

#[test]
fn a_failed_removal_rolls_back_and_preserves_the_progress() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    let mut db = Database::open(&path)?;
    insert_library(&db, LIB)?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 42, true)?;

    // Gatilho criado por uma segunda conexão ao mesmo arquivo: a remoção falha no meio.
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TRIGGER falha_na_remocao BEFORE INSERT ON media_removal
             BEGIN SELECT RAISE(ABORT, 'falha injetada'); END;",
        )?;
    }

    match remove_media(&mut db, LIB, "m1", true) {
        Err(Error::Database(rusqlite::Error::SqliteFailure(_, Some(message)))) => {
            assert!(
                message.contains("falha injetada"),
                "o erro precisa vir do gatilho injetado: {message}"
            );
        }
        other => panic!("esperava Error::Database do gatilho, obtive {other:?}"),
    }

    assert_eq!(removal_count(&db, LIB), 0, "nada pode ficar gravado");
    assert_eq!(
        progress_row(&db, OWNER, "m1"),
        Some((42, 1, SEEDED_AT)),
        "o progresso sobrevive à falha"
    );

    // O rollback libera a transação: a conexão continua utilizável e a remoção volta a
    // funcionar assim que o gatilho sai do caminho.
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch("DROP TRIGGER falha_na_remocao;")?;
    }

    let outcome = remove_media(&mut db, LIB, "m1", true)?;
    assert!(
        outcome.removed_now,
        "a conexão segue funcional após o rollback"
    );
    assert_eq!(removal_count(&db, LIB), 1);
    assert_eq!(progress_row(&db, OWNER, "m1"), Some((42, 1, SEEDED_AT)));
    Ok(())
}

#[test]
fn removal_does_not_modify_the_library() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    write_file(root, "Curso/Aula/a.mp4", b"a")?;
    write_file(root, "Curso/Aula/b.mp4", b"b")?;

    let before = manifest(root);

    let mut db = library_db()?;
    rescan(&mut db, LIB, root)?;
    let media_id = only_media_id(&db, LIB);
    remove_media(&mut db, LIB, &media_id, false)?;

    assert_eq!(manifest(root), before, "a biblioteca permanece intocada");
    Ok(())
}

#[test]
fn removal_does_not_touch_other_media() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    media(&db, "m2", "Curso/Aula/outra.mp4")?;
    seed_progress(&db, OWNER, "m2", 5, false)?;

    remove_media(&mut db, LIB, "m1", false)?;

    assert_eq!(media_count(&db, LIB), 2);
    assert_eq!(progress_row(&db, OWNER, "m2"), Some((5, 0, SEEDED_AT)));
    Ok(())
}

#[test]
fn a_removed_media_is_refused_even_when_absent_or_gone() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();

    // Removida e marcada como ausente na última varredura, com o arquivo ainda no disco:
    // a remoção é o motivo reportado, e não a disponibilidade.
    let mut db = library_db()?;
    write_file(root, "Curso/Aula/aula.mp4", b"a")?;
    insert_media(
        &db,
        LIB,
        "m1",
        "Curso/Aula/aula.mp4",
        1024,
        1_760_000_000,
        "missing",
    )?;
    remove_media(&mut db, LIB, "m1", false)?;

    match open_media(&db, LIB, "m1", root) {
        Err(Error::MediaRemoved { media_id }) => assert_eq!(media_id, "m1"),
        other => panic!("esperava Error::MediaRemoved, obtive {other:?}"),
    }

    // Removida com o arquivo já apagado: a remoção tem precedência sobre a ausência no disco.
    let dir2 = tempfile::tempdir()?;
    let root2 = dir2.path();

    let mut db2 = library_db()?;
    media(&db2, "m2", "Curso/Aula/outra.mp4")?;
    remove_media(&mut db2, LIB, "m2", false)?;

    match open_media(&db2, LIB, "m2", root2) {
        Err(Error::MediaRemoved { .. }) => {}
        other => panic!("esperava Error::MediaRemoved, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn impact_reports_a_removal_that_already_happened() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 42, true)?;

    let before = media_removal_impact(&db, LIB, "m1")?;
    assert_eq!(before.media_id, "m1");
    assert_eq!(before.relative_path, "Curso/Aula/aula.mp4");
    assert_eq!(before.availability, Availability::Available);
    assert_eq!(before.progress_owners, 1);
    assert!(!before.already_removed);

    remove_media(&mut db, LIB, "m1", true)?;

    let after = media_removal_impact(&db, LIB, "m1")?;
    assert!(after.already_removed, "a remoção passa a ser relatada");
    assert_eq!(after.progress_owners, 1, "o progresso continua lá");
    assert_eq!(
        after.availability,
        Availability::Available,
        "disponibilidade e remoção são fatos separados"
    );
    Ok(())
}

#[test]
fn an_idempotent_repeat_does_not_require_confirmation() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 42, true)?;

    let first = remove_media(&mut db, LIB, "m1", true)?;
    assert_eq!(first.media_id, "m1");
    assert!(first.removed_now);
    assert_eq!(first.preserved_owners, 1);

    // Repetir sem confirmação é inofensivo: nada mudaria com a operação.
    let again = remove_media(&mut db, LIB, "m1", false)?;
    assert_eq!(again.media_id, "m1");
    assert!(!again.removed_now);
    assert_eq!(again.preserved_owners, 1, "o progresso continua preservado");
    assert_eq!(removal_count(&db, LIB), 1);
    assert_eq!(progress_row(&db, OWNER, "m1"), Some((42, 1, SEEDED_AT)));
    Ok(())
}

#[test]
fn confirmation_error_reports_every_owner() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 10, false)?;
    seed_progress(&db, OTHER_OWNER, "m1", 20, false)?;

    match remove_media(&mut db, LIB, "m1", false) {
        Err(Error::MediaRemovalNeedsConfirmation {
            media_id,
            progress_owners,
        }) => {
            assert_eq!(media_id, "m1");
            assert_eq!(progress_owners, 2);
        }
        other => panic!("esperava Error::MediaRemovalNeedsConfirmation, obtive {other:?}"),
    }

    assert_eq!(removal_count(&db, LIB), 0, "a recusa não grava nada");
    assert_eq!(progress_row(&db, OWNER, "m1"), Some((10, 0, SEEDED_AT)));
    assert_eq!(
        progress_row(&db, OTHER_OWNER, "m1"),
        Some((20, 0, SEEDED_AT))
    );
    Ok(())
}

#[test]
fn impact_of_an_absent_media_reports_missing() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path();

    let mut db = library_db()?;
    write_file(root, "Curso/Aula/aula.mp4", b"a")?;
    insert_media(
        &db,
        LIB,
        "m1",
        "Curso/Aula/aula.mp4",
        1024,
        1_760_000_000,
        "missing",
    )?;

    let impact = media_removal_impact(&db, LIB, "m1")?;
    assert_eq!(impact.availability, Availability::Missing);
    assert_eq!(impact.progress_owners, 0);

    remove_media(&mut db, LIB, "m1", false)?;
    assert_eq!(removal_count(&db, LIB), 1);
    assert_eq!(
        availability_of(&db, "m1"),
        "missing",
        "a linha de media não muda com a remoção"
    );
    Ok(())
}

#[test]
fn progress_of_a_removed_media_is_still_read_and_written() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 42, true)?;
    remove_media(&mut db, LIB, "m1", true)?;

    // O progresso é preservado de propósito: a remoção não o apaga nem o torna ilegível.
    assert_eq!(
        study_progress(&db, OWNER, "m1")?.map(|state| state.position_seconds),
        Some(42)
    );
    record_position(&db, OWNER, "m1", 43)?;
    set_completed(&db, OWNER, "m1", false)?;
    assert_eq!(progress_row(&db, OWNER, "m1").map(|row| row.0), Some(43));
    Ok(())
}

#[test]
fn recent_study_applies_the_limit_after_hiding_removed_media() -> TestResult {
    let mut db = library_db()?;
    media(&db, "m1", "Curso/Aula/a.mp4")?;
    media(&db, "m2", "Curso/Aula/b.mp4")?;
    seed_progress(&db, OWNER, "m2", 1, false)?;
    seed_progress(&db, OWNER, "m1", 2, false)?;

    // m1 fica mais recente que m2.
    db.connection().execute(
        "UPDATE study_state SET updated_at = ?1 WHERE owner_id = ?2 AND media_id = ?3",
        rusqlite::params![SEEDED_AT + 10, OWNER, "m1"],
    )?;

    let before: Vec<String> = recent_study(&db, OWNER, 1)?
        .into_iter()
        .map(|state| state.media_id)
        .collect();
    assert_eq!(
        before,
        vec!["m1"],
        "sem remoção, o mais recente vem primeiro"
    );

    remove_media(&mut db, LIB, "m1", true)?;

    let after: Vec<String> = recent_study(&db, OWNER, 1)?
        .into_iter()
        .map(|state| state.media_id)
        .collect();
    assert_eq!(after, vec!["m2"], "o limite é aplicado depois do filtro");
    Ok(())
}

#[test]
fn recent_study_ignores_a_removal_recorded_for_another_library() -> TestResult {
    let db = library_db()?;
    insert_library(&db, OTHER_LIB)?;
    media(&db, "m1", "Curso/Aula/aula.mp4")?;
    seed_progress(&db, OWNER, "m1", 42, false)?;

    // Uma remoção gravada com o par divergente (outra biblioteca, mesma mídia) não é a
    // remoção desta mídia: o filtro confere o par, não apenas o identificador.
    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![OTHER_LIB, "m1", SEEDED_AT],
    )?;

    let recent = recent_study(&db, OWNER, 10)?;
    assert_eq!(recent.len(), 1, "a mídia continua nos recentes");
    assert_eq!(recent[0].media_id, "m1");
    Ok(())
}
