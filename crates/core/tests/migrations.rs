//! Testes do schema criado pela migração inicial: restrições, chaves e unicidade.

use learnvault_core::Database;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn database() -> Database {
    Database::open_in_memory().expect("banco em memória deve abrir com o schema aplicado")
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
    for table in ["library", "media", "scan", "study_state", "user", "session"] {
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
    assert!(insert_library(&db, "bib-3", 2).is_err());
    assert!(insert_library(&db, "bib-4", -1).is_err());
    Ok(())
}

#[test]
fn media_availability_accepts_only_known_states() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    assert!(insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4").is_ok());

    let missing = db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES ('mid-2', 'bib-1', 'curso/x.mp4', 'curso/x.mp4', 10, 10, 'ausente')",
        [],
    );
    assert!(
        missing.is_err(),
        "estados fora de available/missing devem ser rejeitados"
    );
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
    assert!(insert("interrupted").is_err());
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
    assert!(insert("owner-c", 2).is_err());
    Ok(())
}

#[test]
fn user_is_admin_accepts_only_boolean_values() -> TestResult {
    let db = database();
    assert!(insert_user(&db, "usr-1", "ana", 0).is_ok());
    assert!(insert_user(&db, "usr-2", "bruno", 1).is_ok());
    assert!(insert_user(&db, "usr-3", "carla", 2).is_err());
    Ok(())
}

#[test]
fn media_requires_an_existing_library() -> TestResult {
    let db = database();
    assert!(insert_media(&db, "mid-1", "bib-inexistente", "curso/aula.mp4").is_err());
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
    assert!(orphan.is_err());
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
    assert!(insert("hash-2", "usr-inexistente").is_err());
    Ok(())
}

#[test]
fn media_path_key_is_unique_per_library() -> TestResult {
    let db = database();
    insert_library(&db, "bib-1", 1)?;
    insert_library(&db, "bib-2", 1)?;

    insert_media(&db, "mid-1", "bib-1", "curso/aula.mp4")?;

    let duplicate_same_library = insert_media(&db, "mid-2", "bib-1", "curso/aula.mp4");
    assert!(
        duplicate_same_library.is_err(),
        "a mesma chave na mesma biblioteca deve ser rejeitada"
    );

    let same_key_other_library = insert_media(&db, "mid-3", "bib-2", "curso/aula.mp4");
    assert!(
        same_key_other_library.is_ok(),
        "a chave é única por biblioteca"
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
    assert!(
        insert("owner-a").is_err(),
        "o par dono/mídia deve ser único"
    );
    Ok(())
}

#[test]
fn usernames_are_unique() -> TestResult {
    let db = database();
    insert_user(&db, "usr-1", "ana", 0)?;
    assert!(insert_user(&db, "usr-2", "ana", 0).is_err());
    Ok(())
}
