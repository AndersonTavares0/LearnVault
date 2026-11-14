//! HTTP tests of the course roots of a remote source (ADR 0004): browsing one folder level,
//! choosing courses, and scans split by those roots, against an in-process synthetic WebDAV.

use std::collections::HashMap;

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

/// Synthetic tree: `None` is a folder, `Some(size)` a file.
fn tree() -> HashMap<&'static str, Vec<(&'static str, Option<u64>)>> {
    HashMap::from([
        ("", vec![("Cursos", None), ("Solto.mp4", Some(10))]),
        (
            "Cursos",
            vec![
                ("Pentest", None),
                ("Infra", None),
                ("temp", None),
                ("leia.txt", Some(5)),
            ],
        ),
        ("Cursos/Pentest", vec![("Redes", None), ("Web", None)]),
        (
            "Cursos/Pentest/Redes",
            vec![
                ("aula1.mp4", Some(10)),
                ("aula2.mp4", Some(10)),
                ("Sub", None),
            ],
        ),
        ("Cursos/Pentest/Redes/Sub", vec![("extra.mp4", Some(10))]),
        ("Cursos/Pentest/Web", vec![("aula.mp4", Some(10))]),
        ("Cursos/Infra", vec![("Linux", None)]),
        ("Cursos/Infra/Linux", vec![("aula.mp4", Some(10))]),
        ("Cursos/temp", vec![("escondido.mp4", Some(10))]),
    ])
}

fn xml_for(dir: &str, children: &[(&str, Option<u64>)]) -> String {
    let base = if dir.is_empty() {
        "/dav/".to_owned()
    } else {
        format!("/dav/{dir}/")
    };
    let mut xml = String::from(r#"<?xml version="1.0"?><D:multistatus xmlns:D="DAV:">"#);
    xml.push_str(&format!(
        "<D:response><D:href>{base}</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>"
    ));
    for (name, size) in children {
        match size {
            None => xml.push_str(&format!(
                "<D:response><D:href>{base}{name}/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>"
            )),
            Some(size) => xml.push_str(&format!(
                "<D:response><D:href>{base}{name}</D:href><D:propstat><D:prop><D:resourcetype/><D:getcontentlength>{size}</D:getcontentlength></D:prop></D:propstat></D:response>"
            )),
        }
    }
    xml.push_str("</D:multistatus>");
    xml
}

type Tree = HashMap<&'static str, Vec<(&'static str, Option<u64>)>>;

async fn dav_server() -> (String, tokio::task::JoinHandle<()>) {
    dav_server_with(tree()).await
}

async fn dav_server_with(tree: Tree) -> (String, tokio::task::JoinHandle<()>) {
    let tree = std::sync::Arc::new(tree);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let tree = std::sync::Arc::clone(&tree);
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
                let text = String::from_utf8_lossy(&request).into_owned();
                let target = text.split_whitespace().nth(1).unwrap_or("").to_owned();
                let dir = target
                    .strip_prefix("/dav")
                    .unwrap_or("")
                    .trim_matches('/')
                    .to_owned();
                let lower = text.to_ascii_lowercase();
                let is_file = |dir: &str| {
                    let (parent, name) = dir.rsplit_once('/').unwrap_or(("", dir));
                    tree.get(parent).is_some_and(|children| {
                        children
                            .iter()
                            .any(|(n, size)| *n == name && size.is_some())
                    })
                };
                let payload = match tree.get(dir.as_str()) {
                    Some(children) if text.starts_with("PROPFIND") => {
                        let xml = xml_for(&dir, children);
                        format!(
                            "HTTP/1.1 207 Multi-Status\r\ncontent-type: application/xml\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{xml}",
                            xml.len()
                        )
                    }
                    _ if text.starts_with("GET") && is_file(&dir) => {
                        // The client always asks for a range; answer it like a real server.
                        let range = lower
                            .lines()
                            .find_map(|line| line.strip_prefix("range: bytes="))
                            .and_then(|spec| spec.split_once('-'))
                            .and_then(|(from, to)| {
                                Some((
                                    from.trim().parse::<usize>().ok()?,
                                    to.trim().parse::<usize>().ok()?,
                                ))
                            });
                        let (from, to) = range.unwrap_or((0, 9));
                        let to = to.min(9);
                        let body = &"0123456789"[from..=to];
                        format!(
                            "HTTP/1.1 206 Partial Content\r\ncontent-range: bytes {from}-{to}/10\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    }
                    _ => "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                        .to_owned(),
                };
                let _ = stream.write_all(payload.as_bytes()).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (format!("dav://{address}/dav"), task)
}

fn config(temp: &TempDir) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("roots.sqlite").to_str().unwrap(),
        "--no-update-check",
    ])
    .unwrap();
    config.admin_username = Some("admin".to_owned());
    config.admin_password = Some("uma senha local muito segura".to_owned());
    config.cookie_secure = false;
    config
}

struct Harness {
    app: axum::Router,
    cookie: String,
    source: String,
    temp: TempDir,
    _server: tokio::task::JoinHandle<()>,
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
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned()
}

async fn json(response: Response) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null)
}

async fn harness() -> Harness {
    harness_with(tree()).await
}

async fn harness_with(tree: Tree) -> Harness {
    let temp = tempfile::tempdir().unwrap();
    let (url, server) = dav_server_with(tree).await;
    let app = build_router(AppState::initialize(config(&temp)).await.unwrap());
    let cookie = login(&app, "admin", "uma senha local muito segura").await;
    let created = app
        .clone()
        .oneshot(
            Request::post("/api/admin/sources")
                .header("origin", "http://localhost")
                .header("cookie", cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"name": "Cursos remotos", "url": url}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let source = json(created).await["id"].as_str().unwrap().to_owned();
    Harness {
        app,
        cookie,
        source,
        temp,
        _server: server,
    }
}

impl Harness {
    async fn call(
        &self,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
        origin: bool,
    ) -> Response {
        self.call_as(&self.cookie, method, uri, body, origin).await
    }

    async fn call_as(
        &self,
        cookie: &str,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
        origin: bool,
    ) -> Response {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header("cookie", cookie);
        if origin {
            request = request.header("origin", "http://localhost");
        }
        let body = match body {
            Some(value) => {
                request = request.header("content-type", "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        self.app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap()
    }

    async fn student(&self, name: &str, allowed: &[&str]) -> String {
        let created = self
            .call(
                "POST",
                "/api/admin/users",
                Some(serde_json::json!({
                    "username": name,
                    "password": "uma senha de aluno bem longa",
                    "allowed_course_ids": allowed,
                })),
                true,
            )
            .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        login(&self.app, name, "uma senha de aluno bem longa").await
    }

    async fn catalog(&self, cookie: &str) -> Vec<serde_json::Value> {
        let response = self
            .call_as(cookie, "GET", "/api/catalog", None, false)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        json(response).await["nodes"].as_array().unwrap().clone()
    }

    async fn media(
        &self,
        cookie: &str,
        course: &str,
        path: &str,
        range: Option<&str>,
    ) -> (StatusCode, String) {
        let mut request = Request::get(format!("/api/media?course={course}&path={path}"))
            .header("cookie", cookie);
        if let Some(range) = range {
            request = request.header("range", range);
        }
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    async fn browse(&self, path: &str) -> Response {
        self.call(
            "GET",
            &format!("/api/admin/sources/{}/browse?path={path}", self.source),
            None,
            false,
        )
        .await
    }

    async fn pick(&self, path: &str, mode: &str) -> Response {
        self.call(
            "POST",
            &format!("/api/admin/sources/{}/roots", self.source),
            Some(serde_json::json!({"path": path, "mode": mode})),
            true,
        )
        .await
    }

    async fn scan(&self) -> Response {
        self.call(
            "POST",
            &format!("/api/admin/sources/{}/scan", self.source),
            None,
            true,
        )
        .await
    }

    fn library_of(&self, path: &str) -> Option<String> {
        let db = learnvault_core::Database::open(self.temp.path().join("roots.sqlite")).unwrap();
        db.connection()
            .query_row(
                "SELECT library_id FROM media WHERE relative_path = ?1",
                [path],
                |row| row.get(0),
            )
            .ok()
    }
}

#[tokio::test]
async fn browsing_lists_one_level_with_counts_and_hides_ignored_folders() {
    let h = harness().await;
    let root = json(h.browse("").await).await;
    assert_eq!(root["path"], "");
    assert_eq!(root["folders"].as_array().unwrap().len(), 1);
    assert_eq!(root["folders"][0]["name"], "Cursos");
    assert_eq!(root["media_files"], 1);

    let cursos = json(h.browse("Cursos").await).await;
    let names: Vec<_> = cursos["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["Infra", "Pentest"],
        "sorted, and `temp` is never listed"
    );
    assert_eq!(cursos["other_files"], 1);
    assert_eq!(cursos["folders"][1]["path"], "Cursos/Pentest");
}

#[tokio::test]
async fn browse_refuses_unclean_paths_and_unknown_folders() {
    let h = harness().await;
    for bad in ["..", "a/../b", "%2Fabs", "a//b", "C:/x", "%00"] {
        assert_eq!(
            h.browse(bad).await.status(),
            StatusCode::BAD_REQUEST,
            "{bad}"
        );
    }
    assert_eq!(
        h.browse(&"a/".repeat(600)).await.status(),
        StatusCode::BAD_REQUEST
    );
    let missing = h.browse("Cursos/NaoExiste").await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(json(missing).await["error"], "not_found");
}

#[tokio::test]
async fn the_endpoints_are_administrator_only_and_need_the_origin_to_change_things() {
    let h = harness().await;
    let anonymous = Request::get(format!("/api/admin/sources/{}/browse", h.source))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        h.app.clone().oneshot(anonymous).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    // A student is refused everywhere.
    let created = h
        .call(
            "POST",
            "/api/admin/users",
            Some(
                serde_json::json!({"username": "ana", "password": "uma senha de aluna bem longa"}),
            ),
            true,
        )
        .await;
    assert_eq!(
        created.status(),
        StatusCode::CREATED,
        "the student account must exist for this check"
    );
    let student = login(&h.app, "ana", "uma senha de aluna bem longa").await;
    for (method, uri) in [
        ("GET", format!("/api/admin/sources/{}/browse", h.source)),
        ("GET", format!("/api/admin/sources/{}/roots", h.source)),
        ("POST", format!("/api/admin/sources/{}/roots", h.source)),
        ("DELETE", format!("/api/admin/sources/{}/roots/x", h.source)),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("cookie", student.clone())
            .header("origin", "http://localhost")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"path":"Cursos","mode":"course"}"#))
            .unwrap();
        assert_eq!(
            h.app.clone().oneshot(request).await.unwrap().status(),
            StatusCode::FORBIDDEN,
            "{method}"
        );
    }

    let without_origin = h
        .call(
            "POST",
            &format!("/api/admin/sources/{}/roots", h.source),
            Some(serde_json::json!({"path": "Cursos", "mode": "course"})),
            false,
        )
        .await;
    assert_eq!(without_origin.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        h.call("GET", "/api/admin/sources/nope/browse", None, false)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn choosing_a_course_checks_the_folder_and_rejects_overlaps() {
    let h = harness().await;
    let created = h.pick("Cursos/Pentest/Redes", "course").await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = json(created).await;
    assert_eq!(body["created"][0]["name"], "Redes");
    assert_eq!(body["created"][0]["path"], "Cursos/Pentest/Redes");
    assert!(!body["created"][0]["id"].as_str().unwrap().is_empty());

    // The same folder, a folder inside it and a folder containing it all overlap.
    for path in [
        "Cursos/Pentest/Redes",
        "Cursos/Pentest",
        "Cursos/Pentest/Redes/Sub",
    ] {
        let conflict = h.pick(path, "course").await;
        assert_eq!(conflict.status(), StatusCode::CONFLICT, "{path}");
        assert_eq!(json(conflict).await["error"], "overlaps_existing_root");
    }
    assert_eq!(
        h.pick("Cursos/NaoExiste", "course").await.status(),
        StatusCode::NOT_FOUND
    );
    for bad in ["", "..", "/abs", "a//b", "C:/x"] {
        assert_eq!(
            h.pick(bad, "course").await.status(),
            StatusCode::BAD_REQUEST,
            "{bad:?}"
        );
    }
    let listed = json(
        h.call(
            "GET",
            &format!("/api/admin/sources/{}/roots", h.source),
            None,
            false,
        )
        .await,
    )
    .await;
    assert_eq!(listed["roots"].as_array().unwrap().len(), 1);

    // Browsing now shows the state of each folder.
    let pentest = json(h.browse("Cursos/Pentest").await).await;
    let states: Vec<_> = pentest["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["name"].as_str().unwrap(), f["state"].as_str().unwrap()))
        .collect();
    assert_eq!(states, [("Redes", "root"), ("Web", "free")]);
    assert_eq!(
        json(h.browse("Cursos").await).await["folders"][1]["state"],
        "contains_root"
    );
}

#[tokio::test]
async fn choosing_every_folder_inside_creates_a_root_for_each_and_skips_the_rest() {
    let h = harness().await;
    assert_eq!(
        h.pick("Cursos/Pentest/Redes", "course").await.status(),
        StatusCode::CREATED
    );

    let response = h.pick("Cursos/Pentest", "children").await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json(response).await;
    let created: Vec<_> = body["created"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(created, ["Web"]);
    assert_eq!(body["skipped"][0]["path"], "Cursos/Pentest/Redes");
    assert_eq!(body["skipped"][0]["reason"], "overlaps_existing_root");

    // Every folder directly under the source root, with nothing skipped but the existing ones.
    let all = json(h.pick("Cursos", "children").await).await;
    assert!(
        all["created"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["path"] == "Cursos/Infra")
    );
    assert!(
        all["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["path"] == "Cursos/Pentest")
    );
}

#[tokio::test]
async fn a_scan_after_choosing_a_course_files_each_media_under_its_own_library() {
    let h = harness().await;
    assert_eq!(h.scan().await.status(), StatusCode::OK);
    assert_eq!(
        h.library_of("Cursos/Pentest/Redes/aula1.mp4").as_deref(),
        Some(h.source.as_str())
    );

    let created = json(h.pick("Cursos/Pentest/Redes", "course").await).await;
    let redes = created["created"][0]["id"].as_str().unwrap().to_owned();
    // Existing media moved at once, before any new scan.
    assert_eq!(
        h.library_of("Cursos/Pentest/Redes/aula1.mp4").as_deref(),
        Some(redes.as_str())
    );

    let scan = h.scan().await;
    assert_eq!(scan.status(), StatusCode::OK);
    let summary = json(scan).await;
    assert_eq!(
        summary["marked_missing"], 0,
        "nothing may look missing after the split"
    );
    assert_eq!(
        h.library_of("Cursos/Pentest/Redes/aula2.mp4").as_deref(),
        Some(redes.as_str())
    );
    assert_eq!(
        h.library_of("Cursos/Pentest/Web/aula.mp4").as_deref(),
        Some(h.source.as_str())
    );
    assert_eq!(
        h.library_of("Solto.mp4").as_deref(),
        Some(h.source.as_str())
    );
    assert_eq!(
        h.library_of("Cursos/temp/escondido.mp4"),
        None,
        "ignored folders are never scanned"
    );
}

#[tokio::test]
async fn choosing_a_course_before_the_first_scan_gets_filled_by_that_scan() {
    let h = harness().await;
    let created = json(h.pick("Cursos/Infra/Linux", "course").await).await;
    let linux = created["created"][0]["id"].as_str().unwrap().to_owned();
    assert_eq!(h.scan().await.status(), StatusCode::OK);
    assert_eq!(
        h.library_of("Cursos/Infra/Linux/aula.mp4").as_deref(),
        Some(linux.as_str())
    );
}

#[tokio::test]
async fn removing_a_course_returns_its_media_to_the_source_and_only_works_through_its_source() {
    let h = harness().await;
    h.scan().await;
    let id = json(h.pick("Cursos/Pentest/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let wrong = h
        .call(
            "DELETE",
            &format!("/api/admin/sources/{}/roots/not-a-root", h.source),
            None,
            true,
        )
        .await;
    assert_eq!(wrong.status(), StatusCode::NOT_FOUND);
    let no_origin = h
        .call(
            "DELETE",
            &format!("/api/admin/sources/{}/roots/{id}", h.source),
            None,
            false,
        )
        .await;
    assert_eq!(no_origin.status(), StatusCode::FORBIDDEN);

    let removed = h
        .call(
            "DELETE",
            &format!("/api/admin/sources/{}/roots/{id}", h.source),
            None,
            true,
        )
        .await;
    assert_eq!(removed.status(), StatusCode::OK);
    assert_eq!(
        h.library_of("Cursos/Pentest/Redes/aula1.mp4").as_deref(),
        Some(h.source.as_str())
    );
    let listed = json(
        h.call(
            "GET",
            &format!("/api/admin/sources/{}/roots", h.source),
            None,
            false,
        )
        .await,
    )
    .await;
    assert!(listed["roots"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn an_ignored_folder_cannot_be_chosen_as_a_course() {
    let h = harness().await;
    let refused = h.pick("Cursos/temp", "course").await;
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(refused).await["error"], "invalid_path");
}

#[tokio::test]
async fn a_root_cannot_be_removed_through_another_source() {
    let h = harness().await;
    let id = json(h.pick("Cursos/Pentest/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (url, _server) = dav_server().await;
    let other = json(
        h.call(
            "POST",
            "/api/admin/sources",
            Some(serde_json::json!({"name": "Outra", "url": url})),
            true,
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let wrong = h
        .call(
            "DELETE",
            &format!("/api/admin/sources/{other}/roots/{id}"),
            None,
            true,
        )
        .await;
    assert_eq!(wrong.status(), StatusCode::NOT_FOUND);
    let still = json(
        h.call(
            "GET",
            &format!("/api/admin/sources/{}/roots", h.source),
            None,
            false,
        )
        .await,
    )
    .await;
    assert_eq!(still["roots"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_failed_scan_is_recorded_for_the_source_and_every_root() {
    let h = harness().await;
    h.scan().await;
    let id = json(h.pick("Cursos/Pentest/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    h._server.abort();

    let failed = h.scan().await;
    assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(json(failed).await["error"], "unreachable");

    let db = learnvault_core::Database::open(h.temp.path().join("roots.sqlite")).unwrap();
    for library in [id.as_str(), h.source.as_str()] {
        let status: String = db
            .connection()
            .query_row(
                "SELECT status FROM scan WHERE library_id = ?1 ORDER BY scan_id DESC LIMIT 1",
                [library],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "failed", "{library}");
    }
}

#[tokio::test]
async fn removing_a_source_detaches_its_roots_and_keeps_every_media_row() {
    let h = harness().await;
    h.scan().await;
    h.pick("Cursos/Pentest/Redes", "course").await;
    h.pick("Cursos/Infra/Linux", "course").await;

    let removed = h
        .call(
            "DELETE",
            &format!("/api/admin/sources/{}", h.source),
            None,
            true,
        )
        .await;
    assert_eq!(removed.status(), StatusCode::OK);

    let db = learnvault_core::Database::open(h.temp.path().join("roots.sqlite")).unwrap();
    let roots: i64 = db
        .connection()
        .query_row("SELECT COUNT(*) FROM library_root", [], |row| row.get(0))
        .unwrap();
    assert_eq!(roots, 0);
    assert_eq!(
        h.library_of("Cursos/Pentest/Redes/aula1.mp4").as_deref(),
        Some(h.source.as_str())
    );
    assert_eq!(
        h.library_of("Cursos/Infra/Linux/aula.mp4").as_deref(),
        Some(h.source.as_str())
    );
}

fn titles(nodes: &[serde_json::Value]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| node["title"].as_str().unwrap().to_owned())
        .collect()
}

fn files_of(node: &serde_json::Value) -> Vec<String> {
    let mut paths: Vec<String> = ["files", "loose_files"]
        .iter()
        .flat_map(|key| node[*key].as_array().into_iter().flatten())
        .map(|file| file["path"].as_str().unwrap().to_owned())
        .collect();
    for child in node["children"].as_array().into_iter().flatten() {
        paths.extend(files_of(child));
    }
    paths.sort();
    paths
}

#[tokio::test]
async fn the_catalog_shows_one_course_per_root_and_keeps_the_rest_of_the_source() {
    let h = harness().await;
    h.scan().await;
    let redes = json(h.pick("Cursos/Pentest/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let linux = json(h.pick("Cursos/Infra/Linux", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    h.scan().await;

    let nodes = h.catalog(&h.cookie).await;
    assert_eq!(titles(&nodes), ["Linux", "Redes", "Cursos remotos"]);
    assert_eq!(nodes[0]["course_id"], linux.as_str());
    assert_eq!(nodes[1]["course_id"], redes.as_str());
    assert_eq!(nodes[2]["course_id"], h.source.as_str());
    // Each node carries only its own lessons, with paths relative to the source.
    assert_eq!(
        files_of(&nodes[1]),
        [
            "Cursos/Pentest/Redes/Sub/extra.mp4",
            "Cursos/Pentest/Redes/aula1.mp4",
            "Cursos/Pentest/Redes/aula2.mp4"
        ]
    );
    assert_eq!(files_of(&nodes[0]), ["Cursos/Infra/Linux/aula.mp4"]);
    assert_eq!(
        files_of(&nodes[2]),
        [
            "Cursos/Pentest/Web/aula.mp4",
            "Cursos/leia.txt",
            "Solto.mp4"
        ]
    );
}

#[tokio::test]
async fn a_split_source_with_nothing_left_shows_only_its_courses_and_tells_equal_names_apart() {
    let tree: Tree = HashMap::from([
        ("", vec![("A", None), ("B", None), ("Solo", None)]),
        ("A", vec![("Redes", None)]),
        ("A/Redes", vec![("1.mp4", Some(10))]),
        ("B", vec![("Redes", None)]),
        ("B/Redes", vec![("1.mp4", Some(10))]),
        ("Solo", vec![("1.mp4", Some(10))]),
    ]);
    let h = harness_with(tree).await;
    h.scan().await;
    for path in ["A/Redes", "B/Redes", "Solo"] {
        assert_eq!(h.pick(path, "course").await.status(), StatusCode::CREATED);
    }
    h.scan().await;
    assert_eq!(
        titles(&h.catalog(&h.cookie).await),
        ["Redes (A)", "Redes (B)", "Solo"]
    );

    // A source without roots is still one course, even before its first scan.
    let plain = harness().await;
    assert_eq!(
        titles(&plain.catalog(&plain.cookie).await),
        ["Cursos remotos"]
    );
}

#[tokio::test]
async fn access_is_decided_by_the_id_of_each_course() {
    let h = harness().await;
    let whole = h.student("ana", &[h.source.as_str()]).await;
    h.scan().await;
    let redes = json(h.pick("Cursos/Pentest/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    h.scan().await;

    // A student who had the source receives the new course and keeps the rest of the source.
    assert_eq!(
        titles(&h.catalog(&whole).await),
        ["Redes", "Cursos remotos"]
    );
    // A student granted only the root sees only that course, never the rest of the source.
    let only_root = h.student("bia", &[redes.as_str()]).await;
    assert_eq!(titles(&h.catalog(&only_root).await), ["Redes"]);
    // A student granted something else sees neither.
    let other = h.student("cia", &["unrelated"]).await;
    assert!(h.catalog(&other).await.is_empty());
}

#[tokio::test]
async fn media_of_a_root_streams_through_its_source_and_stays_inside_its_folder() {
    let h = harness().await;
    h.scan().await;
    let redes = json(h.pick("Cursos/Pentest/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    h.scan().await;
    let inside = "Cursos/Pentest/Redes/aula1.mp4";

    assert_eq!(
        h.media(&h.cookie, &redes, inside, None).await,
        (StatusCode::OK, "0123456789".to_owned())
    );
    assert_eq!(
        h.media(&h.cookie, &redes, inside, Some("bytes=2-5")).await,
        (StatusCode::PARTIAL_CONTENT, "2345".to_owned())
    );
    // A file of the source outside the root's folder is not reachable through the root, and a
    // file of the root is not reachable through the source.
    assert_eq!(
        h.media(&h.cookie, &redes, "Cursos/Pentest/Web/aula.mp4", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        h.media(&h.cookie, &redes, "Solto.mp4", None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        h.media(&h.cookie, &h.source, inside, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        h.media(&h.cookie, &h.source, "Cursos/Pentest/Web/aula.mp4", None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        h.media(&h.cookie, "unknown-id", inside, None).await.0,
        StatusCode::NOT_FOUND
    );

    // A student granted only the root can read it and nothing else of the source.
    let only_root = h.student("bia", &[redes.as_str()]).await;
    assert_eq!(
        h.media(&only_root, &redes, inside, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        h.media(&only_root, &h.source, "Cursos/Pentest/Web/aula.mp4", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let other = h.student("cia", &["unrelated"]).await;
    assert_eq!(
        h.media(&other, &redes, inside, None).await.0,
        StatusCode::NOT_FOUND
    );

    // Removing the root sends its lessons back to the source course.
    h.call(
        "DELETE",
        &format!("/api/admin/sources/{}/roots/{redes}", h.source),
        None,
        true,
    )
    .await;
    assert_eq!(
        h.media(&h.cookie, &h.source, inside, None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        h.media(&h.cookie, &redes, inside, None).await.0,
        StatusCode::NOT_FOUND
    );
}

fn all_titles(node: &serde_json::Value) -> Vec<String> {
    let mut titles = vec![node["title"].as_str().unwrap().to_owned()];
    for child in node["children"].as_array().into_iter().flatten() {
        titles.extend(all_titles(child));
    }
    titles
}

#[tokio::test]
async fn a_root_starts_at_its_own_folder_and_never_shows_the_folders_above_it() {
    let h = harness().await;
    h.scan().await;
    let redes = json(h.pick("Cursos/Pentest/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    h.scan().await;

    let nodes = h.catalog(&h.cookie).await;
    let node = nodes
        .iter()
        .find(|node| node["course_id"] == redes.as_str())
        .unwrap();
    let below = all_titles(node);
    assert!(
        below
            .iter()
            .all(|title| title != "Cursos" && title != "Pentest"),
        "{below:?}"
    );
    assert_eq!(below, ["Redes", "aula1", "aula2", "Sub", "extra"]);
    // Files keep their path relative to the source, which is what media requests use.
    assert_eq!(
        files_of(node),
        [
            "Cursos/Pentest/Redes/Sub/extra.mp4",
            "Cursos/Pentest/Redes/aula1.mp4",
            "Cursos/Pentest/Redes/aula2.mp4"
        ]
    );
    // The student granted only this root sees the same tree, with no ancestor folder.
    let only_root = h.student("bia", &[redes.as_str()]).await;
    let seen = h.catalog(&only_root).await;
    assert_eq!(
        all_titles(&seen[0]),
        ["Redes", "aula1", "aula2", "Sub", "extra"]
    );
}

#[tokio::test]
async fn an_unreadable_area_outside_every_root_keeps_the_catalog_incomplete_even_when_the_rest_is_hidden()
 {
    let tree: Tree = HashMap::from([
        ("", vec![("Curso", None), ("Quebrada", None)]),
        ("Curso", vec![("aula.mp4", Some(10))]),
    ]);
    let h = harness_with(tree).await;
    assert_eq!(
        h.pick("Curso", "course").await.status(),
        StatusCode::CREATED
    );
    assert_eq!(h.scan().await.status(), StatusCode::OK);

    let response = h.call("GET", "/api/catalog", None, false).await;
    let catalog = json(response).await;
    assert_eq!(
        titles(catalog["nodes"].as_array().unwrap()),
        ["Curso"],
        "the rest has nothing to show"
    );
    assert_eq!(catalog["complete"], false);
}

#[tokio::test]
async fn titles_are_told_apart_only_among_the_roots_a_user_can_see() {
    let tree: Tree = HashMap::from([
        ("", vec![("A", None), ("B", None)]),
        ("A", vec![("Redes", None)]),
        ("A/Redes", vec![("1.mp4", Some(10))]),
        ("B", vec![("Redes", None)]),
        ("B/Redes", vec![("1.mp4", Some(10))]),
    ]);
    let h = harness_with(tree).await;
    h.scan().await;
    let a = json(h.pick("A/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    json(h.pick("B/Redes", "course").await).await;
    h.scan().await;

    assert_eq!(
        titles(&h.catalog(&h.cookie).await),
        ["Redes (A)", "Redes (B)"]
    );
    let only_a = h.student("bia", &[a.as_str()]).await;
    assert_eq!(
        titles(&h.catalog(&only_a).await),
        ["Redes"],
        "no hint that another Redes exists"
    );
}

#[tokio::test]
async fn a_user_granted_the_source_after_the_split_does_not_receive_the_roots() {
    // Documented behavior: only choosing a root copies access; later grants are by id.
    let h = harness().await;
    h.scan().await;
    let redes = json(h.pick("Cursos/Pentest/Redes", "course").await).await["created"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    h.scan().await;
    let late = h.student("ana", &[h.source.as_str()]).await;
    assert_eq!(titles(&h.catalog(&late).await), ["Cursos remotos"]);
    assert_eq!(
        h.media(&late, &redes, "Cursos/Pentest/Redes/aula1.mp4", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
