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

const PASSWORD: &str = "senha local muito segura";

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("notes.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some("admin".into());
    config.admin_password = Some(PASSWORD.into());
    config.cookie_secure = false;
    config
}

fn seed(temp: &TempDir) {
    let db = Database::open(temp.path().join("notes.sqlite")).unwrap();
    db.connection()
        .execute(
            "INSERT INTO library (library_id, available) VALUES ('library-1', 1)",
            [],
        )
        .unwrap();
    db.connection().execute("INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability) VALUES ('m1', 'library-1', 'a.mp4', ?1, 1, 1, 'available')", [learnvault_core::paths::relative_path_key("a.mp4", KeyMode::for_host()).unwrap()]).unwrap();
    let hash = hash_password("senha do leitor local").unwrap();
    db.connection().execute("INSERT INTO user (user_id, username, password_hash, is_admin, created_at, must_change_credentials) VALUES ('reader', 'reader', ?1, 0, 1, 0)", [hash]).unwrap();
}

async fn login(app: &axum::Router, username: &str, password: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"username": username, "password": password}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

fn json_request(
    method: &str,
    uri: &str,
    cookie: &str,
    body: Option<serde_json::Value>,
) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("cookie", cookie)
        .header("origin", "http://localhost");
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    request
        .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
        .unwrap()
}

async fn payload(response: Response) -> serde_json::Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn http_notes_crud_is_authenticated_owner_scoped_csrf_protected_and_persistent() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin = login(&app, "admin", PASSWORD).await;
    let reader = login(&app, "reader", "senha do leitor local").await;

    let unauthorized = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes?media_id=m1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let created = app.clone().oneshot(json_request("POST", "/api/study/notes", &admin, Some(serde_json::json!({"media_id":"m1","text":"<em>texto simples</em>","position_seconds":37})))).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let note = payload(created).await;
    assert_eq!(note["text"], "<em>texto simples</em>");
    assert_eq!(note["position_seconds"], 37);
    let id = note["note_id"].as_i64().unwrap();

    let reader_list = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes?media_id=m1")
                .header("cookie", &reader)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        payload(reader_list).await["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let stolen = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/api/study/notes/{id}"),
            &reader,
            Some(serde_json::json!({"text":"stolen","position_seconds":null})),
        ))
        .await
        .unwrap();
    assert_eq!(stolen.status(), StatusCode::NOT_FOUND);

    let edited = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/api/study/notes/{id}"),
            &admin,
            Some(serde_json::json!({"text":"edited","position_seconds":null})),
        ))
        .await
        .unwrap();
    assert_eq!(payload(edited).await["text"], "edited");
    let invalid = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/study/notes",
            &admin,
            Some(serde_json::json!({"media_id":"m1","text":"  ","position_seconds":null})),
        ))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_position = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/study/notes",
            &admin,
            Some(serde_json::json!({"media_id":"m1","text":"negativa","position_seconds":-1})),
        ))
        .await
        .unwrap();
    assert_eq!(invalid_position.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let boundary = app.clone().oneshot(json_request("POST", "/api/study/notes", &admin, Some(serde_json::json!({"media_id":"m1","text":"x".repeat(20_000),"position_seconds":null})))).await.unwrap();
    assert_eq!(boundary.status(), StatusCode::CREATED);
    let boundary_id = payload(boundary).await["note_id"].as_i64().unwrap();
    let too_long = app.clone().oneshot(json_request("POST", "/api/study/notes", &admin, Some(serde_json::json!({"media_id":"m1","text":"x".repeat(20_001),"position_seconds":null})))).await.unwrap();
    assert_eq!(too_long.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let boundary_delete = app
        .clone()
        .oneshot(json_request(
            "DELETE",
            &format!("/api/study/notes/{boundary_id}"),
            &admin,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(boundary_delete.status(), StatusCode::NO_CONTENT);
    let supplied_owner = app.clone().oneshot(json_request("POST", "/api/study/notes", &admin, Some(serde_json::json!({"owner_id":"reader","media_id":"m1","text":"spoof","position_seconds":null})))).await.unwrap();
    // O corpo rejeita o campo desconhecido: o axum classifica "unknown field" como erro
    // de dados (422), não de sintaxe (400). O spoof não cria nota nenhuma — a contagem
    // final do teste prova isso.
    assert_eq!(supplied_owner.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let no_origin = app
        .clone()
        .oneshot(
            Request::post("/api/study/notes")
                .header("cookie", &admin)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"m1","text":"csrf","position_seconds":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_origin.status(), StatusCode::FORBIDDEN);

    drop(app);
    let reopened = Database::open(temp.path().join("notes.sqlite")).unwrap();
    let notes = learnvault_core::list_notes(&reopened, "usr-admin", "m1").unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text, "edited");
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin = login(&app, "admin", PASSWORD).await;
    let deleted = app
        .oneshot(json_request(
            "DELETE",
            &format!("/api/study/notes/{id}"),
            &admin,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn http_notes_reject_missing_credentials_invalid_input_and_oversized_bodies() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin = login(&app, "admin", PASSWORD).await;

    // 401: mutação sem token — a sessão é exigida em toda escrita.
    let no_token = app
        .clone()
        .oneshot(
            Request::post("/api/study/notes")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"media_id":"m1","text":"sem token","position_seconds":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_token.status(), StatusCode::UNAUTHORIZED);

    // 400: query inválida — `media_id` é obrigatório na listagem.
    let invalid_query = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes")
                .header("cookie", &admin)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_query.status(), StatusCode::BAD_REQUEST);
    assert_eq!(payload(invalid_query).await["error"], "invalid_query");

    // 404: mídia e nota inexistentes são distinguidas por código, não por texto livre.
    let unknown_media = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/study/notes",
            &admin,
            Some(serde_json::json!({
                "media_id": "m-inexistente",
                "text": "nota",
                "position_seconds": null
            })),
        ))
        .await
        .unwrap();
    assert_eq!(unknown_media.status(), StatusCode::NOT_FOUND);
    assert_eq!(payload(unknown_media).await["error"], "media_not_found");

    let unknown_note = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/api/study/notes/999",
            &admin,
            Some(serde_json::json!({"text": "nota", "position_seconds": null})),
        ))
        .await
        .unwrap();
    assert_eq!(unknown_note.status(), StatusCode::NOT_FOUND);
    assert_eq!(payload(unknown_note).await["error"], "note_not_found");

    // 413: corpo acima do limite da rota (128 KiB).
    let oversized = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/study/notes",
            &admin,
            Some(serde_json::json!({
                "media_id": "m1",
                "text": "x".repeat(200_000),
                "position_seconds": null
            })),
        ))
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

    // A rota responde private e no-store: a nota não pode ficar em cache compartilhado.
    let list = app
        .clone()
        .oneshot(
            Request::get("/api/study/notes?media_id=m1")
                .header("cookie", &admin)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    assert_eq!(list.headers()["cache-control"], "private, no-store");
}

#[tokio::test]
async fn http_notes_enforce_csrf_on_every_mutation() {
    let temp = tempfile::tempdir().unwrap();
    seed(&temp);
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let admin = login(&app, "admin", PASSWORD).await;

    let created = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/study/notes",
            &admin,
            Some(serde_json::json!({
                "media_id": "m1",
                "text": "nota",
                "position_seconds": null
            })),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = payload(created).await["note_id"].as_i64().unwrap();

    // A mesma origem é exigida em toda escrita: sem `origin`, PUT e DELETE são recusados
    // com 403 e a nota continua intacta.
    for (method, uri, body) in [
        (
            "PUT",
            format!("/api/study/notes/{id}"),
            Some(r#"{"text":"csrf","position_seconds":null}"#.to_owned()),
        ),
        ("DELETE", format!("/api/study/notes/{id}"), None),
    ] {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header("cookie", &admin)
            .header("content-type", "application/json");
        if body.is_none() {
            request = request.header("content-type", "application/json");
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from(body.unwrap_or_default())).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(payload(response).await["error"], "csrf_failed");
    }
}
