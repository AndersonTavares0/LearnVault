use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_core::Database;
use learnvault_server::{AppState, Config, build_router, hash_password};
use tempfile::TempDir;
use tower::ServiceExt;

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("courses.sqlite").to_str().unwrap(),
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

async fn import(app: &axum::Router, cookie: &str, path: &std::path::Path) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/admin/courses")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "path": path.to_str().unwrap() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn json_body(response: Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn course_import_rescan_and_removal_flow() {
    let temp = tempfile::tempdir().unwrap();
    let course = temp.path().join("01 - Física");
    std::fs::create_dir_all(course.join("Aula 1")).unwrap();
    std::fs::write(course.join("Aula 1/video.mp4"), b"video").unwrap();

    let server_config = config(&temp);
    let state = AppState::initialize(server_config.clone()).await.unwrap();
    let app = build_router(state);
    let cookie = login(&app).await;

    // Sem cursos registrados o catálogo é vazio e o rescan é recusado.
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
    assert_eq!(catalog.status(), StatusCode::OK);
    let json = json_body(catalog).await;
    assert_eq!(json["categories"], serde_json::json!([]));

    let empty_rescan = app
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
    assert_eq!(empty_rescan.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(empty_rescan).await["error"],
        "no_courses_registered"
    );

    // Importar sem origem é recusado como CSRF; com origem, registra o curso.
    let no_origin = app
        .clone()
        .oneshot(
            Request::post("/api/admin/courses")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "path": course.to_str().unwrap() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_origin.status(), StatusCode::FORBIDDEN);

    let imported = import(&app, &cookie, &course).await;
    assert_eq!(imported.status(), StatusCode::OK);
    assert_private_no_store(&imported);
    let json = json_body(imported).await;
    let course_id = json["course"]["id"].as_str().unwrap().to_owned();
    assert!(course_id.starts_with("course-"));
    assert_eq!(json["course"]["title"], "Física");
    assert!(
        !course_id.contains(course.to_str().unwrap()),
        "a identidade nunca carrega o caminho"
    );

    // Importar a mesma pasta de novo é recusado; um caminho inválido também.
    let duplicate = import(&app, &cookie, &course).await;
    assert_eq!(duplicate.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(duplicate).await["error"],
        "course_already_registered"
    );

    let missing = import(&app, &cookie, &temp.path().join("nao-existe")).await;
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(missing).await["error"], "invalid_course_path");

    // O registro sobrevive a uma reinicialização do estado antes de qualquer varredura.
    let restarted = AppState::initialize(server_config).await.unwrap();
    let app = build_router(restarted);
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
    let json = json_body(catalog).await;
    assert_eq!(json["categories"][0]["course_id"], course_id);

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
    let json = json_body(rescan).await;
    assert_eq!(json["courses"], 1);
    assert_eq!(json["failed_courses"], 0);
    assert_eq!(json["inserted"], 1);

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
    let json = json_body(catalog).await;
    let course_node = &json["categories"][0];
    assert_eq!(course_node["title"], "Física");
    assert_eq!(course_node["role"], "course");
    assert_eq!(course_node["course_id"], course_id);
    // Dentro do curso a hierarquia se reancora: o primeiro nível vira o conteúdo do
    // curso e os arquivos diretos nele ficam como soltos do próprio curso.
    assert_eq!(
        course_node["children"][0]["loose_files"][0]["path"],
        "Aula 1/video.mp4"
    );

    // Mídia do curso é servida com o par curso+caminho; um curso desconhecido é 404.
    let media = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/media?course={course_id}&path=Aula%201/video.mp4"
            ))
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(media.status(), StatusCode::OK);

    let unknown_course = app
        .clone()
        .oneshot(
            Request::get("/api/media?course=course-unknown&path=Aula%201/video.mp4")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_course.status(), StatusCode::NOT_FOUND);

    // Remover é idempotente e deixa o catálogo vazio; os arquivos ficam intactos.
    let remove = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/admin/courses/{course_id}"))
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(remove.status(), StatusCode::OK);
    assert_eq!(json_body(remove).await["removed"], true);

    let remove_again = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/admin/courses/{course_id}"))
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(remove_again).await["removed"], false);

    let catalog = app
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        json_body(catalog).await["categories"],
        serde_json::json!([])
    );
    assert!(course.join("Aula 1/video.mp4").exists());
}

#[tokio::test]
async fn course_import_requires_admin() {
    let temp = tempfile::tempdir().unwrap();
    let course = temp.path().join("Curso");
    std::fs::create_dir(&course).unwrap();
    let database = Database::open(temp.path().join("courses.sqlite")).unwrap();
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

    let imported = import(&app, &cookie, &course).await;
    assert_eq!(imported.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&imported);

    let removed = app
        .oneshot(
            Request::delete("/api/admin/courses/course-unknown")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn course_import_requires_authentication() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let response = app
        .oneshot(
            Request::post("/api/admin/courses")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"path":"."}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&response);
}
