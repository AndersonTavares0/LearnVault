//! Abertura do banco de dados e garantia do schema.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;
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
