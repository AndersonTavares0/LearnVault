//! Custom course covers: administrative upload, authenticated read and removal.
//!
//! A cover is a single file per course, kept in the server's data directory (next to the
//! database), never inside the course's read-only folder. The type is decided only
//! by the initial bytes (magic bytes): no image decoder is shipped.
//! The physical path never crosses the API; the interface knows only the course's opaque identity.
//!
//! ## Layout on disk
//!
//! Covers live in `<data directory>/covers/<course id>.cover`, that is, in the same
//! directory as the registry file (`courses.json`), derived from
//! [`crate::courses::registry_path`]. The file name is just the course's opaque
//! identity plus a suffix: the identity is validated as a single safe path segment
//! ([`valid_course_id`]) before any disk access, so no identity
//! received through the API can escape the covers directory.
//!
//! Writing is atomic (exclusive temporary file + swap by `rename`), so that a
//! concurrent read never sees a truncated cover. The "version" returned to the client is the
//! file's *mtime* in seconds: it is enough to invalidate the webview's URL (`?v=`) and
//! it is derived from the file on disk, not from an in-memory counter that would be lost on
//! process restart.

use std::{
    fs::{self, Metadata, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::UNIX_EPOCH,
};

use axum::{
    Json,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tokio_stream::StreamExt as _;

use crate::{
    AppState, StateError,
    auth::same_origin,
    catalog::{
        authenticated_user, credentials_required, csrf_failed, internal_error, private_no_store,
        service_unavailable, status_private_no_store,
    },
    courses::replace_file,
};

/// Maximum size accepted for a custom cover (4 MiB).
pub(crate) const MAX_COVER_BYTES: usize = 4 * 1024 * 1024;

/// Name of the covers directory inside the server's data directory.
pub(crate) const COVERS_DIR_NAME: &str = "covers";

/// Suffix of a course's cover file inside the covers directory.
const COVER_FILE_SUFFIX: &str = ".cover";

/// Maximum length accepted for a course identity used as a path segment.
const MAX_COURSE_ID_LEN: usize = 128;

/// Image type recognized by the file's initial bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageType {
    Png,
    Jpeg,
    Webp,
}

impl ImageType {
    /// `Content-Type` matching the detected type.
    pub(crate) const fn content_type(self) -> &'static str {
        match self {
            ImageType::Png => "image/png",
            ImageType::Jpeg => "image/jpeg",
            ImageType::Webp => "image/webp",
        }
    }
}

/// Stable failure of the cover operations. It never carries a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoverError {
    /// The content was refused before touching the disk (unsupported type or above the limit).
    Rejected,
    /// The write or the removal could not complete.
    WriteFailed,
    /// The stored cover could not be read.
    Unreadable,
    /// The stored cover is not a supported image (corruption or tampering on disk).
    Corrupt,
}

/// Covers directory, next to the course registry file (and the database).
pub(crate) fn covers_dir(database_path: &Path) -> PathBuf {
    let parent = crate::courses::registry_path(database_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    parent.join(COVERS_DIR_NAME)
}

/// Physical path of a course's cover. Only called with an already-validated identity.
fn cover_file(covers_dir: &Path, course_id: &str) -> PathBuf {
    covers_dir.join(format!("{course_id}{COVER_FILE_SUFFIX}"))
}

/// Accepts only a safe path segment (the course's opaque identity).
///
/// Every identity the server can issue is `course-` + a UUID (hexadecimal and hyphens,
/// from [`learnvault_core::course_id_from_path`]) or the legacy identity `library-1`, so the
/// set `[A-Za-z0-9_-]` covers all real courses. Anything else is refused:
/// empty, `.`/`..`, path separators, control characters (including NUL),
/// spaces, `%` (the percent-encoded separator) and values that are too long.
pub(crate) fn valid_course_id(course_id: &str) -> bool {
    if course_id.is_empty() || course_id.len() > MAX_COURSE_ID_LEN {
        return false;
    }
    course_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Detects the image type only from the initial bytes.
pub(crate) fn detect_image_type(bytes: &[u8]) -> Option<ImageType> {
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

/// Version (mtime in seconds) of a course's cover, when it exists.
///
/// A path that is not a regular file (directory, link or junction) has no version:
/// the listing never announces a cover that [`read_cover`] would refuse.
pub(crate) fn cover_version(covers_dir: &Path, course_id: &str) -> Option<i64> {
    if !valid_course_id(course_id) {
        return None;
    }
    let metadata = fs::symlink_metadata(cover_file(covers_dir, course_id)).ok()?;
    if !metadata.is_file() {
        return None;
    }
    modified_seconds(&metadata)
}

/// Writes the cover atomically and returns the resulting version.
///
/// The temporary file is created exclusively (`create_new`), synced and swapped into the destination
/// with [`replace_file`]; no reader observes a partial file and no temporary file is left
/// behind. The content is revalidated here (supported type and below the limit), so the
/// primitive never writes something that the read would refuse.
pub(crate) fn store_cover(
    covers_dir: &Path,
    course_id: &str,
    bytes: &[u8],
) -> Result<i64, CoverError> {
    if !valid_course_id(course_id)
        || bytes.len() > MAX_COVER_BYTES
        || detect_image_type(bytes).is_none()
    {
        return Err(CoverError::Rejected);
    }
    fs::create_dir_all(covers_dir).map_err(|_| CoverError::WriteFailed)?;

    let destination = cover_file(covers_dir, course_id);
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
    cover_version(covers_dir, course_id).ok_or(CoverError::WriteFailed)
}

/// Reads the cover and redetects the type from the bytes.
///
/// The `Content-Type` returned always comes from the stored file, never from the upload request.
/// A path that is absent or not a regular file means no cover; a regular
/// file that is not a supported image (or is above the limit) is corruption.
pub(crate) fn read_cover(
    covers_dir: &Path,
    course_id: &str,
) -> Result<Option<(Vec<u8>, ImageType)>, CoverError> {
    if !valid_course_id(course_id) {
        return Ok(None);
    }
    let path = cover_file(covers_dir, course_id);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(CoverError::Unreadable),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > MAX_COVER_BYTES as u64 {
        return Err(CoverError::Corrupt);
    }
    let bytes = fs::read(&path).map_err(|_| CoverError::Unreadable)?;
    let Some(image_type) = detect_image_type(&bytes) else {
        return Err(CoverError::Corrupt);
    };
    Ok(Some((bytes, image_type)))
}

/// Removes the cover and reports whether anything was deleted.
pub(crate) fn remove_cover(covers_dir: &Path, course_id: &str) -> Result<bool, CoverError> {
    if !valid_course_id(course_id) {
        return Ok(false);
    }
    match fs::remove_file(cover_file(covers_dir, course_id)) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(CoverError::WriteFailed),
    }
}

/// Modification seconds of a file, when representable.
pub(crate) fn modified_seconds(metadata: &Metadata) -> Option<i64> {
    let modified = metadata.modified().ok()?;
    let seconds = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}

/// Automatically detects a local cover at the root of the course folder and stores it in `covers_dir`.
///
/// It does not replace a cover already in `covers_dir`. It prioritizes the names `cover.*`, `folder.*`,
/// `poster.*`, `thumb.*` and `thumbnail.*` with supported extensions (`.jpg`, `.jpeg`, `.png`, `.webp`).
/// The course directory is read strictly read-only.
pub(crate) fn auto_detect_course_cover(
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
        if !meta.is_file() || meta.len() > MAX_COVER_BYTES as u64 {
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

    if let Some(bytes) = extract_fallback_video_cover(course_root) {
        store_cover(covers_dir, course_id, &bytes)?;
        return Ok(true);
    }

    Ok(false)
}

/// Finds the first regular video file within a bounded depth under `course_root`,
/// skipping hidden folders and symlinks so the read-only course boundary is preserved.
fn find_first_video_file(course_root: &Path) -> Option<PathBuf> {
    const MAX_DEPTH: usize = 4;
    const VIDEO_EXTS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "m4v", "ts"];

    let mut stack = vec![(course_root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        children.sort();

        let mut subdirs = Vec::new();
        for path in children {
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_file() {
                let is_video = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|ext| VIDEO_EXTS.contains(&ext.to_ascii_lowercase().as_str()));
                if is_video {
                    return Some(path);
                }
            } else if meta.is_dir() && depth < MAX_DEPTH {
                subdirs.push(path);
            }
        }
        // Push in reverse so the lexicographically first directory is popped first.
        for subdir in subdirs.into_iter().rev() {
            stack.push((subdir, depth + 1));
        }
    }
    None
}

/// Extracts a single JPEG frame from the course's first video via `ffmpeg`, returning `None`
/// when no video exists or `ffmpeg` cannot decode a frame (e.g., unavailable or dummy file).
fn extract_fallback_video_cover(course_root: &Path) -> Option<Vec<u8>> {
    let video_path = find_first_video_file(course_root)?;
    for seek in ["00:00:05", "00:00:00"] {
        let output = Command::new("ffmpeg")
            .args(["-nostdin", "-loglevel", "error", "-ss", seek, "-i"])
            .arg(&video_path)
            .args([
                "-frames:v",
                "1",
                "-vf",
                "scale=640:-2:force_original_aspect_ratio=decrease",
                "-f",
                "image2pipe",
                "-vcodec",
                "mjpeg",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        if output.status.success()
            && !output.stdout.is_empty()
            && output.stdout.len() <= MAX_COVER_BYTES
            && detect_image_type(&output.stdout).is_some()
        {
            return Some(output.stdout);
        }
    }
    None
}

/// True when the identity is safe and matches a registered course.
fn course_is_registered(state: &AppState, course_id: &str) -> bool {
    valid_course_id(course_id)
        && state
            .courses_snapshot()
            .iter()
            .any(|course| course.id == course_id)
}

/// Body of the upload request. It is read with an explicit limit, outside the router's default limit.
enum CoverBodyError {
    /// The body exceeded [`MAX_COVER_BYTES`].
    TooLarge,
    /// The body could not be read to the end.
    Unreadable,
}

/// Consumes the request body, accumulating at most [`MAX_COVER_BYTES`].
///
/// The router's default limit is only applied by extractors that respect it; a handler that
/// consumes the `Body` directly must enforce its own limit, and that is what makes the `413`
/// deterministic instead of depending on the size of the transport's chunks.
async fn read_cover_body(body: Body) -> Result<Vec<u8>, CoverBodyError> {
    let mut stream = std::pin::pin!(body.into_data_stream());
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else {
            return Err(CoverBodyError::Unreadable);
        };
        if bytes.len() + chunk.len() > MAX_COVER_BYTES {
            return Err(CoverBodyError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[derive(Serialize)]
struct CoverResponse {
    cover_version: i64,
}

#[derive(Serialize)]
struct RemoveCoverResponse {
    removed: bool,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

/// Stable JSON error, with the private cache policy.
fn json_error(status: StatusCode, message: &'static str) -> Response {
    private_no_store((status, Json(ErrorBody { error: message })).into_response())
}

/// `PUT /api/admin/courses/{id}/cover` — stores a course's custom cover.
///
/// Administrator only, with credentials already set and same origin. The course must be
/// registered: an unknown (or unsafe) identity answers `404` without revealing anything. The
/// upload is accepted only if the initial bytes are PNG, JPEG or WebP and the body fits
/// in [`MAX_COVER_BYTES`]; the write runs off the Tokio threads, with the reservation of
/// blocking-task capacity held until the work ends.
pub(crate) async fn put_cover(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !user.is_admin {
        return status_private_no_store(StatusCode::FORBIDDEN);
    }
    if user.must_change_credentials {
        return credentials_required();
    }
    // Avoids reading the body just to return 503 under operational saturation.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    if !course_is_registered(&state, &course_id) {
        return status_private_no_store(StatusCode::NOT_FOUND);
    }
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_COVER_BYTES as u64)
    {
        return json_error(StatusCode::PAYLOAD_TOO_LARGE, "cover_too_large");
    }
    let bytes = match read_cover_body(body).await {
        Ok(bytes) => bytes,
        Err(CoverBodyError::TooLarge) => {
            return json_error(StatusCode::PAYLOAD_TOO_LARGE, "cover_too_large");
        }
        Err(CoverBodyError::Unreadable) => {
            return json_error(StatusCode::BAD_REQUEST, "cover_unreadable");
        }
    };
    if detect_image_type(&bytes).is_none() {
        return json_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_cover_type");
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let covers = covers_dir(&state.config.database_path);
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        store_cover(&covers, &course_id, &bytes)
    })
    .await;

    match outcome {
        Ok(Ok(cover_version)) => {
            private_no_store(Json(CoverResponse { cover_version }).into_response())
        }
        Ok(Err(CoverError::Rejected)) => {
            json_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_cover_type")
        }
        Ok(Err(_)) | Err(_) => internal_error("cover_write_failed"),
    }
}

/// `DELETE /api/admin/courses/{id}/cover` — removes a course's custom cover.
///
/// Same requirements as the upload. Removing an unknown course answers `404`; removing the cover
/// of a course with no cover answers `200` with `removed: false`, so the interface is
/// idempotent.
pub(crate) async fn delete_cover(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !user.is_admin {
        return status_private_no_store(StatusCode::FORBIDDEN);
    }
    if user.must_change_credentials {
        return credentials_required();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    if !course_is_registered(&state, &course_id) {
        return status_private_no_store(StatusCode::NOT_FOUND);
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let covers = covers_dir(&state.config.database_path);
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        remove_cover(&covers, &course_id)
    })
    .await;

    match outcome {
        Ok(Ok(removed)) => private_no_store(Json(RemoveCoverResponse { removed }).into_response()),
        Ok(Err(_)) | Err(_) => internal_error("cover_write_failed"),
    }
}

/// `GET /api/courses/{id}/cover` — serves a course's custom cover.
///
/// Any authenticated user with credentials set. The response carries the
/// `Content-Type` derived from the stored bytes and is never cacheable; the absence of a cover is
/// a `404` with no body, so neither the path nor the reason for the failure crosses the API.
pub(crate) async fn get_cover(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => user,
        Ok(Some(_)) => return credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !course_is_registered(&state, &course_id) {
        return status_private_no_store(StatusCode::NOT_FOUND);
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    if !user.is_admin {
        let database = state.database.clone().lock_owned().await;
        let Some(blocking_task) = state.try_acquire_blocking_task() else {
            return service_unavailable();
        };
        let check_course_id = course_id.clone();
        let allowed = tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            learnvault_core::can_user_access_course(
                &database,
                &user.user_id,
                false,
                &check_course_id,
            )
        })
        .await;
        match allowed {
            Ok(Ok(true)) => {}
            Ok(Ok(false)) => return status_private_no_store(StatusCode::NOT_FOUND),
            Ok(Err(_)) | Err(_) => return internal_error("cover_unavailable"),
        }
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let covers = covers_dir(&state.config.database_path);
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        read_cover(&covers, &course_id)
    })
    .await;

    match outcome {
        Ok(Ok(Some((bytes, image_type)))) => {
            let mut response = Response::new(Body::from(bytes));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(image_type.content_type()),
            );
            private_no_store(response)
        }
        Ok(Ok(None)) => status_private_no_store(StatusCode::NOT_FOUND),
        Ok(Err(_)) | Err(_) => internal_error("cover_unavailable"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        CoverError, ImageType, MAX_COVER_BYTES, cover_version, covers_dir, detect_image_type,
        read_cover, remove_cover, store_cover, valid_course_id,
    };

    fn png() -> Vec<u8> {
        vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0]
    }

    fn jpeg() -> Vec<u8> {
        vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F', 0]
    }

    fn webp() -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&[0x20, 0, 0, 0]);
        bytes.extend_from_slice(b"WEBP");
        bytes.push(0);
        bytes
    }

    #[test]
    fn detects_supported_image_types_by_magic_bytes() {
        assert_eq!(detect_image_type(&png()), Some(ImageType::Png));
        assert_eq!(detect_image_type(&jpeg()), Some(ImageType::Jpeg));
        assert_eq!(detect_image_type(&webp()), Some(ImageType::Webp));
    }

    #[test]
    fn rejects_unknown_empty_and_truncated_payloads() {
        assert_eq!(detect_image_type(b""), None);
        assert_eq!(detect_image_type(b"not an image"), None);
        assert_eq!(detect_image_type(b"\x89PNG"), None);
        assert_eq!(detect_image_type(b"RIFF\x20\x00\x00\x00WEB"), None);
        // A GIF is not accepted: only PNG, JPEG and WebP.
        assert_eq!(detect_image_type(b"GIF89a"), None);
    }

    #[test]
    fn image_types_map_to_their_content_type() {
        assert_eq!(ImageType::Png.content_type(), "image/png");
        assert_eq!(ImageType::Jpeg.content_type(), "image/jpeg");
        assert_eq!(ImageType::Webp.content_type(), "image/webp");
    }

    #[test]
    fn accepts_opaque_course_identities_and_rejects_unsafe_segments() {
        assert!(valid_course_id("library-1"));
        assert!(valid_course_id(
            "course-3f2a1c40-7e6b-4b1f-9a11-2b3c4d5e6f70"
        ));
        assert!(valid_course_id("A_b-9"));
        assert!(valid_course_id(&"a".repeat(128)));

        assert!(!valid_course_id(""));
        assert!(!valid_course_id(".."));
        assert!(!valid_course_id("."));
        assert!(!valid_course_id("a/b"));
        assert!(!valid_course_id("a\\b"));
        assert!(!valid_course_id("café"));
        assert!(!valid_course_id(&"a".repeat(129)));
        // A decoded percent-encoded separator must also be refused.
        assert!(!valid_course_id("a%2Fb"));
        assert!(!valid_course_id("a b"));
        // NUL and control characters can never become part of a file name.
        assert!(!valid_course_id("a\0b"));
        assert!(!valid_course_id("a\nb"));
        assert!(!valid_course_id("a\tb"));
    }

    #[test]
    fn covers_dir_sits_next_to_the_course_registry() {
        let temp = tempdir().unwrap();
        let data = temp.path().join("data");
        fs::create_dir_all(&data).unwrap();

        assert_eq!(
            covers_dir(&data.join("learnvault.sqlite")),
            data.join("covers")
        );
        // A database with no parent directory uses the current directory, like the registry.
        assert_eq!(
            covers_dir(&std::path::PathBuf::from("learnvault.sqlite")),
            std::path::Path::new(".").join("covers")
        );
    }

    #[test]
    fn store_read_and_remove_round_trip_a_cover_byte_for_byte() {
        let temp = tempdir().unwrap();
        let covers = covers_dir(&temp.path().join("learnvault.sqlite"));
        assert_eq!(cover_version(&covers, "library-1"), None);
        assert_eq!(read_cover(&covers, "library-1").unwrap(), None);

        let version = store_cover(&covers, "library-1", &png()).unwrap();
        assert!(version > 0);
        assert_eq!(cover_version(&covers, "library-1"), Some(version));
        let (stored, image_type) = read_cover(&covers, "library-1").unwrap().unwrap();
        assert_eq!(stored, png());
        assert_eq!(image_type, ImageType::Png);

        assert!(remove_cover(&covers, "library-1").unwrap());
        assert!(!remove_cover(&covers, "library-1").unwrap());
        assert_eq!(cover_version(&covers, "library-1"), None);
        assert_eq!(read_cover(&covers, "library-1").unwrap(), None);
    }

    #[test]
    fn store_replaces_the_previous_cover_and_leaves_no_temporaries() {
        let temp = tempdir().unwrap();
        let covers = covers_dir(&temp.path().join("learnvault.sqlite"));

        store_cover(&covers, "library-1", &png()).unwrap();
        store_cover(&covers, "library-1", &webp()).unwrap();

        let entries: Vec<_> = fs::read_dir(&covers)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["library-1.cover".to_owned()]);

        let (stored, image_type) = read_cover(&covers, "library-1").unwrap().unwrap();
        assert_eq!(stored, webp());
        assert_eq!(image_type, ImageType::Webp);
        assert!(!covers.join("library-2.cover").exists());
    }

    #[test]
    fn store_rejects_unsupported_oversized_and_unsafe_payloads() {
        let temp = tempdir().unwrap();
        let covers = covers_dir(&temp.path().join("learnvault.sqlite"));

        assert_eq!(
            store_cover(&covers, "library-1", b"not an image"),
            Err(CoverError::Rejected)
        );
        assert_eq!(
            store_cover(&covers, "library-1", &vec![b'x'; MAX_COVER_BYTES + 1]),
            Err(CoverError::Rejected)
        );
        assert_eq!(
            store_cover(&covers, "../escape", &png()),
            Err(CoverError::Rejected)
        );
        assert!(!covers.exists());
    }

    #[test]
    fn read_rejects_a_corrupt_or_oversized_stored_cover() {
        let temp = tempdir().unwrap();
        let covers = covers_dir(&temp.path().join("learnvault.sqlite"));
        fs::create_dir_all(&covers).unwrap();

        fs::write(covers.join("library-1.cover"), b"not an image").unwrap();
        assert_eq!(read_cover(&covers, "library-1"), Err(CoverError::Corrupt));
        assert!(cover_version(&covers, "library-1").is_some());

        fs::write(
            covers.join("library-2.cover"),
            vec![b'x'; MAX_COVER_BYTES + 1],
        )
        .unwrap();
        assert_eq!(read_cover(&covers, "library-2"), Err(CoverError::Corrupt));

        // A directory with the cover's name is not a cover.
        fs::create_dir_all(covers.join("library-3.cover")).unwrap();
        assert_eq!(read_cover(&covers, "library-3").unwrap(), None);
        assert_eq!(cover_version(&covers, "library-3"), None);
    }

    #[test]
    fn file_operations_never_escape_the_covers_directory() {
        let temp = tempdir().unwrap();
        let covers = covers_dir(&temp.path().join("learnvault.sqlite"));
        let outside = covers.join("..").join("library-9.cover");

        assert_eq!(
            store_cover(&covers, "..", &png()),
            Err(CoverError::Rejected)
        );
        assert!(!remove_cover(&covers, "../library-9").unwrap());
        assert_eq!(read_cover(&covers, "../library-9").unwrap(), None);
        assert!(!outside.exists());
    }

    #[test]
    fn auto_detect_course_cover_detects_local_cover_and_preserves_custom() {
        use super::auto_detect_course_cover;
        let temp = tempdir().unwrap();
        let covers = covers_dir(&temp.path().join("learnvault.sqlite"));
        let course_dir = temp.path().join("curso_web");
        fs::create_dir_all(&course_dir).unwrap();

        // 1. No images -> does not detect
        assert!(!auto_detect_course_cover(&covers, "course-web", &course_dir).unwrap());
        assert!(cover_version(&covers, "course-web").is_none());

        // 2. Imagem poster.png -> detecta e salva
        let png_bytes = png();
        fs::write(course_dir.join("poster.png"), &png_bytes).unwrap();
        assert!(auto_detect_course_cover(&covers, "course-web", &course_dir).unwrap());
        assert!(cover_version(&covers, "course-web").is_some());
        let (stored_bytes, image_type) = read_cover(&covers, "course-web").unwrap().unwrap();
        assert_eq!(stored_bytes, png_bytes);
        assert_eq!(image_type, ImageType::Png);

        // 3. A cover.jpg image with higher priority added later -> does not overwrite
        fs::write(course_dir.join("cover.jpg"), jpeg()).unwrap();
        assert!(!auto_detect_course_cover(&covers, "course-web", &course_dir).unwrap());
        let (stored_bytes_after, _) = read_cover(&covers, "course-web").unwrap().unwrap();
        assert_eq!(stored_bytes_after, png_bytes);
    }

    #[test]
    fn find_first_video_file_walks_in_order_and_skips_hidden_dirs() {
        use super::{auto_detect_course_cover, find_first_video_file};
        let temp = tempdir().unwrap();
        let covers = covers_dir(&temp.path().join("learnvault.sqlite"));
        let course_dir = temp.path().join("curso_video");
        fs::create_dir_all(course_dir.join(".hidden")).unwrap();
        fs::write(course_dir.join(".hidden/secret.mp4"), b"dummy").unwrap();
        fs::create_dir_all(course_dir.join("01 Intro")).unwrap();
        let expected = course_dir.join("01 Intro/01.mp4");
        fs::write(&expected, b"not-a-real-mp4").unwrap();

        assert_eq!(find_first_video_file(&course_dir), Some(expected));
        // Dummy non-video bytes must gracefully return Ok(false) without failing scans.
        assert!(!auto_detect_course_cover(&covers, "course-video", &course_dir).unwrap());
    }
}
