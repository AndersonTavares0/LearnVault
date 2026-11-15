//! Finds where each course of a source starts (ADR 0004).
//!
//! A source often holds a whole library: categories, sub-categories and then the courses.
//! Read as one course it would have hundreds of "modules". The rule reads the shape from the
//! lessons upward and never looks at names, languages or a fixed depth:
//!
//! - `height` of a folder is how many levels of media-bearing folders lie below it;
//! - a folder with media of its own, or less than two levels, is a course;
//! - from two levels on, a folder is a group of courses when its subfolders are not numbered
//!   like modules ("01 Intro", "Module 2", "Lesson 41"), or when one of them is a group;
//! - a numbered sequence of modules makes its parent a course, however deep it is, unless a
//!   subfolder is itself a group (a numbered list of categories).
//!
//! The result is a list of folders; the caller decides what to do with them. Pure: no disk,
//! network or database.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::db::Database;
use crate::error::{Error, Result};
use crate::ids::new_library_id;
use crate::paths::{KeyMode, is_under_area, relative_path_key};
use crate::roots::{list_roots, promote_root};
use crate::scanner::{MediaKind, ScanOutcome, ScannedFile};
use rusqlite::params;

/// Most courses detection makes of a source at once; a source with more is left for the picker,
/// so a rule that misfires on a huge tree cannot reshape it.
const MAX_AUTO_COURSES: usize = 100;

/// A subfolder named like a module: an optional short word, then a number ("01 - Intro",
/// "Module 15", "Lesson 41", "3. Calculus").
fn numbered(name: &str) -> bool {
    let rest = name.trim_start_matches(|c: char| !c.is_alphanumeric());
    let after_word = {
        let word = rest.chars().take_while(|c| c.is_alphabetic()).count();
        if (2..=10).contains(&word) {
            rest[rest
                .char_indices()
                .nth(word)
                .map_or(rest.len(), |(at, _)| at)..]
                .trim_start_matches(|c: char| !c.is_alphanumeric())
        } else {
            rest
        }
    };
    after_word
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
}

#[derive(Default)]
struct Folder {
    /// Media files directly inside.
    direct: usize,
    /// Subfolders that hold media somewhere below.
    children: Vec<String>,
}

struct Tree {
    folders: BTreeMap<String, Folder>,
    height: HashMap<String, usize>,
    group: HashMap<String, bool>,
}

impl Tree {
    fn new(files: &[ScannedFile]) -> Self {
        let mut folders: BTreeMap<String, Folder> = BTreeMap::new();
        folders.entry(String::new()).or_default();
        for file in files.iter().filter(|file| file.kind == MediaKind::Media) {
            let segments: Vec<&str> = file.relative_path.split('/').collect();
            let Some((_, parents)) = segments.split_last() else {
                continue;
            };
            let mut path = String::new();
            for (depth, segment) in parents.iter().enumerate() {
                let parent = path.clone();
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(segment);
                let known = folders.contains_key(&path);
                folders.entry(path.clone()).or_default();
                if !known && depth < parents.len() {
                    folders
                        .entry(parent)
                        .or_default()
                        .children
                        .push(path.clone());
                }
            }
            folders.entry(path).or_default().direct += 1;
        }
        Self {
            folders,
            height: HashMap::new(),
            group: HashMap::new(),
        }
    }

    fn height(&mut self, path: &str) -> usize {
        if let Some(height) = self.height.get(path) {
            return *height;
        }
        let children = self.folders[path].children.clone();
        let height = children
            .iter()
            .map(|child| self.height(child) + 1)
            .max()
            .unwrap_or(0);
        self.height.insert(path.to_owned(), height);
        height
    }

    /// True when `path` holds courses instead of being one.
    fn is_group(&mut self, path: &str) -> bool {
        if let Some(group) = self.group.get(path) {
            return *group;
        }
        let children = self.folders[path].children.clone();
        let group = self.folders[path].direct == 0 && self.height(path) >= 2 && {
            let numbered_children = children
                .iter()
                .filter(|child| numbered(child.rsplit('/').next().unwrap_or(child)))
                .count();
            numbered_children * 2 < children.len()
                || children.iter().any(|child| self.is_group(child))
        };
        self.group.insert(path.to_owned(), group);
        group
    }

    fn courses(&mut self, path: &str, found: &mut Vec<String>) {
        for child in self.folders[path].children.clone() {
            if self.is_group(&child) {
                self.courses(&child, found);
            } else {
                found.push(child);
            }
        }
    }
}

/// Folders of `files` that look like one course each, in path order. Empty when the whole
/// tree is a single course (or has no media): then there is nothing to split.
pub fn detect_course_folders(files: &[ScannedFile]) -> Vec<String> {
    let mut tree = Tree::new(files);
    let mut found = Vec::new();
    if tree.is_group("") {
        tree.courses("", &mut found);
    }
    found.sort();
    found
}

/// Makes each folder [`detect_course_folders`] finds a course root of `source_id`, unless the
/// administrator removed it before or it overlaps a root the source already has. Only files
/// outside the existing roots are considered. Returns how many roots were created.
///
/// A source scan that could not read some folders is not used: a missing subtree would change
/// the shape.
pub fn auto_promote_roots(
    db: &mut Database,
    source_id: &str,
    scan: &ScanOutcome,
    mode: KeyMode,
) -> Result<usize> {
    if !scan.inaccessible.is_empty() {
        return Ok(0);
    }
    let roots = list_roots(db, source_id)?;
    let root_keys: Vec<String> = roots
        .iter()
        .filter_map(|root| relative_path_key(&root.prefix, mode).ok())
        .collect();
    let outside: Vec<ScannedFile> = scan
        .files
        .iter()
        .filter(|file| {
            !root_keys
                .iter()
                .any(|key| is_under_area(&file.relative_path_key, key))
        })
        .cloned()
        .collect();
    let dismissed = dismissed_keys(db, source_id)?;
    let found = detect_course_folders(&outside);
    if found.len() > MAX_AUTO_COURSES {
        return Ok(0);
    }
    let mut created = 0;
    for folder in found {
        let Ok(key) = relative_path_key(&folder, mode) else {
            continue;
        };
        if dismissed.contains(&key) {
            continue;
        }
        match promote_root(db, source_id, &new_library_id(), &folder, mode) {
            Ok(_) => created += 1,
            // Overlaps and invalid prefixes are skipped; a database failure is reported.
            Err(Error::SourceRoot(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(created)
}

/// Prefix keys of the folders the administrator took back from automatic detection.
pub fn dismissed_keys(db: &Database, source_id: &str) -> Result<HashSet<String>> {
    let mut statement = db
        .connection()
        .prepare("SELECT prefix_key FROM library_root_dismissed WHERE source_id = ?1")?;
    let rows = statement.query_map([source_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
}

/// Remembers that `prefix` must not become a course again by itself.
pub fn dismiss_root_prefix(
    db: &Database,
    source_id: &str,
    prefix: &str,
    mode: KeyMode,
) -> Result<()> {
    if let Ok(key) = relative_path_key(prefix, mode) {
        db.connection().execute(
            "INSERT OR IGNORE INTO library_root_dismissed (source_id, prefix_key) VALUES (?1, ?2)",
            params![source_id, key],
        )?;
    }
    Ok(())
}

/// Forgets a dismissal, because the administrator chose that folder again.
pub fn restore_root_prefix(
    db: &Database,
    source_id: &str,
    prefix: &str,
    mode: KeyMode,
) -> Result<()> {
    if let Ok(key) = relative_path_key(prefix, mode) {
        db.connection().execute(
            "DELETE FROM library_root_dismissed WHERE source_id = ?1 AND prefix_key = ?2",
            params![source_id, key],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{detect_course_folders, numbered};
    use crate::scanner::{MediaKind, ScannedFile};

    fn files(paths: &[&str]) -> Vec<ScannedFile> {
        paths
            .iter()
            .map(|path| ScannedFile {
                relative_path: (*path).to_owned(),
                relative_path_key: (*path).to_owned(),
                kind: MediaKind::Media,
                size: 1,
                mtime: 0,
            })
            .collect()
    }

    /// `count` lessons "<dir>/<n>.mp4".
    fn lessons(dir: &str, count: usize) -> Vec<String> {
        (1..=count).map(|n| format!("{dir}/{n}.mp4")).collect()
    }

    fn detect(groups: &[Vec<String>]) -> Vec<String> {
        let paths: Vec<String> = groups.iter().flatten().cloned().collect();
        let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
        detect_course_folders(&files(&refs))
    }

    #[test]
    fn names_with_a_leading_number_are_modules() {
        for name in [
            "01 - Intro",
            "3. Calculus",
            "Module 15",
            "Lesson 41",
            "Etapa 00 - Start",
            "_02_Atoms",
        ] {
            assert!(numbered(name), "{name}");
        }
        for name in [
            "Docker Basics",
            "Udemy - Python 2024",
            "Cursos Alves",
            "Formation",
        ] {
            assert!(!numbered(name), "{name}");
        }
    }

    #[test]
    fn a_single_course_with_numbered_modules_is_not_split() {
        let found = detect(&[
            lessons("01 Intro", 5),
            lessons("02 Basics", 6),
            lessons("03 Advanced", 4),
        ]);
        assert!(found.is_empty());
    }

    #[test]
    fn a_single_flat_course_is_not_split() {
        assert!(
            detect(&[lessons("", 4)
                .iter()
                .map(|p| p.trim_start_matches('/').to_owned())
                .collect()])
            .is_empty()
        );
    }

    #[test]
    fn a_library_of_plain_courses_gives_one_course_per_folder() {
        let found = detect(&[
            lessons("Rust/01 Intro", 5),
            lessons("Rust/02 Types", 5),
            lessons("Go/01 Intro", 5),
            lessons("Go/02 Types", 5),
        ]);
        assert_eq!(found, ["Go", "Rust"]);
    }

    #[test]
    fn numbered_categories_over_courses_over_modules_find_the_courses() {
        let found = detect(&[
            lessons("01 - Code/Languages/Rust Course/01 Intro", 5),
            lessons("01 - Code/Languages/Rust Course/02 Types", 5),
            lessons("01 - Code/Languages/Go Course/Module 1", 5),
            lessons("01 - Code/Languages/Go Course/Module 2", 5),
            lessons("01 - Code/Tools/Git Course/01 Start", 5),
            lessons("01 - Code/Tools/Git Course/02 Branches", 5),
            lessons("02 - Math/Physics Lessons/01. Motion", 5),
            lessons("02 - Math/Physics Lessons/02. Heat", 5),
            lessons("02 - Math/Calculus Course/01. Limits", 5),
            lessons("02 - Math/Calculus Course/02. Derivatives", 5),
        ]);
        assert_eq!(
            found,
            [
                "01 - Code/Languages/Go Course",
                "01 - Code/Languages/Rust Course",
                "01 - Code/Tools/Git Course",
                "02 - Math/Calculus Course",
                "02 - Math/Physics Lessons",
            ]
        );
    }

    #[test]
    fn a_deep_course_with_parts_and_lesson_folders_stays_whole() {
        // "01 - Part" / "Module 15" / "Lesson 1" / video: four levels, all numbered.
        let deep = [
            lessons("01 - Base/Module 1/Lesson 1", 2),
            lessons("01 - Base/Module 2/Lesson 1", 2),
            lessons("02 - Sea/Module 3/Lesson 1", 2),
            lessons("02 - Sea/Module 4/Lesson 1", 2),
        ];
        // The source itself is the course: nothing to split.
        assert!(detect(&deep).is_empty());
        // The same course inside a folder: that folder is the course, not its parts.
        let wrapped: Vec<Vec<String>> = deep
            .iter()
            .map(|group| group.iter().map(|path| format!("English/{path}")).collect())
            .collect();
        assert_eq!(detect(&wrapped), ["English"]);
    }

    #[test]
    fn a_wrapper_folder_around_one_course_names_the_course_not_the_wrapper() {
        let found = detect(&[
            lessons("Languages/English Course/01 - Base/Module 1", 3),
            lessons("Languages/English Course/01 - Base/Module 2", 3),
            lessons("Languages/English Course/02 - Sea/Module 3", 3),
            lessons("Tools/Git Course/01 Start", 3),
            lessons("Tools/Git Course/02 Branches", 3),
        ]);
        assert_eq!(found, ["Languages/English Course", "Tools/Git Course"]);
    }

    #[test]
    fn only_media_counts() {
        let mut data = files(&["Course/readme.txt", "Other/notes.pdf"]);
        for file in &mut data {
            file.kind = MediaKind::Material;
        }
        assert!(detect_course_folders(&data).is_empty());
    }
}
