//! Fila offline do desktop: durabilidade, replay com o mesmo `op_id` e aplicação do
//! que chega do servidor.

use learnvault_core::{
    Database, KeyMode, SyncOperation, SyncOperationInput, SyncPayload, apply_operations,
    pull_operations, push_operations,
};
use learnvault_desktop::{IpcErrorCode, study, sync};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Converte o erro IPC (que não implementa `Error`) em um erro de teste.
fn ok<T>(result: Result<T, learnvault_desktop::IpcError>) -> Result<T, Box<dyn std::error::Error>> {
    result.map_err(|error| format!("{error:?}").into())
}

/// Abre (ou reabre) o banco local do desktop.
fn database(path: &std::path::Path) -> Database {
    Database::open(path).expect("o banco local deve abrir")
}

fn seed(db: &Database) {
    let key =
        learnvault_core::paths::relative_path_key("curso/aula.mp4", KeyMode::for_host()).unwrap();
    db.connection()
        .execute_batch(&format!(
            "INSERT INTO library (library_id, available) VALUES ('library-1', 1);
             INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('m1', 'library-1', 'curso/aula.mp4', '{key}', 1024, 1760000000, 'available');"
        ))
        .unwrap();
}

#[test]
fn every_syncable_write_lands_in_the_durable_outbox() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("desktop.sqlite");
    {
        let db = database(&path);
        seed(&db);

        ok(study::record_position(&db, "m1", 42))?;
        ok(study::set_completed(&db, "m1", true))?;
        let note = ok(study::create_note(&db, "m1", 10, "primeira anotação"))?;
        ok(study::update_note(&db, &note.note_id, "anotação revisada"))?;
        let card = ok(study::create_flashcard(&db, "m1", "frente", "verso", None))?;
        ok(study::review_flashcard(&db, &card.card_id, "good"))?;
        ok(study::create_bookmark(&db, "m1", 30, "marcador"))?;
        ok(study::save_playback_preference(&db, 125, 100, "pt-BR"))?;

        let pending = ok(sync::pending(&db, 100))?;
        let kinds: Vec<&str> = pending
            .iter()
            .map(|operation| match operation.payload {
                SyncPayload::Progress { .. } => "progress",
                SyncPayload::Note { .. } => "note",
                SyncPayload::Bookmark { .. } => "bookmark",
                SyncPayload::FlashcardReview { .. } => "flashcard_review",
                SyncPayload::Preference { .. } => "preference",
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "progress",
                "progress",
                "note",
                "note",
                "flashcard_review",
                "bookmark",
                "preference",
            ]
        );
        // A versão sincronizada fica marcada na linha local.
        let version: Option<String> = db.connection().query_row(
            "SELECT sync_version FROM study_note WHERE note_id = ?1",
            [&note.note_id],
            |row| row.get(0),
        )?;
        assert!(
            version.is_some(),
            "a anotação local aponta a versão da fila"
        );
    }

    // Reabrir o aplicativo preserva a fila e a identidade do dispositivo.
    let db = database(&path);
    let pending = ok(sync::pending(&db, 100))?;
    assert_eq!(pending.len(), 7, "o outbox sobrevive à reinicialização");
    let state = ok(sync::state(&db))?;
    assert!(!state.device_id.is_empty());
    assert_eq!(ok(sync::state(&db))?.device_id, state.device_id);
    Ok(())
}

#[test]
fn a_retried_batch_is_not_duplicated_on_the_server() -> TestResult {
    let desktop_dir = tempfile::tempdir()?;
    let desktop = database(&desktop_dir.path().join("desktop.sqlite"));
    seed(&desktop);
    ok(study::record_position(&desktop, "m1", 90))?;
    ok(study::set_completed(&desktop, "m1", true))?;

    let state = ok(sync::state(&desktop))?;
    let batch = ok(sync::pending(&desktop, 100))?;
    assert_eq!(batch.len(), 2);

    // O servidor é outro banco; o mesmo lote é enviado duas vezes.
    let server_dir = tempfile::tempdir()?;
    let server = database(&server_dir.path().join("server.sqlite"));
    seed(&server);
    let first = push_operations(&server, "usr-1", &state.device_id, &batch)?;
    assert_eq!((first.applied, first.duplicated), (2, 0));
    let retry = push_operations(&server, "usr-1", &state.device_id, &batch)?;
    assert_eq!((retry.applied, retry.duplicated), (0, 2));

    // Somente depois do aceite o desktop limpa a fila.
    let accepted: Vec<String> = batch
        .iter()
        .map(|operation| operation.op_id.clone())
        .collect();
    assert_eq!(ok(sync::acknowledge(&desktop, &accepted))?, 2);
    assert!(ok(sync::pending(&desktop, 100))?.is_empty());

    let progress: (i64, i64) = server.connection().query_row(
        "SELECT position_seconds, completed FROM study_state WHERE owner_id = 'usr-1'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(progress, (90, 1));
    Ok(())
}

#[test]
fn pulled_operations_apply_locally_and_advance_the_cursor() -> TestResult {
    let desktop_dir = tempfile::tempdir()?;
    let desktop = database(&desktop_dir.path().join("desktop.sqlite"));
    seed(&desktop);
    let local = ok(sync::state(&desktop))?;

    // O servidor recebeu uma escrita de outro dispositivo.
    let server_dir = tempfile::tempdir()?;
    let server = database(&server_dir.path().join("server.sqlite"));
    seed(&server);
    push_operations(
        &server,
        "usr-1",
        "device-other",
        &[SyncOperationInput {
            op_id: "op-remote".to_owned(),
            client_at: 100,
            payload: SyncPayload::Progress {
                media_id: "m1".to_owned(),
                position_seconds: 120,
                completed: true,
            },
        }],
    )?;

    let page = pull_operations(&server, "usr-1", &local.device_id, 0, 100)?;
    assert_eq!(page.items.len(), 1);
    assert_eq!(ok(sync::apply(&desktop, &page.items, page.cursor))?, 1);
    let after = ok(sync::state(&desktop))?;
    assert_eq!(after.pull_cursor, page.cursor);

    let progress = ok(study::progress(&desktop, "m1"))?;
    assert_eq!(progress.position_seconds, 120);
    assert!(progress.completed);

    // Reaplicar a mesma página não duplica o efeito.
    ok(sync::apply(&desktop, &page.items, page.cursor))?;
    assert_eq!(ok(study::progress(&desktop, "m1"))?.position_seconds, 120);
    Ok(())
}

#[test]
fn a_pulled_note_does_not_overwrite_a_divergent_local_note() -> TestResult {
    let desktop_dir = tempfile::tempdir()?;
    let desktop = database(&desktop_dir.path().join("desktop.sqlite"));
    seed(&desktop);
    let note = ok(study::create_note(&desktop, "m1", 10, "local"))?;

    let remote: Vec<SyncOperation> = vec![SyncOperation {
        seq: 1,
        op_id: "op-remote".to_owned(),
        device_id: "device-other".to_owned(),
        client_at: 200,
        payload: SyncPayload::Note {
            note_id: note.note_id.clone(),
            media_id: "m1".to_owned(),
            position_seconds: 10,
            content: "remoto".to_owned(),
            created_at: 200,
            updated_at: 200,
        },
    }];
    ok(sync::apply(&desktop, &remote, 1))?;

    let notes = ok(study::list_notes(&desktop, Some("m1")))?;
    assert_eq!(notes.len(), 2, "o texto remoto vira uma cópia");
    let contents: Vec<&str> = notes.iter().map(|note| note.content.as_str()).collect();
    assert!(contents.contains(&"local"));
    assert!(contents.contains(&"remoto"));
    Ok(())
}

#[test]
fn a_progress_error_is_still_reported_without_touching_the_queue() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = database(&dir.path().join("desktop.sqlite"));
    seed(&db);

    let missing = study::record_position(&db, "desconhecida", 1);
    assert!(matches!(missing, Err(error) if error.code == IpcErrorCode::MediaNotFound));
    assert!(ok(sync::pending(&db, 10))?.is_empty());
    Ok(())
}

#[test]
fn applying_remote_operations_keeps_a_cursor_even_without_effect() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = database(&dir.path().join("desktop.sqlite"));
    seed(&db);

    // Operação cuja mídia não existe localmente: não falha e não perde o cursor.
    let operations = vec![SyncOperation {
        seq: 4,
        op_id: "op-ghost".to_owned(),
        device_id: "device-other".to_owned(),
        client_at: 100,
        payload: SyncPayload::Progress {
            media_id: "ghost".to_owned(),
            position_seconds: 5,
            completed: true,
        },
    }];
    assert_eq!(ok(sync::apply(&db, &operations, 4))?, 1);
    let state = ok(sync::state(&db))?;
    assert_eq!(state.pull_cursor, 4);
    assert!(!state.device_id.is_empty());
    Ok(())
}

#[test]
fn applying_a_legacy_operation_without_a_cursor_does_not_regress() -> TestResult {
    let dir = tempfile::tempdir()?;
    let db = database(&dir.path().join("desktop.sqlite"));
    seed(&db);
    let operation = SyncOperation {
        seq: 9,
        op_id: "op-1".to_owned(),
        device_id: "device-other".to_owned(),
        client_at: 100,
        payload: SyncPayload::Progress {
            media_id: "m1".to_owned(),
            position_seconds: 5,
            completed: false,
        },
    };
    apply_operations(&db, "desktop-local", &[operation], 9)?;
    assert_eq!(ok(sync::state(&db))?.pull_cursor, 9);
    Ok(())
}
