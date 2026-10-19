//! Ordenação manual de módulos e aulas exposta por HTTP (issue #239).
//!
//! Os casos usam uma biblioteca sintética com módulos numerados, registrada como o curso
//! semente, e verificam persistência entre reinício e rescan, além da recusa de listas
//! que não sejam uma permutação dos irmãos.

use std::path::Path;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_server::{AppState, Config, build_router};
use tempfile::TempDir;
use tower::ServiceExt;

fn config(temp: &TempDir, library: &Path) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("catalog.sqlite").to_str().unwrap(),
        "--library-root",
        library.to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some("admin".to_owned());
    config.admin_password = Some("uma senha local muito segura".to_owned());
    config.cookie_secure = false;
    config
}

/// Cria um módulo numerado com uma aula, dentro da biblioteca sintética.
fn write_module(library: &Path, module: &str) {
    let directory = library.join(module).join("Aula 1");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("aula.mp4"), b"video").unwrap();
}

fn library(temp: &TempDir) -> std::path::PathBuf {
    let library = temp.path().join("library");
    for module in ["Modulo 2", "Modulo 3", "Modulo 10"] {
        write_module(&library, module);
    }
    library
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

async fn rescan(app: &axum::Router, cookie: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/admin/catalog/rescan")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn catalog(app: &axum::Router, cookie: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

fn module_paths(catalog: &serde_json::Value) -> Vec<String> {
    catalog["categories"][0]["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["path"].as_str().unwrap().to_owned())
        .collect()
}

async fn put_order(
    app: &axum::Router,
    cookie: &str,
    course_id: &str,
    parent: &str,
    children: &[&str],
) -> Response {
    app.clone()
        .oneshot(
            Request::put(format!("/api/admin/courses/{course_id}/order"))
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "parent": parent, "children": children }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn delete_order(app: &axum::Router, cookie: &str, query: &str) -> Response {
    app.clone()
        .oneshot(
            Request::delete(format!("/api/admin/courses/library-1/order{query}"))
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

/// Listagem recursiva de caminhos relativos da biblioteca, para provar que nada muda.
fn snapshot(root: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path.clone());
            }
            paths.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    paths.sort();
    paths
}

#[tokio::test]
async fn order_requires_authentication_and_same_origin() {
    let temp = tempfile::tempdir().unwrap();
    let library = library(&temp);
    let app = build_router(AppState::initialize(config(&temp, &library)).await.unwrap());

    let unauthenticated = put_order(&app, "learnvault_session=invalid", "library-1", "", &[]).await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&unauthenticated);

    let cookie = login(&app).await;
    let no_origin = app
        .clone()
        .oneshot(
            Request::put("/api/admin/courses/library-1/order")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "parent": "", "children": [] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_origin.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&no_origin);
}

#[tokio::test]
async fn order_round_trips_persists_across_rescan_and_restart() {
    let temp = tempfile::tempdir().unwrap();
    let library = library(&temp);
    let app = build_router(AppState::initialize(config(&temp, &library)).await.unwrap());
    let cookie = login(&app).await;
    rescan(&app, &cookie).await;

    assert_eq!(
        module_paths(&catalog(&app, &cookie).await),
        ["Modulo 2", "Modulo 3", "Modulo 10"]
    );

    let before = snapshot(&library);
    let put = put_order(
        &app,
        &cookie,
        "library-1",
        "",
        &["Modulo 10", "Modulo 2", "Modulo 3"],
    )
    .await;
    assert_eq!(put.status(), StatusCode::OK);
    assert_private_no_store(&put);
    let put_json: serde_json::Value =
        serde_json::from_slice(&put.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(put_json["parent"], "");
    assert_eq!(
        put_json["children"],
        serde_json::json!(["Modulo 10", "Modulo 2", "Modulo 3"])
    );

    assert_eq!(
        module_paths(&catalog(&app, &cookie).await),
        ["Modulo 10", "Modulo 2", "Modulo 3"]
    );

    // A preferência sobrevive a um rescan.
    rescan(&app, &cookie).await;
    assert_eq!(
        module_paths(&catalog(&app, &cookie).await),
        ["Modulo 10", "Modulo 2", "Modulo 3"]
    );

    // Nenhum arquivo do curso foi criado, movido ou alterado.
    assert_eq!(snapshot(&library), before, "a biblioteca é somente leitura");

    // E sobrevive a um reinício: um novo estado abre o mesmo banco e arquivo de ordem.
    let restarted = build_router(AppState::initialize(config(&temp, &library)).await.unwrap());
    let cookie = login(&restarted).await;
    assert_eq!(
        module_paths(&catalog(&restarted, &cookie).await),
        ["Modulo 10", "Modulo 2", "Modulo 3"]
    );
}

#[tokio::test]
async fn order_rejects_lists_that_are_not_a_permutation_of_siblings() {
    let temp = tempfile::tempdir().unwrap();
    let library = library(&temp);
    let app = build_router(AppState::initialize(config(&temp, &library)).await.unwrap());
    let cookie = login(&app).await;
    rescan(&app, &cookie).await;

    // Um caminho de fora do conjunto de irmãos.
    let foreign = put_order(
        &app,
        &cookie,
        "library-1",
        "",
        &["Modulo 10", "Modulo 2", "Modulo 3", "Modulo Extra"],
    )
    .await;
    assert_eq!(foreign.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_private_no_store(&foreign);

    // Um pai desconhecido.
    let unknown_parent = put_order(&app, &cookie, "library-1", "Modulo X", &["Modulo 2"]).await;
    assert_eq!(unknown_parent.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Um filho de outro pai.
    let wrong_child = put_order(&app, &cookie, "library-1", "Modulo 2", &["Modulo 3"]).await;
    assert_eq!(wrong_child.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // A ordem automática continua intacta.
    assert_eq!(
        module_paths(&catalog(&app, &cookie).await),
        ["Modulo 2", "Modulo 3", "Modulo 10"]
    );
}

#[tokio::test]
async fn order_rejects_an_unknown_course() {
    let temp = tempfile::tempdir().unwrap();
    let library = library(&temp);
    let app = build_router(AppState::initialize(config(&temp, &library)).await.unwrap());
    let cookie = login(&app).await;

    let response = put_order(&app, &cookie, "unknown-course", "", &[]).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_private_no_store(&response);
}

#[tokio::test]
async fn reset_restores_the_automatic_order() {
    let temp = tempfile::tempdir().unwrap();
    let library = library(&temp);
    let app = build_router(AppState::initialize(config(&temp, &library)).await.unwrap());
    let cookie = login(&app).await;
    rescan(&app, &cookie).await;

    put_order(
        &app,
        &cookie,
        "library-1",
        "",
        &["Modulo 10", "Modulo 2", "Modulo 3"],
    )
    .await;
    assert_eq!(
        module_paths(&catalog(&app, &cookie).await),
        ["Modulo 10", "Modulo 2", "Modulo 3"]
    );

    let reset = delete_order(&app, &cookie, "").await;
    assert_eq!(reset.status(), StatusCode::OK);
    assert_private_no_store(&reset);
    let json: serde_json::Value =
        serde_json::from_slice(&reset.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json["removed"], true);

    assert_eq!(
        module_paths(&catalog(&app, &cookie).await),
        ["Modulo 2", "Modulo 3", "Modulo 10"]
    );

    // Repetir a restauração é idempotente.
    let again = delete_order(&app, &cookie, "").await;
    let json: serde_json::Value =
        serde_json::from_slice(&again.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json["removed"], false);
}

#[tokio::test]
async fn order_accepts_a_large_sibling_list_above_the_default_body_limit() {
    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    // 200 módulos com nomes longos: o corpo do pedido passa de 8 KiB (o limite padrão do
    // roteador), então a rota precisa do limite próprio para não responder 413.
    let modules: Vec<String> = (0..200)
        .map(|index| format!("Modulo {index:040}"))
        .collect();
    for module in &modules {
        let directory = library.join(module).join("Aula 1");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("aula.mp4"), b"video").unwrap();
    }

    let app = build_router(AppState::initialize(config(&temp, &library)).await.unwrap());
    let cookie = login(&app).await;
    rescan(&app, &cookie).await;

    let reversed: Vec<String> = modules.iter().rev().cloned().collect();
    let body = serde_json::json!({ "parent": "", "children": reversed }).to_string();
    assert!(
        body.len() > 8 * 1024,
        "o corpo do teste precisa exceder o limite padrão: {} bytes",
        body.len()
    );

    let put = app
        .clone()
        .oneshot(
            Request::put("/api/admin/courses/library-1/order")
                .header("origin", "http://localhost")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(put.status(), StatusCode::OK);

    let paths = module_paths(&catalog(&app, &cookie).await);
    assert_eq!(paths.len(), 200);
    assert_eq!(paths[0], reversed[0]);
    assert_eq!(paths[199], reversed[199]);
}

#[tokio::test]
async fn order_rejects_requests_when_blocking_capacity_is_saturated() {
    let temp = tempfile::tempdir().unwrap();
    let library = library(&temp);
    let state = AppState::initialize(config(&temp, &library)).await.unwrap();
    let app = build_router(state.clone());
    let cookie = login(&app).await;
    rescan(&app, &cookie).await;
    let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();

    let response = put_order(
        &app,
        &cookie,
        "library-1",
        "",
        &["Modulo 10", "Modulo 2", "Modulo 3"],
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_private_no_store(&response);
}
