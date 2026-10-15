//! Testes da varredura da biblioteca.
//!
//! Todas as fixtures são criadas em diretórios temporários: nenhuma biblioteca
//! real é acessada e nenhum arquivo original é modificado.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use learnvault_core::Error;
use learnvault_core::scanner::{MediaKind, ScanOutcome, scan};

fn write_file(root: &Path, relative: &str, bytes: &[u8]) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, bytes).unwrap();
    path
}

fn relative_paths(outcome: &ScanOutcome) -> Vec<String> {
    outcome
        .files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect()
}

fn kind_of(outcome: &ScanOutcome, relative: &str) -> MediaKind {
    outcome
        .files
        .iter()
        .find(|file| file.relative_path == relative)
        .unwrap_or_else(|| panic!("{relative} não foi encontrado no resultado"))
        .kind
}

/// Manifesto recursivo da árvore: par (caminho relativo, tamanho) em ordem estável.
fn manifest(root: &Path) -> BTreeSet<(String, u64)> {
    let mut entries = BTreeSet::new();
    collect_manifest(root, root, &mut entries);
    entries
}

fn collect_manifest(root: &Path, current: &Path, entries: &mut BTreeSet<(String, u64)>) {
    for entry in fs::read_dir(current).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");

        if entry.file_type().unwrap().is_dir() {
            entries.insert((format!("{relative}/"), 0));
            collect_manifest(root, &path, entries);
        } else {
            entries.insert((relative, entry.metadata().unwrap().len()));
        }
    }
}

#[test]
fn nested_tree_is_classified() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "Curso/Modulo 1/aula.mp4", b"video");
    write_file(root, "Curso/Modulo 1/aula.srt", b"legenda");
    write_file(root, "Curso/Modulo 1/apoio.pdf", b"material");

    let outcome = scan(root).unwrap();

    assert_eq!(
        relative_paths(&outcome),
        vec![
            "Curso/Modulo 1/apoio.pdf".to_owned(),
            "Curso/Modulo 1/aula.mp4".to_owned(),
            "Curso/Modulo 1/aula.srt".to_owned(),
        ]
    );
    assert_eq!(kind_of(&outcome, "Curso/Modulo 1/aula.mp4"), MediaKind::Media);
    assert_eq!(
        kind_of(&outcome, "Curso/Modulo 1/aula.srt"),
        MediaKind::Subtitle
    );
    assert_eq!(
        kind_of(&outcome, "Curso/Modulo 1/apoio.pdf"),
        MediaKind::Material
    );
    assert!(outcome.inaccessible.is_empty());
}

#[test]
fn small_typescript_like_file_is_material_and_large_one_is_media() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "pequeno.ts", &[0_u8; 16]);
    write_file(root, "grande.ts", &vec![0_u8; 2 * 1024 * 1024]);

    let outcome = scan(root).unwrap();

    assert_eq!(kind_of(&outcome, "pequeno.ts"), MediaKind::Material);
    assert_eq!(kind_of(&outcome, "grande.ts"), MediaKind::Media);
}

#[test]
fn temporary_directories_are_not_walked() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "temp/oculto.mp4", b"x");
    write_file(root, "temp_download/oculto.mp4", b"x");
    write_file(root, "TEMP_EXTRA/oculto.mp4", b"x");
    write_file(root, "0____/oculto.mp4", b"x");
    write_file(root, "Curso/aula.mp4", b"x");

    let outcome = scan(root).unwrap();

    assert_eq!(relative_paths(&outcome), vec!["Curso/aula.mp4".to_owned()]);
}

#[test]
fn hidden_files_are_ignored() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "Curso/.oculto.mp4", b"x");
    write_file(root, ".env", b"x");
    write_file(root, "Curso/aula.mp4", b"x");

    let outcome = scan(root).unwrap();

    assert_eq!(relative_paths(&outcome), vec!["Curso/aula.mp4".to_owned()]);
}

#[cfg(unix)]
#[test]
fn directory_symlink_outside_the_library_is_not_followed() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("biblioteca");
    fs::create_dir_all(&root).unwrap();

    let outside = directory.path().join("fora");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("segredo.mp4"), b"x").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("atalho")).unwrap();

    write_file(&root, "Curso/aula.mp4", b"x");

    let outcome = scan(&root).unwrap();

    assert_eq!(relative_paths(&outcome), vec!["Curso/aula.mp4".to_owned()]);
}

#[cfg(unix)]
#[test]
fn file_symlink_is_not_reported() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let target = root.join("real.mp4");
    fs::write(&target, b"x").unwrap();
    std::os::unix::fs::symlink(&target, root.join("atalho.mp4")).unwrap();

    let outcome = scan(root).unwrap();

    assert_eq!(relative_paths(&outcome), vec!["real.mp4".to_owned()]);
}

#[cfg(unix)]
#[test]
fn unreadable_directory_is_recorded_and_the_scan_continues() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "Curso/aula.mp4", b"x");
    let locked = root.join("Bloqueado");
    fs::create_dir_all(&locked).unwrap();
    fs::write(locked.join("dentro.mp4"), b"x").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    if fs::read_dir(&locked).is_ok() {
        eprintln!("permissões ignoradas neste ambiente (execução privilegiada); caso não verificado");
        return;
    }

    let outcome = scan(root).unwrap();

    assert_eq!(relative_paths(&outcome), vec!["Curso/aula.mp4".to_owned()]);
    assert_eq!(outcome.inaccessible, vec!["Bloqueado".to_owned()]);
}

#[test]
fn files_are_sorted_by_relative_path() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "zebra/aula.mp4", b"x");
    write_file(root, "alfa/aula.mp4", b"x");
    write_file(root, "meio/aula.mp4", b"x");

    let outcome = scan(root).unwrap();

    assert_eq!(
        relative_paths(&outcome),
        vec![
            "alfa/aula.mp4".to_owned(),
            "meio/aula.mp4".to_owned(),
            "zebra/aula.mp4".to_owned(),
        ]
    );
}

#[test]
fn non_directory_root_returns_a_typed_error() {
    let directory = tempfile::tempdir().unwrap();
    let file = write_file(directory.path(), "arquivo.txt", b"x");

    match scan(&file) {
        Err(Error::LibraryRootNotDirectory { path }) => assert_eq!(path, file),
        other => panic!("erro inesperado: {other:?}"),
    }
}

#[test]
fn missing_root_returns_a_typed_error() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("nao-existe");

    match scan(&missing) {
        Err(Error::LibraryRootMissing { path }) => assert_eq!(path, missing),
        other => panic!("erro inesperado: {other:?}"),
    }
}

#[test]
fn scanning_does_not_modify_the_library() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "Curso/Modulo 1/aula.mp4", b"video");
    write_file(root, "Curso/Modulo 1/aula.srt", b"legenda");
    write_file(root, "Curso/Modulo 1/apoio.pdf", b"material");
    write_file(root, "temp_ignorado/rascunho.mp4", b"x");

    let before = manifest(root);
    let outcome = scan(root).unwrap();
    let after = manifest(root);

    assert!(!outcome.files.is_empty());
    assert_eq!(before, after, "a varredura não pode modificar a biblioteca");
}

#[test]
fn reported_paths_are_relative_to_the_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "Curso/Modulo 1/aula.mp4", b"x");

    let outcome = scan(root).unwrap();
    let root_text = root.to_string_lossy().replace('\\', "/");

    assert!(!outcome.files.is_empty());
    for file in &outcome.files {
        assert!(!file.relative_path.starts_with('/'));
        assert!(!file.relative_path.starts_with('\\'));
        assert!(
            !file.relative_path.contains(&root_text),
            "caminho absoluto vazou: {}",
            file.relative_path
        );
        let sensitive = file.relative_path_key.as_str() == file.relative_path.as_str();
        let insensitive =
            file.relative_path_key.as_str() == file.relative_path.to_ascii_lowercase().as_str();
        assert!(
            sensitive || insensitive,
            "chave de caminho inesperada: {}",
            file.relative_path_key
        );
    }
}
