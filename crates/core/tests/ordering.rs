//! Manual ordering of modules and lessons over the catalog's automatic order.
//!
//! The cases simulate a course: paths are relative to the course root, so the top-level
//! list of the catalog is exactly the course's list of modules — the same slice the
//! adapter passes to [`apply_order`]. Manual ordering is applied over the derived
//! catalog, never over the disk.

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

/// Catalog of a course with three modules in natural order: 2, 3 and 10.
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

    // `Lesson 2` precedes `Lesson 10`; the tie of leading zeros is resolved
    // deterministically by the original name ("Lesson 02" before "Lesson 2"), with no locale.
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

    // Manual order never changes the content nor the identity of the files.
    let lesson = node(&catalog.categories[0].children, "Modulo 10/Aula 1");
    assert_eq!(lesson.files[0].media_id, "m10");
    assert_eq!(lesson.files[0].path, "Modulo 10/Aula 1/aula.mp4");

    let _ = db;
    Ok(())
}

#[test]
fn unlisted_children_keep_the_automatic_order() -> TestResult {
    let (_db, mut catalog) = three_module_catalog()?;

    // Only one child is cited: it goes to the top and the others keep the automatic
    // order, without vanishing or duplicating.
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

    // Removing the preference restores the automatic order: the catalog is rederived on
    // every read and `apply_order` of an empty preference preserves the natural order.
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

    // Restoring the automatic order means removing the preference; the next
    // derivation of the catalog already comes natural and no overlay is applied.
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

    // `""` is the top-level list passed to `apply_order`.
    assert_eq!(
        sibling_paths(&catalog.categories, ""),
        Some(vec![
            "Modulo 2".to_owned(),
            "Modulo 3".to_owned(),
            "Modulo 10".to_owned(),
        ])
    );
    // An existing parent returns its children; an unknown parent does not exist.
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
    // Omitting a sibling is not a valid permutation.
    assert!(!validate_siblings(&existing, &owned(&["Modulo 10"])));
    // A path from another parent is refused.
    assert!(!validate_siblings(
        &existing,
        &owned(&["Modulo 10", "Modulo 2", "Modulo Outro"])
    ));
    // Repetition is refused.
    assert!(!validate_siblings(
        &existing,
        &owned(&["Modulo 2", "Modulo 2", "Modulo 3"])
    ));
    // An empty list for a parent with no children is accepted (nothing to reorder).
    assert!(validate_siblings(&[], &[]));
    Ok(())
}
