//! Testes da persistência e da atualização do progresso de estudo.
//!
//! O núcleo recebe `owner_id` e `media_id` — identificadores opacos — e mantém a posição de
//! retomada e a conclusão da aula. O `owner_id` é o limite de isolamento: dados de um dono
//! nunca são visíveis para outro. Nenhum caso acessa o sistema de arquivos.

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, MAX_RECENT_STUDY, course_study_summary, recent_study,
    record_position, set_completed, study_analytics, study_progress,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "bib-1";
const OWNER: &str = "aluno-1";
const OTHER: &str = "aluno-2";

/// Instante conhecido gravado direto no banco. Qualquer escrita real usa o relógio do
/// sistema, que é ordens de grandeza maior, então a comparação não depende de espera.
const SEEDED_AT: i64 = 1_000;

fn key(path: &str) -> String {
    relative_path_key(path, KeyMode::for_host()).expect("caminho de teste válido")
}

fn library_db() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    Ok(db)
}

fn insert_media(db: &Database, media_id: &str, availability: &str) -> rusqlite::Result<usize> {
    let path = format!("curso/{media_id}.mp4");
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![media_id, LIB, path, key(&path), 1024, 1_760_000_000, availability],
    )
}

/// Grava um estado de estudo com `updated_at` controlado, sem passar pela API.
fn seed(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    completed: bool,
    updated_at: i64,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            owner_id,
            media_id,
            position_seconds,
            i64::from(completed),
            updated_at
        ],
    )
}

/// Lê `(position_seconds, completed, updated_at)` direto do banco.
fn row(db: &Database, owner_id: &str, media_id: &str) -> Option<(i64, i64, i64)> {
    db.connection()
        .query_row(
            "SELECT position_seconds, completed, updated_at FROM study_state
             WHERE owner_id = ?1 AND media_id = ?2",
            rusqlite::params![owner_id, media_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()
}

fn recent_ids(db: &Database, owner_id: &str, limit: usize) -> Vec<String> {
    recent_study(db, owner_id, limit)
        .expect("consulta de recentes")
        .into_iter()
        .map(|state| state.media_id)
        .collect()
}

fn count(db: &Database, table: &str) -> i64 {
    db.connection()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("contagem")
}

/// Conteúdo das linhas de mídia, para provar que o progresso não altera nenhuma coluna.
fn media_snapshot(db: &Database) -> Vec<(String, String, String, i64, i64, String)> {
    let conn = db.connection();
    let mut statement = conn
        .prepare(
            "SELECT media_id, library_id, relative_path, size, mtime, availability
             FROM media ORDER BY media_id",
        )
        .expect("preparo da consulta");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .expect("consulta de mídia");

    rows.map(|row| row.expect("linha de mídia")).collect()
}

#[test]
fn progress_of_an_unknown_pair_is_absent_and_is_not_an_error() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;

    assert!(study_progress(&db, OWNER, "m1")?.is_none());
    Ok(())
}

#[test]
fn recording_a_position_creates_the_row_with_default_completion() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;

    let state = record_position(&db, OWNER, "m1", 42)?;

    assert_eq!(state.position_seconds, 42);
    assert!(!state.completed, "uma posição nova não conclui a aula");
    assert!(state.updated_at > SEEDED_AT, "o estado real usa o relógio");
    assert_eq!(row(&db, OWNER, "m1"), Some((42, 0, state.updated_at)));
    Ok(())
}

#[test]
fn recording_a_position_preserves_completion() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    set_completed(&db, OWNER, "m1", true)?;

    let state = record_position(&db, OWNER, "m1", 90)?;

    assert!(
        state.completed,
        "concluir e depois avançar não desfaz a conclusão"
    );
    assert_eq!(state.position_seconds, 90);
    assert_eq!(row(&db, OWNER, "m1").map(|r| r.1), Some(1));
    Ok(())
}

#[test]
fn marking_completed_preserves_the_position() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    record_position(&db, OWNER, "m1", 42)?;

    let state = set_completed(&db, OWNER, "m1", true)?;

    assert_eq!(
        state.position_seconds, 42,
        "marcar a conclusão não pode mover o ponto de retomada"
    );
    assert!(state.completed);
    assert_eq!(row(&db, OWNER, "m1").map(|r| r.0), Some(42));
    Ok(())
}

#[test]
fn repeating_the_same_position_does_not_touch_the_timestamp() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    seed(&db, OWNER, "m1", 42, true, SEEDED_AT)?;

    let state = record_position(&db, OWNER, "m1", 42)?;

    assert_eq!(
        state.updated_at, SEEDED_AT,
        "reenviar o mesmo valor não é uma alteração"
    );
    assert_eq!(state.position_seconds, 42);
    assert!(state.completed);
    assert_eq!(row(&db, OWNER, "m1"), Some((42, 1, SEEDED_AT)));
    Ok(())
}

#[test]
fn repeating_the_same_completion_does_not_touch_the_timestamp() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    seed(&db, OWNER, "m1", 7, true, SEEDED_AT)?;

    let state = set_completed(&db, OWNER, "m1", true)?;

    assert_eq!(
        state.updated_at, SEEDED_AT,
        "reenviar a mesma conclusão não é uma alteração"
    );
    assert_eq!(state.position_seconds, 7);
    assert_eq!(row(&db, OWNER, "m1"), Some((7, 1, SEEDED_AT)));
    Ok(())
}

#[test]
fn a_real_change_moves_the_timestamp() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    seed(&db, OWNER, "m1", 42, false, SEEDED_AT)?;

    let moved = record_position(&db, OWNER, "m1", 43)?;
    assert!(
        moved.updated_at > SEEDED_AT,
        "uma posição diferente é uma alteração real"
    );

    let completed = set_completed(&db, OWNER, "m1", true)?;
    assert!(
        completed.updated_at > SEEDED_AT,
        "uma conclusão diferente é uma alteração real"
    );

    let cleared = set_completed(&db, OWNER, "m1", false)?;
    assert!(
        cleared.updated_at > SEEDED_AT,
        "desmarcar também é alteração"
    );
    Ok(())
}

#[test]
fn repeating_operations_neither_reorder_nor_change_recent_study() -> TestResult {
    let db = library_db()?;
    for media in ["m1", "m2", "m3"] {
        insert_media(&db, media, "available")?;
    }
    seed(&db, OWNER, "m1", 10, false, 300)?;
    seed(&db, OWNER, "m2", 20, true, 200)?;
    seed(&db, OWNER, "m3", 30, false, 100)?;

    assert_eq!(recent_ids(&db, OWNER, 10), ["m1", "m2", "m3"]);

    record_position(&db, OWNER, "m1", 10)?;
    set_completed(&db, OWNER, "m2", true)?;

    assert_eq!(
        recent_ids(&db, OWNER, 10),
        ["m1", "m2", "m3"],
        "operações repetidas não podem reordenar os recentes"
    );
    assert_eq!(row(&db, OWNER, "m1"), Some((10, 0, 300)));
    assert_eq!(row(&db, OWNER, "m2"), Some((20, 1, 200)));
    assert_eq!(row(&db, OWNER, "m3"), Some((30, 0, 100)));
    Ok(())
}

#[test]
fn recent_study_is_ordered_from_the_most_recent_change() -> TestResult {
    let db = library_db()?;
    for media in ["m1", "m2", "m3"] {
        insert_media(&db, media, "available")?;
    }
    seed(&db, OWNER, "m1", 0, false, 100)?;
    seed(&db, OWNER, "m2", 0, false, 300)?;
    seed(&db, OWNER, "m3", 0, false, 200)?;

    assert_eq!(recent_ids(&db, OWNER, 10), ["m2", "m3", "m1"]);
    Ok(())
}

#[test]
fn recent_study_breaks_ties_by_media_id() -> TestResult {
    let db = library_db()?;
    for media in ["m1", "m2"] {
        insert_media(&db, media, "available")?;
    }
    seed(&db, OWNER, "m2", 0, false, 500)?;
    seed(&db, OWNER, "m1", 0, false, 500)?;

    assert_eq!(
        recent_ids(&db, OWNER, 10),
        ["m1", "m2"],
        "o empate precisa de uma ordem determinística"
    );
    Ok(())
}

#[test]
fn recent_study_respects_the_limit_and_an_empty_limit() -> TestResult {
    let db = library_db()?;
    for media in ["m1", "m2", "m3"] {
        insert_media(&db, media, "available")?;
    }
    seed(&db, OWNER, "m1", 0, false, 100)?;
    seed(&db, OWNER, "m2", 0, false, 200)?;
    seed(&db, OWNER, "m3", 0, false, 300)?;

    assert_eq!(recent_ids(&db, OWNER, 2), ["m3", "m2"]);
    assert!(recent_ids(&db, OWNER, 0).is_empty());
    Ok(())
}

#[test]
fn recent_study_never_exceeds_its_ceiling() -> TestResult {
    let db = library_db()?;
    let total = MAX_RECENT_STUDY + 5;
    for index in 0..total {
        let media = format!("m{index:04}");
        insert_media(&db, &media, "available")?;
        seed(&db, OWNER, &media, 0, false, index as i64)?;
    }

    let states = recent_study(&db, OWNER, usize::MAX)?;

    assert_eq!(
        states.len(),
        MAX_RECENT_STUDY,
        "o teto é fixo e não depende de configuração"
    );
    assert_eq!(states[0].media_id, format!("m{:04}", total - 1));
    assert_eq!(
        states.last().map(|state| state.media_id.as_str()),
        Some(format!("m{:04}", total - MAX_RECENT_STUDY).as_str()),
        "o teto corta os mais antigos, não os mais recentes"
    );
    assert!(
        !states.iter().any(|state| state.media_id == "m0000"),
        "o mais antigo fica fora do teto"
    );
    Ok(())
}

#[test]
fn study_progress_is_isolated_by_owner() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    record_position(&db, OWNER, "m1", 42)?;
    set_completed(&db, OWNER, "m1", true)?;

    assert!(study_progress(&db, OTHER, "m1")?.is_none());

    let other = record_position(&db, OTHER, "m1", 7)?;
    assert_eq!(other.position_seconds, 7);
    assert!(!other.completed);

    assert_eq!(
        row(&db, OWNER, "m1").map(|r| (r.0, r.1)),
        Some((42, 1)),
        "o progresso do primeiro dono não pode ser alterado pelo segundo"
    );
    assert_eq!(recent_ids(&db, OWNER, 10), ["m1"]);
    assert_eq!(recent_ids(&db, OTHER, 10), ["m1"]);
    assert_eq!(
        recent_study(&db, OTHER, 10)?[0].position_seconds,
        7,
        "cada dono vê o próprio progresso"
    );
    Ok(())
}

#[test]
fn course_study_summary_is_owner_scoped_and_ignores_unavailable_or_removed_media() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    insert_media(&db, "m2", "available")?;
    insert_media(&db, "m3", "missing")?;
    set_completed(&db, OWNER, "m1", true)?;
    set_completed(&db, OTHER, "m2", true)?;
    set_completed(&db, OWNER, "m3", true)?;
    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![LIB, "m2", SEEDED_AT],
    )?;

    assert_eq!(
        course_study_summary(&db, OWNER)?,
        [learnvault_core::CourseStudySummary {
            course_id: LIB.to_owned(),
            completed_media: 1,
        }]
    );
    assert!(course_study_summary(&db, OTHER)?.is_empty());
    Ok(())
}

#[test]
fn analytics_are_owner_scoped_and_exclude_removed_media() -> TestResult {
    let db = library_db()?;
    for media in ["m1", "m2", "m3"] {
        insert_media(&db, media, "available")?;
    }
    seed(&db, OWNER, "m1", 30, true, 100)?;
    seed(&db, OWNER, "m2", 20, false, 200)?;
    seed(&db, OTHER, "m3", 90, true, 300)?;
    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![LIB, "m2", 400],
    )?;
    assert_eq!(
        study_analytics(&db, OWNER)?,
        learnvault_core::StudyAnalytics {
            tracked_count: 1,
            completed_count: 1,
            total_position_seconds: 30,
            last_activity_at: Some(100),
        }
    );
    assert_eq!(study_analytics(&db, OTHER)?.tracked_count, 1);
    assert!(matches!(
        study_analytics(&db, ""),
        Err(Error::InvalidOwnerId)
    ));
    Ok(())
}

#[test]
fn progress_is_isolated_between_media() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    insert_media(&db, "m2", "available")?;

    record_position(&db, OWNER, "m1", 11)?;
    record_position(&db, OWNER, "m2", 22)?;
    set_completed(&db, OWNER, "m2", true)?;

    assert_eq!(
        study_progress(&db, OWNER, "m1")?.map(|s| s.position_seconds),
        Some(11)
    );
    assert_eq!(
        study_progress(&db, OWNER, "m2")?.map(|state| state.completed),
        Some(true)
    );
    Ok(())
}

#[test]
fn an_unknown_media_is_reported_and_nothing_is_written() -> TestResult {
    let db = library_db()?;

    for result in [
        record_position(&db, OWNER, "m-inexistente", 10).map(|_| ()),
        set_completed(&db, OWNER, "m-inexistente", true).map(|_| ()),
    ] {
        match result {
            Err(Error::MediaNotFound { media_id }) => assert_eq!(media_id, "m-inexistente"),
            other => panic!("esperava Error::MediaNotFound, obtive {other:?}"),
        }
    }

    assert_eq!(count(&db, "study_state"), 0);
    Ok(())
}

#[test]
fn progress_is_read_and_written_while_the_media_is_missing() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "missing")?;

    let recorded = record_position(&db, OWNER, "m1", 33)?;
    assert_eq!(recorded.position_seconds, 33);

    let completed = set_completed(&db, OWNER, "m1", true)?;
    assert!(completed.completed);
    assert_eq!(completed.position_seconds, 33);

    let read = study_progress(&db, OWNER, "m1")?.expect("a linha persiste");
    assert_eq!(read.position_seconds, 33);
    assert!(read.completed);
    Ok(())
}

#[test]
fn a_negative_position_is_rejected_without_creating_a_row() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;

    match record_position(&db, OWNER, "m1", -1) {
        Err(Error::InvalidStudyPosition { position_seconds }) => assert_eq!(position_seconds, -1),
        other => panic!("esperava Error::InvalidStudyPosition, obtive {other:?}"),
    }

    assert_eq!(count(&db, "study_state"), 0);
    Ok(())
}

#[test]
fn an_empty_owner_is_rejected_everywhere() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;

    for owner in ["", "   ", "\t"] {
        for result in [
            record_position(&db, owner, "m1", 10).map(|_| ()),
            set_completed(&db, owner, "m1", true).map(|_| ()),
            study_progress(&db, owner, "m1").map(|_| ()),
            recent_study(&db, owner, 10).map(|_| ()),
        ] {
            match result {
                Err(Error::InvalidOwnerId) => {}
                other => panic!("esperava Error::InvalidOwnerId para {owner:?}, obtive {other:?}"),
            }
        }
    }

    assert_eq!(count(&db, "study_state"), 0);
    Ok(())
}

#[test]
fn a_failed_write_leaves_the_previous_state_intact() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("learnvault.sqlite");
    let db = Database::open(&path)?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    insert_media(&db, "m1", "available")?;
    seed(&db, OWNER, "m1", 10, false, SEEDED_AT)?;

    // Injeção determinística de falha, criada por outra conexão ao mesmo arquivo.
    {
        let injected = rusqlite::Connection::open(&path)?;
        injected.execute_batch(
            "CREATE TRIGGER falha_no_progresso BEFORE UPDATE ON study_state
             BEGIN SELECT RAISE(ABORT, 'falha injetada'); END;",
        )?;
    }

    let result = record_position(&db, OWNER, "m1", 99);

    match result {
        Err(Error::Database(_)) => {}
        other => panic!("esperava a falha injetada no banco, obtive {other:?}"),
    }
    assert_eq!(
        row(&db, OWNER, "m1"),
        Some((10, 0, SEEDED_AT)),
        "uma falha não pode deixar estado parcial"
    );
    Ok(())
}

#[test]
fn recording_progress_does_not_touch_other_tables() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    let media_before = media_snapshot(&db);
    let scan_before = count(&db, "scan");
    let library_before = count(&db, "library");

    record_position(&db, OWNER, "m1", 10)?;
    set_completed(&db, OWNER, "m1", true)?;

    assert_eq!(
        media_snapshot(&db),
        media_before,
        "o progresso não altera nenhuma coluna de mídia"
    );
    assert_eq!(count(&db, "scan"), scan_before);
    assert_eq!(count(&db, "library"), library_before);
    assert_eq!(count(&db, "study_state"), 1);
    Ok(())
}

#[test]
fn progress_of_an_unknown_media_is_reported_on_read() -> TestResult {
    let db = library_db()?;

    match study_progress(&db, OWNER, "m-inexistente") {
        Err(Error::MediaNotFound { media_id }) => assert_eq!(media_id, "m-inexistente"),
        other => panic!("esperava Error::MediaNotFound, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn a_real_change_promotes_the_media_in_recent_study() -> TestResult {
    let db = library_db()?;
    for media in ["m1", "m2"] {
        insert_media(&db, media, "available")?;
    }
    seed(&db, OWNER, "m1", 10, false, 100)?;
    seed(&db, OWNER, "m2", 20, false, 200)?;
    assert_eq!(recent_ids(&db, OWNER, 10), ["m2", "m1"]);

    record_position(&db, OWNER, "m1", 99)?;

    assert_eq!(
        recent_ids(&db, OWNER, 10),
        ["m1", "m2"],
        "uma mudança real torna a mídia a mais recente"
    );
    Ok(())
}

#[test]
fn a_write_from_another_owner_does_not_touch_the_row() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    insert_media(&db, "m2", "available")?;
    record_position(&db, OWNER, "m1", 42)?;
    set_completed(&db, OWNER, "m1", true)?;
    let before = row(&db, OWNER, "m1");

    record_position(&db, OTHER, "m1", 7)?;
    set_completed(&db, OTHER, "m1", true)?;
    record_position(&db, OTHER, "m2", 5)?;

    assert_eq!(
        row(&db, OWNER, "m1"),
        before,
        "nenhuma escrita de outro dono pode tocar a linha do primeiro"
    );
    assert_eq!(
        recent_ids(&db, OWNER, 10),
        ["m1"],
        "os recentes de um dono não incluem mídias do outro"
    );

    let mut other_ids = recent_ids(&db, OTHER, 10);
    other_ids.sort();
    assert_eq!(other_ids, ["m1", "m2"]);
    Ok(())
}

#[test]
fn the_first_write_of_a_neutral_value_still_creates_the_state() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    insert_media(&db, "m2", "available")?;

    let cleared = set_completed(&db, OWNER, "m1", false)?;
    assert!(!cleared.completed);
    assert_eq!(cleared.position_seconds, 0);
    assert!(
        cleared.updated_at > SEEDED_AT,
        "a primeira chamada é atividade do usuário"
    );

    let at_zero = record_position(&db, OWNER, "m2", 0)?;
    assert_eq!(at_zero.position_seconds, 0);
    assert_eq!(count(&db, "study_state"), 2);
    Ok(())
}

#[test]
fn set_completed_is_idempotent_when_already_false() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;
    seed(&db, OWNER, "m1", 5, false, SEEDED_AT)?;

    let state = set_completed(&db, OWNER, "m1", false)?;

    assert_eq!(state.updated_at, SEEDED_AT);
    assert_eq!(row(&db, OWNER, "m1"), Some((5, 0, SEEDED_AT)));
    Ok(())
}

#[test]
fn the_position_boundaries_are_accepted() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;

    assert_eq!(record_position(&db, OWNER, "m1", 0)?.position_seconds, 0);
    assert_eq!(
        record_position(&db, OWNER, "m1", i64::MAX)?.position_seconds,
        i64::MAX
    );
    Ok(())
}

#[test]
fn validation_happens_before_any_write() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;

    match record_position(&db, "", "m1", -1) {
        Err(Error::InvalidOwnerId) => {}
        other => panic!("o dono é validado primeiro: {other:?}"),
    }
    match record_position(&db, OWNER, "m-inexistente", -1) {
        Err(Error::InvalidStudyPosition { .. }) => {}
        other => panic!("a posição é validada antes da mídia: {other:?}"),
    }

    assert_eq!(count(&db, "study_state"), 0);
    Ok(())
}

#[test]
fn recent_study_of_an_owner_without_progress_is_empty() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "available")?;

    assert!(recent_study(&db, OWNER, 10)?.is_empty());
    assert!(recent_study(&db, OWNER, 0)?.is_empty());
    Ok(())
}

#[test]
fn set_completed_is_atomic_under_failure() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("learnvault.sqlite");
    let db = Database::open(&path)?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    insert_media(&db, "m1", "available")?;
    seed(&db, OWNER, "m1", 10, false, SEEDED_AT)?;

    {
        let injected = rusqlite::Connection::open(&path)?;
        injected.execute_batch(
            "CREATE TRIGGER falha_na_conclusao BEFORE UPDATE ON study_state
             BEGIN SELECT RAISE(ABORT, 'falha injetada'); END;",
        )?;
    }

    match set_completed(&db, OWNER, "m1", true) {
        Err(Error::Database(_)) => {}
        other => panic!("esperava a falha injetada no banco, obtive {other:?}"),
    }
    assert_eq!(row(&db, OWNER, "m1"), Some((10, 0, SEEDED_AT)));
    Ok(())
}
