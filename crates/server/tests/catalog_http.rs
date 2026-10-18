use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_core::Database;
use learnvault_server::{AppState, Config, build_router, hash_password};
use std::{
    fs::{self, OpenOptions},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tower::ServiceExt;

/// Deadline generoso para a sincronização entre processos do teste de lock de
/// rescan. Um valor curto (segundos) vira flake em runners de CI carregados.
const RESCAN_LOCK_TIMEOUT: Duration = Duration::from_secs(60);

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some("admin".to_owned());
    config.admin_password = Some("uma senha local muito segura".to_owned());
    config.cookie_secure = false;
    config
}

async fn login(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"uma senha local muito segura"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

fn assert_private_no_store(response: &Response) {
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
}

#[tokio::test]
async fn catalog_requires_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let response = app
        .oneshot(Request::get("/api/catalog").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&response);
}

#[tokio::test]
async fn rescan_requires_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let response = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&response);
}

#[tokio::test]
async fn catalog_and_rescan_reject_invalid_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = "learnvault_session=invalid-token";

    let catalog = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&catalog);

    let rescan = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("cookie", cookie)
                .header("origin", "http://localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rescan.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&rescan);
}

#[tokio::test]
async fn catalog_and_rescan_reject_requests_when_blocking_capacity_is_saturated() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(config(&temp)).await.unwrap();
    let app = build_router(state.clone());
    let cookie = login(&app).await;
    let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();

    let catalog = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_private_no_store(&catalog);
    let catalog_body = catalog.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&catalog_body).unwrap()["error"],
        "server busy"
    );

    let rescan = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rescan.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_private_no_store(&rescan);
    let rescan_body = rescan.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&rescan_body).unwrap()["error"],
        "server busy"
    );
}

#[tokio::test]
async fn non_admin_cannot_rescan() {
    let temp = tempfile::tempdir().unwrap();
    let database = Database::open(temp.path().join("catalog.sqlite")).unwrap();
    let password_hash = hash_password("senha local longa para aluno").unwrap();
    database
        .connection()
        .execute(
            "INSERT INTO user (user_id, username, password_hash, is_admin, created_at)
             VALUES ('usr-reader', 'reader', ?1, 0, 1)",
            [&password_hash],
        )
        .unwrap();
    drop(database);

    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"reader","password":"senha local longa para aluno"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let rescan = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rescan.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&rescan);
}

#[tokio::test]
async fn admin_rescan_populates_the_authenticated_catalog() {
    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    std::fs::create_dir_all(library.join("Categoria/Curso")).unwrap();
    std::fs::write(library.join("Categoria/Curso/lesson.mp4"), b"video").unwrap();
    // Arquivo solto diretamente na categoria: a projeção legada não tem onde guardá-lo, mas
    // ele precisa continuar acessível na árvore canônica `nodes`.
    std::fs::write(library.join("Categoria/nota.pdf"), b"nota").unwrap();
    let mut server_config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        library.to_str().unwrap(),
    ])
    .unwrap();
    server_config.admin_username = Some("admin".to_owned());
    server_config.admin_password = Some("uma senha local muito segura".to_owned());
    server_config.cookie_secure = false;
    let app = build_router(AppState::initialize(server_config).await.unwrap());
    let cookie = login(&app).await;

    let missing_origin = app
        .clone()
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_origin.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&missing_origin);

    let rescan = app
        .clone()
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rescan.status(), StatusCode::OK);
    assert_private_no_store(&rescan);
    let rescan_body = rescan.into_body().collect().await.unwrap().to_bytes();
    let rescan_json: serde_json::Value = serde_json::from_slice(&rescan_body).unwrap();
    assert_eq!(rescan_json["complete"], true);
    assert_eq!(rescan_json["courses"], 1);
    assert_eq!(rescan_json["failed_courses"], 0);
    assert_eq!(rescan_json["inserted"], 2);
    assert!(rescan_json["scan_id"].as_i64().unwrap() > 0);
    // Resposta sanitizada: expõe apenas a contagem de chaves ambíguas, nunca os caminhos
    // relativos que as compõem. Produzir ambiguidade real por HTTP não é portável (depende
    // da normalização de separadores por plataforma); o caso com caminho é coberto de forma
    // determinística pelo teste unitário de `RescanResponse::from` em `catalog.rs`.
    assert!(
        rescan_json["ambiguous_count"].is_u64(),
        "ambiguous_count precisa ser uma contagem numérica: {rescan_json}"
    );
    assert!(
        rescan_json.get("ambiguous").is_none(),
        "a lista de caminhos ambíguos não pode cruzar a API: {rescan_json}"
    );
    let rescan_text = String::from_utf8_lossy(&rescan_body);
    assert!(
        !rescan_text.contains("Categoria")
            && !rescan_text.contains("lesson.mp4")
            && !rescan_text.contains("nota.pdf"),
        "a resposta do rescan não pode vazar caminhos relativos: {rescan_text}"
    );

    let catalog = app
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    assert_private_no_store(&catalog);
    let body = catalog.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["complete"], true);

    // O nó de topo é o curso registrado a partir da raiz semeada: identidade opaca
    // (`library-1`, a identidade histórica da raiz) e título derivado da pasta.
    let course = &json["nodes"][0];
    assert_eq!(course["title"], "library");
    assert_eq!(course["path"], "");
    assert_eq!(course["role"], "course");
    assert_eq!(course["course_id"], "library-1");
    // `nodes` é a árvore recursiva canônica, o contrato preferido pela SPA: a categoria e a
    // subárvore de aulas ficam sob o nó do curso, com identidade de mídia, caminhos
    // relativos, disponibilidade e verificação preservados.
    let category = &course["children"][0];
    assert_eq!(category["title"], "Categoria");
    assert_eq!(category["path"], "Categoria");
    assert_eq!(category["role"], "category");
    let lesson = &category["children"][0];
    assert_eq!(lesson["title"], "Curso");
    assert_eq!(lesson["path"], "Categoria/Curso");
    assert_eq!(lesson["role"], "lesson");
    let node_file = &lesson["files"][0];
    assert_eq!(node_file["name"], "lesson.mp4");
    assert_eq!(node_file["path"], "Categoria/Curso/lesson.mp4");
    assert_eq!(node_file["size"], 5);
    assert!(node_file["mtime_seconds"].as_i64().is_some_and(|mtime| mtime >= 0));
    assert_eq!(node_file["kind"], "media");
    assert_eq!(node_file["availability"], "available");
    assert_eq!(node_file["unverified"], false);
    assert!(
        node_file["media_id"].is_string(),
        "a identidade de mídia precisa sobreviver em nodes"
    );
    // O arquivo solto da categoria não pertence a nenhuma aula: ele continua acessível, sem
    // perda, nos arquivos soltos do próprio nó da categoria.
    let loose = &category["loose_files"][0];
    assert_eq!(loose["name"], "nota.pdf");
    assert_eq!(loose["path"], "Categoria/nota.pdf");
    assert!(
        loose["media_id"].is_string(),
        "o arquivo solto também carrega identidade de mídia em nodes"
    );

    // `categories` expõe a mesma lista de cursos de topo: cada nó tem papel `course`,
    // `course_id` opaco e a subárvore escaneada sob ele.
    assert_eq!(json["categories"], json["nodes"]);
    assert_eq!(json["categories"][0]["role"], "course");
    assert_eq!(json["categories"][0]["course_id"], "library-1");
    let file = find_file(&json["categories"][0]).expect("lesson file in catalog");
    assert_eq!(file["name"], "lesson.mp4");
    assert_eq!(file["path"], "Categoria/Curso/lesson.mp4");
    assert_eq!(file["size"], 5);
    assert_eq!(file["kind"], "media");
    assert_eq!(file["availability"], "available");
    assert_eq!(file["unverified"], false);
}

#[tokio::test]
async fn catalog_reports_database_errors_without_caching_or_leaking_paths() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;
    Database::open(temp.path().join("catalog.sqlite"))
        .unwrap()
        .connection()
        .execute_batch("DROP TABLE media;")
        .unwrap();

    let response = app
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_private_no_store(&response);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "catalog unavailable");
    assert!(
        !body
            .windows(temp.path().to_str().unwrap().len())
            .any(|window| { window == temp.path().to_str().unwrap().as_bytes() })
    );
}

#[tokio::test]
async fn rescan_reports_a_missing_course_root_as_failed_without_exposing_paths() {
    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    fs::create_dir(&library).unwrap();
    let mut server_config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        library.to_str().unwrap(),
    ])
    .unwrap();
    server_config.admin_username = Some("admin".to_owned());
    server_config.admin_password = Some("uma senha local muito segura".to_owned());
    server_config.cookie_secure = false;
    let app = build_router(AppState::initialize(server_config).await.unwrap());
    let cookie = login(&app).await;
    fs::remove_dir(&library).unwrap();

    let response = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Uma raiz removida não derruba os demais cursos: a resposta agrega a falha sem
    // seguir o alvo e sem expor o caminho.
    assert_eq!(response.status(), StatusCode::OK);
    assert_private_no_store(&response);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["complete"], false);
    assert_eq!(json["courses"], 1);
    assert_eq!(json["failed_courses"], 1);
    assert!(
        !body
            .windows(library.to_str().unwrap().len())
            .any(|window| { window == library.to_str().unwrap().as_bytes() })
    );
}

#[tokio::test]
async fn rescan_returns_conflict_while_another_process_holds_its_lock() {
    let temp = tempfile::tempdir().unwrap();
    let lock_path = temp.path().join("catalog.sqlite.rescan.lock");
    let ready_path = temp.path().join("lock-ready");

    // Bootstrap (Argon2id do admin) e login (Argon2id) são deliberadamente lentos
    // e podem levar segundos num runner de CI carregado. Prepare-os antes de
    // spawnar o filho para que a janela entre `lock-ready` e o release cubra
    // apenas o request, e não o trabalho pesado de startup.
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app).await;

    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("rescan_lock_helper")
        .env("LEARNVAULT_RESCAN_LOCK_HELPER", "1")
        .env("LEARNVAULT_RESCAN_LOCK_PATH", &lock_path)
        .env("LEARNVAULT_RESCAN_LOCK_READY", &ready_path)
        // O release é sinalizado pelo canal de stdin do filho, não por um arquivo
        // com deadline: um canal real é determinístico e libera o processo no
        // fechamento mesmo se o pai falhar, sem deixar auxiliar órfão preso.
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_file(&ready_path);

    let response = app
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Fechar o stdin do filho faz o `read_line` retornar EOF e o auxiliar encerrar.
    // `Stdio::piped()` é invariante do teste: se sumir, `wait()` travaria para
    // sempre, então falhe alto em vez de pendurar a suíte.
    let stdin = child
        .stdin
        .take()
        .expect("o filho deve ter stdin encanado para o release");
    drop(stdin);
    assert!(
        child.wait().unwrap().success(),
        "o processo auxiliar deve sair com sucesso depois do release"
    );
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_private_no_store(&response);
}

#[test]
fn rescan_lock_helper() {
    // Marcador dedicado: só o processo auxiliar spawnado por este arquivo o define.
    // Sem ele, o teste nunca executa o corpo, evitando travar em `read_line` quando
    // rodado dentro da suíte principal (regressão que o poll com deadline continha).
    if std::env::var("LEARNVAULT_RESCAN_LOCK_HELPER").as_deref() != Ok("1") {
        return;
    }
    let lock_path = std::env::var("LEARNVAULT_RESCAN_LOCK_PATH").unwrap();
    let ready_path = std::env::var("LEARNVAULT_RESCAN_LOCK_READY").unwrap();
    let _lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .unwrap();
    _lock.lock().unwrap();
    fs::write(ready_path, []).unwrap();

    // Segura o lock até o pai sinalizar o release pelo canal de stdin. Um
    // `read_line` bloqueante (ou EOF) substitui o polling com deadline: o filho
    // nunca faz `sleep` e nunca fica preso além do fechamento do pai.
    let mut release = String::new();
    let _ = std::io::stdin().read_line(&mut release);
}

/// Espera por um arquivo de sincronização entre processos com um deadline
/// generoso: em CI carregado, o processo auxiliar pode demorar bem mais que
/// alguns segundos para ser spawnado e sinalizar `lock-ready`.
fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + RESCAN_LOCK_TIMEOUT;
    while !path.exists() {
        assert!(Instant::now() < deadline, "timed out waiting for {path:?}");
        thread::sleep(Duration::from_millis(10));
    }
}

fn find_file(value: &serde_json::Value) -> Option<&serde_json::Value> {
    if value.get("name").and_then(serde_json::Value::as_str) == Some("lesson.mp4") {
        return Some(value);
    }
    for key in ["files", "loose_files", "children"] {
        if let Some(values) = value.get(key).and_then(serde_json::Value::as_array) {
            for value in values {
                if let Some(file) = find_file(value) {
                    return Some(file);
                }
            }
        }
    }
    None
}
