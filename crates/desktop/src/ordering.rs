//! Manual ordering of modules and lessons on the desktop (issue #239).
//!
//! As on the server, the order chosen by the user is LearnVault state: it lives in the
//! versioned file `catalog-order.json`, inside the application's data directory (next to the
//! database and the covers), never inside the course folders, which stay external and
//! read-only.
//!
//! The preference is applied as an overlay in [`learnvault_core::apply_order`] over the
//! automatic order, derived on every read; removing the preference restores the automatic
//! order without touching any course file.

use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use learnvault_core::CatalogOrder;

/// Name of the ordering file inside the application's data directory.
pub const ORDER_FILE_NAME: &str = "catalog-order.json";

/// Version of the ordering file's format.
pub const ORDER_VERSION: u32 = 1;

/// Ordering preferences of all courses, by the course's opaque identity.
pub type CourseOrders = BTreeMap<String, CatalogOrder>;

/// Stable failure of ordering persistence. It never carries a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderError {
    /// The ordering could not be written safely.
    WriteFailed,
}

/// Serialized format of the ordering file.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OrderFile {
    version: u32,
    /// Course -> (parent path -> desired order of the children).
    courses: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// Path of the ordering file inside the data directory.
pub fn order_path(data_dir: &Path) -> PathBuf {
    data_dir.join(ORDER_FILE_NAME)
}

/// Loads the persisted preferences.
///
/// A missing, corrupted or unknown-version file results in no preference
/// (automatic order); no message carries a path.
pub fn load_orders(data_dir: &Path) -> CourseOrders {
    let Ok(raw) = std::fs::read(order_path(data_dir)) else {
        return CourseOrders::new();
    };
    let Ok(stored) = serde_json::from_slice::<OrderFile>(&raw) else {
        return CourseOrders::new();
    };
    if stored.version != ORDER_VERSION {
        return CourseOrders::new();
    }
    stored
        .courses
        .into_iter()
        .map(|(course_id, parents)| (course_id, CatalogOrder::from_map(parents)))
        .filter(|(_, order)| !order.is_empty())
        .collect()
}

/// Writes the preferences in a serialized, atomic way.
pub fn store_orders(data_dir: &Path, orders: &CourseOrders) -> Result<(), OrderError> {
    let stored = OrderFile {
        version: ORDER_VERSION,
        courses: orders
            .iter()
            .filter(|(_, order)| !order.is_empty())
            .map(|(course_id, order)| (course_id.clone(), order.as_map().clone()))
            .collect(),
    };
    let payload = serde_json::to_vec(&stored).map_err(|_| OrderError::WriteFailed)?;
    write_atomically(&order_path(data_dir), &payload)
}

/// Writes `bytes` to the destination atomically, creating the data directory if needed.
fn write_atomically(dest: &Path, bytes: &[u8]) -> Result<(), OrderError> {
    let directory = dest.parent().ok_or(OrderError::WriteFailed)?;
    std::fs::create_dir_all(directory).map_err(|_| OrderError::WriteFailed)?;

    let temporary = directory.join(format!(".{ORDER_FILE_NAME}.{}.tmp", std::process::id()));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(OrderError::WriteFailed);
    }
    if replace_file(&temporary, dest).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(OrderError::WriteFailed);
    }
    Ok(())
}

/// Replaces the destination using only standard APIs.
#[cfg(windows)]
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(dest) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    std::fs::rename(source, dest)
}

/// Replaces the destination using only standard APIs.
#[cfg(not(windows))]
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::rename(source, dest)
}
