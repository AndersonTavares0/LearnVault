//! Abertura do banco de dados e garantia do schema.

use std::path::{Path, PathBuf};

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
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, true)?;

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

/// Traduz a falha de leitura da versão do schema, que não é uma migração.
fn read_schema_version(source: rusqlite_migration::Error) -> Error {
    Error::ReadSchemaVersion { source }
}
