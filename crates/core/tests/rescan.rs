//! Testes da política de rescan e da persistência da descoberta.
//!
//! Cobrem a ausência confiável por subárvore, a reutilização de `media_id`, a ambiguidade
//! de chaves, o histórico de varreduras com falha, a atomicidade da aplicação e a garantia
//! de que a biblioteca física permanece intocada.

use std::path::Path;

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, MediaKind, RescanSummary, ScanOutcome, ScannedFile, apply_scan,
    rescan,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Identificador da biblioteca usada na maioria dos testes.
const LIB: &str = "bib-1";

fn scanned(path: &str, size: u64, mtime: i64, mode: KeyMode) -> ScannedFile {
    ScannedFile {
        relative_path: path.to_owned(),
        relative_path_key: relative_path_key(path, mode).expect("caminho de teste deve ser válido"),
        kind: MediaKind::Media,
        size,
        mtime,
    }
}

fn outcome(files: Vec<ScannedFile>, inaccessible: &[&str]) -> ScanOutcome {
    ScanOutcome {
        files,
        inaccessible: inaccessible
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect(),
    }
}

fn count(db: &Database, table: &str) -> i64 {
    db.connection()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("a contagem deve ser consultável")
}

/// Identidade, tamanho, mtime e disponibilidade de uma linha de mídia, por chave.
fn media_row(db: &Database, key: &str) -> Option<(String, i64, i64, String)> {
    db.connection()
        .query_row(
            "SELECT media_id, size, mtime, availability FROM media WHERE relative_path_key = ?1",
            rusqlite::params![key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .ok()
}

fn scan_rows(db: &Database) -> Vec<(String, i64)> {
    let mut statement = db
        .connection()
        .prepare("SELECT status, complete FROM scan ORDER BY scan_id")
        .expect("a tabela scan deve ser consultável");
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("a consulta de varreduras deve funcionar");
    rows.map(|row| row.expect("linha de varredura")).collect()
}

fn library_available(db: &Database, library_id: &str) -> Option<i64> {
    db.connection()
        .query_row(
            "SELECT available FROM library WHERE library_id = ?1",
            rusqlite::params![library_id],
            |row| row.get(0),
        )
        .ok()
}

fn record_progress(
    db: &Database,
    owner: &str,
    media_id: &str,
    position: i64,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES (?1, ?2, ?3, 0, ?4)",
        rusqlite::params![owner, media_id, position, 1_760_000_000_i64],
    )
}

fn progress_position(db: &Database, owner: &str, media_id: &str) -> Option<i64> {
    db.connection()
        .query_row(
            "SELECT position_seconds FROM study_state WHERE owner_id = ?1 AND media_id = ?2",
            rusqlite::params![owner, media_id],
            |row| row.get(0),
        )
        .ok()
}

/// Manifesto recursivo de uma árvore: caminho relativo, tamanho e mtime em nanossegundos.
fn manifest(root: &Path) -> Vec<(String, u64, u64)> {
    fn walk(base: &Path, current: &Path, entries: &mut Vec<(String, u64, u64)>) {
        let mut children: Vec<_> = std::fs::read_dir(current)
            .expect("o diretório do manifesto deve ser legível")
            .map(|entry| entry.expect("entrada do manifesto"))
            .collect();
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let path = child.path();
            let relative = path
                .strip_prefix(base)
                .expect("o caminho pertence à raiz")
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = child.metadata().expect("metadados do manifesto");
            if metadata.is_dir() {
                entries.push((relative, 0, 0));
                walk(base, &path, entries);
            } else {
                let modified = metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX))
                    .unwrap_or(0);
                entries.push((relative, metadata.len(), modified));
            }
        }
    }

    let mut entries = Vec::new();
    walk(root, root, &mut entries);
    entries
}

#[test]
fn the_first_scan_persists_every_file_and_creates_the_library() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    let summary = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode),
                scanned("Curso/apoio.pdf", 2_048, 1_760_000_100, mode),
            ],
            &[],
        ),
        mode,
    )?;

    assert_eq!(summary.inserted, 2);
    assert_eq!(summary.updated, 0);
    assert_eq!(summary.marked_missing, 0);
    assert!(summary.complete);
    assert_eq!(count(&db, "media"), 2);
    assert_eq!(library_available(&db, LIB), Some(1));
    assert_eq!(scan_rows(&db), vec![("completed".to_owned(), 1)]);

    let (_, size, mtime, availability) =
        media_row(&db, "Curso/aula.mp4").expect("a mídia deve estar catalogada");
    assert_eq!(size, 1_024);
    assert_eq!(mtime, 1_760_000_000);
    assert_eq!(availability, "available");
    Ok(())
}

#[test]
fn a_second_identical_scan_neither_inserts_nor_changes_identity() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;
    let files = || {
        vec![
            scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode),
            scanned("Curso/apoio.pdf", 2_048, 1_760_000_100, mode),
        ]
    };

    apply_scan(&mut db, LIB, &outcome(files(), &[]), mode)?;
    let first = media_row(&db, "Curso/aula.mp4").expect("mídia catalogada");

    let summary = apply_scan(&mut db, LIB, &outcome(files(), &[]), mode)?;

    assert_eq!(summary.inserted, 0);
    assert_eq!(summary.updated, 2);
    assert_eq!(count(&db, "media"), 2);
    assert_eq!(
        media_row(&db, "Curso/aula.mp4").expect("mídia catalogada"),
        first,
        "a identidade não pode mudar em uma varredura idêntica"
    );
    Ok(())
}

#[test]
fn changed_metadata_updates_the_existing_row() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode)],
            &[],
        ),
        mode,
    )?;
    let (media_id, _, _, _) = media_row(&db, "Curso/aula.mp4").expect("mídia catalogada");

    let summary = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/aula.mp4", 4_096, 1_760_000_500, mode)],
            &[],
        ),
        mode,
    )?;

    assert_eq!(summary.inserted, 0);
    assert_eq!(summary.updated, 1);
    let (updated_id, size, mtime, availability) =
        media_row(&db, "Curso/aula.mp4").expect("mídia catalogada");
    assert_eq!(updated_id, media_id);
    assert_eq!(size, 4_096);
    assert_eq!(mtime, 1_760_000_500);
    assert_eq!(availability, "available");
    Ok(())
}

#[test]
fn absence_in_a_complete_scan_marks_the_row_missing_and_keeps_its_identity() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode),
                scanned("Curso/outra.mp4", 2_048, 1_760_000_000, mode),
            ],
            &[],
        ),
        mode,
    )?;
    let (media_id, _, _, _) = media_row(&db, "Curso/aula.mp4").expect("mídia catalogada");

    let summary = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/outra.mp4", 2_048, 1_760_000_000, mode)],
            &[],
        ),
        mode,
    )?;

    assert_eq!(summary.marked_missing, 1);
    assert!(summary.complete);
    let (missing_id, _, _, availability) =
        media_row(&db, "Curso/aula.mp4").expect("a linha deve permanecer");
    assert_eq!(missing_id, media_id);
    assert_eq!(availability, "missing");
    assert_eq!(count(&db, "media"), 2, "nenhuma linha pode ser excluída");
    Ok(())
}

#[test]
fn a_file_reappearing_at_the_same_key_reuses_its_identity_and_progress() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;
    let present = || vec![scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode)];

    apply_scan(&mut db, LIB, &outcome(present(), &[]), mode)?;
    let (media_id, _, _, _) = media_row(&db, "Curso/aula.mp4").expect("mídia catalogada");
    record_progress(&db, "dono-1", &media_id, 42)?;

    apply_scan(&mut db, LIB, &outcome(Vec::new(), &[]), mode)?;
    assert_eq!(
        media_row(&db, "Curso/aula.mp4")
            .expect("linha preservada")
            .3,
        "missing"
    );

    apply_scan(&mut db, LIB, &outcome(present(), &[]), mode)?;

    let reused = media_row(&db, "Curso/aula.mp4").expect("mídia catalogada");
    assert_eq!(reused.0, media_id, "o media_id deve ser reutilizado");
    assert_eq!(reused.3, "available");
    assert_eq!(count(&db, "media"), 1);
    assert_eq!(progress_position(&db, "dono-1", &media_id), Some(42));
    Ok(())
}

#[test]
fn media_under_an_unreadable_subtree_is_preserved_while_absence_elsewhere_is_recorded() -> TestResult
{
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("A/aula.mp4", 1_024, 1_760_000_000, mode),
                scanned("B/aula.mp4", 1_024, 1_760_000_000, mode),
                scanned("C/aula.mp4", 1_024, 1_760_000_000, mode),
            ],
            &[],
        ),
        mode,
    )?;

    // `A` não pôde ser lida e `C/aula.mp4` desapareceu de uma área verificada.
    let summary = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("B/aula.mp4", 1_024, 1_760_000_000, mode)],
            &["A"],
        ),
        mode,
    )?;

    assert!(
        !summary.complete,
        "uma subárvore ilegível torna a varredura parcial"
    );
    assert_eq!(summary.preserved, 1);
    assert_eq!(summary.marked_missing, 1);
    assert_eq!(
        media_row(&db, "A/aula.mp4").expect("linha preservada").3,
        "available",
        "a subárvore ilegível não pode ser tratada como ausência"
    );
    assert_eq!(
        media_row(&db, "C/aula.mp4").expect("linha preservada").3,
        "missing",
        "a ausência fora da área não verificada ainda é registrada"
    );
    Ok(())
}

#[test]
fn a_walk_error_without_location_freezes_every_absence_decision() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("A/aula.mp4", 1_024, 1_760_000_000, mode),
                scanned("B/aula.mp4", 1_024, 1_760_000_000, mode),
            ],
            &[],
        ),
        mode,
    )?;

    let summary = apply_scan(&mut db, LIB, &outcome(Vec::new(), &["."]), mode)?;

    assert!(!summary.complete);
    assert_eq!(summary.marked_missing, 0);
    assert_eq!(summary.preserved, 2);
    for key in ["A/aula.mp4", "B/aula.mp4"] {
        assert_eq!(
            media_row(&db, key).expect("linha preservada").3,
            "available",
            "sem saber o que foi afetado, nenhuma ausência pode ser registrada"
        );
    }
    Ok(())
}

#[test]
fn a_partial_scan_still_records_absence_outside_the_unverified_area() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("Bloqueado/aula.mp4", 1_024, 1_760_000_000, mode),
                scanned("Livre/aula.mp4", 1_024, 1_760_000_000, mode),
                scanned("Livre/outra.mp4", 1_024, 1_760_000_000, mode),
            ],
            &[],
        ),
        mode,
    )?;

    let summary = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Livre/aula.mp4", 1_024, 1_760_000_000, mode)],
            &["Bloqueado"],
        ),
        mode,
    )?;

    assert!(!summary.complete, "a varredura é parcial, não vazia");
    assert_eq!(summary.marked_missing, 1);
    assert_eq!(summary.preserved, 1);
    assert_eq!(
        media_row(&db, "Livre/outra.mp4").expect("linha").3,
        "missing"
    );
    assert_eq!(
        media_row(&db, "Bloqueado/aula.mp4").expect("linha").3,
        "available"
    );
    Ok(())
}

#[test]
fn ambiguous_keys_are_reported_and_never_applied() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(vec![scanned("a/b.mp4", 1_024, 1_760_000_000, mode)], &[]),
        mode,
    )?;
    let before = media_row(&db, "a/b.mp4").expect("mídia catalogada");

    // No Unix, `a\b.mp4` é um nome de arquivo que produz a mesma chave de `a/b.mp4`.
    let summary = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("a/b.mp4", 9_999, 1_760_000_900, mode),
                scanned("a\\b.mp4", 7_777, 1_760_000_700, mode),
            ],
            &[],
        ),
        mode,
    )?;

    assert_eq!(summary.ambiguous, vec!["a/b.mp4".to_owned()]);
    assert_eq!(summary.inserted, 0);
    assert_eq!(summary.updated, 0);
    assert_eq!(summary.marked_missing, 0);
    assert_eq!(count(&db, "media"), 1);
    assert_eq!(
        media_row(&db, "a/b.mp4").expect("linha intacta"),
        before,
        "uma chave ambígua não pode receber dados de nenhum dos arquivos"
    );
    Ok(())
}

#[test]
fn no_scan_deletes_a_media_row() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("A.mp4", 10, 1_760_000_000, mode),
                scanned("B.mp4", 20, 1_760_000_000, mode),
            ],
            &[],
        ),
        mode,
    )?;

    for step in 0..3 {
        let files = if step % 2 == 0 {
            Vec::new()
        } else {
            vec![scanned("A.mp4", 10, 1_760_000_000, mode)]
        };
        apply_scan(&mut db, LIB, &outcome(files, &[]), mode)?;
        assert!(
            count(&db, "media") >= 2,
            "nenhuma varredura exclui registros"
        );
    }
    assert_eq!(count(&db, "media"), 2);
    Ok(())
}

#[test]
fn case_insensitive_mode_reuses_the_row_when_only_the_case_changes() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Insensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/Aula.mp4", 1_024, 1_760_000_000, mode)],
            &[],
        ),
        mode,
    )?;
    let (media_id, _, _, _) = media_row(&db, "curso/aula.mp4").expect("chave dobrada");

    let summary = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("curso/aula.mp4", 1_024, 1_760_000_000, mode)],
            &[],
        ),
        mode,
    )?;

    assert_eq!(summary.inserted, 0);
    assert_eq!(summary.updated, 1);
    assert_eq!(count(&db, "media"), 1);
    assert_eq!(
        media_row(&db, "curso/aula.mp4").expect("linha").0,
        media_id,
        "a diferença de caixa não cria uma segunda identidade"
    );
    Ok(())
}

#[test]
fn applying_the_same_plan_twice_is_idempotent() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;
    let plan = outcome(
        vec![
            scanned("A.mp4", 10, 1_760_000_000, mode),
            scanned("B.mp4", 20, 1_760_000_000, mode),
        ],
        &[],
    );

    let first: RescanSummary = apply_scan(&mut db, LIB, &plan, mode)?;
    let second = apply_scan(&mut db, LIB, &plan, mode)?;

    assert_eq!(first.inserted, 2);
    assert_eq!(second.inserted, 0);
    assert_eq!(second.updated, 2);
    assert_eq!(count(&db, "media"), 2);
    assert_eq!(
        count(&db, "scan"),
        2,
        "cada aplicação registra a sua varredura"
    );
    Ok(())
}

#[test]
fn a_failed_apply_rolls_the_whole_transaction_back() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");
    let mut db = Database::open(&path)?;
    let mode = KeyMode::Sensitive;

    // Injeção determinística de falha no meio da aplicação: um gatilho que aborta
    // qualquer inserção de mídia. O gatilho é criado por outra conexão ao mesmo arquivo,
    // sem expor acesso interno do `Database`.
    {
        let injected = rusqlite::Connection::open(&path)?;
        injected.execute_batch(
            "CREATE TRIGGER falha_na_midia BEFORE INSERT ON media
             BEGIN SELECT RAISE(ABORT, 'falha injetada'); END;",
        )?;
    }

    let result = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("A.mp4", 10, 1_760_000_000, mode),
                scanned("B.mp4", 20, 1_760_000_000, mode),
            ],
            &[],
        ),
        mode,
    );

    assert!(result.is_err(), "a aplicação deve falhar com o gatilho");
    assert_eq!(count(&db, "media"), 0, "nada pode sobreviver ao rollback");
    assert_eq!(
        count(&db, "scan"),
        0,
        "a varredura não pode ficar registrada"
    );
    assert_eq!(count(&db, "scan_inaccessible"), 0);
    assert_eq!(
        library_available(&db, LIB),
        None,
        "a biblioteca também faz parte da mesma transação"
    );
    Ok(())
}

#[test]
fn a_missing_root_records_a_failed_scan_without_touching_the_library() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;
    let dir = tempfile::tempdir()?;
    let missing = dir.path().join("biblioteca-ausente");

    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        rusqlite::params![LIB],
    )?;
    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode)],
            &[],
        ),
        mode,
    )?;

    let result = rescan(&mut db, LIB, &missing);

    match result {
        Err(Error::LibraryRootMissing { .. }) => {}
        other => panic!("esperava Error::LibraryRootMissing, obtive {other:?}"),
    }
    assert_eq!(
        scan_rows(&db),
        vec![("completed".to_owned(), 1), ("failed".to_owned(), 0)],
        "a tentativa fracassada entra no histórico como parcial"
    );
    assert_eq!(
        library_available(&db, LIB),
        Some(1),
        "a falha não altera a disponibilidade registrada da biblioteca"
    );
    assert_eq!(count(&db, "media"), 1);
    assert_eq!(
        media_row(&db, "Curso/aula.mp4").expect("linha").3,
        "available"
    );
    Ok(())
}

#[test]
fn a_failed_first_scan_creates_the_library_row_as_unavailable() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let dir = tempfile::tempdir()?;

    let result = rescan(&mut db, LIB, &dir.path().join("ausente"));

    match result {
        Err(Error::LibraryRootMissing { .. }) => {}
        other => panic!("esperava Error::LibraryRootMissing, obtive {other:?}"),
    }
    assert_eq!(library_available(&db, LIB), Some(0));
    assert_eq!(scan_rows(&db), vec![("failed".to_owned(), 0)]);
    assert_eq!(count(&db, "media"), 0);
    Ok(())
}

#[test]
fn rescanning_a_real_tree_is_read_only_and_complete() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let library = tempfile::tempdir()?;
    let root = library.path();

    std::fs::create_dir_all(root.join("Curso"))?;
    std::fs::create_dir_all(root.join("Curso/.oculto"))?;
    std::fs::write(root.join("Curso/aula.mp4"), vec![7_u8; 1_024])?;
    std::fs::write(root.join("Curso/legenda.srt"), "1\n")?;
    std::fs::write(root.join("Curso/.oculto/segredo.mp4"), vec![7_u8; 16])?;

    let before = manifest(root);

    let summary = rescan(&mut db, LIB, root)?;

    assert!(summary.complete);
    assert_eq!(summary.inserted, 2, "o diretório oculto não é catalogado");
    assert_eq!(count(&db, "media"), 2);

    assert_eq!(
        manifest(root),
        before,
        "a varredura não pode criar, alterar ou remover nada na biblioteca"
    );

    // Segunda varredura sobre a mesma árvore: nada novo e nada marcado como ausente.
    let second = rescan(&mut db, LIB, root)?;
    assert_eq!(second.inserted, 0);
    assert_eq!(second.marked_missing, 0);
    assert_eq!(manifest(root), before);
    Ok(())
}

#[test]
fn a_partial_scan_records_the_unverified_areas_and_marks_complete_zero() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    let summary = apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode)],
            &["A"],
        ),
        mode,
    )?;

    assert!(
        !summary.complete,
        "com área não verificada a varredura é parcial"
    );
    assert_eq!(scan_rows(&db), vec![("completed".to_owned(), 0)]);

    let mut statement = db
        .connection()
        .prepare("SELECT relative_path FROM scan_inaccessible ORDER BY relative_path")?;
    let recorded: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    assert_eq!(
        recorded,
        vec!["A".to_owned()],
        "a área não verificada precisa ficar registrada na varredura"
    );
    Ok(())
}

#[test]
fn a_row_already_missing_is_not_counted_again() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;
    let present = || vec![scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode)];

    apply_scan(&mut db, LIB, &outcome(present(), &[]), mode)?;
    let absent = apply_scan(&mut db, LIB, &outcome(Vec::new(), &[]), mode)?;
    let again = apply_scan(&mut db, LIB, &outcome(Vec::new(), &[]), mode)?;

    assert_eq!(absent.marked_missing, 1);
    assert_eq!(again.marked_missing, 0, "a linha já está ausente");
    assert_eq!(again.preserved, 0, "não havia área não verificada");
    assert_eq!(
        count(&db, "media"),
        1,
        "a linha ausente continua no catálogo"
    );
    Ok(())
}

#[test]
fn progress_survives_a_marked_absence_and_a_failed_scan() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;
    let dir = tempfile::tempdir()?;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode)],
            &[],
        ),
        mode,
    )?;
    let media_id = media_row(&db, "Curso/aula.mp4").expect("linha").0;
    record_progress(&db, "dono-1", &media_id, 42)?;

    apply_scan(&mut db, LIB, &outcome(Vec::new(), &[]), mode)?;
    assert_eq!(
        progress_position(&db, "dono-1", &media_id),
        Some(42),
        "marcar a mídia como ausente não pode apagar o progresso"
    );

    let failed = rescan(&mut db, LIB, &dir.path().join("ausente"));
    match failed {
        Err(Error::LibraryRootMissing { .. }) => {}
        other => panic!("esperava Error::LibraryRootMissing, obtive {other:?}"),
    }
    assert_eq!(
        progress_position(&db, "dono-1", &media_id),
        Some(42),
        "uma varredura com falha não pode apagar o progresso"
    );
    assert_eq!(
        media_row(&db, "Curso/aula.mp4").expect("linha").3,
        "missing",
        "a falha não reverte a ausência já registrada"
    );
    Ok(())
}

#[test]
fn an_unverified_prefix_does_not_cover_a_sibling_with_a_shared_prefix() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![
                scanned("A/aula.mp4", 1_024, 1_760_000_000, mode),
                scanned("AB/outro.mp4", 1_024, 1_760_000_000, mode),
            ],
            &[],
        ),
        mode,
    )?;

    let summary = apply_scan(&mut db, LIB, &outcome(Vec::new(), &["A"]), mode)?;

    assert_eq!(
        summary.preserved, 1,
        "só a árvore realmente contida em A é preservada"
    );
    assert_eq!(summary.marked_missing, 1, "AB não está dentro de A");
    assert_eq!(media_row(&db, "A/aula.mp4").expect("linha").3, "available");
    assert_eq!(media_row(&db, "AB/outro.mp4").expect("linha").3, "missing");
    Ok(())
}

#[test]
fn libraries_are_isolated_when_deciding_absence() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;
    let files = || vec![scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode)];

    apply_scan(&mut db, LIB, &outcome(files(), &[]), mode)?;
    apply_scan(&mut db, "bib-2", &outcome(files(), &[]), mode)?;

    let summary = apply_scan(&mut db, LIB, &outcome(Vec::new(), &[]), mode)?;

    let availability_of = |library: &str| -> String {
        db.connection()
            .query_row(
                "SELECT availability FROM media WHERE library_id = ?1 AND relative_path_key = ?2",
                rusqlite::params![library, "Curso/aula.mp4"],
                |row| row.get(0),
            )
            .expect("a linha da biblioteca deve existir")
    };

    assert_eq!(
        summary.marked_missing, 1,
        "só a biblioteca varrida é decidida"
    );
    assert_eq!(count(&db, "media"), 2, "nenhuma linha foi removida");
    assert_eq!(availability_of(LIB), "missing");
    assert_eq!(availability_of("bib-2"), "available");
    Ok(())
}

#[test]
fn a_failed_scan_is_followed_by_a_scan_that_restores_availability() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;
    let dir = tempfile::tempdir()?;

    let failed = rescan(&mut db, LIB, &dir.path().join("ausente"));
    assert!(
        matches!(failed, Err(Error::LibraryRootMissing { .. })),
        "a raiz ausente precisa ser reportada como tal"
    );
    assert_eq!(library_available(&db, LIB), Some(0));

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/aula.mp4", 1_024, 1_760_000_000, mode)],
            &[],
        ),
        mode,
    )?;

    assert_eq!(
        library_available(&db, LIB),
        Some(1),
        "uma varredura bem-sucedida volta a marcar a raiz como alcançável"
    );
    Ok(())
}

#[test]
fn an_oversized_file_saturates_the_stored_size() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let mode = KeyMode::Sensitive;

    apply_scan(
        &mut db,
        LIB,
        &outcome(
            vec![scanned("Curso/gigante.mp4", u64::MAX, 1_760_000_000, mode)],
            &[],
        ),
        mode,
    )?;

    assert_eq!(
        media_row(&db, "Curso/gigante.mp4").expect("linha").1,
        i64::MAX,
        "um tamanho acima do limite assinado satura em vez de falhar"
    );
    Ok(())
}

#[test]
fn a_failed_scan_history_write_preserves_the_original_failure() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");
    let mut db = Database::open(&path)?;

    // O histórico também pode falhar. O gatilho é criado por outra conexão ao mesmo
    // arquivo, sem expor acesso interno do `Database`.
    {
        let injected = rusqlite::Connection::open(&path)?;
        injected.execute_batch(
            "CREATE TRIGGER falha_no_historico BEFORE INSERT ON scan
             BEGIN SELECT RAISE(ABORT, 'falha injetada'); END;",
        )?;
    }

    let result = rescan(&mut db, LIB, &dir.path().join("ausente"));

    match result {
        Err(Error::ScanHistoryNotRecorded { scan_error, .. }) => assert!(
            matches!(*scan_error, Error::LibraryRootMissing { .. }),
            "a razão original da falha precisa sobreviver ao erro de escrita"
        ),
        other => panic!("esperava Error::ScanHistoryNotRecorded, obtive {other:?}"),
    }
    assert_eq!(count(&db, "scan"), 0, "o histórico não foi gravado");
    assert_eq!(
        count(&db, "library"),
        0,
        "a transação do histórico também foi revertida"
    );
    Ok(())
}
