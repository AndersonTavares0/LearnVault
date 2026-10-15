//! Testes da fronteira IPC do desktop: DTOs, erros e catálogo.
//!
//! Não sobem o webview nem dependem de display: exercitam as funções livres e os tipos
//! serializáveis que os comandos `#[tauri::command]` apenas embrulham. O alvo é o
//! contrato observável pelo frontend — shape snake_case, códigos de erro estáveis e
//! mensagens sem caminhos — e não o runtime do Tauri.

use learnvault_core::{Availability, Catalog, CatalogFile, CatalogNode, MediaKind, NodeRole};
use learnvault_desktop::{CatalogDto, DesktopState, IpcErrorCode, catalog, config_status};

/// Arquivo de catálogo de teste, com localização fixa e verificável.
fn file(media_id: &str, kind: MediaKind, availability: Availability, unverified: bool) -> CatalogFile {
    CatalogFile {
        media_id: media_id.to_owned(),
        name: "aula.mp4".to_owned(),
        path: "cat/aula.mp4".to_owned(),
        size: 10,
        kind,
        availability,
        unverified,
    }
}

/// Nó de catálogo de teste com papel, arquivos e filhos explícitos.
fn node(title: &str, path: &str, role: NodeRole, files: Vec<CatalogFile>, children: Vec<CatalogNode>) -> CatalogNode {
    CatalogNode {
        title: title.to_owned(),
        path: path.to_owned(),
        role,
        files,
        loose_files: Vec::new(),
        children,
    }
}

#[test]
fn configuration_is_absent_until_a_source_exists() {
    let status = config_status();
    assert!(
        !status.configured,
        "sem fonte canônica de configuração, o desktop não pode se declarar configurado"
    );
    assert_eq!(
        serde_json::to_value(&status).unwrap(),
        serde_json::json!({ "configured": false })
    );
}

#[test]
fn empty_database_produces_a_complete_empty_catalog() {
    let database = learnvault_core::Database::open_in_memory().unwrap();
    let catalog = catalog(&database).unwrap();

    assert!(catalog.complete);
    assert!(catalog.loose_files.is_empty());
    assert!(catalog.categories.is_empty());
}

#[test]
fn catalog_dto_uses_the_snake_case_spa_contract() {
    let source = Catalog {
        complete: false,
        loose_files: vec![file("loose", MediaKind::Material, Availability::Missing, true)],
        categories: vec![node(
            "Curso",
            "cat",
            NodeRole::Category,
            Vec::new(),
            vec![node(
                "Aula",
                "cat/aula",
                NodeRole::Lesson,
                vec![file("m1", MediaKind::Media, Availability::Available, false)],
                Vec::new(),
            )],
        )],
    };

    let json = serde_json::to_value(CatalogDto::from(source)).unwrap();

    assert_eq!(json["complete"], serde_json::json!(false));
    assert_eq!(json["loose_files"][0]["media_id"], "loose");
    assert_eq!(json["loose_files"][0]["name"], "aula.mp4");
    assert_eq!(json["loose_files"][0]["path"], "cat/aula.mp4");
    assert_eq!(json["loose_files"][0]["size"], 10);
    assert_eq!(json["loose_files"][0]["kind"], "material");
    assert_eq!(json["loose_files"][0]["availability"], "missing");
    assert_eq!(json["loose_files"][0]["unverified"], true);
    assert_eq!(json["categories"][0]["title"], "Curso");
    assert_eq!(json["categories"][0]["path"], "cat");
    assert_eq!(json["categories"][0]["role"], "category");
    assert_eq!(json["categories"][0]["files"], serde_json::json!([]));
    assert_eq!(json["categories"][0]["loose_files"], serde_json::json!([]));
    assert_eq!(json["categories"][0]["children"][0]["role"], "lesson");
    assert_eq!(json["categories"][0]["children"][0]["files"][0]["kind"], "media");
    assert_eq!(json["categories"][0]["children"][0]["files"][0]["availability"], "available");
}

#[test]
fn every_core_enum_maps_to_a_stable_string() {
    let roles = [
        (NodeRole::Category, "category"),
        (NodeRole::Course, "course"),
        (NodeRole::Module, "module"),
        (NodeRole::Lesson, "lesson"),
    ];
    for (role, expected) in roles {
        let dto = CatalogDto::from(Catalog {
            complete: true,
            loose_files: Vec::new(),
            categories: vec![node("N", "n", role, Vec::new(), Vec::new())],
        });
        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["categories"][0]["role"], expected, "papel {role:?} precisa ser estável");
    }

    let kinds = [
        (MediaKind::Media, "media"),
        (MediaKind::Subtitle, "subtitle"),
        (MediaKind::Material, "material"),
    ];
    for (kind, expected) in kinds {
        let dto = CatalogDto::from(Catalog {
            complete: true,
            loose_files: vec![file("f", kind, Availability::Available, false)],
            categories: Vec::new(),
        });
        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["loose_files"][0]["kind"], expected, "tipo {kind:?} precisa ser estável");
    }
}

#[test]
fn catalog_failure_is_a_stable_sanitized_error() {
    let database = learnvault_core::Database::open_in_memory().unwrap();
    database
        .connection()
        .execute(
            "INSERT INTO library (library_id, available) VALUES ('library-1', 0)",
            [],
        )
        .unwrap();
    // Tamanho negativo é corrupção: o catálogo precisa falhar alto, sem vazar o caminho.
    database
        .connection()
        .execute(
            "INSERT INTO media
                 (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('m1', 'library-1', 'Cursos secretos/aula.mp4', 'cursos secretos/aula.mp4', -1, 0, 'available')",
            [],
        )
        .unwrap();

    let error = catalog(&database).unwrap_err();
    assert_eq!(error.code, IpcErrorCode::CatalogUnavailable);

    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "catalog_unavailable");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(!message.contains("Cursos secretos"), "a mensagem não pode expor caminhos: {message}");
    assert!(!message.contains("aula.mp4"), "a mensagem não pode expor nomes de arquivo: {message}");
}

#[test]
fn desktop_state_keeps_the_database_in_the_data_directory() {
    let directory = tempfile::tempdir().unwrap();
    let state = DesktopState::open(directory.path()).unwrap();

    let catalog = state.catalog().unwrap();
    assert!(catalog.complete);
    assert!(
        directory.path().join("learnvault.sqlite").exists(),
        "o banco local precisa viver no diretório de dados"
    );
}
