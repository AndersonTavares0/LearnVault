//! Testes da fronteira IPC das notas pessoais do desktop.
//!
//! Não sobem o webview nem dependem de display: exercitam as funções livres
//! `DesktopState::study_notes` e `study_notes_all`, que os comandos `#[tauri::command]`
//! apenas embrulham. O alvo é o contrato observável pelo frontend — shape snake_case,
//! envelope `{ items: [...], truncated }`, códigos de erro estáveis — e as regras que o
//! adaptador **não** pode reimplementar: dono local fixo, isolamento entre donos e teto
//! do núcleo.

use std::path::{Path, PathBuf};

use learnvault_desktop::{DesktopState, IpcErrorCode, LOCAL_OWNER_ID};

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
/// Serve para semear mídia (que no aplicativo vem do rescan) e as notas, e para afirmar
/// o que foi persistido sem passar pelo adaptador sob teste. O diretório de dados é
/// criado por [`DesktopState::open`], então os testes precisam abrir o estado antes de
/// semear.
fn local(root: &Path) -> learnvault_core::Database {
    learnvault_core::Database::open(database_path(root)).unwrap()
}

/// Semeia a biblioteca local e uma mídia conhecida, como uma varredura faria.
fn seed_media(root: &Path, media_id: &str) {
    let database = local(root);
    let path = format!("acervo/{media_id}.mp4");
    database
        .connection()
        .execute_batch(&format!(
            "INSERT OR IGNORE INTO library (library_id, available) VALUES ('library-1', 1);
             INSERT INTO media
                 (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('{media_id}', 'library-1', '{path}', '{path}', 10, 0, 'available');"
        ))
        .unwrap();
}

#[test]
fn notes_are_listed_per_media_and_across_the_owner() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");
    seed_media(dir.path(), "m2");

    // Duas notas do dono local, em mídias diferentes.
    {
        let database = local(dir.path());
        learnvault_core::create_note(&database, LOCAL_OWNER_ID, "m1", "primeira", Some(30))
            .unwrap();
        learnvault_core::create_note(&database, LOCAL_OWNER_ID, "m2", "segunda", None).unwrap();
    }

    // A listagem por mídia devolve só a nota daquela mídia.
    let first = state.study_notes("m1").unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].text, "primeira");
    assert_eq!(first.items[0].media_id, "m1");
    assert_eq!(first.items[0].position_seconds, Some(30));
    assert!(!first.truncated);

    let second = state.study_notes("m2").unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].text, "segunda");

    // A listagem global junta as notas de todas as mídias, da mais recente para a mais
    // antiga.
    let all = state.study_notes_all().unwrap();
    assert_eq!(all.items.len(), 2);
    assert_eq!(all.items[0].media_id, "m2");
    assert_eq!(all.items[1].media_id, "m1");
    assert!(!all.truncated);

    // O envelope é o mesmo que o adaptador HTTP entrega.
    let json = serde_json::to_value(&all).unwrap();
    assert_eq!(json["items"][0]["media_id"], "m2");
    assert_eq!(json["truncated"], false);
}

#[test]
fn notes_of_another_owner_never_cross_the_ipc() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    // Uma nota de outro dono inserida direto no banco: o desktop responde pela
    // identidade local fixa e não expõe a nota alheia.
    {
        let database = local(dir.path());
        database
            .connection()
            .execute(
                "INSERT INTO study_note (owner_id, media_id, text, position_seconds, created_at, updated_at)
                 VALUES ('outro-dono', 'm1', 'segredo', NULL, 1, 1)",
                [],
            )
            .unwrap();
    }

    assert!(state.study_notes("m1").unwrap().items.is_empty());
    assert!(state.study_notes_all().unwrap().items.is_empty());
}

#[test]
fn an_unknown_media_is_a_stable_error_without_the_identifier() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let secret = "Acervo privado/aula.mp4";

    let error = state.study_notes(secret).unwrap_err();

    assert_eq!(error.code, IpcErrorCode::MediaNotFound);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "media_not_found");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(
        !message.contains(secret) && !message.contains("Acervo privado"),
        "a mensagem não pode ecoar o identificador consultado: {message}"
    );
}

#[test]
fn an_empty_note_text_is_refused_with_the_stable_code() {
    let dir = tempfile::tempdir().unwrap();
    // O estado cria o diretório de dados onde o banco local é aberto abaixo.
    let _state = desktop(dir.path());
    seed_media(dir.path(), "m1");

    let database = local(dir.path());
    let error =
        learnvault_core::create_note(&database, LOCAL_OWNER_ID, "m1", "   ", None).unwrap_err();

    // O adaptador traduz o erro do núcleo no código estável da IPC: o texto vazio é
    // recusado como `invalid_note`, e não como uma falha genérica de banco.
    let ipc = learnvault_desktop::study::note_error(error);
    assert_eq!(ipc.code, IpcErrorCode::InvalidNote);
    let json = serde_json::to_value(&ipc).unwrap();
    assert_eq!(json["code"], "invalid_note");
}
