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

#[test]
fn in_memory_database_starts_at_the_backed_schema_version() -> TestResult {
    let db = Database::open_in_memory()?;
    assert_eq!(db.schema_version()?, 1);
    Ok(())
}

#[test]
fn in_memory_database_creates_all_domain_tables() -> TestResult {
    let db = Database::open_in_memory()?;
    assert_eq!(table_count(&db), 6);
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
    assert_eq!(db.schema_version()?, 1);
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

    assert_eq!(db.schema_version()?, 1);
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
        conn.pragma_update(None, "user_version", 2)?;
    }

    match Database::open(&path) {
        Err(Error::SchemaTooNew { found, known }) => {
            assert_eq!(found, 2);
            assert_eq!(known, 1);
        }
        other => panic!("esperava Error::SchemaTooNew, obtive {other:?}"),
    }
    Ok(())
}

#[test]
fn rejecting_a_newer_schema_leaves_the_database_untouched() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.pragma_update(None, "user_version", 2)?;
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
    assert_eq!(version, 2, "a versão do banco não deve ser alterada");

    let journal: String =
        conn.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?;
    assert_ne!(
        journal.to_lowercase(),
        "wal",
        "recusar o schema não deve trocar o journal do arquivo"
    );
    Ok(())
}
