//! Ordenação manual de módulos e aulas sobre a ordem automática do catálogo.
//!
//! Os casos simulam um curso: os caminhos são relativos à raiz do curso, então a lista
//! de topo do catálogo é exatamente a lista de módulos do curso — a mesma fatia que o
//! adaptador passa a [`apply_order`]. A ordenação manual é aplicada sobre o catálogo
//! derivado, nunca sobre o disco.

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Catalog, CatalogNode, CatalogOrder, Database, KeyMode, apply_order, build_catalog,
    sibling_paths, validate_siblings,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "bib-ordering";

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

fn media(db: &Database, media_id: &str, path: &str) -> rusqlite::Result<usize> {
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, ?2, ?3, ?4, 1024, 1760000000, 'available')",
        rusqlite::params![media_id, LIB, path, key(path)],
    )
}

fn node<'a>(nodes: &'a [CatalogNode], path: &str) -> &'a CatalogNode {
    nodes
        .iter()
        .find(|candidate| candidate.path == path)
        .unwrap_or_else(|| panic!("nó {path} não encontrado"))
}

fn child_paths(nodes: &[CatalogNode]) -> Vec<String> {
    nodes.iter().map(|node| node.path.clone()).collect()
}

/// Catálogo de um curso com três módulos em ordem natural: 2, 3 e 10.
fn three_module_catalog() -> Result<(Database, Catalog), Box<dyn std::error::Error>> {
    let db = library_db()?;
    for (id, path) in [
        ("m2", "Modulo 2/Aula 1/aula.mp4"),
        ("m3", "Modulo 3/Aula 1/aula.mp4"),
        ("m10", "Modulo 10/Aula 1/aula.mp4"),
    ] {
        media(&db, id, path)?;
    }
    let catalog = build_catalog(&db, LIB)?;
    Ok((db, catalog))
}

#[test]
fn automatic_order_is_natural_and_ties_break_by_name() -> TestResult {
    let db = library_db()?;
    for (id, path) in [
        ("m10", "Modulo 1/Aula 10/aula.mp4"),
        ("m2", "Modulo 1/Aula 2/aula.mp4"),
        ("m02", "Modulo 1/Aula 02/aula.mp4"),
    ] {
        media(&db, id, path)?;
    }

    let catalog = build_catalog(&db, LIB)?;
    let module = &catalog.categories[0];
    assert_eq!(module.path, "Modulo 1");

    // `Aula 2` precede `Aula 10`; o empate de zeros à esquerda é resolvido de forma
    // determinística pelo nome original ("Aula 02" antes de "Aula 2"), sem locale.
    assert_eq!(
        child_paths(&module.children),
        ["Modulo 1/Aula 02", "Modulo 1/Aula 2", "Modulo 1/Aula 10",]
    );
    Ok(())
}

#[test]
fn manual_order_moves_only_the_listed_siblings() -> TestResult {
    let (db, mut catalog) = three_module_catalog()?;
    assert_eq!(
        child_paths(&catalog.categories),
        ["Modulo 2", "Modulo 3", "Modulo 10"]
    );

    let mut order = CatalogOrder::new();
    order.set_children(
        "",
        vec![
            "Modulo 10".to_owned(),
            "Modulo 2".to_owned(),
            "Modulo 3".to_owned(),
        ],
    );
    apply_order(&mut catalog.categories, &order);

    assert_eq!(
        child_paths(&catalog.categories),
        ["Modulo 10", "Modulo 2", "Modulo 3"]
    );

    // A ordem manual nunca altera o conteúdo nem a identidade dos arquivos.
    let lesson = node(&catalog.categories[0].children, "Modulo 10/Aula 1");
    assert_eq!(lesson.files[0].media_id, "m10");
    assert_eq!(lesson.files[0].path, "Modulo 10/Aula 1/aula.mp4");

    let _ = db;
    Ok(())
}

#[test]
fn unlisted_children_keep_the_automatic_order() -> TestResult {
    let (_db, mut catalog) = three_module_catalog()?;

    // Apenas um filho é citado: ele vai para o topo e os demais permanecem na ordem
    // automática, sem sumir nem duplicar.
    let mut order = CatalogOrder::new();
    order.set_children("", vec!["Modulo 10".to_owned()]);
    apply_order(&mut catalog.categories, &order);

    assert_eq!(
        child_paths(&catalog.categories),
        ["Modulo 10", "Modulo 2", "Modulo 3"]
    );
    Ok(())
}

#[test]
fn order_applies_to_nested_lessons() -> TestResult {
    let db = library_db()?;
    for (id, path) in [
        ("a", "Modulo 1/Aula 2/aula.mp4"),
        ("b", "Modulo 1/Aula 10/aula.mp4"),
    ] {
        media(&db, id, path)?;
    }
    let mut catalog = build_catalog(&db, LIB)?;

    let mut order = CatalogOrder::new();
    order.set_children(
        "Modulo 1",
        vec!["Modulo 1/Aula 10".to_owned(), "Modulo 1/Aula 2".to_owned()],
    );
    apply_order(&mut catalog.categories, &order);

    let module = &catalog.categories[0];
    assert_eq!(
        child_paths(&module.children),
        ["Modulo 1/Aula 10", "Modulo 1/Aula 2"]
    );
    Ok(())
}

#[test]
fn declaring_a_parent_again_replaces_the_previous_preference() -> TestResult {
    let (_db, mut catalog) = three_module_catalog()?;

    let mut order = CatalogOrder::new();
    order.set_children("", vec!["Modulo 10".to_owned()]);
    order.set_children("", vec!["Modulo 3".to_owned()]);
    apply_order(&mut catalog.categories, &order);

    assert_eq!(
        child_paths(&catalog.categories),
        ["Modulo 3", "Modulo 2", "Modulo 10"]
    );

    // Remover a preferência restaura a ordem automática: o catálogo é rederivado a
    // cada leitura e `apply_order` de uma preferência vazia preserva a ordem natural.
    assert!(order.remove(""));
    let mut fresh = build_catalog(&_db, LIB)?;
    apply_order(&mut fresh.categories, &order);
    assert_eq!(
        child_paths(&fresh.categories),
        ["Modulo 2", "Modulo 3", "Modulo 10"]
    );
    Ok(())
}

#[test]
fn reset_restores_the_automatic_order() -> TestResult {
    let (db, mut catalog) = three_module_catalog()?;

    let mut order = CatalogOrder::new();
    order.set_children(
        "",
        vec![
            "Modulo 10".to_owned(),
            "Modulo 2".to_owned(),
            "Modulo 3".to_owned(),
        ],
    );
    apply_order(&mut catalog.categories, &order);
    assert_eq!(
        child_paths(&catalog.categories),
        ["Modulo 10", "Modulo 2", "Modulo 3"]
    );

    // Restaurar a ordem automática significa remover a preferência; a próxima
    // derivação do catálogo já vem natural e nenhuma sobreposição é aplicada.
    order.clear();
    assert!(order.is_empty());
    let mut fresh = build_catalog(&db, LIB)?;
    apply_order(&mut fresh.categories, &order);
    assert_eq!(
        child_paths(&fresh.categories),
        ["Modulo 2", "Modulo 3", "Modulo 10"]
    );
    Ok(())
}

#[test]
fn sibling_paths_returns_children_of_a_parent_and_the_top_level() -> TestResult {
    let (_db, catalog) = three_module_catalog()?;

    // `""` é a lista de topo passada a `apply_order`.
    assert_eq!(
        sibling_paths(&catalog.categories, ""),
        Some(vec![
            "Modulo 2".to_owned(),
            "Modulo 3".to_owned(),
            "Modulo 10".to_owned(),
        ])
    );
    // Um pai existente devolve os filhos; um pai desconhecido não existe.
    assert_eq!(
        sibling_paths(&catalog.categories, "Modulo 2"),
        Some(vec!["Modulo 2/Aula 1".to_owned()])
    );
    assert_eq!(sibling_paths(&catalog.categories, "Modulo X"), None);
    Ok(())
}

#[test]
fn validate_siblings_accepts_permutations_and_rejects_foreign_or_partial_lists() -> TestResult {
    let (_db, catalog) = three_module_catalog()?;
    let existing = child_paths(&catalog.categories);
    let owned = |paths: &[&str]| {
        paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>()
    };

    assert!(validate_siblings(
        &existing,
        &owned(&["Modulo 10", "Modulo 2", "Modulo 3"])
    ));
    // Omitir um irmão não é uma permutação válida.
    assert!(!validate_siblings(&existing, &owned(&["Modulo 10"])));
    // Caminho de outro pai é recusado.
    assert!(!validate_siblings(
        &existing,
        &owned(&["Modulo 10", "Modulo 2", "Modulo Outro"])
    ));
    // Repetição é recusada.
    assert!(!validate_siblings(
        &existing,
        &owned(&["Modulo 2", "Modulo 2", "Modulo 3"])
    ));
    // Lista vazia para um pai sem filhos é aceita (nada a reordenar).
    assert!(validate_siblings(&[], &[]));
    Ok(())
}
