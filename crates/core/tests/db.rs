//! Testes da abertura do banco e da aplicação da cadeia única de migrações.

use learnvault_core::{Database, Error};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn journal_mode(db: &Database) -> String {
    db.connection()
        .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
        .expect("PRAGMA journal_mode deve ser consultável")
        .to_lowercase()
}

fn foreign_keys_enabled(db: &Database) -> i64 {
    db.connection()
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("PRAGMA foreign_keys deve ser consultável")
}

fn table_count(db: &Database) -> i64 {
    db.connection()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .expect("sqlite_master deve ser consultável")
}

fn table_exists(db: &Database, name: &str) -> bool {
    db.connection()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .expect("sqlite_master deve ser consultável")
        == 1
}

#[test]
fn in_memory_database_starts_at_the_current_schema_version() -> TestResult {
    let db = Database::open_in_memory()?;
    assert_eq!(db.schema_version()?, 4);
    Ok(())
}

#[test]
fn in_memory_database_creates_all_domain_tables() -> TestResult {
    let db = Database::open_in_memory()?;
    assert_eq!(table_count(&db), 8);
    Ok(())
}

#[test]
fn foreign_keys_are_enabled_on_the_in_memory_connection() -> TestResult {
    let db = Database::open_in_memory()?;
    assert_eq!(foreign_keys_enabled(&db), 1);
    Ok(())
}

#[test]
fn in_memory_database_does_not_use_the_wal_journal() -> TestResult {
    let db = Database::open_in_memory()?;
    assert_eq!(journal_mode(&db), "memory");
    Ok(())
}

#[test]
fn persistent_database_is_created_and_uses_the_wal_journal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    let db = Database::open(&path)?;

    assert!(path.exists(), "o arquivo do banco deve existir");
    assert_eq!(journal_mode(&db), "wal");
    assert_eq!(foreign_keys_enabled(&db), 1);
    assert_eq!(db.schema_version()?, 4);
    Ok(())
}

#[test]
fn reopening_applies_migrations_once_and_preserves_data() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    {
        let db = Database::open(&path)?;
        db.connection().execute(
            "INSERT INTO library (library_id, available) VALUES (?1, ?2)",
            rusqlite::params!["bib-1", 1],
        )?;
    }

    let db = Database::open(&path)?;

    assert_eq!(db.schema_version()?, 4);
    let libraries: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM library", [], |row| row.get(0))?;
    assert_eq!(libraries, 1);
    Ok(())
}

#[test]
fn opening_an_impossible_path_returns_a_typed_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir
        .path()
        .join("diretorio-ausente")
        .join("learnvault.sqlite");

    match Database::open(&path) {
        Err(Error::OpenDatabase { path: reported, .. }) => assert_eq!(reported, path),
        other => panic!("esperava Error::OpenDatabase, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn database_with_a_newer_schema_is_rejected_without_panicking() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.pragma_update(None, "user_version", 5)?;
    }

    match Database::open(&path) {
        Err(Error::SchemaTooNew { found, known }) => {
            assert_eq!(found, 5);
            assert_eq!(known, 4);
        }
        other => panic!("esperava Error::SchemaTooNew, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn legacy_go_database_is_rejected_before_bootstrap_or_migration() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("legacy.sqlite");
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY);
             CREATE TABLE sessions (token_hash TEXT PRIMARY KEY);
             CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY);",
        )?;
    }

    assert!(matches!(
        Database::open(&path),
        Err(Error::LegacyGoDatabase)
    ));
    let conn = rusqlite::Connection::open(&path)?;
    let rust_tables: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('user', 'session')",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(rust_tables, 0, "a recusa não cria o schema Rust");
    Ok(())
}

#[test]
fn new_empty_database_is_not_mistaken_for_legacy_go() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = Database::open(dir.path().join("new.sqlite"))?;
    assert_eq!(db.schema_version()?, 4);
    assert!(table_exists(&db, "user"));
    Ok(())
}

#[test]
fn rejecting_a_newer_schema_leaves_the_database_untouched() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.pragma_update(None, "user_version", 5)?;
    }

    match Database::open(&path) {
        Err(Error::SchemaTooNew { .. }) => {}
        other => panic!("esperava Error::SchemaTooNew, obtive {other:?}"),
    }

    // A recusa não pode ter efeito colateral: nem tabela criada, nem versão alterada,
    // nem journal trocado em um banco que este binário não conhece.
    let conn = rusqlite::Connection::open(&path)?;
    let tables: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(tables, 0, "nenhuma tabela deve ter sido criada");

    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(version, 5, "a versão do banco não deve ser alterada");

    let journal: String =
        conn.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?;
    assert_ne!(
        journal.to_lowercase(),
        "wal",
        "recusar o schema não deve trocar o journal do arquivo"
    );
    Ok(())
}

#[test]
fn a_version_one_database_is_upgraded_additively_preserving_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    // Banco gravado pela versão 1 do schema: a tabela `scan` ainda não tem a coluna
    // `complete` e a tabela `scan_inaccessible` não existe.
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE scan (
                 scan_id INTEGER PRIMARY KEY,
                 library_id TEXT NOT NULL,
                 status TEXT NOT NULL CHECK (status IN ('completed', 'failed')),
                 completed_at INTEGER NOT NULL
             );
             CREATE TABLE user (
                 user_id TEXT PRIMARY KEY NOT NULL,
                 username TEXT NOT NULL UNIQUE COLLATE NOCASE,
                 password_hash TEXT NOT NULL,
                 is_admin INTEGER NOT NULL CHECK (is_admin IN (0, 1)),
                 created_at INTEGER NOT NULL
             );
             INSERT INTO scan (library_id, status, completed_at)
             VALUES ('bib-1', 'completed', 1760000000);
             INSERT INTO user (user_id, username, password_hash, is_admin, created_at)
             VALUES ('usr-1', 'admin', 'hash', 1, 1760000000);
             PRAGMA user_version = 1;",
        )?;
    }

    let db = Database::open(&path)?;

    assert_eq!(
        db.schema_version()?,
        4,
        "a cadeia deve aplicar a migração aditiva"
    );
    assert!(table_exists(&db, "scan_inaccessible"));

    let (status, complete): (String, i64) =
        db.connection()
            .query_row("SELECT status, complete FROM scan", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
    assert_eq!(status, "completed", "a linha existente deve sobreviver");
    assert_eq!(
        complete, 1,
        "a coluna nova precisa de um valor compatível para as linhas antigas"
    );
    let must_change_credentials: i64 = db.connection().query_row(
        "SELECT must_change_credentials FROM user WHERE user_id = 'usr-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        must_change_credentials, 0,
        "usuários existentes não devem exigir troca de credenciais"
    );
    Ok(())
}

#[test]
fn a_version_two_database_is_upgraded_additively_preserving_rows() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    // Banco gravado pela versão 2 do schema: `scan` já tem `complete` e
    // `scan_inaccessible` existe, mas `media_removal` ainda não.
    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE scan (
                 scan_id INTEGER PRIMARY KEY,
                 library_id TEXT NOT NULL,
                 status TEXT NOT NULL CHECK (status IN ('completed', 'failed')),
                 completed_at INTEGER NOT NULL,
                 complete INTEGER NOT NULL DEFAULT 1 CHECK (complete IN (0, 1))
             );
             CREATE TABLE user (
                 user_id TEXT PRIMARY KEY NOT NULL,
                 username TEXT NOT NULL UNIQUE COLLATE NOCASE,
                 password_hash TEXT NOT NULL,
                 is_admin INTEGER NOT NULL CHECK (is_admin IN (0, 1)),
                 created_at INTEGER NOT NULL
             );
             INSERT INTO scan (library_id, status, completed_at, complete)
             VALUES ('bib-1', 'completed', 1760000000, 0);
             INSERT INTO user (user_id, username, password_hash, is_admin, created_at)
             VALUES ('usr-1', 'admin', 'hash', 1, 1760000000);
             PRAGMA user_version = 2;",
        )?;
    }

    let db = Database::open(&path)?;

    assert_eq!(
        db.schema_version()?,
        4,
        "a cadeia deve aplicar a migração aditiva"
    );
    assert!(
        table_exists(&db, "media_removal"),
        "a tabela nova deve existir"
    );

    let complete: i64 = db
        .connection()
        .query_row("SELECT complete FROM scan", [], |row| row.get(0))?;
    assert_eq!(complete, 0, "a linha existente sobrevive com seu valor");
    let must_change_credentials: i64 = db.connection().query_row(
        "SELECT must_change_credentials FROM user WHERE user_id = 'usr-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        must_change_credentials, 0,
        "usuários existentes não devem exigir troca de credenciais"
    );
    Ok(())
}
