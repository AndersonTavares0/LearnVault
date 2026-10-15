//! Cadeia única de migrações do banco SQLite.
//!
//! Toda a evolução do schema acontece aqui, sobre um único arquivo SQLite. Versões já
//! publicadas nunca são editadas: uma mudança de schema acrescenta um passo novo. O
//! domínio não escreve DDL em nenhum outro lugar.

use rusqlite_migration::{M, Migrations};

/// Versão do schema conhecida por este programa.
///
/// A recusa de um schema mais novo não depende desta constante — ela vem de
/// `SchemaVersion::Outside` —, mas ela deve acompanhar a cadeia de migrações porque
/// aparece na mensagem de diagnóstico de [`crate::Error::SchemaTooNew`].
pub(crate) const SCHEMA_VERSION: usize = 2;

/// Versão 1: bibliotecas, mídia, varreduras e estado de estudo.
///
/// As tabelas `user` e `session` são declaradas nesta mesma cadeia para manter um único
/// caminho de migração sobre um único banco, mas são usadas apenas pelo servidor; o
/// domínio compartilhado não as acessa.
///
/// Nenhuma chave estrangeira usa exclusão em cascata: remover bibliotecas ou mídia exige
/// ação explícita e preserva os dados associados, então a exclusão física de linhas não
/// faz parte da política do domínio e falha alto em vez de propagar remoções. O
/// `ON DELETE NO ACTION` é o padrão do SQLite, mas é escrito para ficar auditável.
///
/// Todas as colunas de tempo (`mtime`, `completed_at`, `updated_at`, `expires_at`) são
/// epoch Unix em segundos, em inteiros, sem fuso.
const V1: &str = "
-- Bibliotecas cadastradas. A raiz física é configuração da plataforma, não dado do domínio.
CREATE TABLE library (
    library_id TEXT PRIMARY KEY NOT NULL,
    available INTEGER NOT NULL CHECK (available IN (0, 1))
);

-- Mídia conhecida. A identidade é o media_id; o caminho é apenas a localização atual.
CREATE TABLE media (
    media_id TEXT PRIMARY KEY NOT NULL,
    library_id TEXT NOT NULL REFERENCES library (library_id) ON DELETE NO ACTION,
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
    library_id TEXT NOT NULL REFERENCES library (library_id) ON DELETE NO ACTION,
    status TEXT NOT NULL CHECK (status IN ('completed', 'failed')),
    completed_at INTEGER NOT NULL
);

-- Estado de estudo por dono (owner_id opaco) e por mídia.
CREATE TABLE study_state (
    owner_id TEXT NOT NULL,
    media_id TEXT NOT NULL REFERENCES media (media_id) ON DELETE NO ACTION,
    position_seconds INTEGER NOT NULL DEFAULT 0,
    completed INTEGER NOT NULL CHECK (completed IN (0, 1)),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, media_id)
);

-- Contas locais. Usadas apenas pelo servidor. O nome de usuário é comparado sem
-- diferenciar maiúsculas de minúsculas, como na implementação de referência.
CREATE TABLE user (
    user_id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL CHECK (is_admin IN (0, 1)),
    created_at INTEGER NOT NULL
);

-- Sessões autenticadas. Usadas apenas pelo servidor.
CREATE TABLE session (
    token_hash TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES user (user_id) ON DELETE NO ACTION,
    expires_at INTEGER NOT NULL
);

-- O servidor descarta sessões vencidas; a consulta filtra por expiração.
CREATE INDEX session_expires_at_idx ON session (expires_at);
";

/// Versão 2: registro da cobertura de cada varredura.
///
/// A migração é aditiva e não toca a versão 1. `scan.complete` distingue uma varredura
/// completa de uma parcial: `0` significa que alguma área da biblioteca não pôde ser
/// lida, e não que nenhuma ausência possa ser registrada. `scan_inaccessible` guarda os
/// caminhos relativos que não puderam ser verificados, para que uma leitura futura
/// consiga separar os registros preservados por falta de informação daqueles que foram
/// encontrados.
///
/// A coluna nasce com `DEFAULT 1` para que as linhas gravadas pela versão 1 continuem
/// válidas: uma varredura só era registrada quando terminava.
const V2: &str = "
ALTER TABLE scan ADD COLUMN complete INTEGER NOT NULL DEFAULT 1 CHECK (complete IN (0, 1));

-- Áreas que a varredura não conseguiu verificar, em caminhos relativos à raiz.
CREATE TABLE scan_inaccessible (
    scan_id INTEGER NOT NULL REFERENCES scan (scan_id) ON DELETE NO ACTION,
    relative_path TEXT NOT NULL,
    PRIMARY KEY (scan_id, relative_path)
);
";

const MIGRATIONS_SLICE: &[M<'static>] = &[M::up(V1), M::up(V2)];

/// Cadeia de migrações aplicada por [`crate::db::Database`].
pub(crate) const MIGRATIONS: Migrations<'static> = Migrations::from_slice(MIGRATIONS_SLICE);
