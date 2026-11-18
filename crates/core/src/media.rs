//! Safe media resolution.
//!
//! The core accepts a `library_id` and `media_id`, never a client-supplied path. It
//! revalidates the relative path stored in the database, confines it to the library root,
//! and returns an already-open read-only file with metadata from that descriptor. Returning
//! the descriptor prevents adapters from reopening a path that may have changed.
//!
//! Unix opens every path component relative to its parent directory descriptor and refuses
//! symlinks. Windows opens files relative to a directory capability, preventing an
//! intermediate junction from redirecting access outside the root. Hard links to files
//! outside the root are indistinguishable from ordinary files. The library is never written
//! by this code, though another process may still change
//! its contents.
//!
//! The platform adapter is responsible for pairing the supplied root with the `library_id`.
//! This module confines the file to the root it receives but cannot verify that association.

use std::fs::{File, Metadata};
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;

use crate::db::Database;
use crate::error::{Error, Result};
use crate::paths::resolve_within;
use crate::paths::{KeyMode, relative_path_key};
use crate::scanner::{MediaKind, classify, mtime_seconds};

/// Persisted reference used by the HTTP adapter to open a media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaReference {
    /// Library that contains the file.
    pub library_id: String,
    /// Stable identity of the file.
    pub media_id: String,
    /// Persisted relative path, never an absolute path.
    pub relative_path: String,
    size: u64,
    mtime: i64,
}

impl MediaReference {
    /// Size in bytes observed in the last scan.
    pub fn size(&self) -> u64 {
        self.size
    }
}

/// Resolves an indexed relative path without touching the file system.
///
/// The function only returns available media and refuses components that could turn the
/// HTTP input into a path outside the library. The opening and the final validation stay in
/// [`open_media`].
pub fn find_media_by_path(
    db: &Database,
    library_id: &str,
    path: &str,
) -> Result<Option<MediaReference>> {
    let candidate = std::path::Path::new(path);
    if path.is_empty() || candidate.is_absolute() {
        return Ok(None);
    }
    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            std::path::Component::Normal(value) => parts.push(value.to_string_lossy().into_owned()),
            _ => return Ok(None),
        }
    }
    if parts.is_empty() {
        return Ok(None);
    }
    let normalized = parts.join("/");
    let key = match relative_path_key(&normalized, KeyMode::for_host()) {
        Ok(key) => key,
        // The lexical key may refuse a path that the host's component scan
        // did not recognize as a traversal (for example `..\x` on Unix, where `\` is an
        // ordinary character). For the client this is still an absent media, never
        // a server corruption — and the result cannot depend on the platform.
        Err(Error::InvalidRelativePath { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    db.connection()
        .query_row(
            "SELECT library_id, media_id, relative_path, size, mtime
             FROM media
            WHERE media.library_id = ?1 AND media.relative_path_key = ?2 AND media.availability = 'available'
              AND NOT EXISTS (SELECT 1 FROM media_removal r WHERE r.library_id = media.library_id AND r.media_id = media.media_id)
          LIMIT 1",
            rusqlite::params![library_id, key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?
        .map(|(library_id, media_id, relative_path, raw_size, mtime)| {
            u64::try_from(raw_size)
                .map(|size| MediaReference {
                    library_id,
                    relative_path,
                    size,
                    mtime,
                    media_id: media_id.clone(),
                })
                .map_err(|_| Error::MediaCorruptSize { media_id })
        })
        .transpose()
}

/// Finds the library that contains a media by its stable identifier.
///
/// The desktop delivers media by opaque `media_id` without knowing the root: this query links the
/// identity to the registered library, and the opening in [`open_media`] revalidates the corresponding
/// root. It does not touch the file system.
pub fn find_media_library_id(db: &Database, media_id: &str) -> Result<Option<String>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT library_id FROM media WHERE media_id = ?1",
            [media_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

/// Media resolved and already open for reading.
#[derive(Debug)]
pub struct OpenMedia {
    /// Identifier of the media.
    pub media_id: String,
    /// File opened read-only, positioned at the start.
    pub file: File,
    /// Size in bytes, read from the open descriptor.
    pub size: u64,
    /// Modification time in seconds since the Unix epoch, read from the open descriptor.
    pub mtime: i64,
    /// Classification derived from the file name.
    pub kind: MediaKind,
    /// File name (only the final component, never a path).
    ///
    /// It is the only location information exposed to an adapter, and it serves content-type
    /// decisions (for example the MIME allowlist of the desktop's media protocol).
    /// The full path never crosses the core's boundary.
    pub name: String,
    /// True when the file differs from what the last scan recorded.
    pub changed: bool,
}

/// Opens a media of the library for reading.
///
/// The media is identified by `library_id` and `media_id`. The root comes from the platform
/// integration, already resolved; this function only confirms that the file is under it.
///
/// It returns a typed error when the identifier is unknown, when the media was removed
/// from the catalog, when the row is marked as missing, when the file disappeared from the
/// disk, when the stored path is invalid or escapes the library, when the
/// result is not a regular file, when the final component is a symbolic link or
/// reparse point, when the opening fails or when the opened file does not match the
/// validated path.
pub fn open_media(
    db: &Database,
    library_id: &str,
    media_id: &str,
    root: &Path,
) -> Result<OpenMedia> {
    let canonical_root = canonical_root(root)?;
    let stored = media_row(db, library_id, media_id)?;

    // A removed media left the catalog by explicit decision: it is not delivered.
    if is_removed(db, library_id, media_id)? {
        return Err(Error::MediaRemoved {
            media_id: media_id.to_owned(),
        });
    }

    if stored.availability != AVAILABLE {
        return Err(Error::MediaUnavailable {
            media_id: media_id.to_owned(),
        });
    }

    // The path comes from the database and is not trusted: the lexical validation is repeated here, and a
    // path that would escape the root is refused before any disk access.
    let candidate = resolve_within(&canonical_root, &stored.relative_path)?;

    let on_path = std::fs::symlink_metadata(&candidate).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::MediaMissingOnDisk {
            media_id: media_id.to_owned(),
        },
        _ => Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        },
    })?;

    // `symlink_metadata` does not follow the link, so the type of the final component itself is
    // observable. Links are not followed during indexing, so they are not known media.
    if on_path.file_type().is_symlink() || is_reparse_point(&on_path) {
        return Err(Error::MediaIsSymlink {
            media_id: media_id.to_owned(),
        });
    }

    // Classifies the type before opening: on Windows, opening a directory fails with
    // PermissionDenied and would be reported as `MediaUnreadable`, while on Unix the
    // open works and the refusal would only come from the post-open `metadata.is_file()`. Using the
    // type already observed on the path keeps `MediaNotAFile` identical on both platforms and
    // preserves `MediaUnreadable` for real failures to open a regular file.
    if !on_path.is_file() {
        return Err(Error::MediaNotAFile {
            media_id: media_id.to_owned(),
        });
    }

    let resolved = std::fs::canonicalize(&candidate).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::MediaMissingOnDisk {
            media_id: media_id.to_owned(),
        },
        _ => Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        },
    })?;

    // Canonicalization resolves intermediate links; the per-component comparison guarantees
    // that the real target is still inside the library.
    if !is_inside(&resolved, &canonical_root) {
        return Err(Error::MediaOutsideLibrary {
            media_id: media_id.to_owned(),
        });
    }

    let file = open_readonly(&canonical_root, &stored.relative_path, &resolved, media_id)?;

    // The metadata and the classification come from the open descriptor, never from the path.
    let metadata = file.metadata().map_err(|source| Error::MediaUnreadable {
        media_id: media_id.to_owned(),
        source,
    })?;

    if !metadata.is_file() {
        return Err(Error::MediaNotAFile {
            media_id: media_id.to_owned(),
        });
    }

    verify_identity(&resolved, &metadata, media_id)?;

    let size = metadata.len();
    let mtime = mtime_seconds(&metadata);
    let changed = size != stored.size || mtime != stored.mtime;
    let name = file_name(&resolved);
    let kind = classify(&name, size);

    Ok(OpenMedia {
        media_id: media_id.to_owned(),
        file,
        size,
        mtime,
        kind,
        name,
        changed,
    })
}

/// Validates the availability and state of a media reference in the catalog.
///
/// It does not touch the file system nor open descriptors on disk.
pub fn validate_media_reference(db: &Database, reference: &MediaReference) -> Result<()> {
    let media_id = &reference.media_id;
    let stored = media_row(db, &reference.library_id, media_id)?;
    if stored.availability != AVAILABLE {
        return Err(Error::MediaUnavailable {
            media_id: media_id.to_owned(),
        });
    }
    if is_removed(db, &reference.library_id, media_id)? {
        return Err(Error::MediaRemoved {
            media_id: media_id.to_owned(),
        });
    }
    if stored.relative_path != reference.relative_path {
        return Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        });
    }
    Ok(())
}

/// Opens the physical file on disk for a previously validated media reference.
///
/// It does not access the database nor require a lock on the SQLite connection, allowing
/// the opening and the streaming to happen fully decoupled from persistence.
pub fn open_media_file(reference: &MediaReference, root: &Path) -> Result<OpenMedia> {
    let canonical_root = canonical_root(root)?;
    let media_id = &reference.media_id;
    let candidate = resolve_within(&canonical_root, &reference.relative_path)?;
    let on_path = std::fs::symlink_metadata(&candidate).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::MediaMissingOnDisk {
            media_id: media_id.to_owned(),
        },
        _ => Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        },
    })?;
    if on_path.file_type().is_symlink() || is_reparse_point(&on_path) {
        return Err(Error::MediaIsSymlink {
            media_id: media_id.to_owned(),
        });
    }
    // Classifies the type before opening: on Windows, opening a directory fails with
    // PermissionDenied and would be reported as `MediaUnreadable`, while on Unix the
    // open works and the refusal would only come from the post-open `metadata.is_file()`. Using the
    // type already observed on the path keeps `MediaNotAFile` identical on both platforms and
    // preserves `MediaUnreadable` for real failures to open a regular file.
    if !on_path.is_file() {
        return Err(Error::MediaNotAFile {
            media_id: media_id.to_owned(),
        });
    }
    let resolved = std::fs::canonicalize(&candidate).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::MediaMissingOnDisk {
            media_id: media_id.to_owned(),
        },
        _ => Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        },
    })?;
    if !is_inside(&resolved, &canonical_root) {
        return Err(Error::MediaOutsideLibrary {
            media_id: media_id.to_owned(),
        });
    }
    let file = open_readonly(
        &canonical_root,
        &reference.relative_path,
        &resolved,
        media_id,
    )?;
    let metadata = file.metadata().map_err(|source| Error::MediaUnreadable {
        media_id: media_id.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(Error::MediaNotAFile {
            media_id: media_id.to_owned(),
        });
    }
    verify_identity(&resolved, &metadata, media_id)?;
    let size = metadata.len();
    let mtime = mtime_seconds(&metadata);
    let name = file_name(&resolved);
    Ok(OpenMedia {
        media_id: media_id.to_owned(),
        file,
        size,
        mtime,
        kind: classify(&name, size),
        name,
        changed: size != reference.size || mtime != reference.mtime,
    })
}

/// Opens a reference freshly read from the catalog.
///
/// Revalidates the reference in the catalog and opens the descriptor on disk.
pub fn open_media_reference(
    db: &Database,
    reference: &MediaReference,
    root: &Path,
) -> Result<OpenMedia> {
    validate_media_reference(db, reference)?;
    open_media_file(reference, root)
}

/// Availability state recorded by the last scan.
const AVAILABLE: &str = "available";

/// The media was explicitly removed from the catalog.
///
/// The removal is read from its own table, not from `availability`: they are separate facts, and a
/// removed media keeps the availability the last scan recorded.
fn is_removed(db: &Database, library_id: &str, media_id: &str) -> Result<bool> {
    let mut statement = db.connection().prepare(
        "SELECT 1
           FROM media_removal
          WHERE library_id = ?1 AND media_id = ?2",
    )?;
    let mut rows = statement.query(rusqlite::params![library_id, media_id])?;

    Ok(rows.next()?.is_some())
}

/// Media row relevant to the resolution.
struct StoredMedia {
    relative_path: String,
    size: u64,
    mtime: i64,
    availability: String,
}

/// Reads the media row of the given library.
fn media_row(db: &Database, library_id: &str, media_id: &str) -> Result<StoredMedia> {
    let mut statement = db.connection().prepare(
        "SELECT relative_path, size, mtime, availability
         FROM media
         WHERE library_id = ?1 AND media_id = ?2",
    )?;
    let mut rows = statement.query(rusqlite::params![library_id, media_id])?;

    match rows.next()? {
        Some(row) => {
            let size: i64 = row.get(1)?;
            let size = u64::try_from(size).map_err(|_| Error::MediaCorruptSize {
                media_id: media_id.to_owned(),
            })?;
            Ok(StoredMedia {
                relative_path: row.get(0)?,
                size,
                mtime: row.get(2)?,
                availability: row.get(3)?,
            })
        }
        None => Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        }),
    }
}

/// Validates the root and returns its canonical path.
///
/// The root already arrives canonicalized from the platform integration. If the final component
/// became a symbolic link, junction or reparse point, following the target would authorize
/// files outside the library: the replacement is refused, never resolved.
fn canonical_root(root: &Path) -> Result<PathBuf> {
    let path = root.to_path_buf();
    // `symlink_metadata` does not follow the link, so the type of the root's own final component
    // is observable before any canonicalization that would follow it.
    let metadata = std::fs::symlink_metadata(&path).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::LibraryRootMissing { path: path.clone() },
        _ => Error::LibraryRootUnreadable {
            path: path.clone(),
            source,
        },
    })?;

    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(Error::LibraryRootIsLink { path });
    }

    if !metadata.is_dir() {
        return Err(Error::LibraryRootNotDirectory { path });
    }

    std::fs::canonicalize(&path).map_err(|source| Error::LibraryRootUnreadable { path, source })
}

/// Tells whether the path is strictly under the root, comparing components.
fn is_inside(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

/// Checks, where the system allows, that the open descriptor is the file of the validated path.
#[cfg(unix)]
fn verify_identity(resolved: &Path, opened: &Metadata, media_id: &str) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    let on_path = std::fs::symlink_metadata(resolved).map_err(|source| Error::MediaUnreadable {
        media_id: media_id.to_owned(),
        source,
    })?;

    if on_path.dev() != opened.dev() || on_path.ino() != opened.ino() {
        return Err(Error::MediaIdentityMismatch {
            media_id: media_id.to_owned(),
        });
    }
    Ok(())
}

/// Outside Unix, stable `std` does not expose descriptor identity.
#[cfg(not(unix))]
fn verify_identity(_resolved: &Path, opened: &Metadata, media_id: &str) -> Result<()> {
    if is_reparse_point(opened) {
        return Err(Error::MediaIsSymlink {
            media_id: media_id.to_owned(),
        });
    }
    Ok(())
}

/// Opens the validated file read-only without allowing path components to escape the root.
fn open_readonly(_root: &Path, _relative_path: &str, _path: &Path, media_id: &str) -> Result<File> {
    #[cfg(unix)]
    {
        use rustix::fs::{CWD, Mode, OFlags, openat};

        let directory_flags =
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
        let mut directory = openat(CWD, "/", directory_flags, Mode::empty()).map_err(|error| {
            Error::MediaUnreadable {
                media_id: media_id.to_owned(),
                source: io::Error::other(error),
            }
        })?;
        for component in _root.components().skip(1) {
            let std::path::Component::Normal(name) = component else {
                return Err(Error::MediaOutsideLibrary {
                    media_id: media_id.to_owned(),
                });
            };
            directory =
                openat(&directory, name, directory_flags, Mode::empty()).map_err(|error| {
                    Error::MediaUnreadable {
                        media_id: media_id.to_owned(),
                        source: io::Error::other(error),
                    }
                })?;
        }
        let components: Vec<_> = Path::new(_relative_path).components().collect();
        let mut file = None;
        for (index, component) in components.iter().enumerate() {
            let std::path::Component::Normal(name) = component else {
                return Err(Error::MediaOutsideLibrary {
                    media_id: media_id.to_owned(),
                });
            };
            let final_component = index + 1 == components.len();
            let flags = if final_component {
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW
            } else {
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
            };
            let opened = openat(&directory, *name, flags, Mode::empty()).map_err(|error| {
                Error::MediaUnreadable {
                    media_id: media_id.to_owned(),
                    source: io::Error::other(error),
                }
            })?;
            if final_component {
                file = Some(File::from(opened));
            } else {
                directory = opened;
            }
        }
        file.ok_or_else(|| Error::MediaOutsideLibrary {
            media_id: media_id.to_owned(),
        })
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        let root = std::fs::OpenOptions::new()
            .read(true)
            // Keeping delete sharing disabled pins the capability to this directory on Windows.
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(_root)
            .map_err(|source| Error::MediaUnreadable {
                media_id: media_id.to_owned(),
                source,
            })?;
        let metadata = root.metadata().map_err(|source| Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        })?;
        if is_reparse_point(&metadata) {
            return Err(Error::LibraryRootIsLink {
                path: _root.to_path_buf(),
            });
        }
        if !metadata.is_dir() {
            return Err(Error::LibraryRootNotDirectory {
                path: _root.to_path_buf(),
            });
        }

        cap_std::fs::Dir::from_std_file(root)
            .open(_relative_path)
            .map(cap_std::fs::File::into_std)
            .map_err(|source| Error::MediaUnreadable {
                media_id: media_id.to_owned(),
                source,
            })
    }

    #[cfg(not(any(unix, windows)))]
    {
        std::fs::OpenOptions::new()
            .read(true)
            .open(_path)
            .map_err(|source| Error::MediaUnreadable {
                media_id: media_id.to_owned(),
                source,
            })
    }
}

/// True when the metadata marks a reparse point (symlink or junction) on Windows.
#[cfg(windows)]
fn is_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Outside Windows only symbolic links exist, and they are already detected by `is_symlink`.
#[cfg(not(windows))]
fn is_reparse_point(_metadata: &Metadata) -> bool {
    false
}

/// Windows attribute marking a symbolic link, junction, or other reparse point.
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

#[cfg(windows)]
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

#[cfg(windows)]
const FILE_SHARE_READ: u32 = 0x0000_0001;

#[cfg(windows)]
const FILE_SHARE_WRITE: u32 = 0x0000_0002;

/// File name at the end of the path, for classification by extension.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
#[cfg(all(test, windows))]
mod windows_open_tests {
    use super::open_readonly;
    #[test]
    fn an_intermediate_directory_link_cannot_escape_the_open_root()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("library");
        let outside = directory.path().join("outside");
        std::fs::create_dir_all(root.join("Course"))?;
        std::fs::create_dir_all(&outside)?;
        std::fs::write(outside.join("video.mp4"), b"synthetic video")?;
        let link = root.join("Course/Alias");
        if std::os::windows::fs::symlink_dir(&outside, &link).is_err() {
            eprintln!("skipped: creating a directory link requires Windows privileges");
            return Ok(());
        }

        let result = open_readonly(
            &root,
            "Course/Alias/video.mp4",
            &root.join("Course/Alias/video.mp4"),
            "media-1",
        );
        assert!(result.is_err(), "directory links must not escape the root");
        Ok(())
    }

    #[test]
    fn root_junction_is_not_followed() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let outside = directory.path().join("outside");
        let root_link = directory.path().join("library");
        std::fs::create_dir_all(&outside)?;
        std::fs::write(outside.join("video.mp4"), b"synthetic video")?;
        if std::os::windows::fs::symlink_dir(&outside, &root_link).is_err() {
            eprintln!("skipped: creating a directory link requires Windows privileges");
            return Ok(());
        }

        assert!(
            open_readonly(
                &root_link,
                "video.mp4",
                &root_link.join("video.mp4"),
                "media-1",
            )
            .is_err()
        );
        Ok(())
    }
}
