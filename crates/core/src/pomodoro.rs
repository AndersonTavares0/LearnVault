//! Sessões persistentes de foco e recompensas idempotentes por dono.

use rusqlite::{OptionalExtension, params};

use crate::{Database, Error, Result, study::study_summary_on};

/// Tipo de intervalo gravado pelo Pomodoro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusKind {
    /// Bloco de estudo.
    Focus,
    /// Pausa curta.
    ShortBreak,
    /// Pausa longa.
    LongBreak,
}

impl FocusKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::ShortBreak => "short",
            Self::LongBreak => "long",
        }
    }
}

/// Sessão recebida do timer, identificada por uma chave estável para repetição segura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSessionInput {
    /// Identificador opaco gerado pelo cliente e reutilizado em tentativas de retry.
    pub id: String,
    /// Modo do timer.
    pub kind: FocusKind,
    /// Duração configurada, em segundos.
    pub planned_seconds: i64,
    /// Duração efetiva, em segundos.
    pub actual_seconds: i64,
    /// Indica que o temporizador chegou ao fim, não que foi pulado/interrompido.
    pub completed: bool,
    /// Início em epoch Unix segundos.
    pub started_at: i64,
    /// Fim em epoch Unix segundos.
    pub ended_at: i64,
}

/// Sessão de foco persistida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSession {
    /// Identificador idempotente no escopo do dono.
    pub id: String,
    /// Modo do timer (`focus`, `short` ou `long`).
    pub kind: FocusKind,
    /// Duração configurada, em segundos.
    pub planned_seconds: i64,
    /// Duração efetiva, em segundos.
    pub actual_seconds: i64,
    /// Indica se a sessão foi concluída normalmente.
    pub completed: bool,
    /// Início em epoch Unix segundos.
    pub started_at: i64,
    /// Fim em epoch Unix segundos.
    pub ended_at: i64,
}

/// Métricas de sessões de foco do dia e da semana local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSummary {
    /// Minutos de foco concluídos hoje.
    pub today_minutes: i64,
    /// Minutos de foco concluídos na semana atual.
    pub week_minutes: i64,
    /// Sessões de foco concluídas nesta semana.
    pub week_sessions: i64,
    /// Minutos de foco em todo o histórico, concluídos ou interrompidos.
    pub total_minutes: i64,
    /// Sessões de foco concluídas em todo o histórico.
    pub total_sessions: i64,
    /// Porcentagem concluída dentre as sessões de foco iniciadas.
    pub completion_rate: i64,
    /// Dia com mais minutos nesta semana, se houver sessões.
    pub best_day: Option<String>,
    /// Atividade de segunda a domingo.
    pub days: Vec<FocusDay>,
}

/// Minutos e sessões de foco em um dia da semana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusDay {
    /// Data local ISO `YYYY-MM-DD`.
    pub day: String,
    /// Minutos concluídos.
    pub minutes: i64,
    /// Sessões concluídas.
    pub sessions: i64,
}

/// Uma missão diária e seu progresso atual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyMission {
    /// Identificador estável: `lesson`, `focus-two` ou `focus-fifty`.
    pub id: String,
    /// Progresso contabilizado no dia local.
    pub progress: i64,
    /// Alvo para liberar o resgate.
    pub target: i64,
    /// Se os 30 XP já foram resgatados neste dia.
    pub claimed: bool,
}

/// Resumo de XP, missões e meta semanal persistidos para o dono.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardSummary {
    /// XP acumulado no ledger imutável.
    pub xp: i64,
    /// Aulas distintas já concluídas ao menos uma vez.
    pub completed_lessons: i64,
    /// Meta semanal configurada, entre 1 e 7 dias.
    pub weekly_goal: i64,
    /// Missões do dia local.
    pub missions: Vec<DailyMission>,
    /// Resumo de ofensiva usando o mesmo dia explícito.
    pub study: crate::StudySummary,
}

/// Persiste uma sessão. Repetir exatamente a mesma chave e payload não duplica sessão/XP.
pub fn record_focus_session(
    db: &Database,
    owner_id: &str,
    input: FocusSessionInput,
) -> Result<FocusSession> {
    validate_owner(owner_id)?;
    validate_day_and_id(
        &input.id,
        input.planned_seconds,
        input.actual_seconds,
        input.started_at,
        input.ended_at,
    )?;
    if input.completed && input.actual_seconds < input.planned_seconds {
        return Err(Error::InvalidFocusSession {
            reason: "sessão concluída antes do tempo planejado",
        });
    }

    let conn = db.connection();
    let inserted = conn.execute(
        "INSERT INTO focus_session
             (owner_id, session_id, kind, planned_seconds, actual_seconds, completed, started_at, ended_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (owner_id, session_id) DO NOTHING",
        params![
            owner_id,
            input.id,
            input.kind.as_str(),
            input.planned_seconds,
            input.actual_seconds,
            i64::from(input.completed),
            input.started_at,
            input.ended_at
        ],
    )?;

    let existing = conn.query_row(
        "SELECT session_id, kind, planned_seconds, actual_seconds, completed, started_at, ended_at
         FROM focus_session WHERE owner_id = ?1 AND session_id = ?2",
        params![owner_id, input.id],
        focus_session_from_row,
    )?;
    if inserted == 0 && !matches_input(&existing, &input) {
        return Err(Error::FocusSessionConflict);
    }
    Ok(existing)
}

/// Resumo de foco para o dia local atual do banco.
pub fn focus_summary(db: &Database, owner_id: &str) -> Result<FocusSummary> {
    let today: String =
        db.connection()
            .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    focus_summary_on(db, owner_id, &today)
}

/// Resumo de foco para um dia local ISO explícito (útil para testes e adaptadores).
pub fn focus_summary_on(db: &Database, owner_id: &str, today: &str) -> Result<FocusSummary> {
    validate_owner(owner_id)?;
    validate_day(db, today)?;
    let conn = db.connection();
    let today_number: i64 = conn.query_row(
        "SELECT CAST(julianday(?1) + 0.5 AS INTEGER)",
        [today],
        |row| row.get(0),
    )?;
    let weekday: i64 = conn.query_row(
        "SELECT CAST(strftime('%w', ?1) AS INTEGER)",
        [today],
        |row| row.get(0),
    )?;
    let monday = today_number - (weekday + 6) % 7;

    let mut days = Vec::with_capacity(7);
    for offset in 0..7 {
        let day: String = conn.query_row("SELECT date(?1)", [monday + offset], |row| row.get(0))?;
        let (seconds, sessions): (i64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(actual_seconds), 0), COUNT(*)
             FROM focus_session
             WHERE owner_id = ?1 AND kind = 'focus' AND completed = 1
               AND date(ended_at, 'unixepoch', 'localtime') = ?2",
            params![owner_id, day],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        days.push(FocusDay {
            day,
            minutes: seconds / 60,
            sessions,
        });
    }

    let today_minutes = days
        .iter()
        .find(|day| day.day == today)
        .map_or(0, |day| day.minutes);
    let week_minutes = days.iter().map(|day| day.minutes).sum();
    let week_sessions = days.iter().map(|day| day.sessions).sum();
    let best_day = days
        .iter()
        .filter(|day| day.minutes > 0)
        .max_by_key(|day| day.minutes)
        .map(|day| day.day.clone());
    let (completed, total): (i64, i64) = conn.query_row(
        "SELECT COUNT(*) FILTER (WHERE completed = 1), COUNT(*)
         FROM focus_session WHERE owner_id = ?1 AND kind = 'focus'",
        [owner_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let total_minutes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(actual_seconds), 0) / 60 FROM focus_session
         WHERE owner_id = ?1 AND kind = 'focus'",
        [owner_id],
        |row| row.get(0),
    )?;

    Ok(FocusSummary {
        today_minutes,
        week_minutes,
        week_sessions,
        total_minutes,
        total_sessions: completed,
        completion_rate: if total == 0 {
            0
        } else {
            completed * 100 / total
        },
        best_day,
        days,
    })
}

/// Resumo persistido de recompensas para o dia local atual do banco.
pub fn reward_summary(db: &Database, owner_id: &str) -> Result<RewardSummary> {
    let today: String =
        db.connection()
            .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    reward_summary_on(db, owner_id, &today)
}

/// Resgata uma missão do dia local atual do banco.
pub fn claim_daily_mission_today(db: &Database, owner_id: &str, mission_id: &str) -> Result<bool> {
    let today: String =
        db.connection()
            .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    claim_daily_mission(db, owner_id, mission_id, &today)
}

/// Retorna XP, progresso de missões e ofensiva do dono para o dia ISO informado.
pub fn reward_summary_on(db: &Database, owner_id: &str, today: &str) -> Result<RewardSummary> {
    validate_owner(owner_id)?;
    validate_day(db, today)?;
    let conn = db.connection();
    let xp = conn.query_row(
        "SELECT COALESCE(SUM(amount), 0) FROM xp_event WHERE owner_id = ?1",
        [owner_id],
        |row| row.get(0),
    )?;
    let completed_lessons = conn.query_row(
        "SELECT COUNT(DISTINCT media_id) FROM lesson_completion_event WHERE owner_id = ?1",
        [owner_id],
        |row| row.get(0),
    )?;
    let weekly_goal = weekly_goal_for_owner(conn, owner_id)?;
    let focus_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM focus_session
         WHERE owner_id = ?1 AND kind = 'focus' AND completed = 1
           AND date(ended_at, 'unixepoch', 'localtime') = ?2",
        params![owner_id, today],
        |row| row.get(0),
    )?;
    let focus_minutes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(actual_seconds), 0) / 60 FROM focus_session
         WHERE owner_id = ?1 AND kind = 'focus' AND completed = 1
           AND date(ended_at, 'unixepoch', 'localtime') = ?2",
        params![owner_id, today],
        |row| row.get(0),
    )?;
    let lesson_today: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT media_id) FROM lesson_completion_event
         WHERE owner_id = ?1 AND date(completed_at, 'unixepoch', 'localtime') = ?2",
        params![owner_id, today],
        |row| row.get(0),
    )?;
    let missions = [
        ("lesson", lesson_today, 1),
        ("focus-two", focus_today, 2),
        ("focus-fifty", focus_minutes, 50),
    ]
    .into_iter()
    .map(|(id, progress, target)| {
        let claimed = conn
            .query_row(
                "SELECT 1 FROM daily_mission_claim
                 WHERE owner_id = ?1 AND day = ?2 AND mission_id = ?3",
                params![owner_id, today, id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        Ok(DailyMission {
            id: id.to_owned(),
            progress: progress.min(target),
            target,
            claimed,
        })
    })
    .collect::<Result<Vec<_>>>()?;

    Ok(RewardSummary {
        xp,
        completed_lessons,
        weekly_goal,
        missions,
        study: study_summary_on(db, owner_id, today)?,
    })
}

/// Resgata uma missão uma vez por dono/dia. O progresso é verificado na própria instrução.
pub fn claim_daily_mission(
    db: &Database,
    owner_id: &str,
    mission_id: &str,
    today: &str,
) -> Result<bool> {
    validate_owner(owner_id)?;
    validate_day(db, today)?;
    let conn = db.connection();
    let progress_sql = match mission_id {
        "lesson" => {
            "SELECT COUNT(DISTINCT media_id) FROM lesson_completion_event
                    WHERE owner_id = ?1 AND date(completed_at, 'unixepoch', 'localtime') = ?2"
        }
        "focus-two" => {
            "SELECT COUNT(*) FROM focus_session
                        WHERE owner_id = ?1 AND kind = 'focus' AND completed = 1
                          AND date(ended_at, 'unixepoch', 'localtime') = ?2"
        }
        "focus-fifty" => {
            "SELECT COALESCE(SUM(actual_seconds), 0) / 60 FROM focus_session
                          WHERE owner_id = ?1 AND kind = 'focus' AND completed = 1
                            AND date(ended_at, 'unixepoch', 'localtime') = ?2"
        }
        _ => {
            return Err(Error::InvalidMission {
                mission_id: mission_id.to_owned(),
            });
        }
    };
    let target = match mission_id {
        "lesson" => 1,
        "focus-two" => 2,
        _ => 50,
    };
    let sql = format!(
        "INSERT INTO daily_mission_claim (owner_id, day, mission_id, claimed_at)
         SELECT ?1, ?2, ?3, ?4 WHERE ({progress_sql}) >= ?5
         ON CONFLICT (owner_id, day, mission_id) DO NOTHING"
    );
    let inserted = conn.execute(
        &sql,
        params![
            owner_id,
            today,
            mission_id,
            crate::db::now_unix_seconds(),
            target
        ],
    )?;
    if inserted > 0 {
        return Ok(true);
    }
    let claimed = conn.query_row(
        "SELECT 1 FROM daily_mission_claim WHERE owner_id = ?1 AND day = ?2 AND mission_id = ?3",
        params![owner_id, today, mission_id],
        |_| Ok(()),
    ).optional()?.is_some();
    if claimed {
        Ok(false)
    } else {
        Err(Error::MissionNotComplete)
    }
}

/// Lê a meta semanal configurada ou o padrão de três dias.
pub fn weekly_goal(db: &Database, owner_id: &str) -> Result<i64> {
    validate_owner(owner_id)?;
    weekly_goal_for_owner(db.connection(), owner_id)
}

/// Persiste a meta semanal entre um e sete dias; o trigger concede o bônus idempotente se
/// a meta da semana corrente já tiver sido atingida.
pub fn set_weekly_goal(db: &Database, owner_id: &str, goal: i64) -> Result<i64> {
    validate_owner(owner_id)?;
    if !(1..=7).contains(&goal) {
        return Err(Error::InvalidWeeklyGoal { weekly_goal: goal });
    }
    db.connection().execute(
        "INSERT INTO gamification_settings (owner_id, weekly_goal) VALUES (?1, ?2)
         ON CONFLICT (owner_id) DO UPDATE SET weekly_goal = excluded.weekly_goal",
        params![owner_id, goal],
    )?;
    Ok(goal)
}

fn validate_owner(owner_id: &str) -> Result<()> {
    if owner_id.trim().is_empty() {
        Err(Error::InvalidOwnerId)
    } else {
        Ok(())
    }
}

fn validate_day(db: &Database, day: &str) -> Result<()> {
    let valid: i64 = db.connection().query_row(
        "SELECT CASE WHEN date(?1) = ?1 THEN 1 ELSE 0 END",
        [day],
        |row| row.get(0),
    )?;
    if valid == 0 {
        Err(Error::InvalidStudyDay {
            day: day.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_day_and_id(
    id: &str,
    planned: i64,
    actual: i64,
    started: i64,
    ended: i64,
) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id.is_ascii()
        || id
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(Error::InvalidFocusSession {
            reason: "identificador inválido",
        });
    }
    if !(1..=7_200).contains(&planned) || !(0..=planned).contains(&actual) {
        return Err(Error::InvalidFocusSession {
            reason: "duração fora do limite",
        });
    }
    if started < 0 || ended < started || ended - started > 7_200 {
        return Err(Error::InvalidFocusSession {
            reason: "intervalo de tempo inválido",
        });
    }
    if actual > ended - started {
        return Err(Error::InvalidFocusSession {
            reason: "duração efetiva maior que o intervalo da sessão",
        });
    }
    Ok(())
}

fn weekly_goal_for_owner(conn: &rusqlite::Connection, owner_id: &str) -> Result<i64> {
    conn.query_row(
        "SELECT weekly_goal FROM gamification_settings WHERE owner_id = ?1",
        [owner_id],
        |row| row.get(0),
    )
    .optional()
    .map(|goal| goal.unwrap_or(3))
    .map_err(Error::from)
}

fn matches_input(session: &FocusSession, input: &FocusSessionInput) -> bool {
    session.id == input.id
        && session.kind == input.kind
        && session.planned_seconds == input.planned_seconds
        && session.actual_seconds == input.actual_seconds
        && session.completed == input.completed
        && session.started_at == input.started_at
        && session.ended_at == input.ended_at
}

fn focus_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FocusSession> {
    let kind: String = row.get(1)?;
    Ok(FocusSession {
        id: row.get(0)?,
        kind: match kind.as_str() {
            "focus" => FocusKind::Focus,
            "short" => FocusKind::ShortBreak,
            _ => FocusKind::LongBreak,
        },
        planned_seconds: row.get(2)?,
        actual_seconds: row.get(3)?,
        completed: row.get::<_, i64>(4)? != 0,
        started_at: row.get(5)?,
        ended_at: row.get(6)?,
    })
}
