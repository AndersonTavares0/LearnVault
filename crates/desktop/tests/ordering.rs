//! Manual ordering of modules and lessons on the desktop (issue #239).
//!
//! It exercises the state's free functions, without bringing up the webview: the order lives in the
//! application's data directory, survives a rescan and a restart, and never touches the course
//! folder.

use std::fs;
use std::path::{Path, PathBuf};

use learnvault_desktop::{DesktopState, IpcErrorCode};

/// Desktop state isolated in temporary directories.
fn desktop(root: &Path) -> DesktopState {
    DesktopState::open(&root.join("data"), &root.join("config")).unwrap()
}

/// Synthetic course with three numbered modules, each with one lesson.
fn make_course(root: &Path) -> PathBuf {
    let course = root.join("curso");
    for module in ["Modulo 2", "Modulo 3", "Modulo 10"] {
        let directory = course.join(module).join("Aula 1");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("aula.mp4"), b"video").unwrap();
    }
    course
}

fn module_paths(state: &DesktopState) -> Vec<String> {
    state.catalog().unwrap().categories[0]
        .children
        .iter()
        .map(|node| node.path.clone())
        .collect()
}

fn owned(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| (*path).to_owned()).collect()
}

/// Recursive listing of the course's relative paths, to prove that nothing changes.
fn snapshot(root: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path.clone());
            }
            paths.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    paths.sort();
    paths
}

#[test]
fn manual_order_persists_across_rescan_and_restart_without_touching_files() {
    let dir = tempfile::tempdir().unwrap();
    let course = make_course(dir.path());
    let state = desktop(dir.path());
    state.import_course(|| Some(course.clone())).unwrap();
    state.rescan().unwrap();
    let id = state.courses()[0].id.clone();

    assert_eq!(module_paths(&state), ["Modulo 2", "Modulo 3", "Modulo 10"]);

    let before = snapshot(&course);
    state
        .set_course_order(&id, "", &owned(&["Modulo 10", "Modulo 2", "Modulo 3"]))
        .unwrap();
    assert_eq!(module_paths(&state), ["Modulo 10", "Modulo 2", "Modulo 3"]);

    // Sobrevive a um rescan.
    state.rescan().unwrap();
    assert_eq!(module_paths(&state), ["Modulo 10", "Modulo 2", "Modulo 3"]);

    // The course folder stays read-only.
    assert_eq!(snapshot(&course), before, "a pasta do curso não pode mudar");

    // Survives a restart.
    drop(state);
    let restarted = desktop(dir.path());
    assert_eq!(
        module_paths(&restarted),
        ["Modulo 10", "Modulo 2", "Modulo 3"]
    );
}

#[test]
fn reset_restores_the_automatic_order() {
    let dir = tempfile::tempdir().unwrap();
    let course = make_course(dir.path());
    let state = desktop(dir.path());
    state.import_course(|| Some(course)).unwrap();
    state.rescan().unwrap();
    let id = state.courses()[0].id.clone();

    state
        .set_course_order(&id, "", &owned(&["Modulo 10", "Modulo 2", "Modulo 3"]))
        .unwrap();
    assert_eq!(module_paths(&state), ["Modulo 10", "Modulo 2", "Modulo 3"]);

    assert!(state.reset_course_order(&id, None).unwrap());
    assert_eq!(module_paths(&state), ["Modulo 2", "Modulo 3", "Modulo 10"]);
    // Idempotente.
    assert!(!state.reset_course_order(&id, None).unwrap());
}

#[test]
fn order_rejects_non_permutations_and_unknown_courses() {
    let dir = tempfile::tempdir().unwrap();
    let course = make_course(dir.path());
    let state = desktop(dir.path());
    state.import_course(|| Some(course)).unwrap();
    state.rescan().unwrap();
    let id = state.courses()[0].id.clone();

    let foreign = state
        .set_course_order(
            &id,
            "",
            &owned(&["Modulo 10", "Modulo 2", "Modulo 3", "Modulo Extra"]),
        )
        .unwrap_err();
    assert_eq!(foreign.code, IpcErrorCode::InvalidOrder);

    let wrong_child = state
        .set_course_order(&id, "Modulo 2", &owned(&["Modulo 3"]))
        .unwrap_err();
    assert_eq!(wrong_child.code, IpcErrorCode::InvalidOrder);

    let unknown = state
        .set_course_order("desconhecido", "", &owned(&["Modulo 2"]))
        .unwrap_err();
    assert_eq!(unknown.code, IpcErrorCode::CourseNotFound);

    // Nothing was persisted: the automatic order continues.
    assert_eq!(module_paths(&state), ["Modulo 2", "Modulo 3", "Modulo 10"]);
}
