//! Abertura do banco de dados e garantia do schema.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, params};
use rusqlite::config::DbConfig;
use rusqlite_migration::SchemaVersion;

use crate::error::{Error, Result};
use crate::migrations::{MIGRATIONS, SCHEMA_VERSION};

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
        Ok(
            match MIGRATIONS
                .current_version(&self.conn)
                .map_err(read_schema_version)?
            {
                SchemaVersion::NoneSet => 0,
                SchemaVersion::Inside(version) | SchemaVersion::Outside(version) => version.get(),
            },
        )
    }

    /// Verifica se o banco está exatamente no schema suportado e mantém as tabelas essenciais.
    ///
    /// A distinção entre `NoneSet`, versões anteriores e versões posteriores é intencional:
    /// nenhuma delas é uma dependência pronta para o servidor HTTP, mesmo quando a leitura da
    /// versão não produz erro.
    pub fn has_current_schema(&self) -> Result<bool> {
        let version = MIGRATIONS
            .current_version(&self.conn)
            .map_err(read_schema_version)?;
        let SchemaVersion::Inside(version) = version else {
            return Ok(false);
        };
        if version.get() != CURRENT_SCHEMA_VERSION {
            return Ok(false);
        }

        for definition in REQUIRED_TABLES {
            if !definition_matches(&self.conn, definition)? {
                return Ok(false);
            }
        }
        let index_sql: Option<String> = self.conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'session_expires_at_idx'",
            [],
            |row| row.get(0),
        )?;
        Ok(index_sql.is_some_and(|sql| {
            sql.to_ascii_lowercase()
                .split_whitespace()
                .collect::<String>()
                == "createindexsession_expires_at_idxonsession(expires_at)"
        }))
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
        if let SchemaVersion::Outside(found) = MIGRATIONS
            .current_version(&conn)
            .map_err(read_schema_version)?
        {
            return Err(Error::SchemaTooNew {
                found: found.get(),
                known: SCHEMA_VERSION,
            });
        }

        if persistent {
            conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        }

        let mut conn = conn;
        MIGRATIONS.to_latest(&mut conn)?;

        Ok(Self { conn })
    }
}

struct TableDefinition {
    name: &'static str,
    columns: &'static [(&'static str, &'static str, bool, Option<&'static str>, i64)],
    fragments: &'static [&'static str],
}

const REQUIRED_TABLES: &[TableDefinition] = &[
    TableDefinition {
        name: "library",
        columns: &[("library_id", "TEXT", true, None, 1), ("available", "INTEGER", true, None, 0)],
        fragments: &["check (available in (0, 1))"],
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
        fragments: &[
            "references library (library_id) on delete no action",
            "check (availability in ('available', 'missing'))",
            "unique (library_id, relative_path_key)",
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
        fragments: &[
            "references library (library_id) on delete no action",
            "check (status in ('completed', 'failed'))",
            "check (complete in (0, 1))",
        ],
    },
    TableDefinition {
        name: "study_state",
        columns: &[
            ("owner_id", "TEXT", true, None, 1),
            ("media_id", "TEXT", true, None, 0),
            ("position_seconds", "INTEGER", true, Some("0"), 0),
            ("completed", "INTEGER", true, None, 0),
            ("updated_at", "INTEGER", true, None, 0),
        ],
        fragments: &[
            "references media (media_id) on delete no action",
            "check (completed in (0, 1))",
            "primary key (owner_id, media_id)",
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
        fragments: &[
            "unique collate nocase",
            "check (is_admin in (0, 1))",
            "check (must_change_credentials in (0, 1))",
        ],
    },
    TableDefinition {
        name: "session",
        columns: &[
            ("token_hash", "TEXT", true, None, 1),
            ("user_id", "TEXT", true, None, 0),
            ("expires_at", "INTEGER", true, None, 0),
        ],
        fragments: &["references user (user_id) on delete no action"],
    },
    TableDefinition {
        name: "scan_inaccessible",
        columns: &[
            ("scan_id", "INTEGER", true, None, 1),
            ("relative_path", "TEXT", true, None, 2),
        ],
        fragments: &["references scan (scan_id) on delete no action"],
    },
    TableDefinition {
        name: "media_removal",
        columns: &[
            ("library_id", "TEXT", true, None, 1),
            ("media_id", "TEXT", true, None, 2),
            ("removed_at", "INTEGER", true, None, 0),
        ],
        fragments: &[
            "references library (library_id) on delete no action",
            "references media (media_id) on delete no action",
        ],
    },
];

fn definition_matches(conn: &Connection, definition: &TableDefinition) -> Result<bool> {
    let sql: Option<String> = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [definition.name],
        |row| row.get(0),
    )?;
    let Some(sql) = sql else { return Ok(false) };
    let sql = sql.to_ascii_lowercase().split_whitespace().collect::<String>();
    if definition
        .fragments
        .iter()
        .any(|fragment| !sql.contains(&fragment.replace(' ', "")))
    {
        return Ok(false);
    }
    let mut statement = conn.prepare(
        "SELECT name, type, notnull, dflt_value, pk
         FROM pragma_table_info(?1) ORDER BY cid",
    )?;
    let columns = statement
        .query_map(params![definition.name], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.len() == definition.columns.len()
        && columns.iter().zip(definition.columns).all(|(actual, expected)| {
            actual.0 == expected.0
                && actual.1.eq_ignore_ascii_case(expected.1)
                && actual.2 == expected.2
                && actual.3.as_deref().map(str::trim) == expected.3.map(str::trim)
                && actual.4 == expected.4
        }))
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
fn read_schema_version(source: rusqlite_migration::Error) -> Error {
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
