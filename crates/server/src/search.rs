//! Global search over notes and catalog titles.
//!
//! The route is authenticated: the owner is always the session user, so notes can only be
//! found by the person who wrote them. Course titles come from the server registry and are
//! refreshed in the FTS index before each query; lesson titles come from the catalog paths.
//! The query is bound as a parameter by [`learnvault_core::search`]; this adapter only
//! validates the HTTP shape and adds presentation metadata.

use std::collections::HashMap;

use axum::{
    Json,
    extract::{RawQuery, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use learnvault_core::SearchResults;
use serde::{Deserialize, Serialize};

use crate::study::{run_study, study_failure_response, user_or_response};

/// Results per group when the client does not ask for a limit.
const DEFAULT_SEARCH_LIMIT: usize = 10;

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchResponse {
    notes: Vec<NoteResult>,
    courses: Vec<CourseResult>,
    lessons: Vec<LessonResult>,
}

#[derive(Serialize)]
struct NoteResult {
    note_id: String,
    media_id: String,
    course_id: String,
    course_title: String,
    lesson_path: String,
    lesson_title: String,
    position_seconds: i64,
    updated_at: i64,
    /// Matched fragment; `<mark>` and `</mark>` wrap the highlighted terms.
    snippet: String,
}

#[derive(Serialize)]
struct CourseResult {
    course_id: String,
    title: String,
}

#[derive(Serialize)]
struct LessonResult {
    course_id: String,
    course_title: String,
    media_id: String,
    path: String,
    title: String,
}

/// `GET /api/search?q=&limit=` — notes of the session user plus shared course/lesson titles.
///
/// A missing or too-short query is a valid request with empty groups, not an error.
pub(crate) async fn search(
    State(state): State<crate::AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) =
        serde_urlencoded::from_str::<SearchQuery>(raw.as_deref().unwrap_or_default()).ok()
    else {
        return crate::catalog::private_no_store(
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "invalid_query" })),
            )
                .into_response(),
        );
    };
    let limit = query.limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
    let text = query.q.unwrap_or_default();
    let owner = user.user_id;
    let admin = user.is_admin;

    let registered = state.courses_snapshot();
    let titles: HashMap<String, String> = registered
        .iter()
        .map(|course| (course.id.clone(), course.title.clone()))
        .collect();
    let index: Vec<(String, String)> = registered
        .iter()
        .map(|course| (course.id.clone(), course.title.clone()))
        .collect();

    let outcome = run_study(&state, move |db| {
        learnvault_core::sync_course_index(db, &index)?;
        learnvault_core::search(db, &owner, admin, &text, limit)
    })
    .await;

    match outcome {
        Ok(results) => {
            crate::catalog::private_no_store(Json(present(results, &titles)).into_response())
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Keeps only registered courses, so a result always has a page to open.
fn present(results: SearchResults, titles: &HashMap<String, String>) -> SearchResponse {
    let notes = results
        .notes
        .into_iter()
        .filter_map(|hit| {
            let course_title = titles.get(&hit.course_id)?.clone();
            Some(NoteResult {
                note_id: hit.note_id,
                media_id: hit.media_id,
                course_id: hit.course_id,
                course_title,
                lesson_title: lesson_title(&hit.relative_path),
                lesson_path: hit.relative_path,
                position_seconds: hit.position_seconds,
                updated_at: hit.updated_at,
                snippet: hit.snippet,
            })
        })
        .collect();
    let courses = results
        .course_ids
        .into_iter()
        .filter_map(|course_id| {
            let title = titles.get(&course_id)?.clone();
            Some(CourseResult { course_id, title })
        })
        .collect();
    let lessons = results
        .lessons
        .into_iter()
        .filter_map(|hit| {
            let course_title = titles.get(&hit.course_id)?.clone();
            Some(LessonResult {
                course_id: hit.course_id,
                course_title,
                media_id: hit.media_id,
                title: lesson_title(&hit.relative_path),
                path: hit.relative_path,
            })
        })
        .collect();
    SearchResponse {
        notes,
        courses,
        lessons,
    }
}

/// Lesson title from its relative path: file name without extension, cleaned like the catalog.
fn lesson_title(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    learnvault_core::catalog::clean_node_title(stem)
}

#[cfg(test)]
mod tests {
    use super::lesson_title;

    #[test]
    fn lesson_titles_drop_the_extension_and_number_prefix() {
        assert_eq!(lesson_title("Module/01 - Ownership.mp4"), "Ownership");
        assert_eq!(lesson_title("Just A File.mkv"), "Just A File");
        assert_eq!(lesson_title("2024"), "2024");
    }
}
