//! Testes da derivação do catálogo a partir dos dados persistidos.
//!
//! Todos os casos partem de um banco em memória com linhas de `media` inseridas
//! diretamente: o catálogo é uma função dos dados, não do sistema de arquivos.

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Availability, CatalogFile, CatalogNode, Database, Error, KeyMode, MediaKind, NodeRole,
    build_catalog, course_file_lessons,
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
    assert_eq!(file.mtime_seconds, 1_760_000_000);
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
fn purely_numeric_titles_preserve_number_as_title() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "2024/01/aula.mp4")?;

    let catalog = build_catalog(&db, LIB)?;
    assert_eq!(catalog.categories[0].title, "2024");
    assert_eq!(catalog.categories[0].children[0].title, "01");
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

#[test]
fn a_node_role_does_not_depend_on_the_order_of_the_files() -> TestResult {
    // A chave de `a-bonus/...` ordena antes da chave de `z.mp4`. O papel do diretório
    // que contém mídia não pode depender de qual arquivo foi lido primeiro.
    let db = library_db()?;
    media(&db, "m1", "Curso/Modulo1/a-bonus/x.mp4")?;
    media(&db, "m2", "Curso/Modulo1/z.mp4")?;

    let catalog = build_catalog(&db, LIB)?;
    let lesson = node(&catalog.categories[0].children, "Curso/Modulo1");

    assert_eq!(
        lesson.role,
        NodeRole::Lesson,
        "um diretório com mídia é uma aula, independentemente da ordem de leitura"
    );
    assert_eq!(lesson.files.len(), 1);
    assert_eq!(lesson.files[0].media_id, "m2");

    let nested = node(&lesson.children, "Curso/Modulo1/a-bonus");
    assert_eq!(nested.role, NodeRole::Lesson);
    assert_eq!(nested.files[0].media_id, "m1");
    Ok(())
}

#[test]
fn an_unverified_area_does_not_cover_a_shared_prefix_sibling() -> TestResult {
    let db = library_db()?;
    let scan_id = insert_scan(&db, 0)?;
    insert_unverified(&db, scan_id, "Cat")?;
    media(&db, "m1", "Catalog/Aula/aula.mp4")?;
    media(&db, "m2", "Cat/Aula/aula.mp4")?;

    let catalog = build_catalog(&db, LIB)?;

    let fora = node(&catalog.categories, "Catalog");
    let fora_lesson = node(&fora.children, "Catalog/Aula");
    assert!(
        !fora_lesson.files[0].unverified,
        "Cat não pode cobrir Catalog"
    );

    let dentro = node(&catalog.categories, "Cat");
    let dentro_lesson = node(&dentro.children, "Cat/Aula");
    assert!(dentro_lesson.files[0].unverified);
    Ok(())
}

#[test]
fn a_shared_prefix_directory_is_not_adopted_as_an_ancestor() -> TestResult {
    let db = library_db()?;
    media(&db, "m1", "A/aula.mp4")?;
    media(&db, "m2", "AB/C/aula.mp4")?;

    let catalog = build_catalog(&db, LIB)?;
    assert_eq!(catalog.categories.len(), 2);

    let a = node(&catalog.categories, "A");
    assert_eq!(a.loose_files.len(), 1);
    assert!(a.children.is_empty(), "A não contém a árvore de AB");

    let ab = node(&catalog.categories, "AB");
    assert_eq!(node(&ab.children, "AB/C").files[0].media_id, "m2");
    Ok(())
}

#[test]
fn a_failed_scan_marks_every_file_as_unverified() -> TestResult {
    let db = library_db()?;
    let scan_id = insert_scan(&db, 0)?;
    db.connection().execute(
        "UPDATE scan SET status = 'failed' WHERE scan_id = ?1",
        [scan_id],
    )?;
    media(&db, "m1", "Cat/Aula/aula.mp4")?;

    let catalog = build_catalog(&db, LIB)?;

    assert!(
        !catalog.complete,
        "a última tentativa não conseguiu ler a raiz"
    );
    let lesson = node(&node(&catalog.categories, "Cat").children, "Cat/Aula");
    assert!(
        lesson.files[0].unverified,
        "nada foi verificado na tentativa que falhou"
    );
    Ok(())
}

#[test]
fn files_are_ordered_by_path_regardless_of_insertion_order() -> TestResult {
    let db = library_db()?;
    media(&db, "m2", "Cat/Aula/b.mp4")?;
    media(&db, "m1", "Cat/Aula/a.mp4")?;

    let catalog = build_catalog(&db, LIB)?;
    let lesson = node(&node(&catalog.categories, "Cat").children, "Cat/Aula");

    let names: Vec<&str> = lesson.files.iter().map(|file| file.name.as_str()).collect();
    assert_eq!(names, ["a.mp4", "b.mp4"]);
    Ok(())
}

#[test]
fn a_negative_size_fails_the_catalog_as_corrupt() -> TestResult {
    let db = library_db()?;
    insert_media(&db, "m1", "Cat/Aula/aula.mp4", -1, "available")?;

    assert!(
        matches!(build_catalog(&db, LIB), Err(Error::MediaCorruptSize { .. })),
        "tamanho negativo indica corrupção do catálogo, não arquivo vazio"
    );
    Ok(())
}

/// A ordenação deve ser natural em todos os níveis: "2" antes de "10", tanto em
/// módulos quanto em aulas e materiais. A ordem é função do caminho registrado,
/// nunca da ordem retornada pelo banco.
#[test]
fn catalog_ordering_is_natural_for_numbered_names() -> TestResult {
    let db = library_db()?;
    for (media_id, path) in [
        ("m1", "Curso/Modulo 10/Aula 2/aula.mp4"),
        ("m2", "Curso/Modulo 10/Aula 10/aula.mp4"),
        ("m3", "Curso/Modulo 2/Aula 1/aula.mp4"),
        ("m4", "Curso/Modulo 2/Aula 1/material.pdf"),
        ("m5", "Material 2.pdf"),
        ("m6", "Material 10.pdf"),
    ] {
        media(&db, media_id, path)?;
    }

    let catalog = build_catalog(&db, LIB)?;
    let category = &catalog.categories[0];

    let module_paths: Vec<&str> = category
        .children
        .iter()
        .map(|child| child.path.as_str())
        .collect();
    assert_eq!(
        module_paths,
        ["Curso/Modulo 2", "Curso/Modulo 10"],
        "módulos em ordem natural"
    );

    let module_ten = node(&category.children, "Curso/Modulo 10");
    let lesson_paths: Vec<&str> = module_ten
        .children
        .iter()
        .map(|child| child.path.as_str())
        .collect();
    assert_eq!(
        lesson_paths,
        ["Curso/Modulo 10/Aula 2", "Curso/Modulo 10/Aula 10"],
        "aulas em ordem natural"
    );

    let lesson = node(&module_ten.children, "Curso/Modulo 10/Aula 2");
    assert!(lesson.files.iter().any(|file| file.name == "aula.mp4"));

    let loose_names: Vec<&str> = catalog
        .loose_files
        .iter()
        .map(|file| file.name.as_str())
        .collect();
    assert_eq!(loose_names, ["Material 2.pdf", "Material 10.pdf"]);
    Ok(())
}

/// Sem numeração, a ordem volta a ser alfabética: previsível e determinística.
#[test]
fn ordering_falls_back_to_alphabetical_without_numbers() -> TestResult {
    let db = library_db()?;
    for (media_id, name) in [("m1", "Bravo"), ("m2", "alfa"), ("m3", "Zulu")] {
        media(&db, media_id, &format!("Curso/{name}/aula.mp4"))?;
    }
    let catalog = build_catalog(&db, LIB)?;
    let course = node(&catalog.categories, "Curso");
    let modules: Vec<String> = course
        .children
        .iter()
        .map(|child| child.title.clone())
        .collect();
    assert_eq!(modules, vec!["Bravo", "Zulu", "alfa"]);
    Ok(())
}

/// Reindexar não muda a sequência: a ordem é função estável dos caminhos e a
/// identidade (media_id) sobrevive à nova leitura.
#[test]
fn reindexing_keeps_the_same_natural_order_and_identity() -> TestResult {
    let db = library_db()?;
    for (media_id, path) in [
        ("m1", "Curso/Modulo 1/Aula 2/aula.mp4"),
        ("m2", "Curso/Modulo 1/Aula 10/aula.mp4"),
    ] {
        media(&db, media_id, path)?;
    }
    let first = build_catalog(&db, LIB)?;
    let second = build_catalog(&db, LIB)?;
    assert_eq!(first, second, "catálogo determinístico entre leituras");
    let lesson = node(
        &node(&node(&first.categories, "Curso").children, "Curso/Modulo 1").children,
        "Curso/Modulo 1/Aula 2",
    );
    assert_eq!(lesson.files[0].media_id, "m1");
    Ok(())
}

fn catalog_file(media_id: &str, path: &str, kind: MediaKind) -> CatalogFile {
    CatalogFile {
        media_id: media_id.to_owned(),
        name: path.rsplit('/').next().unwrap_or(path).to_owned(),
        path: path.to_owned(),
        size: 1024,
        mtime_seconds: 1_760_000_000,
        kind,
        availability: Availability::Available,
        unverified: false,
    }
}

fn grouping_node(
    path: &str,
    children: Vec<CatalogNode>,
    loose_files: Vec<CatalogFile>,
) -> CatalogNode {
    CatalogNode {
        title: path.rsplit('/').next().unwrap_or(path).to_owned(),
        path: path.to_owned(),
        role: NodeRole::Course,
        files: Vec::new(),
        loose_files,
        children,
    }
}

#[test]
fn course_grouping_keeps_material_and_attaches_matching_subtitles() {
    let module = grouping_node(
        "Curso/Modulo",
        Vec::new(),
        vec![
            catalog_file("video", "Curso/Modulo/Lección 1.mp4", MediaKind::Media),
            catalog_file(
                "subtitle",
                "Curso/Modulo/Lección 1.srt",
                MediaKind::Subtitle,
            ),
            catalog_file("material", "Curso/Modulo/Notas.pdf", MediaKind::Material),
            catalog_file(
                "orphan-subtitle",
                "Curso/Modulo/Otra.srt",
                MediaKind::Subtitle,
            ),
        ],
    );

    let grouped = course_file_lessons(module);

    assert_eq!(grouped.role, NodeRole::Module);
    assert_eq!(
        grouped
            .loose_files
            .iter()
            .map(|file| file.media_id.as_str())
            .collect::<Vec<_>>(),
        ["material", "orphan-subtitle"]
    );
    assert_eq!(grouped.children.len(), 1);
    let lesson = &grouped.children[0];
    assert_eq!(lesson.title, "Lección 1");
    assert_eq!(lesson.path, "Curso/Modulo/Lección 1.mp4");
    assert_eq!(
        lesson
            .files
            .iter()
            .map(|file| file.media_id.as_str())
            .collect::<Vec<_>>(),
        ["video", "subtitle"]
    );
}

#[test]
fn course_grouping_recurses_without_flattening_directory_lessons() {
    let directory_lesson = CatalogNode {
        title: "Lección 2".to_owned(),
        path: "Curso/Modulo/Lección 2".to_owned(),
        role: NodeRole::Lesson,
        files: vec![catalog_file(
            "directory-video",
            "Curso/Modulo/Lección 2/video.mp4",
            MediaKind::Media,
        )],
        loose_files: Vec::new(),
        children: Vec::new(),
    };
    let module = grouping_node(
        "Curso/Modulo",
        vec![directory_lesson],
        vec![
            catalog_file(
                "direct-video",
                "Curso/Modulo/Lección 1.mp4",
                MediaKind::Media,
            ),
            catalog_file("material", "Curso/Modulo/Handout.pdf", MediaKind::Material),
        ],
    );
    let course = grouping_node("Curso", vec![module], Vec::new());

    let grouped = course_file_lessons(course);
    let grouped_module = &grouped.children[0];

    assert_eq!(grouped_module.children.len(), 2);
    assert_eq!(
        grouped_module.children[0].path,
        "Curso/Modulo/Lección 1.mp4"
    );
    assert_eq!(grouped_module.children[0].files[0].media_id, "direct-video");
    assert_eq!(grouped_module.children[1].path, "Curso/Modulo/Lección 2");
    assert_eq!(
        grouped_module.children[1].files[0].media_id,
        "directory-video"
    );
    assert_eq!(grouped_module.loose_files[0].media_id, "material");
}

#[test]
fn course_grouping_preserves_nested_modules_and_file_identities() {
    let nested_module = grouping_node(
        "Curso/Modulo 2",
        Vec::new(),
        vec![catalog_file(
            "nested",
            "Curso/Modulo 2/Intro.mp4",
            MediaKind::Media,
        )],
    );
    let course = grouping_node(
        "Curso",
        vec![nested_module],
        vec![catalog_file(
            "course-media",
            "Curso/Intro.mp4",
            MediaKind::Media,
        )],
    );

    let grouped = course_file_lessons(course);

    assert_eq!(grouped.role, NodeRole::Module);
    assert_eq!(grouped.children.len(), 2);
    assert_eq!(grouped.children[0].files[0].media_id, "course-media");
    assert_eq!(grouped.children[1].path, "Curso/Modulo 2");
    assert_eq!(grouped.children[1].children[0].files[0].media_id, "nested");
}
