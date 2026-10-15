//! Testes da seleção e persistência da raiz da biblioteca desktop.
//!
//! Não sobem o webview nem o seletor nativo: exercitam a validação da raiz, a gravação
//! atômica da configuração e a releitura/revalidação no startup. O alvo é o contrato de
//! segurança observável — a biblioteca nunca é escrita, a configuração vive fora dela, a
//! persistência é versionada e nenhuma mensagem de erro carrega caminho do sistema.

use std::fs;
use std::path::{Path, PathBuf};

use learnvault_desktop::{
    CONFIG_FILE_NAME, CONFIG_VERSION, LibraryError, LibraryRoot, load_library_root,
    store_library_root,
};

/// Primeiro nome de temporário que o escritor tenta: o formato histórico
/// `.{CONFIG_FILE_NAME}.{pid}.tmp`. A suíte o usa para provar que um caminho pré-existente
/// nunca é reaproveitado, sobrescrito nem seguido.
fn first_temporary_path(config_dir: &Path) -> PathBuf {
    config_dir.join(format!(".{CONFIG_FILE_NAME}.{}.tmp", std::process::id()))
}

/// Cria um diretório de biblioteca vazio e devolve a raiz já validada.
fn library(dir: &Path) -> LibraryRoot {
    let root = dir.join("biblioteca");
    fs::create_dir(&root).unwrap();
    LibraryRoot::resolve(&root).unwrap()
}

/// Espera a recusa de um caminho e devolve o erro estável.
fn reject(path: &Path) -> LibraryError {
    LibraryRoot::resolve(path).expect_err("o caminho deveria ser recusado como raiz")
}

#[test]
fn a_missing_path_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        reject(&dir.path().join("inexistente")),
        LibraryError::Missing
    );
}

#[test]
fn a_regular_file_is_not_a_library_root() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("arquivo.txt");
    fs::write(&file, b"nao sou diretorio").unwrap();
    assert_eq!(reject(&file), LibraryError::NotADirectory);
}

#[test]
fn a_directory_is_canonicalized_and_readable() {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("biblioteca");
    fs::create_dir(&raw).unwrap();

    let root = LibraryRoot::resolve(&raw).unwrap();
    let canonical = fs::canonicalize(&raw).unwrap();

    assert_eq!(root.as_path(), canonical.as_path());
    assert!(
        fs::read_dir(root.as_path()).is_ok(),
        "a raiz validada precisa permanecer legível"
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_root_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = dir.path().join("atalho");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    assert_eq!(reject(&link), LibraryError::Link);
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_file_is_rejected_before_the_directory_check() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("arquivo.txt");
    fs::write(&file, b"conteudo").unwrap();
    let link = dir.path().join("atalho");
    std::os::unix::fs::symlink(&file, &link).unwrap();

    assert_eq!(reject(&link), LibraryError::Link);
}

#[cfg(unix)]
#[test]
fn an_unreadable_directory_is_rejected_when_the_platform_enforces_it() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("biblioteca");
    fs::create_dir(&raw).unwrap();
    fs::set_permissions(&raw, fs::Permissions::from_mode(0o000)).unwrap();

    // Um usuário privilegiado (root) ignora o modo e continua lendo; nesse caso a
    // validação de leitura não é observável e o teste é pulado com registro.
    if fs::read_dir(&raw).is_ok() {
        eprintln!("pulado: o usuário consegue ler mesmo sem permissão de leitura");
        return;
    }

    assert_eq!(reject(&raw), LibraryError::NotReadable);
}

#[cfg(windows)]
#[test]
fn a_symlink_or_junction_root_is_rejected_on_windows() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = dir.path().join("atalho");
    if std::os::windows::fs::symlink_dir(&real, &link).is_err() {
        eprintln!("pulado: criar symlink no Windows exige privilégio");
        return;
    }
    assert_eq!(reject(&link), LibraryError::Link);
}

#[test]
fn a_missing_configuration_dir_loads_as_unconfigured() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load_library_root(dir.path()).is_none());
}

#[test]
fn store_then_load_roundtrips_the_canonical_root() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let root = library(dir.path());

    store_library_root(&config, &root).unwrap();

    let loaded = load_library_root(&config).expect("a configuração deveria recarregar");
    assert_eq!(loaded.as_path(), root.as_path());
}

#[test]
fn the_configuration_file_is_versioned_and_holds_only_the_canonical_path() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let root = library(dir.path());

    store_library_root(&config, &root).unwrap();

    let raw = fs::read_to_string(config.join(CONFIG_FILE_NAME)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(value["version"], serde_json::json!(CONFIG_VERSION));
    assert_eq!(
        value["library_root"].as_str().unwrap(),
        root.as_path().to_str().unwrap()
    );
}

#[test]
fn a_corrupt_configuration_loads_as_unconfigured() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir(&config).unwrap();
    fs::write(config.join(CONFIG_FILE_NAME), b"{ nao e json").unwrap();

    assert!(load_library_root(&config).is_none());
}

#[test]
fn an_unknown_configuration_version_loads_as_unconfigured() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir(&config).unwrap();
    fs::write(
        config.join(CONFIG_FILE_NAME),
        br#"{ "version": 999, "library_root": "/caminho/antigo" }"#,
    )
    .unwrap();

    assert!(load_library_root(&config).is_none());
}

#[test]
fn a_removed_library_root_loads_as_unconfigured() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let root = library(dir.path());
    store_library_root(&config, &root).unwrap();

    fs::remove_dir_all(root.as_path()).unwrap();

    assert!(load_library_root(&config).is_none());
}

#[test]
fn storing_is_atomic_and_leaves_no_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let root = library(dir.path());

    store_library_root(&config, &root).unwrap();
    // Uma segunda gravação substitui atomicamente a configuração anterior.
    store_library_root(&config, &root).unwrap();

    let entries: Vec<String> = fs::read_dir(&config)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        entries,
        vec![CONFIG_FILE_NAME.to_owned()],
        "só a configuração final pode permanecer: {entries:?}"
    );
}

#[test]
fn storing_never_writes_inside_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let root = library(dir.path());

    store_library_root(&config, &root).unwrap();

    let entries = fs::read_dir(root.as_path())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        entries.is_empty(),
        "a biblioteca permanece somente leitura: {entries:?}"
    );
}

#[test]
fn storing_creates_the_configuration_dir_outside_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("app-config").join("learnvault");
    let root = library(dir.path());

    store_library_root(&config, &root).unwrap();

    assert!(config.join(CONFIG_FILE_NAME).exists());
    assert!(
        !config.starts_with(root.as_path()),
        "a configuração não pode viver dentro da biblioteca"
    );
}

#[test]
fn storing_rejects_a_configuration_dir_inside_the_library_without_creating_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = library(dir.path());
    let nested = root.as_path().join("config");

    assert_eq!(
        store_library_root(&nested, &root),
        Err(LibraryError::WriteFailed)
    );
    assert!(
        !nested.exists(),
        "nada pode ser criado dentro da biblioteca, nem o diretório recusado"
    );
}

#[test]
fn errors_and_debug_never_expose_the_filesystem_path() {
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("Cursos secretos");
    let error = reject(&secret);
    let rendered = format!("{error}");
    assert!(
        !rendered.contains("Cursos secretos"),
        "a mensagem de erro não pode expor o caminho: {rendered}"
    );

    fs::create_dir(&secret).unwrap();
    let root = LibraryRoot::resolve(&secret).unwrap();
    let debug = format!("{root:?}");
    assert!(
        !debug.contains("Cursos secretos"),
        "o Debug da raiz não pode expor o caminho: {debug}"
    );
}

#[test]
fn storing_repeatedly_swaps_the_root_without_leftovers() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let first = library(dir.path());
    let second = dir.path().join("biblioteca-2");
    fs::create_dir(&second).unwrap();
    let second = LibraryRoot::resolve(&second).unwrap();

    store_library_root(&config, &first).unwrap();
    assert_eq!(load_library_root(&config).unwrap().as_path(), first.as_path());

    store_library_root(&config, &second).unwrap();
    assert_eq!(
        load_library_root(&config).unwrap().as_path(),
        second.as_path()
    );

    store_library_root(&config, &first).unwrap();
    assert_eq!(load_library_root(&config).unwrap().as_path(), first.as_path());

    assert_eq!(
        fs::read_dir(&config).unwrap().count(),
        1,
        "a troca de raiz não pode deixar temporários"
    );
}

#[test]
fn a_pre_existing_temporary_file_is_never_reused() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(&config).unwrap();
    let root = library(dir.path());
    let squatter = first_temporary_path(&config);
    fs::write(&squatter, b"lixo pre-existente").unwrap();

    store_library_root(&config, &root).unwrap();

    assert_eq!(
        fs::read(&squatter).unwrap(),
        b"lixo pre-existente",
        "um temporário pré-existente não pode ser reaproveitado nem sobrescrito"
    );
    assert!(load_library_root(&config).is_some());
    assert_eq!(
        fs::read_dir(&config).unwrap().count(),
        2,
        "a configuração final coexiste com o arquivo pré-existente"
    );
}

#[cfg(unix)]
#[test]
fn a_pre_existing_temporary_symlink_is_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(&config).unwrap();
    let root = library(dir.path());
    let victim = dir.path().join("vitima.txt");
    fs::write(&victim, b"intacto").unwrap();
    std::os::unix::fs::symlink(&victim, first_temporary_path(&config)).unwrap();

    store_library_root(&config, &root).unwrap();

    assert_eq!(
        fs::read(&victim).unwrap(),
        b"intacto",
        "gravar o temporário não pode seguir um symlink pré-existente"
    );
    assert!(load_library_root(&config).is_some());
}

#[test]
fn concurrent_stores_never_corrupt_the_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let roots: Vec<LibraryRoot> = (0..4)
        .map(|index| {
            let path = dir.path().join(format!("lib{index}"));
            fs::create_dir(&path).unwrap();
            LibraryRoot::resolve(&path).unwrap()
        })
        .collect();

    std::thread::scope(|scope| {
        for root in &roots {
            let config = config.clone();
            scope.spawn(move || {
                for _ in 0..10 {
                    store_library_root(&config, root).unwrap();
                }
            });
        }
    });

    let loaded = load_library_root(&config).expect("a configuração precisa permanecer válida");
    assert!(
        roots.iter().any(|root| root.as_path() == loaded.as_path()),
        "a raiz final precisa ser uma das gravadas"
    );
    assert_eq!(
        fs::read_dir(&config).unwrap().count(),
        1,
        "gravações concorrentes não podem deixar temporários"
    );
}
