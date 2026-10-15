//! Erros tipados do domínio.
//!
//! Cada variante nomeia uma falha concreta do domínio. Nenhum caminho de erro usa
//! `panic` ou `unwrap`: falhas são propagadas como valores e traduzidas pelos
//! adaptadores (servidor HTTP, comandos Tauri) para o formato de cada plataforma.

use std::path::PathBuf;

/// Erro do domínio do LearnVault.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// O arquivo do banco de dados não pôde ser aberto ou criado.
    #[error("não foi possível abrir o banco de dados em {path}")]
    OpenDatabase {
        /// Caminho do arquivo do banco de dados.
        path: PathBuf,
        /// Causa original reportada pelo SQLite.
        #[source]
        source: rusqlite::Error,
    },

    /// A versão registrada no banco não pôde ser lida.
    ///
    /// É uma leitura, não uma migração: separar as duas variantes evita reportar uma
    /// migração que nunca chegou a ser tentada.
    #[error("não foi possível ler a versão do schema do banco de dados")]
    ReadSchemaVersion {
        /// Causa original reportada pelo SQLite.
        #[source]
        source: rusqlite_migration::Error,
    },

    /// A cadeia de migrações não pôde ser aplicada.
    #[error("não foi possível aplicar as migrações do banco de dados")]
    Migrations(#[from] rusqlite_migration::Error),

    /// O banco foi criado por uma versão mais nova deste programa.
    ///
    /// O schema nunca é rebaixado automaticamente: prosseguir poderia corromper dados
    /// gravados por uma versão que este binário não conhece.
    #[error(
        "o schema do banco está na versão {found}, acima da versão {known} conhecida por este programa"
    )]
    SchemaTooNew {
        /// Versão gravada no banco de dados.
        found: usize,
        /// Versão conhecida por este programa.
        known: usize,
    },

    /// Falha de SQL que não se enquadra nas variantes anteriores.
    #[error("erro de banco de dados")]
    Database(#[from] rusqlite::Error),
}

/// Resultado das operações do domínio.
pub type Result<T> = std::result::Result<T, Error>;
