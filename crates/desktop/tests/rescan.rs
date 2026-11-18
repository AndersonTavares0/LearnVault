//! Tests of the explicit rescan of the desktop library.
//!
//! They do not bring up the webview: they exercise the free function `DesktopState::rescan`, which the
//! `#[tauri::command]` command only wraps. The target is the observable contract — an explicit-only
//! rescan, over the persisted root, with the external library untouched, the database outside the
//! root, the core's policy (`learnvault_core::rescan`) preserved and the busy-state
//! serialization returning a stable error. No error message carries a path.

use std::fs;
use std::path::{Path, PathBuf};

use learnvault_desktop::{DesktopState, IpcErrorCode, RescanSummaryDto};

/// Desktop state isolated in temporary directories, with data and configuration separate.
fn desktop(root: &Path) -> DesktopState {
    DesktopState::open(&root.join("data"), &root.join("config")).unwrap()
}

/// Desktop state with an external course imported and persisted.
///
/// The directory has a name that appears in no message of the contract, so that the
/// tests can prove that no path leaks through the IPC.
fn configured(root: &Path) -> (DesktopState, PathBuf) {
    let state = desktop(root);
    let course = root.join("Acervo privado");
    fs::create_dir_all(&course).unwrap();
    state.import_course(|| Some(course.clone())).unwrap();
    (state, course)
}

/// Path of the local database used by the state.
fn database_path(root: &Path) -> PathBuf {
    root.join("data").join("learnvault.sqlite")
}

/// Tree of files and directories relative to the root, sorted, to prove a pure read.
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

/// Counts media marked as missing straight in the local database.
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

/// Counts all media persisted in the local database.
fn media_rows(root: &Path) -> i64 {
    let database = learnvault_core::Database::open(database_path(root)).unwrap();
    database
        .connection()
        .query_row("SELECT COUNT(*) FROM media", [], |row| row.get(0))
        .unwrap()
}

/// Creates a directory link at `link` pointing to `target`.
///
/// On Windows creating a symlink/junction requires privilege; when the platform refuses, the
/// test is skipped with a log, like the repository's other link tests.
#[cfg(unix)]
fn link_dir(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn link_dir(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, link).is_ok()
}

#[test]
fn rescan_without_courses_is_a_stable_error_without_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());

    let error = state.rescan().unwrap_err();

    assert_eq!(error.code, IpcErrorCode::NoCoursesRegistered);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "no_courses_registered");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(
        !message.contains(&dir.path().to_string_lossy().to_string()),
        "a mensagem não pode expor um caminho absoluto: {message}"
    );
    assert!(
        !dir.path().join("data").join("learnvault.sqlite").exists()
            || missing_rows(dir.path()) == 0,
        "um rescan sem cursos não pode registrar histórico"
    );
}

#[test]
fn rescan_with_a_removed_course_root_reports_a_failure_without_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let (state, course) = configured(dir.path());
    fs::remove_dir_all(&course).unwrap();

    let summary = state.rescan().unwrap();

    assert_eq!(summary.courses, 1);
    assert_eq!(summary.failed, 1, "a pasta removida conta como falha");
    assert!(!summary.complete, "uma falha deixa a agregação incompleta");
    let json = serde_json::to_value(&summary).unwrap();
    let rendered = json.to_string();
    assert!(
        !rendered.contains(&course.to_string_lossy().to_string())
            && !rendered.contains("Acervo privado"),
        "a resposta não pode expor a raiz: {rendered}"
    );
}

#[test]
fn a_root_replaced_by_a_link_is_refused_before_scanning() {
    let dir = tempfile::tempdir().unwrap();
    let (state, course) = configured(dir.path());
    fs::write(course.join("aula.mp4"), b"video").unwrap();
    state.rescan().unwrap();
    assert_eq!(media_rows(dir.path()), 1);

    // The root is swapped for a link to another folder after the import: the immediate
    // revalidation must refuse the scan without following the link.
    fs::remove_dir_all(&course).unwrap();
    let outside = dir.path().join("fora");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("segredo.mp4"), b"x").unwrap();

    if !link_dir(&outside, &course) {
        eprintln!("pulado: criar symlink/junction exige privilégio nesta plataforma");
        return;
    }

    let summary = state.rescan().unwrap();

    assert_eq!(summary.failed, 1, "o link precisa ser recusado como falha");
    assert!(!summary.complete);
    assert_eq!(
        media_rows(dir.path()),
        1,
        "a varredura não pode ter seguido o link e indexado a pasta externa"
    );
}

#[test]
fn rescan_of_an_empty_library_is_a_complete_empty_summary() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());

    let summary = state.rescan().unwrap();

    assert_eq!(summary.courses, 1);
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.inserted, 0);
    assert_eq!(summary.marked_missing, 0);
    assert!(
        summary.complete,
        "uma biblioteca vazia foi lida por completo"
    );
    assert_eq!(summary.ambiguous_count, 0);
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

    // The busy state is held while the library is being read: concurrent calls
    // cannot run in parallel nor corrupt the catalog.
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

#[test]
fn a_failed_course_releases_the_busy_state_and_recovers_when_restored() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    fs::write(library.join("aula.mp4"), b"video").unwrap();
    state.rescan().unwrap();
    assert_eq!(media_rows(dir.path()), 1);

    // The root vanishes after the import: the aggregation reports the failure without locking up the state.
    fs::remove_dir_all(&library).unwrap();
    let first = state.rescan().unwrap();
    assert_eq!(first.failed, 1);
    assert!(!first.complete);

    // The busy guard was released: the second call aggregates again, without returning
    // `rescan_in_progress`.
    let second = state.rescan().unwrap();
    assert_eq!(second.failed, 1);

    // Restoring the root, a new rescan starts and completes normally. The content is
    // different on purpose: a file identical to the previous one would be recognized as the
    // same file at another path (incremental detection preserves the identity).
    fs::create_dir_all(&library).unwrap();
    fs::write(library.join("aula-2.mp4"), b"outro video").unwrap();
    let recovered = state.rescan().unwrap();
    assert_eq!(recovered.failed, 0);
    assert_eq!(recovered.inserted, 1);
    assert_eq!(recovered.marked_missing, 1);
    assert_eq!(media_rows(dir.path()), 2);
}

#[test]
fn two_real_concurrent_rescans_stay_consistent_and_free_the_busy_state() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    for index in 0..50 {
        fs::write(library.join(format!("aula-{index}.mp4")), b"video").unwrap();
    }

    // Real concurrency: both calls are `state.rescan()`, not a manual guard.
    let outcomes: Vec<Result<RescanSummaryDto, IpcErrorCode>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let state = state.clone();
                scope.spawn(move || state.rescan().map_err(|error| error.code))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a thread não pode entrar em pânico"))
            .collect()
    });

    // Each real call either indexes or is refused by the busy state — never for another
    // cause, and never in parallel.
    for outcome in &outcomes {
        match outcome {
            Ok(summary) => assert_eq!(
                summary.inserted + summary.updated + summary.marked_missing,
                50,
                "a varredura vencedora precisa cobrir a biblioteca inteira"
            ),
            Err(code) => assert_eq!(*code, IpcErrorCode::RescanInProgress),
        }
    }
    assert!(
        outcomes.iter().any(|outcome| outcome.is_ok()),
        "ao menos uma chamada real precisa indexar: {outcomes:?}"
    );

    // The catalog stayed intact and the busy state was released: a later call
    // starts and reaffirms the 50 files without inserting duplicates.
    assert_eq!(media_rows(dir.path()), 50);
    let final_summary = state.rescan().unwrap();
    assert_eq!(final_summary.inserted, 0);
    assert_eq!(final_summary.updated, 50);
    assert_eq!(media_rows(dir.path()), 50);
}
