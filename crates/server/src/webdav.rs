//! Read-only WebDAV client.
//!
//! Discovery uses `PROPFIND` with `Depth: 1` and reading uses `GET` with `Range`. No write
//! request (`PUT`, `MKCOL`, `MOVE`, `DELETE`, `PROPPATCH`) is issued. TLS certificates are
//! always verified; there is no option to accept an invalid certificate.

use std::{future::Future, pin::Pin, sync::OnceLock, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use learnvault_core::{CourseSource, MAX_SOURCE_READ, SourceEntry, SourceError, SourceResult};
use percent_encoding::percent_decode_str;
use quick_xml::{Reader, XmlVersion, escape::unescape, events::Event};
use reqwest::{
    Client,
    header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue, RANGE},
};
use url::Url;

/// Largest source URL accepted, in characters.
const MAX_SOURCE_URL_CHARS: usize = 2048;
/// Largest body accepted from a `PROPFIND` response.
const MAX_PROPFIND_BYTES: usize = 4 * 1024 * 1024;
/// Maximum items accepted in a listing.
const MAX_PROPFIND_ENTRIES: usize = 100_000;
/// Maximum time to establish the TLS/TCP connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum time without receiving data from a response.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:resourcetype/>
    <d:getcontentlength/>
  </d:prop>
</d:propfind>"#;

/// Failure to parse the URL of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SourceUrlError {
    /// The scheme is not WebDAV, HTTP(S) or SMB.
    #[error("unsupported scheme")]
    UnsupportedScheme,
    /// The URL could not be parsed.
    #[error("invalid url")]
    InvalidUrl,
    /// The URL has no host.
    #[error("missing host")]
    MissingHost,
    /// The SMB path has no share name.
    #[error("missing share")]
    MissingShare,
    /// The URL carries credentials in plain text, which would leak into logs.
    #[error("credentials in url")]
    CredentialsInUrl,
    /// The URL carries a query or fragment, which do not identify a directory.
    #[error("query or fragment in url")]
    QueryOrFragment,
    /// A path segment would try to leave the root.
    #[error("path traversal in url")]
    Traversal,
}

/// Parses and normalizes the URL of a source.
///
/// `dav://` becomes `http://` and `davs://` becomes `https://`. The trailing slash is
/// guaranteed so that `PROPFIND` `href` resolution stays inside the base directory. In
/// `smb://host/share/path` the share is mandatory and each segment is validated, because
/// it becomes a UNC path.
pub(crate) fn parse_source_url(input: &str) -> Result<Url, SourceUrlError> {
    let input = input.trim();
    if input.is_empty() || input.chars().count() > MAX_SOURCE_URL_CHARS {
        return Err(SourceUrlError::InvalidUrl);
    }
    let (scheme, rest) = input.split_once("://").ok_or(SourceUrlError::InvalidUrl)?;
    let scheme = match scheme.to_ascii_lowercase().as_str() {
        "dav" | "http" => "http",
        "davs" | "https" => "https",
        "smb" => "smb",
        _ => return Err(SourceUrlError::UnsupportedScheme),
    };
    // The URL parser normalizes `.` and `..` segments before exposing them; the traversal
    // check must happen over the original text.
    if scheme == "smb" {
        reject_smb_traversal(rest)?;
    }
    let mut url =
        Url::parse(&format!("{scheme}://{rest}")).map_err(|_| SourceUrlError::InvalidUrl)?;
    if url.host_str().is_none() {
        return Err(SourceUrlError::MissingHost);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(SourceUrlError::CredentialsInUrl);
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(SourceUrlError::QueryOrFragment);
    }
    if url.scheme() == "smb" {
        normalize_smb_url(&mut url)?;
    } else if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

/// Refuses `..`, `%2e%2e`, backslashes and NUL in any segment of the text.
fn reject_smb_traversal(rest: &str) -> Result<(), SourceUrlError> {
    let mut parts = rest.split('/');
    let _host = parts.next();
    for raw in parts {
        let decoded = percent_decode_str(raw)
            .decode_utf8()
            .map_err(|_| SourceUrlError::InvalidUrl)?;
        if matches!(decoded.as_ref(), "." | "..")
            || decoded.contains('/')
            || decoded.contains('\\')
            || decoded.contains('\0')
            || decoded.chars().any(char::is_control)
        {
            return Err(SourceUrlError::Traversal);
        }
    }
    Ok(())
}

/// Requires a host, a share and at most one clean relative path.
fn normalize_smb_url(url: &mut Url) -> Result<(), SourceUrlError> {
    let mut segments = Vec::new();
    for segment in url.path().split('/') {
        if segment.is_empty() {
            continue;
        }
        let decoded = percent_decode_str(segment)
            .decode_utf8()
            .map_err(|_| SourceUrlError::InvalidUrl)?;
        if decoded.is_empty() || decoded.chars().any(char::is_control) {
            return Err(SourceUrlError::InvalidUrl);
        }
        segments.push(decoded.into_owned());
    }
    if segments.is_empty() {
        return Err(SourceUrlError::MissingShare);
    }
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| SourceUrlError::InvalidUrl)?;
        path.clear();
        for segment in &segments {
            path.push(segment);
        }
    }
    Ok(())
}

/// WebDAV client of a source.
#[derive(Clone)]
pub(crate) struct WebDavClient {
    base: Url,
    authorization: Option<HeaderValue>,
}

impl std::fmt::Debug for WebDavClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebDavClient")
            .field("base", &self.base)
            .field("has_credentials", &self.authorization.is_some())
            .finish()
    }
}

impl WebDavClient {
    /// Creates the client for an already normalized base.
    pub(crate) fn new(
        base: Url,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self, SourceError> {
        if !matches!(base.scheme(), "http" | "https") {
            return Err(SourceError::InvalidResponse);
        }
        let authorization = match (username, password) {
            (Some(username), Some(password)) => {
                let encoded = STANDARD.encode(format!("{username}:{password}"));
                Some(
                    HeaderValue::from_str(&format!("Basic {encoded}"))
                        .map_err(|_| SourceError::InvalidResponse)?,
                )
            }
            _ => None,
        };
        Ok(Self {
            base,
            authorization,
        })
    }

    fn directory_url(&self, directory: &str) -> Result<Url, SourceError> {
        file_url(&self.base, directory).map(|mut url| {
            url.set_path(&format!("{}/", url.path().trim_end_matches('/')));
            url
        })
    }

    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.authorization {
            Some(value) => request.header(AUTHORIZATION, value.clone()),
            None => request,
        }
    }

    /// Lists a directory with `PROPFIND Depth: 1`, excluding the directory itself.
    pub(crate) async fn list_directory(&self, directory: &str) -> SourceResult<Vec<SourceEntry>> {
        let directory = directory.trim_matches('/');
        let url = self.directory_url(directory)?;
        let response = self
            .authorize(
                client().map_err(|_| SourceError::Unreachable)?.request(
                    reqwest::Method::from_bytes(b"PROPFIND")
                        .map_err(|_| SourceError::InvalidResponse)?,
                    url.clone(),
                ),
            )
            .header("depth", "1")
            .header(CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(PROPFIND_BODY)
            .timeout(READ_TIMEOUT)
            .send()
            .await
            .map_err(map_request_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(status_error(status));
        }
        let body = read_limited(response, MAX_PROPFIND_BYTES).await?;
        let entries = parse_propfind(&body)?;
        let mut listed = Vec::with_capacity(entries.len());
        for entry in entries {
            let Some(path) = href_to_relative(&self.base, &url, &entry.href)? else {
                continue;
            };
            // `Depth: 1` of a subdirectory returns the directory itself; it already is the
            // root of this listing and re-queueing it would make the scan spin forever.
            if path == directory {
                continue;
            }
            if listed.len() >= MAX_PROPFIND_ENTRIES {
                return Err(SourceError::Limit);
            }
            listed.push(SourceEntry {
                path,
                is_dir: entry.is_dir,
                size: entry.size.unwrap_or(0),
                // WebDAV exposes `getlastmodified` as text; the scan treats `0` as
                // unknown, which is preferable to a misparsed date.
                mtime: 0,
            });
        }
        Ok(listed)
    }

    /// Opens a file range for streaming.
    ///
    /// Requires `206 Partial Content`: a server that ignores `Range` cannot be used for
    /// playback with seeking, and accepting the whole body would deliver bytes outside the
    /// requested range.
    pub(crate) async fn open_range(
        &self,
        path: &str,
        start: u64,
        end: u64,
    ) -> SourceResult<reqwest::Response> {
        let url = file_url(&self.base, path)?;
        let response = self
            .authorize(client().map_err(|_| SourceError::Unreachable)?.get(url))
            .header(RANGE, format!("bytes={start}-{end}"))
            .send()
            .await
            .map_err(map_request_error)?;
        match response.status() {
            reqwest::StatusCode::PARTIAL_CONTENT => Ok(response),
            reqwest::StatusCode::RANGE_NOT_SATISFIABLE => Err(SourceError::NotFound),
            status if status.is_success() => Err(SourceError::InvalidResponse),
            status => Err(status_error(status)),
        }
    }

    /// Confirms reachability, authentication and directory read.
    pub(crate) async fn probe(&self) -> SourceResult<usize> {
        Ok(self.list_directory("").await?.len())
    }
}

impl CourseSource for WebDavClient {
    fn list<'a>(
        &'a self,
        directory: &'a str,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<SourceEntry>>> + Send + 'a>> {
        Box::pin(self.list_directory(directory))
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
            let mut response = self
                .open_range(path, offset, offset + length as u64 - 1)
                .await?;
            let mut bytes = Vec::with_capacity(length.min(64 * 1024));
            while let Some(chunk) = response.chunk().await.map_err(map_request_error)? {
                if bytes.len() + chunk.len() > length {
                    return Err(SourceError::InvalidResponse);
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        })
    }
}

fn client() -> Result<&'static Client, ()> {
    static CLIENT: OnceLock<Result<Client, ()>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            // Same provider as the rest of the server: `ring`, without OpenSSL or CMake.
            let _ = rustls::crypto::ring::default_provider().install_default();
            Client::builder()
                .connect_timeout(CONNECT_TIMEOUT)
                .read_timeout(READ_TIMEOUT)
                // Cross-origin redirects would take the authorization header to another
                // host. The source must point to the final URL.
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| ())
        })
        .as_ref()
        .map_err(|_| ())
}

fn map_request_error(_: reqwest::Error) -> SourceError {
    SourceError::Unreachable
}

fn status_error(status: reqwest::StatusCode) -> SourceError {
    match status.as_u16() {
        401 | 403 => SourceError::Denied,
        404 => SourceError::NotFound,
        _ => SourceError::Unreachable,
    }
}

async fn read_limited(response: reqwest::Response, limit: usize) -> SourceResult<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(SourceError::Limit);
    }
    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_request_error)? {
        if body.len() + chunk.len() > limit {
            return Err(SourceError::Limit);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Builds the URL of a relative path of the source.
fn file_url(base: &Url, path: &str) -> Result<Url, SourceError> {
    let mut url = base.clone();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| SourceError::InvalidResponse)?;
        segments.pop_if_empty();
        for segment in path.split('/').filter(|segment| !segment.is_empty()) {
            if segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.contains('\\')
                || segment.contains('\0')
            {
                return Err(SourceError::InvalidResponse);
            }
            segments.push(segment);
        }
    }
    Ok(url)
}

pub(crate) fn decoded_segments(url: &Url) -> SourceResult<Vec<String>> {
    url.path_segments()
        .ok_or(SourceError::InvalidResponse)?
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            percent_decode_str(segment)
                .decode_utf8()
                .map(|decoded| decoded.into_owned())
                .map_err(|_| SourceError::InvalidResponse)
        })
        .collect()
}

/// Converts a `PROPFIND` `href` into the path relative to the base.
///
/// The `href` is resolved against the queried URL (which may be a subdirectory) and then
/// trimmed against the source base. An `href` outside the base or with segments that try
/// to escape it is ignored, never accepted as a path.
fn href_to_relative(base: &Url, request_url: &Url, href: &str) -> SourceResult<Option<String>> {
    let resolved = request_url
        .join(href.trim())
        .map_err(|_| SourceError::InvalidResponse)?;
    if resolved.scheme() != base.scheme()
        || resolved.host_str() != base.host_str()
        || resolved.port_or_known_default() != base.port_or_known_default()
    {
        return Ok(None);
    }
    let base_segments = decoded_segments(base)?;
    let resolved_segments = decoded_segments(&resolved)?;
    if !resolved_segments.starts_with(&base_segments) {
        return Ok(None);
    }
    let relative = &resolved_segments[base_segments.len()..];
    if relative.is_empty() {
        return Ok(None);
    }
    if relative.iter().any(|segment| {
        segment == "." || segment == ".." || segment.contains('/') || segment.contains('\0')
    }) {
        return Ok(None);
    }
    Ok(Some(relative.join("/")))
}

/// Raw item of a `multistatus`, before resolution against the base.
#[derive(Debug, PartialEq, Eq)]
struct PropfindEntry {
    href: String,
    is_dir: bool,
    size: Option<u64>,
}

/// Parses a `PROPFIND` `multistatus`.
fn parse_propfind(xml: &[u8]) -> SourceResult<Vec<PropfindEntry>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut entries = Vec::new();
    let mut current: Option<PropfindEntry> = None;
    let mut capture: Option<Capture> = None;
    let mut captured = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match event.local_name().as_ref() {
                "response" => {
                    current = Some(PropfindEntry {
                        href: String::new(),
                        is_dir: false,
                        size: None,
                    });
                }
                "href" => {
                    capture = Some(Capture::Href);
                    captured.clear();
                }
                "getcontentlength" => {
                    capture = Some(Capture::Size);
                    captured.clear();
                }
                "collection" => {
                    if let Some(entry) = current.as_mut() {
                        entry.is_dir = true;
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(event)) => {
                if event.local_name().as_ref() == "collection"
                    && let Some(entry) = current.as_mut()
                {
                    entry.is_dir = true;
                }
            }
            Ok(Event::Text(text)) => {
                if capture.is_some() {
                    captured.push_str(&text.xml_content(XmlVersion::Explicit1_0));
                }
            }
            Ok(Event::CData(text)) => {
                if capture.is_some() {
                    captured.push_str(&text.xml_content(XmlVersion::Explicit1_0));
                }
            }
            Ok(Event::End(event)) => match event.local_name().as_ref() {
                "href" => {
                    if let (Some(entry), Some(Capture::Href)) = (current.as_mut(), capture.take()) {
                        entry.href = unescape(&captured)
                            .map_err(|_| SourceError::InvalidResponse)?
                            .into_owned();
                    }
                }
                "getcontentlength" => {
                    if let (Some(entry), Some(Capture::Size)) = (current.as_mut(), capture.take()) {
                        entry.size = captured.trim().parse().ok();
                    }
                }
                "response" => {
                    if let Some(mut entry) = current.take() {
                        if entry.href.ends_with('/') {
                            entry.is_dir = true;
                        }
                        if !entry.href.is_empty() {
                            entries.push(entry);
                            if entries.len() > MAX_PROPFIND_ENTRIES {
                                return Err(SourceError::Limit);
                            }
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(SourceError::InvalidResponse),
        }
        buffer.clear();
    }

    Ok(entries)
}

#[derive(Clone, Copy)]
enum Capture {
    Href,
    Size,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::{WebDavClient, href_to_relative, parse_propfind, parse_source_url};
    use learnvault_core::{CourseSource, SourceError};

    fn base_url(address: std::net::SocketAddr) -> url::Url {
        url::Url::parse(&format!("http://{address}/dav/")).unwrap()
    }

    fn response(status: &str, headers: &[(&str, &str)], body: &str) -> Vec<u8> {
        let mut response = format!("HTTP/1.1 {status}\r\n");
        for (name, value) in headers {
            response.push_str(&format!("{name}: {value}\r\n"));
        }
        response.push_str(&format!("content-length: {}\r\n", body.len()));
        response.push_str("connection: close\r\n\r\n");
        response.push_str(body);
        response.into_bytes()
    }

    /// Starts a minimal HTTP server that serves one connection at a time.
    async fn server(
        handler: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let handler = Arc::new(handler);
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let handler = Arc::clone(&handler);
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    loop {
                        let mut chunk = [0_u8; 2048];
                        let Ok(read) = stream.read(&mut chunk).await else {
                            return;
                        };
                        if read == 0 {
                            return;
                        }
                        request.extend_from_slice(&chunk[..read]);
                        if request.windows(4).any(|window| window == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let request = String::from_utf8_lossy(&request);
                    let _ = stream.write_all(&handler(&request)).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        (address, task)
    }

    #[test]
    fn parses_dav_and_http_urls_into_a_normalized_base() {
        let cases = [
            ("dav://host/dav", "http://host/dav/"),
            ("davs://host/dav", "https://host/dav/"),
            ("https://host/dav/", "https://host/dav/"),
            ("http://host:8080/dav/sub", "http://host:8080/dav/sub/"),
            ("  https://host  ", "https://host/"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                parse_source_url(input).unwrap().as_str(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn parses_smb_urls_with_a_required_share_and_clean_segments() {
        let cases = [
            ("smb://nas/Cursos", "smb://nas/Cursos"),
            ("smb://nas/Cursos/", "smb://nas/Cursos"),
            (
                "smb://nas/Cursos/Modulo%201/aula.mp4",
                "smb://nas/Cursos/Modulo%201/aula.mp4",
            ),
            ("SMB://nas/Cursos", "smb://nas/Cursos"),
            ("smb://192.0.2.10:1445/Curso", "smb://192.0.2.10:1445/Curso"),
            ("smb://[::1]:1445/Curso", "smb://[::1]:1445/Curso"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                parse_source_url(input).unwrap().as_str(),
                expected,
                "{input}"
            );
        }
    }

    #[test]
    fn rejects_unsafe_smb_urls() {
        for input in [
            "smb://nas",
            "smb://nas/",
            "smb://user:secret@nas/Cursos",
            "smb://nas/Cursos?token=secret",
            "smb://nas/Cursos#fragment",
            "smb://nas/Cursos/../fora",
            "smb://nas/Cursos/%2e%2e/fora",
            "smb://nas/Cursos/a%2Fb",
            "smb://nas/Cursos/a%5Cb",
            "smbs://nas/Cursos",
            "smb://",
            "smb:///Cursos",
        ] {
            assert!(parse_source_url(input).is_err(), "{input}");
        }
        let long = format!("smb://nas/Cursos/{}", "a".repeat(4096));
        assert!(parse_source_url(&long).is_err());
    }

    #[test]
    fn rejects_unsupported_or_unsafe_source_urls() {
        for input in [
            "ftp://host/dav",
            "https://user:secret@host/dav",
            "https://host/dav?token=secret",
            "https://host/dav#fragment",
            "https://",
            "not-a-url",
        ] {
            assert!(parse_source_url(input).is_err(), "{input}");
        }
    }

    #[test]
    fn parses_a_multistatus_with_namespaces_and_percent_encoded_hrefs() {
        let xml = br#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/dav/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop>
      <D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/Curso%20novo/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/Curso%20novo/aula%20um.mp4</D:href>
    <D:propstat><D:prop>
      <D:resourcetype/>
      <D:getcontentlength> 1024 </D:getcontentlength>
    </D:prop></D:propstat>
  </D:response>
</D:multistatus>"#;

        let entries = parse_propfind(xml).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].href, "/dav/");
        assert!(entries[0].is_dir);
        assert!(entries[1].is_dir);
        assert!(!entries[2].is_dir);
        assert_eq!(entries[2].size, Some(1024));

        let base = url::Url::parse("http://host/dav/").unwrap();
        assert_eq!(
            href_to_relative(&base, &base, &entries[0].href).unwrap(),
            None
        );
        assert_eq!(
            href_to_relative(&base, &base, &entries[1].href)
                .unwrap()
                .as_deref(),
            Some("Curso novo")
        );
        assert_eq!(
            href_to_relative(&base, &base, &entries[2].href)
                .unwrap()
                .as_deref(),
            Some("Curso novo/aula um.mp4")
        );
    }

    #[test]
    fn ignores_hrefs_outside_the_base_and_rejects_escapes() {
        let base = url::Url::parse("http://host/dav/").unwrap();
        assert_eq!(
            href_to_relative(&base, &base, "https://outro/dav/x.mp4").unwrap(),
            None
        );
        assert_eq!(
            href_to_relative(&base, &base, "/outro/x.mp4").unwrap(),
            None
        );
        assert_eq!(
            href_to_relative(&base, &base, "/dav/../x.mp4").unwrap(),
            None
        );
        assert_eq!(
            href_to_relative(&base, &base, "/dav/%2e%2e/x.mp4").unwrap(),
            None
        );
        assert_eq!(
            href_to_relative(&base, &base, "/dav/a%2Fb.mp4").unwrap(),
            None
        );
    }

    #[test]
    fn malformed_xml_is_an_invalid_response() {
        assert_eq!(
            parse_propfind(b"<multistatus><response></multistatus>").unwrap_err(),
            SourceError::InvalidResponse
        );
    }

    #[tokio::test]
    async fn propfind_lists_children_with_depth_one_and_basic_auth() {
        let (address, task) = server(|request| {
            let lower = request.to_ascii_lowercase();
            assert!(lower.starts_with("propfind /dav/ "), "{request}");
            assert!(lower.contains("\r\ndepth: 1\r\n"), "{request}");
            assert!(
                lower.contains("\r\nauthorization: basic dxnlcjpzzwnyzxq=\r\n"),
                "{request}"
            );
            let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
 <D:response><D:href>/dav/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>
 <D:response><D:href>/dav/Modulo1/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>
 <D:response><D:href>/dav/Modulo1/aula.mp4</D:href><D:propstat><D:prop><D:resourcetype/><D:getcontentlength>10</D:getcontentlength></D:prop></D:propstat></D:response>
</D:multistatus>"#;
            response("200 OK", &[("content-type", "application/xml")], xml)
        })
        .await;

        let client = WebDavClient::new(base_url(address), Some("user"), Some("secret")).unwrap();
        let entries = client.list_directory("").await.unwrap();
        let summary = entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.is_dir, entry.size))
            .collect::<Vec<_>>();
        assert_eq!(
            summary,
            vec![("Modulo1", true, 0), ("Modulo1/aula.mp4", false, 10)]
        );
        task.abort();
    }

    #[tokio::test]
    async fn a_subdirectory_listing_does_not_repeat_itself() {
        let (address, task) = server(|request| {
            let lower = request.to_ascii_lowercase();
            assert!(lower.starts_with("propfind /dav/modulo1/ "), "{request}");
            let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
 <D:response><D:href>/dav/Modulo1/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>
 <D:response><D:href>/dav/Modulo1/aula.mp4</D:href><D:propstat><D:prop><D:resourcetype/><D:getcontentlength>10</D:getcontentlength></D:prop></D:propstat></D:response>
</D:multistatus>"#;
            response("207 Multi-Status", &[("content-type", "application/xml")], xml)
        })
        .await;

        let client = WebDavClient::new(base_url(address), None, None).unwrap();
        let entries = client.list_directory("Modulo1").await.unwrap();
        let paths = entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["Modulo1/aula.mp4"]);
        task.abort();
    }

    #[tokio::test]
    async fn read_range_sends_the_exact_range_and_returns_bytes() {
        let (address, task) = server(|request| {
            let lower = request.to_ascii_lowercase();
            assert!(lower.starts_with("get /dav/modulo1/aula.mp4 "), "{request}");
            assert!(lower.contains("\r\nrange: bytes=2-5\r\n"), "{request}");
            response(
                "206 Partial Content",
                &[("content-range", "bytes 2-5/10")],
                "2345",
            )
        })
        .await;

        let client = WebDavClient::new(base_url(address), None, None).unwrap();
        let bytes = client.read_range("Modulo1/aula.mp4", 2, 4).await.unwrap();
        assert_eq!(bytes, b"2345");
        task.abort();
    }

    #[tokio::test]
    async fn a_server_that_ignores_range_is_refused() {
        let (address, task) = server(|_| response("200 OK", &[], "0123456789")).await;
        let client = WebDavClient::new(base_url(address), None, None).unwrap();
        assert_eq!(
            client.read_range("aula.mp4", 2, 4).await.unwrap_err(),
            SourceError::InvalidResponse
        );
        task.abort();
    }

    #[tokio::test]
    async fn authentication_failure_is_denied_without_leaking_credentials() {
        let (address, task) =
            server(|_| response("401 Unauthorized", &[("www-authenticate", "Basic")], "")).await;
        let client =
            WebDavClient::new(base_url(address), Some("user"), Some("synthetic-secret")).unwrap();

        assert_eq!(
            client.list_directory("").await.unwrap_err(),
            SourceError::Denied
        );
        let debug = format!("{client:?}");
        assert!(!debug.contains("synthetic-secret"));
        assert!(!debug.contains("dXNlcjpzeW50aGV0aWMtc2VjcmV0"));
        task.abort();
    }

    #[tokio::test]
    async fn an_unreachable_host_maps_to_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let client = WebDavClient::new(base_url(address), None, None).unwrap();

        assert_eq!(
            client.list_directory("").await.unwrap_err(),
            SourceError::Unreachable
        );
    }
}
