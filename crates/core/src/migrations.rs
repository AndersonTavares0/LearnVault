//! Cadeia única de migrações do banco SQLite.
//!
//! Toda a evolução do schema acontece aqui, sobre um único arquivo SQLite. Versões já
//! publicadas nunca são editadas: uma mudança de schema acrescenta um passo novo. O
//! domínio não escreve DDL em nenhum outro lugar.

use rusqlite_migration::{M, Migrations};

/// Versão do schema conhecida por este programa.
pub(crate) const SCHEMA_VERSION: usize = 1;

/// Versão 1: bibliotecas, mídia, varreduras e estado de estudo.
///
/// As tabelas `user` e `session` são declaradas nesta mesma cadeia para manter um único
/// caminho de migração sobre um único banco, mas são usadas apenas pelo servidor; o
/// domínio compartilhado não as acessa.
///
/// Nenhuma chave estrangeira usa exclusão em cascata: remover bibliotecas ou mídia exige
/// ação explícita e preserva os dados associados, então a exclusão física de linhas não
/// faz parte da política do domínio e falha alto em vez de propagar remoções.
const V1: &str = "
-- Bibliotecas cadastradas. A raiz física é configuração da plataforma, não dado do domínio.
CREATE TABLE library (
    library_id TEXT PRIMARY KEY NOT NULL,
    available INTEGER NOT NULL CHECK (available IN (0, 1))
);

-- Mídia conhecida. A identidade é o media_id; o caminho é apenas a localização atual.
CREATE TABLE media (
    media_id TEXT PRIMARY KEY NOT NULL,
    library_id TEXT NOT NULL REFERENCES library (library_id),
    relative_path TEXT NOT NULL,
    relative_path_key TEXT NOT NULL,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    availability TEXT NOT NULL CHECK (availability IN ('available', 'missing')),
    UNIQUE (library_id, relative_path_key)
);

-- Varreduras concluídas, usadas para distinguir arquivo ausente de biblioteca indisponível.
CREATE TABLE scan (
    scan_id INTEGER PRIMARY KEY,
    library_id TEXT NOT NULL REFERENCES library (library_id),
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed')),
    completed_at INTEGER NOT NULL
);

-- Estado de estudo por dono (owner_id opaco) e por mídia.
CREATE TABLE study_state (
    owner_id TEXT NOT NULL,
    media_id TEXT NOT NULL REFERENCES media (media_id),
    position_seconds INTEGER NOT NULL DEFAULT 0,
    completed INTEGER NOT NULL CHECK (completed IN (0, 1)),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, media_id)
);

-- Contas locais. Usadas apenas pelo servidor.
CREATE TABLE user (
    user_id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL CHECK (is_admin IN (0, 1)),
    created_at INTEGER NOT NULL
);

-- Sessões autenticadas. Usadas apenas pelo servidor.
CREATE TABLE session (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES user (user_id),
    expires_at INTEGER NOT NULL
);
";

const MIGRATIONS_SLICE: &[M<'static>] = &[M::up(V1)];

/// Cadeia de migrações aplicada por [`crate::db::Database`].
pub(crate) const MIGRATIONS: Migrations<'static> = Migrations::from_slice(MIGRATIONS_SLICE);
