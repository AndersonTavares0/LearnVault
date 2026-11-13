//! Authenticated per-user playback preferences over HTTP.
//!
//! The owner always comes from the session. The desktop build has no network and keeps
//! these values in the webview, so this adapter exists for the server build only.

use axum::{
    Json,
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use learnvault_core::PlayerPreference;
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    auth::same_origin,
    catalog::{csrf_failed, private_no_store},
    study::{run_study, study_failure_response, user_or_response},
};

#[derive(Debug, Deserialize)]
pub(crate) struct PlayerPreferenceRequest {
    video_speed: i64,
    audio_speed: i64,
    #[serde(default)]
    subtitle_language: String,
}

#[derive(Serialize)]
struct PlayerPreferenceResponse {
    video_speed: i64,
    audio_speed: i64,
    subtitle_language: String,
}

impl From<PlayerPreference> for PlayerPreferenceResponse {
    fn from(preference: PlayerPreference) -> Self {
        Self {
            video_speed: preference.video_speed,
            audio_speed: preference.audio_speed,
            subtitle_language: preference.subtitle_language,
        }
    }
}

/// Reads the authenticated owner's playback preferences.
pub(crate) async fn read(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::player_preference(db, &owner)
    })
    .await
    {
        Ok(preference) => {
            private_no_store(Json(PlayerPreferenceResponse::from(preference)).into_response())
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Replaces the authenticated owner's playback preferences.
pub(crate) async fn write(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PlayerPreferenceRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::set_player_preference(
            db,
            &owner,
            request.video_speed,
            request.audio_speed,
            &request.subtitle_language,
        )
    })
    .await
    {
        Ok(preference) => {
            private_no_store(Json(PlayerPreferenceResponse::from(preference)).into_response())
        }
        Err(failure) => study_failure_response(failure),
    }
}
