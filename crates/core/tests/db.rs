//! Tests of opening the database and applying the single migration chain.

use learnvault_core::{Database, Error};
use rusqlite::config::DbConfig;

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
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
               AND name NOT LIKE '%\\_fts\\_%' ESCAPE '\\'",
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
    assert_eq!(db.schema_version()?, 23);
    Ok(())
}

#[test]
fn in_memory_database_creates_all_domain_tables() -> TestResult {
    let db = Database::open_in_memory()?;
    assert_eq!(
        table_count(&db),
        35,
        "29 domain tables plus the 6 FTS5 virtual tables"
    );
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
    assert_eq!(db.schema_version()?, 23);
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

    assert_eq!(db.schema_version()?, 23);
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
        conn.pragma_update(None, "user_version", 24)?;
    }

    match Database::open(&path) {
        Err(Error::SchemaTooNew { found, known }) => {
            assert_eq!(found, 24);
            assert_eq!(known, 23);
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
    assert_eq!(db.schema_version()?, 23);
    assert!(table_exists(&db, "user"));
    Ok(())
}

#[test]
fn rejecting_a_newer_schema_leaves_the_database_untouched() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    {
        let conn = rusqlite::Connection::open(&path)?;
        conn.pragma_update(None, "user_version", 24)?;
    }

    match Database::open(&path) {
        Err(Error::SchemaTooNew { .. }) => {}
        other => panic!("esperava Error::SchemaTooNew, obtive {other:?}"),
    }

    // The refusal cannot have a side effect: no table created, no version changed,
    // no journal switched in a database this binary does not know.
    let conn = rusqlite::Connection::open(&path)?;
    let tables: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(tables, 0, "nenhuma tabela deve ter sido criada");

    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(version, 24, "a versão do banco não deve ser alterada");

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

    // Database written by schema version 1: the `scan` table does not yet have the column
    // `complete` and the `scan_inaccessible` table does not exist.
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
             CREATE TABLE study_state (
                 owner_id TEXT NOT NULL,
                 media_id TEXT NOT NULL,
                 position_seconds INTEGER NOT NULL DEFAULT 0,
                 completed INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY (owner_id, media_id)
             );
             CREATE TABLE library (
                 library_id TEXT PRIMARY KEY NOT NULL,
                 available INTEGER NOT NULL
             );
             CREATE TABLE media (
                 media_id TEXT PRIMARY KEY NOT NULL,
                 library_id TEXT NOT NULL,
                 relative_path TEXT NOT NULL,
                 relative_path_key TEXT NOT NULL,
                 size INTEGER NOT NULL,
                 mtime INTEGER NOT NULL,
                 availability TEXT NOT NULL
             );
             PRAGMA user_version = 1;",
        )?;
    }

    let db = Database::open(&path)?;

    assert_eq!(
        db.schema_version()?,
        23,
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

    // Database written by schema version 2: `scan` already has `complete` and
    // `scan_inaccessible` exists, but `media_removal` does not yet.
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
             CREATE TABLE study_state (
                 owner_id TEXT NOT NULL,
                 media_id TEXT NOT NULL,
                 position_seconds INTEGER NOT NULL DEFAULT 0,
                 completed INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY (owner_id, media_id)
             );
             CREATE TABLE library (
                 library_id TEXT PRIMARY KEY NOT NULL,
                 available INTEGER NOT NULL
             );
             CREATE TABLE media (
                 media_id TEXT PRIMARY KEY NOT NULL,
                 library_id TEXT NOT NULL,
                 relative_path TEXT NOT NULL,
                 relative_path_key TEXT NOT NULL,
                 size INTEGER NOT NULL,
                 mtime INTEGER NOT NULL,
                 availability TEXT NOT NULL
             );
             PRAGMA user_version = 2;",
        )?;
    }

    let db = Database::open(&path)?;

    assert_eq!(
        db.schema_version()?,
        23,
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

#[test]
fn a_version_sixteen_database_is_upgraded_without_losing_notes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    // Simulates a database written by version 16: the sync journal does not exist yet
    // and `study_note` has no `sync_version`.
    {
        let db = Database::open(&path)?;
        db.connection().execute_batch(
            "INSERT INTO library (library_id, available) VALUES ('bib-1', 1);
             INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('mid-1', 'bib-1', 'Curso/Aula/video.mp4', 'curso/aula/video.mp4', 1024, 1760000000, 'available');
             INSERT INTO study_note (note_id, owner_id, media_id, position_seconds, content, created_at, updated_at)
             VALUES ('note-1', 'owner-a', 'mid-1', 10, 'anotação antiga', 1760000000, 1760000000);",
        )?;
        db.connection().execute_batch(
            "DROP TABLE sync_operation; DROP TABLE sync_outbox; DROP TABLE sync_state;
             DROP TABLE sync_tombstone; DROP TABLE library_root; DROP TABLE sync_device;
             DROP TABLE library_root_dismissed;",
        )?;
        db.connection().execute_batch(
            "ALTER TABLE study_note DROP COLUMN sync_version;
             ALTER TABLE flashcard DROP COLUMN text_updated_at;",
        )?;
        db.connection().pragma_update(None, "user_version", 16)?;
    }

    let db = Database::open(&path)?;

    assert_eq!(db.schema_version()?, 23, "a cadeia aplica de V17 a V23");
    assert!(table_exists(&db, "sync_operation"));
    assert!(table_exists(&db, "sync_outbox"));
    assert!(table_exists(&db, "sync_state"));
    let (content, version): (String, Option<String>) = db.connection().query_row(
        "SELECT content, sync_version FROM study_note WHERE note_id = 'note-1'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(content, "anotação antiga", "a anotação existente sobrevive");
    assert_eq!(version, None, "linhas anteriores à V17 não têm versão");
    Ok(())
}

#[test]
fn the_current_schema_is_recognized_on_a_fresh_in_memory_database() -> TestResult {
    let db = Database::open_in_memory()?;
    assert!(
        db.has_current_schema()?,
        "um banco recém-migrado deve ser reconhecido como pronto para o servidor"
    );
    Ok(())
}

#[test]
fn the_current_schema_is_recognized_on_a_fresh_file_database() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    let db = Database::open(&path)?;

    assert!(
        db.has_current_schema()?,
        "o banco criado pelo próprio binário não pode ser recusado no primeiro start"
    );
    Ok(())
}

#[test]
fn a_drifted_schema_is_not_reported_as_current() -> TestResult {
    let db = Database::open_in_memory()?;
    assert!(db.has_current_schema()?);

    // Any observable deviation (here, a required index that went missing) must
    // fail readiness instead of trusting only the recorded version.
    db.connection()
        .execute("DROP INDEX session_expires_at_idx", [])?;

    assert!(
        !db.has_current_schema()?,
        "o banco sem o índice exigido não está pronto para o servidor"
    );
    Ok(())
}

#[test]
fn backup_to_creates_a_consistent_and_valid_snapshot() -> TestResult {
    let dir = tempfile::tempdir()?;
    let original_path = dir.path().join("original.sqlite");
    let backup_path = dir.path().join("backups").join("backup.sqlite");

    let db = Database::open(&original_path)?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        ["lib-backup-1"],
    )?;

    db.backup_to(&backup_path)?;
    assert!(
        backup_path.is_file(),
        "arquivo de backup deve existir no destino"
    );

    let backup_db = Database::open(&backup_path)?;
    assert!(backup_db.has_current_schema()?);

    let count: i64 = backup_db.connection().query_row(
        "SELECT COUNT(*) FROM library WHERE library_id = 'lib-backup-1'",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(count, 1, "dados originais devem estar presentes no backup");
    Ok(())
}

#[test]
fn restore_from_recovers_database_and_preserves_schema() -> TestResult {
    let dir = tempfile::tempdir()?;
    let original_path = dir.path().join("original.sqlite");
    let backup_path = dir.path().join("backup.sqlite");
    let target_path = dir.path().join("restored.sqlite");

    let db = Database::open(&original_path)?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        ["lib-restore-test"],
    )?;
    db.backup_to(&backup_path)?;

    Database::restore_from(&backup_path, &target_path)?;
    assert!(target_path.is_file());

    let restored_db = Database::open(&target_path)?;
    assert!(restored_db.has_current_schema()?);
    let count: i64 = restored_db.connection().query_row(
        "SELECT COUNT(*) FROM library WHERE library_id = 'lib-restore-test'",
        [],
        |r| r.get(0),
    )?;
    assert_eq!(count, 1);
    Ok(())
}

#[test]
fn restore_from_rejects_unrelated_sqlite_without_touching_destination() -> TestResult {
    let dir = tempfile::tempdir()?;
    let backup_path = dir.path().join("unrelated.sqlite");
    let target_path = dir.path().join("target.sqlite");

    Database::open_in_memory()?.backup_to(&target_path)?;
    let original = std::fs::read(&target_path)?;
    rusqlite::Connection::open(&backup_path)?.execute_batch(
        "CREATE TABLE unrelated (value TEXT); INSERT INTO unrelated VALUES ('synthetic');",
    )?;

    assert!(matches!(
        Database::restore_from(&backup_path, &target_path),
        Err(Error::RestoreInvalidSchema)
    ));
    assert_eq!(std::fs::read(&target_path)?, original);
    Ok(())
}

#[test]
fn restore_safety_copy_includes_rows_still_in_the_destination_wal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let target_path = dir.path().join("target.sqlite");
    let backup_path = dir.path().join("replacement.sqlite");
    Database::open_in_memory()?.backup_to(&backup_path)?;

    let target = Database::open(&target_path)?;
    preserve_wal_on_close(&target)?;
    target.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('wal-only-row', 1)",
        [],
    )?;
    let wal_path = appended_sidecar(&target_path, "-wal");
    assert!(
        wal_path.exists(),
        "the synthetic row must remain in the WAL"
    );
    drop(target);
    assert!(
        wal_path.exists(),
        "the WAL should remain after the connection closes"
    );

    Database::restore_from(&backup_path, &target_path)?;

    let rollback_path = std::fs::read_dir(dir.path())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().contains("rollback."))
        .expect("restore creates a safety copy");
    let rollback = Database::open(rollback_path)?;
    let count: i64 = rollback.connection().query_row(
        "SELECT COUNT(*) FROM library WHERE library_id = 'wal-only-row'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(count, 1);
    Ok(())
}

#[test]
fn restore_removes_sidecars_for_custom_database_extensions() -> TestResult {
    let dir = tempfile::tempdir()?;
    let backup_path = dir.path().join("backup.sqlite");
    let target_path = dir.path().join("data.db");
    Database::open_in_memory()?.backup_to(&backup_path)?;
    let target = Database::open(&target_path)?;
    preserve_wal_on_close(&target)?;
    target.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('custom-extension-row', 1)",
        [],
    )?;
    let wal_path = appended_sidecar(&target_path, "-wal");
    let shm_path = appended_sidecar(&target_path, "-shm");
    assert!(wal_path.exists());
    drop(target);
    assert!(
        wal_path.exists(),
        "the WAL should remain after the connection closes"
    );

    Database::restore_from(&backup_path, &target_path)?;

    assert!(!wal_path.exists());
    assert!(!shm_path.exists());
    Ok(())
}

#[test]
fn restore_replaces_stale_wal_and_leaves_an_intact_database() -> TestResult {
    let dir = tempfile::tempdir()?;
    let backup_path = dir.path().join("backup.sqlite");
    let target_path = dir.path().join("target.sqlite");
    let backup = Database::open_in_memory()?;
    backup.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('restored-row', 1)",
        [],
    )?;
    backup.backup_to(&backup_path)?;

    let target = Database::open(&target_path)?;
    preserve_wal_on_close(&target)?;
    target.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('stale-row', 1)",
        [],
    )?;
    let wal_path = appended_sidecar(&target_path, "-wal");
    assert!(wal_path.exists());
    drop(target);
    assert!(wal_path.exists(), "the stale WAL should remain after close");

    Database::restore_from(&backup_path, &target_path)?;

    assert!(!wal_path.exists());
    let restored = Database::open(&target_path)?;
    let integrity: String =
        restored
            .connection()
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    assert_eq!(integrity, "ok");
    let restored_count: i64 = restored.connection().query_row(
        "SELECT COUNT(*) FROM library WHERE library_id = 'restored-row'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(restored_count, 1);
    let stale_count: i64 = restored.connection().query_row(
        "SELECT COUNT(*) FROM library WHERE library_id = 'stale-row'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(stale_count, 0);
    Ok(())
}

#[test]
fn restore_fails_when_destination_cannot_be_checkpointed_without_replacing_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let backup_path = dir.path().join("backup.sqlite");
    let target_path = dir.path().join("target.sqlite");
    Database::open_in_memory()?.backup_to(&backup_path)?;
    drop(Database::open(&target_path)?);
    let original = std::fs::read(&target_path)?;

    let mut locked_connection = rusqlite::Connection::open(&target_path)?;
    locked_connection.pragma_update(None, "journal_mode", "WAL")?;
    let transaction = locked_connection.transaction()?;
    transaction.execute(
        "INSERT INTO library (library_id, available) VALUES ('uncommitted-row', 1)",
        [],
    )?;

    assert!(Database::restore_from(&backup_path, &target_path).is_err());
    assert_eq!(std::fs::read(&target_path)?, original);
    drop(transaction);
    Ok(())
}

#[test]
fn restore_rejects_a_too_new_schema_version() -> TestResult {
    let dir = tempfile::tempdir()?;
    let backup_path = dir.path().join("future.sqlite");
    let target_path = dir.path().join("target.sqlite");
    Database::open_in_memory()?.backup_to(&backup_path)?;
    rusqlite::Connection::open(&backup_path)?.pragma_update(None, "user_version", 999)?;

    assert!(matches!(
        Database::restore_from(&backup_path, &target_path),
        Err(Error::SchemaTooNew {
            found: 999,
            known: 23
        })
    ));
    assert!(!target_path.exists());
    Ok(())
}

#[test]
fn restore_rejects_foreign_key_violations() -> TestResult {
    let dir = tempfile::tempdir()?;
    let backup_path = dir.path().join("orphaned.sqlite");
    let target_path = dir.path().join("target.sqlite");
    let database = Database::open_in_memory()?;
    database
        .connection()
        .pragma_update(None, "foreign_keys", "OFF")?;
    database.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES ('orphan-media', 'missing-library', 'lesson.mp4', 'lesson.mp4', 1, 0, 'available')",
        [],
    )?;
    database.backup_to(&backup_path)?;

    assert!(matches!(
        Database::restore_from(&backup_path, &target_path),
        Err(Error::RestoreInvalidSchema)
    ));
    assert!(!target_path.exists());
    Ok(())
}

#[test]
fn restore_accepts_an_older_valid_schema_for_migration_on_next_open() -> TestResult {
    let dir = tempfile::tempdir()?;
    let backup_path = dir.path().join("version-20.sqlite");
    let target_path = dir.path().join("restored.sqlite");
    let database = Database::open_in_memory()?;
    database.connection().execute_batch(
        "DROP TABLE library_root_dismissed;
         ALTER TABLE flashcard DROP COLUMN text_updated_at;",
    )?;
    database
        .connection()
        .pragma_update(None, "user_version", 20)?;
    database.backup_to(&backup_path)?;

    Database::restore_from(&backup_path, &target_path)?;
    assert_eq!(learnvault_core::file_schema_version(&target_path)?, 20);
    assert_eq!(Database::open(&target_path)?.schema_version()?, 23);
    Ok(())
}

fn appended_sidecar(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    name.into()
}

fn preserve_wal_on_close(database: &Database) -> rusqlite::Result<()> {
    database
        .connection()
        .pragma_update(None, "wal_autocheckpoint", 0)?;
    database
        .connection()
        .set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)?;
    Ok(())
}

#[test]
fn restore_from_rejects_missing_or_corrupt_file() -> TestResult {
    let dir = tempfile::tempdir()?;
    let missing_path = dir.path().join("missing.sqlite");
    let target_path = dir.path().join("target.sqlite");

    assert!(matches!(
        Database::restore_from(&missing_path, &target_path),
        Err(Error::BackupFileNotFound(_))
    ));

    let corrupt_path = dir.path().join("corrupt.sqlite");
    std::fs::write(&corrupt_path, b"not a valid sqlite database file")?;
    assert!(Database::restore_from(&corrupt_path, &target_path).is_err());

    Ok(())
}

#[cfg(unix)]
#[test]
fn backup_and_restore_do_not_follow_predictable_staging_links() -> TestResult {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = tempfile::tempdir()?;
    let backup = directory.path().join("backup.sqlite");
    let target = directory.path().join("target.sqlite");
    let victim = directory.path().join("unrelated.sqlite");
    Database::open_in_memory()?.backup_to(&victim)?;
    let original = std::fs::read(&victim)?;
    let source = Database::open_in_memory()?;
    source.connection().execute_batch("CREATE TABLE synthetic_marker(value TEXT); INSERT INTO synthetic_marker VALUES ('source');")?;
    for path in [
        backup.with_extension(format!("tmp.{}", std::process::id())),
        target.with_extension(format!("tmp.restore.{}", std::process::id())),
    ] {
        symlink(&victim, path)?;
    }
    source.backup_to(&backup)?;
    Database::restore_from(&backup, &target)?;
    assert!(
        std::fs::read(&victim)? == original,
        "unrelated database must stay unchanged"
    );
    for path in [&backup, &target] {
        assert_eq!(std::fs::metadata(path)?.permissions().mode() & 0o077, 0);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn restore_refuses_existing_rollback_paths_without_modifying_files() -> TestResult {
    use std::os::unix::fs::symlink;
    use std::time::{SystemTime, UNIX_EPOCH};
    let directory = tempfile::tempdir()?;
    let backup = directory.path().join("backup.sqlite");
    let target = directory.path().join("target.sqlite");
    let victim = directory.path().join("unrelated.txt");
    Database::open_in_memory()?.backup_to(&backup)?;
    Database::open_in_memory()?.backup_to(&target)?;
    let original = std::fs::read(&target)?;
    std::fs::write(&victim, b"synthetic unrelated file")?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    for timestamp in now..=now + 60 {
        symlink(
            &victim,
            target.with_extension(format!("rollback.{timestamp}.bak")),
        )?;
    }
    assert!(Database::restore_from(&backup, &target).is_err());
    assert_eq!(std::fs::read(&victim)?, b"synthetic unrelated file");
    assert!(
        std::fs::read(&target)? == original,
        "restore must preserve the target on collision"
    );
    Ok(())
}

#[test]
fn a_version_nineteen_database_gets_the_coalesce_keys_of_its_journal() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");
    {
        let db = Database::open(&path)?;
        // Rebuild the shape of version 19 from the current schema, with journal rows already in it.
        db.connection().execute_batch(
            "DROP TABLE sync_device;
             DROP TABLE library_root_dismissed;
             DROP INDEX sync_operation_key_idx;
             ALTER TABLE sync_operation DROP COLUMN coalesce_key;
             ALTER TABLE flashcard DROP COLUMN text_updated_at;
             INSERT INTO sync_operation (op_id, owner_id, device_id, client_at, received_at, payload) VALUES
               ('o1', 'ana', 'pc', 1, 1, '{\"kind\":\"progress\",\"media_id\":\"m1\",\"position_seconds\":5,\"completed\":false}'),
               ('o2', 'ana', 'pc', 2, 2, '{\"kind\":\"preference\",\"video_speed\":120,\"audio_speed\":100,\"subtitle_language\":\"\"}'),
               ('o3', 'ana', 'pc', 3, 3, '{\"kind\":\"note_deleted\",\"note_id\":\"n\"}');",
        )?;
        db.connection().pragma_update(None, "user_version", 19)?;
    }

    let db = Database::open(&path)?;
    assert_eq!(db.schema_version()?, 23);
    let key = |op: &str| -> Option<String> {
        db.connection()
            .query_row(
                "SELECT coalesce_key FROM sync_operation WHERE op_id = ?1",
                [op],
                |row| row.get(0),
            )
            .unwrap()
    };
    assert_eq!(key("o1").as_deref(), Some("p:m1"));
    assert_eq!(key("o2").as_deref(), Some("pref"));
    assert_eq!(
        key("o3"),
        None,
        "an operation that is never replaced has no key"
    );
    assert!(table_exists(&db, "sync_device"));
    assert!(db.has_current_schema()?);
    Ok(())
}

#[test]
fn populated_v096_database_migrates_without_losing_study_data() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("learnvault.sqlite");

    {
        let mut conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let tx = conn.transaction()?;
        // v0.9.6 shipped schema 21. Build that schema without running current migrations first.
        for step in &learnvault_core::migration_steps_for_test()[..21] {
            tx.execute_batch(step)?;
        }
        tx.pragma_update(None, "user_version", 21)?;
        tx.commit()?;

        conn.execute_batch(
            "INSERT INTO library (library_id, available) VALUES ('bib-1', 1);
             INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('mid-1', 'bib-1', 'Course/Lesson/video.mp4', 'course/lesson/video.mp4', 2048, 1760000000, 'available');
             INSERT INTO scan (library_id, status, completed_at, complete)
             VALUES ('bib-1', 'completed', 1760000000, 1);
             INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
             VALUES ('owner-a', 'mid-1', 42, 1, 1760000000);
             INSERT INTO user (user_id, username, password_hash, is_admin, created_at)
             VALUES ('usr-1', 'admin', 'hash', 1, 1760000000);
             INSERT INTO session (token_hash, user_id, expires_at)
             VALUES ('tok-1', 'usr-1', 1760003600);
             INSERT INTO study_note (note_id, owner_id, media_id, position_seconds, content, created_at, updated_at)
             VALUES ('note-1', 'owner-a', 'mid-1', 10, 'old note', 1760000000, 1760000000);
             INSERT INTO flashcard (card_id, owner_id, media_id, front, back, interval_days, next_due, created_at, updated_at)
             VALUES ('card-1', 'owner-a', 'mid-1', 'Photosynthesis', 'Plants use light', 1, 1760086400, 1760000000, 1760000000);
             INSERT INTO flashcard_review (review_id, owner_id, card_id, rating, previous_interval_days, next_due, reviewed_at)
             VALUES (7, 'owner-a', 'card-1', 'good', 0, 1760086400, 1760000000);
             INSERT INTO sync_state (owner_id, device_id, pull_cursor, updated_at)
             VALUES ('owner-a', 'desktop-a', 42, 1760000000);
             INSERT INTO sync_outbox (op_id, owner_id, device_id, client_at, payload, queued_at)
             VALUES ('pending-review', 'owner-a', 'desktop-a', 1760000000,
             '{\"kind\":\"flashcard_review\",\"card_id\":\"card-1\",\"rating\":\"good\",\"reviewed_at\":1760000000}', 1760000000);",
        )?;
    }

    let db = Database::open(&path)?;
    assert_eq!(
        db.schema_version()?,
        learnvault_core::CURRENT_SCHEMA_VERSION
    );

    let integrity: String = db
        .connection()
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    assert_eq!(
        integrity, "ok",
        "migrated database must pass PRAGMA integrity_check"
    );

    let mut fk_stmt = db.connection().prepare("PRAGMA foreign_key_check")?;
    let fk_violations: Vec<String> = fk_stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    assert!(
        fk_violations.is_empty(),
        "migrated database must have no foreign key violations: {fk_violations:?}"
    );

    let note_content: String = db.connection().query_row(
        "SELECT content FROM study_note WHERE note_id = 'note-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(note_content, "old note");

    let (card_front, card_updated, card_text_updated): (String, i64, i64) =
        db.connection().query_row(
            "SELECT front, updated_at, text_updated_at FROM flashcard WHERE card_id = 'card-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    assert_eq!(card_front, "Photosynthesis");
    assert_eq!(card_text_updated, card_updated);
    let history = learnvault_core::list_flashcard_reviews(&db, "owner-a", "card-1", 10)?;
    assert_eq!(history.len(), 1);
    assert_eq!((history[0].review_id, history[0].next_due), (7, 1760086400));
    assert_eq!(history[0].rating, learnvault_core::ReviewRating::Good);
    let results = learnvault_core::search::search(&db, "owner-a", true, "photosynthesis", 10)?;
    assert_eq!(results.flashcards[0].card_id, "card-1");
    let state = learnvault_core::sync_state(&db, "owner-a")?;
    assert_eq!(
        (state.device_id.as_str(), state.pull_cursor),
        ("desktop-a", 42)
    );
    let pending = learnvault_core::pending_operations(&db, "owner-a", 10)?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].op_id, "pending-review");
    assert!(
        matches!(&pending[0].payload, learnvault_core::SyncPayload::FlashcardReview { rating, .. } if rating == "good")
    );

    let position: i64 = db.connection().query_row(
        "SELECT position_seconds FROM study_state WHERE owner_id = 'owner-a' AND media_id = 'mid-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(position, 42);
    Ok(())
}

#[test]
fn a_fresh_replica_matches_the_upgraded_legacy_card_text() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("legacy.sqlite");
    let media = "INSERT INTO library (library_id, available) VALUES ('library-1', 1);
                 INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
                 VALUES ('media-a', 'library-1', 'lesson.mp4', 'lesson.mp4', 128, 1, 'available');";
    {
        let mut conn = rusqlite::Connection::open(&path)?;
        let tx = conn.transaction()?;
        for step in &learnvault_core::migration_steps_for_test()[..21] {
            tx.execute_batch(step)?;
        }
        tx.pragma_update(None, "user_version", 21)?;
        tx.execute_batch(media)?;
        tx.execute_batch(
            r#"INSERT INTO flashcard (card_id, owner_id, media_id, front, back, interval_days, next_due, created_at, updated_at)
               VALUES ('card-a', 'owner-a', 'media-a', 'Original', 'Answer', 1, 86600, 100, 200);
               INSERT INTO flashcard_review (owner_id, card_id, rating, previous_interval_days, next_due, reviewed_at)
               VALUES ('owner-a', 'card-a', 'good', 0, 86600, 200);
               INSERT INTO sync_device (owner_id, device_id, last_seen_at)
               VALUES ('owner-a', 'writer', 200);
               INSERT INTO sync_operation (op_id, owner_id, device_id, client_at, received_at, payload) VALUES
               ('bad-json', 'owner-other', 'writer', 50, 50, '{not-json'),
               ('create', 'owner-a', 'writer', 100, 100, '{"kind":"flashcard_created","card_id":"card-a","media_id":"media-a","front":"Original","back":"Answer","created_at":100}'),
               ('review', 'owner-a', 'writer', 200, 200, '{"kind":"flashcard_review","card_id":"card-a","rating":"good","reviewed_at":200}'),
               ('edit', 'owner-a', 'offline', 150, 201, '{"kind":"flashcard_updated","card_id":"card-a","front":"Offline edit","back":"Answer","updated_at":150}');
               INSERT INTO flashcard (card_id, owner_id, media_id, front, back, interval_days, next_due, created_at, updated_at)
               VALUES ('card-b', 'owner-a', 'media-a', 'Browser edit', 'Answer', 1, 86600, 100, 200);
               INSERT INTO flashcard_review (owner_id, card_id, rating, previous_interval_days, next_due, reviewed_at)
               VALUES ('owner-a', 'card-b', 'good', 0, 86600, 200);
               INSERT INTO sync_operation (op_id, owner_id, device_id, client_at, received_at, payload) VALUES
               ('create-b', 'owner-a', 'writer', 100, 100, '{"kind":"flashcard_created","card_id":"card-b","media_id":"media-a","front":"Original B","back":"Answer","created_at":100}'),
               ('edit-b-old', 'owner-a', 'offline', 150, 150, '{"kind":"flashcard_updated","card_id":"card-b","front":"Older edit","back":"Answer","updated_at":150}');
               INSERT INTO sync_outbox (op_id, owner_id, device_id, client_at, payload, queued_at) VALUES
               ('edit-b-server', 'owner-a', 'server', 180, '{"kind":"flashcard_updated","card_id":"card-b","front":"Browser edit","back":"Answer","updated_at":180}', 180);"#,
        )?;
        tx.commit()?;
    }
    let server = Database::open(&path)?;
    let replica = Database::open_in_memory()?;
    replica.connection().execute_batch(media)?;
    let page = learnvault_core::pull_operations(&server, "owner-a", "reader", 0, 10)?;
    learnvault_core::apply_operations(&replica, "owner-a", &page.items, page.cursor)?;
    let mut server_cards = learnvault_core::list_flashcards(&server, "owner-a", "media-a")?;
    let mut replica_cards = learnvault_core::list_flashcards(&replica, "owner-a", "media-a")?;
    server_cards.sort_by(|a, b| a.card_id.cmp(&b.card_id));
    replica_cards.sort_by(|a, b| a.card_id.cmp(&b.card_id));
    assert_eq!(server_cards.len(), 2);
    assert_eq!(replica_cards.len(), 2);
    for (server_card, replica_card) in server_cards.iter().zip(replica_cards.iter()) {
        assert_eq!(server_card.card_id, replica_card.card_id);
        assert_eq!(server_card.front, replica_card.front);
    }
    assert_eq!(server_cards[0].front, "Offline edit");
    assert_eq!(server_cards[1].front, "Browser edit");
    Ok(())
}
