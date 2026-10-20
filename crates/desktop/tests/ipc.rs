//! Testes da fronteira IPC do desktop: DTOs, erros e catálogo.
//!
//! Não sobem o webview nem dependem de display: exercitam as funções livres e os tipos
//! serializáveis que os comandos `#[tauri::command]` apenas embrulham. O alvo é o
//! contrato observável pelo frontend — shape snake_case, códigos de erro estáveis e
//! mensagens sem caminhos — e não o runtime do Tauri.

use std::fs;
use std::path::{Path, PathBuf};

use learnvault_core::{
    Availability, Catalog, CatalogFile, CatalogNode, CourseRoot, MediaKind, NodeRole,
};
use learnvault_desktop::{
    CatalogDto, CatalogNodeDto, CourseEntry, DesktopState, IpcErrorCode, RescanSummaryDto, catalog,
    load_courses,
};

/// Estado desktop isolado em diretórios temporários, com dados e configuração separados.
fn desktop(root: &Path) -> DesktopState {
    DesktopState::open(&root.join("data"), &root.join("config")).unwrap()
}

/// Cria um diretório de curso válido e devolve o caminho escolhido.
fn make_course(root: &Path) -> PathBuf {
    let course = root.join("curso");
    fs::create_dir_all(&course).unwrap();
    course
}

/// Forma canônica de um caminho para comparação.
fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap()
}

/// Curso registrado de teste, com identidade explícita.
fn entry(id: &str, path: &Path) -> CourseEntry {
    CourseEntry {
        id: id.to_owned(),
        root: CourseRoot::resolve(path).unwrap(),
    }
}

/// Arquivo de catálogo de teste, com localização fixa e verificável.
fn file(
    media_id: &str,
    kind: MediaKind,
    availability: Availability,
    unverified: bool,
) -> CatalogFile {
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
fn node(
    title: &str,
    path: &str,
    role: NodeRole,
    files: Vec<CatalogFile>,
    children: Vec<CatalogNode>,
) -> CatalogNode {
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
fn configuration_starts_unconfigured_without_exposing_a_path() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());

    assert!(
        !state.config_status().configured,
        "sem cursos persistidos, o desktop não pode se declarar configurado"
    );
    assert_eq!(
        serde_json::to_value(state.config_status()).unwrap(),
        serde_json::json!({ "configured": false })
    );
}

#[test]
fn empty_database_produces_a_complete_empty_catalog() {
    let database = learnvault_core::Database::open_in_memory().unwrap();
    let catalog = catalog(&database, &[]).unwrap();

    assert!(catalog.complete);
    assert!(catalog.loose_files.is_empty());
    assert!(catalog.nodes.is_empty());
    assert!(catalog.categories.is_empty());
}

#[test]
fn catalog_dto_uses_the_snake_case_spa_contract() {
    let source = Catalog {
        complete: false,
        loose_files: vec![file(
            "loose",
            MediaKind::Material,
            Availability::Missing,
            true,
        )],
        categories: vec![node(
            "Aula",
            "aula",
            NodeRole::Lesson,
            vec![file("m1", MediaKind::Media, Availability::Available, false)],
            Vec::new(),
        )],
    };
    let complete = source.complete;
    let nodes = vec![CatalogNodeDto::course("course-42", "Curso", source)];
    let dto = CatalogDto {
        complete,
        loose_files: Vec::new(),
        nodes: nodes.clone(),
        categories: nodes,
    };

    let json = serde_json::to_value(dto).unwrap();

    assert_eq!(json["complete"], serde_json::json!(false));
    assert_eq!(json["categories"][0]["title"], "Curso");
    assert_eq!(json["categories"][0]["path"], "");
    assert_eq!(json["categories"][0]["role"], "course");
    assert_eq!(json["categories"][0]["course_id"], "course-42");
    assert_eq!(json["categories"][0]["files"], serde_json::json!([]));
    assert_eq!(json["categories"][0]["loose_files"][0]["media_id"], "loose");
    assert_eq!(json["categories"][0]["loose_files"][0]["name"], "aula.mp4");
    assert_eq!(
        json["categories"][0]["loose_files"][0]["path"],
        "cat/aula.mp4"
    );
    assert_eq!(json["categories"][0]["loose_files"][0]["size"], 10);
    assert_eq!(json["categories"][0]["loose_files"][0]["kind"], "material");
    assert_eq!(
        json["categories"][0]["loose_files"][0]["availability"],
        "missing"
    );
    assert_eq!(json["categories"][0]["loose_files"][0]["unverified"], true);
    assert_eq!(json["categories"][0]["children"][0]["role"], "lesson");
    assert_eq!(
        json["categories"][0]["children"][0]["files"][0]["kind"],
        "media"
    );
    // O desktop agrega cursos: a lista solta de topo fica vazia e os arquivos soltos vivem
    // no nó do curso.
    assert_eq!(json["loose_files"], serde_json::json!([]));

    // `nodes` é a árvore recursiva canônica, o contrato preferido pela SPA, e carrega o
    // mesmo nó de topo de curso: identidade opaca, identidade de mídia, caminhos relativos,
    // disponibilidade e verificação dos arquivos seguem preservados, e a aula continua
    // alcançável sob o curso sem depender de nenhuma projeção legada.
    assert_eq!(json["nodes"], json["categories"]);
    let node = &json["nodes"][0];
    assert_eq!(node["title"], "Curso");
    assert_eq!(node["path"], "");
    assert_eq!(node["role"], "course");
    assert_eq!(node["course_id"], "course-42");
    assert_eq!(node["loose_files"][0]["media_id"], "loose");
    assert_eq!(node["loose_files"][0]["name"], "aula.mp4");
    assert_eq!(node["loose_files"][0]["path"], "cat/aula.mp4");
    assert_eq!(node["loose_files"][0]["size"], 10);
    assert_eq!(node["loose_files"][0]["kind"], "material");
    assert_eq!(node["loose_files"][0]["availability"], "missing");
    assert_eq!(node["loose_files"][0]["unverified"], true);
    assert_eq!(node["children"][0]["role"], "lesson");
    assert_eq!(node["children"][0]["path"], "aula");
    let node_file = &node["children"][0]["files"][0];
    assert_eq!(node_file["media_id"], "m1");
    assert_eq!(node_file["name"], "aula.mp4");
    assert_eq!(node_file["path"], "cat/aula.mp4");
    assert_eq!(node_file["size"], 10);
    assert_eq!(node_file["kind"], "media");
    assert_eq!(node_file["availability"], "available");
    assert_eq!(node_file["unverified"], false);
}

#[test]
fn internal_nodes_omit_the_course_id_field() {
    let internal = CatalogNodeDto::from(node(
        "Módulo",
        "mod",
        NodeRole::Module,
        Vec::new(),
        Vec::new(),
    ));

    let json = serde_json::to_value(internal).unwrap();

    assert!(
        json.get("course_id").is_none(),
        "apenas o nó de topo carrega a identidade do curso: {json}"
    );
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
        let dto = CatalogNodeDto::from(node("N", "n", role, Vec::new(), Vec::new()));
        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["role"], expected, "papel {role:?} precisa ser estável");
    }

    let kinds = [
        (MediaKind::Media, "media"),
        (MediaKind::Subtitle, "subtitle"),
        (MediaKind::Material, "material"),
    ];
    for (kind, expected) in kinds {
        let course = CatalogNodeDto::course(
            "course-1",
            "Curso",
            Catalog {
                complete: true,
                loose_files: vec![file("f", kind, Availability::Available, false)],
                categories: Vec::new(),
            },
        );
        let dto = CatalogDto {
            complete: true,
            loose_files: Vec::new(),
            nodes: vec![course.clone()],
            categories: vec![course],
        };
        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(
            json["nodes"][0]["loose_files"][0]["kind"], expected,
            "tipo {kind:?} precisa ser estável na árvore canônica"
        );
        assert_eq!(
            json["categories"][0]["loose_files"][0]["kind"], expected,
            "tipo {kind:?} precisa ser estável"
        );
    }
}

#[test]
fn catalog_failure_is_a_stable_sanitized_error() {
    let dir = tempfile::tempdir().unwrap();
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

    let courses = [entry("library-1", dir.path())];
    let error = catalog(&database, &courses).unwrap_err();
    assert_eq!(error.code, IpcErrorCode::CatalogUnavailable);

    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "catalog_unavailable");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(
        !message.contains("Cursos secretos"),
        "a mensagem não pode expor caminhos: {message}"
    );
    assert!(
        !message.contains("aula.mp4"),
        "a mensagem não pode expor nomes de arquivo: {message}"
    );
}

#[test]
fn rescan_summary_dto_exposes_only_sanitized_counts() {
    let summary = learnvault_core::RescanSummary {
        ambiguous: vec!["Cursos secretos/aula.mp4".to_owned()],
        ..learnvault_core::RescanSummary::default()
    };

    let json = serde_json::to_value(RescanSummaryDto::from(summary)).unwrap();

    assert_eq!(json["ambiguous_count"], 1);
    assert!(
        json.get("ambiguous").is_none(),
        "o resumo não pode expor a lista de chaves relativas: {json}"
    );
    let rendered = json.to_string();
    assert!(
        !rendered.contains("Cursos secretos") && !rendered.contains("aula.mp4"),
        "nenhum caminho relativo pode cruzar o IPC: {rendered}"
    );
}

#[test]
fn desktop_state_keeps_the_database_in_the_data_directory() {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    let config = directory.path().join("config");
    let state = DesktopState::open(&data, &config).unwrap();

    let catalog = state.catalog().unwrap();
    assert!(catalog.complete);
    assert!(
        data.join("learnvault.sqlite").exists(),
        "o banco local precisa viver no diretório de dados"
    );
    assert!(
        !config.join("learnvault.sqlite").exists(),
        "a configuração não guarda o banco"
    );
}

#[test]
fn importing_a_course_persists_it_and_reports_configured() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let course = make_course(dir.path());

    let status = state.import_course(|| Some(course.clone())).unwrap();

    assert!(status.configured);
    assert_eq!(
        serde_json::to_value(status).unwrap(),
        serde_json::json!({ "configured": true })
    );
    let courses = state.courses();
    assert_eq!(courses.len(), 1);
    assert_eq!(courses[0].root.as_path(), canonical(&course).as_path());
    assert!(
        courses[0].id.starts_with("course-"),
        "a identidade é opaca e estável: {}",
        courses[0].id
    );
    assert!(
        dir.path().join("config").join("library.json").exists(),
        "a configuração precisa ser persistida fora do curso"
    );
    let entries = fs::read_dir(&course)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        entries.is_empty(),
        "a importação não pode criar nem modificar nada dentro do curso: {entries:?}"
    );
}

#[test]
fn importing_the_same_folder_twice_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let course = make_course(dir.path());
    state.import_course(|| Some(course.clone())).unwrap();

    let status = state.import_course(|| Some(course.clone())).unwrap();

    assert!(status.configured);
    assert_eq!(
        state.courses().len(),
        1,
        "a mesma pasta não pode virar dois cursos"
    );
}

#[test]
fn cancelling_the_import_preserves_the_previous_state() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let course = make_course(dir.path());
    state.import_course(|| Some(course.clone())).unwrap();

    let cancelled = state.import_course(|| None).unwrap();

    assert!(
        cancelled.configured,
        "cancelar o seletor preserva a configuração anterior e devolve o estado atual"
    );
    assert_eq!(state.courses().len(), 1);
    assert_eq!(
        state.courses()[0].root.as_path(),
        canonical(&course).as_path()
    );
}

#[test]
fn an_invalid_import_preserves_state_and_hides_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let secret = dir.path().join("Cursos secretos.mp4");
    fs::write(&secret, b"nao sou pasta").unwrap();

    let error = state.import_course(|| Some(secret.clone())).unwrap_err();

    assert_eq!(error.code, IpcErrorCode::CoursePathInvalid);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "course_path_invalid");
    let message = json["message"].as_str().unwrap();
    assert!(!message.is_empty(), "a mensagem precisa existir");
    assert!(
        !message.contains("Cursos secretos") && !message.contains("Cursos"),
        "a mensagem não pode expor o caminho: {message}"
    );
    assert!(
        !state.config_status().configured,
        "uma escolha inválida não pode configurar nem alterar o estado"
    );
}

#[test]
fn a_configuration_write_failure_is_a_stable_sanitized_error() {
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("config");
    fs::write(&blocked, b"nao sou diretorio").unwrap();
    let state = DesktopState::open(&dir.path().join("data"), &blocked).unwrap();
    let course = make_course(dir.path());

    let error = state.import_course(|| Some(course)).unwrap_err();

    assert_eq!(error.code, IpcErrorCode::CoursesConfigUnavailable);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "courses_config_unavailable");
    let message = json["message"].as_str().unwrap();
    assert!(
        !message.contains("nao sou diretorio")
            && !message.contains(&blocked.to_string_lossy().to_string()),
        "a mensagem não pode expor detalhes do sistema: {message}"
    );
    assert!(!state.config_status().configured);
}

#[test]
fn restart_reloads_the_persisted_courses() {
    let dir = tempfile::tempdir().unwrap();
    let course = make_course(dir.path());
    {
        let state = desktop(dir.path());
        state.import_course(|| Some(course.clone())).unwrap();
    }

    let restarted = desktop(dir.path());

    assert!(restarted.config_status().configured);
    assert_eq!(restarted.courses().len(), 1);
    assert_eq!(
        restarted.courses()[0].root.as_path(),
        canonical(&course).as_path()
    );
}

#[test]
fn a_legacy_single_root_configuration_migrates_to_one_course() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(&config).unwrap();
    let course = make_course(dir.path());
    let legacy = serde_json::json!({
        "version": 1,
        "library_root": canonical(&course)
    });
    fs::write(config.join("library.json"), legacy.to_string()).unwrap();

    let courses = load_courses(&config);

    assert_eq!(courses.len(), 1);
    assert_eq!(
        courses[0].id, "library-1",
        "a raiz antiga preserva a identidade histórica para não perder o progresso"
    );
    assert_eq!(courses[0].root.as_path(), canonical(&course).as_path());
}

#[test]
fn restart_ignores_a_corrupt_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("library.json"), b"{ corrompido").unwrap();

    let state = desktop(dir.path());

    assert!(
        !state.config_status().configured,
        "configuração corrompida resulta em estado não configurado, sem erros fatais"
    );
}

#[test]
fn restart_ignores_a_removed_course_folder() {
    let dir = tempfile::tempdir().unwrap();
    let course = make_course(dir.path());
    {
        let state = desktop(dir.path());
        state.import_course(|| Some(course.clone())).unwrap();
    }
    fs::remove_dir_all(&course).unwrap();

    let restarted = desktop(dir.path());

    assert!(
        !restarted.config_status().configured,
        "um curso cuja pasta sumiu precisa ser descartado no carregamento"
    );
    assert!(restarted.courses().is_empty());
}

#[test]
fn removing_a_course_updates_memory_disk_and_status() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let course = make_course(dir.path());
    state.import_course(|| Some(course.clone())).unwrap();
    let id = state.courses()[0].id.clone();

    let status = state.remove_course(&id).unwrap();

    assert!(!status.configured);
    assert!(state.courses().is_empty());
    let persisted = load_courses(&dir.path().join("config"));
    assert!(persisted.is_empty(), "a remoção precisa ser persistida");
    assert!(
        course.exists(),
        "remover um curso nunca pode apagar arquivos"
    );

    // Remover de novo é idempotente.
    let again = state.remove_course(&id).unwrap();
    assert!(!again.configured);
}

#[test]
fn runtime_configuration_revalidates_a_revoked_course() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let course = make_course(dir.path());
    state.import_course(|| Some(course.clone())).unwrap();
    assert!(state.config_status().configured);

    fs::remove_dir_all(&course).unwrap();

    assert_eq!(
        serde_json::to_value(state.config_status()).unwrap(),
        serde_json::json!({ "configured": false })
    );
}

#[test]
fn concurrent_imports_keep_memory_and_disk_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let roots: Vec<PathBuf> = (0..4)
        .map(|index| {
            let path = dir.path().join(format!("curso{index}"));
            fs::create_dir(&path).unwrap();
            path
        })
        .collect();

    std::thread::scope(|scope| {
        for root in &roots {
            let state = &state;
            scope.spawn(move || {
                let root = root.clone();
                state.import_course(|| Some(root)).unwrap();
            });
        }
    });

    let mut memory: Vec<PathBuf> = state
        .courses()
        .into_iter()
        .map(|course| course.root.as_path().to_path_buf())
        .collect();
    memory.sort();
    let mut disk: Vec<PathBuf> = load_courses(&dir.path().join("config"))
        .into_iter()
        .map(|course| course.root.as_path().to_path_buf())
        .collect();
    disk.sort();
    assert_eq!(
        memory, disk,
        "importações concorrentes não podem divergir entre memória e disco"
    );
}

#[test]
fn media_removal_impact_and_removal_flow_in_desktop() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());
    let course_dir = make_course(dir.path());
    let aula_dir = course_dir.join("Aula 1");
    fs::create_dir_all(&aula_dir).unwrap();
    fs::write(aula_dir.join("video.mp4"), b"conteudo de video").unwrap();

    state.import_course(|| Some(course_dir.clone())).unwrap();
    state.rescan().unwrap();

    let cat = state.catalog().unwrap();
    assert_eq!(cat.categories.len(), 1);
    let media_id = cat.categories[0].children[0].children[0].files[0]
        .media_id
        .clone();

    // 1. Impacto inicial sem progresso
    let impact = state.media_removal_impact(&media_id).unwrap();
    assert_eq!(impact.media_id, media_id);
    assert_eq!(impact.progress_owners, 0);
    assert!(!impact.already_removed);

    // 2. Grava progresso no desktop
    state.record_position(&media_id, 25).unwrap();

    // 3. Impacto agora acusa 1 dono com progresso
    let impact = state.media_removal_impact(&media_id).unwrap();
    assert_eq!(impact.progress_owners, 1);

    // 4. Tentar remover sem confirmação deve falhar com MediaRemovalNeedsConfirmation
    let err = state.remove_media(&media_id, false).unwrap_err();
    assert_eq!(err.code, IpcErrorCode::MediaRemovalNeedsConfirmation);

    // 5. Remover com confirmação tem sucesso
    let outcome = state.remove_media(&media_id, true).unwrap();
    assert_eq!(outcome.media_id, media_id);
    assert!(outcome.removed_now);
    assert_eq!(outcome.preserved_owners, 1);

    // 6. Repetir remoção é idempotente
    let repeat = state.remove_media(&media_id, true).unwrap();
    assert_eq!(repeat.media_id, media_id);
    assert!(!repeat.removed_now);

    // 7. Catálogo passa a omitir a mídia
    let cat_after = state.catalog().unwrap();
    assert!(cat_after.categories[0].children.is_empty());

    // 8. Rescan não desfaz a remoção
    state.rescan().unwrap();
    let cat_rescan = state.catalog().unwrap();
    assert!(cat_rescan.categories[0].children.is_empty());

    // 9. Progresso continua legível e preservado
    let progress = state.study_progress(&media_id).unwrap();
    assert!(progress.recorded);
    assert_eq!(progress.position_seconds, 25);
}

#[test]
fn desktop_catalog_order_persists_locally_validates_siblings_and_resets() {
    let dir = tempfile::tempdir().unwrap();
    let course = make_course(dir.path());
    for module in ["Módulo 1", "Módulo 2"] {
        let lesson = course.join(module).join("Aula 1");
        fs::create_dir_all(&lesson).unwrap();
        fs::write(lesson.join("video.mp4"), b"video").unwrap();
    }
    let state = desktop(dir.path());
    state.import_course(|| Some(course.clone())).unwrap();
    state.rescan().unwrap();
    let course_id = state.courses()[0].id.clone();

    state
        .reorder_catalog(
            &course_id,
            "",
            &["Módulo 2".to_owned(), "Módulo 1".to_owned()],
        )
        .unwrap();
    let catalog = state.catalog().unwrap();
    assert_eq!(catalog.categories[0].children[0].path, "Módulo 2");

    let invalid = state.reorder_catalog(&course_id, "", &["not-a-sibling".to_owned()]);
    assert_eq!(invalid.unwrap_err().code, IpcErrorCode::InvalidCatalogOrder);
    assert_eq!(
        state.catalog().unwrap().categories[0].children[0].path,
        "Módulo 2",
        "ordem inválida não pode substituir a preferência salva"
    );

    let restarted = desktop(dir.path());
    assert_eq!(
        restarted.catalog().unwrap().categories[0].children[0].path,
        "Módulo 2",
        "preferência local deve persistir ao reiniciar"
    );

    restarted.reset_catalog_order(&course_id, "").unwrap();
    assert_eq!(
        restarted.catalog().unwrap().categories[0].children[0].path,
        "Módulo 1",
        "reset deve restaurar ordem natural"
    );
    assert!(course.join("Módulo 1/Aula 1/video.mp4").exists());
}
