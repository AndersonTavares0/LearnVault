//! Read-only SMB2/3 client.
//!
//! The connection uses TCP/445 with NTLM authentication, SMB signing and SMB3 encryption
//! when the server requires it. DFS is deliberately disabled: a DFS reference could
//! forward the credentials to another server, as an HTTP redirect would.
//!
//! No write operation is issued: the client only opens existing resources with
//! `GENERIC_READ`, lists directories and reads ranges.

use std::{future::Future, io, pin::Pin, str::FromStr, sync::Arc, time::Duration};

use learnvault_core::{CourseSource, MAX_SOURCE_READ, SourceEntry, SourceError, SourceResult};
use smb::{
    Client, ClientConfig, ConnectionConfig, Directory, File, FileAccessMask, FileCreateArgs,
    FileDirectoryInformation, ReadAt, Resource, UncPath,
};
use tokio::sync::Mutex;
use tokio_stream::{Stream, StreamExt, wrappers::ReceiverStream};
use url::Url;

/// Maximum time to resolve and establish the SMB connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum time of a listing or of a read block.
const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum items accepted in a single directory listing.
const MAX_DIRECTORY_ENTRIES: usize = 100_000;
/// Bytes per SMB read request; the protocol limits each response.
const READ_CHUNK: usize = 1024 * 1024;
/// `FILETIME` counts 100 ns intervals.
const FILETIME_TICKS_PER_SECOND: u64 = 10_000_000;
/// Seconds between 1601-01-01 (the `FILETIME` epoch) and 1970-01-01.
const FILETIME_UNIX_OFFSET: i64 = 11_644_473_600;

/// Converts a `FILETIME` into seconds since the Unix epoch.
///
/// The scan reads `0` as an unknown mtime, so a date before 1970 keeps that meaning.
fn unix_seconds(ticks: u64) -> i64 {
    let seconds = i64::try_from(ticks / FILETIME_TICKS_PER_SECOND).unwrap_or(0);
    seconds.saturating_sub(FILETIME_UNIX_OFFSET).max(0)
}

/// Target of an `smb://host[:port]/share/path` URL.
#[derive(Debug, Clone)]
pub(crate) struct SmbLocation {
    /// Host and optional port, in the format accepted by the SMB client.
    server: String,
    /// Share name, without slashes.
    share: String,
    /// Initial path inside the share, with `/` and no trailing slash.
    prefix: String,
    /// Resolved UNC of the share.
    base: UncPath,
}

impl SmbLocation {
    /// Derives the target from a URL already validated by [`crate::webdav::parse_source_url`].
    pub(crate) fn from_url(url: &Url) -> SourceResult<Self> {
        if url.scheme() != "smb" {
            return Err(SourceError::NotFound);
        }
        let host = url.host_str().ok_or(SourceError::NotFound)?;
        // `host_str` returns IPv6 already in brackets in some schemes.
        let host = host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(host);
        let server = match url.port() {
            // An IPv6 literal needs brackets so it does not confuse the port.
            Some(port) if host.contains(':') => format!("[{host}]:{port}"),
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        let segments = crate::webdav::decoded_segments(url)?;
        let (share, prefix) = segments.split_first().ok_or(SourceError::NotFound)?;
        let base = UncPath::from_str(&format!("\\\\{server}\\{share}"))
            .map_err(|_| SourceError::NotFound)?;
        Ok(Self {
            server,
            share: share.clone(),
            prefix: prefix.join("/"),
            base,
        })
    }
}

/// SMB client of a source, with the session reused between scan calls.
pub(crate) struct SmbSource {
    location: SmbLocation,
    username: Option<String>,
    password: Option<String>,
    client: Mutex<Option<Arc<Client>>>,
}

impl std::fmt::Debug for SmbSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SmbSource")
            .field("server", &self.location.server)
            .field("share", &self.location.share)
            .field("prefix", &self.location.prefix)
            .field("username", &self.username)
            .field("has_password", &self.password.is_some())
            .finish()
    }
}

impl SmbSource {
    /// Creates the client for an already validated URL.
    pub(crate) fn new(
        url: Url,
        username: Option<&str>,
        password: Option<&str>,
    ) -> SourceResult<Self> {
        Ok(Self {
            location: SmbLocation::from_url(&url)?,
            username: username.map(str::to_owned),
            password: password.map(str::to_owned),
            client: Mutex::new(None),
        })
    }

    async fn client(&self) -> SourceResult<Arc<Client>> {
        let mut slot = self.client.lock().await;
        if let Some(client) = slot.as_ref() {
            return Ok(Arc::clone(client));
        }
        let client = Client::new(ClientConfig {
            // A DFS reference would take the credentials to another server.
            dfs: false,
            connection: ConnectionConfig {
                timeout: Some(CONNECT_TIMEOUT),
                compression_enabled: false,
                ..Default::default()
            },
            ..Default::default()
        });
        let connected = tokio::time::timeout(
            CONNECT_TIMEOUT,
            client.share_connect(
                &self.location.base,
                self.username.as_deref().unwrap_or(""),
                self.password.clone().unwrap_or_default(),
            ),
        )
        .await;
        match connected {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(map_smb_error(&error)),
            Err(_) => return Err(SourceError::Unreachable),
        }
        let client = Arc::new(client);
        *slot = Some(Arc::clone(&client));
        Ok(client)
    }

    /// Discards the cached session after a transport failure.
    async fn invalidate(&self) {
        *self.client.lock().await = None;
    }

    /// Joins the source prefix with the relative path, refusing escapes.
    fn join(&self, relative: &str) -> SourceResult<String> {
        let mut segments = Vec::new();
        if !self.location.prefix.is_empty() {
            segments.push(self.location.prefix.clone());
        }
        for segment in relative.split('/') {
            if segment.is_empty() {
                continue;
            }
            if segment == "."
                || segment == ".."
                || segment.contains('\\')
                || segment.contains('\0')
                || segment.chars().any(char::is_control)
            {
                return Err(SourceError::NotFound);
            }
            segments.push(segment.to_owned());
        }
        Ok(segments.join("/"))
    }

    fn unc(&self, path: &str) -> UncPath {
        if path.is_empty() {
            self.location.base.clone()
        } else {
            self.location.base.clone().with_path(path)
        }
    }

    fn open_args() -> FileCreateArgs {
        FileCreateArgs::make_open_existing(
            FileAccessMask::new()
                .with_generic_read(true)
                .with_file_read_data(true)
                .with_file_read_attributes(true),
        )
    }

    /// Lists a directory, limiting items and time.
    pub(crate) async fn probe(&self) -> SourceResult<usize> {
        Ok(self.list_entries("").await?.len())
    }

    async fn list_entries(&self, directory: &str) -> SourceResult<Vec<SourceEntry>> {
        let result = timed(self.list_inner(directory)).await;
        if let Err(error) = result {
            self.note_failure(error).await;
            return Err(error);
        }
        result
    }

    async fn list_inner(&self, directory: &str) -> SourceResult<Vec<SourceEntry>> {
        let client = self.client().await?;
        let path = self.join(directory)?;
        let resource = client
            .create_file(&self.unc(&path), &Self::open_args())
            .await
            .map_err(|error| map_smb_error(&error))?;
        let directory = match resource {
            Resource::Directory(directory) => Arc::new(directory),
            _ => return Err(SourceError::NotFound),
        };
        let listing = async {
            let mut stream = Directory::query::<FileDirectoryInformation>(&directory, "*")
                .await
                .map_err(|error| map_smb_error(&error))?;
            let mut entries = Vec::new();
            while let Some(item) = stream.next().await {
                let item = item.map_err(|error| map_smb_error(&error))?;
                let name = item.file_name.to_string();
                if name.is_empty() || name == "." || name == ".." {
                    continue;
                }
                if name.contains('/')
                    || name.contains('\\')
                    || name.contains('\0')
                    || name.chars().any(char::is_control)
                {
                    return Err(SourceError::InvalidResponse);
                }
                if entries.len() >= MAX_DIRECTORY_ENTRIES {
                    return Err(SourceError::Limit);
                }
                entries.push(SourceEntry {
                    path: if path.is_empty() {
                        name
                    } else {
                        format!("{path}/{name}")
                    },
                    is_dir: item.file_attributes.directory(),
                    size: if item.file_attributes.directory() {
                        0
                    } else {
                        item.end_of_file
                    },
                    mtime: unix_seconds(*item.last_write_time),
                });
            }
            Ok(entries)
        }
        .await;
        let _ = directory.close().await;
        listing
    }

    async fn open_file(&self, path: &str) -> SourceResult<File> {
        let path = self.join(path)?;
        if path.is_empty() {
            return Err(SourceError::NotFound);
        }
        let client = self.client().await?;
        let resource = timed(async {
            client
                .create_file(&self.unc(&path), &Self::open_args())
                .await
                .map_err(|error| map_smb_error(&error))
        })
        .await?;
        match resource {
            Resource::File(file) => Ok(file),
            _ => Err(SourceError::NotFound),
        }
    }

    async fn read_inner(&self, path: &str, offset: u64, length: usize) -> SourceResult<Vec<u8>> {
        let file = self.open_file(path).await?;
        let result = async {
            let mut bytes = Vec::with_capacity(length.min(64 * 1024));
            let mut position = offset;
            let mut remaining = length;
            while remaining > 0 {
                let chunk = remaining.min(READ_CHUNK);
                let mut buffer = vec![0_u8; chunk];
                let read = timed(async {
                    file.read_at(&mut buffer, position)
                        .await
                        .map_err(|error| map_smb_error(&error))
                })
                .await?;
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                position += read as u64;
                remaining -= read;
            }
            Ok(bytes)
        }
        .await;
        let _ = file.close().await;
        result
    }

    /// Opens a media range as a stream, without loading the whole file.
    pub(crate) async fn open_stream(
        &self,
        path: &str,
        start: u64,
        length: u64,
    ) -> SourceResult<impl Stream<Item = Result<Vec<u8>, io::Error>> + Send + 'static> {
        let file = self.open_file(path).await?;
        let (sender, receiver) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let mut position = start;
            let mut remaining = length;
            while remaining > 0 {
                let chunk = remaining.min(READ_CHUNK as u64) as usize;
                let mut buffer = vec![0_u8; chunk];
                let read = match tokio::time::timeout(
                    OPERATION_TIMEOUT,
                    file.read_at(&mut buffer, position),
                )
                .await
                {
                    Ok(Ok(read)) => read,
                    Ok(Err(_)) | Err(_) => {
                        // The protocol message may name the server and the path;
                        // the HTTP body only carries a generic failure.
                        let _ = sender
                            .send(Err(io::Error::other("source read failed")))
                            .await;
                        break;
                    }
                };
                if read == 0 {
                    break;
                }
                buffer.truncate(read);
                if sender.send(Ok(buffer)).await.is_err() {
                    break;
                }
                position += read as u64;
                remaining -= read as u64;
            }
            let _ = file.close().await;
        });
        Ok(ReceiverStream::new(receiver))
    }

    async fn note_failure(&self, error: SourceError) {
        if matches!(
            error,
            SourceError::Unreachable | SourceError::InvalidResponse
        ) {
            self.invalidate().await;
        }
    }
}

impl CourseSource for SmbSource {
    fn list<'a>(
        &'a self,
        directory: &'a str,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<SourceEntry>>> + Send + 'a>> {
        Box::pin(self.list_entries(directory))
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
            if length == 0 {
                return Ok(Vec::new());
            }
            let result = self.read_inner(path, offset, length).await;
            if let Err(error) = result {
                self.note_failure(error).await;
                return Err(error);
            }
            result
        })
    }
}

/// Runs an operation with the maximum time of a listing or read block.
async fn timed<T>(future: impl Future<Output = SourceResult<T>>) -> SourceResult<T> {
    match tokio::time::timeout(OPERATION_TIMEOUT, future).await {
        Ok(result) => result,
        Err(_) => Err(SourceError::Unreachable),
    }
}

/// Translates a protocol failure into a sanitized source failure.
fn map_smb_error(error: &smb::Error) -> SourceError {
    use smb::Error::*;
    match error {
        UnexpectedMessageStatus(status) | ReceivedErrorMessage(status, _) => status_error(*status),
        MissingPermissions(_) | SspiError(_) => SourceError::Denied,
        NotFound(_) => SourceError::NotFound,
        InvalidArgument(_) | InvalidState(_) => SourceError::InvalidResponse,
        OperationTimeout(..) | IoError(_) | ConnectionStopped | TranformFailed(_)
        | TransportError(_) | ChannelNotFound(..) => SourceError::Unreachable,
        _ => SourceError::InvalidResponse,
    }
}

/// NTSTATUS of authentication, authorization and missing object, without details to the
/// client.
fn status_error(status: u32) -> SourceError {
    match status {
        0xC000_0022 // STATUS_ACCESS_DENIED
        | 0xC000_006D // STATUS_LOGON_FAILURE
        | 0xC000_006E // STATUS_ACCOUNT_RESTRICTION
        | 0xC000_0064 // STATUS_NO_SUCH_USER
        | 0xC000_00A2 // STATUS_PASSWORD_EXPIRED
        => SourceError::Denied,
        0xC000_0034 // STATUS_OBJECT_NAME_NOT_FOUND
        | 0xC000_003A // STATUS_OBJECT_PATH_NOT_FOUND
        | 0xC000_00CC // STATUS_BAD_NETWORK_NAME
        | 0xC000_00C9 // STATUS_NETWORK_NAME_DELETED
        => SourceError::NotFound,
        0xC000_0120 // STATUS_CANCELLED
        | 0xC000_020C // STATUS_CONNECTION_DISCONNECTED
        | 0xC000_0001 // STATUS_UNSUCCESSFUL
        => SourceError::Unreachable,
        _ => SourceError::InvalidResponse,
    }
}

#[cfg(test)]
mod tests {
    use super::{FILETIME_UNIX_OFFSET, SmbLocation, SmbSource, status_error, unix_seconds};
    use learnvault_core::{CourseSource, SourceError};
    use std::str::FromStr;

    fn parsed(input: &str) -> url::Url {
        crate::webdav::parse_source_url(input).unwrap()
    }

    #[test]
    fn file_times_become_unix_seconds() {
        let epoch = 1_445_412_480_i64;
        let ticks = (epoch + FILETIME_UNIX_OFFSET) * 10_000_000;
        assert_eq!(unix_seconds(ticks as u64), epoch);
        assert_eq!(unix_seconds(FILETIME_UNIX_OFFSET as u64 * 10_000_000), 0);
        assert_eq!(unix_seconds(0), 0);
    }

    #[test]
    fn location_splits_server_share_and_prefix() {
        let location = SmbLocation::from_url(&parsed("smb://nas.local/Cursos/Modulo%201")).unwrap();
        assert_eq!(location.server, "nas.local");
        assert_eq!(location.share, "Cursos");
        assert_eq!(location.prefix, "Modulo 1");
        assert_eq!(location.base.to_string(), r"\\nas.local\Cursos");

        let port = SmbLocation::from_url(&parsed("smb://192.0.2.10:1445/Curso")).unwrap();
        assert_eq!(port.server, "192.0.2.10:1445");
        assert_eq!(port.prefix, "");

        let ipv6 = SmbLocation::from_url(&parsed("smb://[::1]:1445/Curso")).unwrap();
        assert_eq!(ipv6.server, "[::1]:1445");
    }

    #[test]
    fn relative_paths_cannot_escape_the_prefix() {
        let source = SmbSource::new(parsed("smb://nas/Cursos/base"), None, None).unwrap();
        assert_eq!(source.join("").unwrap(), "base");
        assert_eq!(
            source.join("Modulo/aula.mp4").unwrap(),
            "base/Modulo/aula.mp4"
        );
        for path in ["../fora", "a/../../fora", "a\\b", "a\0b", "a/./b"] {
            assert_eq!(
                source.join(path).unwrap_err(),
                SourceError::NotFound,
                "{path}"
            );
        }
    }

    #[test]
    fn credentials_never_appear_in_debug() {
        let source = SmbSource::new(
            parsed("smb://nas/Cursos"),
            Some("learner"),
            Some("synthetic-secret"),
        )
        .unwrap();
        let debug = format!("{source:?}");
        assert!(!debug.contains("synthetic-secret"));
        assert!(debug.contains("has_password: true"));
        assert!(debug.contains("username: Some(\"learner\")"));
    }

    #[test]
    fn oversized_or_wrong_scheme_urls_are_refused() {
        assert!(SmbLocation::from_url(&parsed("dav://nas/Cursos")).is_err());
        let long = format!("smb://nas/Cursos/{}", "a".repeat(4096));
        assert!(crate::webdav::parse_source_url(&long).is_err());
    }

    #[test]
    fn ntstatus_maps_to_sanitized_errors() {
        assert_eq!(status_error(0xC000_0022), SourceError::Denied);
        assert_eq!(status_error(0xC000_006D), SourceError::Denied);
        assert_eq!(status_error(0xC000_0034), SourceError::NotFound);
        assert_eq!(status_error(0xC000_00CC), SourceError::NotFound);
        assert_eq!(status_error(0xC000_020C), SourceError::Unreachable);
        assert_eq!(status_error(0x1234_5678), SourceError::InvalidResponse);
    }

    #[test]
    fn the_unc_base_round_trips_through_the_client_parser() {
        let location = SmbLocation::from_url(&parsed("smb://nas/Cursos")).unwrap();
        let parsed = smb::UncPath::from_str(&location.base.to_string()).unwrap();
        assert_eq!(parsed.server(), "nas");
        assert_eq!(parsed.share(), Some("Cursos"));
    }

    #[tokio::test]
    async fn a_read_above_the_cap_is_refused_without_connecting() {
        let source = SmbSource::new(parsed("smb://nas/Cursos"), None, None).unwrap();
        assert_eq!(
            source
                .read_range("aula.mp4", 0, learnvault_core::MAX_SOURCE_READ + 1)
                .await
                .unwrap_err(),
            SourceError::Limit
        );
        assert!(
            source
                .read_range("aula.mp4", 0, 0)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
