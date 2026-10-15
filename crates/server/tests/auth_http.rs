use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use learnvault_server::{AppState, Config, build_router};
use tempfile::TempDir;
use tower::ServiceExt;

fn config(temp: &TempDir, with_initial_admin: bool) -> Config {
    let mut config = Config::from_args([
        "learnvault-server",
        "--listen-addr",
        "127.0.0.1:18090",
        "--database",
        temp.path().join("learnvault.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    if with_initial_admin {
        config.admin_username = Some("admin".to_owned());
        config.admin_password = Some("uma senha local muito segura".to_owned());
    }
    config.cookie_secure = false;
    config
}

async fn body(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn login(app: &axum::Router, username: &str, password: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("content-type", "application/json")
                .header("origin", "http://localhost")
                .body(Body::from(
                    serde_json::json!({"username": username, "password": password}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .unwrap_or_default()
        .to_owned();
    (status, cookie)
}

#[tokio::test]
async fn login_me_and_logout_use_an_expiring_secure_cookie() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(config(&temp, true)).await.unwrap();
    let app = build_router(state);

    let login = app
        .clone()
        .oneshot(
            Request::post("/api/login")
                .header("content-type", "application/json")
                .header("origin", "http://localhost")
                .body(Body::from(
                    r#"{"username":"admin","password":"uma senha local muito segura"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(login.status(), StatusCode::OK);
    assert_eq!(login.headers()["cache-control"], "no-store");
    assert_eq!(login.headers()["x-content-type-options"], "nosniff");
    assert_eq!(login.headers()["x-frame-options"], "DENY");
    assert_eq!(login.headers()["referrer-policy"], "no-referrer");
    let csp = login.headers()["content-security-policy"].to_str().unwrap();
    assert!(csp.contains("frame-ancestors"));
    assert!(csp.contains("script-src 'self'; style-src"));
    let cookie = login.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(cookie.starts_with("learnvault_session="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    assert!(cookie.contains("Max-Age=43200"));
    assert!(!cookie.contains("Secure"));
    assert_eq!(body(login).await["username"], "admin");

    let me = app
        .clone()
        .oneshot(
            Request::get("/api/me")
                .header("cookie", cookie.split(';').next().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    assert_eq!(me.headers()["cache-control"], "private, no-store");
    assert_eq!(body(me).await["is_admin"], true);

    let logout = app
        .clone()
        .oneshot(
            Request::post("/api/logout")
                .header("origin", "http://localhost")
                .header("cookie", cookie.split(';').next().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    assert_eq!(logout.headers()["cache-control"], "no-store");
    assert!(
        logout.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );

    let after_logout = app
        .oneshot(
            Request::get("/api/me")
                .header("cookie", cookie.split(';').next().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after_logout.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(after_logout.headers()["cache-control"], "private, no-store");
}

#[tokio::test]
async fn me_never_caches_unauthorized_responses() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp, true)).await.unwrap());

    for request in [
        Request::get("/api/me").body(Body::empty()).unwrap(),
        Request::get("/api/me")
            .header("cookie", "learnvault_session=unknown")
            .body(Body::empty())
            .unwrap(),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()["cache-control"], "private, no-store");
    }
}

#[tokio::test]
async fn invalid_credentials_have_a_uniform_unauthorized_response() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp, true)).await.unwrap());

    for username in ["admin", "unknown"] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/login")
                    .header("content-type", "application/json")
                    .header("origin", "http://localhost")
                    .body(Body::from(format!(
                        r#"{{"username":"{username}","password":"senha errada e longa"}}"#
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(body(response).await["error"], "invalid credentials");
    }
}

#[tokio::test]
async fn login_rejects_when_blocking_capacity_is_saturated() {
    let temp = tempfile::tempdir().unwrap();
    let state = AppState::initialize(config(&temp, true)).await.unwrap();
    let app = build_router(state.clone());
    let permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();
    assert_eq!(permits.len(), 4);

    let response = app
        .oneshot(
            Request::post("/api/login")
                .header("content-type", "application/json")
                .header("origin", "http://localhost")
                .body(Body::from(
                    r#"{"username":"admin","password":"uma senha local muito segura"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body(response).await,
        serde_json::json!({ "error": "server busy" })
    );
}

#[tokio::test]
async fn startup_without_credentials_creates_temporary_admin() {
    let temp = tempfile::tempdir().unwrap();

    let app = build_router(AppState::initialize(config(&temp, false)).await.unwrap());
    let login = app
        .oneshot(
            Request::post("/api/login")
                .header("content-type", "application/json")
                .header("origin", "http://localhost")
                .body(Body::from(r#"{"username":"admin","password":"admin"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(login.status(), StatusCode::OK);
    assert_eq!(body(login).await["must_change_credentials"], true);
}

#[tokio::test]
async fn login_is_rate_limited_and_secure_is_default() {
    let temp = tempfile::tempdir().unwrap();
    let mut secure_config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("secure.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    secure_config.admin_username = Some("admin".to_owned());
    secure_config.admin_password = Some("uma senha local muito segura".to_owned());
    let app = build_router(AppState::initialize(secure_config).await.unwrap());

    let mut last = None;
    for _ in 0..6 {
        last = Some(
            app.clone()
                .oneshot(
                    Request::post("/api/login")
                        .header("origin", "http://localhost")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            r#"{"username":"admin","password":"senha errada e longa"}"#,
                        ))
                        .unwrap(),
                )
                .await
                .unwrap(),
        );
    }
    assert_eq!(last.unwrap().status(), StatusCode::TOO_MANY_REQUESTS);

    let temp = tempfile::tempdir().unwrap();
    let app = build_router(
        AppState::initialize({
            let mut config = config(&temp, true);
            config.cookie_secure = true;
            config
        })
        .await
        .unwrap(),
    );
    let response = app
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
    assert!(
        response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Secure")
    );
}

#[tokio::test]
async fn startup_creates_missing_database_parent_and_rejects_oversized_login() {
    let temp = tempfile::tempdir().unwrap();
    let mut config = Config::from_args([
        "learnvault-server",
        "--database",
        temp.path().join("nested/data.sqlite").to_str().unwrap(),
        "--library-root",
        temp.path().to_str().unwrap(),
    ])
    .unwrap();
    config.admin_username = Some("admin".to_owned());
    config.admin_password = Some("uma senha local muito segura".to_owned());
    config.cookie_secure = false;
    let app = build_router(AppState::initialize(config).await.unwrap());

    let response = app
        .oneshot(
            Request::post("/api/login")
                .header("origin", "http://localhost")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"username":"admin","password":"{}"}}"#,
                    "x".repeat(10_000)
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn credentials_change_is_transactional_and_invalidates_old_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp, false)).await.unwrap());
    let (status, old_cookie) = login(&app, "admin", "admin").await;
    assert_eq!(status, StatusCode::OK);

    let invalid = app
        .clone()
        .oneshot(
            Request::post("/api/account/credentials")
                .header("content-type", "application/json")
                .header("origin", "http://localhost")
                .header("cookie", &old_cookie)
                .body(Body::from(
                    serde_json::json!({
                        "current_password": "senha atual incorreta",
                        "username": "owner",
                        "password": "uma senha nova muito segura"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(body(invalid).await["error"], "invalid credentials");

    let invalid_new = app
        .clone()
        .oneshot(
            Request::post("/api/account/credentials")
                .header("content-type", "application/json")
                .header("origin", "http://localhost")
                .header("cookie", &old_cookie)
                .body(Body::from(
                    serde_json::json!({
                        "current_password": "admin",
                        "username": "x",
                        "password": "curta"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_new.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let changed = app
        .clone()
        .oneshot(
            Request::post("/api/account/credentials")
                .header("content-type", "application/json")
                .header("origin", "http://localhost")
                .header("cookie", &old_cookie)
                .body(Body::from(
                    serde_json::json!({
                        "current_password": "admin",
                        "username": "owner",
                        "password": "uma senha nova muito segura"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::OK);
    let new_cookie = changed.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let old_session = app
        .clone()
        .oneshot(
            Request::get("/api/me")
                .header("cookie", &old_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_session.status(), StatusCode::UNAUTHORIZED);
    let new_session = app
        .oneshot(
            Request::get("/api/me")
                .header("cookie", new_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(new_session.status(), StatusCode::OK);
    assert_eq!(body(new_session).await["username"], "owner");
}

#[tokio::test]
async fn temporary_credentials_block_protected_endpoints_until_changed() {
    let temp = tempfile::tempdir().unwrap();
    let app = build_router(AppState::initialize(config(&temp, false)).await.unwrap());
    let (_, cookie) = login(&app, "admin", "admin").await;
    for mut request in [
        Request::get("/api/catalog").body(Body::empty()).unwrap(),
        Request::get("/api/media").body(Body::empty()).unwrap(),
        Request::post("/api/admin/catalog/rescan")
            .header("origin", "http://localhost")
            .body(Body::empty())
            .unwrap(),
    ] {
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(body(response).await["error"], "credentials_change_required");
    }
}
