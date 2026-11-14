//! Virtual course roots of a source (ADR 0004).
//!
//! A root says that the part of a source under `prefix` is its own library. This module is
//! pure: it splits one finished [`ScanOutcome`] by prefix and never touches the network,
//! the disk or the database. File paths stay relative to the source, so reads and range
//! requests do not change.

use std::collections::HashSet;

use rusqlite::{OptionalExtension, params};

use crate::db::Database;
use crate::error::Result;
use crate::paths::{KeyMode, is_under_area, relative_path_key};
use crate::scanner::ScanOutcome;

/// Maps `library_id` to the part of a source under `prefix`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRoot {
    /// Library that owns the files under the prefix.
    pub library_id: String,
    /// Path of the root folder, relative to the source, with `/` separators.
    pub prefix: String,
}

/// A set of roots that cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RootError {
    /// The library id is empty.
    #[error("a root needs a library id")]
    EmptyLibraryId,
    /// The prefix is empty or is not a clean relative path.
    #[error("invalid root prefix: {0:?}")]
    InvalidPrefix(String),
    /// Two roots use the same library id.
    #[error("duplicate root library id: {0:?}")]
    DuplicateLibrary(String),
    /// The new root would use the id of the source itself or of a library that exists.
    #[error("library id already in use: {0:?}")]
    LibraryExists(String),
    /// No root has this library id.
    #[error("unknown root: {0:?}")]
    UnknownRoot(String),
    /// One root is the same folder as another or lies inside it.
    #[error("root {inner:?} overlaps root {outer:?}")]
    Overlap {
        /// The shorter prefix, or either one when both are equal.
        outer: String,
        /// The other prefix.
        inner: String,
    },
}

/// The scan of a source split by roots.
#[derive(Debug)]
pub struct Partition {
    /// One entry per root, in the order the roots were given. A root without files stays
    /// in the list so the caller can mark it unavailable.
    pub roots: Vec<(String, ScanOutcome)>,
    /// Everything outside every root; it stays the source's own course.
    pub remainder: ScanOutcome,
}

/// Splits `outcome` by `roots`.
///
/// Roots must not overlap (a nested root is rejected), so a path belongs to at most one
/// root. Matching is by whole folder names: `A` covers `A/x` but not `AB`. An unreadable
/// area keeps its place: it goes to the root that contains it, and a root below an
/// unreadable folder gets `.` so the caller knows the root was not verified.
pub fn partition_scan(
    outcome: ScanOutcome,
    roots: &[SourceRoot],
    mode: KeyMode,
) -> std::result::Result<Partition, RootError> {
    let keys = validated_keys(roots, mode)?;
    let mut parts: Vec<ScanOutcome> = roots.iter().map(|_| empty()).collect();
    let mut remainder = empty();

    for file in outcome.files {
        match keys
            .iter()
            .position(|key| is_under_area(&file.relative_path_key, key))
        {
            Some(index) => parts[index].files.push(file),
            None => remainder.files.push(file),
        }
    }

    for path in outcome.inaccessible {
        let key = relative_path_key(&path, mode).ok();
        let mut placed = false;
        for (index, root_key) in keys.iter().enumerate() {
            let Some(key) = key.as_deref() else { break };
            if is_under_area(key, root_key) {
                parts[index].inaccessible.push(path.clone());
                placed = true;
            } else if is_under_area(root_key, key) {
                parts[index].inaccessible.push(".".to_owned());
            }
        }
        // A path that is inside a root is not also reported for the source's own course.
        if !placed {
            remainder.inaccessible.push(path);
        }
    }

    Ok(Partition {
        roots: roots
            .iter()
            .map(|root| root.library_id.clone())
            .zip(parts)
            .collect(),
        remainder,
    })
}

fn empty() -> ScanOutcome {
    ScanOutcome {
        files: Vec::new(),
        inaccessible: Vec::new(),
    }
}

fn validated_keys(
    roots: &[SourceRoot],
    mode: KeyMode,
) -> std::result::Result<Vec<String>, RootError> {
    let mut ids = HashSet::new();
    let mut keys: Vec<String> = Vec::with_capacity(roots.len());
    for root in roots {
        if root.library_id.is_empty() {
            return Err(RootError::EmptyLibraryId);
        }
        if !ids.insert(root.library_id.as_str()) {
            return Err(RootError::DuplicateLibrary(root.library_id.clone()));
        }
        let key = relative_path_key(&root.prefix, mode)
            .map_err(|_| RootError::InvalidPrefix(root.prefix.clone()))?;
        if let Some(other) = keys
            .iter()
            .find(|other| is_under_area(&key, other) || is_under_area(other, &key))
        {
            return Err(RootError::Overlap {
                outer: other.clone(),
                inner: key,
            });
        }
        keys.push(key);
    }
    Ok(keys)
}

/// Rows of `media` under a prefix key: the folder itself or anything inside it. Done in SQL
/// with `substr` (no `LIKE`), so `%`, `_` and `\` in folder names are plain text.
const UNDER_PREFIX: &str =
    "(relative_path_key = ?3 OR substr(relative_path_key, 1, length(?3) + 1) = ?3 || '/')";

/// Roots of a source, ordered by prefix.
pub fn list_roots(db: &Database, source_id: &str) -> Result<Vec<SourceRoot>> {
    let mut statement = db.connection().prepare(
        "SELECT library_id, prefix FROM library_root WHERE source_id = ?1 ORDER BY prefix_key",
    )?;
    let rows = statement.query_map([source_id], |row| {
        Ok(SourceRoot {
            library_id: row.get(0)?,
            prefix: row.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Makes `library_id` the library of the part of `source_id` under `prefix`.
///
/// One transaction: the matching `media` rows (and their `media_removal` rows) move to the
/// new library, `media_id` never changes, so study state, notes, bookmarks, annotations and
/// flashcards follow without being touched. The new library gets the source's last scan
/// narrowed to the prefix, so it is not shown as verified when the source was not. A user
/// restricted to the source gets the same access to the new library. Returns how many media
/// rows moved.
///
/// The id must be new, and the prefix must not overlap a root the source already has.
pub fn promote_root(
    db: &mut Database,
    source_id: &str,
    library_id: &str,
    prefix: &str,
    mode: KeyMode,
) -> Result<usize> {
    let prefix_key =
        relative_path_key(prefix, mode).map_err(|_| RootError::InvalidPrefix(prefix.to_owned()))?;
    let tx = db.conn.transaction()?;

    let exists = |id: &str| -> rusqlite::Result<bool> {
        tx.query_row("SELECT 1 FROM library WHERE library_id = ?1", [id], |_| {
            Ok(())
        })
        .optional()
        .map(|row| row.is_some())
    };
    if library_id == source_id || exists(library_id)? {
        return Err(RootError::LibraryExists(library_id.to_owned()).into());
    }

    let mut roots = {
        let mut statement =
            tx.prepare("SELECT library_id, prefix FROM library_root WHERE source_id = ?1")?;
        let rows = statement.query_map([source_id], |row| {
            Ok(SourceRoot {
                library_id: row.get(0)?,
                prefix: row.get(1)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    roots.push(SourceRoot {
        library_id: library_id.to_owned(),
        prefix: prefix.to_owned(),
    });
    validated_keys(&roots, mode)?;

    // The source row may not exist before its first scan; the roots then start empty.
    tx.execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 0) ON CONFLICT (library_id) DO NOTHING",
        [source_id],
    )?;
    tx.execute(
        "INSERT INTO library (library_id, available)
         SELECT ?1, available FROM library WHERE library_id = ?2",
        [library_id, source_id],
    )?;
    tx.execute(
        "INSERT INTO library_root (library_id, source_id, prefix, prefix_key) VALUES (?1, ?2, ?3, ?4)",
        params![library_id, source_id, prefix, prefix_key],
    )?;

    let moved = tx.execute(
        &format!("UPDATE media SET library_id = ?1 WHERE library_id = ?2 AND {UNDER_PREFIX}"),
        params![library_id, source_id, prefix_key],
    )?;
    tx.execute(
        "UPDATE media_removal SET library_id = ?1
          WHERE library_id = ?2 AND media_id IN (SELECT media_id FROM media WHERE library_id = ?1)",
        [library_id, source_id],
    )?;
    copy_scan_history(&tx, source_id, library_id, &prefix_key, mode)?;
    tx.execute(
        "INSERT OR IGNORE INTO user_course_access (user_id, course_id)
         SELECT user_id, ?1 FROM user_course_access WHERE course_id = ?2",
        [library_id, source_id],
    )?;
    tx.commit()?;
    Ok(moved)
}

/// Gives the last scan of the source to the new root, narrowed to its prefix.
fn copy_scan_history(
    tx: &rusqlite::Transaction<'_>,
    source_id: &str,
    library_id: &str,
    prefix_key: &str,
    mode: KeyMode,
) -> rusqlite::Result<()> {
    let last: Option<(i64, String, i64, i64)> = tx
        .query_row(
            "SELECT scan_id, status, complete, completed_at FROM scan
              WHERE library_id = ?1 ORDER BY scan_id DESC LIMIT 1",
            [source_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((scan_id, status, complete, completed_at)) = last else {
        return Ok(());
    };
    let mut areas = Vec::new();
    {
        let mut statement =
            tx.prepare("SELECT relative_path FROM scan_inaccessible WHERE scan_id = ?1")?;
        let paths = statement.query_map([scan_id], |row| row.get::<_, String>(0))?;
        for path in paths {
            let path = path?;
            let Ok(key) = relative_path_key(&path, mode) else {
                continue;
            };
            if is_under_area(&key, prefix_key) {
                areas.push(path);
            } else if is_under_area(prefix_key, &key) {
                areas.push(".".to_owned());
            }
        }
    }
    areas.sort();
    areas.dedup();
    let verified = status != "failed" && (complete != 0 || areas.is_empty());
    tx.execute(
        "INSERT INTO scan (library_id, status, completed_at, complete) VALUES (?1, ?2, ?3, ?4)",
        params![
            library_id,
            status,
            completed_at,
            verified && areas.is_empty()
        ],
    )?;
    let new_scan = tx.last_insert_rowid();
    for area in areas {
        tx.execute(
            "INSERT INTO scan_inaccessible (scan_id, relative_path) VALUES (?1, ?2)",
            params![new_scan, area],
        )?;
    }
    Ok(())
}

/// Undoes [`promote_root`]: the media (and removals) go back to the source, the root's
/// scan history and access rows are dropped, its course favorites move to the source
/// course, and the root disappears. Source files are never touched. Returns how many media
/// rows moved back.
pub fn demote_root(db: &mut Database, library_id: &str) -> Result<usize> {
    let tx = db.conn.transaction()?;
    let source_id: String = tx
        .query_row(
            "SELECT source_id FROM library_root WHERE library_id = ?1",
            [library_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| RootError::UnknownRoot(library_id.to_owned()))?;

    let moved = tx.execute(
        "UPDATE media SET library_id = ?1 WHERE library_id = ?2",
        [&source_id, library_id],
    )?;
    tx.execute(
        "UPDATE media_removal SET library_id = ?1 WHERE library_id = ?2",
        [&source_id, library_id],
    )?;
    tx.execute(
        "DELETE FROM scan_inaccessible WHERE scan_id IN (SELECT scan_id FROM scan WHERE library_id = ?1)",
        [library_id],
    )?;
    tx.execute("DELETE FROM scan WHERE library_id = ?1", [library_id])?;
    tx.execute(
        "DELETE FROM user_course_access WHERE course_id = ?1",
        [library_id],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO study_favorite (owner_id, target_kind, target_id, created_at)
         SELECT owner_id, 'course', ?1, created_at FROM study_favorite
          WHERE target_kind = 'course' AND target_id = ?2",
        [&source_id, library_id],
    )?;
    tx.execute(
        "DELETE FROM study_favorite WHERE target_kind = 'course' AND target_id = ?1",
        [library_id],
    )?;
    tx.execute(
        "DELETE FROM library_root WHERE library_id = ?1",
        [library_id],
    )?;
    tx.execute("DELETE FROM library WHERE library_id = ?1", [library_id])?;
    tx.commit()?;
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{MediaKind, ScannedFile};

    fn file(path: &str, mode: KeyMode) -> ScannedFile {
        ScannedFile {
            relative_path: path.to_owned(),
            relative_path_key: relative_path_key(path, mode).unwrap(),
            kind: MediaKind::Media,
            size: 1,
            mtime: 0,
        }
    }

    fn scan(paths: &[&str], inaccessible: &[&str]) -> ScanOutcome {
        ScanOutcome {
            files: paths
                .iter()
                .map(|path| file(path, KeyMode::Sensitive))
                .collect(),
            inaccessible: inaccessible.iter().map(|path| (*path).to_owned()).collect(),
        }
    }

    fn root(library_id: &str, prefix: &str) -> SourceRoot {
        SourceRoot {
            library_id: library_id.to_owned(),
            prefix: prefix.to_owned(),
        }
    }

    fn paths(outcome: &ScanOutcome) -> Vec<&str> {
        outcome
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect()
    }

    fn split(outcome: ScanOutcome, roots: &[SourceRoot]) -> Partition {
        partition_scan(outcome, roots, KeyMode::Sensitive).unwrap()
    }

    #[test]
    fn a_source_without_roots_stays_whole() {
        let part = split(scan(&["a/1.mp4", "b.mp4"], &[]), &[]);
        assert!(part.roots.is_empty());
        assert_eq!(paths(&part.remainder), ["a/1.mp4", "b.mp4"]);
    }

    #[test]
    fn grouped_courses_split_by_prefix_and_keep_source_relative_paths() {
        let outcome = scan(
            &[
                "Cursos/Pentest/Redes/01.mp4",
                "Cursos/Pentest/Redes/02.mp4",
                "Cursos/Pentest/Web/01.mp4",
                "Cursos/Infra/Linux/01.mp4",
            ],
            &[],
        );
        let roots = [
            root("redes", "Cursos/Pentest/Redes"),
            root("web", "Cursos/Pentest/Web"),
        ];
        let part = split(outcome, &roots);
        assert_eq!(part.roots[0].0, "redes");
        assert_eq!(
            paths(&part.roots[0].1),
            ["Cursos/Pentest/Redes/01.mp4", "Cursos/Pentest/Redes/02.mp4"]
        );
        assert_eq!(paths(&part.roots[1].1), ["Cursos/Pentest/Web/01.mp4"]);
        assert_eq!(paths(&part.remainder), ["Cursos/Infra/Linux/01.mp4"]);
    }

    #[test]
    fn every_file_lands_in_exactly_one_place() {
        let all = ["a/x.mp4", "a/b/y.mp4", "c/z.mp4", "loose.mp4", "ab/w.mp4"];
        let part = split(scan(&all, &[]), &[root("a", "a"), root("c", "c")]);
        let total: usize = part.roots.iter().map(|(_, o)| o.files.len()).sum::<usize>()
            + part.remainder.files.len();
        assert_eq!(total, all.len());
    }

    #[test]
    fn a_prefix_matches_whole_folder_names_only() {
        let part = split(scan(&["A/x.mp4", "AB/y.mp4"], &[]), &[root("a", "A")]);
        assert_eq!(paths(&part.roots[0].1), ["A/x.mp4"]);
        assert_eq!(paths(&part.remainder), ["AB/y.mp4"]);
    }

    #[test]
    fn equal_folder_names_under_different_parents_do_not_collide() {
        let outcome = scan(&["Pentest/Redes/1.mp4", "Infra/Redes/1.mp4"], &[]);
        let part = split(
            outcome,
            &[root("p", "Pentest/Redes"), root("i", "Infra/Redes")],
        );
        assert_eq!(paths(&part.roots[0].1), ["Pentest/Redes/1.mp4"]);
        assert_eq!(paths(&part.roots[1].1), ["Infra/Redes/1.mp4"]);
    }

    #[test]
    fn a_course_with_modules_and_a_flat_course_can_share_a_source() {
        let outcome = scan(
            &["Rust/01 Intro/1.mp4", "Rust/02 Types/1.mp4", "Go/1.mp4"],
            &[],
        );
        let part = split(outcome, &[root("rust", "Rust"), root("go", "Go")]);
        assert_eq!(part.roots[0].1.files.len(), 2);
        assert_eq!(part.roots[1].1.files.len(), 1);
        assert!(part.remainder.files.is_empty());
    }

    #[test]
    fn a_root_without_files_is_kept_for_the_caller() {
        let part = split(scan(&["a/1.mp4"], &[]), &[root("gone", "Missing")]);
        assert_eq!(part.roots.len(), 1);
        assert!(part.roots[0].1.files.is_empty());
        assert_eq!(paths(&part.remainder), ["a/1.mp4"]);
    }

    #[test]
    fn case_matters_in_sensitive_mode_and_not_in_insensitive_mode() {
        let sensitive = split(scan(&["rust/1.mp4"], &[]), &[root("r", "Rust")]);
        assert!(sensitive.roots[0].1.files.is_empty());

        let insensitive = ScanOutcome {
            files: vec![file("rust/1.mp4", KeyMode::Insensitive)],
            inaccessible: Vec::new(),
        };
        let part = partition_scan(insensitive, &[root("r", "Rust")], KeyMode::Insensitive).unwrap();
        assert_eq!(part.roots[0].1.files.len(), 1);
    }

    #[test]
    fn overlapping_or_equal_roots_are_rejected() {
        let check = |roots: &[SourceRoot]| {
            partition_scan(scan(&[], &[]), roots, KeyMode::Sensitive).unwrap_err()
        };
        assert!(matches!(
            check(&[root("a", "X"), root("b", "X/Y")]),
            RootError::Overlap { .. }
        ));
        assert!(matches!(
            check(&[root("a", "X/Y"), root("b", "X")]),
            RootError::Overlap { .. }
        ));
        assert!(matches!(
            check(&[root("a", "X"), root("b", "X")]),
            RootError::Overlap { .. }
        ));
        // Sibling folders that only share a name prefix are not an overlap.
        assert!(
            partition_scan(
                scan(&[], &[]),
                &[root("a", "X"), root("b", "XY")],
                KeyMode::Sensitive
            )
            .is_ok()
        );
    }

    #[test]
    fn prefixes_that_could_escape_or_are_unclean_are_rejected() {
        for prefix in [
            "", "..", "a/../b", "/abs", "a//b", "a/", ".", "C:/x", "a\0b",
        ] {
            let result = partition_scan(scan(&[], &[]), &[root("a", prefix)], KeyMode::Sensitive);
            assert_eq!(
                result.unwrap_err(),
                RootError::InvalidPrefix(prefix.to_owned()),
                "{prefix:?}"
            );
        }
    }

    #[test]
    fn library_ids_must_be_set_and_unique() {
        let run = |roots: &[SourceRoot]| {
            partition_scan(scan(&[], &[]), roots, KeyMode::Sensitive).unwrap_err()
        };
        assert_eq!(run(&[root("", "a")]), RootError::EmptyLibraryId);
        assert_eq!(
            run(&[root("x", "a"), root("x", "b")]),
            RootError::DuplicateLibrary("x".into())
        );
    }

    #[test]
    fn unreadable_areas_follow_the_root_that_contains_them() {
        let outcome = scan(&["R/1.mp4"], &["R/private", "Other/dir"]);
        let part = split(outcome, &[root("r", "R")]);
        assert_eq!(part.roots[0].1.inaccessible, ["R/private"]);
        assert_eq!(part.remainder.inaccessible, ["Other/dir"]);
    }

    #[test]
    fn a_root_below_an_unreadable_folder_is_marked_unverified() {
        let part = split(scan(&[], &["Cursos"]), &[root("r", "Cursos/Rust")]);
        assert_eq!(part.roots[0].1.inaccessible, ["."]);
        assert_eq!(part.remainder.inaccessible, ["Cursos"]);
    }
}
