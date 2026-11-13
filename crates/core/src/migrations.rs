//! Cadeia única de migrações do banco SQLite.
//!
//! Toda a evolução do schema acontece aqui, sobre um único arquivo SQLite. Versões já
//! publicadas nunca são editadas: uma mudança de schema acrescenta um passo novo. O
//! domínio não escreve DDL em nenhum outro lugar.

use rusqlite::Connection;

/// Versão do schema conhecida por este programa.
///
/// É sempre o tamanho de [`MIGRATIONS`]: um banco acima dela foi gravado por uma versão
/// mais nova e é recusado com [`crate::Error::SchemaTooNew`].
pub(crate) const SCHEMA_VERSION: usize = MIGRATIONS.len();

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

/// Versão 9: favoritos de cursos e aulas por dono.
///
/// Permite ao aluno marcar cursos (`course_id`) e aulas (`media_id`) como favoritos
/// para filtragem rápida na biblioteca e no índice do curso.
const V9: &str = "
CREATE TABLE study_favorite (
    owner_id TEXT NOT NULL,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('course', 'lesson')),
    target_id TEXT NOT NULL CHECK (length(trim(target_id)) > 0 AND length(target_id) <= 256),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, target_kind, target_id)
);

CREATE INDEX study_favorite_owner_kind_idx ON study_favorite (owner_id, target_kind, created_at);
";

/// Versão 10: permissões de acesso a cursos por usuário local.
///
/// Quando não há linhas para `user_id` em `user_course_access`, o usuário possui acesso
/// irrestrito a todos os cursos. Quando há linhas, apenas os `course_id` listados são
/// autorizados (usando a sentinela `__none__` quando restrito a zero cursos).
const V10: &str = "
CREATE TABLE user_course_access (
    user_id TEXT NOT NULL REFERENCES user (user_id) ON DELETE CASCADE,
    course_id TEXT NOT NULL CHECK (length(trim(course_id)) > 0 AND length(course_id) <= 256),
    PRIMARY KEY (user_id, course_id)
);
";

/// Versão 11: anotações do aluno sobre páginas de PDF.
///
/// O arquivo do curso nunca é alterado: marca-textos, sublinhados e notas ficam aqui, presos
/// a uma mídia e a uma página. `rects` guarda os retângulos como texto `x,y,largura,altura`
/// separados por `;`, em frações da página (0 a 1), para sobreviver a qualquer zoom.
/// A regra de que uma nota exige texto é validada no domínio, não no banco.
const V11: &str = "
CREATE TABLE pdf_annotation (
    annotation_id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    media_id TEXT NOT NULL REFERENCES media (media_id) ON DELETE NO ACTION,
    page INTEGER NOT NULL CHECK (page >= 1 AND page <= 100000),
    kind TEXT NOT NULL CHECK (kind IN ('highlight', 'underline', 'note')),
    color TEXT NOT NULL CHECK (color IN ('yellow', 'green', 'blue', 'pink')),
    rects TEXT NOT NULL CHECK (length(rects) > 0 AND length(rects) <= 20000),
    quote TEXT NOT NULL CHECK (length(quote) <= 4000),
    content TEXT NOT NULL CHECK (length(content) <= 10000),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX pdf_annotation_owner_media_idx ON pdf_annotation (owner_id, media_id, page);
";

/// Versão 12: marcadores de tempo por aula e por dono.
///
/// Um marcador prende um rótulo curto a um instante da mídia. A linha é do aluno, não da
/// biblioteca: o arquivo original nunca é escrito. O índice cobre a consulta do player, que
/// lista os marcadores de uma mídia em ordem cronológica.
const V12: &str = "
CREATE TABLE media_bookmark (
    bookmark_id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    media_id TEXT NOT NULL REFERENCES media (media_id) ON DELETE NO ACTION,
    position_seconds INTEGER NOT NULL CHECK (position_seconds >= 0),
    label TEXT NOT NULL CHECK (length(trim(label)) > 0 AND length(label) <= 200),
    created_at INTEGER NOT NULL
);

CREATE INDEX media_bookmark_owner_media_idx
    ON media_bookmark (owner_id, media_id, position_seconds, created_at);
";

/// Versão 13: índice de busca global em FTS5.
///
/// `study_note_fts` and `media_fts` are external-content indexes over the tables that own
/// the text, so the content is not duplicated. Triggers keep both indexes correct on every
/// insert, update and delete; `rebuild` backfills the rows that already existed before this
/// migration. `course_fts` is standalone because registered course titles live in the
/// server's configuration file, not in the database; the server refreshes it from its
/// current course registry. The tokenizer removes diacritics so "programacao" matches
/// "Programação". Note rows stay private through the `study_note.owner_id` join.
const V13: &str = "
CREATE VIRTUAL TABLE study_note_fts USING fts5(
    content,
    content = 'study_note',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER study_note_fts_after_insert
AFTER INSERT ON study_note
BEGIN
    INSERT INTO study_note_fts (rowid, content) VALUES (NEW.rowid, NEW.content);
END;

CREATE TRIGGER study_note_fts_after_update
AFTER UPDATE OF content ON study_note
BEGIN
    INSERT INTO study_note_fts (study_note_fts, rowid, content)
    VALUES ('delete', OLD.rowid, OLD.content);
    INSERT INTO study_note_fts (rowid, content) VALUES (NEW.rowid, NEW.content);
END;

CREATE TRIGGER study_note_fts_after_delete
AFTER DELETE ON study_note
BEGIN
    INSERT INTO study_note_fts (study_note_fts, rowid, content)
    VALUES ('delete', OLD.rowid, OLD.content);
END;

INSERT INTO study_note_fts (study_note_fts) VALUES ('rebuild');

CREATE VIRTUAL TABLE media_fts USING fts5(
    relative_path,
    content = 'media',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER media_fts_after_insert
AFTER INSERT ON media
BEGIN
    INSERT INTO media_fts (rowid, relative_path) VALUES (NEW.rowid, NEW.relative_path);
END;

CREATE TRIGGER media_fts_after_update
AFTER UPDATE OF relative_path ON media
BEGIN
    INSERT INTO media_fts (media_fts, rowid, relative_path)
    VALUES ('delete', OLD.rowid, OLD.relative_path);
    INSERT INTO media_fts (rowid, relative_path) VALUES (NEW.rowid, NEW.relative_path);
END;

CREATE TRIGGER media_fts_after_delete
AFTER DELETE ON media
BEGIN
    INSERT INTO media_fts (media_fts, rowid, relative_path)
    VALUES ('delete', OLD.rowid, OLD.relative_path);
END;

INSERT INTO media_fts (media_fts) VALUES ('rebuild');

CREATE VIRTUAL TABLE course_fts USING fts5(
    course_id UNINDEXED,
    title,
    tokenize = 'unicode61 remove_diacritics 2'
);
";

/// Versão 14: vínculo do cartão com um instante da aula e histórico de revisões.
///
/// O cartão continua preso à mídia (a aula); `position_seconds` registra o momento do player
/// em que ele foi criado, e não altera o agendamento. `flashcard_review` é um log somente de
/// acréscimo: cada revisão guarda a avaliação, o intervalo anterior e o próximo vencimento,
/// para que o histórico não dependa do estado atual do cartão. Sem chave estrangeira para
/// `flashcard`, como as demais tabelas de eventos: excluir um cartão não apaga o passado nem
/// falha por causa dele.
const V14: &str = "
ALTER TABLE flashcard ADD COLUMN position_seconds INTEGER
    CHECK (position_seconds IS NULL OR position_seconds >= 0);

CREATE TABLE flashcard_review (
    review_id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL,
    card_id TEXT NOT NULL,
    rating TEXT NOT NULL CHECK (rating IN ('again', 'good')),
    previous_interval_days INTEGER NOT NULL CHECK (previous_interval_days >= 0 AND previous_interval_days <= 365),
    next_due INTEGER NOT NULL,
    reviewed_at INTEGER NOT NULL
);

CREATE INDEX flashcard_review_owner_card_idx
    ON flashcard_review (owner_id, card_id, reviewed_at);
";

/// Cadeia de migrações aplicada por [`crate::db::Database`]; o índice `n` leva o banco da
/// versão `n` para `n + 1`.
pub(crate) const MIGRATIONS: &[&str] =
    &[V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14];

/// Versão do schema gravada em `PRAGMA user_version` (0 quando nunca migrado).
///
/// O mesmo pragma usado pela antiga dependência `rusqlite_migration`, então bancos
/// existentes continuam na versão que já tinham.
pub(crate) fn schema_version(conn: &Connection) -> rusqlite::Result<usize> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    usize::try_from(version).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, version))
}

/// Aplica as migrações pendentes numa única transação: ou todas entram, ou nenhuma.
pub(crate) fn migrate_to_latest(conn: &mut Connection) -> rusqlite::Result<()> {
    let current = schema_version(conn)?;
    if current >= SCHEMA_VERSION {
        return Ok(());
    }
    let tx = conn.transaction()?;
    for step in &MIGRATIONS[current..] {
        tx.execute_batch(step)?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION as i64)?;
    tx.commit()
}
