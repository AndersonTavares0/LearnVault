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
pub(crate) const SCHEMA_VERSION: usize = 8;

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

/// Versão 3: remoção explícita de mídia.
///
/// A migração é aditiva e não toca as versões anteriores. A presença do par
/// `(library_id, media_id)` nesta tabela é o fato da remoção: a linha de `media` e o
/// `study_state` associado permanecem intactos, porque remover do catálogo não é apagar
/// dados. Por isso a tabela é separada em vez de ser um novo valor no `CHECK` de
/// `media.availability`, que exigiria reconstruir `media` inteira para alterar a
/// restrição.
///
/// A chave primária composta é a chave natural da operação: ela garante a idempotência
/// (não existe remoção duplicada), delimita o escopo por biblioteca e serve de índice
/// para a consulta do catálogo, que filtra por `library_id`.
///
/// As duas chaves estrangeiras garantem que a biblioteca e a mídia existem. O vínculo
/// entre elas é garantido por quem escreve: a inserção seleciona o par direto de
/// `media`, de modo que o `library_id` gravado é o da própria mídia. Uma chave
/// estrangeira composta seria a alternativa, mas exigiria declarar
/// `UNIQUE (library_id, media_id)` em `media`, o que obriga a reconstruir a tabela.
const V3: &str = "
-- Mídia removida explicitamente do catálogo. Nenhuma varredura cria ou apaga esta linha.
CREATE TABLE media_removal (
    library_id TEXT NOT NULL REFERENCES library (library_id) ON DELETE NO ACTION,
    media_id TEXT NOT NULL REFERENCES media (media_id) ON DELETE NO ACTION,
    removed_at INTEGER NOT NULL,
    PRIMARY KEY (library_id, media_id)
);
";

const V4: &str = "
-- Usuários existentes já tinham credenciais; somente o bootstrap temporário define 1.
ALTER TABLE user ADD COLUMN must_change_credentials INTEGER NOT NULL DEFAULT 0 CHECK (must_change_credentials IN (0, 1));
";

// `study_state` guarda só a última interação por aula, então não serve para
// reconstruir ofensiva. `study_day` agrega um registro por dono e dia local.
const V5: &str = "
CREATE TABLE study_day (
    owner_id TEXT NOT NULL,
    day TEXT NOT NULL,
    last_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, day)
);

INSERT INTO study_day (owner_id, day, last_at)
SELECT owner_id, date(updated_at, 'unixepoch', 'localtime'), MAX(updated_at)
FROM study_state
GROUP BY owner_id, date(updated_at, 'unixepoch', 'localtime');

CREATE TRIGGER study_state_activity_after_insert
AFTER INSERT ON study_state
BEGIN
    INSERT INTO study_day (owner_id, day, last_at)
    VALUES (NEW.owner_id, date(NEW.updated_at, 'unixepoch', 'localtime'), NEW.updated_at)
    ON CONFLICT(owner_id, day) DO UPDATE SET last_at = MAX(study_day.last_at, excluded.last_at);
END;

CREATE TRIGGER study_state_activity_after_update
AFTER UPDATE OF updated_at ON study_state
WHEN NEW.updated_at > OLD.updated_at
BEGIN
    INSERT INTO study_day (owner_id, day, last_at)
    VALUES (NEW.owner_id, date(NEW.updated_at, 'unixepoch', 'localtime'), NEW.updated_at)
    ON CONFLICT(owner_id, day) DO UPDATE SET last_at = MAX(study_day.last_at, excluded.last_at);
END;
";

// Sessões e recompensas são fatos imutáveis e idempotentes; `study_state` continua sendo
// somente o estado atual de retomada/conclusão.
const V6: &str = "
CREATE TABLE focus_session (
    owner_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('focus', 'short', 'long')),
    planned_seconds INTEGER NOT NULL CHECK (planned_seconds BETWEEN 1 AND 7200),
    actual_seconds INTEGER NOT NULL CHECK (actual_seconds BETWEEN 0 AND 7200),
    completed INTEGER NOT NULL CHECK (completed IN (0, 1)),
    started_at INTEGER NOT NULL,
    ended_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, session_id),
    CHECK (ended_at >= started_at)
);

CREATE INDEX focus_session_owner_ended_idx ON focus_session (owner_id, ended_at);

CREATE TABLE lesson_completion_event (
    event_id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL,
    media_id TEXT NOT NULL,
    completed_at INTEGER NOT NULL
);

CREATE INDEX lesson_completion_owner_time_idx
    ON lesson_completion_event (owner_id, completed_at);

CREATE TABLE xp_event (
    owner_id TEXT NOT NULL,
    event_key TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('lesson', 'focus', 'weekly_goal', 'mission')),
    source_id TEXT NOT NULL,
    amount INTEGER NOT NULL CHECK (amount > 0),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, event_key)
);

CREATE TABLE daily_mission_claim (
    owner_id TEXT NOT NULL,
    day TEXT NOT NULL,
    mission_id TEXT NOT NULL CHECK (mission_id IN ('lesson', 'focus-two', 'focus-fifty')),
    claimed_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, day, mission_id)
);

CREATE TABLE gamification_settings (
    owner_id TEXT PRIMARY KEY NOT NULL,
    weekly_goal INTEGER NOT NULL DEFAULT 3 CHECK (weekly_goal BETWEEN 1 AND 7)
);

-- Conclusão de uma aula rende XP uma única vez, mas cada nova conclusão é mantida para
-- progresso diário das missões. A migração inicial é uma aproximação best-effort.
INSERT INTO lesson_completion_event (owner_id, media_id, completed_at)
SELECT owner_id, media_id, updated_at FROM study_state WHERE completed = 1;

INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
SELECT owner_id, 'lesson:' || media_id, 'lesson', media_id, 40, updated_at
FROM study_state WHERE completed = 1;

CREATE TRIGGER lesson_completion_after_insert
AFTER INSERT ON study_state WHEN NEW.completed = 1
BEGIN
    INSERT INTO lesson_completion_event (owner_id, media_id, completed_at)
    VALUES (NEW.owner_id, NEW.media_id, NEW.updated_at);
    INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    SELECT NEW.owner_id, 'lesson:' || NEW.media_id, 'lesson', NEW.media_id, 40, NEW.updated_at
    WHERE NOT EXISTS (SELECT 1 FROM xp_event WHERE owner_id = NEW.owner_id AND event_key = 'lesson:' || NEW.media_id);
END;

CREATE TRIGGER lesson_completion_after_update
AFTER UPDATE OF completed ON study_state
WHEN NEW.completed = 1 AND OLD.completed = 0
BEGIN
    INSERT INTO lesson_completion_event (owner_id, media_id, completed_at)
    VALUES (NEW.owner_id, NEW.media_id, NEW.updated_at);
    INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    SELECT NEW.owner_id, 'lesson:' || NEW.media_id, 'lesson', NEW.media_id, 40, NEW.updated_at
    WHERE NOT EXISTS (SELECT 1 FROM xp_event WHERE owner_id = NEW.owner_id AND event_key = 'lesson:' || NEW.media_id);
END;

-- O trigger duplica o upsert de study_day da V5 de propósito: assim a regra semanal
-- observa a atividade da mesma mutação, sem depender da ordem de execução dos triggers.
CREATE TRIGGER weekly_goal_after_study_insert
AFTER INSERT ON study_state
BEGIN
    INSERT INTO study_day (owner_id, day, last_at)
    VALUES (NEW.owner_id, date(NEW.updated_at, 'unixepoch', 'localtime'), NEW.updated_at)
    ON CONFLICT(owner_id, day) DO UPDATE SET last_at = MAX(study_day.last_at, excluded.last_at);
    INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    SELECT NEW.owner_id,
           'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           'weekly_goal',
           date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           50,
           NEW.updated_at
    FROM (SELECT date(NEW.updated_at, 'unixepoch', 'localtime') AS day) d
    WHERE (SELECT COUNT(*) FROM study_day s
           WHERE s.owner_id = NEW.owner_id
             AND s.day BETWEEN date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days')
                           AND date(date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'), '+6 days'))
          >= COALESCE((SELECT weekly_goal FROM gamification_settings WHERE owner_id = NEW.owner_id), 3)
      AND NOT EXISTS (SELECT 1 FROM xp_event e WHERE e.owner_id = NEW.owner_id
                      AND e.event_key = 'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'));
END;

CREATE TRIGGER weekly_goal_after_study_update
AFTER UPDATE OF updated_at ON study_state
WHEN NEW.updated_at > OLD.updated_at
BEGIN
    INSERT INTO study_day (owner_id, day, last_at)
    VALUES (NEW.owner_id, date(NEW.updated_at, 'unixepoch', 'localtime'), NEW.updated_at)
    ON CONFLICT(owner_id, day) DO UPDATE SET last_at = MAX(study_day.last_at, excluded.last_at);
    INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    SELECT NEW.owner_id,
           'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           'weekly_goal',
           date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           50,
           NEW.updated_at
    FROM (SELECT date(NEW.updated_at, 'unixepoch', 'localtime') AS day) d
    WHERE (SELECT COUNT(*) FROM study_day s
           WHERE s.owner_id = NEW.owner_id
             AND s.day BETWEEN date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days')
                           AND date(date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'), '+6 days'))
          >= COALESCE((SELECT weekly_goal FROM gamification_settings WHERE owner_id = NEW.owner_id), 3)
      AND NOT EXISTS (SELECT 1 FROM xp_event e WHERE e.owner_id = NEW.owner_id
                      AND e.event_key = 'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'));
END;

CREATE TRIGGER focus_session_rewards_after_insert
AFTER INSERT ON focus_session
WHEN NEW.kind = 'focus' AND NEW.completed = 1
BEGIN
    INSERT INTO study_day (owner_id, day, last_at)
    VALUES (NEW.owner_id, date(NEW.ended_at, 'unixepoch', 'localtime'), NEW.ended_at)
    ON CONFLICT(owner_id, day) DO UPDATE SET last_at = MAX(study_day.last_at, excluded.last_at);
    INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    VALUES (NEW.owner_id, 'focus:' || NEW.session_id, 'focus', NEW.session_id, 20, NEW.ended_at);
    INSERT OR IGNORE INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    SELECT NEW.owner_id,
           'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           'weekly_goal',
           date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           50,
           NEW.ended_at
    FROM (SELECT date(NEW.ended_at, 'unixepoch', 'localtime') AS day) d
    WHERE (SELECT COUNT(*) FROM study_day s
           WHERE s.owner_id = NEW.owner_id
             AND s.day BETWEEN date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days')
                           AND date(date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'), '+6 days'))
          >= COALESCE((SELECT weekly_goal FROM gamification_settings WHERE owner_id = NEW.owner_id), 3)
      AND NOT EXISTS (SELECT 1 FROM xp_event e WHERE e.owner_id = NEW.owner_id
                      AND e.event_key = 'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'));
END;

CREATE TRIGGER weekly_goal_preference_after_insert
AFTER INSERT ON gamification_settings
BEGIN
    INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    SELECT NEW.owner_id,
           'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           'weekly_goal',
           date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           50,
           unixepoch('now')
    FROM (SELECT date('now', 'localtime') AS day) d
    WHERE (SELECT COUNT(*) FROM study_day s
           WHERE s.owner_id = NEW.owner_id
             AND s.day BETWEEN date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days')
                           AND date(date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'), '+6 days'))
          >= NEW.weekly_goal
      AND NOT EXISTS (SELECT 1 FROM xp_event e WHERE e.owner_id = NEW.owner_id
                      AND e.event_key = 'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'));
END;

CREATE TRIGGER weekly_goal_preference_after_update
AFTER UPDATE OF weekly_goal ON gamification_settings
WHEN NEW.weekly_goal <> OLD.weekly_goal
BEGIN
    INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    SELECT NEW.owner_id,
           'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           'weekly_goal',
           date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'),
           50,
           unixepoch('now')
    FROM (SELECT date('now', 'localtime') AS day) d
    WHERE (SELECT COUNT(*) FROM study_day s
           WHERE s.owner_id = NEW.owner_id
             AND s.day BETWEEN date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days')
                           AND date(date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'), '+6 days'))
          >= NEW.weekly_goal
      AND NOT EXISTS (SELECT 1 FROM xp_event e WHERE e.owner_id = NEW.owner_id
                      AND e.event_key = 'weekly:' || date(d.day, '-' || ((CAST(strftime('%w', d.day) AS INTEGER) + 6) % 7) || ' days'));
END;

CREATE TRIGGER mission_claim_rewards_after_insert
AFTER INSERT ON daily_mission_claim
BEGIN
    INSERT INTO xp_event (owner_id, event_key, source, source_id, amount, created_at)
    SELECT NEW.owner_id,
           'mission:' || NEW.day || ':' || NEW.mission_id,
           'mission',
           NEW.day || ':' || NEW.mission_id,
           30,
           NEW.claimed_at
    WHERE NOT EXISTS (SELECT 1 FROM xp_event e WHERE e.owner_id = NEW.owner_id
                      AND e.event_key = 'mission:' || NEW.day || ':' || NEW.mission_id);
END;
";

/// Versão 7: anotações de estudo na linha do tempo das aulas.
///
/// Permite ao aluno registrar anotações e marcadores temporais vinculados a uma mídia
/// durante o estudo, ordenáveis pela posição do player em segundos.
const V7: &str = "
CREATE TABLE study_note (
    note_id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    media_id TEXT NOT NULL REFERENCES media (media_id) ON DELETE NO ACTION,
    position_seconds INTEGER NOT NULL CHECK (position_seconds >= 0),
    content TEXT NOT NULL CHECK (length(trim(content)) > 0 AND length(content) <= 10000),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX study_note_owner_media_idx ON study_note (owner_id, media_id, position_seconds);
";

/// Versão 8: cartões pessoais de estudo com repetição espaçada.
///
/// Permite ao aluno criar cartões de memorização (flashcards) com frente e verso vinculados
/// a uma aula, com agendamento de revisão espaçada (next_due e interval_days).
const V8: &str = "
CREATE TABLE flashcard (
    card_id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    media_id TEXT NOT NULL REFERENCES media (media_id) ON DELETE NO ACTION,
    front TEXT NOT NULL CHECK (length(trim(front)) > 0 AND length(front) <= 4000),
    back TEXT NOT NULL CHECK (length(trim(back)) > 0 AND length(back) <= 4000),
    interval_days INTEGER NOT NULL CHECK (interval_days >= 0 AND interval_days <= 365),
    next_due INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX flashcard_owner_media_due_idx ON flashcard (owner_id, media_id, next_due);
";

const MIGRATIONS_SLICE: &[M<'static>] = &[
    M::up(V1),
    M::up(V2),
    M::up(V3),
    M::up(V4),
    M::up(V5),
    M::up(V6),
    M::up(V7),
    M::up(V8),
];

/// Cadeia de migrações aplicada por [`crate::db::Database`].
pub(crate) const MIGRATIONS: Migrations<'static> = Migrations::from_slice(MIGRATIONS_SLICE);
