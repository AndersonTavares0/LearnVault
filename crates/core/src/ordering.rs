//! Manual ordering of modules and lessons without changing the courses.
//!
//! The catalog's automatic order is the deterministic natural order of
//! [`crate::catalog::natural_cmp`]: runs of digits are compared as a number
//! (`Lesson 2` before `Lesson 10`) and equivalent ties (`Lesson 2`/`Lesson 02`) are
//! broken by the original name, never by the system locale.
//!
//! This module only **overlays**, per parent node, the order chosen by the user. The
//! preference is a list of already-catalogued relative paths; it never reads from nor
//! writes to the course files and never changes a media's identity (`media_id`).
//!
//! The key `""` (empty path) represents the top-level list received by
//! [`apply_order`]: in the adapter, that is the list of modules directly under the course. A
//! cited child that no longer exists in the tree is simply ignored; children not
//! cited keep the automatic order, so a reindex never loses a lesson.

use std::collections::BTreeMap;

use crate::catalog::{CatalogNode, natural_cmp};

/// Manual ordering preference of a single course, among siblings.
///
/// Each entry maps the path of a parent node to the desired order of its children.
/// The top-level list passed to [`apply_order`] corresponds to the parent with an empty path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogOrder {
    parents: BTreeMap<String, Vec<String>>,
}

impl CatalogOrder {
    /// Creates an empty preference (automatic order under every parent).
    pub fn new() -> Self {
        Self::default()
    }

    /// True when there is no preference recorded.
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty()
    }

    /// Records the desired order of the children of `parent`.
    ///
    /// An empty list removes that parent's preference (back to the automatic order).
    pub fn set_children(&mut self, parent: &str, children: Vec<String>) {
        if children.is_empty() {
            self.parents.remove(parent);
        } else {
            self.parents.insert(parent.to_owned(), children);
        }
    }

    /// Desired order of the children of `parent`, when recorded.
    pub fn children(&self, parent: &str) -> Option<&[String]> {
        self.parents.get(parent).map(Vec::as_slice)
    }

    /// Removes a parent's preference. True when there was a preference.
    pub fn remove(&mut self, parent: &str) -> bool {
        self.parents.remove(parent).is_some()
    }

    /// Removes all preferences, restoring the automatic order.
    pub fn clear(&mut self) {
        self.parents.clear();
    }

    /// Replaces the content with the persisted representation (`parent` -> children).
    pub fn from_map(parents: BTreeMap<String, Vec<String>>) -> Self {
        let mut order = Self::default();
        for (parent, children) in parents {
            order.set_children(&parent, children);
        }
        order
    }

    /// Persistable representation (`parent` -> children), ready to serialize.
    pub fn as_map(&self) -> &BTreeMap<String, Vec<String>> {
        &self.parents
    }
}

/// Reorders a list of siblings by the preference, without losing or duplicating anyone.
///
/// The paths cited come first, in the desired order; siblings absent from the list
/// keep the natural order. The sort is stable and purely deterministic.
pub fn order_siblings(children: &mut [CatalogNode], desired: &[String]) {
    if desired.is_empty() {
        return;
    }
    let rank: BTreeMap<&str, usize> = desired
        .iter()
        .enumerate()
        .map(|(index, path)| (path.as_str(), index))
        .collect();
    children.sort_by(|left, right| {
        match (rank.get(left.path.as_str()), rank.get(right.path.as_str())) {
            (Some(left_rank), Some(right_rank)) => left_rank.cmp(right_rank),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => natural_cmp(&left.path, &right.path),
        }
    });
}

/// Applies the preference to a tree of nodes, recursively.
///
/// The top-level `nodes` list is treated as the children of the parent with an empty path. Each node with
/// a preference recorded has its children reordered; the subtree is then walked.
pub fn apply_order(nodes: &mut [CatalogNode], order: &CatalogOrder) {
    if let Some(desired) = order.children("") {
        order_siblings(nodes, desired);
    }
    for node in nodes.iter_mut() {
        apply_node(node, order);
    }
}

/// Applies the preference to a node and to its whole subtree.
fn apply_node(node: &mut CatalogNode, order: &CatalogOrder) {
    if let Some(desired) = order.children(&node.path) {
        order_siblings(&mut node.children, desired);
    }
    for child in node.children.iter_mut() {
        apply_node(child, order);
    }
}

/// Paths of the children of the given parent, when the parent exists in the tree.
///
/// The top-level list is the parent with an empty path (`""`), the same convention as
/// [`apply_order`]. A parent that does not exist in the tree returns `None`, and the adapter
/// treats it as an invalid target instead of creating an orphan preference.
pub fn sibling_paths(nodes: &[CatalogNode], parent: &str) -> Option<Vec<String>> {
    if parent.is_empty() {
        return Some(nodes.iter().map(|node| node.path.clone()).collect());
    }
    find_node(nodes, parent).map(|node| {
        node.children
            .iter()
            .map(|child| child.path.clone())
            .collect()
    })
}

/// Ancestor at an exact path, at any depth.
fn find_node<'a>(nodes: &'a [CatalogNode], path: &str) -> Option<&'a CatalogNode> {
    for node in nodes {
        if node.path == path {
            return Some(node);
        }
        if let Some(found) = find_node(&node.children, path) {
            return Some(found);
        }
    }
    None
}

/// True when `desired` is an exact permutation of `existing`.
///
/// A manual reordering can only swap siblings' positions: the list must contain
/// exactly the same paths, with no repetition and without omitting or adding anyone.
/// This refuses moves between different parents and stale lists.
pub fn validate_siblings(existing: &[String], desired: &[String]) -> bool {
    if desired.len() != existing.len() {
        return false;
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for path in desired {
        *counts.entry(path.as_str()).or_insert(0) += 1;
    }
    if counts.len() != existing.len() {
        return false;
    }
    existing
        .iter()
        .all(|path| counts.get(path.as_str()) == Some(&1))
}
