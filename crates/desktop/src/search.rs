//! Global search over notes and catalog titles for the desktop.
//!
//! This adapter does not implement search: it fixes the local owner, refreshes the course
//! index with the registered course titles and delegates to [`learnvault_core::search`].
//! The owner is [`LOCAL_OWNER_ID`], resolved in Rust, so the webview can only find its own
//! notes. Course and lesson titles are shared metadata.
//!
//! The payload mirrors the HTTP shape field by field, so the SPA validates both transports
//! with the same normalizer.

use std::collections::HashMap;

use learnvault_core::{Database, SearchResults};

use crate::dto::{SearchCourseDto, SearchLessonDto, SearchNoteDto, SearchResultsDto};
use crate::error::IpcError;
use crate::study::LOCAL_OWNER_ID;

/// Results per group when the webview does not ask for a limit.
///
/// Mirrors the HTTP adapter; [`learnvault_core::MAX_SEARCH_RESULTS`] remains the ceiling.
pub const DEFAULT_SEARCH_LIMIT: usize = 10;

/// Searches notes and catalog titles of the local owner.
///
/// `courses` is the registry snapshot: it feeds the course index before each query, like
/// the server does. A missing or too-short query is a valid request with empty groups.
pub fn search(
    database: &Database,
    courses: &[(String, String)],
    query: &str,
    limit: Option<usize>,
) -> Result<SearchResultsDto, IpcError> {
    learnvault_core::sync_course_index(database, courses)
        .map_err(|_| IpcError::study_unavailable())?;
    let results = learnvault_core::search(
        database,
        LOCAL_OWNER_ID,
        true,
        query,
        limit.unwrap_or(DEFAULT_SEARCH_LIMIT),
    )
    .map_err(|_| IpcError::study_unavailable())?;
    let titles: HashMap<&str, &str> = courses
        .iter()
        .map(|(id, title)| (id.as_str(), title.as_str()))
        .collect();
    Ok(present(results, &titles))
}

/// Keeps only registered courses, so every result has a page to open.
fn present(results: SearchResults, titles: &HashMap<&str, &str>) -> SearchResultsDto {
    let notes = results
        .notes
        .into_iter()
        .filter_map(|hit| {
            Some(SearchNoteDto {
                course_title: titles.get(hit.course_id.as_str())?.to_string(),
                lesson_title: lesson_title(&hit.relative_path),
                note_id: hit.note_id,
                media_id: hit.media_id,
                course_id: hit.course_id,
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
            Some(SearchCourseDto {
                title: titles.get(course_id.as_str())?.to_string(),
                course_id,
            })
        })
        .collect();
    let lessons = results
        .lessons
        .into_iter()
        .filter_map(|hit| {
            Some(SearchLessonDto {
                course_title: titles.get(hit.course_id.as_str())?.to_string(),
                title: lesson_title(&hit.relative_path),
                course_id: hit.course_id,
                media_id: hit.media_id,
                path: hit.relative_path,
            })
        })
        .collect();
    SearchResultsDto {
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
