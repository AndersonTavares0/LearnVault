//! Progresso de estudo: posição de retomada e conclusão, por mídia e por dono.
//!
//! O núcleo mantém, para cada par `owner_id` + `media_id`, onde a reprodução parou e se a
//! aula foi concluída. O `owner_id` é opaco e explícito — no desktop é um identificador
//! fixo, no servidor é o usuário autenticado — e é o único limite de isolamento entre
//! dados de pessoas diferentes: nenhuma operação cruza donos.
//!
//! # Regras
//!
//! - posição e conclusão são independentes: avançar não desfaz a conclusão e concluir não
//!   move o ponto de retomada;
//! - reenviar um valor idêntico não é uma alteração e **não** move `updated_at`, que
//!   representa a última mudança real;
//! - a mídia precisa existir no catálogo, tanto na leitura quanto na escrita; estar marcada
//!   como ausente **não** impede ler nem gravar o progresso, porque a identidade da mídia
//!   sobrevive à falta temporária do arquivo;
//! - a primeira chamada para um par sem estado cria a linha e conta como atividade do
//!   usuário, mesmo quando o valor enviado é neutro (posição zero ou não concluída);
//! - cada mutação é uma única instrução SQL: não há leitura-modificação-escrita do estado, e
//!   a existência da mídia é uma pré-checagem cuja garantia última é a chave estrangeira;
//! - nenhuma operação escreve em `media`, `scan`, `library` ou na biblioteca física.
//!
//! # Limites conhecidos
//!
//! - `updated_at` tem resolução de um segundo: duas alterações reais no mesmo segundo gravam
//!   o mesmo instante, e a ordem dos recentes cai no desempate por `media_id`. Um relógio que
//!   retroceda também faz a alteração mais nova perder prioridade. Os dois casos exigiriam
//!   uma precisão que a coluna não oferece.
//! - o par `owner_id` + `media_id` é global: o núcleo não sabe a qual biblioteca a mídia
//!   pertence, então decidir se um dono pode pedir aquela mídia é responsabilidade do
//!   adaptador.
//! - `owner_id` é usado exatamente como recebido e precisa ser canônico — o mesmo valor para
//!   o mesmo dono em todas as chamadas. A coluna compara bytes, então `"a"` e `"a "` seriam
//!   donos diferentes.

use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};

/// Teto de resultados de [`recent_study`].
///
/// É um limite fixo de proteção contra consultas ilimitadas, e não uma configuração: o
/// valor acompanha o código e muda só com uma decisão de projeto.
pub const MAX_RECENT_STUDY: usize = 100;

/// Progresso de estudo de uma mídia para um dono.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyProgress {
    /// Identificador da mídia, a identidade que sobrevive a renomeações e ausências.
    pub media_id: String,
    /// Ponto de retomada em segundos.
    pub position_seconds: i64,
    /// Indica que a aula foi concluída.
    pub completed: bool,
    /// Instante da última alteração real, em segundos desde a época Unix.
    ///
    /// Resolução de um segundo: duas alterações no mesmo segundo gravam o mesmo instante.
    pub updated_at: i64,
}

/// Lê o progresso registrado, se existir.
///
/// Ausência de progresso não é erro: significa que a mídia nunca foi estudada por este dono.
/// Uma mídia que não existe no catálogo é reportada, para não confundir "desconhecida" com
/// "nunca estudada".
pub fn study_progress(
    db: &Database,
    owner_id: &str,
    media_id: &str,
) -> Result<Option<StudyProgress>> {
    validate_owner(owner_id)?;

    let conn = db.connection();
    require_media(conn, media_id)?;

    conn.query_row(
        "SELECT media_id, position_seconds, completed, updated_at FROM study_state
             WHERE owner_id = ?1 AND media_id = ?2",
        rusqlite::params![owner_id, media_id],
        progress_from_row,
    )
    .optional()
    .map_err(Error::from)
}

/// Registra a posição de retomada, preservando a conclusão já registrada.
pub fn record_position(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
) -> Result<StudyProgress> {
    record_position_on(db.connection(), owner_id, media_id, position_seconds)
}

/// [`record_position`] sobre uma conexão já aberta.
///
/// A forma interna permite que uma transação do chamador agrupe a escrita de estudo e o
/// registro da operação de sincronização.
pub(crate) fn record_position_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
) -> Result<StudyProgress> {
    validate_owner(owner_id)?;

    if position_seconds < 0 {
        return Err(Error::InvalidStudyPosition { position_seconds });
    }

    require_media(conn, media_id)?;

    // `completed` não aparece no `DO UPDATE`, então a conclusão é preservada pela própria
    // instrução. `updated_at` só avança quando a posição muda de fato: reenviar o mesmo
    // valor é idempotente e não conta como alteração.
    conn.query_row(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES (?1, ?2, ?3, 0, ?4)
         ON CONFLICT (owner_id, media_id) DO UPDATE SET
             position_seconds = excluded.position_seconds,
             updated_at = CASE
                 WHEN study_state.position_seconds <> excluded.position_seconds
                 THEN excluded.updated_at
                 ELSE study_state.updated_at
             END
         RETURNING media_id, position_seconds, completed, updated_at",
        rusqlite::params![owner_id, media_id, position_seconds, now_unix_seconds()],
        progress_from_row,
    )
    .map_err(Error::from)
}

/// Marca ou desmarca a conclusão, preservando a posição de retomada.
pub fn set_completed(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    completed: bool,
) -> Result<StudyProgress> {
    set_completed_on(db.connection(), owner_id, media_id, completed)
}

/// [`set_completed`] sobre uma conexão já aberta.
pub(crate) fn set_completed_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    completed: bool,
) -> Result<StudyProgress> {
    validate_owner(owner_id)?;

    require_media(conn, media_id)?;

    // Simétrico a [`record_position`]: `position_seconds` não é tocado pelo `DO UPDATE`, e
    // o instante só avança quando a conclusão realmente muda.
    conn.query_row(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES (?1, ?2, 0, ?3, ?4)
         ON CONFLICT (owner_id, media_id) DO UPDATE SET
             completed = excluded.completed,
             updated_at = CASE
                 WHEN study_state.completed <> excluded.completed
                 THEN excluded.updated_at
                 ELSE study_state.updated_at
             END
         RETURNING media_id, position_seconds, completed, updated_at",
        rusqlite::params![owner_id, media_id, i64::from(completed), now_unix_seconds()],
        progress_from_row,
    )
    .map_err(Error::from)
}

/// Lista o progresso do dono, do alterado mais recentemente para o mais antigo.
///
/// O empate de instantes é desfeito pelo `media_id`, para que a ordem seja determinística.
/// O limite é reduzido ao teto de [`MAX_RECENT_STUDY`]; um limite zero não consulta nada.
/// Mídias removidas do catálogo não aparecem, mas o progresso delas continua gravado.
pub fn recent_study(db: &Database, owner_id: &str, limit: usize) -> Result<Vec<StudyProgress>> {
    validate_owner(owner_id)?;

    let limit = limit.min(MAX_RECENT_STUDY);
    if limit == 0 {
        return Ok(Vec::new());
    }

    let conn = db.connection();
    let mut statement = conn.prepare(
        "SELECT media_id, position_seconds, completed, updated_at FROM study_state
         WHERE owner_id = ?1
           AND NOT EXISTS (SELECT 1
                             FROM media_removal r
                             JOIN media m
                               ON m.library_id = r.library_id
                              AND m.media_id = r.media_id
                            WHERE m.media_id = study_state.media_id)
         ORDER BY updated_at DESC, media_id ASC
         LIMIT ?2",
    )?;
    let rows = statement.query_map(
        rusqlite::params![owner_id, i64::try_from(limit).unwrap_or(i64::MAX)],
        progress_from_row,
    )?;

    let mut states = Vec::new();
    for state in rows {
        states.push(state?);
    }

    Ok(states)
}

/// Recusa um dono não identificado.
pub(crate) fn validate_owner(owner_id: &str) -> Result<()> {
    if owner_id.trim().is_empty() {
        return Err(Error::InvalidOwnerId);
    }

    Ok(())
}

/// Recusa uma mídia que não existe no catálogo.
///
/// A disponibilidade do arquivo não é consultada: uma mídia ausente continua sendo uma
/// mídia conhecida, com identidade e progresso próprios.
pub(crate) fn require_media(conn: &rusqlite::Connection, media_id: &str) -> Result<()> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM media WHERE media_id = ?1",
            [media_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        })
    }
}

/// Traduz uma linha de `study_state` no estado de progresso.
fn progress_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StudyProgress> {
    Ok(StudyProgress {
        media_id: row.get(0)?,
        position_seconds: row.get(1)?,
        completed: row.get::<_, i64>(2)? != 0,
        updated_at: row.get(3)?,
    })
}

/// Um dia da semana corrente e se ele teve atividade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyDay {
    /// Data local no formato `YYYY-MM-DD`.
    pub day: String,
    /// `true` quando houve progresso ou conclusão nesse dia.
    pub active: bool,
}

/// Resumo de ofensiva e da semana corrente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudySummary {
    /// Dias consecutivos até hoje (ou até ontem, quando a ofensiva está em risco).
    pub current_streak: i64,
    /// Maior sequência já registrada.
    pub longest_streak: i64,
    /// Dia local usado como hoje, no formato `YYYY-MM-DD`.
    pub today: String,
    /// Se houve atividade hoje.
    pub today_active: bool,
    /// `true` quando hoje ainda não conta, mas a ofensiva de ontem segue viva.
    pub at_risk: bool,
    /// Dias ativos na semana corrente, de segunda a domingo.
    pub week_active_days: i64,
    /// Aulas atualmente marcadas como concluídas pelo dono.
    pub completed_lessons: i64,
    /// Os sete dias da semana corrente, de segunda a domingo.
    pub days: Vec<StudyDay>,
    /// Dias ativos nos últimos 365 dias até hoje, em ordem crescente, no formato `YYYY-MM-DD`.
    pub active_days_year: Vec<String>,
}

/// Resumo usando o dia local atual do banco.
pub fn study_summary(db: &Database, owner_id: &str) -> Result<StudySummary> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let today: String = conn.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    summary_for_connection(conn, owner_id, &today)
}

/// Resumo com um dia explícito, para testes determinísticos.
///
/// O dia precisa ser uma data ISO canônica (`YYYY-MM-DD`); datas ambíguas como
/// `2026-1-5` são recusadas em vez de normalizadas silenciosamente.
pub fn study_summary_on(db: &Database, owner_id: &str, today: &str) -> Result<StudySummary> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let valid: i64 = conn.query_row(
        "SELECT CASE WHEN date(?1) = ?1 THEN 1 ELSE 0 END",
        [today],
        |row| row.get(0),
    )?;
    if valid == 0 {
        return Err(Error::InvalidStudyDay {
            day: today.to_owned(),
        });
    }
    summary_for_connection(conn, owner_id, today)
}

fn summary_for_connection(
    conn: &rusqlite::Connection,
    owner_id: &str,
    today: &str,
) -> Result<StudySummary> {
    let mut statement = conn.prepare(
        "SELECT CAST(julianday(day) + 0.5 AS INTEGER) AS day_number
         FROM study_day
         WHERE owner_id = ?1 AND day <= ?2
         ORDER BY day ASC",
    )?;
    let active_days = statement
        .query_map(params![owner_id, today], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    let active: HashSet<i64> = active_days.iter().copied().collect();
    let today_number: i64 = conn.query_row(
        "SELECT CAST(julianday(?1) + 0.5 AS INTEGER)",
        [today],
        |row| row.get(0),
    )?;
    let today_active = active.contains(&today_number);
    let yesterday_number = today_number - 1;
    let yesterday_active = active.contains(&yesterday_number);
    let at_risk = !today_active && yesterday_active;

    let mut current_streak = 0;
    let mut cursor = if today_active {
        today_number
    } else if yesterday_active {
        yesterday_number
    } else {
        0
    };
    if cursor != 0 {
        while active.contains(&cursor) {
            current_streak += 1;
            cursor -= 1;
        }
    }

    let mut longest_streak = 0;
    let mut run = 0;
    let mut previous = None;
    for day in &active_days {
        run = if previous.is_some_and(|value| value + 1 == *day) {
            run + 1
        } else {
            1
        };
        longest_streak = longest_streak.max(run);
        previous = Some(*day);
    }

    let weekday: i64 = conn.query_row(
        "SELECT CAST(strftime('%w', ?1) AS INTEGER)",
        [today],
        |row| row.get(0),
    )?;
    let monday = today_number - (weekday + 6) % 7;
    let mut days = Vec::with_capacity(7);
    for offset in 0..7 {
        let number = monday + offset;
        let day: String = conn.query_row("SELECT date(?1)", [number], |row| row.get(0))?;
        days.push(StudyDay {
            day,
            active: active.contains(&number),
        });
    }
    let week_active_days = days.iter().filter(|day| day.active).count() as i64;
    let mut active_days_year = Vec::new();
    for number in active_days
        .iter()
        .copied()
        .filter(|number| *number > today_number - 365)
    {
        let day: String = conn.query_row("SELECT date(?1)", [number], |row| row.get(0))?;
        active_days_year.push(day);
    }
    let completed_lessons: i64 = conn.query_row(
        "SELECT COUNT(*) FROM study_state WHERE owner_id = ?1 AND completed = 1",
        [owner_id],
        |row| row.get(0),
    )?;

    Ok(StudySummary {
        current_streak,
        longest_streak,
        today: today.to_owned(),
        today_active,
        at_risk,
        week_active_days,
        completed_lessons,
        days,
        active_days_year,
    })
}

/// Anotação de estudo associada a uma mídia e um ponto da linha do tempo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyNote {
    /// Identificador opaco da anotação (UUID v7).
    pub note_id: String,
    /// Identificador da mídia vinculada.
    pub media_id: String,
    /// Posição no player de vídeo em segundos.
    pub position_seconds: i64,
    /// Conteúdo textual da anotação.
    pub content: String,
    /// Instante de criação em segundos desde a época Unix.
    pub created_at: i64,
    /// Instante da última edição em segundos desde a época Unix.
    pub updated_at: i64,
}

/// Anotação de estudo com metadados da mídia para navegação global.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalStudyNote {
    /// Identificador opaco da anotação (UUID v7).
    pub note_id: String,
    /// Identificador da mídia vinculada.
    pub media_id: String,
    /// Caminho relativo da mídia no catálogo.
    pub relative_path: String,
    /// Posição no player de vídeo em segundos.
    pub position_seconds: i64,
    /// Conteúdo textual da anotação.
    pub content: String,
    /// Instante de criação em segundos desde a época Unix.
    pub created_at: i64,
    /// Instante da última edição em segundos desde a época Unix.
    pub updated_at: i64,
}

/// Lista as anotações do dono para uma mídia específica, ordenadas cronologicamente pelo timestamp da aula.
pub fn list_study_notes(db: &Database, owner_id: &str, media_id: &str) -> Result<Vec<StudyNote>> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    require_media(conn, media_id)?;

    let mut statement = conn.prepare(
        "SELECT note_id, media_id, position_seconds, content, created_at, updated_at
         FROM study_note
         WHERE owner_id = ?1 AND media_id = ?2
         ORDER BY position_seconds ASC, created_at ASC",
    )?;
    let rows = statement.query_map(rusqlite::params![owner_id, media_id], note_from_row)?;

    let mut notes = Vec::new();
    for note in rows {
        notes.push(note?);
    }
    Ok(notes)
}

/// Finds the media linked to a study note without returning the note contents.
pub fn study_note_media_id(db: &Database, note_id: &str) -> Result<Option<String>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT media_id FROM study_note WHERE note_id = ?1",
            [note_id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Lista todas as anotações do dono em todo o catálogo, ordenadas da mais recente para a mais antiga.
pub fn list_all_study_notes(db: &Database, owner_id: &str) -> Result<Vec<GlobalStudyNote>> {
    validate_owner(owner_id)?;
    let conn = db.connection();

    let mut statement = conn.prepare(
        "SELECT n.note_id, n.media_id, m.relative_path, n.position_seconds, n.content, n.created_at, n.updated_at
         FROM study_note n
         JOIN media m ON n.media_id = m.media_id
         WHERE n.owner_id = ?1
         ORDER BY n.updated_at DESC, n.created_at DESC",
    )?;
    let rows = statement.query_map(rusqlite::params![owner_id], |row| {
        Ok(GlobalStudyNote {
            note_id: row.get(0)?,
            media_id: row.get(1)?,
            relative_path: row.get(2)?,
            position_seconds: row.get(3)?,
            content: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;

    let mut notes = Vec::new();
    for note in rows {
        notes.push(note?);
    }
    Ok(notes)
}

/// Cria uma nova anotação de estudo com marcador de tempo.
pub fn create_study_note(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    content: &str,
) -> Result<StudyNote> {
    create_study_note_on(
        db.connection(),
        owner_id,
        media_id,
        position_seconds,
        content,
    )
}

/// [`create_study_note`] sobre uma conexão já aberta.
pub(crate) fn create_study_note_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    content: &str,
) -> Result<StudyNote> {
    validate_owner(owner_id)?;
    if position_seconds < 0 {
        return Err(Error::InvalidStudyPosition { position_seconds });
    }
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.len() > 10000 {
        return Err(Error::InvalidStudyNoteContent);
    }

    require_media(conn, media_id)?;

    let note_id = crate::ids::new_note_id();
    let now = now_unix_seconds();

    conn.execute(
        "INSERT INTO study_note (note_id, owner_id, media_id, position_seconds, content, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![note_id, owner_id, media_id, position_seconds, trimmed, now, now],
    )?;

    Ok(StudyNote {
        note_id,
        media_id: media_id.to_owned(),
        position_seconds,
        content: trimmed.to_owned(),
        created_at: now,
        updated_at: now,
    })
}

/// Atualiza o conteúdo de uma anotação existente do dono.
pub fn update_study_note(
    db: &Database,
    owner_id: &str,
    note_id: &str,
    content: &str,
) -> Result<StudyNote> {
    update_study_note_on(db.connection(), owner_id, note_id, content)
}

/// [`update_study_note`] sobre uma conexão já aberta.
pub(crate) fn update_study_note_on(
    conn: &Connection,
    owner_id: &str,
    note_id: &str,
    content: &str,
) -> Result<StudyNote> {
    validate_owner(owner_id)?;
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.len() > 10000 {
        return Err(Error::InvalidStudyNoteContent);
    }

    let now = now_unix_seconds();

    let updated = conn
        .query_row(
            "UPDATE study_note
         SET content = ?1, updated_at = ?2
         WHERE owner_id = ?3 AND note_id = ?4
         RETURNING note_id, media_id, position_seconds, content, created_at, updated_at",
            rusqlite::params![trimmed, now, owner_id, note_id],
            note_from_row,
        )
        .optional()?;

    updated.ok_or_else(|| Error::StudyNoteNotFound {
        note_id: note_id.to_owned(),
    })
}

/// Remove uma anotação de estudo pertencente ao dono.
///
/// The tombstone is recorded here, not in the sync adapter, so every deletion path —
/// including the server handler, which is the journal itself and has no outbox to drain —
/// blocks a stale offline create from resurrecting the row. Tracked in #472.
pub fn delete_study_note(db: &Database, owner_id: &str, note_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    Ok(delete_study_note_on(db.connection(), owner_id, note_id)?.is_some())
}

/// [`delete_study_note`] over an open connection; returns the delete instant.
pub(crate) fn delete_study_note_on(
    conn: &Connection,
    owner_id: &str,
    note_id: &str,
) -> Result<Option<i64>> {
    let deleted_at = now_unix_seconds();
    let changes = conn.execute(
        "DELETE FROM study_note WHERE owner_id = ?1 AND note_id = ?2",
        rusqlite::params![owner_id, note_id],
    )?;
    if changes == 0 {
        return Ok(None);
    }
    crate::sync::record_tombstone_on(
        conn,
        owner_id,
        crate::sync::TOMBSTONE_NOTE,
        note_id,
        deleted_at,
    )?;
    Ok(Some(deleted_at))
}

fn note_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StudyNote> {
    Ok(StudyNote {
        note_id: row.get(0)?,
        media_id: row.get(1)?,
        position_seconds: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

/// Avaliação de uma revisão de cartão de memorização.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewRating {
    /// O aluno errou ou precisa repetir logo.
    Again,
    /// O aluno acertou e entendeu o conceito.
    Good,
}

impl ReviewRating {
    /// Converte de string ("again" ou "good"), case-insensitive.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "again" => Some(Self::Again),
            "good" => Some(Self::Good),
            _ => None,
        }
    }

    /// Retorna a representação estática em string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Again => "again",
            Self::Good => "good",
        }
    }
}

/// Cartão pessoal de estudo para repetição espaçada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flashcard {
    /// Identificador opaco do cartão (UUID v7).
    pub card_id: String,
    /// Dono do cartão.
    pub owner_id: String,
    /// Mídia vinculada.
    pub media_id: String,
    /// Instante da aula em que o cartão foi criado, quando informado.
    pub position_seconds: Option<i64>,
    /// Pergunta ou conteúdo da frente.
    pub front: String,
    /// Resposta ou conteúdo do verso.
    pub back: String,
    /// Intervalo atual de repetição em dias (0 a 365).
    pub interval_days: i64,
    /// Instante da próxima revisão em segundos Unix.
    pub next_due: i64,
    /// Instante de criação em segundos Unix.
    pub created_at: i64,
    /// Instante da última edição em segundos Unix.
    pub updated_at: i64,
}

/// Valida o texto da frente ou do verso do cartão (1 a 4000 escalares Unicode, não-vazio).
pub(crate) fn validate_card_text(text: &str) -> Result<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 4000 {
        return Err(Error::InvalidFlashcardContent);
    }
    Ok(trimmed.to_owned())
}

/// Lista os cartões do dono para uma mídia específica, ordenados por vencimento (`next_due ASC, created_at ASC`).
pub fn list_flashcards(db: &Database, owner_id: &str, media_id: &str) -> Result<Vec<Flashcard>> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    require_media(conn, media_id)?;

    let mut statement = conn.prepare(
        "SELECT card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at
         FROM flashcard
         WHERE owner_id = ?1 AND media_id = ?2
         ORDER BY next_due ASC, created_at ASC",
    )?;
    let rows = statement.query_map(rusqlite::params![owner_id, media_id], card_from_row)?;

    let mut cards = Vec::new();
    for card in rows {
        cards.push(card?);
    }
    Ok(cards)
}

/// Finds the media linked to a flashcard without returning its contents.
pub fn flashcard_media_id(db: &Database, card_id: &str) -> Result<Option<String>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT media_id FROM flashcard WHERE card_id = ?1",
            [card_id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Lists every card of the owner across all lessons, soonest due first. The review hub needs
/// what is due everywhere, not one lesson at a time.
pub fn list_all_flashcards(db: &Database, owner_id: &str) -> Result<Vec<Flashcard>> {
    validate_owner(owner_id)?;
    let mut statement = db.connection().prepare(
        "SELECT card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at
         FROM flashcard
         WHERE owner_id = ?1
         ORDER BY next_due ASC, created_at ASC",
    )?;
    let rows = statement.query_map([owner_id], card_from_row)?;
    let mut cards = Vec::new();
    for card in rows {
        cards.push(card?);
    }
    Ok(cards)
}

/// Cria um novo cartão de estudo vinculado à aula.
///
/// O cartão novo vence imediatamente (`next_due = now`, `interval_days = 0`).
pub fn create_flashcard(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    front: &str,
    back: &str,
    position_seconds: Option<i64>,
) -> Result<Flashcard> {
    create_flashcard_on(
        db.connection(),
        owner_id,
        media_id,
        front,
        back,
        position_seconds,
    )
}

/// [`create_flashcard`] over an open connection, so the sync queue can reuse the rules
/// inside its own transaction.
pub(crate) fn create_flashcard_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    front: &str,
    back: &str,
    position_seconds: Option<i64>,
) -> Result<Flashcard> {
    validate_owner(owner_id)?;
    let front_trimmed = validate_card_text(front)?;
    let back_trimmed = validate_card_text(back)?;
    if let Some(position) = position_seconds
        && position < 0
    {
        return Err(Error::InvalidStudyPosition {
            position_seconds: position,
        });
    }

    require_media(conn, media_id)?;

    let card_id = crate::ids::new_card_id();
    let now = now_unix_seconds();
    let interval_days = 0_i64;
    let next_due = now;

    conn.execute(
        "INSERT INTO flashcard (card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            card_id,
            owner_id,
            media_id,
            position_seconds,
            front_trimmed,
            back_trimmed,
            interval_days,
            next_due,
            now,
            now
        ],
    )?;

    Ok(Flashcard {
        card_id,
        owner_id: owner_id.to_owned(),
        media_id: media_id.to_owned(),
        position_seconds,
        front: front_trimmed,
        back: back_trimmed,
        interval_days,
        next_due,
        created_at: now,
        updated_at: now,
    })
}

/// Atualiza o texto da frente e/ou do verso de um cartão existente.
///
/// Preserva o agendamento (`next_due` e `interval_days`) e atualiza `updated_at`.
pub fn update_flashcard(
    db: &Database,
    owner_id: &str,
    card_id: &str,
    front: &str,
    back: &str,
) -> Result<Flashcard> {
    update_flashcard_on(db.connection(), owner_id, card_id, front, back)
}

/// [`update_flashcard`] over an open connection.
pub(crate) fn update_flashcard_on(
    conn: &Connection,
    owner_id: &str,
    card_id: &str,
    front: &str,
    back: &str,
) -> Result<Flashcard> {
    validate_owner(owner_id)?;
    let front_trimmed = validate_card_text(front)?;
    let back_trimmed = validate_card_text(back)?;

    let now = now_unix_seconds();

    let updated = conn
        .query_row(
            "UPDATE flashcard
             SET front = ?1, back = ?2, updated_at = ?3
             WHERE owner_id = ?4 AND card_id = ?5
             RETURNING card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at",
            rusqlite::params![front_trimmed, back_trimmed, now, owner_id, card_id],
            card_from_row,
        )
        .optional()?;

    updated.ok_or_else(|| Error::FlashcardNotFound {
        card_id: card_id.to_owned(),
    })
}

/// Remove um cartão de estudo pertencente ao dono.
///
/// Records the tombstone for the same reason as [`delete_study_note`]: a card created
/// offline must not resurrect a deleted one.
pub fn delete_flashcard(db: &Database, owner_id: &str, card_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    Ok(delete_flashcard_on(db.connection(), owner_id, card_id)?.is_some())
}

/// [`delete_flashcard`] over an open connection; returns the delete instant.
pub(crate) fn delete_flashcard_on(
    conn: &Connection,
    owner_id: &str,
    card_id: &str,
) -> Result<Option<i64>> {
    let deleted_at = now_unix_seconds();
    let changes = conn.execute(
        "DELETE FROM flashcard WHERE owner_id = ?1 AND card_id = ?2",
        rusqlite::params![owner_id, card_id],
    )?;
    if changes == 0 {
        return Ok(None);
    }
    crate::sync::record_tombstone_on(
        conn,
        owner_id,
        crate::sync::TOMBSTONE_FLASHCARD,
        card_id,
        deleted_at,
    )?;
    Ok(Some(deleted_at))
}

/// Registra uma revisão (Again ou Good) em um cartão existente.
///
/// Regras de agendamento:
/// - `Again`: zera o intervalo (`interval_days = 0`) e agenda para daqui a 10 minutos (`next_due = now + 600`).
/// - `Good`: se `interval_days == 0`, define 1 dia (`interval_days = 1`), caso contrário dobra o intervalo
///   (`interval_days = min(interval_days * 2, 365)`), e agenda para `now + interval_days * 86400`.
///
/// O agendamento e o log de revisões mudam na mesma transação: ou a revisão inteira existe, ou
/// nenhuma parte dela. O log é somente de acréscimo e guarda o intervalo anterior.
pub fn review_flashcard(
    db: &Database,
    owner_id: &str,
    card_id: &str,
    rating: ReviewRating,
) -> Result<Flashcard> {
    let conn = db.connection();
    let transaction = conn.unchecked_transaction()?;
    let card = review_flashcard_on(&transaction, owner_id, card_id, rating)?;
    transaction.commit()?;
    Ok(card)
}

/// Próximo estado de agendamento de uma revisão: `(interval_days, next_due)`.
///
/// Compartilhado pela revisão local e pela revisão que chega pelo diário de sincronização,
/// para que as duas nunca divirjam.
pub(crate) fn schedule_review(
    prev_interval: i64,
    rating: ReviewRating,
    reviewed_at: i64,
) -> (i64, i64) {
    match rating {
        ReviewRating::Again => (0_i64, reviewed_at + 600),
        ReviewRating::Good => {
            let next_interval = if prev_interval == 0 {
                1_i64
            } else {
                prev_interval.saturating_mul(2).min(365)
            };
            (
                next_interval,
                reviewed_at + next_interval.saturating_mul(86_400),
            )
        }
    }
}

/// [`review_flashcard`] sobre uma conexão já dentro de uma transação.
pub(crate) fn review_flashcard_on(
    conn: &Connection,
    owner_id: &str,
    card_id: &str,
    rating: ReviewRating,
) -> Result<Flashcard> {
    validate_owner(owner_id)?;
    let now = now_unix_seconds();

    // Consulta o cartão atual para obter o interval_days prévio
    let current_interval: Option<i64> = conn
        .query_row(
            "SELECT interval_days FROM flashcard WHERE owner_id = ?1 AND card_id = ?2",
            rusqlite::params![owner_id, card_id],
            |row| row.get(0),
        )
        .optional()?;

    let Some(prev_interval) = current_interval else {
        return Err(Error::FlashcardNotFound {
            card_id: card_id.to_owned(),
        });
    };

    let (new_interval, next_due) = schedule_review(prev_interval, rating, now);

    let updated = conn.query_row(
        "UPDATE flashcard
         SET interval_days = ?1, next_due = ?2, updated_at = ?3
         WHERE owner_id = ?4 AND card_id = ?5
         RETURNING card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at",
        rusqlite::params![new_interval, next_due, now, owner_id, card_id],
        card_from_row,
    )?;

    conn.execute(
        "INSERT INTO flashcard_review
             (owner_id, card_id, rating, previous_interval_days, next_due, reviewed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            owner_id,
            card_id,
            rating.as_str(),
            prev_interval,
            next_due,
            now
        ],
    )?;

    Ok(updated)
}

/// Uma revisão registrada no histórico de um cartão.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashcardReview {
    /// Identificador sequencial da revisão.
    pub review_id: i64,
    /// Cartão revisado.
    pub card_id: String,
    /// Avaliação dada pelo aluno.
    pub rating: ReviewRating,
    /// Intervalo em dias antes desta revisão.
    pub previous_interval_days: i64,
    /// Próximo vencimento calculado por esta revisão.
    pub next_due: i64,
    /// Instante da revisão em segundos Unix.
    pub reviewed_at: i64,
}

/// Teto de resultados de [`list_flashcard_reviews`].
///
/// Acompanha o código e muda só com uma decisão de projeto, como [`MAX_RECENT_STUDY`].
pub const MAX_FLASHCARD_REVIEWS: usize = 100;

/// Lista as revisões mais recentes do dono para um cartão, da mais nova para a mais antiga.
///
/// O limite é reduzido ao teto de [`MAX_FLASHCARD_REVIEWS`]; um limite zero não consulta
/// nada. O empate de instantes é desfeito por `review_id`, para uma ordem determinística.
pub fn list_flashcard_reviews(
    db: &Database,
    owner_id: &str,
    card_id: &str,
    limit: usize,
) -> Result<Vec<FlashcardReview>> {
    validate_owner(owner_id)?;

    let limit = limit.min(MAX_FLASHCARD_REVIEWS);
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut statement = db.connection().prepare(
        "SELECT review_id, card_id, rating, previous_interval_days, next_due, reviewed_at
         FROM flashcard_review
         WHERE owner_id = ?1 AND card_id = ?2
         ORDER BY reviewed_at DESC, review_id DESC
         LIMIT ?3",
    )?;
    let rows = statement.query_map(
        rusqlite::params![owner_id, card_id, i64::try_from(limit).unwrap_or(i64::MAX)],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        },
    )?;

    let mut reviews = Vec::new();
    for row in rows {
        let (review_id, card_id, rating, previous_interval_days, next_due, reviewed_at) = row?;
        let Some(rating) = ReviewRating::parse(&rating) else {
            return Err(Error::InvalidFlashcardRating);
        };
        reviews.push(FlashcardReview {
            review_id,
            card_id,
            rating,
            previous_interval_days,
            next_due,
            reviewed_at,
        });
    }
    Ok(reviews)
}

/// Conta todo o histórico de revisões de um cartão do dono, mesmo além do teto da listagem.
pub fn count_flashcard_reviews(db: &Database, owner_id: &str, card_id: &str) -> Result<i64> {
    validate_owner(owner_id)?;
    Ok(db.connection().query_row(
        "SELECT COUNT(*) FROM flashcard_review WHERE owner_id = ?1 AND card_id = ?2",
        rusqlite::params![owner_id, card_id],
        |row| row.get(0),
    )?)
}

/// Conta os cartões do dono que estão vencidos agora, o mesmo predicado da fila de revisão.
///
/// Cartões nunca revisados e revisados no passado contam; um `next_due` futuro não. O dono
/// é o único limite: a filtragem por acesso ao curso cabe ao adaptador.
pub fn count_due_flashcards(db: &Database, owner_id: &str) -> Result<i64> {
    validate_owner(owner_id)?;
    Ok(db.connection().query_row(
        "SELECT COUNT(*) FROM flashcard WHERE owner_id = ?1 AND next_due <= ?2",
        rusqlite::params![owner_id, now_unix_seconds()],
        |row| row.get(0),
    )?)
}

fn card_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Flashcard> {
    Ok(Flashcard {
        card_id: row.get(0)?,
        owner_id: row.get(1)?,
        media_id: row.get(2)?,
        position_seconds: row.get(3)?,
        front: row.get(4)?,
        back: row.get(5)?,
        interval_days: row.get(6)?,
        next_due: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// Limite de caracteres do rótulo de um marcador de tempo.
pub const MAX_BOOKMARK_LABEL_CHARS: usize = 200;

/// Marcador de tempo de uma mídia, visível apenas para o dono.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaBookmark {
    /// Identificador opaco do marcador (UUID v7).
    pub bookmark_id: String,
    /// Identificador da mídia vinculada.
    pub media_id: String,
    /// Instante da mídia em segundos.
    pub position_seconds: i64,
    /// Rótulo curto escrito pelo aluno.
    pub label: String,
    /// Instante de criação em segundos desde a época Unix.
    pub created_at: i64,
}

/// Lista os marcadores do dono para uma mídia, em ordem cronológica.
pub fn list_media_bookmarks(
    db: &Database,
    owner_id: &str,
    media_id: &str,
) -> Result<Vec<MediaBookmark>> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    require_media(conn, media_id)?;

    let mut statement = conn.prepare(
        "SELECT bookmark_id, media_id, position_seconds, label, created_at
         FROM media_bookmark
         WHERE owner_id = ?1 AND media_id = ?2
         ORDER BY position_seconds ASC, created_at ASC",
    )?;
    let rows = statement.query_map(rusqlite::params![owner_id, media_id], bookmark_from_row)?;

    let mut bookmarks = Vec::new();
    for bookmark in rows {
        bookmarks.push(bookmark?);
    }
    Ok(bookmarks)
}

/// Cria um marcador de tempo para o dono e a mídia.
pub fn create_media_bookmark(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    label: &str,
) -> Result<MediaBookmark> {
    create_media_bookmark_on(db.connection(), owner_id, media_id, position_seconds, label)
}

/// [`create_media_bookmark`] sobre uma conexão já aberta.
pub(crate) fn create_media_bookmark_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    label: &str,
) -> Result<MediaBookmark> {
    validate_owner(owner_id)?;
    if position_seconds < 0 {
        return Err(Error::InvalidStudyPosition { position_seconds });
    }
    let trimmed = label.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_BOOKMARK_LABEL_CHARS {
        return Err(Error::InvalidBookmarkLabel);
    }

    require_media(conn, media_id)?;

    let bookmark_id = crate::ids::new_bookmark_id();
    let now = now_unix_seconds();

    conn.execute(
        "INSERT INTO media_bookmark (bookmark_id, owner_id, media_id, position_seconds, label, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![bookmark_id, owner_id, media_id, position_seconds, trimmed, now],
    )?;

    Ok(MediaBookmark {
        bookmark_id,
        media_id: media_id.to_owned(),
        position_seconds,
        label: trimmed.to_owned(),
        created_at: now,
    })
}

/// Remove um marcador pertencente ao dono.
///
/// Records the tombstone for the same reason as [`delete_study_note`]: a bookmark created
/// offline must not resurrect a deleted one. Tracked in #472.
pub fn delete_media_bookmark(db: &Database, owner_id: &str, bookmark_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    Ok(delete_media_bookmark_on(db.connection(), owner_id, bookmark_id)?.is_some())
}

/// [`delete_media_bookmark`] over an open connection; returns the delete instant.
pub(crate) fn delete_media_bookmark_on(
    conn: &Connection,
    owner_id: &str,
    bookmark_id: &str,
) -> Result<Option<i64>> {
    let deleted_at = now_unix_seconds();
    let changes = conn.execute(
        "DELETE FROM media_bookmark WHERE owner_id = ?1 AND bookmark_id = ?2",
        rusqlite::params![owner_id, bookmark_id],
    )?;
    if changes == 0 {
        return Ok(None);
    }
    crate::sync::record_tombstone_on(
        conn,
        owner_id,
        crate::sync::TOMBSTONE_BOOKMARK,
        bookmark_id,
        deleted_at,
    )?;
    Ok(Some(deleted_at))
}

/// Finds the media linked to a bookmark without returning its contents.
pub fn media_bookmark_media_id(db: &Database, bookmark_id: &str) -> Result<Option<String>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT media_id FROM media_bookmark WHERE bookmark_id = ?1",
            [bookmark_id],
            |row| row.get(0),
        )
        .optional()?)
}

fn bookmark_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaBookmark> {
    Ok(MediaBookmark {
        bookmark_id: row.get(0)?,
        media_id: row.get(1)?,
        position_seconds: row.get(2)?,
        label: row.get(3)?,
        created_at: row.get(4)?,
    })
}

/// Favoritos de cursos e aulas de um dono.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StudyFavorites {
    /// Identificadores de cursos favoritados, do mais recente ao mais antigo.
    pub course_ids: Vec<String>,
    /// Identificadores de mídias (aulas) favoritadas, do mais recente ao mais antigo.
    pub media_ids: Vec<String>,
}

fn validate_favorite_target(target_id: &str) -> Result<String> {
    let trimmed = target_id.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return Err(Error::InvalidFavoriteTarget);
    }
    Ok(trimmed.to_owned())
}

/// Lista todos os cursos e aulas favoritados pelo dono.
///
/// Aulas que foram explicitamente removidas do catálogo (`media_removal`) não aparecem.
pub fn list_favorites(db: &Database, owner_id: &str) -> Result<StudyFavorites> {
    validate_owner(owner_id)?;
    let conn = db.connection();

    let mut course_stmt = conn.prepare(
        "SELECT target_id FROM study_favorite
         WHERE owner_id = ?1 AND target_kind = 'course'
         ORDER BY created_at DESC, target_id ASC",
    )?;
    let course_rows = course_stmt.query_map(params![owner_id], |row| row.get::<_, String>(0))?;
    let mut course_ids = Vec::new();
    for id in course_rows {
        course_ids.push(id?);
    }

    let mut lesson_stmt = conn.prepare(
        "SELECT f.target_id FROM study_favorite f
         WHERE f.owner_id = ?1 AND f.target_kind = 'lesson'
           AND NOT EXISTS (SELECT 1
                             FROM media_removal r
                             JOIN media m
                               ON m.library_id = r.library_id
                              AND m.media_id = r.media_id
                            WHERE m.media_id = f.target_id)
         ORDER BY f.created_at DESC, f.target_id ASC",
    )?;
    let lesson_rows = lesson_stmt.query_map(params![owner_id], |row| row.get::<_, String>(0))?;
    let mut media_ids = Vec::new();
    for id in lesson_rows {
        media_ids.push(id?);
    }

    Ok(StudyFavorites {
        course_ids,
        media_ids,
    })
}

/// Marca ou desmarca um curso (`course_id`) como favorito para o dono.
pub fn set_course_favorite(
    db: &Database,
    owner_id: &str,
    course_id: &str,
    favorite: bool,
) -> Result<bool> {
    validate_owner(owner_id)?;
    let target_id = validate_favorite_target(course_id)?;
    let conn = db.connection();

    if favorite {
        conn.execute(
            "INSERT INTO study_favorite (owner_id, target_kind, target_id, created_at)
             VALUES (?1, 'course', ?2, ?3)
             ON CONFLICT (owner_id, target_kind, target_id) DO NOTHING",
            params![owner_id, target_id, now_unix_seconds()],
        )?;
        Ok(true)
    } else {
        conn.execute(
            "DELETE FROM study_favorite WHERE owner_id = ?1 AND target_kind = 'course' AND target_id = ?2",
            params![owner_id, target_id],
        )?;
        Ok(false)
    }
}

/// Marca ou desmarca uma aula (`media_id`) como favorita para o dono.
pub fn set_lesson_favorite(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    favorite: bool,
) -> Result<bool> {
    validate_owner(owner_id)?;
    let target_id = validate_favorite_target(media_id)?;
    let conn = db.connection();

    if favorite {
        require_media(conn, &target_id)?;
        conn.execute(
            "INSERT INTO study_favorite (owner_id, target_kind, target_id, created_at)
             VALUES (?1, 'lesson', ?2, ?3)
             ON CONFLICT (owner_id, target_kind, target_id) DO NOTHING",
            params![owner_id, target_id, now_unix_seconds()],
        )?;
        Ok(true)
    } else {
        conn.execute(
            "DELETE FROM study_favorite WHERE owner_id = ?1 AND target_kind = 'lesson' AND target_id = ?2",
            params![owner_id, target_id],
        )?;
        Ok(false)
    }
}
