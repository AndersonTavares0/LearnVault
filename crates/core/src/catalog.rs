//! Derivation of the catalog from persisted data.
//!
//! The catalog is a function of what is stored: it reads `media`, the last row of `scan`
//! and the areas of `scan_inaccessible`. It never touches the file system and never writes
//! to the database. The scan (1.3) discovers the files and the rescan policy (1.4) decides
//! availability; this layer only organizes what is already recorded.
//!
//! The hierarchy is derived bottom-up from the persisted paths:
//!
//! - **lesson**: directory that directly contains files;
//! - **module**: parent directory of the lesson;
//! - **course**: parent directory of the module;
//! - **category**: first segment of the path.
//!
//! Levels that do not exist in the library are omitted, instead of synthesizing artificial
//! nodes: in `Category/Course/lesson.mp4` there is no module and the lesson sits directly under
//! the category.
//!
//! The role of each node is decided by the final structure, never by the order in which the files
//! were read: a node that receives files is a lesson, the node that organizes lessons is a module and the one that
//! organizes modules is a course. This way a directory's role does not change because of the name of a
//! neighbouring file.
//!
//! A directory with media inside a lesson — for example `Lesson 1/extras/more.mp4` — is
//! another lesson, nested under the first. Paths deeper than the four-level model do not
//! create extra levels: the segments between the category and the course are folded
//! into the path of the course node.
//!
//! No file is discarded for being at another depth — whatever finds no matching node
//! is preserved in `Catalog::loose_files` (library root) or in
//! `CatalogNode::loose_files` (file directly in the category).
//!
//! When the last scan attempt failed, nothing could be verified: the catalog is
//! marked incomplete and all files are flagged as unverified.
//!
//! Each node is identified by the **full relative path**, never by the title: two
//! courses with the same name at different paths remain distinct nodes.

use std::collections::BTreeMap;

use crate::db::Database;
use crate::error::Result;
use crate::paths::{KeyMode, is_under_area, relative_path_key};
use crate::scanner::{MediaKind, classify};

/// Availability of a file according to the last scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    /// The file was found in the last scan.
    Available,
    /// The file was known and was not found. The record and the identity
    /// remain valid.
    Missing,
}

/// Role of a node in the catalog hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    /// First level under the library root.
    Category,
    /// Set of modules.
    Course,
    /// Set of lessons.
    Module,
    /// Directory that contains the files.
    Lesson,
}

/// File of the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogFile {
    /// Stable identity of the file, independent of its location.
    pub media_id: String,
    /// Name of the file, without directories.
    pub name: String,
    /// Full relative path, used as a structural reference.
    pub path: String,
    /// Size in bytes at the last observation.
    pub size: u64,
    /// Modification time of the media in seconds since the Unix epoch at the last observation.
    pub mtime_seconds: i64,
    /// Type derived from the name and the size.
    pub kind: MediaKind,
    /// Availability in the last scan.
    pub availability: Availability,
    /// True when the file is under an area that the last scan could not
    /// verify.
    pub unverified: bool,
}

/// Node of the catalog tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogNode {
    /// Displayed title, with numeric prefixes and separators removed.
    pub title: String,
    /// Full relative path that identifies the node.
    pub path: String,
    /// Role of the node in the hierarchy.
    pub role: NodeRole,
    /// Files of the directory. Only nodes with the `Lesson` role fill in this field.
    pub files: Vec<CatalogFile>,
    /// Files without a lesson of their own, such as materials and subtitles with no matching media.
    /// Categories and modules may fill it in.
    pub loose_files: Vec<CatalogFile>,
    /// Child nodes, ordered by path.
    pub children: Vec<CatalogNode>,
}

/// Catalog derived from a library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// True when the last scan covered the whole library. With no scan
    /// recorded, nothing is pending and the value is true. A failed attempt
    /// leaves the value false.
    pub complete: bool,
    /// Files at the library root, which belong to no category.
    pub loose_files: Vec<CatalogFile>,
    /// Categories, ordered by path.
    pub categories: Vec<CatalogNode>,
}

/// Builds the catalog of a library from the persisted data.
///
/// It deliberately does not receive the library root: without a path, this function has no way to
/// read the disk. It does not write to the database either.
pub fn build_catalog(db: &Database, library_id: &str) -> Result<Catalog> {
    build_catalog_below(db, library_id, 0)
}

/// Like [`build_catalog`], for a library that owns only the part of a source under a folder
/// of `skip` segments (a course root): the tree starts below that folder, so the folders above
/// it never appear. Every file keeps its path relative to the source, which is what media
/// requests use, and its unverified marker is still judged on that path.
pub fn build_catalog_below(db: &Database, library_id: &str, skip: usize) -> Result<Catalog> {
    let mode = KeyMode::for_host();
    let scan = last_scan(db, library_id, mode)?;
    let rows = media_rows(db, library_id)?;

    let mut loose_files = Vec::new();
    let mut nodes: BTreeMap<String, PendingNode> = BTreeMap::new();

    for row in rows {
        let size = u64::try_from(row.size).map_err(|_| crate::error::Error::MediaCorruptSize {
            media_id: row.media_id.clone(),
        })?;
        let name = row
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(&row.relative_path)
            .to_owned();
        let file = CatalogFile {
            kind: classify(&name, size),
            media_id: row.media_id,
            name,
            path: row.relative_path.clone(),
            size,
            mtime_seconds: row.mtime,
            availability: availability_of(&row.availability),
            unverified: scan.failed || is_under_unverified(&row.relative_path_key, &scan.areas),
        };

        let mut segments: Vec<&str> = row.relative_path.split('/').collect();
        // The last segment is the file name; the directory is the rest.
        segments.pop();
        // A course root starts below its own folder.
        segments.drain(..skip.min(segments.len()));

        match segments.len() {
            0 => loose_files.push(file),
            1 => {
                let category = ensure_node(&mut nodes, segments[0]);
                category.loose_files.push(file);
            }
            depth => {
                if depth >= 4 {
                    ensure_node(&mut nodes, &segments[..depth - 2].join("/"));
                }
                if depth >= 3 {
                    ensure_node(&mut nodes, &segments[..depth - 1].join("/"));
                }
                ensure_node(&mut nodes, segments[0]);
                let lesson = ensure_node(&mut nodes, &segments.join("/"));
                lesson.files.push(file);
            }
        }
    }

    loose_files.sort_by(|a, b| natural_cmp(a.path.as_str(), b.path.as_str()));
    let categories = assemble(&mut nodes);
    Ok(Catalog {
        complete: scan.complete,
        loose_files,
        categories,
    })
}

/// Node under construction, before the tree is assembled.
struct PendingNode {
    title: String,
    files: Vec<CatalogFile>,
    loose_files: Vec<CatalogFile>,
}

/// Returns the node for the given path, creating it if it does not exist yet.
///
/// The role of the node is not decided here: it depends on the final structure and is computed
/// when the tree is assembled, in [`role_of`].
fn ensure_node<'a>(
    nodes: &'a mut BTreeMap<String, PendingNode>,
    path: &str,
) -> &'a mut PendingNode {
    nodes.entry(path.to_owned()).or_insert_with(|| PendingNode {
        title: clean_node_title(last_segment(path)),
        files: Vec::new(),
        loose_files: Vec::new(),
    })
}

/// Assembles the tree from the pending nodes, linking each node to the deepest
/// existing ancestor.
fn assemble(nodes: &mut BTreeMap<String, PendingNode>) -> Vec<CatalogNode> {
    let children: BTreeMap<String, Vec<String>> = {
        let keys: Vec<&str> = nodes.keys().map(String::as_str).collect();
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for path in &keys {
            if let Some(parent) = deepest_existing_ancestor(path, nodes) {
                children
                    .entry(parent.to_owned())
                    .or_default()
                    .push((*path).to_owned());
            }
        }
        for list in children.values_mut() {
            list.sort_by(|a, b| natural_cmp(a, b));
        }
        children
    };
    let mut roots: Vec<String> = nodes
        .keys()
        .filter_map(|path| {
            if deepest_existing_ancestor(path, nodes).is_none() {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect();
    roots.sort_by(|a, b| natural_cmp(a, b));
    roots
        .iter()
        .filter_map(|path| take_node(path, nodes, &children))
        .collect()
}

/// Removes the pending node and returns it with its children already assembled.
fn take_node(
    path: &str,
    nodes: &mut BTreeMap<String, PendingNode>,
    children: &BTreeMap<String, Vec<String>>,
) -> Option<CatalogNode> {
    let pending = nodes.remove(path)?;
    let natural: fn(&CatalogFile, &CatalogFile) -> std::cmp::Ordering =
        |file_a, file_b| natural_cmp(file_a.path.as_str(), file_b.path.as_str());
    let mut files = pending.files;
    files.sort_by(natural);
    let mut loose_files = pending.loose_files;
    loose_files.sort_by(natural);
    let nested: Vec<CatalogNode> = children
        .get(path)
        .map(|list| {
            list.iter()
                .filter_map(|child| take_node(child, nodes, children))
                .collect()
        })
        .unwrap_or_default();
    Some(CatalogNode {
        title: pending.title,
        path: path.to_owned(),
        role: role_of(path, &files, &nested),
        files,
        loose_files,
        children: nested,
    })
}

/// Role of a node, decided by the final structure and never by reading order.
///
/// The first level is always the category. Below it, a node that received files is a
/// lesson, the node that organizes lessons is a module and the one that organizes modules is a course.
fn role_of(path: &str, files: &[CatalogFile], children: &[CatalogNode]) -> NodeRole {
    if !path.contains('/') {
        return NodeRole::Category;
    }
    if !files.is_empty() {
        return NodeRole::Lesson;
    }
    if children.iter().any(|child| child.role == NodeRole::Module) {
        return NodeRole::Course;
    }
    NodeRole::Module
}

/// Compares texts with natural ordering: runs of digits are read as a number
/// (tie-broken by notation, for total determinism) and the rest is compared
/// character by character. "2" comes before "10" at every level of the tree.
pub fn natural_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (left, right);
    loop {
        match (natural_segment(a), natural_segment(b)) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some((segment_a, rest_a)), Some((segment_b, rest_b))) => {
                let ordering = compare_natural_segment(segment_a, segment_b);
                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
                a = rest_a;
                b = rest_b;
            }
        }
    }
}

/// First homogeneous run of the string (only digits or no digits) and the rest.
fn natural_segment(input: &str) -> Option<(&str, &str)> {
    if input.is_empty() {
        return None;
    }
    let digit_run = input.as_bytes()[0].is_ascii_digit();
    let end = input
        .char_indices()
        .find(|(_, character)| character.is_ascii_digit() != digit_run)
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    Some((&input[..end], &input[end..]))
}

/// Compares two homogeneous runs, reading pairs of digits as a number.
fn compare_natural_segment(a: &str, b: &str) -> std::cmp::Ordering {
    if a.as_bytes()[0].is_ascii_digit() && b.as_bytes()[0].is_ascii_digit() {
        let number_a = a.trim_start_matches('0');
        let number_b = b.trim_start_matches('0');
        return number_a
            .len()
            .cmp(&number_b.len())
            .then_with(|| number_a.cmp(number_b))
            .then_with(|| a.cmp(b));
    }
    a.cmp(b)
}

/// Deepest existing ancestor of a path, if there is one.
///
/// The walk looks for the ancestor by removing segments from the path starting at the deepest,
/// reducing the search cost from O(N) to O(depth × log N).
fn deepest_existing_ancestor(path: &str, nodes: &BTreeMap<String, PendingNode>) -> Option<String> {
    let mut current = path;
    while let Some((parent, _)) = current.rsplit_once('/') {
        if nodes.contains_key(parent) {
            return Some(parent.to_owned());
        }
        current = parent;
    }
    None
}

/// Last segment of a relative path.
fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Displayed title of a node: without numeric prefixes or ordering separators.
///
/// Shared with the course registry, which derives the card's title from the folder name,
/// and with the file lesson, which cleans the file name before stripping the extension.
pub fn clean_title(name: &str) -> String {
    name.trim_start_matches(|character: char| "0123456789._- •\t".contains(character))
        .trim()
        .to_owned()
}

/// Displayed title of a catalog node or lesson: if the cleanup would remove all the
/// characters (as with folders or files named "01" or "2024"), it keeps the number.
pub fn clean_node_title(name: &str) -> String {
    let cleaned = crate::titles::presentable(&clean_title(name));
    if cleaned.is_empty() {
        let trimmed = name.trim();
        let fallback = trimmed
            .trim_end_matches(|character: char| "._- •\t".contains(character))
            .trim();
        if fallback.is_empty() {
            trimmed.to_owned()
        } else {
            fallback.to_owned()
        }
    } else {
        cleaned
    }
}

/// Lesson title derived from a file: without extension and without ordering prefix.
fn file_lesson_title(name: &str) -> String {
    let stem = match name.rfind('.') {
        Some(index) if index > 0 => &name[..index],
        _ => name,
    };
    clean_node_title(stem)
}

/// Recomposes courses and modules: media files directly in the directory become lessons,
/// accompanied by subtitles with the same base name. Materials and subtitles with no matching
/// media remain available in `loose_files`.
///
/// The transformation is recursive and does not alter lessons the indexer already derived from
/// directories, including when the module also contains files directly.
pub fn course_file_lessons(node: CatalogNode) -> CatalogNode {
    let CatalogNode {
        title,
        path,
        role,
        files,
        loose_files,
        children,
    } = node;
    let mut children: Vec<_> = children.into_iter().map(course_file_lessons).collect();
    // Lessons of one module often repeat the course name; module and course titles keep theirs.
    if children.iter().all(|child| child.role == NodeRole::Lesson) {
        let mut titles: Vec<String> = children.iter().map(|child| child.title.clone()).collect();
        crate::titles::strip_shared_prefix(&mut titles);
        for (child, title) in children.iter_mut().zip(titles) {
            child.title = title;
        }
    }
    let (has_media, loose) = promote_loose_media_to_lessons(loose_files, &mut children);

    CatalogNode {
        title,
        path,
        role: if has_media { NodeRole::Module } else { role },
        files,
        loose_files: loose,
        children,
    }
}

fn promote_loose_media_to_lessons(
    loose_files: Vec<CatalogFile>,
    children: &mut Vec<CatalogNode>,
) -> (bool, Vec<CatalogFile>) {
    let mut media = Vec::new();
    let mut loose = Vec::new();
    for file in loose_files {
        if file.kind == MediaKind::Media {
            media.push(file);
        } else {
            loose.push(file);
        }
    }

    if media.is_empty() {
        return (false, loose);
    }

    for file in media {
        let stem = file_stem(&file.name).to_string();
        let mut lesson_files = vec![file];
        let mut index = 0;
        while index < loose.len() {
            let candidate = &loose[index];
            if candidate.kind == MediaKind::Subtitle
                && subtitle_belongs_to(&stem, file_stem(&candidate.name))
            {
                lesson_files.push(loose.remove(index));
            } else {
                index += 1;
            }
        }
        let media_file = &lesson_files[0];
        children.push(CatalogNode {
            title: file_lesson_title(&media_file.name),
            path: media_file.path.clone(),
            role: NodeRole::Lesson,
            files: lesson_files,
            loose_files: Vec::new(),
            children: Vec::new(),
        });
    }
    children.sort_by(|left, right| natural_cmp(&left.path, &right.path));
    (true, loose)
}

/// Prepares the subtree and files of a course for uniform presentation:
/// 1. Converts media files at the course root into first-level lessons.
/// 2. Associates matching subtitles with those lessons.
/// 3. Preserves materials at the root in `loose_files`.
/// 4. Recursively applies `course_file_lessons` to all categories and modules.
/// 5. Returns the lessons and modules naturally ordered and the remaining materials.
pub fn shape_course_catalog(catalog: Catalog) -> (Vec<CatalogNode>, Vec<CatalogFile>) {
    let mut children: Vec<CatalogNode> = catalog
        .categories
        .into_iter()
        .map(course_file_lessons)
        .collect();

    let (_, loose) = promote_loose_media_to_lessons(catalog.loose_files, &mut children);
    (children, loose)
}

fn file_stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// Sidecar subtitles share the media stem, optionally followed by a language tag.
///
/// It accepts `lesson.srt` and `lesson.pt-BR.vtt`; a suffix is a language only when its
/// first segment is two or three letters and every later segment is a short region or
/// script. So `lesson.notes.srt` stays a loose material instead of becoming a caption.
fn subtitle_belongs_to(media_stem: &str, subtitle_stem: &str) -> bool {
    if subtitle_stem == media_stem {
        return true;
    }
    let Some(tag) = subtitle_stem
        .strip_prefix(media_stem)
        .and_then(|rest| rest.strip_prefix('.'))
    else {
        return false;
    };
    let tag = tag.trim();
    let mut parts = tag.split(['-', '_']);
    let Some(primary) = parts.next() else {
        return false;
    };
    (2..=3).contains(&primary.len())
        && primary.bytes().all(|byte| byte.is_ascii_alphabetic())
        && parts.all(|part| {
            (2..=4).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

/// Converts the state stored in `media.availability`.
///
/// Shared with explicit removal, which reports availability in the impact.
pub(crate) fn availability_of(stored: &str) -> Availability {
    if stored == "missing" {
        Availability::Missing
    } else {
        Availability::Available
    }
}

/// Tells whether a path key is under some unverified area.
fn is_under_unverified(key: &str, areas: &[String]) -> bool {
    areas.iter().any(|area| is_under_area(key, area))
}

/// `media` row needed to assemble the catalog.
struct MediaRow {
    media_id: String,
    relative_path: String,
    relative_path_key: String,
    size: i64,
    mtime: i64,
    availability: String,
}

/// State of the last scan recorded for the library.
struct LastScan {
    /// True when the scan covered the whole library.
    complete: bool,
    /// True when the last attempt failed: nothing could be verified.
    failed: bool,
    /// Areas the scan could not verify, already normalized.
    areas: Vec<String>,
}

/// Reads the last scan of the library and the areas it could not verify.
fn last_scan(db: &Database, library_id: &str, mode: KeyMode) -> Result<LastScan> {
    let connection = db.connection();
    let (scan_id, complete, status) = {
        let mut statement = connection.prepare(
            "SELECT scan_id, complete, status FROM scan WHERE library_id = ?1 ORDER BY scan_id DESC LIMIT 1",
        )?;
        let mut rows = statement.query([library_id])?;
        match rows.next()? {
            Some(row) => (
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ),
            None => {
                return Ok(LastScan {
                    complete: true,
                    failed: false,
                    areas: Vec::new(),
                });
            }
        }
    };

    let mut statement = connection.prepare(
        "SELECT relative_path FROM scan_inaccessible WHERE scan_id = ?1 ORDER BY relative_path",
    )?;
    let stored = statement
        .query_map([scan_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    let mut areas = Vec::new();
    for path in stored {
        if let Ok(key) = relative_path_key(&path, mode) {
            areas.push(key);
        }
    }
    Ok(LastScan {
        complete: complete != 0,
        failed: status == "failed",
        areas,
    })
}

/// Reads the library's media rows, already ordered by path key.
///
/// Explicitly removed media are left out: the removal takes the item out of the catalog
/// without deleting the row, and so it is filtered here, on read, instead of being a state of the
/// row itself.
fn media_rows(db: &Database, library_id: &str) -> Result<Vec<MediaRow>> {
    let mut statement = db.connection().prepare(
        "SELECT media_id, relative_path, relative_path_key, size, mtime, availability
         FROM media
         WHERE library_id = ?1
           AND NOT EXISTS (SELECT 1
                             FROM media_removal r
                            WHERE r.library_id = media.library_id
                              AND r.media_id = media.media_id)
         ORDER BY relative_path_key",
    )?;
    let rows = statement.query_map([library_id], |row| {
        Ok(MediaRow {
            media_id: row.get(0)?,
            relative_path: row.get(1)?,
            relative_path_key: row.get(2)?,
            size: row.get(3)?,
            mtime: row.get(4)?,
            availability: row.get(5)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}
