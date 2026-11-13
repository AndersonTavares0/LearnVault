//! Playback preferences: defaults, bounds, owner isolation and upsert.

use learnvault_core::{Database, Error, player_preference, set_player_preference};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const OWNER: &str = "aluno-1";
const OTHER: &str = "aluno-2";

#[test]
fn a_missing_preference_reads_neutral_defaults() -> TestResult {
    let db = Database::open_in_memory()?;

    let preference = player_preference(&db, OWNER)?;

    assert_eq!(preference.video_speed, 100);
    assert_eq!(preference.audio_speed, 100);
    assert_eq!(preference.subtitle_language, "");
    assert_eq!(preference.updated_at, 0);
    Ok(())
}

#[test]
fn saves_round_trip_per_owner_and_update_in_place() -> TestResult {
    let db = Database::open_in_memory()?;

    let saved = set_player_preference(&db, OWNER, 150, 125, "pt-BR")?;
    assert_eq!((saved.video_speed, saved.audio_speed), (150, 125));
    assert_eq!(saved.subtitle_language, "pt-BR");

    // A second save replaces the row instead of duplicating it.
    let updated = set_player_preference(&db, OWNER, 75, 100, "")?;
    assert_eq!(updated.video_speed, 75);
    assert_eq!(updated.subtitle_language, "");

    assert_eq!(player_preference(&db, OWNER)?, updated);
    // Another owner never sees these values.
    assert_eq!(player_preference(&db, OTHER)?.video_speed, 100);
    let rows: i64 =
        db.connection()
            .query_row("SELECT COUNT(*) FROM player_preference", [], |row| {
                row.get(0)
            })?;
    assert_eq!(rows, 1);
    Ok(())
}

#[test]
fn rejects_speeds_outside_the_supported_bounds_and_invalid_languages() -> TestResult {
    let db = Database::open_in_memory()?;

    for speed in [49, 301, -1] {
        assert!(matches!(
            set_player_preference(&db, OWNER, speed, 100, ""),
            Err(Error::InvalidPlayerPreference)
        ));
    }
    for language in ["pt BR", "en\n", &"a".repeat(33)] {
        assert!(matches!(
            set_player_preference(&db, OWNER, 100, 100, language),
            Err(Error::InvalidPlayerPreference)
        ));
    }
    // Empty means "no preference" and is accepted.
    assert!(set_player_preference(&db, OWNER, 50, 300, "").is_ok());
    Ok(())
}
