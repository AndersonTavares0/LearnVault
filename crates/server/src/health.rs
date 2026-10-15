use axum::{Json, extract::State};
use serde::Serialize;

use crate::{AppState, VERSION};

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

pub(crate) async fn healthz(State(_state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: VERSION,
    })
}
