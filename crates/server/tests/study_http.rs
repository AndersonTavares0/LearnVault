//! Testes HTTP do progresso de estudo.
//!
//! O dono do progresso é sempre o usuário da sessão: cada caso prova que o adaptador
//! deriva o dono da autenticação, valida o corpo tipado, traduz os erros do núcleo e
//! persiste no banco SQLite real (sobrevivendo a um reinício equivalente do servidor).
//! Nenhum caso lê ou escreve a biblioteca física.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_core::{Database, KeyMode};
use learnvault_server::{AppState, Config, build_router, hash_password};
use tempfile::TempDir;
use tower::ServiceExt;

const ADMIN: &str = "admin";
const ADMIN_PASSWORD: &str = "uma senha local muito segura";
const READER: &str = "reader";
const READER_PASSWORD: &str = "senha local longa para aluno";

/// Usuário criado pelo bootstrap do servidor; dono do progresso do administrador.
const ADMIN_ID: &str = "usr-admin";
/// Usuário comum criado pelo próprio teste, para provar o isolamento por dono.
const READER_ID: &str = "usr-reader";

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("study.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some(ADMIN.to_owned());
    config.admin_password = Some(ADMIN_PASSWORD.to_owned());
    config.cookie_secure = false;
    config
}

fn database_path(temp: &TempDir) -> std::path::PathBuf {
    temp.path().join("study.sqlite")
}

/// Prepara diretório de dados, mídia conhecida e, opcionalmente, estado de estudo inicial.
///
/// O banco é gravado antes do `AppState` abrir: as linhas de mídia precisam existir para
/// que o progresso seja aceito pela chave estrangeira do schema.
fn seed(temp: &TempDir, media: &[&str], states: &[(&str, &str, i64, bool, i64)]) {
    let db = Database::open(database_path(temp)).unwrap();
    db.connection()
        .execute_batch("INSERT INTO library (library_id, available) VALUES ('library-1', 1);")
        .unwrap();
    for media_id in media {
        let path = format!("curso/{media_id}.mp4");
        let key = learnvault_core::paths::relative_path_key(&path, KeyMode::for_host()).unwrap();
        db.connection()
            .execute_batch(&format!(
                "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
                 VALUES ('{media_id}', 'library-1', '{path}', '{key}', 1024, 1760000000, 'available');"
            ))
            .unwrap();
    }
    for (owner, media_id, position, completed, updated_at) in states {
        db.connection()
            .execute_batch(&format!(
                "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
                 VALUES ('{owner}', '{media_id}', {position}, {}, {updated_at});",
                i64::from(*completed)
            ))
            .unwrap();
    }
}

/// Cria um usuário comum antes do `AppState` abrir o banco.
fn seed_user(temp: &TempDir, user_id: &str, username: &str, password: &str) {
    let hash = hash_password(password).unwrap();
    Database::open(database_path(temp))
        .unwrap()
        .connection()
        .execute_batch(&format!(
            "INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials)
             VALUES ('{user_id}', '{username}', '{hash}', 0, 1, 0);"
        ))
        .unwrap();
}

async fn login(app: &axum::Router, username: &str, password: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"username":"{username}","password":"{password}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "login de teste");
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

fn get_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::get(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

fn post_json(uri: &str, cookie: &str, body: &str) -> Request<Body> {
    Request::post(uri)
        .header("origin", "http://localhost")
        .header("cookie", cookie)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn json_body(response: Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
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
async fn study_requires_authentication() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let requests = [
        Request::get("/api/study/recent")
            .body(Body::empty())
            .unwrap(),
        Request::get("/api/study/progress?media_id=m1")
            .body(Body::empty())
            .unwrap(),
        Request::get("/api/study/courses/library-1/progress")
            .body(Body::empty())
            .unwrap(),
        Request::post("/api/study/position")
            .header("origin", "http://localhost")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"media_id":"m1","position_seconds":5}"#))
            .unwrap(),
        Request::post("/api/study/completed")
            .header("origin", "http://localhost")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"media_id":"m1","completed":true}"#))
            .unwrap(),
        Request::get("/api/study/capabilities")
            .body(Body::empty())
            .unwrap(),
    ];

    for request in requests {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_private_no_store(&response);
    }
}

/// O endpoint de capacidades é o contrato explícito de detecção de recursos do Study. Ele
/// tem o mesmo shape do serviço Go de referência, mas relata os valores reais deste
/// servidor: a identidade de mídia do Rust é o `media_id` opaco, que sobrevive a
/// renomeações, então `stable_media_identity` é `true` (o Go de referência relata o
/// `relative_path`, não estável). A resposta exige sessão e não é armazenável.
#[tokio::test]
async fn study_capabilities_reports_the_feature_contract() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let response = app
        .oneshot(get_with_cookie("/api/study/capabilities", &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_private_no_store(&response);
    let body = json_body(response).await;
    assert_eq!(body["study"], true);
    assert_eq!(body["media_identity"], "media_id");
    assert_eq!(body["stable_media_identity"], true);
    assert_eq!(body["recent_default_limit"].as_u64(), Some(20));
    assert_eq!(
        body["recent_max_limit"].as_u64(),
        Some(learnvault_core::MAX_RECENT_STUDY as u64)
    );
    assert_eq!(body["removal_keeps_progress"], true);
    // O contrato é fechado: nada além dos campos acordados é exposto.
    assert_eq!(body.as_object().unwrap().len(), 6);
}

#[tokio::test]
async fn study_capabilities_rejects_unsupported_methods() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let response = app
        .oneshot(
            Request::post("/api/study/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn study_rejects_invalid_sessions() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let recent = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/recent",
            "learnvault_session=invalid-token",
        ))
        .await
        .unwrap();
    assert_eq!(recent.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&recent);

    let position = app
        .oneshot(post_json(
            "/api/study/position",
            "learnvault_session=invalid-token",
            r#"{"media_id":"m1","position_seconds":5}"#,
        ))
        .await
        .unwrap();
    assert_eq!(position.status(), StatusCode::UNAUTHORIZED);
    assert_private_no_store(&position);
}

#[tokio::test]
async fn study_rejects_requests_when_blocking_capacity_is_saturated() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let state = AppState::initialize(config(&temp)).await.unwrap();
    let app = build_router(state.clone());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();

    let recent = app
        .clone()
        .oneshot(get_with_cookie("/api/study/recent", &cookie))
        .await
        .unwrap();
    assert_eq!(recent.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_private_no_store(&recent);
    assert_eq!(json_body(recent).await["error"], "server busy");

    let position = app
        .oneshot(post_json(
            "/api/study/position",
            &cookie,
            r#"{"media_id":"m1","position_seconds":5}"#,
        ))
        .await
        .unwrap();
    assert_eq!(position.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_private_no_store(&position);
}

#[tokio::test]
async fn record_position_round_trips_and_appears_in_recent() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let recorded = app
        .clone()
        .oneshot(post_json(
            "/api/study/position",
            &cookie,
            r#"{"media_id":"m1","position_seconds":42}"#,
        ))
        .await
        .unwrap();
    assert_eq!(recorded.status(), StatusCode::OK);
    assert_private_no_store(&recorded);
    let body = json_body(recorded).await;
    assert_eq!(body["media_id"], "m1");
    assert_eq!(body["position_seconds"], 42);
    assert_eq!(body["completed"], false);
    assert_eq!(body["recorded"], true);
    let updated_at = body["updated_at"].as_i64().expect("instante persistido");
    assert!(updated_at > 1_000, "o estado real usa o relógio");

    let read = app
        .clone()
        .oneshot(get_with_cookie("/api/study/progress?media_id=m1", &cookie))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    let read = json_body(read).await;
    assert_eq!(read["recorded"], true);
    assert_eq!(read["position_seconds"], 42);
    assert_eq!(read["completed"], false);
    assert_eq!(read["updated_at"], updated_at);

    let recent = app
        .oneshot(get_with_cookie("/api/study/recent", &cookie))
        .await
        .unwrap();
    assert_eq!(recent.status(), StatusCode::OK);
    let recent = json_body(recent).await;
    assert_eq!(recent["items"].as_array().unwrap().len(), 1);
    assert_eq!(recent["items"][0]["media_id"], "m1");
    assert_eq!(recent["items"][0]["position_seconds"], 42);
}

#[tokio::test]
async fn an_unknown_media_without_progress_is_absent_and_not_an_error() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let response = app
        .oneshot(get_with_cookie("/api/study/progress?media_id=m1", &cookie))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_private_no_store(&response);
    let body = json_body(response).await;
    assert_eq!(body["recorded"], false);
    assert_eq!(body["position_seconds"], 0);
    assert_eq!(body["completed"], false);
    assert!(body["updated_at"].is_null());
}

#[tokio::test]
async fn a_negative_position_is_rejected_and_not_persisted() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let rejected = app
        .clone()
        .oneshot(post_json(
            "/api/study/position",
            &cookie,
            r#"{"media_id":"m1","position_seconds":-1}"#,
        ))
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_private_no_store(&rejected);
    assert_eq!(json_body(rejected).await["error"], "invalid_position");

    let recent = app
        .oneshot(get_with_cookie("/api/study/recent", &cookie))
        .await
        .unwrap();
    let recent = json_body(recent).await;
    assert!(
        recent["items"].as_array().unwrap().is_empty(),
        "uma posição recusada não pode gravar estado"
    );
}

#[tokio::test]
async fn unknown_media_is_rejected_for_reads_and_writes() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let read = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/progress?media_id=missing",
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_body(read).await["error"], "media_not_found");

    let position = app
        .clone()
        .oneshot(post_json(
            "/api/study/position",
            &cookie,
            r#"{"media_id":"missing","position_seconds":5}"#,
        ))
        .await
        .unwrap();
    assert_eq!(position.status(), StatusCode::NOT_FOUND);
    assert_private_no_store(&position);

    let completed = app
        .oneshot(post_json(
            "/api/study/completed",
            &cookie,
            r#"{"media_id":"missing","completed":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(completed.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn completion_and_position_are_independent() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let completed = app
        .clone()
        .oneshot(post_json(
            "/api/study/completed",
            &cookie,
            r#"{"media_id":"m1","completed":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(completed.status(), StatusCode::OK);
    let completed = json_body(completed).await;
    assert_eq!(completed["completed"], true);
    assert_eq!(completed["position_seconds"], 0);

    let moved = app
        .clone()
        .oneshot(post_json(
            "/api/study/position",
            &cookie,
            r#"{"media_id":"m1","position_seconds":90}"#,
        ))
        .await
        .unwrap();
    let moved = json_body(moved).await;
    assert_eq!(moved["position_seconds"], 90);
    assert_eq!(moved["completed"], true, "avançar não desfaz a conclusão");

    let cleared = app
        .oneshot(post_json(
            "/api/study/completed",
            &cookie,
            r#"{"media_id":"m1","completed":false}"#,
        ))
        .await
        .unwrap();
    let cleared = json_body(cleared).await;
    assert_eq!(cleared["completed"], false);
    assert_eq!(cleared["position_seconds"], 90);
}

#[tokio::test]
async fn recent_is_ordered_and_respects_the_limit() {
    let temp = tempfile::tempdir().unwrap();
    seed(
        &temp,
        &["m1", "m2", "m3"],
        &[
            (ADMIN_ID, "m1", 30, false, 300),
            (ADMIN_ID, "m2", 20, true, 200),
            (ADMIN_ID, "m3", 10, false, 100),
        ],
    );
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let full = app
        .clone()
        .oneshot(get_with_cookie("/api/study/recent", &cookie))
        .await
        .unwrap();
    let full = json_body(full).await;
    let ids: Vec<_> = full["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["media_id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        ids,
        ["m1", "m2", "m3"],
        "ordem do mais recente ao mais antigo"
    );
    assert_eq!(full["items"][1]["completed"], true);
    assert_eq!(full["items"][1]["position_seconds"], 20);

    let limited = app
        .clone()
        .oneshot(get_with_cookie("/api/study/recent?limit=2", &cookie))
        .await
        .unwrap();
    let limited = json_body(limited).await;
    assert_eq!(limited["items"].as_array().unwrap().len(), 2);
    assert_eq!(limited["items"][0]["media_id"], "m1");
    assert_eq!(limited["items"][1]["media_id"], "m2");

    let empty = app
        .oneshot(get_with_cookie("/api/study/recent?limit=0", &cookie))
        .await
        .unwrap();
    assert!(
        json_body(empty).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

/// A playlist precisa das conclusões do curso inteiro, e o teto de recentes não cobre um
/// curso grande: a rota por curso devolve só as conclusões daquela biblioteca, sem limite.
#[tokio::test]
async fn course_progress_lists_only_completed_lessons_of_the_course() {
    let temp = tempfile::tempdir().unwrap();
    seed(
        &temp,
        &["m1", "m2", "m3"],
        &[
            (ADMIN_ID, "m1", 30, true, 300),
            (ADMIN_ID, "m2", 20, false, 200),
            (ADMIN_ID, "m3", 10, true, 100),
        ],
    );
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let response = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/courses/library-1/progress",
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_private_no_store(&response);
    let body = json_body(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "só as conclusões do curso aparecem");
    assert_eq!(
        items
            .iter()
            .map(|item| item["media_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["m1", "m3"],
        "da alteração mais recente para a mais antiga"
    );
    assert!(items.iter().all(|item| item["completed"] == true));
    assert_eq!(items[0]["recorded"], true);

    let unknown = app
        .oneshot(get_with_cookie(
            "/api/study/courses/curso-sem-progresso/progress",
            &cookie,
        ))
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::OK);
    assert!(
        json_body(unknown).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn an_empty_course_id_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let response = app
        .oneshot(get_with_cookie("/api/study/courses/%20/progress", &cookie))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_private_no_store(&response);
    assert_eq!(json_body(response).await["error"], "invalid_query");
}

#[tokio::test]
async fn malformed_queries_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let bad_limit = app
        .clone()
        .oneshot(get_with_cookie("/api/study/recent?limit=abc", &cookie))
        .await
        .unwrap();
    assert_eq!(bad_limit.status(), StatusCode::BAD_REQUEST);
    assert_private_no_store(&bad_limit);
    assert_eq!(json_body(bad_limit).await["error"], "invalid_query");

    let negative_limit = app
        .clone()
        .oneshot(get_with_cookie("/api/study/recent?limit=-1", &cookie))
        .await
        .unwrap();
    assert_eq!(negative_limit.status(), StatusCode::BAD_REQUEST);

    let missing_media = app
        .oneshot(get_with_cookie("/api/study/progress", &cookie))
        .await
        .unwrap();
    assert_eq!(missing_media.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(missing_media).await["error"], "invalid_query");
}

#[tokio::test]
async fn mutations_require_the_same_origin() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let cross_origin = app
        .clone()
        .oneshot(
            Request::post("/api/study/position")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"media_id":"m1","position_seconds":5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_origin.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&cross_origin);
    assert_eq!(json_body(cross_origin).await["error"], "csrf_failed");

    let recent = app
        .oneshot(get_with_cookie("/api/study/recent", &cookie))
        .await
        .unwrap();
    assert!(
        json_body(recent).await["items"]
            .as_array()
            .unwrap()
            .is_empty(),
        "uma mutação cross-origin não pode gravar estado"
    );
}

#[tokio::test]
async fn progress_is_isolated_between_authenticated_users() {
    let temp = tempfile::tempdir().unwrap();
    seed_user(&temp, READER_ID, READER, READER_PASSWORD);
    seed(
        &temp,
        &["m1"],
        &[
            (ADMIN_ID, "m1", 42, true, 300),
            (READER_ID, "m1", 7, false, 200),
        ],
    );
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let admin_cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let admin_read = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/progress?media_id=m1",
            &admin_cookie,
        ))
        .await
        .unwrap();
    let admin_read = json_body(admin_read).await;
    assert_eq!(admin_read["position_seconds"], 42);
    assert_eq!(admin_read["completed"], true);

    let reader_cookie = login(&app, READER, READER_PASSWORD).await;
    let reader_read = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/study/progress?media_id=m1",
            &reader_cookie,
        ))
        .await
        .unwrap();
    let reader_read = json_body(reader_read).await;
    assert_eq!(
        reader_read["position_seconds"], 7,
        "cada sessão vê apenas o próprio progresso"
    );
    assert_eq!(reader_read["completed"], false);

    // A escrita de um dono não pode tocar a linha do outro.
    let written = app
        .clone()
        .oneshot(post_json(
            "/api/study/position",
            &reader_cookie,
            r#"{"media_id":"m1","position_seconds":99}"#,
        ))
        .await
        .unwrap();
    assert_eq!(written.status(), StatusCode::OK);

    let admin_after = app
        .oneshot(get_with_cookie(
            "/api/study/progress?media_id=m1",
            &admin_cookie,
        ))
        .await
        .unwrap();
    let admin_after = json_body(admin_after).await;
    assert_eq!(
        admin_after["position_seconds"], 42,
        "a escrita do segundo dono não altera o primeiro"
    );
    assert_eq!(admin_after["completed"], true);
}

#[tokio::test]
async fn progress_persists_across_a_server_restart() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);

    let first_state = AppState::initialize(config(&temp)).await.unwrap();
    let first_app = build_router(first_state.clone());
    let cookie = login(&first_app, ADMIN, ADMIN_PASSWORD).await;

    let recorded = first_app
        .clone()
        .oneshot(post_json(
            "/api/study/position",
            &cookie,
            r#"{"media_id":"m1","position_seconds":55}"#,
        ))
        .await
        .unwrap();
    assert_eq!(recorded.status(), StatusCode::OK);
    let completed = first_app
        .clone()
        .oneshot(post_json(
            "/api/study/completed",
            &cookie,
            r#"{"media_id":"m1","completed":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(completed.status(), StatusCode::OK);
    drop(first_app);
    drop(first_state);

    // Reinício equivalente: um novo processo abriria o mesmo arquivo SQLite.
    let second_state = AppState::initialize(config(&temp)).await.unwrap();
    let second_app = build_router(second_state);
    let cookie = login(&second_app, ADMIN, ADMIN_PASSWORD).await;

    let read = second_app
        .oneshot(get_with_cookie("/api/study/progress?media_id=m1", &cookie))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    let read = json_body(read).await;
    assert_eq!(read["recorded"], true, "o progresso sobrevive ao reinício");
    assert_eq!(read["position_seconds"], 55);
    assert_eq!(read["completed"], true);
}

#[tokio::test]
async fn credentials_pending_flag_blocks_study_until_changed() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let mut setup = config(&temp);
    setup.admin_username = None;
    setup.admin_password = None;
    {
        AppState::initialize(setup).await.unwrap();
    }
    // O bootstrap não marca mais a troca pendente; o gate segue valendo para
    // usuários legados importados com `must_change_credentials = 1`.
    {
        let database = learnvault_core::Database::open(database_path(&temp)).unwrap();
        database
            .connection()
            .execute("UPDATE user SET must_change_credentials = 1", [])
            .unwrap();
    }
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());

    let cookie = login(&app, "admin", "admin").await;
    let me = app
        .clone()
        .oneshot(get_with_cookie("/api/me", &cookie))
        .await
        .unwrap();
    assert_eq!(json_body(me).await["must_change_credentials"], true);

    let read = app
        .clone()
        .oneshot(get_with_cookie("/api/study/recent", &cookie))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&read);
    assert_eq!(
        json_body(read).await["error"],
        "credentials_change_required"
    );

    // O endpoint de capacidades segue a mesma fronteira de sessão: a sessão temporária
    // precisa trocar as credenciais antes de qualquer leitura do Study.
    let capabilities = app
        .clone()
        .oneshot(get_with_cookie("/api/study/capabilities", &cookie))
        .await
        .unwrap();
    assert_eq!(capabilities.status(), StatusCode::FORBIDDEN);
    assert_private_no_store(&capabilities);
    assert_eq!(
        json_body(capabilities).await["error"],
        "credentials_change_required"
    );
}

/// Uma base persistida já sai da cadeia de migrações na versão 6 e **já contém**
/// `study_state`: a tabela nasce na migração V1 (a primeira da cadeia) e a migração V6,
/// que reconstrói `study_note` para trocar o limite de bytes por caracteres, não a
/// remove. Este teste prova isso no arquivo real (não em memória) e verifica as
/// restrições que o adaptador HTTP depende.
#[tokio::test]
async fn an_existing_database_already_carries_study_state_and_its_constraints() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);

    let db = Database::open(database_path(&temp)).unwrap();
    let version: i64 = db
        .connection()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 6, "o formato persistido permanece na versão 6");
    let present: i64 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'study_state'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        present, 1,
        "a versão 4 já contém study_state; nenhuma migração nova é necessária"
    );

    // Chave estrangeira: uma mídia inexistente é recusada e nada é gravado.
    assert!(
        db.connection()
            .execute(
                "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
                 VALUES ('usr-admin', 'mid-inexistente', 0, 0, 1)",
                [],
            )
            .is_err(),
        "a chave estrangeira para media deve recusar mídia inexistente"
    );
    // Chave primária composta (owner_id, media_id): o par duplicado é recusado.
    db.connection()
        .execute_batch(
            "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
             VALUES ('usr-admin', 'm1', 0, 0, 1);",
        )
        .unwrap();
    assert!(
        db.connection()
            .execute(
                "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
                 VALUES ('usr-admin', 'm1', 0, 0, 2)",
                [],
            )
            .is_err(),
        "o par dono/mídia deve ser único"
    );
    // CHECK: `completed` só aceita 0 ou 1.
    assert!(
        db.connection()
            .execute(
                "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
                 VALUES ('usr-outro', 'm1', 0, 2, 1)",
                [],
            )
            .is_err(),
        "completed só aceita 0 ou 1"
    );
    let rows: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM study_state", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1, "apenas a linha válida persiste");
    drop(db);

    // O servidor sobre uma base v4 existente lê e grava progresso pela API.
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;
    let written = app
        .clone()
        .oneshot(post_json(
            "/api/study/position",
            &cookie,
            r#"{"media_id":"m1","position_seconds":7}"#,
        ))
        .await
        .unwrap();
    assert_eq!(written.status(), StatusCode::OK);
    let read = app
        .oneshot(get_with_cookie("/api/study/progress?media_id=m1", &cookie))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(json_body(read).await["position_seconds"], 7);
}

/// Várias escritas concorrentes para o mesmo par dono/mídia precisam ser serializadas
/// sem duplicar a linha nem falhar. O servidor mantém uma única conexão SQLite atrás de
/// um `Mutex`, então a concorrência é resolvida antes do banco; o teste prova a
/// invariante observável (upsert único e respostas íntegras).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_progress_writes_are_serialized_without_corruption() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let mut tasks = Vec::new();
    for position in 1..=8 {
        let app = app.clone();
        let cookie = cookie.clone();
        tasks.push(tokio::spawn(async move {
            app.oneshot(post_json(
                "/api/study/position",
                &cookie,
                &format!(r#"{{"media_id":"m1","position_seconds":{position}}}"#),
            ))
            .await
            .unwrap()
            .status()
        }));
    }
    for completed in [true, false, true, false] {
        let app = app.clone();
        let cookie = cookie.clone();
        tasks.push(tokio::spawn(async move {
            app.oneshot(post_json(
                "/api/study/completed",
                &cookie,
                &format!(r#"{{"media_id":"m1","completed":{completed}}}"#),
            ))
            .await
            .unwrap()
            .status()
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap(), StatusCode::OK);
    }

    let recent = app
        .oneshot(get_with_cookie("/api/study/recent", &cookie))
        .await
        .unwrap();
    let recent = json_body(recent).await;
    let items = recent["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "o upsert não pode duplicar a linha dono/mídia"
    );
    let position = items[0]["position_seconds"].as_i64().unwrap();
    assert!(
        (1..=8).contains(&position),
        "a posição final precisa ser uma das escritas concorrentes"
    );
}

/// Decisão explícita do adaptador: uma mídia removida do catálogo continua aceitando
/// leitura e escrita de progresso — a identidade sobrevive à remoção —, mas deixa de
/// aparecer em `/api/study/recent`, como no núcleo de remoção. Nenhuma política nova é
/// introduzida: o servidor apenas espelha a semântica existente.
#[tokio::test]
async fn removed_catalog_media_keeps_progress_but_leaves_recent() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp, &["m1"], &[]);
    Database::open(database_path(&temp))
        .unwrap()
        .connection()
        .execute_batch(
            "INSERT INTO media_removal (library_id, media_id, removed_at)
             VALUES ('library-1', 'm1', 1760000000);",
        )
        .unwrap();
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, ADMIN, ADMIN_PASSWORD).await;

    let absent = app
        .clone()
        .oneshot(get_with_cookie("/api/study/progress?media_id=m1", &cookie))
        .await
        .unwrap();
    assert_eq!(absent.status(), StatusCode::OK);
    assert_eq!(json_body(absent).await["recorded"], false);

    let written = app
        .clone()
        .oneshot(post_json(
            "/api/study/position",
            &cookie,
            r#"{"media_id":"m1","position_seconds":42}"#,
        ))
        .await
        .unwrap();
    assert_eq!(written.status(), StatusCode::OK);
    let completed = app
        .clone()
        .oneshot(post_json(
            "/api/study/completed",
            &cookie,
            r#"{"media_id":"m1","completed":true}"#,
        ))
        .await
        .unwrap();
    assert_eq!(completed.status(), StatusCode::OK);

    let read = app
        .clone()
        .oneshot(get_with_cookie("/api/study/progress?media_id=m1", &cookie))
        .await
        .unwrap();
    let read = json_body(read).await;
    assert_eq!(read["recorded"], true);
    assert_eq!(read["position_seconds"], 42);
    assert_eq!(read["completed"], true);

    let recent = app
        .oneshot(get_with_cookie("/api/study/recent", &cookie))
        .await
        .unwrap();
    assert!(
        json_body(recent).await["items"]
            .as_array()
            .unwrap()
            .is_empty(),
        "uma mídia removida sai dos recentes sem apagar o progresso"
    );
}
