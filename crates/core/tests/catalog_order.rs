//! Persistência de preferências de ordem manual do catálogo.

use learnvault_core::{
    CatalogNode, Database, NodeRole, apply_catalog_order, apply_saved_catalog_order, catalog_order,
    persist_catalog_order, reset_catalog_order,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn database() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('lib-a', 1), ('lib-b', 1)",
        [],
    )?;
    Ok(db)
}

#[test]
fn manual_order_is_scoped_by_owner_library_and_parent() -> TestResult {
    let db = database()?;
    let expected = ["Curso/Módulo 2", "Curso/Módulo 1"];

    persist_catalog_order(&db, "owner-a", "lib-a", "Curso", &expected)?;

    assert_eq!(catalog_order(&db, "owner-a", "lib-a", "Curso")?, expected);
    assert!(catalog_order(&db, "owner-b", "lib-a", "Curso")?.is_empty());
    assert!(catalog_order(&db, "owner-a", "lib-b", "Curso")?.is_empty());
    assert!(catalog_order(&db, "owner-a", "lib-a", "Outro")?.is_empty());
    Ok(())
}

#[test]
fn invalid_replacement_does_not_destroy_existing_order() -> TestResult {
    let db = database()?;
    let original = ["Curso/Módulo 2", "Curso/Módulo 1"];
    persist_catalog_order(&db, "owner-a", "lib-a", "Curso", &original)?;

    assert!(
        persist_catalog_order(
            &db,
            "owner-a",
            "lib-a",
            "Curso",
            &["Curso/Módulo 1", "Curso/Módulo 1"]
        )
        .is_err()
    );

    assert_eq!(catalog_order(&db, "owner-a", "lib-a", "Curso")?, original);
    Ok(())
}

#[test]
fn replacement_keeps_previously_saved_paths_that_are_not_currently_present() -> TestResult {
    let db = database()?;
    persist_catalog_order(
        &db,
        "owner-a",
        "lib-a",
        "Curso",
        &["Curso/Módulo 1", "Curso/Módulo 2"],
    )?;

    persist_catalog_order(
        &db,
        "owner-a",
        "lib-a",
        "Curso",
        &["Curso/Módulo 2", "Curso/Módulo 3"],
    )?;

    assert_eq!(
        catalog_order(&db, "owner-a", "lib-a", "Curso")?,
        ["Curso/Módulo 2", "Curso/Módulo 3", "Curso/Módulo 1"]
    );
    Ok(())
}

#[test]
fn reset_removes_only_the_selected_sibling_order() -> TestResult {
    let db = database()?;
    persist_catalog_order(
        &db,
        "owner-a",
        "lib-a",
        "Curso",
        &["Curso/Módulo 2", "Curso/Módulo 1"],
    )?;
    persist_catalog_order(
        &db,
        "owner-a",
        "lib-a",
        "Curso/Módulo 1",
        &["Curso/Módulo 1/Aula 2", "Curso/Módulo 1/Aula 1"],
    )?;

    reset_catalog_order(&db, "owner-a", "lib-a", "Curso")?;

    assert!(catalog_order(&db, "owner-a", "lib-a", "Curso")?.is_empty());
    assert_eq!(
        catalog_order(&db, "owner-a", "lib-a", "Curso/Módulo 1")?,
        ["Curso/Módulo 1/Aula 2", "Curso/Módulo 1/Aula 1"]
    );
    Ok(())
}

#[test]
fn applying_saved_order_keeps_new_children_in_natural_order_at_the_end() {
    let mut children = vec!["Aula 1", "Aula 2", "Aula 10"];
    let saved = vec!["Aula 10".to_owned(), "Aula 1".to_owned()];

    apply_catalog_order(&mut children, &saved, |path| path);

    assert_eq!(children, ["Aula 10", "Aula 1", "Aula 2"]);
}

#[test]
fn saved_order_is_applied_recursively_to_modules_and_lessons() -> TestResult {
    let db = database()?;
    persist_catalog_order(
        &db,
        "owner-a",
        "lib-a",
        "Curso",
        &["Curso/Módulo 2", "Curso/Módulo 1"],
    )?;
    persist_catalog_order(
        &db,
        "owner-a",
        "lib-a",
        "Curso/Módulo 1",
        &["Curso/Módulo 1/Aula 2", "Curso/Módulo 1/Aula 1"],
    )?;

    let mut modules = vec![
        node(
            "Curso/Módulo 1",
            NodeRole::Module,
            vec![
                node("Curso/Módulo 1/Aula 1", NodeRole::Lesson, vec![]),
                node("Curso/Módulo 1/Aula 2", NodeRole::Lesson, vec![]),
            ],
        ),
        node("Curso/Módulo 2", NodeRole::Module, vec![]),
    ];

    apply_saved_catalog_order(&db, "owner-a", "lib-a", &mut modules, "Curso")?;

    assert_eq!(modules[0].path, "Curso/Módulo 2");
    assert_eq!(modules[1].path, "Curso/Módulo 1");
    assert_eq!(modules[1].children[0].path, "Curso/Módulo 1/Aula 2");
    assert_eq!(modules[1].children[1].path, "Curso/Módulo 1/Aula 1");
    Ok(())
}

fn node(path: &str, role: NodeRole, children: Vec<CatalogNode>) -> CatalogNode {
    CatalogNode {
        title: path.rsplit('/').next().unwrap_or_default().to_owned(),
        path: path.to_owned(),
        role,
        files: Vec::new(),
        loose_files: Vec::new(),
        children,
    }
}
