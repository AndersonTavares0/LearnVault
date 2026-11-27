//! Single migration chain of the SQLite database.
//!
//! All schema evolution happens here, over a single SQLite file. Versions already
//! published are never edited: a schema change adds a new step. The
//! domain writes DDL nowhere else.

use rusqlite::Connection;

/// Schema version known to this program.
///
/// It is always the length of [`MIGRATIONS`]: a database above it was written by a newer
/// version and is refused with [`crate::Error::SchemaTooNew`].
pub(crate) const SCHEMA_VERSION: usize = MIGRATIONS.len();

/// Version 1: libraries, media, scans and study state.
///
/// The `user` and `session` tables are declared in this same chain to keep a single
/// migration path over a single database, but they are used only by the server; the
/// shared domain does not access them.
///
/// No foreign key uses cascading deletes: removing libraries or media requires an
/// explicit action and preserves the associated data, so physical deletion of rows is not
/// part of the domain's policy and fails loudly instead of propagating removals. The
/// `ON DELETE NO ACTION` is SQLite's default, but it is written out to stay auditable.
///
/// All time columns (`mtime`, `completed_at`, `updated_at`, `expires_at`) are
/// Unix epoch in seconds, as integers, with no time zone.
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

/// Version 2: record of each scan's coverage.
///
/// The migration is additive and does not touch version 1. `scan.complete` tells a complete
/// scan from a partial one: `0` means some area of the library could not be
/// read, and not that no absence can be recorded. `scan_inaccessible` keeps the
/// relative paths that could not be verified, so that a later read
/// can separate the records preserved for lack of information from those that were
/// found.
///
/// The column is created with `DEFAULT 1` so that the rows written by version 1 stay
/// valid: a scan was only recorded when it finished.
const V2: &str = "
ALTER TABLE scan ADD COLUMN complete INTEGER NOT NULL DEFAULT 1 CHECK (complete IN (0, 1));

-- Áreas que a varredura não conseguiu verificar, em caminhos relativos à raiz.
CREATE TABLE scan_inaccessible (
    scan_id INTEGER NOT NULL REFERENCES scan (scan_id) ON DELETE NO ACTION,
    relative_path TEXT NOT NULL,
    PRIMARY KEY (scan_id, relative_path)
);
";

/// Version 3: explicit removal of media.
///
/// The migration is additive and does not touch the previous versions. The presence of the pair
/// `(library_id, media_id)` in this table is the fact of the removal: the `media` row and the associated
/// `study_state` stay intact, because removing from the catalog is not deleting
/// data. That is why the table is separate instead of being a new value in the `CHECK` of
/// `media.availability`, which would require rebuilding all of `media` to change the
/// constraint.
///
/// The composite primary key is the natural key of the operation: it guarantees idempotence
/// (no duplicate removal exists), bounds the scope by library and serves as an index
/// for the catalog query, which filters by `library_id`.
///
/// The two foreign keys guarantee that the library and the media exist. The link
/// between them is guaranteed by whoever writes: the insert selects the pair straight from
/// `media`, so that the stored `library_id` is that of the media itself. A composite
/// foreign key would be the alternative, but it would require declaring
/// `UNIQUE (library_id, media_id)` on `media`, which forces rebuilding the table.
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

// `study_state` keeps only the last interaction per lesson, so it cannot be used to
// rebuild a streak. `study_day` aggregates one record per owner and local day.
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

// Sessions and rewards are immutable, idempotent facts; `study_state` remains
// only the current resume/completion state.
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

/// Version 7: study notes on the lessons' timeline.
///
/// Lets the learner record notes and time markers linked to a media
/// while studying, sortable by the player position in seconds.
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

/// Version 8: personal study cards with spaced repetition.
///
/// Lets the learner create memorization cards (flashcards) with a front and back linked
/// to a lesson, with spaced-review scheduling (next_due and interval_days).
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

/// Version 9: favourites of courses and lessons per owner.
///
/// Lets the learner mark courses (`course_id`) and lessons (`media_id`) as favourites
/// for quick filtering in the library and in the course index.
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

/// Version 10: course access permissions per local user.
///
/// When there are no rows for `user_id` in `user_course_access`, the user has unrestricted
/// access to every course. When there are rows, only the listed `course_id`s are
/// authorized (using the `__none__` sentinel when restricted to zero courses).
const V10: &str = "
CREATE TABLE user_course_access (
    user_id TEXT NOT NULL REFERENCES user (user_id) ON DELETE CASCADE,
    course_id TEXT NOT NULL CHECK (length(trim(course_id)) > 0 AND length(course_id) <= 256),
    PRIMARY KEY (user_id, course_id)
);
";

/// Version 11: the learner's annotations on PDF pages.
///
/// The course file is never altered: highlights, underlines and notes live here, tied
/// to a media and a page. `rects` stores the rectangles as `x,y,width,height` text
/// separated by `;`, in fractions of the page (0 to 1), so they survive any zoom.
/// The rule that a note requires text is validated in the domain, not in the database.
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

/// Version 12: time markers per lesson and per owner.
///
/// A marker ties a short label to an instant of the media. The row belongs to the learner, not to the
/// library: the original file is never written. The index covers the player's query, which
/// lists a media's markers in chronological order.
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

/// Version 13: global search index in FTS5.
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

/// Version 14: the card's link to an instant of the lesson and the review history.
///
/// The card stays tied to the media (the lesson); `position_seconds` records the moment in the player
/// when it was created, and does not change the schedule. `flashcard_review` is an append-only
/// log: each review keeps the rating, the previous interval and the next due date,
/// so the history does not depend on the card's current state. No foreign key to
/// `flashcard`, like the other event tables: deleting a card does not erase the past nor
/// fail because of it.
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

/// Version 15: playback preferences per owner.
///
/// Speeds are stored as a whole percentage of normal speed (100 = 1×), with no
/// floating point. `subtitle_language` keeps the last language chosen (for example
/// `pt-BR`); empty means "no preference". The desktop has no accounts and keeps using
/// the webview's local storage.
const V15: &str = "
CREATE TABLE player_preference (
    owner_id TEXT PRIMARY KEY NOT NULL,
    video_speed INTEGER NOT NULL DEFAULT 100 CHECK (video_speed BETWEEN 50 AND 300),
    audio_speed INTEGER NOT NULL DEFAULT 100 CHECK (audio_speed BETWEEN 50 AND 300),
    subtitle_language TEXT NOT NULL DEFAULT '' CHECK (length(subtitle_language) <= 32),
    updated_at INTEGER NOT NULL
);
";

/// Version 16: sampled hash of the media content.
///
/// The lesson's identity is still the `media_id`; the hash makes it possible to recognize the same
/// file when it changes path, without depending on the name. The column is nullable because
/// rows from before the migration and unreadable files have no hash; the comparison of
/// size and mtime still applies in those cases. No course file is altered:
/// the hash is read only from the ends of the file.
const V16: &str = "
ALTER TABLE media ADD COLUMN content_hash TEXT;
";

/// Version 17: sync journal between desktop and server.
///
/// The journal (`sync_operation`) is append-only: each row has a stable, unique `op_id`,
/// and `seq` is the monotonic cursor of the incremental read. `payload` keeps the
/// typed operation as JSON; the row is written by the server after it is applied, in the
/// same transaction, so that a replay of the same `op_id` does not duplicate the effect.
///
/// `sync_outbox` is the desktop's local queue: it survives restarts and keeps the
/// `op_id` across attempts, so resending never duplicates. `sync_state` keeps the
/// local device and the server cursor per owner.
///
/// `study_note.sync_version` identifies the synced version that occupies the
/// main row; the losing version becomes a copy with a deterministic identity, so that
/// two devices converge without losing text. Rows from before the migration keep
/// the column null and are never replaced.
const V17: &str = "
ALTER TABLE study_note ADD COLUMN sync_version TEXT;

CREATE TABLE sync_operation (
    seq INTEGER PRIMARY KEY,
    op_id TEXT NOT NULL UNIQUE,
    owner_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    client_at INTEGER NOT NULL,
    received_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX sync_operation_owner_seq_idx ON sync_operation (owner_id, seq);

CREATE TABLE sync_outbox (
    op_id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    client_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    queued_at INTEGER NOT NULL
);

CREATE INDEX sync_outbox_owner_queued_idx ON sync_outbox (owner_id, queued_at, op_id);

CREATE TABLE sync_state (
    owner_id TEXT PRIMARY KEY NOT NULL,
    device_id TEXT NOT NULL,
    pull_cursor INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);
";

/// Version 18: tombstones of deleted notes and bookmarks.
///
/// Deleting a row is not an append-only effect: a create that is still pending on another
/// device would resurrect it when it arrives. A tombstone (`kind` is `note` or `bookmark`)
/// records the delete instant so an older create is skipped on apply, and it prevents
/// resurrecting deleted rows without keeping the content itself.
const V18: &str = "
CREATE TABLE sync_tombstone (
    owner_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    target_id TEXT NOT NULL,
    deleted_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, kind, target_id)
);
";

/// Version 19: course roots of a source (ADR 0004).
///
/// Additive. A root is a library of its own that owns the part of a source under `prefix`.
/// `media.relative_path` keeps being relative to the source, so reads do not change.
/// `prefix_key` is the comparison key of `prefix` for the host's [`crate::paths::KeyMode`].
const V19: &str = "
CREATE TABLE library_root (
    library_id TEXT PRIMARY KEY NOT NULL REFERENCES library (library_id) ON DELETE NO ACTION,
    source_id TEXT NOT NULL REFERENCES library (library_id) ON DELETE NO ACTION,
    prefix TEXT NOT NULL,
    prefix_key TEXT NOT NULL,
    CHECK (library_id <> source_id),
    UNIQUE (source_id, prefix_key)
);
";

/// Version 20: server journal of browser writes and its retention (ADR 0005).
///
/// Additive. `coalesce_key` lets a newer progress or preference replace the older ones with an
/// index lookup instead of a scan. `sync_device` records which devices an owner syncs with: the
/// server journals the owner's writes only while one exists.
const V20: &str = "
ALTER TABLE sync_operation ADD COLUMN coalesce_key TEXT;

CREATE INDEX sync_operation_key_idx ON sync_operation (owner_id, coalesce_key)
    WHERE coalesce_key IS NOT NULL;

UPDATE sync_operation SET coalesce_key = 'p:' || json_extract(payload, '$.media_id')
 WHERE json_extract(payload, '$.kind') = 'progress';
UPDATE sync_operation SET coalesce_key = 'pref'
 WHERE json_extract(payload, '$.kind') = 'preference';

CREATE TABLE sync_device (
    owner_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    last_seen_at INTEGER NOT NULL,
    PRIMARY KEY (owner_id, device_id)
);
";

/// Version 21: folders the administrator took back from automatic course detection (ADR 0004).
///
/// Additive. Without it, removing an automatically found course would bring it back on the
/// next scan. Plain keys, no foreign key: a source that is removed leaves nothing worth keeping,
/// and the rows are tiny.
const V21: &str = "
CREATE TABLE library_root_dismissed (
    source_id TEXT NOT NULL,
    prefix_key TEXT NOT NULL,
    PRIMARY KEY (source_id, prefix_key)
);
";

/// Version 22: 4-grade flashcard ratings (`hard` and `easy`) and FTS5 search indexes for
/// flashcards, time bookmarks, and PDF annotations.
const V22: &str = "
CREATE TABLE flashcard_review_next (
    review_id INTEGER PRIMARY KEY,
    owner_id TEXT NOT NULL,
    card_id TEXT NOT NULL,
    rating TEXT NOT NULL CHECK (rating IN ('again', 'hard', 'good', 'easy')),
    previous_interval_days INTEGER NOT NULL CHECK (previous_interval_days >= 0 AND previous_interval_days <= 365),
    next_due INTEGER NOT NULL,
    reviewed_at INTEGER NOT NULL
);

INSERT INTO flashcard_review_next
    (review_id, owner_id, card_id, rating, previous_interval_days, next_due, reviewed_at)
SELECT review_id, owner_id, card_id, rating, previous_interval_days, next_due, reviewed_at
FROM flashcard_review;

DROP TABLE flashcard_review;
ALTER TABLE flashcard_review_next RENAME TO flashcard_review;
CREATE INDEX IF NOT EXISTS flashcard_review_owner_card_idx
    ON flashcard_review (owner_id, card_id, reviewed_at);

CREATE VIRTUAL TABLE IF NOT EXISTS flashcard_fts USING fts5(
    front,
    back,
    content = 'flashcard',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS flashcard_fts_after_insert
AFTER INSERT ON flashcard
BEGIN
    INSERT INTO flashcard_fts (rowid, front, back) VALUES (NEW.rowid, NEW.front, NEW.back);
END;

CREATE TRIGGER IF NOT EXISTS flashcard_fts_after_update
AFTER UPDATE OF front, back ON flashcard
BEGIN
    INSERT INTO flashcard_fts (flashcard_fts, rowid, front, back)
    VALUES ('delete', OLD.rowid, OLD.front, OLD.back);
    INSERT INTO flashcard_fts (rowid, front, back) VALUES (NEW.rowid, NEW.front, NEW.back);
END;

CREATE TRIGGER IF NOT EXISTS flashcard_fts_after_delete
AFTER DELETE ON flashcard
BEGIN
    INSERT INTO flashcard_fts (flashcard_fts, rowid, front, back)
    VALUES ('delete', OLD.rowid, OLD.front, OLD.back);
END;

INSERT INTO flashcard_fts (flashcard_fts) VALUES ('rebuild');

CREATE VIRTUAL TABLE IF NOT EXISTS media_bookmark_fts USING fts5(
    label,
    content = 'media_bookmark',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS media_bookmark_fts_after_insert
AFTER INSERT ON media_bookmark
BEGIN
    INSERT INTO media_bookmark_fts (rowid, label) VALUES (NEW.rowid, NEW.label);
END;

CREATE TRIGGER IF NOT EXISTS media_bookmark_fts_after_update
AFTER UPDATE OF label ON media_bookmark
BEGIN
    INSERT INTO media_bookmark_fts (media_bookmark_fts, rowid, label)
    VALUES ('delete', OLD.rowid, OLD.label);
    INSERT INTO media_bookmark_fts (rowid, label) VALUES (NEW.rowid, NEW.label);
END;

CREATE TRIGGER IF NOT EXISTS media_bookmark_fts_after_delete
AFTER DELETE ON media_bookmark
BEGIN
    INSERT INTO media_bookmark_fts (media_bookmark_fts, rowid, label)
    VALUES ('delete', OLD.rowid, OLD.label);
END;

INSERT INTO media_bookmark_fts (media_bookmark_fts) VALUES ('rebuild');

CREATE VIRTUAL TABLE IF NOT EXISTS pdf_annotation_fts USING fts5(
    quote,
    content,
    content = 'pdf_annotation',
    content_rowid = 'rowid',
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS pdf_annotation_fts_after_insert
AFTER INSERT ON pdf_annotation
BEGIN
    INSERT INTO pdf_annotation_fts (rowid, quote, content) VALUES (NEW.rowid, NEW.quote, NEW.content);
END;

CREATE TRIGGER IF NOT EXISTS pdf_annotation_fts_after_update
AFTER UPDATE OF quote, content ON pdf_annotation
BEGIN
    INSERT INTO pdf_annotation_fts (pdf_annotation_fts, rowid, quote, content)
    VALUES ('delete', OLD.rowid, OLD.quote, OLD.content);
    INSERT INTO pdf_annotation_fts (rowid, quote, content) VALUES (NEW.rowid, NEW.quote, NEW.content);
END;

CREATE TRIGGER IF NOT EXISTS pdf_annotation_fts_after_delete
AFTER DELETE ON pdf_annotation
BEGIN
    INSERT INTO pdf_annotation_fts (pdf_annotation_fts, rowid, quote, content)
    VALUES ('delete', OLD.rowid, OLD.quote, OLD.content);
END;

INSERT INTO pdf_annotation_fts (pdf_annotation_fts) VALUES ('rebuild');
";

/// Migration chain applied by [`crate::db::Database`]; index `n` takes the database from
/// version `n` to `n + 1`.
pub(crate) const MIGRATIONS: &[&str] = &[
    V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14, V15, V16, V17, V18, V19, V20, V21,
    V22,
];

/// Schema version stored in `PRAGMA user_version` (0 when never migrated).
///
/// The same pragma used by the old `rusqlite_migration` dependency, so existing
/// databases stay at the version they already had.
pub(crate) fn schema_version(conn: &Connection) -> rusqlite::Result<usize> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    usize::try_from(version).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, version))
}

/// Applies the pending migrations in a single transaction: either all go in, or none do.
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
