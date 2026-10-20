//! Regression coverage for stable natural ordering in persisted catalogs.

use learnvault_core::paths::relative_path_key;
use learnvault_core::{CatalogNode, Database, KeyMode, build_catalog};

const LIB: &str = "ordering-regression";
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn key(path: &str) -> String {
    relative_path_key(path, KeyMode::for_host()).expect("valid test path")
}

fn database() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    Ok(db)
}

fn insert(db: &Database, id: &str, path: &str) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, ?2, ?3, ?4, 1024, 1760000000, 'available')",
        rusqlite::params![id, LIB, path, key(path)],
    )
}

fn node<'a>(nodes: &'a [CatalogNode], path: &str) -> &'a CatalogNode {
    nodes
        .iter()
        .find(|candidate| candidate.path == path)
        .expect("catalog node exists")
}

#[test]
fn natural_order_handles_unicode_and_arbitrarily_large_digit_runs() -> TestResult {
    let db = database()?;
    for (id, path) in [
        ("m10", "Curso/Éclair 10/Aula.mp4"),
        ("m2", "Curso/Éclair 2/Aula.mp4"),
        ("u10", "Curso/Tóquio 10/Aula.mp4"),
        ("u2", "Curso/Tóquio 2/Aula.mp4"),
        (
            "h10",
            "Curso/Parte 9999999999999999999999999999999999999999/Aula.mp4",
        ),
        (
            "h2",
            "Curso/Parte 8888888888888888888888888888888888888888/Aula.mp4",
        ),
    ] {
        insert(&db, id, path)?;
    }
    let catalog = build_catalog(&db, LIB)?;
    let course = &catalog.categories[0];
    let names: Vec<&str> = course.children.iter().map(|n| n.path.as_str()).collect();
    assert_eq!(
        names,
        [
            "Curso/Parte 8888888888888888888888888888888888888888",
            "Curso/Parte 9999999999999999999999999999999999999999",
            "Curso/Tóquio 2",
            "Curso/Tóquio 10",
            "Curso/Éclair 2",
            "Curso/Éclair 10",
        ]
    );
    Ok(())
}

#[test]
fn zero_prefixes_nested_paths_and_material_files_sort_naturally() -> TestResult {
    let db = database()?;
    for (id, path) in [
        ("m1", "Curso/Modulo 1/Aula 10/Arquivo 10.mp4"),
        ("m2", "Curso/Modulo 1/Aula 2/Arquivo 10.mp4"),
        ("m3", "Curso/Modulo 1/Aula 2/Arquivo 2.mp4"),
        ("m4", "Curso/Modulo 1/Aula 2/Arquivo 02.mp4"),
        ("m5", "Curso/Modulo 1/Aula 2/Arquivo 002.mp4"),
        ("m6", "Curso/Modulo 1/Aula 2/Material 10.pdf"),
        ("m7", "Curso/Modulo 1/Aula 2/Material 2.pdf"),
        ("m8", "Curso/Modulo 1/Aula 2/Legenda 10.srt"),
        ("m9", "Curso/Modulo 1/Aula 2/Legenda 2.srt"),
    ] {
        insert(&db, id, path)?;
    }
    let catalog = build_catalog(&db, LIB)?;
    let course = &catalog.categories[0];
    let module = node(&course.children, "Curso/Modulo 1");
    let lessons: Vec<&str> = module.children.iter().map(|n| n.path.as_str()).collect();
    assert_eq!(lessons, ["Curso/Modulo 1/Aula 2", "Curso/Modulo 1/Aula 10"]);
    let aula2 = node(&module.children, "Curso/Modulo 1/Aula 2");
    let files: Vec<&str> = aula2.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        files,
        [
            "Arquivo 002.mp4",
            "Arquivo 02.mp4",
            "Arquivo 2.mp4",
            "Arquivo 10.mp4",
            "Legenda 2.srt",
            "Legenda 10.srt",
            "Material 2.pdf",
            "Material 10.pdf"
        ]
    );
    Ok(())
}

#[test]
fn ordering_does_not_depend_on_database_row_insertion_order() -> TestResult {
    let paths = [
        ("two", "Curso/Modulo 2/Aula 2/video.mp4"),
        ("ten", "Curso/Modulo 10/Aula 10/video.mp4"),
        ("three", "Curso/Modulo 3/Aula 3/video.mp4"),
    ];
    let forward = database()?;
    let reverse = database()?;
    for (id, path) in paths {
        insert(&forward, id, path)?;
    }
    for (id, path) in paths.into_iter().rev() {
        insert(&reverse, id, path)?;
    }
    let a = build_catalog(&forward, LIB)?;
    let b = build_catalog(&reverse, LIB)?;
    assert_eq!(
        a, b,
        "same persisted paths produce same tree independent of row insertion"
    );
    let course = &a.categories[0];
    let modules: Vec<&str> = course.children.iter().map(|n| n.path.as_str()).collect();
    assert_eq!(
        modules,
        ["Curso/Modulo 2", "Curso/Modulo 3", "Curso/Modulo 10"]
    );
    Ok(())
}
