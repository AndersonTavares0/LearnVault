//! Testes do schema criado pela migração inicial: restrições, chaves e unicidade.

use learnvault_core::Database;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn database() -> Database {
    Database::open_in_memory().expect("banco em memória deve abrir com o schema aplicado")
}

/// Afirma que a operação falhou por violação de restrição do SQLite.
///
/// Verificar o tipo do erro em vez de apenas `is_err()` garante que o teste falhe pelo
/// motivo certo: uma restrição declarada no schema, e não um erro de sintaxe, de tipo ou
/// de coluna inexistente.
fn assert_constraint_violation(result: rusqlite::Result<usize>, context: &str) {
    match result {
        Err(rusqlite::Error::SqliteFailure(error, message)) => assert_eq!(
            error.code,
            rusqlite::ErrorCode::ConstraintViolation,
            "{context}: esperava violação de restrição, obtive {message:?}"
        ),
        other => panic!("{context}: esperava violação de restrição, obtive {other:?}"),
    }
}

fn count(db: &Database, table: &str) -> i64 {
    db.connection()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("a contagem deve ser consultável")
}

fn insert_library(db: &Database, library_id: &str, available: i64) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, ?2)",
        rusqlite::params![library_id, available],
    )
}

fn insert_media(
    db: &Database,
    media_id: &str,
    library_id: &str,
    key: &str,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'available')",
        rusqlite::params![media_id, library_id, key, key, 1024_i64, 1_760_000_000_i64],
    )
}

fn insert_user(
    db: &Database,
    user_id: &str,
    username: &str,
    is_admin: i64,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO user (user_id, username, password_hash, is_admin, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            user_id,
            username,
            "$argon2id$stub",
            is_admin,
            1_760_000_000_i64
        ],
    )
}

#[test]
fn all_domain_tables_are_created() -> TestResult {
    let db = database();
    for table in [
        "library",
        "media",
        "media_removal",
        "scan",
        "scan_inaccessible",
        "study_state",
        "user",
        "session",
    ] {
        let found: i64 = db.connection().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        assert_eq!(found, 1, "a tabela {table} deve existir");
    }
    Ok(())
}

#[test]
fn library_available_accepts_only_boolean_values() -> TestResult {
    let db = database();
    assert!(insert_library(&db, "bib-1", 0).is_ok());
    assert!(insert_library(&db, "bib-2", 1).is_ok());
    assert_constraint_violation(insert_library(&db, "bib-3", 2), "available = 2");
    assert_constraint_violation(insert_library(&db, "bib-4", -1), "available = -1");
    Ok(())
}

#[test]
fn media_availability_accepts_both_known_states() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    assert!(insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4").is_ok());

    let missing_state = db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES ('mid-2', 'bib-1', 'curso/x.mp4', 'curso/x.mp4', 10, 10, 'missing')",
        [],
    );
    assert!(
        missing_state.is_ok(),
        "o estado 'missing' é válido: o arquivo ausente mantém o registro"
    );
    Ok(())
}

#[test]
fn media_availability_rejects_unknown_states() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;

    let unknown = db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES ('mid-2', 'bib-1', 'curso/x.mp4', 'curso/x.mp4', 10, 10, 'ausente')",
        [],
    );
    assert_constraint_violation(unknown, "estado desconhecido em availability");
    Ok(())
}

#[test]
fn scan_status_accepts_only_known_states() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    let insert = |status: &str| {
        db.connection().execute(
            "INSERT INTO scan (library_id, status, completed_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["bib-1", status, 1_760_000_000_i64],
        )
    };
    assert!(insert("completed").is_ok());
    assert!(insert("failed").is_ok());
    assert_constraint_violation(insert("interrupted"), "status desconhecido em scan");
    Ok(())
}

#[test]
fn scan_complete_accepts_only_boolean_values() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;

    let insert = |complete: i64| {
        db.connection().execute(
            "INSERT INTO scan (library_id, status, completed_at, complete)
             VALUES (?1, 'completed', ?2, ?3)",
            rusqlite::params!["bib-1", 1_760_000_000_i64, complete],
        )
    };
    assert!(
        insert(0).is_ok(),
        "uma varredura parcial é registrada como 0"
    );
    assert!(insert(1).is_ok());
    assert_constraint_violation(insert(2), "complete = 2");
    assert_constraint_violation(insert(-1), "complete = -1");
    Ok(())
}

#[test]
fn scan_inaccessible_requires_an_existing_scan() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;

    let orphan = db.connection().execute(
        "INSERT INTO scan_inaccessible (scan_id, relative_path) VALUES (999, 'Curso/aula.mp4')",
        [],
    );
    assert_constraint_violation(orphan, "varredura inexistente em scan_inaccessible");

    db.connection().execute(
        "INSERT INTO scan (library_id, status, completed_at) VALUES (?1, 'completed', ?2)",
        rusqlite::params!["bib-1", 1_760_000_000_i64],
    )?;
    let scan_id = db.connection().last_insert_rowid();

    let linked = |path: &str| {
        db.connection().execute(
            "INSERT INTO scan_inaccessible (scan_id, relative_path) VALUES (?1, ?2)",
            rusqlite::params![scan_id, path],
        )
    };
    assert!(linked("Curso/aula.mp4").is_ok());
    assert!(linked("temp_download").is_ok());
    assert_constraint_violation(
        linked("Curso/aula.mp4"),
        "o mesmo caminho não se repete na mesma varredura",
    );
    Ok(())
}

#[test]
fn study_state_completed_accepts_only_boolean_values() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;

    let insert = |owner: &str, completed: i64| {
        db.connection().execute(
            "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
             VALUES (?1, 'mid-1', 0, ?2, ?3)",
            rusqlite::params![owner, completed, 1_760_000_000_i64],
        )
    };
    assert!(insert("owner-a", 0).is_ok());
    assert!(insert("owner-b", 1).is_ok());
    assert_constraint_violation(insert("owner-c", 2), "completed = 2");
    Ok(())
}

#[test]
fn user_is_admin_accepts_only_boolean_values() -> TestResult {
    let db = database();
    assert!(insert_user(&db, "usr-1", "ana", 0).is_ok());
    assert!(insert_user(&db, "usr-2", "bruno", 1).is_ok());
    assert_constraint_violation(insert_user(&db, "usr-3", "carla", 2), "is_admin = 2");
    Ok(())
}

#[test]
fn media_requires_an_existing_library() -> TestResult {
    let db = database();
    assert_constraint_violation(
        insert_media(&db, "mid-1", "bib-inexistente", "curso/aula.mp4"),
        "mídia apontando para biblioteca inexistente",
    );
    Ok(())
}

#[test]
fn scan_requires_an_existing_library() -> TestResult {
    let db = database();
    let orphan = db.connection().execute(
        "INSERT INTO scan (library_id, status, completed_at)
         VALUES ('bib-inexistente', 'completed', 1760000000)",
        [],
    );
    assert_constraint_violation(orphan, "varredura apontando para biblioteca inexistente");
    Ok(())
}

#[test]
fn study_state_requires_an_existing_media() -> TestResult {
    let db = database();
    let orphan = db.connection().execute(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES ('owner-a', 'mid-inexistente', 0, 0, 1)",
        [],
    );
    assert_constraint_violation(orphan, "estado de estudo apontando para mídia inexistente");
    Ok(())
}

#[test]
fn session_requires_an_existing_user() -> TestResult {
    let db = database();
    insert_user(&db, "usr-1", "ana", 0)?;
    let insert = |hash: &str, user_id: &str| {
        db.connection().execute(
            "INSERT INTO session (token_hash, user_id, expires_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![hash, user_id, 1_760_000_000_i64],
        )
    };
    assert!(insert("hash-1", "usr-1").is_ok());
    assert_constraint_violation(
        insert("hash-2", "usr-inexistente"),
        "sessão apontando para usuário inexistente",
    );
    Ok(())
}

#[test]
fn media_path_key_is_unique_per_library() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_library(&db, "bib-2", 1)?;

    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;

    assert_constraint_violation(
        insert_media(&db, "mid-2", "bib-1", "curso/aula.mp4"),
        "a mesma chave na mesma biblioteca deve ser rejeitada",
    );

    assert!(
        insert_media(&db, "mid-3", "bib-2", "curso/aula.mp4").is_ok(),
        "a chave é única por biblioteca, não global"
    );
    Ok(())
}

#[test]
fn media_ids_are_unique() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;
    assert_constraint_violation(
        insert_media(&db, "mid-1", "bib-1", "curso/outra.mp4"),
        "media_id duplicado",
    );
    Ok(())
}

#[test]
fn study_state_identity_is_the_pair_owner_and_media() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;

    let insert = |owner: &str| {
        db.connection().execute(
            "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
             VALUES (?1, 'mid-1', 0, 0, ?2)",
            rusqlite::params![owner, 1_760_000_000_i64],
        )
    };

    assert!(insert("owner-a").is_ok());
    assert!(
        insert("owner-b").is_ok(),
        "dois donos podem acompanhar a mesma mídia"
    );
    assert_constraint_violation(insert("owner-a"), "o par dono/mídia deve ser único");
    Ok(())
}

#[test]
fn study_state_position_defaults_to_zero() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;

    db.connection().execute(
        "INSERT INTO study_state (owner_id, media_id, completed, updated_at)
         VALUES ('owner-a', 'mid-1', 0, 1760000000)",
        [],
    )?;

    let position: i64 = db.connection().query_row(
        "SELECT position_seconds FROM study_state WHERE owner_id = 'owner-a'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(position, 0, "a posição inicial deve ser o começo da aula");
    Ok(())
}

#[test]
fn not_null_columns_are_enforced() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;

    let missing_path = db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES ('mid-1', 'bib-1', NULL, 'curso/aula.mp4', 10, 10, 'available')",
        [],
    );
    assert_constraint_violation(missing_path, "relative_path nulo");

    let missing_availability = db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES ('mid-2', 'bib-1', 'curso/aula.mp4', 'curso/aula.mp4', 10, 10, NULL)",
        [],
    );
    assert_constraint_violation(missing_availability, "availability nula");
    Ok(())
}

#[test]
fn deleting_a_library_with_media_is_refused_and_preserves_rows() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;
    db.connection().execute(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES ('owner-a', 'mid-1', 30, 1, 1760000000)",
        [],
    )?;

    assert_constraint_violation(
        db.connection()
            .execute("DELETE FROM library WHERE library_id = 'bib-1'", []),
        "remover biblioteca com mídia associada",
    );
    assert_constraint_violation(
        db.connection()
            .execute("DELETE FROM media WHERE media_id = 'mid-1'", []),
        "remover mídia com estado de estudo associado",
    );

    assert_eq!(count(&db, "library"), 1, "a biblioteca deve permanecer");
    assert_eq!(count(&db, "media"), 1, "a mídia deve permanecer");
    assert_eq!(
        count(&db, "study_state"),
        1,
        "o estado de estudo deve permanecer"
    );
    Ok(())
}

#[test]
fn usernames_are_unique() -> TestResult {
    let db = database();
    insert_user(&db, "usr-1", "ana", 0)?;
    assert_constraint_violation(insert_user(&db, "usr-2", "ana", 0), "username duplicado");
    Ok(())
}

#[test]
fn usernames_are_unique_ignoring_ascii_case() -> TestResult {
    let db = database();
    insert_user(&db, "usr-1", "Ana", 0)?;
    assert_constraint_violation(
        insert_user(&db, "usr-2", "ana", 0),
        "usuário duplicado apenas por diferença de caixa",
    );
    Ok(())
}

fn insert_removal(
    db: &Database,
    library_id: &str,
    media_id: &str,
    removed_at: i64,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![library_id, media_id, removed_at],
    )
}

#[test]
fn media_removal_requires_an_existing_library() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;
    assert_constraint_violation(
        insert_removal(&db, "bib-ausente", "mid-1", 1_760_000_000),
        "remoção em biblioteca inexistente",
    );
    Ok(())
}

#[test]
fn media_removal_requires_an_existing_media() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    assert_constraint_violation(
        insert_removal(&db, "bib-1", "mid-ausente", 1_760_000_000),
        "remoção de mídia inexistente",
    );
    Ok(())
}

#[test]
fn media_removal_identity_is_the_pair_library_and_media() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;
    insert_removal(&db, "bib-1", "mid-1", 1_760_000_000)?;
    assert_constraint_violation(
        insert_removal(&db, "bib-1", "mid-1", 1_760_000_001),
        "remoção repetida do mesmo par",
    );
    assert_eq!(count(&db, "media_removal"), 1);
    Ok(())
}
