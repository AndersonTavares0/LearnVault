//! Delivery of course materials and media through the webview's custom protocol.
//!
//! The delivery uses Tauri 2's own `URI scheme` protocol,
//! `learnvault-media://localhost/<media-id>`. Materials stay limited to small
//! responses; video, audio and subtitles use the same safe resolution by opaque identifier and
//! support single ranges, as HTML media elements require.
//!
//! # Security contract
//!
//! - the webview only supplies an **opaque identifier** (`media_id`); no path crosses the
//!   boundary. Parsing is strict: scheme and host exactly the ones registered, method
//!   `GET`/`HEAD`, path with a single opaque segment, no query, no fragment, no
//!   extra segments and with an identifier of safe characters;
//! - the `media_id` is resolved **only** through the persisted catalog, and the media is opened
//!   exactly once by [`learnvault_core::open_media`], which revalidates the root, the
//!   availability, the confinement under the root, the final component (symbolic link) and the
//!   identity of the open descriptor. The already-open [`std::fs::File`] is what is read;
//! - materials have the explicit limit of [`MAX_SMALL_FILE_BYTES`]; course media is delivered
//!   by ranges, with each response limited by [`MAX_MEDIA_RESPONSE_BYTES`];
//! - only allowlisted extensions and content types (materials, video, audio and subtitles) are
//!   served;
//! - no body or header carries a file-system path or an I/O error message:
//!   failures become only the status, with `Cache-Control: no-store`;
//! - the delivery is **read-only**: the only operation on the library is opening and reading the
//!   file.
//!
//! # Scope of the response
//!
//! `200`/`HEAD`/`206`/`416` are supported with a single-range `Range`. A multi-range
//! or unsatisfiable request becomes `416` with `Content-Range: bytes */<size>`. The successful
//! `Content-Range` is `bytes <start>-<end>/<size>`.

use std::borrow::Cow;
use std::io::{Read, Seek, SeekFrom};

use learnvault_core::{CourseRoot, Error, MediaKind, OpenMedia};
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, Url};

use crate::state::DesktopState;

/// Scheme of the desktop's custom media protocol.
pub const MEDIA_SCHEME: &str = "learnvault-media";

/// Canonical host of the custom protocol.
///
/// On all supported platforms the handler receives the URI already reverted to
/// `learnvault-media://localhost/...`: on Windows/Android the webview uses the prefix
/// `http://learnvault-media.localhost`, which the runtime reverts before calling the handler.
pub const MEDIA_HOST: &str = "localhost";

/// Label of the only window that may request the protocol.
pub const MAIN_WEBVIEW_LABEL: &str = "main";

/// Explicit response limit for small materials.
///
/// The response is buffered in memory by Tauri's API; files above this limit
/// (or a range that exceeds it) are refused without being read in full. Course media
/// (video, audio and subtitles) uses the response limit [`MAX_MEDIA_RESPONSE_BYTES`].
pub const MAX_SMALL_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Limit of bytes materialized in a single media response.
///
/// Tauri's protocol receives the body as a `Vec<u8>`, so both a complete response
/// and an explicit range must be limited before allocation.
pub const MAX_MEDIA_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// Maximum length accepted for the opaque media identifier.
const MAX_MEDIA_ID_LEN: usize = 128;

/// HTTP methods accepted by the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMethod {
    /// Request with a body.
    Get,
    /// Headers-only request.
    Head,
}

/// Raw request of the handler, already normalized into simple, testable fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMediaRequest<'a> {
    /// Method as sent by the webview.
    pub method: &'a str,
    /// Scheme of the URI.
    pub scheme: &'a str,
    /// Host of the URI, if any.
    pub host: Option<&'a str>,
    /// Path of the URI, starting with `/`.
    pub path: &'a str,
    /// Query of the URI, if any (always refused).
    pub query: Option<&'a str>,
    /// `Range` header, if any.
    pub range: Option<&'a str>,
    /// Label of the webview that originated the request.
    pub webview_label: &'a str,
}

/// Validated request: method, opaque identifier and optional range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRequest {
    /// Validated method.
    pub method: MediaMethod,
    /// Authorized opaque identifier, with no path.
    pub media_id: String,
    /// `Range` passed verbatim to the range validation.
    pub range: Option<String>,
}

/// Parsing refusal, before any file-system access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaRejection {
    /// The request did not come from the application's main window.
    Forbidden,
    /// Invalid scheme, host, path, query or identifier.
    Malformed,
    /// Method other than `GET`/`HEAD`.
    MethodNotAllowed,
}

impl MediaRejection {
    /// Stable HTTP status of the refusal.
    pub fn status(self) -> u16 {
        match self {
            MediaRejection::Forbidden => status::FORBIDDEN,
            MediaRejection::Malformed => status::BAD_REQUEST,
            MediaRejection::MethodNotAllowed => status::METHOD_NOT_ALLOWED,
        }
    }
}

/// Ready response of the protocol, knowing nothing of the Tauri runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaResponse {
    /// Status HTTP.
    pub status: u16,
    /// Headers, in emission order.
    pub headers: Vec<(String, String)>,
    /// Body of the response.
    pub body: Vec<u8>,
}

impl MediaResponse {
    /// Response with no body and no headers.
    ///
    /// Shared with the covers ([`crate::covers`]): the two deliveries of the custom
    /// protocol speak the same response contract and get the same CORS.
    pub(crate) fn bare(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Stable error response: only the status and `Cache-Control: no-store`.
    pub(crate) fn empty(status: u16) -> Self {
        let mut response = Self::bare(status);
        response.push("Cache-Control", "no-store");
        response
    }

    /// Adds a header.
    pub(crate) fn push(&mut self, name: &str, value: impl Into<String>) {
        self.headers.push((name.to_owned(), value.into()));
    }

    /// Value of a header, comparing the name case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Adds CORS only for the exact origin of the window that requested the media.
///
/// CORS controls the reading of pixels by the canvas; it is not authorization of the media and, so,
/// a missing or non-matching origin does not change the response's status or body.
///
/// No inherited `Access-Control-Allow-Origin` is reflected: the header is only emitted on an
/// exact match. Every request that carries `Origin` marks the response with
/// `Vary: Origin`, because the CORS header (and therefore the representation) depends on the origin
/// even when the ACAO is not emitted.
pub fn apply_media_cors(
    mut response: MediaResponse,
    request_origin: Option<&str>,
    window_origin: &str,
) -> MediaResponse {
    response
        .headers
        .retain(|(name, _)| !name.eq_ignore_ascii_case("Access-Control-Allow-Origin"));

    let Some(request_origin) = request_origin else {
        return response;
    };

    let has_origin_vary = response.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("Vary")
            && value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("Origin"))
    });
    if !has_origin_vary {
        if let Some((_, value)) = response
            .headers
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case("Vary"))
        {
            value.push_str(", Origin");
        } else {
            response
                .headers
                .push(("Vary".to_owned(), "Origin".to_owned()));
        }
    }

    let matches = parse_serialized_origin(request_origin)
        .zip(parse_serialized_origin(window_origin))
        .is_some_and(|(request, window)| request == window && request == window_origin);
    if matches && (200..300).contains(&response.status) {
        response.headers.push((
            "Access-Control-Allow-Origin".to_owned(),
            window_origin.to_owned(),
        ));
    }
    response
}

/// Validates a serialized origin with the URL parser and returns its canonical form.
fn parse_serialized_origin(origin: &str) -> Option<String> {
    if origin.is_empty() || origin.trim() != origin || origin.eq_ignore_ascii_case("null") {
        return None;
    }
    let url = Url::parse(origin).ok()?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (!url.path().is_empty() && url.path() != "/")
    {
        return None;
    }
    let parsed = url.origin().ascii_serialization();
    if parsed != "null" {
        return (parsed == origin).then_some(parsed);
    }
    let host = url.host_str()?;
    if !host.eq_ignore_ascii_case("localhost") || url.port().is_some() {
        return None;
    }
    let canonical = format!("{}://{}", url.scheme(), host.to_ascii_lowercase());
    (canonical == origin).then_some(canonical)
}

/// Extracts the origin from the window's URL without depending on a fixed origin per platform.
///
/// Shared with the covers ([`crate::covers`]), which apply the same exact-origin CORS.
pub(crate) fn window_url_origin(url: &Url) -> Option<String> {
    let serialized = url.origin().ascii_serialization();
    if serialized != "null" {
        return Some(serialized);
    }
    let host = url.host_str()?;
    (host.eq_ignore_ascii_case("localhost") && url.port().is_none())
        .then(|| format!("{}://{}", url.scheme(), host.to_ascii_lowercase()))
}

/// HTTP statuses used by the protocol.
mod status {
    /// Invalid request.
    pub const BAD_REQUEST: u16 = 400;
    /// Request from another window.
    pub const FORBIDDEN: u16 = 403;
    /// Media missing, unavailable, removed or with no root configured.
    pub const NOT_FOUND: u16 = 404;
    /// Unsupported method.
    pub const METHOD_NOT_ALLOWED: u16 = 405;
    /// File above the response limit.
    pub const PAYLOAD_TOO_LARGE: u16 = 413;
    /// Content type outside the allowlist.
    pub const UNSUPPORTED_MEDIA_TYPE: u16 = 415;
    /// Unsatisfiable range.
    pub const RANGE_NOT_SATISFIABLE: u16 = 416;
    /// Internal failure of reading or of state.
    pub const INTERNAL_SERVER_ERROR: u16 = 500;
    /// Full response.
    pub const OK: u16 = 200;
    /// Partial response.
    pub const PARTIAL_CONTENT: u16 = 206;
}

/// Allowlist of extensions and their content types.
const ALLOWED_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("avif", "image/avif"),
    ("pdf", "application/pdf"),
    ("txt", "text/plain; charset=utf-8"),
    ("md", "text/markdown; charset=utf-8"),
    ("mp4", "video/mp4"),
    ("m4v", "video/mp4"),
    ("mov", "video/quicktime"),
    ("webm", "video/webm"),
    ("ogv", "video/ogg"),
    ("mkv", "video/x-matroska"),
    ("avi", "video/x-msvideo"),
    ("wmv", "video/x-ms-wmv"),
    ("flv", "video/x-flv"),
    ("m3u8", "application/vnd.apple.mpegurl"),
    ("ts", "video/mp2t"),
    ("mp3", "audio/mpeg"),
    ("wav", "audio/wav"),
    ("ogg", "audio/ogg"),
    ("oga", "audio/ogg"),
    ("m4a", "audio/mp4"),
    ("flac", "audio/flac"),
    ("aac", "audio/aac"),
    ("opus", "audio/opus"),
    ("weba", "audio/webm"),
    ("wma", "audio/x-ms-wma"),
    ("vtt", "text/vtt"),
    ("srt", "application/x-subrip"),
    ("ass", "text/x-ass"),
    ("ssa", "text/x-ssa"),
    ("sub", "text/plain; charset=utf-8"),
];

/// Validates and normalizes the raw request, without touching the file system.
///
/// It refuses anything that is not exactly the protocol registered for the main
/// window: another scheme, another host, a query, a fragment (absent from the path), a path
/// with more than one segment and identifiers outside the safe character set.
pub fn parse_media_request(raw: &RawMediaRequest<'_>) -> Result<MediaRequest, MediaRejection> {
    if raw.webview_label != MAIN_WEBVIEW_LABEL {
        return Err(MediaRejection::Forbidden);
    }

    let method = match raw.method {
        "GET" => MediaMethod::Get,
        "HEAD" => MediaMethod::Head,
        _ => return Err(MediaRejection::MethodNotAllowed),
    };

    if raw.scheme != MEDIA_SCHEME || raw.host != Some(MEDIA_HOST) || raw.query.is_some() {
        return Err(MediaRejection::Malformed);
    }

    let Some(media_id) = raw.path.strip_prefix('/') else {
        return Err(MediaRejection::Malformed);
    };
    if !valid_media_id(media_id) {
        return Err(MediaRejection::Malformed);
    }

    Ok(MediaRequest {
        method,
        media_id: media_id.to_owned(),
        range: raw.range.map(str::to_owned),
    })
}

/// A valid opaque identifier is a single segment of safe characters.
///
/// The real identifier is a UUID generated by the domain, but the validation accepts the
/// safe alphabet without decoding percent-encoding: a `%` would indicate an attempt to
/// escape the canonical form and is refused, never decoded.
fn valid_media_id(media_id: &str) -> bool {
    !media_id.is_empty()
        && media_id.len() <= MAX_MEDIA_ID_LEN
        && media_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

/// Registers the media protocol on the Tauri builder.
///
/// The protocol is global, but each request is restricted to the main window by the
/// webview's label — the handler refuses any other origin. No capability permission is
/// granted: the ACL stays deny-by-default.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_uri_scheme_protocol(MEDIA_SCHEME, |context, request| {
        let state = context.app_handle().state::<DesktopState>();
        let raw = raw_request(context.webview_label(), &request);
        let request_origin = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok());
        let window_origin = context
            .app_handle()
            .get_webview_window(context.webview_label())
            .and_then(|window| window.url().ok())
            .and_then(|url| window_url_origin(&url));
        let response = state.serve_media(&raw);
        let response = match window_origin.as_deref() {
            Some(origin) => apply_media_cors(response, request_origin, origin),
            None => response,
        };
        http_response(response)
    })
}

/// Builds the raw request from Tauri's HTTP request.
fn raw_request<'a>(webview_label: &'a str, request: &'a Request<Vec<u8>>) -> RawMediaRequest<'a> {
    let uri = request.uri();
    RawMediaRequest {
        method: request.method().as_str(),
        scheme: uri.scheme_str().unwrap_or_default(),
        host: uri.host(),
        path: uri.path(),
        query: uri.query(),
        range: request
            .headers()
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok()),
        webview_label,
    }
}

/// Converts the protocol's response into the runtime's HTTP response.
pub(crate) fn http_response(response: MediaResponse) -> Response<Cow<'static, [u8]>> {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = Response::builder().status(status);
    for (name, value) in &response.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder
        .body(Cow::Owned(response.body))
        .unwrap_or_else(|_| fallback_response())
}

/// Minimal response used only if building the headers fails (it never happens with
/// our own values, but it keeps the handler free of `panic`).
fn fallback_response() -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Cow::Owned(Vec::new()))
        .expect("uma resposta de erro vazia é sempre construível")
}

impl DesktopState {
    /// Delivers a media authorized by the custom protocol.
    ///
    /// Parsing is pure and prefers the stable refusal; only after the request is validated are the
    /// configured root and the catalog queried. With no root configured, the response is
    /// `404`, exposing no state at all.
    pub fn serve_media(&self, raw: &RawMediaRequest<'_>) -> MediaResponse {
        match parse_media_request(raw) {
            Ok(request) => self.open_media_request(&request),
            Err(rejection) => MediaResponse::empty(rejection.status()),
        }
    }

    /// Opens the authorized media once and assembles the response.
    fn open_media_request(&self, request: &MediaRequest) -> MediaResponse {
        let Ok(database) = self.lock_database() else {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        };
        // The opaque identity links the media to the registered course's library; with no matching
        // course, the response is `404`, exposing no state at all.
        let Some(library_id) =
            (match learnvault_core::find_media_library_id(&database, &request.media_id) {
                Ok(library_id) => library_id,
                Err(_) => return MediaResponse::empty(status::INTERNAL_SERVER_ERROR),
            })
        else {
            return MediaResponse::empty(status::NOT_FOUND);
        };
        let Some(course) = self.course(&library_id) else {
            return MediaResponse::empty(status::NOT_FOUND);
        };
        // The persisted root is revalidated immediately before the open, preserving the
        // import's semantics: a folder removed, inaccessible or replaced by a
        // link/junction is refused, without following the target.
        let Ok(root) = CourseRoot::resolve(course.root.as_path()) else {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        };

        // The single open revalidates root, availability, confinement, symbolic link and
        // descriptor identity. The already-open `File` is what will be read.
        let opened = match learnvault_core::open_media(
            &database,
            &library_id,
            &request.media_id,
            root.as_path(),
        ) {
            Ok(opened) => opened,
            Err(error) => return MediaResponse::empty(media_error_status(&error)),
        };

        let Some(content_type) = content_type(&opened.name) else {
            return MediaResponse::empty(status::UNSUPPORTED_MEDIA_TYPE);
        };
        if opened.kind == MediaKind::Material && opened.size > MAX_SMALL_FILE_BYTES {
            return MediaResponse::empty(status::PAYLOAD_TOO_LARGE);
        }

        build_response(request, opened, content_type)
    }
}

/// Assembles the success response (`200`/`206`) from the already-open media.
fn build_response(
    request: &MediaRequest,
    opened: OpenMedia,
    content_type: &'static str,
) -> MediaResponse {
    let size = opened.size;
    let range = match single_range(request.range.as_deref(), size) {
        Ok(range) => range,
        Err(()) => return range_not_satisfiable(size),
    };
    let partial = range.is_some();
    let (start, mut end) = range.unwrap_or((0, size.saturating_sub(1)));
    let mut length = if size == 0 { 0 } else { end - start + 1 };

    if partial && length > MAX_MEDIA_RESPONSE_BYTES {
        end = start.saturating_add(MAX_MEDIA_RESPONSE_BYTES - 1);
        length = MAX_MEDIA_RESPONSE_BYTES;
    } else if !partial && request.method == MediaMethod::Get && length > MAX_MEDIA_RESPONSE_BYTES {
        return MediaResponse::empty(status::PAYLOAD_TOO_LARGE);
    }

    let mut body = Vec::new();
    if request.method == MediaMethod::Get && length > 0 {
        let mut file = opened.file;
        if file.seek(SeekFrom::Start(start)).is_err() {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        }
        let Ok(buffer_length) = usize::try_from(length) else {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        };
        let mut buffer = vec![0u8; buffer_length];
        if file.read_exact(&mut buffer).is_err() {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        }
        body = buffer;
    }

    let mut response = MediaResponse::bare(if partial {
        status::PARTIAL_CONTENT
    } else {
        status::OK
    });
    response.push("Accept-Ranges", "bytes");
    response.push("Content-Type", content_type);
    response.push("Content-Length", length.to_string());
    response.push("Cache-Control", "no-store");
    response.push("X-Content-Type-Options", "nosniff");
    if partial {
        response.push("Content-Range", format!("bytes {start}-{end}/{size}"));
    }
    response.body = body;
    response
}

/// Validates a single-range `Range`.
///
/// It returns `Ok(None)` when there is no `Range`, `Ok(Some((start, end)))` for a valid
/// range and `Err(())` when the range is multi-range, empty, malformed or unsatisfiable.
fn single_range(value: Option<&str>, size: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(value) = value else {
        return Ok(None);
    };
    let spec = value.strip_prefix("bytes=").ok_or(())?;
    if spec.contains(',') || size == 0 {
        return Err(());
    }
    let (start, end) = spec.split_once('-').ok_or(())?;
    if start.is_empty() {
        let suffix: u64 = end.parse().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        let length = suffix.min(size);
        return Ok(Some((size - length, size - 1)));
    }
    let start: u64 = start.parse().map_err(|_| ())?;
    if start >= size {
        return Err(());
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(size - 1)
    };
    if start > end {
        return Err(());
    }
    Ok(Some((start, end)))
}

/// Stable `416` response, with the total-size probe and no body.
fn range_not_satisfiable(size: u64) -> MediaResponse {
    let mut response = MediaResponse::empty(status::RANGE_NOT_SATISFIABLE);
    response.push("Content-Range", format!("bytes */{size}"));
    response
}

/// Translates the core's error into a stable status, without leaking the cause.
///
/// Legitimate absence becomes `404`; integrity and operational failures become `500`, as in the
/// HTTP adapter, so as not to confuse real absence with an inconsistent catalog/system.
fn media_error_status(error: &Error) -> u16 {
    match error {
        Error::MediaNotFound { .. }
        | Error::MediaUnavailable { .. }
        | Error::MediaMissingOnDisk { .. }
        | Error::MediaRemoved { .. }
        | Error::LibraryRootMissing { .. } => status::NOT_FOUND,
        _ => status::INTERNAL_SERVER_ERROR,
    }
}

/// Content type allowed for a file name, or `None` if outside the allowlist.
fn content_type(name: &str) -> Option<&'static str> {
    let (_, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    ALLOWED_TYPES
        .iter()
        .find(|(candidate, _)| *candidate == extension.as_str())
        .map(|(_, mime)| *mime)
}
