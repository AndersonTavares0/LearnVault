//! Testes HTTP das fontes remotas: configuração, segredo, teste de conexão, varredura,
//! catálogo e entrega de faixa de mídia contra um WebDAV sintético em processo.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use http_body_util::BodyExt;
use learnvault_server::{AppState, Config, build_router};
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tower::ServiceExt;

const PASSWORD: &str = "synthetic-secret";
const WRONG_PASSWORD: &str = "wrong-secret";
const AUTHORIZATION: &str = "Basic dXNlcjpzeW50aGV0aWMtc2VjcmV0";

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("remote.sqlite").to_str().unwrap(),
        "--no-update-check",
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

async fn json_body(response: Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

fn response(status: &str, headers: &[(&str, &str)], body: &str) -> Vec<u8> {
    let mut response = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    response.push_str(&format!("content-length: {}\r\n", body.len()));
    response.push_str("connection: close\r\n\r\n");
    response.push_str(body);
    response.into_bytes()
}

/// WebDAV sintético: uma pasta `Modulo1` com `aula.mp4` de 10 bytes.
async fn dav_server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut request = Vec::new();
                loop {
                    let mut chunk = [0_u8; 2048];
                    let Ok(read) = stream.read(&mut chunk).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    request.extend_from_slice(&chunk[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8_lossy(&request).into_owned();
                let lower = request.to_ascii_lowercase();
                let authorized = lower.contains(&format!(
                    "\r\nauthorization: {}\r\n",
                    AUTHORIZATION.to_ascii_lowercase()
                ));
                let payload = if !authorized {
                    response("401 Unauthorized", &[("www-authenticate", "Basic")], "")
                } else if lower.starts_with("propfind /dav/ ") {
                    let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
 <D:response><D:href>/dav/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>
 <D:response><D:href>/dav/Modulo1/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>
</D:multistatus>"#;
                    response(
                        "207 Multi-Status",
                        &[("content-type", "application/xml")],
                        xml,
                    )
                } else if lower.starts_with("propfind /dav/modulo1/ ") {
                    let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
 <D:response><D:href>/dav/Modulo1/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>
 <D:response><D:href>/dav/Modulo1/aula.mp4</D:href><D:propstat><D:prop><D:resourcetype/><D:getcontentlength>10</D:getcontentlength></D:prop></D:propstat></D:response>
</D:multistatus>"#;
                    response(
                        "207 Multi-Status",
                        &[("content-type", "application/xml")],
                        xml,
                    )
                } else if lower.starts_with("get /dav/modulo1/aula.mp4 ") {
                    if lower.contains("\r\nrange: bytes=2-5\r\n") {
                        response(
                            "206 Partial Content",
                            &[("content-range", "bytes 2-5/10")],
                            "2345",
                        )
                    } else {
                        response("200 OK", &[], "0123456789")
                    }
                } else {
                    response("404 Not Found", &[], "")
                };
                let _ = stream.write_all(&payload).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (format!("dav://{address}/dav"), task)
}

async fn create_source(app: &axum::Router, cookie: &str, url: &str, password: &str) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/admin/sources")
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "name": "Curso remoto",
                        "url": url,
                        "username": "user",
                        "password": password,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn remote_source_flow_keeps_the_secret_out_of_responses_and_streams_ranges() {
    let temp = tempfile::tempdir().unwrap();
    let (url, server) = dav_server().await;
    let state = AppState::initialize(config(&temp)).await.unwrap();
    let app = build_router(state);
    let cookie = login(&app).await;

    let created = create_source(&app, &cookie, &url, PASSWORD).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = created.into_body().collect().await.unwrap().to_bytes();
    let created_text = String::from_utf8_lossy(&created_body);
    assert!(!created_text.contains(PASSWORD));
    assert!(!created_text.contains("secret"));
    let created: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
    assert_eq!(created["name"], "Curso remoto");
    assert_eq!(created["username"], "user");
    assert_eq!(created["has_password"], true);
    assert_eq!(created["credential_error"], false);
    let id = created["id"].as_str().unwrap().to_owned();

    let listed = app
        .clone()
        .oneshot(
            Request::get("/api/admin/sources")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_text =
        String::from_utf8_lossy(&listed.into_body().collect().await.unwrap().to_bytes())
            .into_owned();
    assert!(!listed_text.contains(PASSWORD));
    assert!(!listed_text.contains("\"secret\""));
    assert!(listed_text.contains("\"has_password\":true"));

    let tested = app
        .clone()
        .oneshot(
            Request::post(format!("/api/admin/sources/{id}/test"))
                .header("origin", "http://localhost")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tested.status(), StatusCode::OK);
    let tested = json_body(tested).await;
    assert_eq!(tested["ok"], true);
    assert_eq!(tested["entries"], 1);

    let scanned = app
        .clone()
        .oneshot(
            Request::post(format!("/api/admin/sources/{id}/scan"))
                .header("origin", "http://localhost")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(scanned.status(), StatusCode::OK);
    let scanned = json_body(scanned).await;
    assert_eq!(scanned["inserted"], 1);
    assert_eq!(scanned["complete"], true);

    let catalog = app
        .clone()
        .oneshot(
            Request::get("/api/catalog")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog = json_body(catalog).await;
    let course = catalog["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["course_id"] == id.as_str())
        .expect("a fonte remota precisa aparecer no catálogo");
    assert_eq!(course["title"], "Curso remoto");
    assert!(!course["children"].as_array().unwrap().is_empty());

    let media = app
        .clone()
        .oneshot(
            Request::get(format!("/api/media?course={id}&path=Modulo1%2Faula.mp4"))
                .header("cookie", cookie.clone())
                .header("range", "bytes=2-5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(media.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(media.headers()["content-range"], "bytes 2-5/10");
    assert_eq!(
        media.into_body().collect().await.unwrap().to_bytes(),
        "2345"
    );

    let wrong = create_source(&app, &cookie, &url, WRONG_PASSWORD).await;
    assert_eq!(wrong.status(), StatusCode::CREATED);
    let wrong = json_body(wrong).await;
    let wrong_id = wrong["id"].as_str().unwrap();
    let tested = app
        .clone()
        .oneshot(
            Request::post(format!("/api/admin/sources/{wrong_id}/test"))
                .header("origin", "http://localhost")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let tested_text =
        String::from_utf8_lossy(&tested.into_body().collect().await.unwrap().to_bytes())
            .into_owned();
    assert!(tested_text.contains("\"ok\":false"));
    assert!(tested_text.contains("denied"));
    assert!(!tested_text.contains(WRONG_PASSWORD));

    let deleted = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/admin/sources/{id}"))
                .header("origin", "http://localhost")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::OK);
    assert_eq!(json_body(deleted).await["removed"], true);

    server.abort();
}

#[tokio::test]
async fn smb_source_urls_are_accepted_and_never_echo_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(config(&temp)).await.unwrap();
    let app = build_router(state);
    let cookie = login(&app).await;

    let created = create_source(&app, &cookie, "smb://nas/Cursos/Modulo%201", PASSWORD).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = created.into_body().collect().await.unwrap().to_bytes();
    let created_text = String::from_utf8_lossy(&created_body);
    assert!(!created_text.contains(PASSWORD));
    assert!(!created_text.contains("secret"));
    let created: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
    assert_eq!(created["url"], "smb://nas/Cursos/Modulo%201");
    assert_eq!(created["has_password"], true);
    assert_eq!(created["credential_error"], false);

    let listed = app
        .clone()
        .oneshot(
            Request::get("/api/admin/sources")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_text =
        String::from_utf8_lossy(&listed.into_body().collect().await.unwrap().to_bytes())
            .into_owned();
    assert!(!listed_text.contains(PASSWORD));
    assert!(listed_text.contains("\"has_password\":true"));

    for (url, expected) in [
        ("smb://nas", "invalid_source_url"),
        ("smb://nas/Cursos/../fora", "invalid_source_url"),
        (
            "smb://user:synthetic-secret@nas/Cursos",
            "source_url_contains_credentials",
        ),
    ] {
        let refused = create_source(&app, &cookie, url, PASSWORD).await;
        assert_eq!(refused.status(), StatusCode::BAD_REQUEST, "{url}");
        let body =
            String::from_utf8_lossy(&refused.into_body().collect().await.unwrap().to_bytes())
                .into_owned();
        assert!(body.contains(expected), "{url}: {body}");
        assert!(!body.contains(PASSWORD), "{url}");
        assert!(!body.contains("synthetic-secret"), "{url}");
    }
}

#[tokio::test]
async fn invalid_source_urls_are_refused_without_echoing_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(config(&temp)).await.unwrap();
    let app = build_router(state);
    let cookie = login(&app).await;

    let refused = create_source(
        &app,
        &cookie,
        "https://user:synthetic-secret@example.test/dav",
        PASSWORD,
    )
    .await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let body = String::from_utf8_lossy(&refused.into_body().collect().await.unwrap().to_bytes())
        .into_owned();
    assert!(body.contains("source_url_contains_credentials"));
    assert!(!body.contains("synthetic-secret"));
}

#[tokio::test]
async fn link_local_targets_are_refused_without_echoing_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(config(&temp)).await.unwrap();
    let app = build_router(state);
    let cookie = login(&app).await;

    let refused = create_source(&app, &cookie, "dav://169.254.169.254/dav", PASSWORD).await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let body = String::from_utf8_lossy(&refused.into_body().collect().await.unwrap().to_bytes())
        .into_owned();
    assert!(body.contains("unsafe_source_target"));
    assert!(!body.contains(PASSWORD));
}

#[tokio::test]
async fn sources_are_admin_only() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(config(&temp)).await.unwrap();
    let app = build_router(state);

    let response = app
        .oneshot(
            Request::get("/api/admin/sources")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store")
    );
}
