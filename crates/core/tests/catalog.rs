//! Testes da derivação do catálogo a partir dos dados persistidos.
//!
//! Todos os casos partem de um banco em memória com linhas de `media` inseridas
//! diretamente: o catálogo é uma função dos dados, não do sistema de arquivos.

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Availability, CatalogNode, Database, KeyMode, MediaKind, NodeRole, build_catalog,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "bib-1";

fn key(path: &str) -> String {
    relative_path_key(path, KeyMode::for_host()).expect("caminho de teste deve ser válido")
}

fn library_db() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    Ok(db)
}

fn insert_media(
    db: &Database,
    media_id: &str,
    path: &str,
    size: i64,
    availability: &str,
) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, ?2, ?3, ?4, ?5, 1760000000, ?6)",
        rusqlite::params![media_id, LIB, path, key(path), size, availability],
    )
}

fn media(db: &Database, media_id: &str, path: &str) -> rusqlite::Result<usize> {
    insert_media(db, media_id, path, 1024, "available")
}

fn insert_scan(db: &Database, complete: i64) -> rusqlite::Result<i64> {
    db.connection().execute(
        "INSERT INTO scan (library_id, status, completed_at, complete)
         VALUES (?1, 'completed', 1760000000, ?2)",
        rusqlite::params![LIB, complete],
    )?;
    Ok(db.connection().last_insert_rowid())
}

fn insert_unverified(db: &Database, scan_id: i64, path: &str) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO scan_inaccessible (scan_id, relative_path) VALUES (?1, ?2)",
        rusqlite::params![scan_id, path],
    )
}

fn node<'a>(nodes: &'a [CatalogNode], path: &str) -> &'a CatalogNode {
    nodes
        .iter()
        .find(|candidate| candidate.path == path)
        .unwrap_or_else(|| {
            let found: Vec<&str> = nodes.iter().map(|n| n.path.as_str()).collect();
            panic!("nó {path} não encontrado; disponíveis: {found:?}");
        })
}

#[test]
fn a_four_level_tree_is_derived_bottom_up() -> TestResult {
    let db = library_db()?;
    media(
        &db,
        "m1",
        "Programação/Go Essencial/Módulo 1/Aula 1/aula.mp4",
    )?;

    let catalog = build_catalog(&db, LIB)?;

    assert_eq!(catalog.categories.len(), 1);
    let category = &catalog.categories[0];
    assert_eq!(category.path, "Programação");
    assert_eq!(category.title, "Programação");
    assert_eq!(category.role, NodeRole::Category);

    let course = node(&category.children, "Programação/Go Essencial");
    assert_eq!(course.role, NodeRole::Course);
    assert_eq!(course.title, "Go Essencial");

    let module = node(&course.children, "Programação/Go Essencial/Módulo 1");
    assert_eq!(module.role, NodeRole::Module);

    let lesson = node(&module.children, "Programação/Go Essencial/Módulo 1/Aula 1");
    assert_eq!(lesson.role, NodeRole::Lesson);
    assert_eq!(lesson.files.len(), 1);

    let file = &lesson.files[0];
    assert_eq!(file.media_id, "m1");
    assert_eq!(file.name, "aula.mp4");
    assert_eq!(
        file.path,
        "Programação/Go Essencial/Módulo 1/Aula 1/aula.mp4"
    );
    assert_eq!(file.kind, MediaKind::Media);
    assert_eq!(file.availability, Availability::Available);
    assert!(!file.unverified);
    Ok(())
}

#[test]
fn a_missing_course_level_is_omitted() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "Cat/Curso/Aula/aula.mp4")?;

    let catalog = build_catalog(&db, LIB)?;
    let category = &catalog.categories[0];

    // Aula = diretório com mídia; módulo = pai; curso = pai do módulo, que aqui é a
    // própria categoria e por isso não é criado.
    let module = node(&category.children, "Cat/Curso");
    assert_eq!(module.role, NodeRole::Module);
    let lesson = node(&module.children, "Cat/Curso/Aula");
    assert_eq!(lesson.role, NodeRole::Lesson);
    Ok(())
}

#[test]
fn a_missing_module_level_is_omitted() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "Cat/Curso/aula.mp4")?;

    let catalog = build_catalog(&db, LIB)?;
    let category = &catalog.categories[0];

    let lesson = node(&category.children, "Cat/Curso");
    assert_eq!(lesson.role, NodeRole::Lesson);
    assert_eq!(lesson.files.len(), 1);
    Ok(())
}

#[test]
fn files_at_the_library_root_are_preserved() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "aula.mp4")?;

    let catalog = build_catalog(&db, LIB)?;

    assert!(catalog.categories.is_empty());
    assert_eq!(catalog.loose_files.len(), 1);
    assert_eq!(catalog.loose_files[0].media_id, "m1");
    assert_eq!(catalog.loose_files[0].name, "aula.mp4");
    Ok(())
}

#[test]
fn files_directly_in_a_category_are_preserved() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "Programação/aula1.mp4")?;

    let catalog = build_catalog(&db, LIB)?;
    let category = &catalog.categories[0];

    assert_eq!(category.path, "Programação");
    assert!(category.children.is_empty());
    assert_eq!(category.loose_files.len(), 1);
    assert_eq!(category.loose_files[0].media_id, "m1");
    Ok(())
}

#[test]
fn deeper_lessons_keep_every_file_and_nest_deterministically() -> TestResult {
    let db = library_db()?;
    media(
        &db,
        "m1",
        "Programação/Go Essencial/Módulo 1/Aula 1/aula.mp4",
    )?;
    media(
        &db,
        "m2",
        "Programação/Go Essencial/Módulo 1/Aula 1/extras/mais.mp4",
    )?;

    let catalog = build_catalog(&db, LIB)?;
    let category = &catalog.categories[0];
    let course = node(&category.children, "Programação/Go Essencial");
    assert_eq!(course.role, NodeRole::Course);
    let module = node(&course.children, "Programação/Go Essencial/Módulo 1");
    assert_eq!(module.role, NodeRole::Module);

    // A primeira aula reivindica `Aula 1` como aula; a segunda cria `extras` como
    // outra aula, aninhada. Nenhum arquivo é descartado.
    let lesson = node(&module.children, "Programação/Go Essencial/Módulo 1/Aula 1");
    assert_eq!(lesson.role, NodeRole::Lesson);
    assert_eq!(lesson.files.len(), 1);
    let nested = node(
        &lesson.children,
        "Programação/Go Essencial/Módulo 1/Aula 1/extras",
    );
    assert_eq!(nested.role, NodeRole::Lesson);
    assert_eq!(nested.files[0].media_id, "m2");
    Ok(())
}

#[test]
fn titles_are_cleaned_and_the_path_keeps_the_identity() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "01 - Programação/02. Go Essencial/aula.mp4")?;
    media(&db, "m2", "03 - Programação/04. Go Essencial/outra.mp4")?;

    let catalog = build_catalog(&db, LIB)?;

    assert_eq!(catalog.categories.len(), 2);
    assert_eq!(catalog.categories[0].title, "Programação");
    assert_eq!(catalog.categories[1].title, "Programação");
    assert_eq!(catalog.categories[0].path, "01 - Programação");
    assert_eq!(catalog.categories[1].path, "03 - Programação");
    // Títulos iguais em caminhos diferentes continuam sendo nós distintos.
    assert_ne!(
        catalog.categories[0].children[0].path,
        catalog.categories[1].children[0].path
    );
    Ok(())
}

#[test]
fn missing_files_stay_in_the_catalog_with_their_identity() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "Cat/Aula/aula.mp4", 1024, "missing")?;

    let catalog = build_catalog(&db, LIB)?;
    let category = &catalog.categories[0];
    let lesson = node(&category.children, "Cat/Aula");
    let file = &lesson.files[0];

    assert_eq!(file.media_id, "m1");
    assert_eq!(file.availability, Availability::Missing);
    assert!(!file.unverified);
    Ok(())
}

#[test]
fn files_under_an_unverified_area_are_flagged() -> TestResult {
    let db = library_db()?;
    let scan_id = insert_scan(&db, 0)?;
    insert_unverified(&db, scan_id, "Cat")?;
    media(&db, "m1", "Cat/Curso/Aula/aula.mp4")?;
    media(&db, "m2", "Outra/Aula/outra.mp4")?;

    let catalog = build_catalog(&db, LIB)?;

    assert!(!catalog.complete, "a última varredura foi parcial");
    let cat = node(&catalog.categories, "Cat");
    let lesson = node(&node(&cat.children, "Cat/Curso").children, "Cat/Curso/Aula");
    assert!(
        lesson.files[0].unverified,
        "arquivo sob área não verificada"
    );

    let outra = node(&catalog.categories, "Outra");
    let outra_lesson = node(&outra.children, "Outra/Aula");
    assert!(
        !outra_lesson.files[0].unverified,
        "área verificada não é marcada"
    );
    Ok(())
}

#[test]
fn an_absent_file_is_not_flagged_as_unverified() -> TestResult {
    let db = library_db()?;
    // Uma varredura completa não deixa área não verificada: o arquivo falta porque a
    // biblioteca foi lida, não porque a área ficou fora do alcance.
    insert_scan(&db, 1)?;
    insert_media(&db, "m1", "Cat/Aula/aula.mp4", 1024, "missing")?;

    let catalog = build_catalog(&db, LIB)?;
    let category = &catalog.categories[0];
    let lesson = node(&category.children, "Cat/Aula");

    assert!(catalog.complete);
    assert_eq!(lesson.files[0].availability, Availability::Missing);
    assert!(!lesson.files[0].unverified);
    Ok(())
}

#[test]
fn the_kind_is_derived_from_the_extension_including_the_typescript_rule() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "Cat/Aula/aula.mp4", 1024, "available")?;
    insert_media(&db, "m2", "Cat/Aula/legenda.srt", 1024, "available")?;
    insert_media(&db, "m3", "Cat/Aula/apoio.pdf", 1024, "available")?;
    insert_media(&db, "m4", "Cat/Aula/pequeno.ts", 1024, "available")?;
    insert_media(
        &db,
        "m5",
        "Cat/Aula/grande.ts",
        2 * 1024 * 1024,
        "available",
    )?;

    let catalog = build_catalog(&db, LIB)?;
    let lesson = node(&catalog.categories[0].children, "Cat/Aula");
    let kind_of = |id: &str| {
        lesson
            .files
            .iter()
            .find(|file| file.media_id == id)
            .expect("arquivo presente")
            .kind
    };

    assert_eq!(kind_of("m1"), MediaKind::Media);
    assert_eq!(kind_of("m2"), MediaKind::Subtitle);
    assert_eq!(kind_of("m3"), MediaKind::Material);
    assert_eq!(kind_of("m4"), MediaKind::Material);
    assert_eq!(kind_of("m5"), MediaKind::Media);
    Ok(())
}

#[test]
fn the_catalog_is_built_only_from_persisted_data() -> TestResult {
    let db = library_db()?;
    // O diretório `Fantasma` e o arquivo abaixo não existem no sistema de arquivos.
    media(&db, "m1", "Fantasma/Inexistente/aula.mp4")?;

    let catalog = build_catalog(&db, LIB)?;

    let category = &catalog.categories[0];
    assert_eq!(category.path, "Fantasma");
    let lesson = node(&category.children, "Fantasma/Inexistente");
    assert_eq!(lesson.files[0].media_id, "m1");
    Ok(())
}

#[test]
fn libraries_are_isolated() -> TestResult {
    let db = library_db()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('bib-2', 1)",
        [],
    )?;
    media(&db, "m1", "Cat/Aula/aula.mp4")?;
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES ('m2', 'bib-2', 'Outra/Aula/outra.mp4', ?1, 1024, 1760000000, 'available')",
        [key("Outra/Aula/outra.mp4")],
    )?;

    let catalog = build_catalog(&db, LIB)?;

    assert_eq!(catalog.categories.len(), 1);
    assert_eq!(catalog.categories[0].path, "Cat");
    Ok(())
}

#[test]
fn siblings_are_ordered_by_path() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "Zebra/Aula/z.mp4")?;
    media(&db, "m2", "Alfa/Aula/a.mp4")?;
    media(&db, "m3", "Alfa/Aula/b.mp4")?;

    let catalog = build_catalog(&db, LIB)?;

    let paths: Vec<&str> = catalog
        .categories
        .iter()
        .map(|node| node.path.as_str())
        .collect();
    assert_eq!(paths, ["Alfa", "Zebra"]);

    let alfa = node(&catalog.categories, "Alfa");
    let lesson = node(&alfa.children, "Alfa/Aula");
    let names: Vec<&str> = lesson.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["a.mp4", "b.mp4"]);
    Ok(())
}

#[test]
fn an_empty_library_produces_an_empty_catalog() -> TestResult {
    let db = library_db()?;

    let catalog = build_catalog(&db, LIB)?;

    assert!(catalog.categories.is_empty());
    assert!(catalog.loose_files.is_empty());
    assert!(
        catalog.complete,
        "sem varredura registrada nada está pendente"
    );
    Ok(())
}

#[test]
fn building_the_catalog_does_not_write() -> TestResult {
    let db = library_db()?;
    let scan_id = insert_scan(&db, 0)?;
    insert_unverified(&db, scan_id, "Cat")?;
    media(&db, "m1", "Cat/Aula/aula.mp4")?;
    insert_media(&db, "m2", "Livre/solto.mp4", 1024, "missing")?;

    let media_before: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM media", [], |row| row.get(0))?;
    let scans_before: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scan", [], |row| row.get(0))?;

    let _ = build_catalog(&db, LIB)?;

    let media_after: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM media", [], |row| row.get(0))?;
    let scans_after: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM scan", [], |row| row.get(0))?;

    assert_eq!(media_before, media_after, "o catálogo não altera mídia");
    assert_eq!(
        scans_before, scans_after,
        "o catálogo não registra varredura"
    );
    Ok(())
}
