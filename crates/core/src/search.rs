//! Global search over study notes and the catalog, backed by SQLite FTS5.
//!
//! Notes are private: every note query joins `study_note` on the authenticated owner.
//! Course and lesson titles are shared metadata. The index tables are maintained by the
//! triggers created in the migration, so create, update and delete stay correct without a
//! rebuild; course titles come from the server registry and are refreshed by
//! [`sync_course_index`] before a search.
//!
//! The caller never supplies FTS5 syntax: [`fts_query`] quotes every term as a prefix
//! query, so operators like `NEAR`, `*` or `-` are searched literally. The query is bound
//! as a SQL parameter, never interpolated.

use crate::db::Database;
use crate::error::Result;

/// Shortest accepted query in Unicode scalar values. Shorter queries return no results.
pub const MIN_SEARCH_QUERY_CHARS: usize = 2;

/// Longest accepted query in Unicode scalar values. Longer input is truncated before use.
pub const MAX_SEARCH_QUERY_CHARS: usize = 100;

/// Largest number of results returned by each group.
pub const MAX_SEARCH_RESULTS: usize = 50;

/// Most terms taken from one query. Extra terms are ignored.
const MAX_QUERY_TERMS: usize = 8;

/// A note matched by the query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchNoteHit {
    /// Identifier of the note.
    pub note_id: String,
    /// Media (lesson) the note belongs to.
    pub media_id: String,
    /// Course that owns the media.
    pub course_id: String,
    /// Relative path of the lesson, used to open it at the note timestamp.
    pub relative_path: String,
    /// Player position in seconds.
    pub position_seconds: i64,
    /// Last edit in seconds since the Unix epoch.
    pub updated_at: i64,
    /// Matched fragment with `<mark>`/`</mark>` around the terms.
    pub snippet: String,
}

/// A lesson whose path matched the query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchLessonHit {
    /// Media identity.
    pub media_id: String,
    /// Course that owns the media.
    pub course_id: String,
    /// Relative path inside the course.
    pub relative_path: String,
}

/// Grouped search results.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchResults {
    /// Notes of the current owner, best match first.
    pub notes: Vec<SearchNoteHit>,
    /// Course identities whose title matched, best match first.
    pub course_ids: Vec<String>,
    /// Lessons whose path matched, best match first.
    pub lessons: Vec<SearchLessonHit>,
}

/// Replaces the standalone course index with the current registered courses.
///
/// The registry is configuration, not database state, so this runs before a search with
/// the server's snapshot. The course list is small; deleting and reinserting keeps it
/// exact without stale rows.
pub fn sync_course_index(db: &Database, courses: &[(String, String)]) -> Result<()> {
    let conn = db.connection();
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM course_fts", [])?;
    {
        let mut statement =
            tx.prepare("INSERT INTO course_fts (course_id, title) VALUES (?1, ?2)")?;
        for (course_id, title) in courses {
            statement.execute(rusqlite::params![course_id, title])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Searches notes of one owner and the shared course/lesson titles.
///
/// `limit` is clamped to [`MAX_SEARCH_RESULTS`]. A query shorter than
/// [`MIN_SEARCH_QUERY_CHARS`] returns empty groups without touching SQLite.
pub fn search(
    db: &Database,
    owner_id: &str,
    is_admin: bool,
    query: &str,
    limit: usize,
) -> Result<SearchResults> {
    crate::study::validate_owner(owner_id)?;
    let limit = limit.min(MAX_SEARCH_RESULTS);
    let Some(match_query) = fts_query(query) else {
        return Ok(SearchResults::default());
    };
    if limit == 0 {
        return Ok(SearchResults::default());
    }

    let conn = db.connection();

    let mut notes = Vec::new();
    {
        let mut statement = conn.prepare(&format!(
            "SELECT n.note_id, n.media_id, m.library_id, m.relative_path,
                    n.position_seconds, n.updated_at,
                    snippet(study_note_fts, 0, '<mark>', '</mark>', '…', 16)
             FROM study_note_fts
             JOIN study_note n ON n.rowid = study_note_fts.rowid
             JOIN media m ON m.media_id = n.media_id
             WHERE study_note_fts MATCH ?1 AND n.owner_id = ?2
               AND {access_predicate}
             ORDER BY rank
             LIMIT ?4",
            access_predicate = access_predicate("m.library_id"),
        ))?;
        let rows = statement.query_map(
            rusqlite::params![match_query, owner_id, is_admin, limit as i64],
            |row| {
                Ok(SearchNoteHit {
                    note_id: row.get(0)?,
                    media_id: row.get(1)?,
                    course_id: row.get(2)?,
                    relative_path: row.get(3)?,
                    position_seconds: row.get(4)?,
                    updated_at: row.get(5)?,
                    snippet: row.get(6)?,
                })
            },
        )?;
        for row in rows {
            notes.push(row?);
        }
    }

    let mut lessons = Vec::new();
    {
        let mut statement = conn.prepare(&format!(
            "SELECT m.media_id, m.library_id, m.relative_path
             FROM media_fts
             JOIN media m ON m.rowid = media_fts.rowid
             WHERE media_fts MATCH ?1
               AND NOT EXISTS (SELECT 1
                                 FROM media_removal r
                                WHERE r.library_id = m.library_id
                                  AND r.media_id = m.media_id)
               AND {access_predicate}
             ORDER BY rank
             LIMIT ?4",
            access_predicate = access_predicate("m.library_id"),
        ))?;
        let rows = statement.query_map(
            rusqlite::params![match_query, owner_id, is_admin, limit as i64],
            |row| {
                Ok(SearchLessonHit {
                    media_id: row.get(0)?,
                    course_id: row.get(1)?,
                    relative_path: row.get(2)?,
                })
            },
        )?;
        for row in rows {
            lessons.push(row?);
        }
    }

    let mut course_ids = Vec::new();
    {
        let mut statement = conn.prepare(&format!(
            "SELECT course_id FROM course_fts
             WHERE course_fts MATCH ?1
               AND {access_predicate}
             ORDER BY rank
             LIMIT ?4",
            access_predicate = access_predicate("course_fts.course_id"),
        ))?;
        let rows = statement.query_map(
            rusqlite::params![match_query, owner_id, is_admin, limit as i64],
            |row| row.get::<_, String>(0),
        )?;
        for row in rows {
            course_ids.push(row?);
        }
    }

    Ok(SearchResults {
        notes,
        course_ids,
        lessons,
    })
}

/// SQL predicate that keeps only rows the caller may open.
///
/// A user without rows in `user_course_access` is unrestricted, which mirrors
/// [`crate::auth::can_user_access_course`]; an administrator always passes. Filtering
/// inside the statement matters: it must happen before `LIMIT`, otherwise a restricted
/// user loses matches that the truncated window already discarded.
fn access_predicate(course_column: &str) -> String {
    format!(
        "(?3 OR NOT EXISTS (SELECT 1 FROM user_course_access a WHERE a.user_id = ?2)
             OR EXISTS (SELECT 1 FROM user_course_access a
                         WHERE a.user_id = ?2 AND a.course_id = {course_column}))"
    )
}

/// Turns arbitrary input into a safe FTS5 prefix query, or `None` when nothing is left.
///
/// Every term is a quoted string, so FTS5 operators are literal text; embedded double
/// quotes are doubled as FTS5 requires. Diacritics are handled by the index tokenizer.
pub fn fts_query(query: &str) -> Option<String> {
    let trimmed: String = query.trim().chars().take(MAX_SEARCH_QUERY_CHARS).collect();
    if trimmed.chars().count() < MIN_SEARCH_QUERY_CHARS {
        return None;
    }
    let terms: Vec<String> = trimmed
        .split_whitespace()
        .filter(|term| term.chars().any(char::is_alphanumeric))
        .take(MAX_QUERY_TERMS)
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_QUERY_TERMS, fts_query};

    #[test]
    fn quoted_terms_neutralize_fts_operators() {
        assert_eq!(
            fts_query("rust ownership").as_deref(),
            Some("\"rust\"* AND \"ownership\"*")
        );
        assert_eq!(
            fts_query("NEAR(foo bar)").as_deref(),
            Some("\"NEAR(foo\"* AND \"bar)\"*")
        );
        assert_eq!(fts_query("a\"b").as_deref(), Some("\"a\"\"b\"*"));
        assert_eq!(fts_query("-*").as_deref(), None);
    }

    #[test]
    fn short_and_punctuation_only_queries_have_no_terms() {
        assert_eq!(fts_query(" a "), None);
        assert_eq!(fts_query("  "), None);
        assert_eq!(fts_query("！？"), None);
    }

    #[test]
    fn the_term_count_is_capped() {
        let query = (0..(MAX_QUERY_TERMS + 5))
            .map(|index| format!("term{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let built = fts_query(&query).unwrap();
        assert_eq!(built.matches(" AND ").count(), MAX_QUERY_TERMS - 1);
    }
}
