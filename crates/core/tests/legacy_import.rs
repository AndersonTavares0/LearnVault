use std::path::{Path, PathBuf};

use learnvault_core::{Error, LegacyImportOptions, import_go_database};
use rusqlite::Connection;
use tempfile::TempDir;

fn legacy_fixture(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("go.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
         CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL UNIQUE COLLATE NOCASE,
           password_hash TEXT NOT NULL, role TEXT NOT NULL, created_at TEXT NOT NULL,
           must_change_credentials INTEGER NOT NULL DEFAULT 0 CHECK (must_change_credentials IN (0, 1)));
         CREATE TABLE sessions (token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL,
           expires_at TEXT NOT NULL, created_at TEXT NOT NULL);
         CREATE TABLE catalog_categories (id INTEGER PRIMARY KEY, path TEXT NOT NULL, title TEXT NOT NULL);
         CREATE TABLE catalog_courses (id INTEGER PRIMARY KEY, category_id INTEGER NOT NULL, path TEXT NOT NULL, title TEXT NOT NULL);
         CREATE TABLE catalog_lessons (id INTEGER PRIMARY KEY, module_id INTEGER NOT NULL,
           path TEXT NOT NULL, title TEXT NOT NULL);
         CREATE TABLE catalog_modules (id INTEGER PRIMARY KEY, course_id INTEGER NOT NULL,
           path TEXT NOT NULL, title TEXT NOT NULL);
         CREATE TABLE catalog_files (id INTEGER PRIMARY KEY, lesson_id INTEGER NOT NULL,
           path TEXT NOT NULL UNIQUE, name TEXT NOT NULL, kind TEXT NOT NULL, size INTEGER NOT NULL);
         CREATE TABLE catalog_root_files (id INTEGER PRIMARY KEY, path TEXT NOT NULL, name TEXT NOT NULL, size INTEGER NOT NULL);
         CREATE TABLE catalog_nodes_json (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
          CREATE TABLE media_removal (relative_path TEXT PRIMARY KEY, removed_at INTEGER NOT NULL,
            is_media INTEGER CHECK (is_media IN (0, 1)));
         CREATE TABLE study_state (user_id INTEGER NOT NULL, media_path TEXT NOT NULL,
           position_seconds INTEGER NOT NULL, completed INTEGER NOT NULL, updated_at INTEGER NOT NULL,
           PRIMARY KEY (user_id, media_path));
           INSERT INTO schema_migrations VALUES
             (1, '2026-01-01 00:00:00'), (2, '2026-01-01 00:00:00'),
             (3, '2026-01-01 00:00:00'), (4, '2026-01-01 00:00:00'),
             (5, '2026-01-01 00:00:00'), (6, '2026-01-01 00:00:00'),
             (7, '2026-01-01 00:00:00'), (8, '2026-01-01 00:00:00');
         INSERT INTO catalog_modules VALUES (1, 1, 'Curso', 'Curso');
         INSERT INTO catalog_lessons VALUES (2, 1, 'Curso/Aula', 'Aula');
          INSERT INTO users VALUES (7, 'Alice', '$argon2id$v=19$m=1,t=1,p=1$x$y', 'admin', '2026-01-01 00:00:00', 1);
         INSERT INTO sessions VALUES ('token-hash', 7, '2030-01-01 00:00:00', '2026-01-01 00:00:00');
          INSERT INTO catalog_files VALUES (1, 2, 'Curso\\Aula\\video.mp4', 'video.mp4', 'media', 42);
          INSERT INTO catalog_root_files VALUES (1, 'root.pdf', 'root.pdf', 7);
          INSERT INTO media_removal VALUES ('Curso/Aula/video.mp4', 1760000000, 1);
         INSERT INTO study_state VALUES (7, 'Curso/Aula/video.mp4', 12, 1, 1760000000);",
    )
    .unwrap();
    path
}

fn options(dir: &TempDir, input: &Path) -> LegacyImportOptions {
    LegacyImportOptions {
        input: input.to_owned(),
        output: dir.path().join("rust.sqlite"),
        backup: dir.path().join("go.sqlite.backup"),
        library_id: "library-1".to_owned(),
    }
}

#[test]
fn imports_users_media_sessions_progress_and_removals_after_backup() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input)
        .unwrap()
        .execute(
            "UPDATE sessions SET expires_at = '2030-01-01T00:00:00.123456789Z'",
            [],
        )
        .unwrap();
    let report = import_go_database(options(&dir, &input)).unwrap();

    assert_eq!(report.users, 1);
    assert_eq!(report.media, 2);
    assert_eq!(report.sessions, 1);
    assert_eq!(report.study_states, 1);
    assert_eq!(report.removals, 1);
    assert!(dir.path().join("go.sqlite.backup").is_file());
    let db = learnvault_core::Database::open(dir.path().join("rust.sqlite")).unwrap();
    assert_eq!(
        db.connection()
            .query_row("SELECT COUNT(*) FROM user", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.connection()
            .query_row("SELECT expires_at FROM session", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1893456000
    );
    assert_eq!(
        db.connection()
            .query_row(
                "SELECT relative_path FROM media WHERE relative_path = 'Curso/Aula/video.mp4'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "Curso/Aula/video.mp4"
    );
    assert_eq!(
        db.connection()
            .query_row(
                "SELECT COUNT(*) FROM media WHERE relative_path = 'root.pdf' AND size = 7",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        db.connection()
            .query_row("SELECT must_change_credentials FROM user", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn rejects_schema_other_than_go_v8_before_backup() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input)
        .unwrap()
        .execute("DELETE FROM schema_migrations WHERE version = 8", [])
        .unwrap();
    let result = import_go_database(options(&dir, &input));
    assert!(
        matches!(result, Err(Error::LegacyImportInvalidValue { field, .. }) if field == "schema_migrations")
    );
    assert!(!dir.path().join("go.sqlite.backup").exists());
}

#[test]
fn refuses_existing_output_without_replacing_it() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    let options = options(&dir, &input);
    std::fs::write(&options.output, b"sentinel").unwrap();
    let result = import_go_database(options.clone());
    assert!(
        matches!(result, Err(Error::LegacyImportInvalidValue { field, .. }) if field == "output/backup")
    );
    assert_eq!(std::fs::read(options.output).unwrap(), b"sentinel");
}

#[test]
fn backup_includes_uncheckpointed_wal_data() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    let writer = Connection::open(&input).unwrap();
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer
        .execute("UPDATE users SET username = 'WalAlice'", [])
        .unwrap();

    let report = import_go_database(options(&dir, &input)).unwrap();
    assert_eq!(report.users, 1);
    let backup = Connection::open(dir.path().join("go.sqlite.backup")).unwrap();
    assert_eq!(
        backup
            .query_row("SELECT username FROM users", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "WalAlice"
    );
}

#[test]
fn rejects_orphan_study_state_without_promoting_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input)
        .unwrap()
        .execute("UPDATE study_state SET media_path = 'missing.mp4'", [])
        .unwrap();
    let output = options(&dir, &input).output.clone();
    assert!(matches!(
        import_go_database(options(&dir, &input)),
        Err(Error::LegacyImportOrphan { .. })
    ));
    assert!(!output.exists());
}

#[test]
fn rejects_invalid_role_and_leaves_source_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input)
        .unwrap()
        .execute("UPDATE users SET role = 'owner'", [])
        .unwrap();
    let before = std::fs::read(&input).unwrap();
    assert!(matches!(
        import_go_database(options(&dir, &input)),
        Err(Error::LegacyImportInvalidValue { .. })
    ));
    assert_eq!(std::fs::read(&input).unwrap(), before);
}

#[test]
fn rejects_normalized_path_collision() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input).unwrap().execute(
        "INSERT INTO catalog_files VALUES (2, 2, 'Curso/Aula/video.mp4', 'video.mp4', 'media', 42)", [],
    ).unwrap();
    assert!(matches!(
        import_go_database(options(&dir, &input)),
        Err(Error::LegacyImportCollision { .. })
    ));
}

#[test]
fn rejects_case_folded_collision_on_case_insensitive_hosts() {
    if !cfg!(windows) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input).unwrap().execute(
        "INSERT INTO catalog_files VALUES (2, 2, 'curso/aula/VIDEO.mp4', 'video.mp4', 'media', 42)", [],
    ).unwrap();
    assert!(matches!(
        import_go_database(options(&dir, &input)),
        Err(Error::LegacyImportCollision { .. })
    ));
}

#[test]
fn path_policy_rejects_nul_and_only_rejects_colon_on_windows() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input)
        .unwrap()
        .execute(
            "UPDATE catalog_files SET path = 'Curso/Aula/bad' || char(0) || '.mp4'",
            [],
        )
        .unwrap();
    assert!(matches!(
        import_go_database(options(&dir, &input)),
        Err(Error::LegacyImportInvalidPath { .. })
    ));

    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input)
        .unwrap()
        .execute(
            "UPDATE catalog_files SET path = 'Curso/Aula/name:part.mp4'",
            [],
        )
        .unwrap();
    Connection::open(&input)
        .unwrap()
        .execute_batch(
            "UPDATE media_removal SET relative_path = 'Curso/Aula/name:part.mp4'; UPDATE study_state SET media_path = 'Curso/Aula/name:part.mp4'",
        )
        .unwrap();
    if cfg!(windows) {
        assert!(matches!(
            import_go_database(options(&dir, &input)),
            Err(Error::LegacyImportInvalidPath { .. })
        ));
    } else {
        assert!(import_go_database(options(&dir, &input)).is_ok());
    }
}

#[test]
fn orphan_tombstone_is_reported_instead_of_silently_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input)
        .unwrap()
        .execute(
            "INSERT INTO media_removal VALUES ('missing.mp4', 1760000000, 1)",
            [],
        )
        .unwrap();
    let result = import_go_database(options(&dir, &input));
    assert!(
        matches!(result, Err(Error::LegacyImportOrphan { relation, .. }) if relation == "media_removal.relative_path")
    );
    assert!(!dir.path().join("rust.sqlite").exists());
}

#[test]
fn rejects_tombstone_with_unrepresentable_media_flag() {
    let dir = tempfile::tempdir().unwrap();
    let input = legacy_fixture(&dir);
    Connection::open(&input)
        .unwrap()
        .execute("UPDATE media_removal SET is_media = 0", [])
        .unwrap();
    let result = import_go_database(options(&dir, &input));
    assert!(
        matches!(result, Err(Error::LegacyImportInvalidValue { field, .. }) if field == "media_removal.is_media")
    );
    assert!(!dir.path().join("rust.sqlite").exists());
}
