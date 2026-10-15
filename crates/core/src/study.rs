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
//! - a mídia precisa existir no catálogo; estar marcada como ausente **não** impede ler nem
//!   gravar o progresso, porque a identidade da mídia sobrevive à falta temporária do
//!   arquivo;
//! - cada mutação é uma única instrução SQL, então não existe leitura-modificação-escrita;
//! - nenhuma operação escreve em `media`, `scan`, `library` ou na biblioteca física.

use rusqlite::OptionalExtension;

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
    pub updated_at: i64,
}

/// Lê o progresso registrado, se existir.
///
/// Ausência de progresso não é erro: significa que a mídia nunca foi estudada por este dono.
pub fn study_progress(
    db: &Database,
    owner_id: &str,
    media_id: &str,
) -> Result<Option<StudyProgress>> {
    validate_owner(owner_id)?;

    db.connection()
        .query_row(
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
