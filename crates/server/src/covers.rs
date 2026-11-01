//! Capas personalizadas dos cursos: upload administrativo, leitura autenticada e remoção.
//!
//! Uma capa é um único arquivo por curso, guardado no diretório de dados do servidor (ao
//! lado do banco), jamais dentro da pasta somente-leitura do curso. O tipo é decidido
//! apenas pelos bytes iniciais (magic bytes): nenhum decodificador de imagem é embarcado.
//! O caminho físico nunca cruza a API; a interface conhece só a identidade opaca do curso.
//!
//! ## Leiaute em disco
//!
//! As capas ficam em `<diretório de dados>/covers/<id do curso>.cover`, ou seja no mesmo
//! diretório do arquivo de registro (`courses.json`), derivado de
//! [`crate::courses::registry_path`]. O nome do arquivo é apenas a identidade opaca do
//! curso mais um sufixo: a identidade é validada como um único segmento de caminho seguro
//! ([`valid_course_id`]) antes de qualquer acesso ao disco, então nenhuma identidade
//! recebida pela API pode escapar do diretório de capas.
//!
//! A gravação é atômica (arquivo temporário exclusivo + troca por `rename`), para que uma
//! leitura concorrente nunca veja uma capa truncada. A "versão" devolvida ao cliente é o
//! *mtime* do arquivo em segundos: é o suficiente para invalidar a URL (`?v=`) do webview e
//! é derivada do arquivo em disco, não de um contador em memória que se perderia no
//! reinício do processo.

use std::{
    fs::{self, Metadata, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
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

/// Tamanho máximo aceito para uma capa personalizada (4 MiB).
pub(crate) const MAX_COVER_BYTES: usize = 4 * 1024 * 1024;

/// Nome do diretório de capas dentro do diretório de dados do servidor.
pub(crate) const COVERS_DIR_NAME: &str = "covers";

/// Sufixo do arquivo de capa de um curso dentro do diretório de capas.
const COVER_FILE_SUFFIX: &str = ".cover";

/// Comprimento máximo aceito para a identidade de um curso usada como segmento de caminho.
const MAX_COURSE_ID_LEN: usize = 128;

/// Tipo de imagem reconhecido pelos bytes iniciais do arquivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImageType {
    Png,
    Jpeg,
    Webp,
}

impl ImageType {
    /// `Content-Type` correspondente ao tipo detectado.
    pub(crate) const fn content_type(self) -> &'static str {
        match self {
            ImageType::Png => "image/png",
            ImageType::Jpeg => "image/jpeg",
            ImageType::Webp => "image/webp",
        }
    }
}

/// Falha estável das operações de capa. Nunca carrega caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoverError {
    /// O conteúdo foi recusado antes de tocar o disco (tipo não suportado ou acima do limite).
    Rejected,
    /// A gravação ou a remoção não pôde concluir.
    WriteFailed,
    /// A capa guardada não pôde ser lida.
    Unreadable,
    /// A capa guardada não é uma imagem suportada (corrupção ou adulteração no disco).
    Corrupt,
}

/// Diretório de capas, ao lado do arquivo de registro de cursos (e do banco).
pub(crate) fn covers_dir(database_path: &Path) -> PathBuf {
    let parent = crate::courses::registry_path(database_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    parent.join(COVERS_DIR_NAME)
}

/// Caminho físico da capa de um curso. Só é chamado com uma identidade já validada.
fn cover_file(covers_dir: &Path, course_id: &str) -> PathBuf {
    covers_dir.join(format!("{course_id}{COVER_FILE_SUFFIX}"))
}

/// Aceita apenas um segmento de caminho seguro (identidade opaca do curso).
///
/// Toda identidade que o servidor pode emitir é `course-` + um UUID (hexadecimal e hífens,
/// de [`learnvault_core::course_id_from_path`]) ou a identidade legada `library-1`, então o
/// conjunto `[A-Za-z0-9_-]` cobre todos os cursos reais. Qualquer outra coisa é recusada:
/// vazio, `.`/`..`, separadores de caminho, caracteres de controle (incluindo NUL),
/// espaços, `%` (separador percent-encoded) e valores longos demais.
pub(crate) fn valid_course_id(course_id: &str) -> bool {
    if course_id.is_empty() || course_id.len() > MAX_COURSE_ID_LEN {
        return false;
    }
    course_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Detecta o tipo da imagem apenas pelos bytes iniciais.
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
    // `RIFF<4 bytes de tamanho>WEBP`: os quatro bytes entre os marcadores são o tamanho
    // declarado do contêiner e não participam da identificação do formato.
    if bytes.len() >= 12 && bytes.starts_with(RIFF) && &bytes[8..12] == WEBP {
        return Some(ImageType::Webp);
    }
    None
}

/// Versão (mtime em segundos) da capa de um curso, quando existir.
///
/// Um caminho que não seja um arquivo comum (diretório, link ou junction) não tem versão:
/// a listagem nunca anuncia uma capa que [`read_cover`] recusaria.
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

/// Grava a capa de forma atômica e devolve a versão resultante.
///
/// O temporário é criado exclusivamente (`create_new`), sincronizado e trocado pelo destino
/// com [`replace_file`]; nenhum leitor observa um arquivo parcial e nenhum temporário fica
/// para trás. O conteúdo é revalidado aqui (tipo suportado e abaixo do limite), para que a
/// primitiva nunca grave algo que a leitura recusaria.
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

/// Lê a capa e redetecta o tipo pelos bytes.
///
/// O `Content-Type` devolvido vem sempre do arquivo guardado, nunca do pedido de upload.
/// Um caminho ausente ou que não seja um arquivo comum é ausência de capa; um arquivo
/// comum que não seja uma imagem suportada (ou acima do limite) é corrupção.
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

/// Remove a capa e informa se algo foi apagado.
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

/// Segundos de modificação de um arquivo, quando representáveis.
pub(crate) fn modified_seconds(metadata: &Metadata) -> Option<i64> {
    let modified = metadata.modified().ok()?;
    let seconds = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}

/// Detecta automaticamente uma capa local na raiz da pasta do curso e a armazena em `covers_dir`.
///
/// Não substitui capa já existente em `covers_dir`. Prioriza os nomes `cover.*`, `folder.*`,
/// `poster.*`, `thumb.*` e `thumbnail.*` com extensões suportadas (`.jpg`, `.jpeg`, `.png`, `.webp`).
/// O diretório do curso é lido de forma estritamente somente-leitura.
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

    Ok(false)
}

/// Verdadeiro quando a identidade é segura e corresponde a um curso registrado.
fn course_is_registered(state: &AppState, course_id: &str) -> bool {
    valid_course_id(course_id)
        && state
            .courses_snapshot()
            .iter()
            .any(|course| course.id == course_id)
}

/// Corpo do pedido de upload. É lido com limite explícito, fora do limite padrão do roteador.
enum CoverBodyError {
    /// O corpo excedeu [`MAX_COVER_BYTES`].
    TooLarge,
    /// O corpo não pôde ser lido até o fim.
    Unreadable,
}

/// Consome o corpo do pedido acumulando no máximo [`MAX_COVER_BYTES`].
///
/// O limite padrão do roteador só é aplicado por extratores que o respeitam; um handler que
/// consome o `Body` diretamente precisa impor o próprio limite, e é isso que torna o `413`
/// determinístico em vez de depender do tamanho dos chunks do transporte.
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

/// Erro JSON estável, com a política de cache privada.
fn json_error(status: StatusCode, message: &'static str) -> Response {
    private_no_store((status, Json(ErrorBody { error: message })).into_response())
}

/// `PUT /api/admin/courses/{id}/cover` — grava a capa personalizada de um curso.
///
/// Somente administrador com credenciais já definidas e mesma origem. O curso precisa estar
/// registrado: uma identidade desconhecida (ou insegura) responde `404` sem revelar nada. O
/// upload é aceito apenas se os bytes iniciais forem de PNG, JPEG ou WebP e o corpo couber
/// em [`MAX_COVER_BYTES`]; a gravação roda fora das threads Tokio, com a reserva de
/// capacidade de tarefas blocking mantida até o fim do trabalho.
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
    // Evita ler o corpo só para devolver 503 sob saturação operacional.
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

/// `DELETE /api/admin/courses/{id}/cover` — remove a capa personalizada de um curso.
///
/// Mesmas exigências do upload. Remover um curso desconhecido responde `404`; remover a capa
/// de um curso sem capa responde `200` com `removed: false`, para que a interface seja
/// idempotente.
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

/// `GET /api/courses/{id}/cover` — serve a capa personalizada de um curso.
///
/// Qualquer usuário autenticado com credenciais definidas. A resposta carrega o
/// `Content-Type` derivado dos bytes guardados e nunca é armazenável; a ausência de capa é
/// um `404` sem corpo, para que nem o caminho nem a razão da falha cruzem a API.
pub(crate) async fn get_cover(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => {}
        Ok(Some(_)) => return credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    }
    if !course_is_registered(&state, &course_id) {
        return status_private_no_store(StatusCode::NOT_FOUND);
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
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
        // Um GIF não é aceito: só PNG, JPEG e WebP.
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
        // Um separador percent-encoded decodificado também precisa ser recusado.
        assert!(!valid_course_id("a%2Fb"));
        assert!(!valid_course_id("a b"));
        // NUL e caracteres de controle nunca podem virar parte de um nome de arquivo.
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
        // Um banco sem diretório pai usa o diretório atual, como o registro.
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

        // Um diretório com o nome da capa não é uma capa.
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

        // 1. Sem imagens -> não detecta
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

        // 3. Imagem cover.jpg com prioridade mais alta adicionada depois -> não sobrescreve
        fs::write(course_dir.join("cover.jpg"), jpeg()).unwrap();
        assert!(!auto_detect_course_cover(&covers, "course-web", &course_dir).unwrap());
        let (stored_bytes_after, _) = read_cover(&covers, "course-web").unwrap().unwrap();
        assert_eq!(stored_bytes_after, png_bytes);
    }
}
