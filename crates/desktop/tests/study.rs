//! Testes da fronteira IPC de progresso de estudo do desktop.
//!
//! Não sobem o webview nem dependem de display: exercitam as funções livres
//! `DesktopState::study_progress`, `record_position`, `set_completed` e `recent_study`, que
//! os comandos `#[tauri::command]` apenas embrulham. O alvo é o contrato observável pelo
//! frontend — shape snake_case, códigos de erro estáveis, mensagens sem identificadores nem
//! caminhos — e as regras que o adaptador **não** pode reimplementar: dono local fixo,
//! persistência, independência entre posição e conclusão, teto do núcleo e ausência de
//! qualquer escrita no catálogo ou na biblioteca física.

use std::path::{Path, PathBuf};

use learnvault_core::{FocusKind, FocusSessionInput};
use learnvault_desktop::{
    DEFAULT_RECENT_LIMIT, DesktopState, IpcErrorCode, LOCAL_OWNER_ID, RecentStudyDto,
    StudyProgressDto,
};

/// Estado desktop isolado em diretórios temporários, com dados e configuração separados.
fn desktop(root: &Path) -> DesktopState {
    DesktopState::open(&root.join("data"), &root.join("config")).unwrap()
}

/// Caminho do banco local usado pelo estado.
fn database_path(root: &Path) -> PathBuf {
    root.join("data").join("learnvault.sqlite")
}

/// Segunda conexão ao banco local, como os testes de rescan fazem.
///
/// Serve para semear mídia (que no aplicativo vem do rescan) e para afirmar o que foi
/// persistido sem passar pelo adaptador sob teste. O diretório de dados é criado por
/// [`DesktopState::open`], então os testes precisam abrir o estado antes de semear.
fn local(root: &Path) -> learnvault_core::Database {
    learnvault_core::Database::open(database_path(root)).unwrap()
}

/// Semeia a biblioteca local e uma mídia conhecida, como uma varredura faria.
///
/// A disponibilidade é sempre `available`; o teste que precisa de mídia ausente chama
/// [`seed_media_with_availability`].
fn seed_media(root: &Path, media_id: &str) {
    seed_media_with_availability(root, media_id, "available");
}

/// Semeia a biblioteca local e uma mídia conhecida com disponibilidade explícita.
///
/// Os valores são controlados pelo teste e restritos a ASCII minúsculo, como nos testes do
/// servidor: o SQL é montado por `execute_batch` porque o crate desktop não depende de
/// `rusqlite` diretamente.
fn seed_media_with_availability(root: &Path, media_id: &str, availability: &str) {
    let database = local(root);
    let path = format!("acervo/{media_id}.mp4");
    database
        .connection()
        .execute_batch(&format!(
            "INSERT OR IGNORE INTO library (library_id, available) VALUES ('library-1', 1);
             INSERT INTO media
                 (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('{media_id}', 'library-1', '{path}', '{path}', 10, 0, '{availability}');"
        ))
        .unwrap();
}

/// Semeia uma linha de progresso com instante explícito, para controlar a ordenação.
///
/// Reescreve a linha do par dono/mídia: a chave primária composta não admite duplicata, e o
/// teste de ordenação precisa mover o instante da mesma mídia.
fn seed_state(
    root: &Path,
    owner_id: &str,
    media_id: &str,
    position: i64,
    completed: bool,
    at: i64,
) {
    let database = local(root);
    database
        .connection()
        .execute_batch(&format!(
            "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
             VALUES ('{owner_id}', '{media_id}', {position}, {}, {at})
             ON CONFLICT (owner_id, media_id) DO UPDATE SET
                 position_seconds = excluded.position_seconds,
                 completed = excluded.completed,
                 updated_at = excluded.updated_at;",
            i64::from(completed)
        ))
        .unwrap();
}

/// Dono gravado para uma mídia, lido direto do banco local.
fn stored_owner(root: &Path, media_id: &str) -> Option<String> {
    local(root)
        .connection()
        .query_row(
            "SELECT owner_id FROM study_state WHERE media_id = ?1",
            [media_id],
            |row| row.get(0),
        )
        .ok()
}

/// Total de linhas de progresso no banco local.
fn state_rows(root: &Path) -> i64 {
    local(root)
        .connection()
        .query_row("SELECT COUNT(*) FROM study_state", [], |row| row.get(0))
        .unwrap()
}

/// Contagem de linhas de uma tabela do catálogo, para provar que o estudo não a toca.
fn table_rows(root: &Path, table: &str) -> i64 {
    local(root)
        .connection()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

/// Nomes dos itens do histórico recente, na ordem devolvida pelo adaptador.
fn recent_ids(recent: &RecentStudyDto) -> Vec<String> {
    recent
        .items
        .iter()
        .map(|item| item.media_id.clone())
        .collect()
}

#[test]
fn known_media_without_progress_is_reported_as_not_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    let progress = state.study_progress("m1").unwrap();

    assert!(!progress.recorded);
    assert_eq!(progress.media_id, "m1");
    assert_eq!(progress.position_seconds, 0);
    assert!(!progress.completed);
    assert_eq!(progress.updated_at, None);
    assert_eq!(
        serde_json::to_value(&progress).unwrap(),
        serde_json::json!({
            "media_id": "m1",
            "position_seconds": 0,
            "completed": false,
            "updated_at": null,
            "recorded": false
        }),
        "o payload precisa usar o mesmo contrato snake_case do adaptador HTTP"
    );
    assert_eq!(
        state_rows(dir.path()),
        0,
        "uma leitura não pode criar estado de estudo"
    );
}

#[test]
fn unknown_media_is_a_stable_error_without_the_identifier() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let secret = "Acervo privado/aula.mp4";

    let error = state.study_progress(secret).unwrap_err();

    assert_eq!(error.code, IpcErrorCode::MediaNotFound);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "media_not_found");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(
        !message.contains(secret)
            && !message.contains("Acervo privado")
            && !message.contains("aula"),
        "a mensagem não pode ecoar o identificador consultado: {message}"
    );
}

#[test]
fn mutations_refuse_an_unknown_media_without_writing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());

    let recorded = state.record_position("desconhecida", 42).unwrap_err();
    let completed = state.set_completed("desconhecida", true).unwrap_err();

    assert_eq!(recorded.code, IpcErrorCode::MediaNotFound);
    assert_eq!(completed.code, IpcErrorCode::MediaNotFound);
    assert_eq!(
        state_rows(dir.path()),
        0,
        "recusar uma mídia desconhecida não pode gravar estado"
    );
}

#[test]
fn a_negative_position_is_a_stable_error_without_writing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    let error = state.record_position("m1", -1).unwrap_err();

    assert_eq!(error.code, IpcErrorCode::InvalidPosition);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "invalid_position");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(
        !message.contains("m1"),
        "a mensagem não pode ecoar o identificador: {message}"
    );
    assert_eq!(state_rows(dir.path()), 0);
}

#[test]
fn progress_is_persisted_under_the_fixed_local_owner() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    let progress = state.record_position("m1", 42).unwrap();

    assert_eq!(progress.position_seconds, 42);
    assert!(progress.recorded);
    assert!(
        !LOCAL_OWNER_ID.is_empty(),
        "o dono local precisa ser um identificador real"
    );
    assert_eq!(
        stored_owner(dir.path(), "m1").as_deref(),
        Some(LOCAL_OWNER_ID),
        "o dono gravado é o identificador local fixo do Rust"
    );

    // O isolamento por dono continua sendo do núcleo: um dono diferente não enxerga o
    // estado gravado pelo desktop.
    let database = local(dir.path());
    let other = learnvault_core::study_progress(&database, "outro-dono", "m1").unwrap();
    let own = learnvault_core::study_progress(&database, LOCAL_OWNER_ID, "m1").unwrap();
    assert!(other.is_none(), "outro dono não pode ler o progresso local");
    assert_eq!(own.map(|state| state.position_seconds), Some(42));
}

#[test]
fn recent_study_only_returns_the_local_owner() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");
    seed_media(dir.path(), "m2");
    seed_state(dir.path(), LOCAL_OWNER_ID, "m1", 10, false, 300);
    seed_state(dir.path(), "outro-dono", "m2", 20, true, 400);

    let recent = state.recent_study(None).unwrap();

    assert_eq!(
        recent_ids(&recent),
        ["m1"],
        "o histórico é sempre do dono local fixo: {recent:?}"
    );
    assert!(recent.items[0].recorded);
    let json = serde_json::to_value(&recent).unwrap();
    assert_eq!(json["items"][0]["media_id"], "m1");
    assert_eq!(json["items"][0]["position_seconds"], 10);
    assert_eq!(json["items"][0]["completed"], false);
    assert_eq!(json["items"][0]["updated_at"], 300);
    assert_eq!(json["items"][0]["recorded"], true);
}

#[test]
fn position_and_completion_stay_independent_through_the_adapter() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    state.record_position("m1", 42).unwrap();
    let completed = state.set_completed("m1", true).unwrap();
    assert_eq!(
        (completed.position_seconds, completed.completed),
        (42, true),
        "concluir não pode mover o ponto de retomada"
    );

    let advanced = state.record_position("m1", 90).unwrap();
    assert_eq!(
        (advanced.position_seconds, advanced.completed),
        (90, true),
        "avançar não pode desfazer a conclusão"
    );

    let cleared = state.set_completed("m1", false).unwrap();
    assert_eq!(
        (cleared.position_seconds, cleared.completed),
        (90, false),
        "desmarcar não pode mover a posição"
    );
}

#[test]
fn repeating_the_same_value_does_not_move_the_timestamp() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    let first = state.record_position("m1", 42).unwrap();
    let repeated = state.record_position("m1", 42).unwrap();
    let same_completion = state.set_completed("m1", false).unwrap();

    assert_eq!(
        repeated.updated_at, first.updated_at,
        "reenviar a mesma posição não é uma alteração real"
    );
    assert_eq!(
        same_completion.updated_at, first.updated_at,
        "reenviar a mesma conclusão não é uma alteração real"
    );
}

#[test]
fn progress_survives_a_restart_of_the_application() {
    let dir = tempfile::tempdir().unwrap();
    let first = desktop(dir.path());
    seed_media(dir.path(), "m1");
    first.record_position("m1", 42).unwrap();
    first.set_completed("m1", true).unwrap();
    drop(first);

    let restarted = desktop(dir.path());
    let progress = restarted.study_progress("m1").unwrap();

    assert!(progress.recorded);
    assert_eq!((progress.position_seconds, progress.completed), (42, true));
    assert_eq!(recent_ids(&restarted.recent_study(None).unwrap()), ["m1"]);
}

#[test]
fn study_works_without_a_configured_library_root() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    assert!(
        !state.config_status().configured,
        "o estudo não pode depender de uma raiz de biblioteca configurada"
    );
    let progress = state.record_position("m1", 7).unwrap();

    assert_eq!(progress.position_seconds, 7);
}

#[test]
fn a_media_marked_missing_still_accepts_reading_and_writing_progress() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media_with_availability(dir.path(), "m1", "missing");

    let written = state.record_position("m1", 12).unwrap();
    let read = state.study_progress("m1").unwrap();

    assert_eq!(written.position_seconds, 12);
    assert_eq!(read.position_seconds, 12);
    assert!(
        read.recorded,
        "a identidade da mídia sobrevive à ausência temporária do arquivo"
    );
}

#[test]
fn study_never_writes_the_catalog_or_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");
    seed_media(dir.path(), "m2");
    let before = (
        table_rows(dir.path(), "media"),
        table_rows(dir.path(), "scan"),
        table_rows(dir.path(), "library"),
    );

    state.record_position("m1", 42).unwrap();
    state.set_completed("m1", true).unwrap();
    state.set_completed("m2", true).unwrap();
    state.recent_study(Some(10)).unwrap();

    let after = (
        table_rows(dir.path(), "media"),
        table_rows(dir.path(), "scan"),
        table_rows(dir.path(), "library"),
    );
    assert_eq!(
        before, after,
        "o estudo não pode inserir, apagar nem reescrever catálogo, varredura ou biblioteca"
    );
    assert_eq!(state_rows(dir.path()), 2);
}

#[test]
fn recent_study_is_ordered_by_the_last_real_change_with_a_deterministic_tie_break() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    for media_id in ["m1", "m2", "m3"] {
        seed_media(dir.path(), media_id);
    }
    seed_state(dir.path(), LOCAL_OWNER_ID, "m1", 10, false, 300);
    seed_state(dir.path(), LOCAL_OWNER_ID, "m2", 20, true, 200);
    seed_state(dir.path(), LOCAL_OWNER_ID, "m3", 30, false, 100);

    assert_eq!(
        recent_ids(&state.recent_study(None).unwrap()),
        ["m1", "m2", "m3"]
    );

    // Um empate de instantes é desfeito pelo `media_id`, então a ordem é determinística.
    seed_state(dir.path(), LOCAL_OWNER_ID, "m1", 11, false, 500);
    seed_state(dir.path(), LOCAL_OWNER_ID, "m2", 21, true, 500);
    assert_eq!(
        recent_ids(&state.recent_study(None).unwrap()),
        ["m1", "m2", "m3"]
    );
}

#[test]
fn recent_study_honours_the_requested_limit_and_the_core_cap() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let total = learnvault_core::MAX_RECENT_STUDY + 5;
    for index in 0..total {
        let media_id = format!("m{index:03}");
        seed_media(dir.path(), &media_id);
        seed_state(
            dir.path(),
            LOCAL_OWNER_ID,
            &media_id,
            index as i64,
            false,
            1_000,
        );
    }

    assert!(state.recent_study(Some(0)).unwrap().items.is_empty());
    assert_eq!(state.recent_study(Some(2)).unwrap().items.len(), 2);
    assert_eq!(
        state.recent_study(None).unwrap().items.len(),
        DEFAULT_RECENT_LIMIT,
        "sem limite informado, o adaptador aplica o padrão do contrato"
    );
    assert_eq!(
        state.recent_study(Some(usize::MAX)).unwrap().items.len(),
        learnvault_core::MAX_RECENT_STUDY,
        "o teto do núcleo continua valendo para qualquer limite recebido"
    );
}

#[test]
fn an_empty_history_is_an_empty_list_and_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());

    let recent = state.recent_study(Some(6)).unwrap();

    assert!(recent.items.is_empty());
    assert_eq!(
        serde_json::to_value(&recent).unwrap(),
        serde_json::json!({ "items": [] })
    );
}

#[test]
fn concurrent_mutations_of_one_media_leave_a_single_consistent_row() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");
    let positions = [10_i64, 20, 30, 40, 50, 60, 70, 80];

    std::thread::scope(|scope| {
        for position in positions {
            let state = state.clone();
            scope.spawn(move || {
                state
                    .record_position("m1", position)
                    .expect("gravar posição não pode falhar sob concorrência");
            });
        }
    });

    let progress = state.study_progress("m1").unwrap();
    assert_eq!(
        state_rows(dir.path()),
        1,
        "a chave composta (owner_id, media_id) não pode duplicar a linha"
    );
    assert!(
        positions.contains(&progress.position_seconds),
        "a posição final precisa ser uma das gravadas: {}",
        progress.position_seconds
    );
}

#[test]
fn concurrent_mutations_of_distinct_media_are_all_persisted() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let total = 8_usize;
    for index in 0..total {
        seed_media(dir.path(), &format!("m{index}"));
    }

    std::thread::scope(|scope| {
        for index in 0..total {
            let state = state.clone();
            scope.spawn(move || {
                let media_id = format!("m{index}");
                state
                    .record_position(&media_id, index as i64)
                    .expect("gravar posição não pode falhar sob concorrência");
                state
                    .set_completed(&media_id, true)
                    .expect("concluir não pode falhar sob concorrência");
            });
        }
    });

    let recent = state.recent_study(None).unwrap();
    assert_eq!(
        recent.items.len(),
        total,
        "todas as gravações precisam sobreviver: {recent:?}"
    );
    assert!(
        recent.items.iter().all(|item| item.completed),
        "as conclusões gravadas em paralelo precisam persistir"
    );
}

#[test]
fn concurrent_reads_and_writes_do_not_corrupt_the_state() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    let observed: Vec<Result<StudyProgressDto, IpcErrorCode>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let state = state.clone();
                scope.spawn(move || state.study_progress("m1").map_err(|error| error.code))
            })
            .chain((0..4).map(|index| {
                let state = state.clone();
                scope.spawn(move || {
                    state
                        .record_position("m1", 10 + index)
                        .map_err(|error| error.code)
                })
            }))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a thread não pode entrar em pânico"))
            .collect()
    });

    for outcome in observed {
        assert!(
            outcome.is_ok(),
            "ler e gravar não podem falhar: {:?}",
            outcome.err()
        );
    }
    assert_eq!(state_rows(dir.path()), 1);
}

#[test]
fn summary_reports_the_local_owner_streak_and_week() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    seed_state(dir.path(), LOCAL_OWNER_ID, "m1", 10, false, now);

    let summary = state.study_summary().unwrap();
    assert!(summary.today_active);
    assert_eq!(summary.current_streak, 1);
    assert_eq!(summary.longest_streak, 1);
    assert_eq!(summary.week_active_days, 1);
    assert_eq!(summary.completed_lessons, 0);
    assert_eq!(summary.days.len(), 7);
    assert_eq!(summary.today.len(), 10);
}

#[test]
fn focus_rewards_and_mission_claims_persist_for_the_local_owner() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let session = FocusSessionInput {
        id: "session-1".into(),
        kind: FocusKind::Focus,
        planned_seconds: 1_500,
        actual_seconds: 1_500,
        completed: true,
        started_at: now - 1_500,
        ended_at: now,
    };

    let first = state.record_focus_session(session.clone()).unwrap();
    let duplicate = state.record_focus_session(session).unwrap();

    assert_eq!(
        first, duplicate,
        "retries devem devolver a sessão existente"
    );
    assert_eq!(serde_json::to_value(&first).unwrap()["kind"], "focus");
    let focus = state.focus_summary().unwrap();
    assert_eq!(focus.total_sessions, 1);
    assert_eq!(focus.total_minutes, 25);
    assert_eq!(focus.completion_rate, 100);
    assert_eq!(focus.week_sessions, 1);
    let rewards = state.reward_summary().unwrap();
    assert_eq!(rewards.xp, 20);
    assert!(rewards.study.today_active);
    assert_eq!(rewards.missions[1].progress, 1);

    assert_eq!(state.set_weekly_goal(1).unwrap(), 1);
    assert_eq!(state.reward_summary().unwrap().xp, 70);
}

#[test]
fn desktop_mission_claim_is_one_time_and_reward_summary_uses_snake_case() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    for index in 0..2 {
        state
            .record_focus_session(FocusSessionInput {
                id: format!("mission-session-{index}"),
                kind: FocusKind::Focus,
                planned_seconds: 1_500,
                actual_seconds: 1_500,
                completed: true,
                started_at: now - 3_000 + index * 1_500,
                ended_at: now - 1_500 + index * 1_500,
            })
            .unwrap();
    }

    assert!(state.claim_daily_mission("focus-two").unwrap());
    assert!(!state.claim_daily_mission("focus-two").unwrap());
    let summary = state.reward_summary().unwrap();
    assert_eq!(summary.xp, 70, "20 XP por sessão e 30 pelo resgate");
    assert!(summary.missions[1].claimed);
    let json = serde_json::to_value(summary).unwrap();
    assert_eq!(json["weekly_goal"], 3);
    assert_eq!(json["missions"][1]["claimed"], true);
    assert!(json.get("study").is_some());
}
