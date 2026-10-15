//! Testes do rescan explícito da biblioteca desktop.
//!
//! Não sobem o webview: exercitam a função livre `DesktopState::rescan`, que o comando
//! `#[tauri::command]` apenas embrulha. O alvo é o contrato observável — rescan somente
//! explícito, sobre a raiz persistida, com a biblioteca externa intocada, o banco fora da
//! raiz, a política do núcleo (`learnvault_core::rescan`) preservada e a serialização por
//! estado ocupado devolvendo um erro estável. Nenhuma mensagem de erro carrega caminho.

use std::fs;
use std::path::{Path, PathBuf};

use learnvault_desktop::{DesktopState, IpcErrorCode};

/// Estado desktop isolado em diretórios temporários, com dados e configuração separados.
fn desktop(root: &Path) -> DesktopState {
    DesktopState::open(&root.join("data"), &root.join("config")).unwrap()
}

/// Estado desktop com uma biblioteca externa escolhida e persistida.
///
/// O diretório tem um nome que não existe em nenhuma mensagem do contrato, para que os
/// testes possam provar que nenhum caminho vaza pelo IPC.
fn configured(root: &Path) -> (DesktopState, PathBuf) {
    let state = desktop(root);
    let library = root.join("Acervo privado");
    fs::create_dir_all(&library).unwrap();
    state.select_library(|| Some(library.clone())).unwrap();
    (state, library)
}

/// Caminho do banco local usado pelo estado.
fn database_path(root: &Path) -> PathBuf {
    root.join("data").join("learnvault.sqlite")
}

/// Árvore de arquivos e diretórios relativos à raiz, ordenada, para provar leitura pura.
fn tree(root: &Path) -> Vec<String> {
    fn walk(base: &Path, current: &Path, out: &mut Vec<String>) {
        let mut entries: Vec<_> = fs::read_dir(current)
            .unwrap_or_else(|error| panic!("não foi possível ler {}: {error}", current.display()))
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let relative = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push(relative);
            if path.is_dir() {
                walk(base, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

/// Conta mídias marcadas como ausentes diretamente no banco local.
fn missing_rows(root: &Path) -> i64 {
    let database = learnvault_core::Database::open(database_path(root)).unwrap();
    database
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM media WHERE availability = 'missing'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn rescan_without_a_configured_root_is_a_stable_error_without_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());

    let error = state.rescan().unwrap_err();

    assert_eq!(error.code, IpcErrorCode::LibraryNotConfigured);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "library_not_configured");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(
        !message.contains(&dir.path().to_string_lossy().to_string()),
        "a mensagem não pode expor um caminho absoluto: {message}"
    );
    assert!(
        !dir.path().join("data").join("learnvault.sqlite").exists()
            || missing_rows(dir.path()) == 0,
        "um rescan não configurado não pode registrar histórico"
    );
}

#[test]
fn rescan_with_a_removed_root_is_a_stable_error_without_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    fs::remove_dir_all(&library).unwrap();

    let error = state.rescan().unwrap_err();

    assert_eq!(error.code, IpcErrorCode::RescanUnavailable);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "rescan_unavailable");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(
        !message.contains(&library.to_string_lossy().to_string())
            && !message.contains("Acervo privado"),
        "a mensagem não pode expor a raiz: {message}"
    );
}

#[test]
fn rescan_of_an_empty_library_is_a_complete_empty_summary() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());

    let summary = state.rescan().unwrap();

    assert_eq!(summary.inserted, 0);
    assert_eq!(summary.marked_missing, 0);
    assert!(
        summary.complete,
        "uma biblioteca vazia foi lida por completo"
    );
    assert!(summary.ambiguous.is_empty());
}

#[test]
fn rescan_indexes_the_initial_catalog_from_the_persisted_root() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    fs::create_dir_all(library.join("Curso 1")).unwrap();
    fs::write(library.join("Curso 1").join("aula.mp4"), b"video").unwrap();
    fs::write(library.join("Curso 1").join("aula.srt"), b"legenda").unwrap();

    let summary = state.rescan().unwrap();

    assert_eq!(summary.inserted, 2);
    assert_eq!(summary.updated, 0);
    assert_eq!(summary.marked_missing, 0);

    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["inserted"], 2);
    assert_eq!(json["complete"], true);

    let catalog = state.catalog().unwrap();
    assert!(
        !catalog.categories.is_empty(),
        "o catálogo precisa refletir a varredura aplicada"
    );
}

#[test]
fn a_second_rescan_detects_and_adds_new_files() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    fs::write(library.join("aula.mp4"), b"video").unwrap();

    let first = state.rescan().unwrap();
    assert_eq!(first.inserted, 1);

    fs::write(library.join("aula-2.mp4"), b"video 2").unwrap();
    let second = state.rescan().unwrap();

    assert_eq!(second.inserted, 1, "o arquivo novo precisa ser inserido");
    assert_eq!(second.updated, 1, "o arquivo antigo precisa ser reafirmado");
}

#[test]
fn a_rescan_marks_a_vanished_file_as_missing_through_the_core_policy() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    let file = library.join("aula.mp4");
    fs::write(&file, b"video").unwrap();
    state.rescan().unwrap();

    fs::remove_file(&file).unwrap();
    let summary = state.rescan().unwrap();

    assert_eq!(summary.marked_missing, 1);
    assert_eq!(
        missing_rows(dir.path()),
        1,
        "o registro precisa ser marcado como ausente, sem ser excluído"
    );
}

#[test]
fn rescan_reads_the_library_without_writing_inside_it() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    fs::create_dir_all(library.join("Curso 1").join("Aula")).unwrap();
    fs::write(
        library.join("Curso 1").join("Aula").join("aula.mp4"),
        b"video",
    )
    .unwrap();
    let before = tree(&library);

    state.rescan().unwrap();
    state.rescan().unwrap();

    assert_eq!(
        tree(&library),
        before,
        "a biblioteca externa permanece somente leitura"
    );
}

#[test]
fn the_database_stays_outside_the_library_root() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    fs::write(library.join("aula.mp4"), b"video").unwrap();

    state.rescan().unwrap();

    assert!(
        database_path(dir.path()).exists(),
        "o banco precisa viver no diretório de dados do aplicativo"
    );
    assert!(
        !library.join("learnvault.sqlite").exists(),
        "o banco não pode viver na biblioteca externa"
    );
    assert!(
        !tree(&library).iter().any(|entry| entry.ends_with(".sqlite")
            || entry.ends_with(".sqlite-wal")
            || entry.ends_with(".sqlite-shm")),
        "nenhum artefato do banco pode ser criado na biblioteca"
    );
}

#[test]
fn concurrent_rescans_do_not_run_together_and_return_a_stable_error() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    fs::write(library.join("aula.mp4"), b"video").unwrap();

    // O estado ocupado é mantido enquanto a biblioteca está sendo lida: chamadas
    // concorrentes não podem entrar em paralelo nem corromper o catálogo.
    let guard = state.begin_rescan().unwrap();

    let codes: Vec<IpcErrorCode> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let state = state.clone();
                scope.spawn(move || state.rescan().unwrap_err().code)
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a thread não pode entrar em pânico"))
            .collect()
    });

    for code in codes {
        assert_eq!(
            code,
            IpcErrorCode::RescanInProgress,
            "chamadas concorrentes precisam devolver o erro estável"
        );
    }

    drop(guard);

    let summary = state.rescan().unwrap();
    assert_eq!(
        summary.inserted, 1,
        "as tentativas concorrentes recusadas não podem ter varrido a biblioteca"
    );
}
