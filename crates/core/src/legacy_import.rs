//! Importação explícita e limitada do banco SQLite do serviço Go.
//!
//! O importador nunca abre a origem para escrita, faz o backup antes de criar a saída,
//! grava a saída em um arquivo temporário e só o promove depois de validar integridade,
//! chaves estrangeiras e contagens. A raiz física da biblioteca não participa da operação.
//! A entrada aceita somente o schema Go v8 completo. Tombstones órfãos e tombstones com
//! `is_media` diferente de 1 são recusados: o schema Rust exige uma mídia real e não possui
//! uma coluna equivalente, portanto inventá-la ou descartá-la perderia informação.

use std::{
    collections::BTreeMap,
    fs,
    fs::OpenOptions,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use rusqlite::{
    Connection, OpenFlags, TransactionBehavior,
    backup::{Backup, StepResult},
};
use uuid::Uuid;

use crate::{
    Database, Error, Result,
    paths::{KeyMode, relative_path_key},
};

/// Entradas explícitas da conversão de um banco Go para um banco Rust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyImportOptions {
    /// Banco SQLite Go, aberto somente para leitura.
    pub input: PathBuf,
    /// Caminho final do banco Rust.
    pub output: PathBuf,
    /// Cópia de segurança da origem, criada antes da importação.
    pub backup: PathBuf,
    /// Identidade da biblioteca a ser criada no banco Rust.
    pub library_id: String,
}

/// Quantidades verificadas e promovidas pela importação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyImportReport {
    /// Usuários importados.
    pub users: usize,
    /// Sessões importadas.
    pub sessions: usize,
    /// Arquivos de catálogo importados como mídia.
    pub media: usize,
    /// Estados de estudo importados.
    pub study_states: usize,
    /// Remoções explícitas importadas.
    pub removals: usize,
}

/// Importa um banco Go sem alterar a origem nem a biblioteca física.
pub fn import_go_database(options: LegacyImportOptions) -> Result<LegacyImportReport> {
    validate_paths(&options)?;

    let source = Connection::open_with_flags(&options.input, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|source| Error::LegacyImportInvalidValue {
            field: "input".to_owned(),
            value: source.to_string(),
        })?;
    validate_source(&source)?;
    validate_go_schema(&source)?;
    backup_database(&source, &options.backup)?;
    drop(source);

    // Import exclusively from the completed backup. The live Go database may continue
    // changing after the backup completes; using it here would make the output differ
    // from the backup that was just announced.
    let snapshot = Connection::open_with_flags(&options.backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|source| Error::LegacyImportInvalidValue {
        field: "backup".to_owned(),
        value: source.to_string(),
    })?;
    validate_source(&snapshot)?;
    validate_go_schema(&snapshot)?;

    let temporary = temporary_path(&options.output);
    let result = import_into_temporary(&snapshot, &temporary, &options.library_id);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    let report = result?;
    // hard_link is an atomic create-new operation on both supported platforms. Unlike
    // rename, it can never replace a destination created after validate_paths.
    if let Err(source) = fs::hard_link(&temporary, &options.output) {
        let _ = fs::remove_file(&temporary);
        return Err(Error::LegacyImportIo {
            operation: "promover a saída".to_owned(),
            path: options.output.clone(),
            source,
        });
    }
    fs::remove_file(&temporary).map_err(|source| Error::LegacyImportIo {
        operation: "remover o temporário promovido".to_owned(),
        path: temporary,
        source,
    })?;
    Ok(report)
}

fn validate_paths(options: &LegacyImportOptions) -> Result<()> {
    if options.library_id.trim().is_empty() {
        return Err(Error::LegacyImportInvalidValue {
            field: "library_id".to_owned(),
            value: options.library_id.clone(),
        });
    }
    let input = fs::canonicalize(&options.input).map_err(|source| Error::LegacyImportIo {
        operation: "ler a origem".to_owned(),
        path: options.input.clone(),
        source,
    })?;
    let output = absolute_target(&options.output)?;
    let backup = absolute_target(&options.backup)?;
    if input == output || input == backup || output == backup {
        return Err(Error::LegacyImportInvalidValue {
            field: "input/output/backup".to_owned(),
            value: "caminhos coincidentes".to_owned(),
        });
    }
    if options.output.exists() || options.backup.exists() {
        return Err(Error::LegacyImportInvalidValue {
            field: "output/backup".to_owned(),
            value: "o destino já existe".to_owned(),
        });
    }
    Ok(())
}

fn absolute_target(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|source| Error::LegacyImportIo {
        operation: "resolver o diretório de destino".to_owned(),
        path: parent.to_owned(),
        source,
    })?;
    Ok(parent.join(
        path.file_name()
            .ok_or_else(|| Error::LegacyImportInvalidPath {
                path: path.display().to_string(),
            })?,
    ))
}

fn backup_database(source: &Connection, backup: &Path) -> Result<()> {
    // fs::copy omits an active WAL. SQLite's online backup API takes a consistent
    // snapshot of the connection, including WAL pages, without writing to source.
    reserve_new_file(backup, "reservar o backup")?;
    let mut destination = match Connection::open(backup) {
        Ok(destination) => destination,
        Err(source) => {
            let _ = fs::remove_file(backup);
            return Err(Error::LegacyImportIo {
                operation: "criar o backup".to_owned(),
                path: backup.to_owned(),
                source: std::io::Error::other(source.to_string()),
            });
        }
    };
    let result = (|| -> Result<()> {
        let backup_job = Backup::new(source, &mut destination)?;
        let mut retries = 0;
        loop {
            match backup_job.step(256)? {
                StepResult::Done => break,
                StepResult::More => continue,
                StepResult::Busy | StepResult::Locked => {
                    retries += 1;
                    if retries > 300 {
                        return Err(invalid(
                            "backup",
                            "a origem permaneceu ocupada por mais de 3 segundos",
                        ));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                _ => return Err(invalid("backup", "resultado desconhecido do backup SQLite")),
            }
        }
        Ok(())
    })();
    if result.is_err() {
        drop(destination);
        let _ = fs::remove_file(backup);
    }
    result
}

fn reserve_new_file(path: &Path, operation: &str) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|source| Error::LegacyImportIo {
            operation: operation.to_owned(),
            path: path.to_owned(),
            source,
        })
}

fn temporary_path(output: &Path) -> PathBuf {
    output.with_extension(format!("tmp-{}", Uuid::now_v7()))
}

fn validate_source(conn: &Connection) -> Result<()> {
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(Error::LegacyImportInvalidValue {
            field: "integrity_check".to_owned(),
            value: integrity,
        });
    }
    let mut fk = conn.prepare("PRAGMA foreign_key_check")?;
    if fk.query([])?.next()?.is_some() {
        return Err(Error::LegacyImportInvalidValue {
            field: "foreign_key_check".to_owned(),
            value: "violação".to_owned(),
        });
    }
    Ok(())
}

fn validate_go_schema(conn: &Connection) -> Result<()> {
    let tables = [
        ("schema_migrations", &["version"] as &[&str]),
        (
            "users",
            &[
                "id",
                "username",
                "password_hash",
                "role",
                "created_at",
                "must_change_credentials",
            ],
        ),
        (
            "sessions",
            &["token_hash", "user_id", "expires_at", "created_at"],
        ),
        ("catalog_categories", &["id", "path", "title"]),
        ("catalog_courses", &["id", "category_id", "path", "title"]),
        ("catalog_lessons", &["id", "module_id", "path", "title"]),
        ("catalog_modules", &["id", "course_id", "path", "title"]),
        (
            "catalog_files",
            &["id", "lesson_id", "path", "name", "kind", "size"],
        ),
        ("catalog_root_files", &["id", "path", "name", "size"]),
        ("catalog_nodes_json", &["id", "value"]),
        (
            "media_removal",
            &["relative_path", "removed_at", "is_media"],
        ),
        (
            "study_state",
            &[
                "user_id",
                "media_path",
                "position_seconds",
                "completed",
                "updated_at",
            ],
        ),
    ];
    for (table, required) in tables {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let actual: std::collections::BTreeSet<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<_>>()?;
        if actual.is_empty() || required.iter().any(|column| !actual.contains(*column)) {
            return Err(invalid(
                "schema",
                format!("Go v8: tabela {table} ou suas colunas estão ausentes"),
            ));
        }
    }
    let versions: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
        stmt.query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    if versions != (1..=8).collect::<Vec<_>>() {
        return Err(invalid(
            "schema_migrations",
            format!("esperado exatamente Go v8 (1..8), encontrado {versions:?}"),
        ));
    }
    Ok(())
}

fn import_into_temporary(
    source: &Connection,
    path: &Path,
    library_id: &str,
) -> Result<LegacyImportReport> {
    let mut db = Database::open(path)?;
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [library_id],
    )?;

    let mut users = BTreeMap::new();
    let mut stmt = source.prepare("SELECT id, username, password_hash, role, created_at, must_change_credentials FROM users ORDER BY id")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;
    let mut user_count = 0;
    for row in rows {
        let (old_id, username, password_hash, role, created_at, must_change_credentials) = row?;
        let is_admin = match role.as_str() {
            "admin" => 1,
            "user" => 0,
            _ => return Err(invalid("users.role", role)),
        };
        let created_at = parse_timestamp(&created_at)
            .filter(|value| *value >= 0)
            .ok_or_else(|| invalid("users.created_at", created_at.clone()))?;
        if !matches!(must_change_credentials, 0 | 1) {
            return Err(invalid(
                "users.must_change_credentials",
                must_change_credentials.to_string(),
            ));
        }
        let new_id = Uuid::now_v7().to_string();
        tx.execute("INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", rusqlite::params![new_id, username, password_hash, is_admin, created_at, must_change_credentials])?;
        users.insert(old_id, new_id);
        user_count += 1;
    }

    let mut sessions = 0;
    let mut stmt = source
        .prepare("SELECT token_hash, user_id, expires_at FROM sessions ORDER BY token_hash")?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (token, old_user, expires) = row?;
        let user = users
            .get(&old_user)
            .ok_or_else(|| orphan("sessions.user_id", old_user.to_string()))?;
        let expires = parse_timestamp(&expires)
            .filter(|value| *value >= 0)
            .ok_or_else(|| invalid("sessions.expires_at", expires.clone()))?;
        tx.execute(
            "INSERT INTO session (token_hash, user_id, expires_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![token, user, expires],
        )?;
        sessions += 1;
    }

    let mut media = BTreeMap::new();
    let mut stmt = source.prepare("SELECT path, kind, size FROM catalog_files ORDER BY id")?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })? {
        let (raw_path, kind, size) = row?;
        if !matches!(kind.as_str(), "media" | "subtitle" | "material") {
            return Err(invalid("catalog_files.kind", kind));
        }
        let path = normalize_path(&raw_path)?;
        let key = path_key(&raw_path)?;
        if media.contains_key(&key) {
            return Err(Error::LegacyImportCollision { path });
        }
        if size < 0 {
            return Err(invalid("catalog_files.size", size.to_string()));
        }
        let id = Uuid::now_v7().to_string();
        tx.execute("INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability) VALUES (?1, ?2, ?3, ?4, ?5, 0, 'available')", rusqlite::params![id, library_id, path, key, size])?;
        media.insert(key, (path, id));
    }

    let mut stmt = source.prepare("SELECT path, size FROM catalog_root_files ORDER BY id")?;
    for row in stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (raw_path, size) = row?;
        let path = normalize_path(&raw_path)?;
        let key = path_key(&raw_path)?;
        if media.contains_key(&key) {
            return Err(Error::LegacyImportCollision { path });
        }
        if size < 0 {
            return Err(invalid("catalog_root_files.size", size.to_string()));
        }
        let id = Uuid::now_v7().to_string();
        tx.execute("INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability) VALUES (?1, ?2, ?3, ?4, ?5, 0, 'available')", rusqlite::params![id, library_id, path, key, size])?;
        media.insert(key, (path, id));
    }
    let media_count = media.len();

    let mut removals = 0;
    // Rust media_removal has a mandatory media FK and no is_media column. An orphan
    // tombstone, or a Go tombstone known not to be media, cannot be represented without
    // inventing a media row; fail closed rather than silently losing or falsifying it.
    let mut stmt = source.prepare(
        "SELECT relative_path, removed_at, is_media FROM media_removal ORDER BY relative_path",
    )?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })? {
        let (raw_path, removed_at, is_media) = row?;
        let path = normalize_path(&raw_path)?;
        let key = path_key(&raw_path)?;
        let (_, id) = media
            .get(&key)
            .ok_or_else(|| orphan("media_removal.relative_path", path.clone()))?;
        if is_media != Some(1) {
            return Err(invalid(
                "media_removal.is_media",
                format!("tombstone não é mídia: {path}"),
            ));
        }
        if removed_at < 0 {
            return Err(invalid("media_removal.removed_at", removed_at.to_string()));
        }
        tx.execute(
            "INSERT INTO media_removal (library_id, media_id, removed_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![library_id, id, removed_at],
        )?;
        removals += 1;
    }

    let mut study_states = 0;
    let mut stmt = source.prepare("SELECT user_id, media_path, position_seconds, completed, updated_at FROM study_state ORDER BY user_id, media_path")?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })? {
        let (old_user, raw_path, position, completed, updated_at) = row?;
        let user = users
            .get(&old_user)
            .ok_or_else(|| orphan("study_state.user_id", old_user.to_string()))?;
        let path = normalize_path(&raw_path)?;
        let key = path_key(&raw_path)?;
        let (_, id) = media
            .get(&key)
            .ok_or_else(|| orphan("study_state.media_path", path.clone()))?;
        if position < 0 || updated_at < 0 || !matches!(completed, 0 | 1) {
            return Err(invalid(
                "study_state",
                format!("{position}/{completed}/{updated_at}"),
            ));
        }
        tx.execute("INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)", rusqlite::params![user, id, position, completed, updated_at])?;
        study_states += 1;
    }
    tx.execute("INSERT INTO scan (library_id, status, completed_at, complete) VALUES (?1, 'completed', strftime('%s','now'), 1)", [library_id])?;
    tx.commit()?;
    validate_output(
        &db,
        user_count,
        sessions,
        media_count,
        study_states,
        removals,
    )?;
    db.connection()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    Ok(LegacyImportReport {
        users: user_count,
        sessions,
        media: media_count,
        study_states,
        removals,
    })
}

fn validate_output(
    db: &Database,
    users: usize,
    sessions: usize,
    media: usize,
    study: usize,
    removals: usize,
) -> Result<()> {
    let conn = db.connection();
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        return Err(invalid("output.integrity_check", integrity));
    }
    let mut fk = conn.prepare("PRAGMA foreign_key_check")?;
    if fk.query([])?.next()?.is_some() {
        return Err(invalid("output.foreign_key_check", "violação"));
    }
    for (table, expected) in [
        ("user", users),
        ("session", sessions),
        ("media", media),
        ("study_state", study),
        ("media_removal", removals),
    ] {
        let actual: i64 =
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
        if usize::try_from(actual).ok() != Some(expected) {
            return Err(invalid("output.count", table));
        }
    }
    Ok(())
}

fn invalid(field: impl Into<String>, value: impl Into<String>) -> Error {
    Error::LegacyImportInvalidValue {
        field: field.into(),
        value: value.into(),
    }
}
fn orphan(relation: impl Into<String>, key: impl Into<String>) -> Error {
    Error::LegacyImportOrphan {
        relation: relation.into(),
        key: key.into(),
    }
}

fn normalize_path(raw: &str) -> Result<String> {
    path_key(raw)?;
    Ok(raw.replace('\\', "/"))
}

fn path_key(raw: &str) -> Result<String> {
    relative_path_key(raw, KeyMode::for_host()).map_err(|_| Error::LegacyImportInvalidPath {
        path: raw.to_owned(),
    })
}

fn parse_timestamp(value: &str) -> Option<i64> {
    if !value.is_ascii() {
        return None;
    }
    let (value, offset_seconds) = if let Some(value) = value.strip_suffix('Z') {
        (value, 0)
    } else if value.len() >= 25 {
        let offset = &value[value.len() - 6..];
        let sign = match offset.as_bytes()[0] {
            b'+' => 1,
            b'-' => -1,
            _ => return None,
        };
        if offset.as_bytes()[3] != b':' {
            return None;
        }
        let hours = offset[1..3].parse::<i64>().ok()?;
        let minutes = offset[4..6].parse::<i64>().ok()?;
        if hours > 23 || minutes > 59 {
            return None;
        }
        (
            &value[..value.len() - 6],
            sign * (hours * 3_600 + minutes * 60),
        )
    } else {
        (value, 0)
    };
    let value = value.strip_suffix('Z').unwrap_or(value);
    let value = match value.as_bytes().get(19) {
        None => value,
        Some(b'.') => {
            let fraction = &value[20..];
            if fraction.is_empty()
                || fraction.len() > 9
                || !fraction.bytes().all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            &value[..19]
        }
        _ => return None,
    };
    if value.len() != 19 || !matches!(value.as_bytes()[10], b' ' | b'T') {
        return None;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    if !digits.iter().all(|&i| value.as_bytes()[i].is_ascii_digit()) {
        return None;
    }
    if value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || value.as_bytes()[13] != b':'
        || value.as_bytes()[16] != b':'
    {
        return None;
    }
    let n = |a, b| value[a..b].parse::<i64>().ok();
    let (year, month, day, hour, minute, second) = (
        n(0, 4)?,
        n(5, 7)?,
        n(8, 10)?,
        n(11, 13)?,
        n(14, 16)?,
        n(17, 19)?,
    );
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(1..=days_in_month(year, month)).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let days = 365 * (year - 1970) + (year - 1969) / 4 - (year - 1901) / 100
        + (year - 1900 + 299) / 400
        + month_days_before(year, month)
        + day
        - 1;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second - offset_seconds)
}

fn month_days_before(year: i64, month: i64) -> i64 {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    days[(month - 1) as usize] + i64::from(leap && month > 2)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
