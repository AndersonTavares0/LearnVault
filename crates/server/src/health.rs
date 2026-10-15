use axum::{
    Json,
    extract::State,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::{AppState, VERSION};

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

pub(crate) async fn healthz(State(_state): State<AppState>) -> Response {
    let mut response = Json(HealthResponse {
        status: "ok",
        version: VERSION,
    })
    .into_response();
    // A saúde é pública e sem dados sensíveis, mas nunca deve ser armazenada:
    // um 200 ou 503 em cache poderia mascarar uma indisponibilidade já resolvida.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
