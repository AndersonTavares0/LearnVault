//! Custom course covers on the desktop: native picking, validation and local delivery.
//!
//! The cover is a single image per course, kept in the application's own data directory
//! (`<app data>/covers/<course id>.cover`), never inside the course folder,
//! which stays external and read-only. The path never crosses the webview boundary: the
//! SPA asks for the cover through a scheme of its own (`learnvault-cover://localhost/<course id>`) and the
//! Tauri handler resolves the opaque identity to a file inside the covers directory.
//!
//! Security rules preserved here:
//!
//! - only PNG, JPEG and WebP are accepted, and the type is decided **only by the initial bytes**
//!   (magic bytes): no image decoder is shipped and the chosen file's extension
//!   decides nothing;
//! - the [`MAX_COVER_BYTES`] limit applies on picking and on reading, and the write is atomic
//!   (exclusive temporary file + `rename`), so no read observes a truncated cover;
//! - the course's identity is validated as a single safe path segment before
//!   any disk access, and the course must be registered;
//! - no response carries a file-system path: an absent cover is a `404` with no
//!   body and any internal failure is a `500` with no body, always with `Cache-Control: no-store`;
//! - the scheme's CORS is the same as media's ([`crate::media::apply_media_cors`]): it reflects
//!   only the window's exact origin, marks `Vary: Origin` when there is an `Origin` and never uses
//!   a wildcard or credentials. No capability permission is granted to the webview.

use std::fs::{self, Metadata, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use tauri::http::{Request, header};
use tauri::{Manager, Runtime};

use crate::media::{
    MAIN_WEBVIEW_LABEL, MediaResponse, apply_media_cors, http_response, window_url_origin,
};
use crate::state::DesktopState;

/// Scheme of the covers' custom protocol.
pub const COVER_SCHEME: &str = "learnvault-cover";

/// Canonical host of the custom protocol.
///
/// As with the media protocol, the handler always receives the URI already reverted to
/// `learnvault-cover://localhost/...`: on Windows/Android the webview uses the prefix
/// `http://learnvault-cover.localhost`, which the runtime reverts before calling the handler.
pub const COVER_HOST: &str = "localhost";

/// Name of the covers directory inside the application's data directory.
pub const COVERS_DIR_NAME: &str = "covers";

/// Maximum size accepted for a custom cover (4 MiB).
pub const MAX_COVER_BYTES: u64 = 4 * 1024 * 1024;

/// Suffix of a course's cover file inside the covers directory.
const COVER_FILE_SUFFIX: &str = ".cover";

/// Maximum length accepted for a course identity used as a path segment.
const MAX_COURSE_ID_LEN: usize = 128;

/// Image type recognized by the file's initial bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageType {
    /// PNG (`\x89PNG\r\n\x1a\n`).
    Png,
    /// JPEG (`\xFF\xD8\xFF`).
    Jpeg,
    /// WebP (`RIFF` + size + `WEBP`).
    Webp,
}

impl ImageType {
    /// `Content-Type` matching the detected type.
    pub const fn content_type(self) -> &'static str {
        match self {
            ImageType::Png => "image/png",
            ImageType::Jpeg => "image/jpeg",
            ImageType::Webp => "image/webp",
        }
    }
}

/// Stable failure of the cover operations. It never carries a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverError {
    /// The content was refused before touching the disk (unsupported type or above the limit).
    Rejected,
    /// The write or the removal could not complete.
    WriteFailed,
    /// The stored cover could not be read.
    Unreadable,
    /// The stored cover is not a supported image (corruption or tampering on disk).
    Corrupt,
}

/// Methods accepted by the covers protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverMethod {
    /// Request with a body.
    Get,
    /// Headers-only request.
    Head,
}

/// Parsing refusal, before any file-system access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverRejection {
    /// The request did not come from the application's main window.
    Forbidden,
    /// Invalid scheme, host, path, query or identity.
    Malformed,
    /// Method other than `GET`/`HEAD`.
    MethodNotAllowed,
}

impl CoverRejection {
    /// Stable HTTP status of the refusal.
    pub fn status(self) -> u16 {
        match self {
            CoverRejection::Forbidden => COVER_STATUS_FORBIDDEN,
            CoverRejection::Malformed => COVER_STATUS_BAD_REQUEST,
            CoverRejection::MethodNotAllowed => COVER_STATUS_METHOD_NOT_ALLOWED,
        }
    }
}

/// Raw request of the handler, already normalized into simple, testable fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCoverRequest<'a> {
    /// Method as sent by the webview.
    pub method: &'a str,
    /// Scheme of the URI.
    pub scheme: &'a str,
    /// Host of the URI, if any.
    pub host: Option<&'a str>,
    /// Path of the URI, starting with `/`.
    pub path: &'a str,
    /// Query of the URI, if any (always refused; the cover's version does not reach the handler).
    pub query: Option<&'a str>,
    /// Label of the webview that originated the request.
    pub webview_label: &'a str,
}

/// Validated request: method and the course's opaque identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverRequest {
    /// Validated method.
    pub method: CoverMethod,
    /// Authorized opaque identity, with no path.
    pub course_id: String,
}

/// HTTP statuses used by the covers protocol.
const COVER_STATUS_OK: u16 = 200;
/// Invalid request.
const COVER_STATUS_BAD_REQUEST: u16 = 400;
/// Request from another window.
const COVER_STATUS_FORBIDDEN: u16 = 403;
/// Missing cover or unknown course.
const COVER_STATUS_NOT_FOUND: u16 = 404;
/// Unsupported method.
const COVER_STATUS_METHOD_NOT_ALLOWED: u16 = 405;
/// Internal failure of reading or of state.
const COVER_STATUS_INTERNAL_SERVER_ERROR: u16 = 500;

/// Accepts only a safe path segment (the course's opaque identity).
///
/// The real identifier is `course-` + a UUID (hexadecimal and hyphens) or the legacy identity
/// `library-1`, so the safe alphabet covers all courses. A `%` would indicate an attempt
/// to escape the scheme's canonical form and is refused, never decoded.
pub fn valid_course_id(course_id: &str) -> bool {
    if course_id.is_empty() || course_id.len() > MAX_COURSE_ID_LEN {
        return false;
    }
    course_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Detects the image type only from the initial bytes.
pub fn detect_image_type(bytes: &[u8]) -> Option<ImageType> {
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF];
    const RIFF: &[u8] = b"RIFF";
    const WEBP: &[u8] = b"WEBP";

    if bytes.starts_with(PNG) {
        return Some(ImageType::Png);
    }
    if bytes.starts_with(JPEG) {
        return Some(ImageType::Jpeg);
    }
    // `RIFF<4 bytes of size>WEBP`: the four bytes between the markers are the container's
    // declared size and do not take part in identifying the format.
    if bytes.len() >= 12 && bytes.starts_with(RIFF) && &bytes[8..12] == WEBP {
        return Some(ImageType::Webp);
    }
    None
}

/// Physical path of a course's cover, or `None` when the identity is unsafe.
pub fn cover_file(covers_dir: &Path, course_id: &str) -> Option<PathBuf> {
    valid_course_id(course_id).then(|| covers_dir.join(format!("{course_id}{COVER_FILE_SUFFIX}")))
}

/// Version (mtime in seconds) of a course's cover, when it exists.
pub fn cover_version(covers_dir: &Path, course_id: &str) -> Option<i64> {
    let metadata = fs::symlink_metadata(cover_file(covers_dir, course_id)?).ok()?;
    if !metadata.is_file() {
        return None;
    }
    modified_seconds(&metadata)
}

/// Automatically detects a local cover at the root of the course folder and stores it in `covers_dir`.
///
/// It does not replace a cover already in `covers_dir`. It prioritizes the names `cover.*`, `folder.*`,
/// `poster.*`, `thumb.*` and `thumbnail.*` with supported extensions (`.jpg`, `.jpeg`, `.png`, `.webp`).
/// The course directory is read strictly read-only.
pub fn auto_detect_course_cover(
    covers_dir: &Path,
    course_id: &str,
    course_root: &Path,
) -> Result<bool, CoverError> {
    if !valid_course_id(course_id) {
        return Ok(false);
    }
    if cover_version(covers_dir, course_id).is_some() {
        return Ok(false);
    }
    let read_dir = match fs::read_dir(course_root) {
        Ok(iter) => iter,
        Err(_) => return Ok(false),
    };

    let priority_stems = ["cover", "folder", "poster", "thumb", "thumbnail"];
    let allowed_exts = ["jpg", "jpeg", "png", "webp"];
    let mut candidates: Vec<(usize, PathBuf)> = Vec::new();

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        let Some((stem, ext)) = lower.rsplit_once('.') else {
            continue;
        };
        if !allowed_exts.contains(&ext) {
            continue;
        }
        if let Some(priority) = priority_stems.iter().position(|&s| s == stem) {
            candidates.push((priority, path));
        }
    }

    candidates.sort_by_key(|(p, _)| *p);

    for (_, candidate_path) in candidates {
        let Ok(meta) = fs::symlink_metadata(&candidate_path) else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_COVER_BYTES {
            continue;
        }
        let Ok(bytes) = fs::read(&candidate_path) else {
            continue;
        };
        if detect_image_type(&bytes).is_some() {
            store_cover(covers_dir, course_id, &bytes)?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Writes the cover atomically, revalidating type and size before touching the disk.
pub fn store_cover(covers_dir: &Path, course_id: &str, bytes: &[u8]) -> Result<(), CoverError> {
    let Some(destination) = cover_file(covers_dir, course_id) else {
        return Err(CoverError::Rejected);
    };
    if bytes.len() as u64 > MAX_COVER_BYTES || detect_image_type(bytes).is_none() {
        return Err(CoverError::Rejected);
    }
    fs::create_dir_all(covers_dir).map_err(|_| CoverError::WriteFailed)?;

    static ATOMIC_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let counter = ATOMIC_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = covers_dir.join(format!(".{course_id}.{}.{counter}.tmp", std::process::id()));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(CoverError::WriteFailed);
    }
    if replace_file(&temporary, &destination).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(CoverError::WriteFailed);
    }
    Ok(())
}

/// Reads the cover and redetects the type from the bytes.
///
/// A path that is absent or not a regular file means no cover; a regular file
/// that is not a supported image (or is above the limit) is corruption.
pub fn read_cover(
    covers_dir: &Path,
    course_id: &str,
) -> Result<Option<(Vec<u8>, ImageType)>, CoverError> {
    let Some(path) = cover_file(covers_dir, course_id) else {
        return Ok(None);
    };
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CoverError::Unreadable),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > MAX_COVER_BYTES {
        return Err(CoverError::Corrupt);
    }
    let bytes = fs::read(&path).map_err(|_| CoverError::Unreadable)?;
    let Some(image_type) = detect_image_type(&bytes) else {
        return Err(CoverError::Corrupt);
    };
    Ok(Some((bytes, image_type)))
}

/// Removes the cover and reports whether anything was deleted.
pub fn remove_cover(covers_dir: &Path, course_id: &str) -> Result<bool, CoverError> {
    let Some(path) = cover_file(covers_dir, course_id) else {
        return Ok(false);
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(CoverError::WriteFailed),
    }
}

/// Reads the file chosen in the native picker, refusing above the limit before reading.
pub fn read_picked_cover(path: &Path) -> Result<Vec<u8>, CoverError> {
    let metadata = fs::metadata(path).map_err(|_| CoverError::Unreadable)?;
    if !metadata.is_file() || metadata.len() > MAX_COVER_BYTES {
        return Err(CoverError::Rejected);
    }
    let bytes = fs::read(path).map_err(|_| CoverError::Unreadable)?;
    if detect_image_type(&bytes).is_none() {
        return Err(CoverError::Rejected);
    }
    Ok(bytes)
}

/// Modification seconds of a file, when representable.
fn modified_seconds(metadata: &Metadata) -> Option<i64> {
    let modified = metadata.modified().ok()?;
    let seconds = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}

/// Replaces the destination using only standard APIs.
///
/// Outside Windows, `fs::rename` swaps the name on the same file system and replaces the
/// destination. On Windows, the standard API does not replace an existing destination: removing it
/// immediately before `rename` leaves a non-atomic window in which the destination may not
/// exist.
#[cfg(windows)]
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    match fs::remove_file(dest) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(source, dest)
}

/// Replaces the destination using only standard APIs.
#[cfg(not(windows))]
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    fs::rename(source, dest)
}

/// Validates and normalizes the raw request, without touching the file system.
///
/// It refuses anything that is not exactly the protocol registered for the main
/// window: another scheme, another host, a query other than `v=<digits>`, a path with more than
/// one segment and identities outside the safe alphabet.
pub fn parse_cover_request(raw: &RawCoverRequest<'_>) -> Result<CoverRequest, CoverRejection> {
    if raw.webview_label != MAIN_WEBVIEW_LABEL {
        return Err(CoverRejection::Forbidden);
    }

    let method = match raw.method {
        "GET" => CoverMethod::Get,
        "HEAD" => CoverMethod::Head,
        _ => return Err(CoverRejection::MethodNotAllowed),
    };

    if raw.scheme != COVER_SCHEME || raw.host != Some(COVER_HOST) || !valid_cover_version(raw.query)
    {
        return Err(CoverRejection::Malformed);
    }

    let Some(course_id) = raw.path.strip_prefix('/') else {
        return Err(CoverRejection::Malformed);
    };
    if !valid_course_id(course_id) {
        return Err(CoverRejection::Malformed);
    }

    Ok(CoverRequest {
        method,
        course_id: course_id.to_owned(),
    })
}

/// The cover's version is optional and can only arrive as `v=<digits>`.
///
/// The SPA uses the value to change the `<img>` URL when the cover changes; the handler ignores it
/// in the resolution and keeps answering with `Cache-Control: no-store`. Any other
/// query is refused instead of silently discarded.
fn valid_cover_version(query: Option<&str>) -> bool {
    let Some(query) = query else {
        return true;
    };
    let Some(version) = query.strip_prefix("v=") else {
        return false;
    };
    !version.is_empty() && version.len() <= 20 && version.bytes().all(|byte| byte.is_ascii_digit())
}

/// Registers the covers protocol on the Tauri builder.
///
/// The protocol is global, but each request is restricted to the main window by the
/// webview's label, and the CORS is the same as media's: the window's exact origin, never a wildcard or
/// credentials. No capability permission is granted: the ACL stays deny-by-default.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_uri_scheme_protocol(COVER_SCHEME, |context, request| {
        let state = context.app_handle().state::<DesktopState>();
        let raw = raw_cover_request(context.webview_label(), &request);
        let request_origin = request
            .headers()
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok());
        let window_origin = context
            .app_handle()
            .get_webview_window(context.webview_label())
            .and_then(|window| window.url().ok())
            .and_then(|url| window_url_origin(&url));
        let response = state.serve_cover(&raw);
        let response = match window_origin.as_deref() {
            Some(origin) => apply_media_cors(response, request_origin, origin),
            None => response,
        };
        http_response(response)
    })
}

/// Builds the raw request from Tauri's HTTP request.
fn raw_cover_request<'a>(
    webview_label: &'a str,
    request: &'a Request<Vec<u8>>,
) -> RawCoverRequest<'a> {
    let uri = request.uri();
    RawCoverRequest {
        method: request.method().as_str(),
        scheme: uri.scheme_str().unwrap_or_default(),
        host: uri.host(),
        path: uri.path(),
        query: uri.query(),
        webview_label,
    }
}

impl DesktopState {
    /// Delivers the cover authorized by the custom protocol.
    ///
    /// Parsing is pure and prefers the stable refusal; only after the request is validated are the
    /// course registry and the covers directory queried. An unregistered course
    /// answers like an absent cover, revealing nothing.
    pub fn serve_cover(&self, raw: &RawCoverRequest<'_>) -> MediaResponse {
        match parse_cover_request(raw) {
            Ok(request) => self.open_cover_request(&request),
            Err(rejection) => MediaResponse::empty(rejection.status()),
        }
    }

    /// Assembles the response for a registered course's cover.
    fn open_cover_request(&self, request: &CoverRequest) -> MediaResponse {
        if self.course(&request.course_id).is_none() {
            return MediaResponse::empty(COVER_STATUS_NOT_FOUND);
        }
        match read_cover(&self.covers_dir(), &request.course_id) {
            Ok(Some((bytes, image_type))) => {
                let mut response = MediaResponse::bare(COVER_STATUS_OK);
                response.push("Content-Type", image_type.content_type());
                response.push("Content-Length", bytes.len().to_string());
                response.push("Cache-Control", "no-store");
                response.push("X-Content-Type-Options", "nosniff");
                if request.method == CoverMethod::Get {
                    response.body = bytes;
                }
                response
            }
            Ok(None) => MediaResponse::empty(COVER_STATUS_NOT_FOUND),
            Err(_) => MediaResponse::empty(COVER_STATUS_INTERNAL_SERVER_ERROR),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        COVER_HOST, COVER_SCHEME, RawCoverRequest, cover_file, parse_cover_request,
        raw_cover_request, read_picked_cover,
    };
    use crate::media::MAIN_WEBVIEW_LABEL;

    #[test]
    fn raw_request_maps_the_tauri_uri_into_the_protocol_fields() {
        let request = tauri::http::Request::builder()
            .method("GET")
            .uri("learnvault-cover://localhost/course-1?v=7")
            .body(Vec::new())
            .unwrap();

        let raw = raw_cover_request(MAIN_WEBVIEW_LABEL, &request);

        assert_eq!(
            raw,
            RawCoverRequest {
                method: "GET",
                scheme: COVER_SCHEME,
                host: Some(COVER_HOST),
                path: "/course-1",
                query: Some("v=7"),
                webview_label: MAIN_WEBVIEW_LABEL,
            }
        );
        // The version in the query is accepted by the SPA as a cache-buster and does not change the resolution.
        assert_eq!(
            parse_cover_request(&raw).unwrap().course_id,
            "course-1".to_owned()
        );
    }

    #[test]
    fn cover_file_accepts_only_a_single_safe_segment() {
        let directory = Path::new("covers");

        assert_eq!(
            cover_file(directory, "course-1"),
            Some(directory.join("course-1.cover"))
        );
        for unsafe_id in ["", ".", "..", "a/b", "a\\b", "a\0b", "café", "a%2Fb", "a b"] {
            assert!(cover_file(directory, unsafe_id).is_none(), "{unsafe_id:?}");
        }
        assert!(cover_file(directory, &"a".repeat(129)).is_none());
    }

    #[test]
    fn read_picked_cover_rejects_directories_missing_files_and_non_images() {
        let temp = tempfile::tempdir().unwrap();

        // A chosen directory is not an image.
        assert!(read_picked_cover(temp.path()).is_err());
        assert!(read_picked_cover(&temp.path().join("nao-existe.png")).is_err());

        let text = temp.path().join("capa.png");
        std::fs::write(&text, b"nao e uma imagem").unwrap();
        assert!(read_picked_cover(&text).is_err());

        let png = temp.path().join("real.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
        assert_eq!(read_picked_cover(&png).unwrap(), b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn auto_detect_course_cover_finds_local_cover_and_preserves_custom() {
        use super::{auto_detect_course_cover, cover_version, read_cover};

        let temp = tempfile::tempdir().unwrap();
        let covers_dir = temp.path().join("covers");
        let course_dir = temp.path().join("curso_rust");
        std::fs::create_dir_all(&course_dir).unwrap();

        // 1. No images in the course folder -> does not detect
        assert!(!auto_detect_course_cover(&covers_dir, "rust-101", &course_dir).unwrap());
        assert!(cover_version(&covers_dir, "rust-101").is_none());

        // 2. A folder.png image present -> detects and saves to covers_dir
        let png_bytes = b"\x89PNG\r\n\x1a\nfake-png-data";
        std::fs::write(course_dir.join("folder.png"), png_bytes).unwrap();
        assert!(auto_detect_course_cover(&covers_dir, "rust-101", &course_dir).unwrap());
        assert!(cover_version(&covers_dir, "rust-101").is_some());
        let (stored_bytes, image_type) = read_cover(&covers_dir, "rust-101").unwrap().unwrap();
        assert_eq!(stored_bytes, png_bytes);
        assert_eq!(image_type, crate::covers::ImageType::Png);

        // 3. A cover.jpg image with higher priority added later -> does not overwrite an existing cover
        let jpeg_bytes = b"\xff\xd8\xff\xe0fake-jpeg";
        std::fs::write(course_dir.join("cover.jpg"), jpeg_bytes).unwrap();
        assert!(!auto_detect_course_cover(&covers_dir, "rust-101", &course_dir).unwrap());
        let (stored_bytes_after, _) = read_cover(&covers_dir, "rust-101").unwrap().unwrap();
        assert_eq!(stored_bytes_after, png_bytes);
    }
}
