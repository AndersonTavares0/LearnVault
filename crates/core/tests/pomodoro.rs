//! Persistência de sessões de foco e recompensas, sem acessar arquivos da biblioteca.

use learnvault_core::{
    Database, Error, FocusKind, FocusSessionInput, claim_daily_mission, focus_summary_on,
    record_focus_session, reward_summary_on, set_completed, set_weekly_goal,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const OWNER: &str = "aluno-1";
const OTHER: &str = "aluno-2";
const TODAY: &str = "2026-10-26";

fn db() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('test-library', 1)",
        [],
    )?;
    Ok(db)
}

fn insert_media(db: &Database, media_id: &str) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, 'test-library', ?2, ?2, 1, 1, 'available')",
        rusqlite::params![media_id, format!("{media_id}.mp4")],
    )
}

fn timestamp(day: &str, hour: &str) -> i64 {
    let db = Database::open_in_memory().expect("banco de teste");
    db.connection()
        .query_row("SELECT unixepoch(?1)", [format!("{day} {hour}")], |row| {
            row.get(0)
        })
        .expect("timestamp válido")
}

fn session(
    id: &str,
    kind: FocusKind,
    completed: bool,
    day: &str,
    actual_seconds: i64,
) -> FocusSessionInput {
    let started_at = timestamp(day, "12:00:00");
    FocusSessionInput {
        id: id.to_owned(),
        kind,
        planned_seconds: if kind == FocusKind::Focus {
            1_500
        } else {
            actual_seconds.max(1)
        },
        actual_seconds,
        completed,
        started_at,
        ended_at: started_at + actual_seconds,
    }
}

#[test]
fn a_completed_focus_session_is_persisted_and_idempotent() -> TestResult {
    let db = db()?;
    let input = session("session-1", FocusKind::Focus, true, TODAY, 1_500);

    let first = record_focus_session(&db, OWNER, input.clone())?;
    let repeated = record_focus_session(&db, OWNER, input)?;
    let summary = focus_summary_on(&db, OWNER, TODAY)?;
    let rewards = reward_summary_on(&db, OWNER, TODAY)?;

    assert_eq!(first, repeated);
    assert_eq!(summary.today_minutes, 25);
    assert_eq!(summary.total_minutes, 25);
    assert_eq!(summary.week_sessions, 1);
    assert_eq!(rewards.xp, 20);
    assert!(rewards.study.today_active);
    Ok(())
}

#[test]
fn reusing_a_session_id_with_different_data_is_rejected() -> TestResult {
    let db = db()?;
    record_focus_session(
        &db,
        OWNER,
        session("session-1", FocusKind::Focus, true, TODAY, 1_500),
    )?;

    let mut changed = session("session-1", FocusKind::Focus, true, TODAY, 1_500);
    changed.started_at += 1;
    changed.ended_at += 1;
    let result = record_focus_session(&db, OWNER, changed);

    assert!(matches!(result, Err(Error::FocusSessionConflict)));
    assert_eq!(focus_summary_on(&db, OWNER, TODAY)?.week_sessions, 1);
    Ok(())
}

#[test]
fn breaks_and_incomplete_focus_do_not_award_xp_or_activate_a_day() -> TestResult {
    let db = db()?;
    record_focus_session(
        &db,
        OWNER,
        session("short", FocusKind::ShortBreak, true, TODAY, 300),
    )?;
    record_focus_session(
        &db,
        OWNER,
        session("partial", FocusKind::Focus, false, TODAY, 600),
    )?;

    let summary = focus_summary_on(&db, OWNER, TODAY)?;
    let rewards = reward_summary_on(&db, OWNER, TODAY)?;
    assert_eq!(summary.total_sessions, 0);
    assert_eq!(rewards.xp, 0);
    assert!(!rewards.study.today_active);
    Ok(())
}

#[test]
fn sessions_and_rewards_are_isolated_by_owner() -> TestResult {
    let db = db()?;
    record_focus_session(
        &db,
        OWNER,
        session("same-id", FocusKind::Focus, true, TODAY, 1_500),
    )?;
    record_focus_session(
        &db,
        OTHER,
        session("same-id", FocusKind::Focus, true, TODAY, 1_500),
    )?;

    assert_eq!(focus_summary_on(&db, OWNER, TODAY)?.total_sessions, 1);
    assert_eq!(focus_summary_on(&db, OTHER, TODAY)?.total_sessions, 1);
    assert_eq!(reward_summary_on(&db, OWNER, TODAY)?.xp, 20);
    assert_eq!(reward_summary_on(&db, OTHER, TODAY)?.xp, 20);
    Ok(())
}

#[test]
fn daily_mission_claim_requires_progress_and_can_only_pay_once() -> TestResult {
    let db = db()?;
    assert!(matches!(
        claim_daily_mission(&db, OWNER, "focus-two", TODAY),
        Err(Error::MissionNotComplete)
    ));
    for id in ["focus-1", "focus-2"] {
        record_focus_session(
            &db,
            OWNER,
            session(id, FocusKind::Focus, true, TODAY, 1_500),
        )?;
    }

    assert!(claim_daily_mission(&db, OWNER, "focus-two", TODAY)?);
    assert!(!claim_daily_mission(&db, OWNER, "focus-two", TODAY)?);
    assert_eq!(reward_summary_on(&db, OWNER, TODAY)?.xp, 70);
    Ok(())
}

#[test]
fn lesson_xp_is_one_time_but_completion_events_feed_the_daily_mission() -> TestResult {
    let db = db()?;
    insert_media(&db, "lesson-1")?;
    set_completed(&db, OWNER, "lesson-1", true)?;
    set_completed(&db, OWNER, "lesson-1", false)?;
    set_completed(&db, OWNER, "lesson-1", true)?;

    let before_claim = reward_summary_on(&db, OWNER, TODAY)?;
    assert_eq!(before_claim.xp, 40);
    assert_eq!(before_claim.completed_lessons, 1);
    assert_eq!(before_claim.missions[0].progress, 1);
    assert!(claim_daily_mission(&db, OWNER, "lesson", TODAY)?);
    assert_eq!(reward_summary_on(&db, OWNER, TODAY)?.xp, 70);
    Ok(())
}

#[test]
fn weekly_goal_is_owner_scoped_and_awarded_only_once_per_week() -> TestResult {
    let db = db()?;
    let today: String =
        db.connection()
            .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    db.connection().execute(
        "INSERT INTO study_day (owner_id, day, last_at) VALUES (?1, ?2, 1)",
        rusqlite::params![OWNER, today],
    )?;

    assert_eq!(set_weekly_goal(&db, OWNER, 1)?, 1);
    assert_eq!(reward_summary_on(&db, OWNER, &today)?.xp, 50);

    record_focus_session(
        &db,
        OWNER,
        session("weekly-focus-1", FocusKind::Focus, true, &today, 1_500),
    )?;
    record_focus_session(
        &db,
        OWNER,
        session("weekly-focus-2", FocusKind::Focus, true, &today, 1_500),
    )?;
    assert_eq!(reward_summary_on(&db, OWNER, &today)?.xp, 90);
    assert_eq!(reward_summary_on(&db, OTHER, &today)?.xp, 0);
    assert!(matches!(
        set_weekly_goal(&db, OWNER, 8),
        Err(Error::InvalidWeeklyGoal { weekly_goal: 8 })
    ));
    Ok(())
}

#[test]
fn invalid_focus_payload_is_rejected_before_persistence() -> TestResult {
    let db = db()?;
    let mut invalid = session("bad", FocusKind::Focus, true, TODAY, 1_500);
    invalid.actual_seconds = 1_499;
    invalid.ended_at -= 1;

    assert!(matches!(
        record_focus_session(&db, OWNER, invalid),
        Err(Error::InvalidFocusSession { .. })
    ));
    assert_eq!(focus_summary_on(&db, OWNER, TODAY)?.week_sessions, 0);
    Ok(())
}
