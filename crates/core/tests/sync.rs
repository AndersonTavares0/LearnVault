//! Contratos do diário de sincronização, do outbox local e das regras de conflito.

use learnvault_core::{
    Database, ReviewRating, SyncOperation, SyncOperationInput, SyncPayload, acknowledge_operations,
    apply_operations, enqueue_operation, pending_count, pending_operations, pull_operations,
    push_operations, queue_bookmark_create, queue_bookmark_delete, queue_flashcard_create,
    queue_flashcard_delete, queue_note_create, queue_note_delete, queue_position, queue_review,
    set_completed, sync_state,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const OWNER: &str = "desktop-local";

fn database() -> Database {
    let db = Database::open_in_memory().expect("banco em memória deve abrir");
    db.connection()
        .execute_batch(
            "INSERT INTO library (library_id, available) VALUES ('library-1', 1);
             INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('m1', 'library-1', 'curso/aula.mp4', 'curso/aula.mp4', 1024, 1760000000, 'available');",
        )
        .expect("catálogo mínimo de teste");
    db
}

fn progress(media_id: &str, position_seconds: i64, completed: bool) -> SyncPayload {
    SyncPayload::Progress {
        media_id: media_id.to_owned(),
        position_seconds,
        completed,
    }
}

fn note(note_id: &str, content: &str, updated_at: i64) -> SyncPayload {
    SyncPayload::Note {
        note_id: note_id.to_owned(),
        media_id: "m1".to_owned(),
        position_seconds: 10,
        content: content.to_owned(),
        created_at: 1_000,
        updated_at,
    }
}

fn input(op_id: &str, client_at: i64, payload: SyncPayload) -> SyncOperationInput {
    SyncOperationInput {
        op_id: op_id.to_owned(),
        client_at,
        payload,
    }
}

fn card_created(card_id: &str, front: &str, back: &str, created_at: i64) -> SyncPayload {
    SyncPayload::FlashcardCreated {
        card_id: card_id.to_owned(),
        media_id: "m1".to_owned(),
        position_seconds: Some(10),
        front: front.to_owned(),
        back: back.to_owned(),
        created_at,
    }
}

fn card_updated(card_id: &str, front: &str, back: &str, updated_at: i64) -> SyncPayload {
    SyncPayload::FlashcardUpdated {
        card_id: card_id.to_owned(),
        front: front.to_owned(),
        back: back.to_owned(),
        updated_at,
    }
}

fn card_front(db: &Database, card_id: &str) -> String {
    db.connection()
        .query_row(
            "SELECT front FROM flashcard WHERE card_id = ?1",
            [card_id],
            |row| row.get(0),
        )
        .expect("o cartão deve existir")
}

fn count(db: &Database, table: &str) -> i64 {
    db.connection()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("a contagem deve ser consultável")
}

#[test]
fn replaying_the_same_operation_applies_it_once() -> TestResult {
    let db = database();
    let operation = input("op-1", 100, progress("m1", 42, false));

    let first = push_operations(&db, OWNER, "device-a", std::slice::from_ref(&operation))?;
    assert_eq!((first.applied, first.duplicated), (1, 0));
    let second = push_operations(&db, OWNER, "device-a", &[operation])?;
    assert_eq!(
        (second.applied, second.duplicated),
        (0, 1),
        "o mesmo op_id é uma duplicata"
    );

    assert_eq!(count(&db, "sync_operation"), 1);
    assert_eq!(count(&db, "study_state"), 1);
    let position: i64 = db.connection().query_row(
        "SELECT position_seconds FROM study_state WHERE owner_id = ?1 AND media_id = 'm1'",
        [OWNER],
        |row| row.get(0),
    )?;
    assert_eq!(position, 42);
    Ok(())
}

#[test]
fn an_operation_id_from_another_owner_is_refused() -> TestResult {
    let db = database();
    db.connection().execute_batch(
        "INSERT INTO user (user_id, username, password_hash, is_admin, created_at)
         VALUES ('intruder', 'intruder', 'x', 0, 1);",
    )?;
    let operation = input("op-1", 100, progress("m1", 42, false));
    push_operations(&db, OWNER, "device-a", std::slice::from_ref(&operation))?;

    let refused = push_operations(&db, "intruder", "device-z", &[operation]);
    assert!(
        matches!(
            refused,
            Err(learnvault_core::Error::InvalidSyncOperation { .. })
        ),
        "um op_id de outro dono não pode virar duplicata silenciosa"
    );
    assert_eq!(count(&db, "sync_operation"), 1);
    Ok(())
}

#[test]
fn pull_returns_new_operations_in_order_and_skips_the_own_device() -> TestResult {
    let db = database();
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[
            input("op-1", 100, progress("m1", 10, false)),
            input("op-2", 110, progress("m1", 20, false)),
            input("op-3", 120, progress("m1", 30, false)),
        ],
    )?;

    let first = pull_operations(&db, OWNER, "device-b", 0, 2)?;
    assert_eq!(first.items.len(), 2);
    assert!(first.has_more, "a terceira operação ainda não veio");
    assert!(first.items[0].seq < first.items[1].seq);

    let second = pull_operations(&db, OWNER, "device-b", first.cursor, 2)?;
    assert_eq!(second.items.len(), 1);
    assert!(!second.has_more);
    assert_eq!(second.items[0].op_id, "op-3");

    let own = pull_operations(&db, OWNER, "device-a", 0, 10)?;
    assert!(
        own.items.is_empty(),
        "o dispositivo não deve receber as próprias operações"
    );

    let empty = pull_operations(&db, OWNER, "device-b", second.cursor, 10)?;
    assert!(empty.items.is_empty());
    assert_eq!(empty.cursor, second.cursor);
    Ok(())
}

#[test]
fn pull_is_isolated_per_owner() -> TestResult {
    let db = database();
    push_operations(
        &db,
        "owner-a",
        "device-a",
        &[input("op-a", 100, progress("m1", 10, false))],
    )?;
    push_operations(
        &db,
        "owner-b",
        "device-b",
        &[input("op-b", 100, progress("m1", 20, false))],
    )?;

    let a = pull_operations(&db, "owner-a", "device-b", 0, 10)?;
    assert_eq!(a.items.len(), 1);
    assert_eq!(a.items[0].op_id, "op-a");
    let b = pull_operations(&db, "owner-b", "device-a", 0, 10)?;
    assert_eq!(b.items.len(), 1);
    assert_eq!(b.items[0].op_id, "op-b");
    Ok(())
}

#[test]
fn progress_is_last_write_wins_with_clamping_and_monotonic_completion() -> TestResult {
    let db = database();
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-1", 100, progress("m1", 120, false))],
    )?;

    // Escrita mais nova vence.
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-2", 200, progress("m1", 30, false))],
    )?;
    // Escrita antiga não retrocede a posição.
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-3", 150, progress("m1", 900, false))],
    )?;
    let (position, completed): (i64, i64) = db.connection().query_row(
        "SELECT position_seconds, completed FROM study_state WHERE owner_id = ?1",
        [OWNER],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!((position, completed), (30, 0));

    // Concluir com um instante antigo ainda preserva a conclusão observada.
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-4", 50, progress("m1", 30, true))],
    )?;
    let (position, completed): (i64, i64) = db.connection().query_row(
        "SELECT position_seconds, completed FROM study_state WHERE owner_id = ?1",
        [OWNER],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!((position, completed), (30, 1));

    // Conclusão é monotônica: nem uma escrita mais nova desmarca o que já foi observado,
    // e uma escrita antiga posterior não retrocede a posição.
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-5", 400, progress("m1", 40, false))],
    )?;
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-6", 300, progress("m1", 100, false))],
    )?;
    let (position, completed): (i64, i64) = db.connection().query_row(
        "SELECT position_seconds, completed FROM study_state WHERE owner_id = ?1",
        [OWNER],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!((position, completed), (40, 1));

    // Valores fora da faixa são limitados, não recusados.
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-7", 500, progress("m1", -50, false))],
    )?;
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-8", 600, progress("m1", i64::MAX, false))],
    )?;
    let position: i64 = db.connection().query_row(
        "SELECT position_seconds FROM study_state WHERE owner_id = ?1",
        [OWNER],
        |row| row.get(0),
    )?;
    assert_eq!(position, learnvault_core::MAX_SYNC_POSITION_SECONDS);
    Ok(())
}

#[test]
fn divergent_note_versions_keep_a_conflict_copy() -> TestResult {
    let db = database();
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-1", 100, note("note-1", "primeiro", 100))],
    )?;
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-2", 200, note("note-1", "segundo", 200))],
    )?;

    let main: (String, String) = db.connection().query_row(
        "SELECT content, sync_version FROM study_note WHERE note_id = 'note-1'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(main.0, "segundo");
    assert_eq!(main.1, "op-2");
    let copy: String = db.connection().query_row(
        "SELECT content FROM study_note WHERE note_id = 'note-1~op-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(copy, "primeiro", "o texto antigo nunca se perde");
    assert_eq!(count(&db, "study_note"), 2);

    // Replay e conteúdo idêntico não criam cópias novas.
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-2", 200, note("note-1", "segundo", 200))],
    )?;
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-3", 300, note("note-1", "segundo", 300))],
    )?;
    assert_eq!(count(&db, "study_note"), 2);

    // Uma versão mais antiga também sobrevive como cópia.
    push_operations(
        &db,
        OWNER,
        "device-c",
        &[input("op-0", 300, note("note-1", "terceiro", 300))],
    )?;
    let main: String = db.connection().query_row(
        "SELECT content FROM study_note WHERE note_id = 'note-1'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(main, "segundo");
    let older: String = db.connection().query_row(
        "SELECT content FROM study_note WHERE note_id = 'note-1~op-0'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(older, "terceiro");
    Ok(())
}

#[test]
fn flashcard_reviews_are_append_only() -> TestResult {
    let db = database();
    let card = learnvault_core::create_flashcard(&db, OWNER, "m1", "frente", "verso", None)?;
    let review = |rating: &str, reviewed_at: i64| SyncPayload::FlashcardReview {
        card_id: card.card_id.clone(),
        rating: rating.to_owned(),
        reviewed_at,
    };

    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-1", 100, review("good", 100))],
    )?;
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-1", 100, review("good", 100))],
    )?;
    assert_eq!(count(&db, "flashcard_review"), 1, "replay não duplica");

    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-2", 200, review("again", 200))],
    )?;
    assert_eq!(count(&db, "flashcard_review"), 2);
    let (interval, due): (i64, i64) = db.connection().query_row(
        "SELECT interval_days, next_due FROM flashcard WHERE card_id = ?1",
        [&card.card_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!((interval, due), (0, 800), "again reagenda em 10 minutos");
    Ok(())
}

#[test]
fn preferences_are_last_write_wins_with_clamped_speeds() -> TestResult {
    let db = database();
    let preference = |video: i64, audio: i64, language: &str| SyncPayload::Preference {
        video_speed: video,
        audio_speed: audio,
        subtitle_language: language.to_owned(),
    };

    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-1", 100, preference(100, 100, ""))],
    )?;
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-2", 200, preference(400, 10, "pt-BR"))],
    )?;
    let stored: (i64, i64, String) = db.connection().query_row(
        "SELECT video_speed, audio_speed, subtitle_language FROM player_preference WHERE owner_id = ?1",
        [OWNER],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(stored, (300, 50, "pt-BR".to_owned()));

    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-3", 150, preference(200, 200, "en"))],
    )?;
    let speed: i64 = db.connection().query_row(
        "SELECT video_speed FROM player_preference WHERE owner_id = ?1",
        [OWNER],
        |row| row.get(0),
    )?;
    assert_eq!(speed, 300, "a escrita mais antiga não vence");
    Ok(())
}

#[test]
fn bookmarks_are_idempotent_and_unknown_media_stays_in_the_log() -> TestResult {
    let db = database();
    let bookmark = SyncPayload::Bookmark {
        bookmark_id: "bookmark-1".to_owned(),
        media_id: "m1".to_owned(),
        position_seconds: 30,
        label: "ideia".to_owned(),
        created_at: 100,
    };
    push_operations(&db, OWNER, "device-a", &[input("op-1", 100, bookmark)])?;
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[
            input("op-1", 100, progress("m1", 0, false)),
            input(
                "op-2",
                200,
                SyncPayload::Bookmark {
                    bookmark_id: "bookmark-1".to_owned(),
                    media_id: "m1".to_owned(),
                    position_seconds: 30,
                    label: "ideia".to_owned(),
                    created_at: 100,
                },
            ),
        ],
    )?;
    assert_eq!(count(&db, "media_bookmark"), 1);

    // Mídia desconhecida: a operação entra no diário, mas não há linha órfã.
    let outcome = push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-3", 300, progress("ghost", 10, true))],
    )?;
    assert_eq!(outcome.applied, 1);
    assert_eq!(count(&db, "study_state"), 0);
    assert_eq!(count(&db, "sync_operation"), 3);
    Ok(())
}

#[test]
fn outbox_survives_and_acknowledges_only_accepted_operations() -> TestResult {
    let db = database();
    let state = sync_state(&db, OWNER)?;
    let again = sync_state(&db, OWNER)?;
    assert_eq!(state.device_id, again.device_id, "o dispositivo é estável");
    assert_eq!(state.pull_cursor, 0);

    let first = enqueue_operation(&db, OWNER, &state.device_id, &progress("m1", 5, false), 100)?;
    let second = enqueue_operation(&db, OWNER, &state.device_id, &note("n1", "texto", 110), 110)?;
    let pending = pending_operations(&db, OWNER, 10)?;
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].op_id, first);
    assert_eq!(pending[1].op_id, second);

    // Um envio interrompido mantém a fila; o mesmo `op_id` é reenviado.
    assert_eq!(pending_operations(&db, OWNER, 10)?[0].op_id, first);
    let accepted = push_operations(&db, OWNER, &state.device_id, &pending)?;
    assert_eq!(accepted.applied, 2);
    assert_eq!(acknowledge_operations(&db, OWNER, &[first, second])?, 2);
    assert!(pending_operations(&db, OWNER, 10)?.is_empty());
    Ok(())
}

#[test]
fn applying_pulled_operations_advances_the_cursor_once() -> TestResult {
    let server = database();
    push_operations(
        &server,
        "user-1",
        "device-a",
        &[
            input("op-1", 100, progress("m1", 77, true)),
            input("op-2", 110, note("note-1", "do outro", 110)),
        ],
    )?;
    let batch = pull_operations(&server, "user-1", "device-b", 0, 10)?;
    assert_eq!(batch.items.len(), 2);

    // O dispositivo replica o estado local de outro dono; a regra de conflito é a mesma.
    let local = database();
    let applied = apply_operations(&local, OWNER, &batch.items, batch.cursor)?;
    assert_eq!(applied, 2);
    assert_eq!(sync_state(&local, OWNER)?.pull_cursor, batch.cursor);
    assert!(
        !sync_state(&local, OWNER)?.device_id.is_empty(),
        "aplicar antes da primeira leitura não pode gravar um dispositivo vazio"
    );

    let (position, completed): (i64, i64) = local.connection().query_row(
        "SELECT position_seconds, completed FROM study_state WHERE owner_id = ?1",
        [OWNER],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!((position, completed), (77, 1));
    assert_eq!(count(&local, "study_note"), 1);

    // Reaplicar a mesma página é inofensivo para progresso e anotações.
    let retry: Vec<SyncOperation> = batch.items.clone();
    apply_operations(&local, OWNER, &retry, 1)?;
    assert_eq!(count(&local, "study_note"), 1);
    assert_eq!(
        sync_state(&local, OWNER)?.pull_cursor,
        batch.cursor,
        "o cursor nunca retrocede"
    );
    Ok(())
}

#[test]
fn a_failed_enqueue_rolls_back_the_study_write() -> TestResult {
    let db = database();
    let failed = learnvault_core::queue_note_create(&db, OWNER, "", "m1", 10, "anotação");
    assert!(matches!(
        failed,
        Err(learnvault_core::Error::InvalidSyncOperation { .. })
    ));
    assert_eq!(
        count(&db, "study_note"),
        0,
        "a escrita de estudo volta atrás"
    );
    assert_eq!(count(&db, "sync_outbox"), 0, "nada fica na fila");

    let note = learnvault_core::queue_note_create(&db, OWNER, "device-a", "m1", 10, "anotação")?;
    assert_eq!(count(&db, "study_note"), 1);
    assert_eq!(count(&db, "sync_outbox"), 1);
    let version: Option<String> = db.connection().query_row(
        "SELECT sync_version FROM study_note WHERE note_id = ?1",
        [&note.note_id],
        |row| row.get(0),
    )?;
    assert!(version.is_some(), "a versão é marcada na mesma transação");
    Ok(())
}

#[test]
fn two_devices_apply_note_versions_in_any_order_to_the_same_rows() -> TestResult {
    let operations: Vec<SyncOperation> = ["op-1", "op-2", "op-3"]
        .iter()
        .enumerate()
        .map(|(index, op_id)| SyncOperation {
            seq: index as i64 + 1,
            op_id: (*op_id).to_owned(),
            device_id: "device-a".to_owned(),
            client_at: 100 + index as i64,
            payload: note("note-1", &format!("versão {index}"), 100 + index as i64),
        })
        .collect();

    let first = database();
    let second = database();
    apply_operations(&first, OWNER, &operations, operations.last().unwrap().seq)?;
    // A segunda ordem chega fora de ordem e precisa terminar nas mesmas linhas.
    let reordered: Vec<SyncOperation> = operations
        .iter()
        .rev()
        .cloned()
        .chain(std::iter::once(operations[1].clone()))
        .collect();
    apply_operations(&second, OWNER, &reordered, operations.last().unwrap().seq)?;

    let rows = |db: &Database| -> Vec<(String, String, String)> {
        let mut statement = db
            .connection()
            .prepare("SELECT note_id, content, sync_version FROM study_note ORDER BY note_id ASC")
            .expect("a consulta de anotações deve preparar");
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })
            .expect("as anotações devem ser consultáveis")
            .map(|row| row.expect("linha válida"))
            .collect()
    };
    assert_eq!(rows(&first), rows(&second));
    assert_eq!(rows(&first).len(), 3, "principal + duas cópias de conflito");
    Ok(())
}

#[test]
fn an_interrupted_pull_resumes_without_duplicating() -> TestResult {
    let server = database();
    push_operations(
        &server,
        "user-1",
        "device-a",
        &[
            input("op-1", 100, note("note-1", "um", 100)),
            input("op-2", 200, note("note-2", "dois", 200)),
            input("op-3", 300, note("note-3", "três", 300)),
        ],
    )?;
    let local = database();
    let mut cursor = 0;
    let mut applied = 0;
    loop {
        let batch = pull_operations(&server, "user-1", "device-b", cursor, 2)?;
        if batch.items.is_empty() {
            break;
        }
        applied += apply_operations(&local, OWNER, &batch.items, batch.cursor)?;
        cursor = sync_state(&local, OWNER)?.pull_cursor;
        if !batch.has_more {
            break;
        }
    }
    assert_eq!(applied, 3);
    assert_eq!(count(&local, "study_note"), 3);
    assert_eq!(cursor, 3);
    Ok(())
}

#[test]
fn completion_recorded_locally_is_preserved_when_an_older_sync_arrives() -> TestResult {
    let db = database();
    learnvault_core::record_position(&db, OWNER, "m1", 10)?;
    set_completed(&db, OWNER, "m1", true)?;
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-1", 1, progress("m1", 5, false))],
    )?;
    let (position, completed): (i64, i64) = db.connection().query_row(
        "SELECT position_seconds, completed FROM study_state WHERE owner_id = ?1",
        [OWNER],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert!(completed == 1, "conclusão local não é desfeita");
    assert_eq!(position, 10, "a posição local mais nova vence");
    Ok(())
}

#[test]
fn a_deleted_note_disappears_on_the_other_device() -> TestResult {
    let device_a = database();
    let note = queue_note_create(&device_a, OWNER, "device-a", "m1", 10, "rascunho")?;
    let created = pending_operations(&device_a, OWNER, 10)?;
    let server = database();
    push_operations(&server, OWNER, "device-a", &created)?;

    let device_b = database();
    let batch = pull_operations(&server, OWNER, "device-b", 0, 10)?;
    apply_operations(&device_b, OWNER, &batch.items, batch.cursor)?;
    assert_eq!(count(&device_b, "study_note"), 1);

    assert!(queue_note_delete(
        &device_a,
        OWNER,
        "device-a",
        &note.note_id
    )?);
    let accepted: Vec<String> = created
        .iter()
        .map(|operation| operation.op_id.clone())
        .collect();
    acknowledge_operations(&device_a, OWNER, &accepted)?;
    let deleted = pending_operations(&device_a, OWNER, 10)?;
    assert_eq!(deleted.len(), 1, "só a exclusão continua pendente");
    push_operations(&server, OWNER, "device-a", &deleted)?;

    let batch = pull_operations(&server, OWNER, "device-b", batch.cursor, 10)?;
    apply_operations(&device_b, OWNER, &batch.items, batch.cursor)?;
    assert_eq!(
        count(&device_b, "study_note"),
        0,
        "a exclusão chega ao outro dispositivo"
    );
    assert_eq!(count(&server, "study_note"), 0);
    Ok(())
}

#[test]
fn a_stale_create_does_not_resurrect_a_deleted_note() -> TestResult {
    let db = database();
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-1", 1_000, note("note-1", "rascunho", 1_000))],
    )?;
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input(
            "op-2",
            2_000,
            SyncPayload::NoteDeleted {
                note_id: "note-1".to_owned(),
            },
        )],
    )?;
    assert_eq!(count(&db, "study_note"), 0);
    assert_eq!(count(&db, "sync_tombstone"), 1);

    // Um create antigo que chega atrasado é ignorado.
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-3", 3_000, note("note-1", "rascunho", 1_500))],
    )?;
    assert_eq!(
        count(&db, "study_note"),
        0,
        "um create antigo não ressuscita a nota"
    );

    // Uma edição mais nova que a exclusão ainda vence.
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-4", 4_000, note("note-1", "novo", 2_500))],
    )?;
    assert_eq!(count(&db, "study_note"), 1);
    Ok(())
}

#[test]
fn a_deleted_bookmark_disappears_on_the_other_device() -> TestResult {
    let device_a = database();
    let bookmark = queue_bookmark_create(&device_a, OWNER, "device-a", "m1", 30, "ideia")?;
    let created = pending_operations(&device_a, OWNER, 10)?;
    let server = database();
    push_operations(&server, OWNER, "device-a", &created)?;

    let device_b = database();
    let batch = pull_operations(&server, OWNER, "device-b", 0, 10)?;
    apply_operations(&device_b, OWNER, &batch.items, batch.cursor)?;
    assert_eq!(count(&device_b, "media_bookmark"), 1);

    assert!(queue_bookmark_delete(
        &device_a,
        OWNER,
        "device-a",
        &bookmark.bookmark_id
    )?);
    let accepted: Vec<String> = created
        .iter()
        .map(|operation| operation.op_id.clone())
        .collect();
    acknowledge_operations(&device_a, OWNER, &accepted)?;
    let deleted = pending_operations(&device_a, OWNER, 10)?;
    push_operations(&server, OWNER, "device-a", &deleted)?;

    let batch = pull_operations(&server, OWNER, "device-b", batch.cursor, 10)?;
    apply_operations(&device_b, OWNER, &batch.items, batch.cursor)?;
    assert_eq!(count(&device_b, "media_bookmark"), 0);

    // O mesmo create antigo reenviado não ressuscita o marcador no servidor.
    push_operations(
        &server,
        OWNER,
        "device-b",
        &[input(
            "op-stale",
            300,
            SyncPayload::Bookmark {
                bookmark_id: bookmark.bookmark_id.clone(),
                media_id: "m1".to_owned(),
                position_seconds: 30,
                label: "ideia".to_owned(),
                created_at: 100,
            },
        )],
    )?;
    assert_eq!(count(&server, "media_bookmark"), 0);
    Ok(())
}

#[test]
fn deleting_an_absent_row_enqueues_nothing() -> TestResult {
    let db = database();

    assert!(!queue_note_delete(&db, OWNER, "device-a", "missing-note")?);
    assert!(!queue_bookmark_delete(
        &db,
        OWNER,
        "device-a",
        "missing-bookmark"
    )?);

    assert_eq!(pending_count(&db, OWNER)?, 0, "nada entra na fila");
    assert_eq!(count(&db, "sync_tombstone"), 0, "sem linha, sem tombstone");
    Ok(())
}

#[test]
fn a_card_from_one_device_applies_on_the_other_and_accepts_reviews() -> TestResult {
    let device_a = database();
    let card = queue_flashcard_create(
        &device_a,
        OWNER,
        "device-a",
        "m1",
        "pergunta",
        "resposta",
        Some(10),
    )?;
    let created = pending_operations(&device_a, OWNER, 10)?;
    let server = database();
    push_operations(&server, OWNER, "device-a", &created)?;

    let device_b = database();
    let batch = pull_operations(&server, OWNER, "device-b", 0, 10)?;
    apply_operations(&device_b, OWNER, &batch.items, batch.cursor)?;
    assert_eq!(count(&device_b, "flashcard"), 1);
    assert_eq!(card_front(&device_b, &card.card_id), "pergunta");

    // A review of the synced card is no longer dropped: it schedules on both devices.
    let reviewed = queue_review(
        &device_b,
        OWNER,
        "device-b",
        &card.card_id,
        ReviewRating::Good,
    )?;
    assert_eq!(reviewed.interval_days, 1);
    let operations = pending_operations(&device_b, OWNER, 10)?;
    push_operations(&server, OWNER, "device-b", &operations)?;
    let batch = pull_operations(&server, OWNER, "device-a", 0, 10)?;
    apply_operations(&device_a, OWNER, &batch.items, batch.cursor)?;

    assert_eq!(count(&device_a, "flashcard_review"), 1);
    let (interval, due): (i64, i64) = device_a.connection().query_row(
        "SELECT interval_days, next_due FROM flashcard WHERE card_id = ?1",
        [&card.card_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(
        (interval, due),
        (reviewed.interval_days, reviewed.next_due),
        "os dois dispositivos chegam ao mesmo agendamento"
    );
    Ok(())
}

#[test]
fn card_updates_are_last_write_wins() -> TestResult {
    let db = database();
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input(
            "op-1",
            2_000,
            card_created("card-1", "original", "verso", 2_000),
        )],
    )?;

    // Uma edição mais antiga não sobrescreve.
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input(
            "op-2",
            3_000,
            card_updated("card-1", "antiga", "verso", 1_500),
        )],
    )?;
    assert_eq!(card_front(&db, "card-1"), "original");

    // Uma edição mais nova vence.
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input(
            "op-3",
            4_000,
            card_updated("card-1", "nova", "verso", 2_500),
        )],
    )?;
    assert_eq!(card_front(&db, "card-1"), "nova");
    Ok(())
}

#[test]
fn a_deleted_card_leaves_a_tombstone_that_blocks_a_stale_create() -> TestResult {
    let device_a = database();
    let card = queue_flashcard_create(
        &device_a, OWNER, "device-a", "m1", "pergunta", "resposta", None,
    )?;
    let created = pending_operations(&device_a, OWNER, 10)?;
    let server = database();
    push_operations(&server, OWNER, "device-a", &created)?;
    let accepted: Vec<String> = created
        .iter()
        .map(|operation| operation.op_id.clone())
        .collect();
    acknowledge_operations(&device_a, OWNER, &accepted)?;

    assert!(queue_flashcard_delete(
        &device_a,
        OWNER,
        "device-a",
        &card.card_id
    )?);
    assert_eq!(count(&device_a, "sync_tombstone"), 1);
    let deleted = pending_operations(&device_a, OWNER, 10)?;
    assert_eq!(deleted.len(), 1, "só a exclusão continua pendente");
    push_operations(&server, OWNER, "device-a", &deleted)?;
    assert_eq!(count(&server, "flashcard"), 0);
    assert_eq!(count(&server, "sync_tombstone"), 1);

    // Um create antigo que chega atrasado é ignorado.
    push_operations(
        &server,
        OWNER,
        "device-b",
        &[input(
            "op-stale",
            4_000,
            card_created(&card.card_id, "pergunta", "resposta", 100),
        )],
    )?;
    assert_eq!(
        count(&server, "flashcard"),
        0,
        "um create antigo não ressuscita o cartão"
    );
    Ok(())
}

#[test]
fn consecutive_progress_writes_leave_one_pending_operation() -> TestResult {
    let db = database();
    let state = sync_state(&db, OWNER)?;

    queue_position(&db, OWNER, &state.device_id, "m1", 10)?;
    let newest = queue_position(&db, OWNER, &state.device_id, "m1", 42)?;

    assert_eq!(pending_count(&db, OWNER)?, 1, "só a posição mais nova fica");
    let pending = pending_operations(&db, OWNER, 10)?;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].payload, progress("m1", 42, false));
    assert_eq!(newest.position_seconds, 42);
    Ok(())
}

#[test]
fn a_future_client_clock_is_clamped_to_the_server_time() -> TestResult {
    let db = database();
    push_operations(
        &db,
        OWNER,
        "device-a",
        &[input("op-1", i64::MAX, progress("m1", 10, false))],
    )?;
    let stored_client_at: i64 = db.connection().query_row(
        "SELECT client_at FROM sync_operation WHERE op_id = 'op-1'",
        [],
        |row| row.get(0),
    )?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    assert!(
        stored_client_at <= now,
        "o instante recebido é limitado ao instante do servidor"
    );

    // Uma escrita com o relógio real ainda vence a escrita futura.
    push_operations(
        &db,
        OWNER,
        "device-b",
        &[input("op-2", now, progress("m1", 77, false))],
    )?;
    let position: i64 = db.connection().query_row(
        "SELECT position_seconds FROM study_state WHERE owner_id = ?1 AND media_id = 'm1'",
        [OWNER],
        |row| row.get(0),
    )?;
    assert_eq!(position, 77);
    Ok(())
}
