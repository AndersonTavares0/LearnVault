//! Rescan policy: persists the result of a scan and decides what changes.
//!
//! A scan never deletes records. Files found are inserted or
//! updated; files previously known and not found are only marked as
//! missing when the scan really covered the area where they were. If a
//! subtree could not be read, its records are preserved; if the location of the
//! error is imprecise, no absence is recorded in that scan.
//!
//! The identity of the media is the `media_id`, never the path. A file that reappears at the
//! same normalized key reuses the existing row and, with it, the associated progress: the
//! missing row stays in the database and becomes available again.
//!
//! A file that changes path also keeps its identity: when the old row is
//! missing and the new file is not known, size, mtime and a sampled hash of the
//! content recognize the same file. The hash is read only from the ends of the file, and
//! only for new files or files with changed metadata; without it (unreadable file or
//! row from before the migration), the decision falls back to size and mtime, and is only taken when the
//! candidate is unique.
//!
//! The whole application happens in a single transaction. Either the entire scan is
//! recorded, or nothing changes — including the history and the library's state.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};
use crate::ids::new_media_id;
use crate::paths::{KeyMode, relative_path_key};
use crate::scanner::{ScanOutcome, ScannedFile, scan};

/// Bytes read at each end of the file for the sampled hash.
const SAMPLE_BYTES: u64 = 64 * 1024;

/// Summary of what a scan changed in the catalog.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RescanSummary {
    /// Identifier of the record created in the `scan` table.
    pub scan_id: i64,
    /// Tells whether the scan covered the whole library.
    ///
    /// `false` means some area could not be read. This does not prevent recording
    /// absences in the areas that were verified.
    pub complete: bool,
    /// New files inserted into the catalog.
    pub inserted: usize,
    /// Already-known records that the scan reaffirmed.
    pub updated: usize,
    /// Records whose size, mtime or content changed.
    pub changed: usize,
    /// Records whose file changed path and kept its identity.
    pub moved: usize,
    /// Records marked as missing.
    pub marked_missing: usize,
    /// Records preserved because they are in an unverified area.
    pub preserved: usize,
    /// Normalized keys contested by more than one file in this scan.
    pub ambiguous: Vec<String>,
}

/// Applies an already-performed scan to the catalog, without touching the file system.
///
/// The function is pure with respect to the disk: it receives the result of [`scan`] and persists the
/// decisions. This allows testing the policy without assembling a real library and keeps
/// the file-system read isolated at the scanner's boundary.
///
/// Since there is no root, new or changed files do not receive a sampled hash; move detection
/// uses only size and mtime. [`apply_incremental_scan`] receives the root and
/// confirms the move by content.
///
/// `mode` must be the same mode with which the result was produced. The files' keys
/// already come ready in [`ScanOutcome`], but the unverified areas are
/// normalized here: using a different mode from the scanner would make an unreadable directory not
/// cover the records it should protect. [`rescan`] guarantees coherence by using
/// the host's own mode, the same as [`scan`].
pub fn apply_scan(
    db: &mut Database,
    library_id: &str,
    outcome: &ScanOutcome,
    mode: KeyMode,
) -> Result<RescanSummary> {
    apply(db, library_id, outcome, mode, None)
}

/// Applies a scan, reading the root only for the sampled hash of new or
/// changed files.
///
/// The library stays read-only: the hash is read from the ends of the file, never
/// written. It is the form used by [`rescan`] and by automatic indexing.
pub fn apply_incremental_scan(
    db: &mut Database,
    library_id: &str,
    outcome: &ScanOutcome,
    mode: KeyMode,
    root: &Path,
) -> Result<RescanSummary> {
    apply(db, library_id, outcome, mode, Some(root))
}

/// Already-known media row, read in a single snapshot before the decisions.
struct KnownMedia {
    media_id: String,
    size: i64,
    mtime: i64,
    availability: String,
    content_hash: Option<String>,
}

fn apply(
    db: &mut Database,
    library_id: &str,
    outcome: &ScanOutcome,
    mode: KeyMode,
    root: Option<&Path>,
) -> Result<RescanSummary> {
    let tx = db.conn.transaction()?;

    // The library must exist before the media and the history, which reference it.
    tx.execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 0)
         ON CONFLICT (library_id) DO NOTHING",
        rusqlite::params![library_id],
    )?;

    let unverified = unverified_prefixes(outcome, mode);
    let freeze_absences = outcome.inaccessible.iter().any(|path| path == ".");
    let covered = |key: &str| freeze_absences || is_covered_by_unverified(key, &unverified);

    let complete = outcome.inaccessible.is_empty();
    let scan_id = insert_scan(&tx, library_id, "completed", complete)?;
    // The history is a set of areas, not a list: repeating the same path must not
    // violate the primary key and undo the whole scan because of a duplicate.
    let unverified_areas: BTreeSet<&str> =
        outcome.inaccessible.iter().map(String::as_str).collect();
    for path in unverified_areas {
        tx.execute(
            "INSERT INTO scan_inaccessible (scan_id, relative_path) VALUES (?1, ?2)",
            rusqlite::params![scan_id, path],
        )?;
    }

    // Files found, indexed by normalized key. A key contested by
    // more than one file is ambiguous: neither is applied, because choosing one
    // arbitrarily would tie the identity of one file to the content of the other.
    let mut found: BTreeMap<&str, &ScannedFile> = BTreeMap::new();
    let mut ambiguous: BTreeSet<String> = BTreeSet::new();
    for file in &outcome.files {
        if found
            .insert(file.relative_path_key.as_str(), file)
            .is_some()
        {
            ambiguous.insert(file.relative_path_key.clone());
        }
    }

    let mut summary = RescanSummary {
        scan_id,
        complete,
        ambiguous: ambiguous.iter().cloned().collect(),
        ..RescanSummary::default()
    };

    let known = known_media(&tx, library_id)?;
    let moves = MoveIndex::new(&known, &found, &covered);
    // Identities already relocated in this scan: the old row cannot be marked as
    // missing after the file was found at the new path.
    let mut moved_ids: BTreeSet<String> = BTreeSet::new();

    for (key, file) in &found {
        if ambiguous.contains(*key) {
            continue;
        }

        let size = size_as_integer(file.size);
        match known.get(*key) {
            Some(row) => {
                let metadata_changed = row.size != size || row.mtime != file.mtime;
                let mut content_hash = row.content_hash.clone();
                let mut content_changed = false;
                if metadata_changed {
                    match root.and_then(|root| sampled_hash(root, &file.relative_path)) {
                        Some(hash) => {
                            content_changed = row.content_hash.as_deref() != Some(hash.as_str());
                            content_hash = Some(hash);
                        }
                        // Without a hash, the metadata has already proven the change.
                        None => content_changed = true,
                    }
                }
                if content_changed {
                    summary.changed += 1;
                }
                tx.execute(
                    "UPDATE media
                     SET relative_path = ?1, size = ?2, mtime = ?3, availability = 'available',
                         content_hash = ?4
                     WHERE media_id = ?5
                       AND (relative_path <> ?1 OR size <> ?2 OR mtime <> ?3
                            OR availability <> 'available' OR content_hash IS NOT ?4)",
                    rusqlite::params![
                        file.relative_path,
                        size,
                        file.mtime,
                        content_hash,
                        row.media_id
                    ],
                )?;
                summary.updated += 1;
            }
            None => {
                let hash = root.and_then(|root| sampled_hash(root, &file.relative_path));
                match moves.moved(&moved_ids, size, file.mtime, hash.as_deref()) {
                    Some(media_id) => {
                        // Same file at another path: the identity is preserved and,
                        // with it, the notes, the markers and the progress.
                        tx.execute(
                            "UPDATE media
                             SET relative_path = ?1, relative_path_key = ?2, size = ?3,
                                 mtime = ?4, availability = 'available', content_hash = ?5
                             WHERE media_id = ?6",
                            rusqlite::params![
                                file.relative_path,
                                key,
                                size,
                                file.mtime,
                                hash,
                                media_id
                            ],
                        )?;
                        moved_ids.insert(media_id.clone());
                        summary.moved += 1;
                        summary.updated += 1;
                    }
                    None => {
                        tx.execute(
                            "INSERT INTO media
                                 (media_id, library_id, relative_path, relative_path_key,
                                  size, mtime, availability, content_hash)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'available', ?7)",
                            rusqlite::params![
                                new_media_id(),
                                library_id,
                                file.relative_path,
                                key,
                                size,
                                file.mtime,
                                hash,
                            ],
                        )?;
                        summary.inserted += 1;
                    }
                }
            }
        }
    }

    for (key, row) in &known {
        if found.contains_key(key.as_str()) || moved_ids.contains(&row.media_id) {
            continue;
        }
        if row.availability != "available" {
            continue;
        }
        if covered(key) {
            summary.preserved += 1;
            continue;
        }

        tx.execute(
            "UPDATE media SET availability = 'missing' WHERE media_id = ?1",
            rusqlite::params![row.media_id],
        )?;
        summary.marked_missing += 1;
    }

    // `available` only means that the root was reached in this scan; it does not claim that
    // the catalog is intact. A partial scan also reaches the root — whoever needs to
    // know whether some area was left out of the read consults `complete`, in the summary.
    tx.execute(
        "UPDATE library SET available = 1 WHERE library_id = ?1",
        rusqlite::params![library_id],
    )?;

    tx.commit()?;
    Ok(summary)
}

/// Reads the library and applies the result to the catalog.
///
/// If the root cannot be read, the attempt is recorded as a failed scan,
/// without changing the media, the study state or the library's availability.
pub fn rescan(db: &mut Database, library_id: &str, root: &Path) -> Result<RescanSummary> {
    apply_rescan_result(db, library_id, scan(root), root)
}

/// Applies a pre-scanned local filesystem outcome to the catalog database.
///
/// Allows callers to walk the filesystem outside the database lock and acquire it only
/// for the persistence transaction.
pub fn apply_rescan_result(
    db: &mut Database,
    library_id: &str,
    outcome: Result<ScanOutcome>,
    root: &Path,
) -> Result<RescanSummary> {
    match outcome {
        // The host key mode must match the one used during directory scanning.
        Ok(outcome) => apply_incremental_scan(db, library_id, &outcome, KeyMode::for_host(), root),
        Err(scan_error) => Err(record_failed_scan(db, library_id, scan_error)),
    }
}

/// Applies the result of a source that has already been walked, without touching the network or the disk.
///
/// It lets a remote adapter read the tree outside the database lock and only then write
/// the catalog. A failure of the source is recorded in the history exactly like a failure of a
/// local scan, preserving the existing media and progress.
pub fn apply_source_scan(
    db: &mut Database,
    library_id: &str,
    outcome: std::result::Result<ScanOutcome, crate::source::SourceError>,
) -> Result<RescanSummary> {
    match outcome {
        Ok(outcome) => apply_scan(db, library_id, &outcome, KeyMode::for_host()),
        Err(error) => Err(record_failed_scan(db, library_id, error.into())),
    }
}

/// Records the attempt that failed, preserving the original failure as the cause.
fn record_failed_scan(db: &mut Database, library_id: &str, scan_error: Error) -> Error {
    match write_failed_scan(db, library_id) {
        Ok(()) => scan_error,
        Err(history_error) => Error::ScanHistoryNotRecorded {
            scan_error: Box::new(scan_error),
            history_error,
        },
    }
}

/// Records the history of the attempt that failed in a transaction of its own.
fn write_failed_scan(db: &mut Database, library_id: &str) -> rusqlite::Result<()> {
    let tx = db.conn.transaction()?;

    // The library row is needed for the history's foreign key. If it
    // already exists, nothing changes: a failed scan does not alter availability.
    tx.execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 0)
         ON CONFLICT (library_id) DO NOTHING",
        rusqlite::params![library_id],
    )?;
    tx.execute(
        "INSERT INTO scan (library_id, status, completed_at, complete)
         VALUES (?1, 'failed', ?2, 0)",
        rusqlite::params![library_id, now_unix_seconds()],
    )?;

    tx.commit()
}

/// Inserts the scan record and returns the generated identifier.
fn insert_scan(
    tx: &rusqlite::Transaction<'_>,
    library_id: &str,
    status: &str,
    complete: bool,
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO scan (library_id, status, completed_at, complete)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![library_id, status, now_unix_seconds(), complete],
    )?;
    Ok(tx.last_insert_rowid())
}

/// Lists the library's known rows, indexed by normalized key.
fn known_media(
    tx: &rusqlite::Transaction<'_>,
    library_id: &str,
) -> rusqlite::Result<BTreeMap<String, KnownMedia>> {
    let mut statement = tx.prepare(
        "SELECT relative_path_key, media_id, size, mtime, availability, content_hash
         FROM media WHERE library_id = ?1 ORDER BY relative_path_key",
    )?;
    let rows = statement.query_map(rusqlite::params![library_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            KnownMedia {
                media_id: row.get(1)?,
                size: row.get(2)?,
                mtime: row.get(3)?,
                availability: row.get(4)?,
                content_hash: row.get(5)?,
            },
        ))
    })?;

    let mut collected = BTreeMap::new();
    for row in rows {
        let (key, media) = row?;
        collected.insert(key, media);
    }
    Ok(collected)
}

/// Missing candidates indexed by hash and by metadata.
///
/// The scan of a large library cannot compare every new file with every old row:
/// the index keeps the move decision at constant time and keeps the rule of
/// refusing any ambiguous choice.
struct MoveIndex<'a> {
    by_hash: BTreeMap<&'a str, Vec<&'a str>>,
    by_metadata: BTreeMap<(i64, i64), Vec<&'a str>>,
}

impl<'a> MoveIndex<'a> {
    fn new(
        known: &'a BTreeMap<String, KnownMedia>,
        found: &BTreeMap<&str, &ScannedFile>,
        covered: &impl Fn(&str) -> bool,
    ) -> Self {
        let mut index = Self {
            by_hash: BTreeMap::new(),
            by_metadata: BTreeMap::new(),
        };
        for (key, row) in known {
            if found.contains_key(key.as_str()) || covered(key) {
                continue;
            }
            if let Some(hash) = row.content_hash.as_deref() {
                index.by_hash.entry(hash).or_default().push(&row.media_id);
            }
            index
                .by_metadata
                .entry((row.size, row.mtime))
                .or_default()
                .push(&row.media_id);
        }
        index
    }

    /// Returns the missing identity that matches the new file, when the choice is
    /// unique. A confirmed hash wins over the metadata comparison; two rows with the same
    /// content or the same metadata make the move ambiguous, because choosing one would
    /// tie notes and progress to the wrong file.
    fn moved(
        &self,
        moved_ids: &BTreeSet<String>,
        size: i64,
        mtime: i64,
        hash: Option<&str>,
    ) -> Option<String> {
        if let Some(ids) = hash.and_then(|hash| self.by_hash.get(hash)) {
            return unique_unclaimed(ids, moved_ids).map(str::to_owned);
        }
        unique_unclaimed(self.by_metadata.get(&(size, mtime))?, moved_ids).map(str::to_owned)
    }
}

/// Returns the only identifier not yet relocated, or `None` when there are zero or several.
fn unique_unclaimed<'a>(ids: &[&'a str], moved_ids: &BTreeSet<String>) -> Option<&'a str> {
    let mut fresh = ids
        .iter()
        .copied()
        .filter(|media_id| !moved_ids.contains(*media_id));
    match (fresh.next(), fresh.next()) {
        (Some(media_id), None) => Some(media_id),
        _ => None,
    }
}

/// SHA-256 hash of the size and of the ends of the file.
///
/// Files of up to two samples are read whole. An unreadable file has no hash:
/// the move decision falls back to metadata and never fails the scan because of it.
///
/// shortcut: a file changed in the middle without changing size or mtime is not detected,
/// because the sampled hash is only computed when the metadata changes; revisit when there is
/// an event source reliable enough to dispense with the comparison.
fn sampled_hash(root: &Path, relative_path: &str) -> Option<String> {
    let mut file = File::open(root.join(relative_path)).ok()?;
    let length = file.metadata().ok()?.len();
    let sample_len = usize::try_from(SAMPLE_BYTES).ok()?;
    let mut sample = Vec::with_capacity(sample_len.saturating_mul(2));

    if length <= SAMPLE_BYTES.saturating_mul(2) {
        file.read_to_end(&mut sample).ok()?;
    } else {
        let mut head = vec![0; sample_len];
        file.read_exact(&mut head).ok()?;
        file.seek(SeekFrom::End(-(SAMPLE_BYTES as i64))).ok()?;
        let mut tail = vec![0; sample_len];
        file.read_exact(&mut tail).ok()?;
        sample.extend_from_slice(&head);
        sample.extend_from_slice(&tail);
    }

    let mut digest = Sha256::new();
    digest.update(length.to_le_bytes());
    digest.update(&sample);
    Some(
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
}

/// Normalizes the unverified areas into key prefixes.
///
/// An entry that cannot be normalized cannot match any existing row;
/// it only makes the scan partial.
fn unverified_prefixes(outcome: &ScanOutcome, mode: KeyMode) -> Vec<String> {
    outcome
        .inaccessible
        .iter()
        .filter(|path| path.as_str() != ".")
        .filter_map(|path| relative_path_key(path, mode).ok())
        .collect()
}

/// Tells whether the key is inside an area the scan did not verify.
fn is_covered_by_unverified(key: &str, unverified: &[String]) -> bool {
    unverified.iter().any(|prefix| {
        key == prefix
            || (key.len() > prefix.len()
                && key.starts_with(prefix.as_str())
                && key.as_bytes()[prefix.len()] == b'/')
    })
}

/// Converts the size in bytes to the signed integer used by SQLite.
///
/// Sizes above `i64::MAX` (unfeasible on any real file system)
/// saturate at the limit instead of failing or panicking.
fn size_as_integer(size: u64) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}
