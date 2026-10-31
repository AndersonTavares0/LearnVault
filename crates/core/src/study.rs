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

use rusqlite::{OptionalExtension, params};
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
    validate_owner(owner_id)?;

    if position_seconds < 0 {
        return Err(Error::InvalidStudyPosition { position_seconds });
    }

    let conn = db.connection();
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
    validate_owner(owner_id)?;

    let conn = db.connection();
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
fn validate_owner(owner_id: &str) -> Result<()> {
    if owner_id.trim().is_empty() {
        return Err(Error::InvalidOwnerId);
    }

    Ok(())
}

/// Recusa uma mídia que não existe no catálogo.
///
/// A disponibilidade do arquivo não é consultada: uma mídia ausente continua sendo uma
/// mídia conhecida, com identidade e progresso próprios.
fn require_media(conn: &rusqlite::Connection, media_id: &str) -> Result<()> {
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

/// Cria uma nova anotação de estudo com marcador de tempo.
pub fn create_study_note(
    db: &Database,
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

    let conn = db.connection();
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
    validate_owner(owner_id)?;
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.len() > 10000 {
        return Err(Error::InvalidStudyNoteContent);
    }

    let conn = db.connection();
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
pub fn delete_study_note(db: &Database, owner_id: &str, note_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let changes = conn.execute(
        "DELETE FROM study_note WHERE owner_id = ?1 AND note_id = ?2",
        rusqlite::params![owner_id, note_id],
    )?;
    Ok(changes > 0)
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
