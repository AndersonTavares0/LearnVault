//! Read-only course sources.
//!
//! A source exposes a file tree without revealing whether it lives on the local disk or on a
//! remote server. The scan uses the same naming policy and the same classification as the
//! local library, so a remote course produces the same [`ScanOutcome`] as a local
//! course with the same files.
//!
//! The contract is strictly read-only: `list` and `read_range` are the only operations.
//! No implementation may create, change, rename, move or remove items of the source.

use std::{
    collections::VecDeque,
    future::Future,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    pin::Pin,
};

use futures_util::{StreamExt, stream::FuturesUnordered};

use crate::error::Error;
use crate::paths::{KeyMode, relative_path_key, resolve_within};
use crate::scanner::{ScanOutcome, ScannedFile, classify, is_ignored_directory};

/// Maximum depth of directories walked in a source.
///
/// It protects the scan from pathological trees (remote ones included) without limiting real
/// courses, which rarely go beyond a few levels.
pub const MAX_SOURCE_DEPTH: usize = 32;

/// Maximum number of files a scan accepts from a source.
pub const MAX_SOURCE_FILES: usize = 200_000;

/// Largest block that [`CourseSource::read_range`] returns at once.
pub const MAX_SOURCE_READ: usize = 8 * 1024 * 1024;

/// Item of a directory listing of a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    /// Path relative to the source root, with `/` separators and the original case.
    pub path: String,
    /// True when the item is a directory.
    pub is_dir: bool,
    /// Size in bytes. Zero for directories and when the source does not report it.
    pub size: u64,
    /// Modification time in seconds since the Unix epoch. `0` means unknown.
    pub mtime: i64,
}

/// Sanitized failure of a source.
///
/// The variants carry no URLs, local paths or credentials: the HTTP adapter
/// chooses the message shown to the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    /// The source could not be reached.
    #[error("a fonte não está acessível")]
    Unreachable,
    /// The source refused authentication or authorization.
    #[error("a fonte recusou o acesso")]
    Denied,
    /// The requested item does not exist in the source.
    #[error("o item não existe na fonte")]
    NotFound,
    /// The source returned a response that could not be interpreted.
    #[error("a resposta da fonte é inválida")]
    InvalidResponse,
    /// The source exceeded a scan safety limit.
    #[error("a fonte excedeu o limite permitido")]
    Limit,
}

/// Result of a source operation.
pub type SourceResult<T> = Result<T, SourceError>;

/// Read-only course tree.
///
/// The methods return futures because remote sources use the network. The trait stays
/// usable as an object (`&dyn CourseSource`) so the scan does not know the
/// transport.
pub trait CourseSource: Send + Sync {
    /// Lists the items directly inside `directory`.
    ///
    /// The root is the empty string. Each item returns the full relative path, never an
    /// absolute path or one outside the root.
    fn list<'a>(
        &'a self,
        directory: &'a str,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<SourceEntry>>> + Send + 'a>>;

    /// Reads up to `length` bytes of `path` starting at `offset`.
    fn read_range<'a>(
        &'a self,
        path: &'a str,
        offset: u64,
        length: usize,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<u8>>> + Send + 'a>>;
}

/// Local source, confined to the given root.
#[derive(Debug, Clone)]
pub struct LocalSource {
    root: PathBuf,
}

impl LocalSource {
    /// Creates a local source rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Root of the source.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn directory_path(&self, directory: &str) -> SourceResult<PathBuf> {
        if directory.is_empty() {
            return Ok(self.root.clone());
        }
        resolve_within(&self.root, directory).map_err(|_| SourceError::NotFound)
    }

    fn file_path(&self, path: &str) -> SourceResult<PathBuf> {
        if path.is_empty() {
            return Err(SourceError::NotFound);
        }
        resolve_within(&self.root, path).map_err(|_| SourceError::NotFound)
    }
}

impl CourseSource for LocalSource {
    fn list<'a>(
        &'a self,
        directory: &'a str,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<SourceEntry>>> + Send + 'a>> {
        Box::pin(async move {
            let path = self.directory_path(directory)?;
            // The root is never followed when it is a link, the same rule as the local scan.
            if directory.is_empty() {
                let metadata =
                    std::fs::symlink_metadata(&path).map_err(|_| SourceError::NotFound)?;
                if metadata.file_type().is_symlink() {
                    return Ok(Vec::new());
                }
                if !metadata.is_dir() {
                    return Err(SourceError::NotFound);
                }
            }

            let read_dir = std::fs::read_dir(&path).map_err(|_| SourceError::Denied)?;
            let mut entries = Vec::new();
            for entry in read_dir {
                let entry = entry.map_err(|_| SourceError::Denied)?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let file_type = entry.file_type().map_err(|_| SourceError::Denied)?;
                // Links are neither followed nor reported, as in the local scan.
                if file_type.is_symlink() {
                    continue;
                }
                let relative = if directory.is_empty() {
                    name
                } else {
                    format!("{directory}/{name}")
                };
                if file_type.is_dir() {
                    entries.push(SourceEntry {
                        path: relative,
                        is_dir: true,
                        size: 0,
                        mtime: 0,
                    });
                    continue;
                }
                if !file_type.is_file() {
                    continue;
                }
                let metadata = entry.metadata().map_err(|_| SourceError::Denied)?;
                entries.push(SourceEntry {
                    path: relative,
                    is_dir: false,
                    size: metadata.len(),
                    mtime: crate::scanner::mtime_seconds(&metadata),
                });
            }
            Ok(entries)
        })
    }

    fn read_range<'a>(
        &'a self,
        path: &'a str,
        offset: u64,
        length: usize,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            if length > MAX_SOURCE_READ {
                return Err(SourceError::Limit);
            }
            let path = self.file_path(path)?;
            let mut file = std::fs::File::open(path).map_err(|_| SourceError::NotFound)?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|_| SourceError::InvalidResponse)?;
            let mut buffer = vec![0; length];
            let mut filled = 0;
            while filled < length {
                match file.read(&mut buffer[filled..]) {
                    Ok(0) => break,
                    Ok(read) => filled += read,
                    Err(_) => return Err(SourceError::InvalidResponse),
                }
            }
            buffer.truncate(filled);
            Ok(buffer)
        })
    }
}

/// Directories listed at the same time while scanning a source.
const SCAN_CONCURRENCY: usize = 8;

/// Extra attempts for a directory whose listing failed on the network.
const LIST_RETRIES: usize = 2;

/// Walks `source` and returns the same result as [`crate::scanner::scan`] for an
/// equivalent local tree.
///
/// Hidden directories, temporary directories and links follow the library's policy.
/// A failure at the root interrupts the scan; a failure in a subtree is recorded in
/// [`ScanOutcome::inaccessible`] and the scan continues.
pub async fn scan_source(source: &dyn CourseSource) -> SourceResult<ScanOutcome> {
    let mode = KeyMode::for_host();
    let mut files = Vec::new();
    let mut inaccessible = Vec::new();
    let mut directories = VecDeque::from([String::new()]);
    // A remote listing is a network round trip, so several run at once; the order they
    // finish in does not matter because the result is sorted at the end.
    let mut listings = FuturesUnordered::new();

    loop {
        while listings.len() < SCAN_CONCURRENCY {
            let Some(directory) = directories.pop_front() else {
                break;
            };
            listings.push(async move {
                let mut listed = source.list(&directory).await;
                // A network blip on one folder must not leave it out of the whole scan.
                for _ in 0..LIST_RETRIES {
                    if !matches!(
                        listed,
                        Err(SourceError::Unreachable | SourceError::InvalidResponse)
                    ) {
                        break;
                    }
                    listed = source.list(&directory).await;
                }
                (directory, listed)
            });
        }
        let Some((directory, listed)) = listings.next().await else {
            break;
        };
        let entries = match listed {
            Ok(entries) => entries,
            Err(error) if directory.is_empty() => return Err(error),
            Err(_) => {
                inaccessible.push(directory);
                continue;
            }
        };

        for entry in entries {
            let Some(name) = entry.path.rsplit('/').next().map(str::to_owned) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            if entry.is_dir {
                if name.starts_with('.') || is_ignored_directory(&name) {
                    continue;
                }
                if entry.path.split('/').count() > MAX_SOURCE_DEPTH {
                    inaccessible.push(entry.path);
                    continue;
                }
                directories.push_back(entry.path);
                continue;
            }
            if name.starts_with('.') {
                continue;
            }
            let Ok(key) = relative_path_key(&entry.path, mode) else {
                inaccessible.push(entry.path);
                continue;
            };
            files.push(ScannedFile {
                relative_path: entry.path,
                relative_path_key: key,
                kind: classify(&name, entry.size),
                size: entry.size,
                mtime: entry.mtime,
            });
            if files.len() > MAX_SOURCE_FILES {
                return Err(SourceError::Limit);
            }
        }
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    inaccessible.sort();
    Ok(ScanOutcome {
        files,
        inaccessible,
    })
}

/// Converts a source failure into a domain error, so the scan history
/// records the attempt without hiding the cause.
impl From<SourceError> for Error {
    fn from(error: SourceError) -> Self {
        Error::SourceUnavailable { source: error }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        future::Future,
        task::{Context, Poll, Waker},
    };

    use super::{CourseSource, LocalSource, SourceEntry, SourceError, scan_source};
    use crate::scanner::{MediaKind, scan};

    /// Runs a future that never truly suspends (local tests and fixtures).
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    struct FixtureSource {
        entries: Vec<SourceEntry>,
    }

    impl CourseSource for FixtureSource {
        fn list<'a>(
            &'a self,
            directory: &'a str,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = super::SourceResult<Vec<SourceEntry>>> + Send + 'a>,
        > {
            Box::pin(async move {
                Ok(self
                    .entries
                    .iter()
                    .filter(|entry| parent(&entry.path) == directory)
                    .cloned()
                    .collect())
            })
        }

        fn read_range<'a>(
            &'a self,
            _path: &'a str,
            _offset: u64,
            _length: usize,
        ) -> std::pin::Pin<Box<dyn Future<Output = super::SourceResult<Vec<u8>>> + Send + 'a>>
        {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    fn parent(path: &str) -> &str {
        path.rsplit_once('/').map_or("", |(parent, _)| parent)
    }

    fn entry(path: &str, is_dir: bool, size: u64) -> SourceEntry {
        SourceEntry {
            path: path.to_owned(),
            is_dir,
            size,
            mtime: 0,
        }
    }

    /// Fails the first listing of one folder, like a dropped connection, then answers.
    struct FlakySource {
        inner: FixtureSource,
        failures_left: std::sync::atomic::AtomicUsize,
        failing: &'static str,
    }

    impl CourseSource for FlakySource {
        fn list<'a>(
            &'a self,
            directory: &'a str,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = super::SourceResult<Vec<SourceEntry>>> + Send + 'a>,
        > {
            use std::sync::atomic::Ordering;
            if directory == self.failing
                && self
                    .failures_left
                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                        left.checked_sub(1)
                    })
                    .is_ok()
            {
                return Box::pin(async { Err(SourceError::Unreachable) });
            }
            self.inner.list(directory)
        }

        fn read_range<'a>(
            &'a self,
            path: &'a str,
            offset: u64,
            length: usize,
        ) -> std::pin::Pin<Box<dyn Future<Output = super::SourceResult<Vec<u8>>> + Send + 'a>>
        {
            self.inner.read_range(path, offset, length)
        }
    }

    fn flaky(failures: usize) -> FlakySource {
        FlakySource {
            inner: FixtureSource {
                entries: vec![
                    entry("Curso", true, 0),
                    entry("Curso/Modulo", true, 0),
                    entry("Curso/Modulo/aula.mp4", false, 10),
                ],
            },
            failures_left: failures.into(),
            failing: "Curso/Modulo",
        }
    }

    #[test]
    fn a_folder_that_fails_once_is_listed_again() {
        let outcome = block_on(scan_source(&flaky(1))).unwrap();
        assert_eq!(outcome.files.len(), 1);
        assert!(outcome.inaccessible.is_empty());
    }

    #[test]
    fn a_folder_that_keeps_failing_is_reported_and_the_rest_is_kept() {
        let outcome = block_on(scan_source(&flaky(10))).unwrap();
        assert!(outcome.files.is_empty());
        assert_eq!(outcome.inaccessible, vec!["Curso/Modulo".to_owned()]);
    }

    #[test]
    fn local_source_matches_the_local_scanner() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir_all(root.join("Curso/Modulo 1")).unwrap();
        fs::write(root.join("Curso/Modulo 1/aula.mp4"), b"video").unwrap();
        fs::write(root.join("Curso/Modulo 1/aula.srt"), b"legenda").unwrap();
        fs::write(root.join("Curso/Modulo 1/apoio.pdf"), b"material").unwrap();
        fs::create_dir_all(root.join("temp_download")).unwrap();
        fs::write(root.join("temp_download/rascunho.mp4"), b"x").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), b"x").unwrap();

        let outcome = block_on(scan_source(&LocalSource::new(root))).unwrap();
        let expected = scan(root).unwrap();

        let paths = |outcome: &crate::scanner::ScanOutcome| {
            outcome
                .files
                .iter()
                .map(|file| (file.relative_path.clone(), file.kind, file.size))
                .collect::<Vec<_>>()
        };
        assert_eq!(paths(&outcome), paths(&expected));
        assert!(outcome.inaccessible.is_empty());
    }

    #[test]
    fn a_remote_like_source_produces_the_same_tree_and_classification() {
        let source = FixtureSource {
            entries: vec![
                entry("Curso", true, 0),
                entry("Curso/Modulo 1", true, 0),
                entry("Curso/Modulo 1/aula.mp4", false, 10),
                entry("Curso/Modulo 1/aula.srt", false, 4),
                entry("Curso/Modulo 1/apoio.pdf", false, 3),
                entry("Curso/.oculto.mp4", false, 1),
                entry("temp", true, 0),
                entry("temp/ignorado.mp4", false, 1),
            ],
        };

        let outcome = block_on(scan_source(&source)).unwrap();
        let files = outcome
            .files
            .iter()
            .map(|file| (file.relative_path.as_str(), file.kind))
            .collect::<Vec<_>>();

        assert_eq!(
            files,
            vec![
                ("Curso/Modulo 1/apoio.pdf", MediaKind::Material),
                ("Curso/Modulo 1/aula.mp4", MediaKind::Media),
                ("Curso/Modulo 1/aula.srt", MediaKind::Subtitle),
            ]
        );
        assert!(outcome.inaccessible.is_empty());
    }

    #[test]
    fn local_source_reads_a_range_and_refuses_paths_outside_the_root() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("aula.mp4"), b"0123456789").unwrap();
        let source = LocalSource::new(root);

        let bytes = block_on(source.read_range("aula.mp4", 2, 4)).unwrap();
        assert_eq!(bytes, b"2345");
        let tail = block_on(source.read_range("aula.mp4", 8, 10)).unwrap();
        assert_eq!(tail, b"89");
        assert_eq!(
            block_on(source.read_range("../fora.mp4", 0, 1)).unwrap_err(),
            SourceError::NotFound
        );
        assert_eq!(
            block_on(source.read_range("aula.mp4", 0, super::MAX_SOURCE_READ + 1)).unwrap_err(),
            SourceError::Limit
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_entries_are_not_listed() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("biblioteca");
        fs::create_dir_all(&root).unwrap();
        let outside = directory.path().join("fora");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("segredo.mp4"), b"x").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("atalho")).unwrap();

        let entries = block_on(LocalSource::new(&root).list("")).unwrap();
        assert!(entries.is_empty());

        let outcome = block_on(scan_source(&LocalSource::new(root))).unwrap();
        assert!(outcome.files.is_empty());
    }
}
