//! Erros tipados do domínio.
//!
//! Cada variante nomeia uma falha concreta do domínio. Nenhum caminho de erro usa
//! `panic` ou `unwrap`: falhas são propagadas como valores e traduzidas pelos
//! adaptadores (servidor HTTP, comandos Tauri) para o formato de cada plataforma.
//!
//! As mensagens de `Display` são para log e diagnóstico local. Os adaptadores escolhem o
//! texto apresentado ao cliente e não devem ecoar caminhos do sistema de arquivos.

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

    /// A raiz da biblioteca não existe.
    #[error("a raiz da biblioteca não existe em {path}")]
    LibraryRootMissing {
        /// Caminho informado como raiz da biblioteca.
        path: PathBuf,
    },

    /// A raiz da biblioteca existe, mas não é um diretório.
    #[error("a raiz da biblioteca em {path} não é um diretório")]
    LibraryRootNotDirectory {
        /// Caminho informado como raiz da biblioteca.
        path: PathBuf,
    },

    /// A raiz da biblioteca existe, mas não pôde ser lida.
    #[error("não foi possível ler a raiz da biblioteca em {path}")]
    LibraryRootUnreadable {
        /// Caminho informado como raiz da biblioteca.
        path: PathBuf,
        /// Causa original reportada pelo sistema de arquivos.
        #[source]
        source: std::io::Error,
    },

    /// Um caminho relativo da biblioteca é inválido ou tentaria escapar da raiz.
    #[error("caminho relativo inválido na biblioteca: {path}")]
    InvalidRelativePath {
        /// Caminho recusado.
        path: String,
    },
}

/// Resultado das operações do domínio.
pub type Result<T> = std::result::Result<T, Error>;
