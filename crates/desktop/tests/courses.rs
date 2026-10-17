//! Testes da persistência de cursos do desktop.
//!
//! Exercitam a camada de configuração diretamente: round-trip do arquivo versionado,
//! migração do formato antigo (raiz única → um curso), revalidação de pastas removidas,
//! recusa de escrever a configuração dentro de um curso e serialização de gravações
//! concorrentes. Nenhum caminho cruza o IPC; nenhum teste toca dados reais.

use std::fs;
use std::path::{Path, PathBuf};

use learnvault_core::CourseRoot;
use learnvault_desktop::{CourseEntry, LibraryError, load_courses, store_courses};

/// Cria um diretório de curso válido e devolve o caminho escolhido.
fn make_course(root: &Path, name: &str) -> PathBuf {
    let course = root.join(name);
    fs::create_dir_all(&course).unwrap();
    course
}

/// Curso registrado com identidade derivada do caminho canônico.
fn entry(path: &Path) -> CourseEntry {
    CourseEntry::from_root(CourseRoot::resolve(path).unwrap())
}

#[test]
fn round_trip_preserves_courses_and_drops_removed_roots() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let first = make_course(dir.path(), "Física");
    let second = make_course(dir.path(), "Matemática");
    let courses = vec![entry(&first), entry(&second)];

    store_courses(&config, &courses).unwrap();
    let loaded = load_courses(&config);
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].id, courses[0].id);
    assert_eq!(loaded[1].id, courses[1].id);

    // A pasta removida antes da recarga é descartada em silêncio, sem reescrever o arquivo.
    fs::remove_dir_all(&second).unwrap();
    let reloaded = load_courses(&config);
    assert_eq!(reloaded.len(), 1);
    assert_eq!(reloaded[0].id, courses[0].id);
}

#[test]
fn legacy_single_root_migrates_to_one_course_with_the_historic_identity() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(&config).unwrap();
    let course = make_course(dir.path(), "Acervo");
    let legacy = serde_json::json!({
        "version": 1,
        "library_root": course.canonicalize().unwrap()
    });
    fs::write(config.join("library.json"), legacy.to_string()).unwrap();

    let loaded = load_courses(&config);

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "library-1");
}

#[test]
fn unknown_versions_and_corrupt_files_load_as_no_courses() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(&config).unwrap();

    fs::write(config.join("library.json"), b"{ corrompido").unwrap();
    assert!(load_courses(&config).is_empty());

    let unknown = serde_json::json!({ "version": 99, "courses": [] });
    fs::write(config.join("library.json"), unknown.to_string()).unwrap();
    assert!(load_courses(&config).is_empty());
}

#[test]
fn writing_the_config_inside_a_course_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let course = make_course(dir.path(), "Curso");
    let inside = course.join("config");
    let courses = vec![entry(&course)];

    let error = store_courses(&inside, &courses).unwrap_err();

    assert_eq!(error, LibraryError::WriteFailed);
    assert!(
        !inside.exists(),
        "nada pode ser criado dentro da pasta do curso"
    );
}

#[test]
fn store_is_atomic_and_leaves_no_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let course = make_course(dir.path(), "Curso");

    store_courses(&config, &[entry(&course)]).unwrap();
    store_courses(&config, &[]).unwrap();
    store_courses(&config, &[entry(&course)]).unwrap();

    let leftovers: Vec<_> = fs::read_dir(&config)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temporários não podem sobrar: {leftovers:?}"
    );
    assert_eq!(load_courses(&config).len(), 1);
}

#[test]
fn concurrent_stores_leave_a_valid_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(&config).unwrap();
    let roots: Vec<PathBuf> = (0..4)
        .map(|index| make_course(dir.path(), &format!("curso{index}")))
        .collect();
    let lists: Vec<Vec<CourseEntry>> = roots.iter().map(|root| vec![entry(root)]).collect();

    std::thread::scope(|scope| {
        for list in &lists {
            let config = config.clone();
            scope.spawn(move || store_courses(&config, list).unwrap());
        }
    });

    let loaded = load_courses(&config);
    assert_eq!(
        loaded.len(),
        1,
        "a configuração precisa ser o resultado de uma gravação completa"
    );
    assert!(
        roots.iter().any(|root| entry(root).id == loaded[0].id),
        "a configuração final precisa ser uma das listas gravadas"
    );
}

#[test]
fn a_course_replaced_by_a_link_is_dropped_on_load() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    let course = make_course(dir.path(), "Curso");
    store_courses(&config, &[entry(&course)]).unwrap();

    fs::remove_dir_all(&course).unwrap();
    let target = make_course(dir.path(), "alvo");

    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(&target, &course).is_ok();
    #[cfg(windows)]
    let created = std::os::windows::fs::symlink_dir(&target, &course).is_ok();
    #[cfg(not(any(unix, windows)))]
    let created = false;

    if !created {
        eprintln!("pulado: criar symlink/junction exige privilégio nesta plataforma");
        return;
    }

    assert!(
        load_courses(&config).is_empty(),
        "uma pasta de curso que virou link é descartada na leitura"
    );
}
