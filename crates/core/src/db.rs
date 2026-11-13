//! Abertura do banco de dados e garantia do schema.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::config::DbConfig;
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Error, Result};
use crate::migrations::{SCHEMA_VERSION, migrate_to_latest, schema_version};

/// Conexão SQLite com o schema do domínio aplicado.
///
/// A estrutura garante três invariantes em toda conexão: chaves estrangeiras
/// habilitadas, `journal_mode` em WAL solicitado aos bancos persistentes e schema na
/// versão conhecida por este programa. Um banco gravado por uma versão mais nova é
/// recusado antes de qualquer escrita.
#[derive(Debug)]
pub struct Database {
    pub(crate) conn: Connection,
}

/// Versão do schema que este binário consegue servir.
pub const CURRENT_SCHEMA_VERSION: usize = SCHEMA_VERSION;
const MAX_SCHEMA_SQL_BYTES: i64 = 64 * 1024;

impl Database {
    /// Abre (ou cria) o banco em `path` e aplica a cadeia de migrações.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let conn = Connection::open(path).map_err(|source| Error::OpenDatabase {
            path: PathBuf::from(path),
            source,
        })?;
        Self::from_connection(conn, true)
    }

    /// Abre um banco em memória com o schema aplicado.
    ///
    /// Útil para execuções efêmeras e para os testes. Não habilita WAL, que só faz
    /// sentido em arquivos persistentes.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::from_connection(conn, false)
    }

    /// Empresta a conexão para leitura e escrita.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Versão do schema registrada no banco (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<usize> {
        schema_version(&self.conn).map_err(read_schema_version)
    }

    /// Verifica se o banco está exatamente no schema suportado e mantém as tabelas essenciais.
    ///
    /// A distinção entre `NoneSet`, versões anteriores e versões posteriores é intencional:
    /// nenhuma delas é uma dependência pronta para o servidor HTTP, mesmo quando a leitura da
    /// versão não produz erro.
    pub fn has_current_schema(&self) -> Result<bool> {
        if schema_version(&self.conn).map_err(read_schema_version)? != CURRENT_SCHEMA_VERSION {
            return Ok(false);
        }

        for table in REQUIRED_FTS_TABLES {
            let found: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_list
                 WHERE schema = 'main' AND type = 'virtual' AND name = ?1",
                [table],
                |row| row.get(0),
            )?;
            if found != 1 {
                return Ok(false);
            }
        }

        for definition in REQUIRED_TABLES {
            if !definition_matches(&self.conn, definition)? {
                return Ok(false);
            }
        }
        let table_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM (
                 SELECT 1 FROM pragma_table_list
                 WHERE schema = 'main' AND type = 'table' AND name NOT LIKE 'sqlite_%'
                 LIMIT ?1
             )",
            [REQUIRED_TABLES.len() as i64 + 1],
            |row| row.get(0),
        )?;
        if table_count != REQUIRED_TABLES.len() as i64
            || !index_matches(
                &self.conn,
                "session",
                Some("session_expires_at_idx"),
                false,
                &[("expires_at", "BINARY")],
            )?
            || !index_matches(
                &self.conn,
                "study_note",
                Some("study_note_owner_media_idx"),
                false,
                &[
                    ("owner_id", "BINARY"),
                    ("media_id", "BINARY"),
                    ("position_seconds", "BINARY"),
                ],
            )?
            || !index_matches(
                &self.conn,
                "pdf_annotation",
                Some("pdf_annotation_owner_media_idx"),
                false,
                &[
                    ("owner_id", "BINARY"),
                    ("media_id", "BINARY"),
                    ("page", "BINARY"),
                ],
            )?
            || !index_matches(
                &self.conn,
                "flashcard",
                Some("flashcard_owner_media_due_idx"),
                false,
                &[
                    ("owner_id", "BINARY"),
                    ("media_id", "BINARY"),
                    ("next_due", "BINARY"),
                ],
            )?
            || !index_matches(
                &self.conn,
                "flashcard_review",
                Some("flashcard_review_owner_card_idx"),
                false,
                &[
                    ("owner_id", "BINARY"),
                    ("card_id", "BINARY"),
                    ("reviewed_at", "BINARY"),
                ],
            )?
            || !index_matches(
                &self.conn,
                "study_favorite",
                Some("study_favorite_owner_kind_idx"),
                false,
                &[
                    ("owner_id", "BINARY"),
                    ("target_kind", "BINARY"),
                    ("created_at", "BINARY"),
                ],
            )?
            || !index_matches(
                &self.conn,
                "media_bookmark",
                Some("media_bookmark_owner_media_idx"),
                false,
                &[
                    ("owner_id", "BINARY"),
                    ("media_id", "BINARY"),
                    ("position_seconds", "BINARY"),
                    ("created_at", "BINARY"),
                ],
            )?
        {
            return Ok(false);
        }
        Ok(
            unique_index_matches(&self.conn, "user", &[("username", "NOCASE")])?
                && unique_index_matches(
                    &self.conn,
                    "media",
                    &[("library_id", "BINARY"), ("relative_path_key", "BINARY")],
                )?,
        )
    }

    fn from_connection(conn: Connection, persistent: bool) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, true)?;

        // A assinatura legada é lida antes de qualquer alteração no arquivo. Isso precisa
        // ocorrer antes da versão para que um banco Go nunca seja tratado como uma migração
        // Rust, ainda que o arquivo tenha uma versão SQLite inesperada.
        if is_legacy_go_database(&conn)? {
            return Err(Error::LegacyGoDatabase);
        }

        // A versão é lida antes de qualquer alteração no arquivo. Recusar um schema mais
        // novo precisa ser uma operação sem efeito colateral: nada de trocar o journal ou
        // criar tabelas em um banco que este binário não conhece.
        let found = schema_version(&conn).map_err(read_schema_version)?;
        if found > SCHEMA_VERSION {
            return Err(Error::SchemaTooNew {
                found,
                known: SCHEMA_VERSION,
            });
        }

        if persistent {
            conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        }

        let mut conn = conn;
        migrate_to_latest(&mut conn).map_err(Error::Migrations)?;

        Ok(Self { conn })
    }

    /// Cria uma cópia de segurança transacional e íntegra do banco de dados em `destination`.
    ///
    /// Utiliza a API de backup online do SQLite para capturar o estado atual da conexão,
    /// incluindo páginas ativas do journal WAL, gravando atomicamente no destino final e
    /// validando com `PRAGMA integrity_check`.
    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<()> {
        let dest_path = destination.as_ref();
        if let Some(parent) = dest_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|source| Error::BackupIo {
                operation: "criar diretório de destino".to_owned(),
                path: dest_path.to_path_buf(),
                source,
            })?;
        }
        let (_staging_directory, staging_file) = private_staging_file(dest_path)?;
        let temp_path = staging_file.path().to_path_buf();
        let mut dest_conn = Connection::open(&temp_path).map_err(|source| Error::OpenDatabase {
            path: temp_path.clone(),
            source,
        })?;
        dest_conn.busy_timeout(Duration::from_secs(5))?;

        let backup_job = rusqlite::backup::Backup::new(&self.conn, &mut dest_conn)?;
        let mut retries = 0;
        loop {
            match backup_job.step(256)? {
                rusqlite::backup::StepResult::Done => break,
                rusqlite::backup::StepResult::More => continue,
                rusqlite::backup::StepResult::Busy | rusqlite::backup::StepResult::Locked => {
                    retries += 1;
                    if retries > 300 {
                        drop(backup_job);
                        drop(dest_conn);
                        let _ = std::fs::remove_file(&temp_path);
                        return Err(Error::BackupTimeout);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => {
                    drop(backup_job);
                    drop(dest_conn);
                    let _ = std::fs::remove_file(&temp_path);
                    return Err(Error::BackupFailed(
                        "resultado inesperado do backup".to_owned(),
                    ));
                }
            }
        }
        drop(backup_job);

        let integrity: String =
            dest_conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            drop(dest_conn);
            let _ = std::fs::remove_file(&temp_path);
            return Err(Error::BackupIntegrityFailed(integrity));
        }
        drop(dest_conn);

        staging_file
            .persist(dest_path)
            .map_err(|error| Error::BackupIo {
                source: error.error,
                operation: "renomear arquivo de backup para destino final".to_owned(),
                path: dest_path.to_path_buf(),
            })?;

        Ok(())
    }

    /// Restaura um banco de dados a partir de um arquivo de backup em `backup_path`.
    ///
    /// Valida a integridade (`PRAGMA integrity_check`) e o schema antes de aplicar,
    /// cria uma cópia de segurança prévia se o banco de destino já existia, e substitui
    /// atomicamente o banco alvo.
    pub fn restore_from(
        backup_path: impl AsRef<Path>,
        target_path: impl AsRef<Path>,
    ) -> Result<()> {
        let backup_path = backup_path.as_ref();
        let target_path = target_path.as_ref();

        if !backup_path.is_file() {
            return Err(Error::BackupFileNotFound(backup_path.to_path_buf()));
        }

        let backup_conn =
            Connection::open_with_flags(backup_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|source| Error::OpenDatabase {
                    path: backup_path.to_path_buf(),
                    source,
                })?;
        backup_conn.busy_timeout(Duration::from_secs(5))?;

        let integrity: String =
            backup_conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(Error::BackupIntegrityFailed(integrity));
        }

        if is_legacy_go_database(&backup_conn)? {
            return Err(Error::LegacyGoDatabase);
        }

        let found = schema_version(&backup_conn).map_err(read_schema_version)?;
        if found > SCHEMA_VERSION {
            return Err(Error::SchemaTooNew {
                found,
                known: SCHEMA_VERSION,
            });
        }

        if let Some(parent) = target_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|source| Error::BackupIo {
                operation: "criar diretório de destino".to_owned(),
                path: target_path.to_path_buf(),
                source,
            })?;
        }

        if target_path.exists() {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let copy_path = target_path.with_extension(format!("rollback.{timestamp}.bak"));
            let (_directory, mut copy) = private_staging_file(target_path)?;
            let copy_result = (|| -> std::io::Result<()> {
                std::io::copy(&mut std::fs::File::open(target_path)?, copy.as_file_mut())?;
                copy.as_file().sync_all()?;
                copy.persist_noclobber(&copy_path)
                    .map_err(|error| error.error)?;
                Ok(())
            })();
            copy_result.map_err(|source| Error::BackupIo {
                operation: "create rollback copy".to_owned(),
                path: copy_path.clone(),
                source,
            })?;
        }

        let (_staging_directory, staging_file) = private_staging_file(target_path)?;
        let temp_target = staging_file.path().to_path_buf();
        let mut dest_conn =
            Connection::open(&temp_target).map_err(|source| Error::OpenDatabase {
                path: temp_target.clone(),
                source,
            })?;
        dest_conn.busy_timeout(Duration::from_secs(5))?;
        dest_conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;

        let backup_job = rusqlite::backup::Backup::new(&backup_conn, &mut dest_conn)?;
        let mut retries = 0;
        loop {
            match backup_job.step(256)? {
                rusqlite::backup::StepResult::Done => break,
                rusqlite::backup::StepResult::More => continue,
                rusqlite::backup::StepResult::Busy | rusqlite::backup::StepResult::Locked => {
                    retries += 1;
                    if retries > 300 {
                        drop(backup_job);
                        drop(dest_conn);
                        let _ = std::fs::remove_file(&temp_target);
                        return Err(Error::BackupTimeout);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => {
                    drop(backup_job);
                    drop(dest_conn);
                    let _ = std::fs::remove_file(&temp_target);
                    return Err(Error::BackupFailed(
                        "falha inesperada na restauração".to_owned(),
                    ));
                }
            }
        }
        drop(backup_job);
        drop(backup_conn);
        drop(dest_conn);

        let wal = target_path.with_extension("sqlite-wal");
        let shm = target_path.with_extension("sqlite-shm");
        let _ = std::fs::remove_file(&wal);
        let _ = std::fs::remove_file(&shm);

        staging_file
            .persist(target_path)
            .map_err(|error| Error::BackupIo {
                operation: "replace restored database".to_owned(),
                path: target_path.to_path_buf(),
                source: error.error,
            })?;

        Ok(())
    }
}

struct TableDefinition {
    name: &'static str,
    columns: &'static [(&'static str, &'static str, bool, Option<&'static str>, i64)],
}

/// FTS5 virtual tables created by schema version 13.
const REQUIRED_FTS_TABLES: &[&str] = &["study_note_fts", "media_fts", "course_fts"];

const REQUIRED_TABLES: &[TableDefinition] = &[
    TableDefinition {
        name: "library",
        columns: &[
            ("library_id", "TEXT", true, None, 1),
            ("available", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "media",
        columns: &[
            ("media_id", "TEXT", true, None, 1),
            ("library_id", "TEXT", true, None, 0),
            ("relative_path", "TEXT", true, None, 0),
            ("relative_path_key", "TEXT", true, None, 0),
            ("size", "INTEGER", true, None, 0),
            ("mtime", "INTEGER", true, None, 0),
            ("availability", "TEXT", true, None, 0),
        ],
    },
    TableDefinition {
        name: "scan",
        columns: &[
            ("scan_id", "INTEGER", false, None, 1),
            ("library_id", "TEXT", true, None, 0),
            ("status", "TEXT", true, None, 0),
            ("completed_at", "INTEGER", true, None, 0),
            ("complete", "INTEGER", true, Some("1"), 0),
        ],
    },
    TableDefinition {
        name: "study_state",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("media_id", "TEXT", true, None, 2),
            ("position_seconds", "INTEGER", true, Some("0"), 0),
            ("completed", "INTEGER", true, None, 0),
            ("updated_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "study_day",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("day", "TEXT", true, None, 2),
            ("last_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "focus_session",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("session_id", "TEXT", true, None, 2),
            ("kind", "TEXT", true, None, 0),
            ("planned_seconds", "INTEGER", true, None, 0),
            ("actual_seconds", "INTEGER", true, None, 0),
            ("completed", "INTEGER", true, None, 0),
            ("started_at", "INTEGER", true, None, 0),
            ("ended_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "lesson_completion_event",
        columns: &[
            ("event_id", "INTEGER", false, None, 1),
            ("owner_id", "TEXT", true, None, 0),
            ("media_id", "TEXT", true, None, 0),
            ("completed_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "xp_event",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("event_key", "TEXT", true, None, 2),
            ("source", "TEXT", true, None, 0),
            ("source_id", "TEXT", true, None, 0),
            ("amount", "INTEGER", true, None, 0),
            ("created_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "daily_mission_claim",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("day", "TEXT", true, None, 2),
            ("mission_id", "TEXT", true, None, 3),
            ("claimed_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "gamification_settings",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("weekly_goal", "INTEGER", true, Some("3"), 0),
        ],
    },
    TableDefinition {
        name: "user",
        columns: &[
            ("user_id", "TEXT", true, None, 1),
            ("username", "TEXT", true, None, 0),
            ("password_hash", "TEXT", true, None, 0),
            ("is_admin", "INTEGER", true, None, 0),
            ("created_at", "INTEGER", true, None, 0),
            ("must_change_credentials", "INTEGER", true, Some("0"), 0),
        ],
    },
    TableDefinition {
        name: "session",
        columns: &[
            ("token_hash", "TEXT", true, None, 1),
            ("user_id", "TEXT", true, None, 0),
            ("expires_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "scan_inaccessible",
        columns: &[
            ("scan_id", "INTEGER", true, None, 1),
            ("relative_path", "TEXT", true, None, 2),
        ],
    },
    TableDefinition {
        name: "media_removal",
        columns: &[
            ("library_id", "TEXT", true, None, 1),
            ("media_id", "TEXT", true, None, 2),
            ("removed_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "study_note",
        columns: &[
            ("note_id", "TEXT", true, None, 1),
            ("owner_id", "TEXT", true, None, 0),
            ("media_id", "TEXT", true, None, 0),
            ("position_seconds", "INTEGER", true, None, 0),
            ("content", "TEXT", true, None, 0),
            ("created_at", "INTEGER", true, None, 0),
            ("updated_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "pdf_annotation",
        columns: &[
            ("annotation_id", "TEXT", true, None, 1),
            ("owner_id", "TEXT", true, None, 0),
            ("media_id", "TEXT", true, None, 0),
            ("page", "INTEGER", true, None, 0),
            ("kind", "TEXT", true, None, 0),
            ("color", "TEXT", true, None, 0),
            ("rects", "TEXT", true, None, 0),
            ("quote", "TEXT", true, None, 0),
            ("content", "TEXT", true, None, 0),
            ("created_at", "INTEGER", true, None, 0),
            ("updated_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "flashcard",
        columns: &[
            ("card_id", "TEXT", true, None, 1),
            ("owner_id", "TEXT", true, None, 0),
            ("media_id", "TEXT", true, None, 0),
            ("front", "TEXT", true, None, 0),
            ("back", "TEXT", true, None, 0),
            ("interval_days", "INTEGER", true, None, 0),
            ("next_due", "INTEGER", true, None, 0),
            ("created_at", "INTEGER", true, None, 0),
            ("updated_at", "INTEGER", true, None, 0),
            ("position_seconds", "INTEGER", false, None, 0),
        ],
    },
    TableDefinition {
        name: "flashcard_review",
        columns: &[
            ("review_id", "INTEGER", false, None, 1),
            ("owner_id", "TEXT", true, None, 0),
            ("card_id", "TEXT", true, None, 0),
            ("rating", "TEXT", true, None, 0),
            ("previous_interval_days", "INTEGER", true, None, 0),
            ("next_due", "INTEGER", true, None, 0),
            ("reviewed_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "study_favorite",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("target_kind", "TEXT", true, None, 2),
            ("target_id", "TEXT", true, None, 3),
            ("created_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "user_course_access",
        columns: &[
            ("user_id", "TEXT", true, None, 1),
            ("course_id", "TEXT", true, None, 2),
        ],
    },
    TableDefinition {
        name: "media_bookmark",
        columns: &[
            ("bookmark_id", "TEXT", true, None, 1),
            ("owner_id", "TEXT", true, None, 0),
            ("media_id", "TEXT", true, None, 0),
            ("position_seconds", "INTEGER", true, None, 0),
            ("label", "TEXT", true, None, 0),
            ("created_at", "INTEGER", true, None, 0),
        ],
    },
    TableDefinition {
        name: "player_preference",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("video_speed", "INTEGER", true, Some("100"), 0),
            ("audio_speed", "INTEGER", true, Some("100"), 0),
            ("subtitle_language", "TEXT", true, Some("''"), 0),
            ("updated_at", "INTEGER", true, None, 0),
        ],
    },
];

fn definition_matches(conn: &Connection, definition: &TableDefinition) -> Result<bool> {
    let shape: Option<(String, i64, i64, i64)> = conn
        .query_row(
            "SELECT type, ncol, wr, strict FROM pragma_table_list
         WHERE schema = 'main' AND name = ?1",
            [definition.name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    if shape != Some(("table".to_owned(), definition.columns.len() as i64, 0, 0)) {
        return Ok(false);
    }

    // SQLite exposes CHECK expressions only through sqlite_master. Tokenize the
    // bounded definition instead of using substring matching, so comments and
    // quoted spoof text cannot satisfy the contract.
    let sql: Option<String> = conn.query_row(
        "SELECT CASE WHEN length(CAST(sql AS BLOB)) <= ?2 THEN sql ELSE NULL END
         FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![definition.name, MAX_SCHEMA_SQL_BYTES],
        |row| row.get(0),
    )?;
    let Some(sql) = sql else { return Ok(false) };
    let expected_checks = expected_checks(definition.name)
        .iter()
        .map(|check| tokenize_sql(check))
        .collect::<Option<Vec<_>>>();
    if check_expressions(&sql) != expected_checks {
        return Ok(false);
    }

    let column_limit = definition.columns.len() as i64 + 1;
    let mut statement = conn.prepare(
        "SELECT cid, name, type, \"notnull\", dflt_value, pk, hidden
         FROM pragma_table_xinfo(?1) ORDER BY cid LIMIT ?2",
    )?;
    let columns = statement
        .query_map(params![definition.name, column_limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if columns.len() != definition.columns.len() || columns.iter().any(|column| column.6 != 0) {
        return Ok(false);
    }
    if !columns
        .iter()
        .zip(definition.columns)
        .enumerate()
        .all(|(cid, (actual, expected))| {
            actual.0 == cid as i64
                && actual.1 == expected.0
                && actual.2.eq_ignore_ascii_case(expected.1)
                && actual.3 == expected.2
                && actual.4.as_deref().map(str::trim) == expected.3.map(str::trim)
                && actual.5 == expected.4
        })
    {
        return Ok(false);
    }

    let expected_foreign_keys = expected_foreign_keys(definition.name);
    let mut statement = conn.prepare(
        "SELECT \"from\", \"table\", \"to\", on_update, on_delete
         FROM pragma_foreign_key_list(?1) ORDER BY id, seq LIMIT ?2",
    )?;
    let foreign_keys = statement
        .query_map(
            params![definition.name, expected_foreign_keys.len() as i64 + 1],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(foreign_keys.len() == expected_foreign_keys.len()
        && foreign_keys
            .iter()
            .zip(expected_foreign_keys)
            .all(|(actual, expected)| {
                actual.0 == expected.0
                    && actual.1 == expected.1
                    && actual.2 == expected.2
                    && actual.3.eq_ignore_ascii_case("NO ACTION")
                    && actual.4.eq_ignore_ascii_case(expected.3)
            }))
}

fn expected_checks(table: &str) -> &'static [&'static str] {
    match table {
        "library" => &["available in (0, 1)"],
        "media" => &["availability in ('available', 'missing')"],
        "scan" => &["status in ('completed', 'failed')", "complete in (0, 1)"],
        "study_state" => &["completed in (0, 1)"],
        "focus_session" => &[
            "kind in ('focus', 'short', 'long')",
            "planned_seconds between 1 and 7200",
            "actual_seconds between 0 and 7200",
            "completed in (0, 1)",
            "ended_at >= started_at",
        ],
        "xp_event" => &[
            "source in ('lesson', 'focus', 'weekly_goal', 'mission')",
            "amount > 0",
        ],
        "daily_mission_claim" => &["mission_id in ('lesson', 'focus-two', 'focus-fifty')"],
        "gamification_settings" => &["weekly_goal between 1 and 7"],
        "user" => &["is_admin in (0, 1)", "must_change_credentials in (0, 1)"],
        "study_note" => &[
            "position_seconds >= 0",
            "length(trim(content)) > 0 and length(content) <= 10000",
        ],
        "pdf_annotation" => &[
            "page >= 1 and page <= 100000",
            "kind in ('highlight', 'underline', 'note')",
            "color in ('yellow', 'green', 'blue', 'pink')",
            "length(rects) > 0 and length(rects) <= 20000",
            "length(quote) <= 4000",
            "length(content) <= 10000",
        ],
        "flashcard" => &[
            "length(trim(front)) > 0 and length(front) <= 4000",
            "length(trim(back)) > 0 and length(back) <= 4000",
            "interval_days >= 0 and interval_days <= 365",
            "position_seconds is null or position_seconds >= 0",
        ],
        "flashcard_review" => &[
            "rating in ('again', 'good')",
            "previous_interval_days >= 0 and previous_interval_days <= 365",
        ],
        "study_favorite" => &[
            "target_kind in ('course', 'lesson')",
            "length(trim(target_id)) > 0 and length(target_id) <= 256",
        ],
        "user_course_access" => &["length(trim(course_id)) > 0 and length(course_id) <= 256"],
        "media_bookmark" => &[
            "position_seconds >= 0",
            "length(trim(label)) > 0 and length(label) <= 200",
        ],
        "player_preference" => &[
            "video_speed between 50 and 300",
            "audio_speed between 50 and 300",
            "length(subtitle_language) <= 32",
        ],
        _ => &[],
    }
}

fn expected_foreign_keys(
    table: &str,
) -> &'static [(&'static str, &'static str, &'static str, &'static str)] {
    match table {
        "media" => &[("library_id", "library", "library_id", "NO ACTION")],
        "scan" => &[("library_id", "library", "library_id", "NO ACTION")],
        "study_state" => &[("media_id", "media", "media_id", "NO ACTION")],
        "session" => &[("user_id", "user", "user_id", "NO ACTION")],
        "scan_inaccessible" => &[("scan_id", "scan", "scan_id", "NO ACTION")],
        // `pragma_foreign_key_list` devolve as chaves na ordem inversa da declaração no
        // DDL, e o SQLite não promete outra ordem; o esperado acompanha a ordem
        // observada em vez de depender de `id` como se fosse semântico.
        "media_removal" => &[
            ("media_id", "media", "media_id", "NO ACTION"),
            ("library_id", "library", "library_id", "NO ACTION"),
        ],
        "study_note" => &[("media_id", "media", "media_id", "NO ACTION")],
        "pdf_annotation" => &[("media_id", "media", "media_id", "NO ACTION")],
        "flashcard" => &[("media_id", "media", "media_id", "NO ACTION")],
        "media_bookmark" => &[("media_id", "media", "media_id", "NO ACTION")],
        "user_course_access" => &[("user_id", "user", "user_id", "CASCADE")],
        _ => &[],
    }
}

fn index_matches(
    conn: &Connection,
    table: &str,
    name: Option<&str>,
    unique: bool,
    expected_columns: &[(&str, &str)],
) -> Result<bool> {
    let Some(name) = name else { return Ok(false) };
    let index: Option<(i64, String, i64)> = conn
        .query_row(
            "SELECT \"unique\", origin, partial FROM pragma_index_list(?1) WHERE name = ?2",
            params![table, name],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((is_unique, origin, partial)) = index else {
        return Ok(false);
    };
    if (is_unique != i64::from(unique)) || origin != "c" || partial != 0 {
        return Ok(false);
    }
    let mut statement = conn.prepare(
        "SELECT name, coll FROM pragma_index_xinfo(?1)
         WHERE key = 1 ORDER BY seqno LIMIT ?2",
    )?;
    let columns = statement
        .query_map(params![name, expected_columns.len() as i64 + 1], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.len() == expected_columns.len()
        && columns
            .iter()
            .zip(expected_columns)
            .all(|(actual, expected)| {
                actual.0.as_deref() == Some(expected.0) && actual.1.eq_ignore_ascii_case(expected.1)
            }))
}

fn unique_index_matches(
    conn: &Connection,
    table: &str,
    expected_columns: &[(&str, &str)],
) -> Result<bool> {
    let mut statement = conn.prepare(
        "SELECT name FROM pragma_index_list(?1)
         WHERE \"unique\" = 1 AND origin = 'u' AND partial = 0 LIMIT 2",
    )?;
    let names = statement
        .query_map([table], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if names.len() != 1 {
        return Ok(false);
    }
    let mut statement = conn.prepare(
        "SELECT name, coll FROM pragma_index_xinfo(?1)
         WHERE key = 1 ORDER BY seqno LIMIT ?2",
    )?;
    let columns = statement
        .query_map(
            params![names[0], expected_columns.len() as i64 + 1],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.len() == expected_columns.len()
        && columns
            .iter()
            .zip(expected_columns)
            .all(|(actual, expected)| {
                actual.0.as_deref() == Some(expected.0) && actual.1.eq_ignore_ascii_case(expected.1)
            }))
}

fn check_expressions(sql: &str) -> Option<Vec<Vec<String>>> {
    let tokens = tokenize_sql(sql)?;
    let mut checks = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index] == "check" {
            if tokens.get(index + 1).map(String::as_str) != Some("(") {
                return None;
            }
            let start = index + 2;
            let mut depth = 1;
            let mut end = start;
            while end < tokens.len() {
                match tokens[end].as_str() {
                    "(" => depth += 1,
                    ")" => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                end += 1;
            }
            if depth != 0 {
                return None;
            }
            checks.push(tokens[start..end].to_vec());
            index = end;
        }
        index += 1;
    }
    Some(checks)
}

/// Tokenizes only the bounded schema definition. Comments are discarded and
/// quoted literals remain one token, preventing comment/text spoofing.
fn tokenize_sql(sql: &str) -> Option<Vec<String>> {
    let bytes = sql.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-') {
            index += 2;
            while bytes.get(index).is_some_and(|byte| *byte != b'\n') {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                index += 1;
            }
            if index + 1 >= bytes.len() {
                return None;
            }
            index += 2;
            continue;
        }
        let start = index;
        match bytes[index] {
            b'\'' | b'"' | b'`' => {
                let quote = bytes[index];
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == quote {
                        if bytes.get(index + 1) == Some(&quote) {
                            index += 2;
                            continue;
                        }
                        index += 1;
                        break;
                    }
                    index += 1;
                }
                if index > bytes.len() || bytes.get(index.saturating_sub(1)) != Some(&quote) {
                    return None;
                }
            }
            b'[' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b']' {
                    index += 1;
                }
                if index == bytes.len() {
                    return None;
                }
                index += 1;
            }
            b'(' | b')' | b',' | b';' => index += 1,
            _ => {
                while index < bytes.len()
                    && !bytes[index].is_ascii_whitespace()
                    && !matches!(bytes[index], b'(' | b')' | b',' | b';')
                    && !(bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*'))
                    && !(bytes[index] == b'-' && bytes.get(index + 1) == Some(&b'-'))
                {
                    index += 1;
                }
            }
        }
        let token = if bytes[start] == b'\'' {
            // SQL string literals are case-sensitive values, unlike keywords
            // and identifiers. Preserve their exact UTF-8 bytes for CHECKs.
            String::from_utf8_lossy(&bytes[start..index]).into_owned()
        } else {
            String::from_utf8_lossy(&bytes[start..index]).to_ascii_lowercase()
        };
        tokens.push(token);
    }
    Some(tokens)
}

/// Reconhece a assinatura do banco Go sem criar schema, alterar pragmas persistentes ou
/// consultar tabelas que possam não existir. A checagem acontece antes do bootstrap.
fn is_legacy_go_database(conn: &Connection) -> Result<bool> {
    let has_signature: i64 = conn.query_row(
        "SELECT (
            EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'users')
            AND EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sessions')
            AND EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')
        )",
        [],
        |row| row.get(0),
    )?;
    Ok(has_signature != 0)
}

/// Traduz a falha de leitura da versão do schema, que não é uma migração.
fn read_schema_version(source: rusqlite::Error) -> Error {
    Error::ReadSchemaVersion { source }
}

/// Instante atual em segundos desde a época Unix.
///
/// Um relógio anterior à época Unix é registrado como `0`, que o domínio documenta como
/// "momento desconhecido". É a fonte única das colunas de tempo do domínio (`completed_at`
/// na varredura e `updated_at` no progresso de estudo).
pub(crate) fn now_unix_seconds() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

/// Keep SQLite staging and sidecars private until atomic promotion to the destination.
fn private_staging_file(
    destination: &Path,
) -> Result<(tempfile::TempDir, tempfile::NamedTempFile)> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staging = (|| -> std::io::Result<_> {
        let directory = tempfile::Builder::new()
            .prefix(".learnvault-")
            .tempdir_in(parent)?;
        let file = tempfile::NamedTempFile::new_in(directory.path())?;
        Ok((directory, file))
    })();
    staging.map_err(|source| Error::BackupIo {
        operation: "create private database staging".to_owned(),
        path: destination.to_path_buf(),
        source,
    })
}
