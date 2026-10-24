//! Estado persistente de sessões Pomodoro por dono.

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};

/// Estado possível de uma sessão Pomodoro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PomodoroStatus {
    /// Nenhuma sessão em andamento.
    Idle,
    /// Sessão contando tempo.
    Running,
    /// Sessão pausada.
    Paused,
}

/// Estado persistido de uma sessão Pomodoro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PomodoroState {
    /// Dono opaco da sessão.
    pub owner_id: String,
    /// Estado atual.
    pub status: PomodoroStatus,
    /// Duração alvo em segundos.
    pub duration_seconds: i64,
    /// Tempo transcorrido em segundos.
    pub elapsed_seconds: i64,
    /// Última alteração real.
    pub updated_at: i64,
}

/// Duração padrão de uma sessão.
pub const DEFAULT_POMODORO_SECONDS: i64 = 25 * 60;

/// Lê o estado, retornando uma sessão ociosa quando ainda não há registro.
pub fn pomodoro_state(db: &Database, owner_id: &str) -> Result<PomodoroState> {
    validate_owner(owner_id)?;
    match db.connection().query_row(
        "SELECT owner_id, status, duration_seconds, elapsed_seconds, updated_at FROM pomodoro_state WHERE owner_id = ?1",
        [owner_id],
        state_from_row,
    ) {
        Ok(state) => Ok(state),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(PomodoroState {
            owner_id: owner_id.to_owned(),
            status: PomodoroStatus::Idle,
            duration_seconds: DEFAULT_POMODORO_SECONDS,
            elapsed_seconds: 0,
            updated_at: 0,
        }),
        Err(error) => Err(error.into()),
    }
}

/// Inicia ou retoma uma sessão com a duração escolhida.
pub fn start_pomodoro(
    db: &Database,
    owner_id: &str,
    duration_seconds: i64,
) -> Result<PomodoroState> {
    validate_owner(owner_id)?;
    if !(60..=86_400).contains(&duration_seconds) {
        return Err(Error::InvalidPomodoroDuration { duration_seconds });
    }
    db.connection().execute(
        "INSERT INTO pomodoro_state (owner_id, status, duration_seconds, elapsed_seconds, updated_at)
         VALUES (?1, 'running', ?2, 0, ?3)
         ON CONFLICT(owner_id) DO UPDATE SET status = 'running', duration_seconds = excluded.duration_seconds,
             updated_at = excluded.updated_at",
        rusqlite::params![owner_id, duration_seconds, now_unix_seconds()],
    )?;
    pomodoro_state(db, owner_id)
}

/// Pausa a sessão e limita o tempo decorrido à duração configurada.
pub fn pause_pomodoro(
    db: &Database,
    owner_id: &str,
    elapsed_seconds: i64,
) -> Result<PomodoroState> {
    validate_owner(owner_id)?;
    if elapsed_seconds < 0 {
        return Err(Error::InvalidStudyPosition {
            position_seconds: elapsed_seconds,
        });
    }
    db.connection().execute(
        "UPDATE pomodoro_state SET status = 'paused', elapsed_seconds = MIN(?2, duration_seconds), updated_at = ?3 WHERE owner_id = ?1",
        rusqlite::params![owner_id, elapsed_seconds, now_unix_seconds()],
    )?;
    pomodoro_state(db, owner_id)
}

/// Reinicia a sessão e mantém a duração configurada.
pub fn reset_pomodoro(db: &Database, owner_id: &str) -> Result<PomodoroState> {
    validate_owner(owner_id)?;
    db.connection().execute(
        "UPDATE pomodoro_state SET status = 'idle', elapsed_seconds = 0, updated_at = ?2 WHERE owner_id = ?1",
        rusqlite::params![owner_id, now_unix_seconds()],
    )?;
    pomodoro_state(db, owner_id)
}

fn validate_owner(owner_id: &str) -> Result<()> {
    if owner_id.trim().is_empty() {
        Err(Error::InvalidOwnerId)
    } else {
        Ok(())
    }
}

fn state_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PomodoroState> {
    Ok(PomodoroState {
        owner_id: row.get(0)?,
        status: match row.get::<_, String>(1)?.as_str() {
            "running" => PomodoroStatus::Running,
            "paused" => PomodoroStatus::Paused,
            _ => PomodoroStatus::Idle,
        },
        duration_seconds: row.get(2)?,
        elapsed_seconds: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_pause_reset_and_isolates_owners() {
        let db = Database::open_in_memory().unwrap();
        let state = start_pomodoro(&db, "a", 120).unwrap();
        assert_eq!(state.status, PomodoroStatus::Running);
        let state = pause_pomodoro(&db, "a", 300).unwrap();
        assert_eq!(
            (state.status, state.elapsed_seconds),
            (PomodoroStatus::Paused, 120)
        );
        assert_eq!(
            pomodoro_state(&db, "b").unwrap().status,
            PomodoroStatus::Idle
        );
        let state = reset_pomodoro(&db, "a").unwrap();
        assert_eq!(
            (state.status, state.elapsed_seconds, state.duration_seconds),
            (PomodoroStatus::Idle, 0, 120)
        );
    }

    #[test]
    fn resuming_a_paused_session_preserves_elapsed_time() {
        let db = Database::open_in_memory().unwrap();
        start_pomodoro(&db, "a", 120).unwrap();
        pause_pomodoro(&db, "a", 45).unwrap();

        let resumed = start_pomodoro(&db, "a", 120).unwrap();

        assert_eq!(resumed.status, PomodoroStatus::Running);
        assert_eq!(resumed.elapsed_seconds, 45);
    }

    #[test]
    fn rejects_out_of_range_duration_without_creating_state() {
        let db = Database::open_in_memory().unwrap();
        assert!(matches!(
            start_pomodoro(&db, "a", 59),
            Err(Error::InvalidPomodoroDuration { .. })
        ));
        assert_eq!(
            pomodoro_state(&db, "a").unwrap().status,
            PomodoroStatus::Idle
        );
    }
}
