//! Virtual course roots of a source (ADR 0004).
//!
//! A root says that the part of a source under `prefix` is its own library. This module is
//! pure: it splits one finished [`ScanOutcome`] by prefix and never touches the network,
//! the disk or the database. File paths stay relative to the source, so reads and range
//! requests do not change.

use std::collections::HashSet;

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
) -> Result<Partition, RootError> {
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

fn validated_keys(roots: &[SourceRoot], mode: KeyMode) -> Result<Vec<String>, RootError> {
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
