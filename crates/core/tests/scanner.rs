//! Testes da varredura da biblioteca.
//!
//! Todas as fixtures são criadas em diretórios temporários: nenhuma biblioteca
//! real é acessada e nenhum arquivo original é modificado.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use learnvault_core::Error;
use learnvault_core::paths::KeyMode;
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

/// Manifesto recursivo da árvore: par (caminho relativo, tamanho, instante de
/// modificação) em ordem estável. No Windows, o NTFS atualiza o mtime de
/// diretórios de forma preguiçosa, então `include_directories` é falso lá e a
/// comparação fica restrita aos arquivos.
fn manifest(root: &Path, include_directories: bool) -> BTreeSet<(String, u64, u128)> {
    let mut entries = BTreeSet::new();
    collect_manifest(root, root, include_directories, &mut entries);
    entries
}

fn collect_manifest(
    root: &Path,
    current: &Path,
    include_directories: bool,
    entries: &mut BTreeSet<(String, u64, u128)>,
) {
    for entry in fs::read_dir(current).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let metadata = entry.metadata().unwrap();
        let modified = metadata
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        if entry.file_type().unwrap().is_dir() {
            if include_directories {
                entries.insert((format!("{relative}/"), 0, modified));
            }
            collect_manifest(root, &path, include_directories, entries);
        } else {
            entries.insert((relative, metadata.len(), modified));
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
    assert_eq!(
        kind_of(&outcome, "Curso/Modulo 1/aula.mp4"),
        MediaKind::Media
    );
    assert_eq!(
        kind_of(&outcome, "Curso/Modulo 1/aula.srt"),
        MediaKind::Subtitle
    );
    assert_eq!(
        kind_of(&outcome, "Curso/Modulo 1/apoio.pdf"),
        MediaKind::Material
    );
    assert!(outcome.inaccessible.is_empty());

    let video = outcome
        .files
        .iter()
        .find(|file| file.relative_path == "Curso/Modulo 1/aula.mp4")
        .unwrap();
    assert_eq!(video.size, 5);
    assert!(video.mtime > 1_600_000_000, "mtime não foi reportado");
}

#[test]
fn known_media_and_subtitle_extensions_are_classified() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let media = ["mp4", "webm", "mkv", "m3u8"];
    let subtitles = ["srt", "vtt", "ass", "ssa"];

    for extension in media {
        write_file(root, &format!("media/arquivo.{extension}"), b"x");
    }
    for extension in subtitles {
        write_file(root, &format!("legenda/arquivo.{extension}"), b"x");
    }
    // A extensão `.ts` exige tamanho mínimo e tem teste próprio.
    write_file(root, "media/video.ts", &vec![0_u8; 2 * 1024 * 1024]);

    let outcome = scan(root).unwrap();

    for extension in media {
        assert_eq!(
            kind_of(&outcome, &format!("media/arquivo.{extension}")),
            MediaKind::Media,
            "{extension} deveria ser mídia"
        );
    }
    for extension in subtitles {
        assert_eq!(
            kind_of(&outcome, &format!("legenda/arquivo.{extension}")),
            MediaKind::Subtitle,
            "{extension} deveria ser legenda"
        );
    }
}

#[test]
fn uppercase_extensions_are_classified() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "AULA.MP4", b"x");
    write_file(root, "LEGENDA.SRT", b"x");

    let outcome = scan(root).unwrap();

    assert_eq!(kind_of(&outcome, "AULA.MP4"), MediaKind::Media);
    assert_eq!(kind_of(&outcome, "LEGENDA.SRT"), MediaKind::Subtitle);
}

#[test]
fn files_without_extension_or_with_trailing_dot_are_material() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "README", b"x");
    write_file(root, "aula.mp4.srt", b"x");
    // O NTFS não representa nomes terminados em ponto: no Windows, "aula." é
    // criado como "aula" e o caso não existe para a varredura do host.
    if !cfg!(windows) {
        write_file(root, "aula.", b"x");
    }

    let outcome = scan(root).unwrap();

    assert_eq!(kind_of(&outcome, "README"), MediaKind::Material);
    if !cfg!(windows) {
        assert_eq!(kind_of(&outcome, "aula."), MediaKind::Material);
    }
    assert_eq!(kind_of(&outcome, "aula.mp4.srt"), MediaKind::Subtitle);
}

#[test]
fn typescript_like_files_are_media_only_at_one_megabyte() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "abaixo.ts", &vec![0_u8; (1024 * 1024) - 1]);
    write_file(root, "limite.ts", &vec![0_u8; 1024 * 1024]);

    let outcome = scan(root).unwrap();

    assert_eq!(kind_of(&outcome, "abaixo.ts"), MediaKind::Material);
    assert_eq!(kind_of(&outcome, "limite.ts"), MediaKind::Media);
}

#[test]
fn temporary_and_hidden_directories_are_not_walked() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "temp/oculto.mp4", b"x");
    write_file(root, "TEMP/oculto.mp4", b"x");
    write_file(root, "temp_download/oculto.mp4", b"x");
    write_file(root, "TEMP_EXTRA/oculto.mp4", b"x");
    write_file(root, "0____/oculto.mp4", b"x");
    write_file(root, ".git/config", b"x");
    write_file(root, ".obsidian/app.json", b"x");
    write_file(root, "Curso/aula.mp4", b"x");
    // O filtro de diretórios ignorados não vale para arquivos com o mesmo nome.
    write_file(root, "Curso/temp.mp4", b"x");

    let outcome = scan(root).unwrap();

    assert_eq!(
        relative_paths(&outcome),
        vec!["Curso/aula.mp4".to_owned(), "Curso/temp.mp4".to_owned()]
    );
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
fn a_symlinked_root_is_not_traversed() {
    let directory = tempfile::tempdir().unwrap();
    let real = directory.path().join("real");
    fs::create_dir_all(&real).unwrap();
    fs::write(real.join("aula.mp4"), b"x").unwrap();
    let link = directory.path().join("atalho");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let outcome = scan(&link).unwrap();

    assert!(
        outcome.files.is_empty(),
        "um link no topo da raiz não pode ser seguido: {:?}",
        relative_paths(&outcome)
    );
    assert!(outcome.inaccessible.is_empty());
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
        eprintln!(
            "AVISO: permissões ignoradas neste ambiente (execução privilegiada); \
             o caso de diretório ilegível NÃO foi verificado nesta execução"
        );
        return;
    }

    let outcome = scan(root).unwrap();

    assert_eq!(relative_paths(&outcome), vec!["Curso/aula.mp4".to_owned()]);
    assert_eq!(outcome.inaccessible, vec!["Bloqueado".to_owned()]);
}

#[cfg(unix)]
#[test]
fn unreadable_root_returns_a_typed_error() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("biblioteca");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("aula.mp4"), b"x").unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();

    if fs::read_dir(&root).is_ok() {
        eprintln!(
            "AVISO: permissões ignoradas neste ambiente (execução privilegiada); \
             o caso de raiz ilegível NÃO foi verificado nesta execução"
        );
        return;
    }

    match scan(&root) {
        Err(Error::LibraryRootUnreadable { path, .. }) => assert_eq!(path, root),
        other => panic!("erro inesperado: {other:?}"),
    }
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
fn an_empty_library_returns_no_files() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    write_file(root, "temp/ignorado.mp4", b"x");

    let outcome = scan(root).unwrap();

    assert!(outcome.files.is_empty());
    assert!(outcome.inaccessible.is_empty());
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

    let before = manifest(root, !cfg!(windows));
    let outcome = scan(root).unwrap();
    let after = manifest(root, !cfg!(windows));

    assert!(!outcome.files.is_empty());
    assert_eq!(
        before, after,
        "a varredura não pode modificar a biblioteca (caminho, tamanho ou mtime)"
    );
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
        assert!(!file.relative_path.contains(".."));
        assert!(
            !file.relative_path.contains(&root_text),
            "caminho absoluto vazou: {}",
            file.relative_path
        );

        let expected = match KeyMode::for_host() {
            KeyMode::Sensitive => file.relative_path.clone(),
            KeyMode::Insensitive => file.relative_path.to_ascii_lowercase(),
        };
        assert_eq!(file.relative_path_key, expected);
    }
}
