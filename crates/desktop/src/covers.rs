//! Capas personalizadas dos cursos no desktop: escolha nativa, validação e entrega local.
//!
//! A capa é uma única imagem por curso, guardada no diretório de dados do próprio
//! aplicativo (`<dados do app>/covers/<id do curso>.cover`), nunca dentro da pasta do curso,
//! que permanece externa e somente leitura. O caminho nunca cruza a fronteira do webview: a
//! SPA pede a capa por um esquema próprio (`learnvault-cover://localhost/<id do curso>`) e o
//! handler do Tauri resolve a identidade opaca para um arquivo dentro do diretório de capas.
//!
//! Regras de segurança preservadas aqui:
//!
//! - só PNG, JPEG e WebP são aceitos, e o tipo é decidido **apenas pelos bytes iniciais**
//!   (magic bytes): nenhum decodificador de imagem é embarcado e a extensão do arquivo
//!   escolhido não decide nada;
//! - o limite de [`MAX_COVER_BYTES`] vale na escolha e na leitura, e a gravação é atômica
//!   (temporário exclusivo + `rename`), então nenhuma leitura observa uma capa truncada;
//! - a identidade do curso é validada como um único segmento de caminho seguro antes de
//!   qualquer acesso ao disco, e o curso precisa estar registrado;
//! - nenhuma resposta carrega caminho do sistema de arquivos: uma capa ausente é `404` sem
//!   corpo e qualquer falha interna é `500` sem corpo, sempre com `Cache-Control: no-store`;
//! - o CORS do esquema é o mesmo da mídia ([`crate::media::apply_media_cors`]): reflete
//!   somente a origem exata da janela, marca `Vary: Origin` quando há `Origin` e nunca usa
//!   curinga nem credenciais. Nenhuma permissão de capability é concedida ao webview.

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

/// Esquema do protocolo customizado das capas.
pub const COVER_SCHEME: &str = "learnvault-cover";

/// Host canônico do protocolo customizado.
///
/// Como no protocolo de mídia, o handler sempre recebe a URI já revertida para
/// `learnvault-cover://localhost/...`: no Windows/Android o webview usa o prefixo
/// `http://learnvault-cover.localhost`, que o runtime reverte antes de chamar o handler.
pub const COVER_HOST: &str = "localhost";

/// Nome do diretório de capas dentro do diretório de dados do aplicativo.
pub const COVERS_DIR_NAME: &str = "covers";

/// Tamanho máximo aceito para uma capa personalizada (4 MiB).
pub const MAX_COVER_BYTES: u64 = 4 * 1024 * 1024;

/// Sufixo do arquivo de capa de um curso dentro do diretório de capas.
const COVER_FILE_SUFFIX: &str = ".cover";

/// Comprimento máximo aceito para a identidade de um curso usada como segmento de caminho.
const MAX_COURSE_ID_LEN: usize = 128;

/// Tipo de imagem reconhecido pelos bytes iniciais do arquivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageType {
    /// PNG (`\x89PNG\r\n\x1a\n`).
    Png,
    /// JPEG (`\xFF\xD8\xFF`).
    Jpeg,
    /// WebP (`RIFF` + tamanho + `WEBP`).
    Webp,
}

impl ImageType {
    /// `Content-Type` correspondente ao tipo detectado.
    pub const fn content_type(self) -> &'static str {
        match self {
            ImageType::Png => "image/png",
            ImageType::Jpeg => "image/jpeg",
            ImageType::Webp => "image/webp",
        }
    }
}

/// Falha estável das operações de capa. Nunca carrega caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverError {
    /// O conteúdo foi recusado antes de tocar o disco (tipo não suportado ou acima do limite).
    Rejected,
    /// A gravação ou a remoção não pôde concluir.
    WriteFailed,
    /// A capa guardada não pôde ser lida.
    Unreadable,
    /// A capa guardada não é uma imagem suportada (corrupção ou adulteração no disco).
    Corrupt,
}

/// Métodos aceitos pelo protocolo de capas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverMethod {
    /// Requisição com corpo.
    Get,
    /// Requisição somente de cabeçalhos.
    Head,
}

/// Recusa de parsing, antes de qualquer acesso ao sistema de arquivos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverRejection {
    /// A requisição não veio da janela principal do aplicativo.
    Forbidden,
    /// Esquema, host, caminho, query ou identidade inválidos.
    Malformed,
    /// Método diferente de `GET`/`HEAD`.
    MethodNotAllowed,
}

impl CoverRejection {
    /// Status HTTP estável da recusa.
    pub fn status(self) -> u16 {
        match self {
            CoverRejection::Forbidden => COVER_STATUS_FORBIDDEN,
            CoverRejection::Malformed => COVER_STATUS_BAD_REQUEST,
            CoverRejection::MethodNotAllowed => COVER_STATUS_METHOD_NOT_ALLOWED,
        }
    }
}

/// Requisição crua do handler, já normalizada em campos simples e testáveis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCoverRequest<'a> {
    /// Método como enviado pelo webview.
    pub method: &'a str,
    /// Esquema da URI.
    pub scheme: &'a str,
    /// Host da URI, se houver.
    pub host: Option<&'a str>,
    /// Caminho da URI, começando por `/`.
    pub path: &'a str,
    /// Query da URI, se houver (sempre recusada; a versão da capa não chega ao handler).
    pub query: Option<&'a str>,
    /// Rótulo do webview que originou a requisição.
    pub webview_label: &'a str,
}

/// Requisição validada: método e identidade opaca do curso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverRequest {
    /// Método validado.
    pub method: CoverMethod,
    /// Identidade opaca autorizada, sem caminho.
    pub course_id: String,
}

/// Status HTTP usados pelo protocolo de capas.
const COVER_STATUS_OK: u16 = 200;
/// Requisição inválida.
const COVER_STATUS_BAD_REQUEST: u16 = 400;
/// Requisição de outra janela.
const COVER_STATUS_FORBIDDEN: u16 = 403;
/// Capa ausente ou curso desconhecido.
const COVER_STATUS_NOT_FOUND: u16 = 404;
/// Método não suportado.
const COVER_STATUS_METHOD_NOT_ALLOWED: u16 = 405;
/// Falha interna de leitura ou de estado.
const COVER_STATUS_INTERNAL_SERVER_ERROR: u16 = 500;

/// Aceita apenas um segmento de caminho seguro (identidade opaca do curso).
///
/// O identificador real é `course-` + um UUID (hexadecimal e hífens) ou a identidade legada
/// `library-1`, então o alfabeto seguro cobre todos os cursos. Um `%` indicaria uma tentativa
/// de escapar a forma canônica do esquema e é recusado, nunca decodificado.
pub fn valid_course_id(course_id: &str) -> bool {
    if course_id.is_empty() || course_id.len() > MAX_COURSE_ID_LEN {
        return false;
    }
    course_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Detecta o tipo da imagem apenas pelos bytes iniciais.
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
    // `RIFF<4 bytes de tamanho>WEBP`: os quatro bytes entre os marcadores são o tamanho
    // declarado do contêiner e não participam da identificação do formato.
    if bytes.len() >= 12 && bytes.starts_with(RIFF) && &bytes[8..12] == WEBP {
        return Some(ImageType::Webp);
    }
    None
}

/// Caminho físico da capa de um curso, ou `None` quando a identidade é insegura.
pub fn cover_file(covers_dir: &Path, course_id: &str) -> Option<PathBuf> {
    valid_course_id(course_id).then(|| covers_dir.join(format!("{course_id}{COVER_FILE_SUFFIX}")))
}

/// Versão (mtime em segundos) da capa de um curso, quando existir.
pub fn cover_version(covers_dir: &Path, course_id: &str) -> Option<i64> {
    let metadata = fs::symlink_metadata(cover_file(covers_dir, course_id)?).ok()?;
    if !metadata.is_file() {
        return None;
    }
    modified_seconds(&metadata)
}

/// Grava a capa de forma atômica, revalidando tipo e tamanho antes de tocar o disco.
pub fn store_cover(covers_dir: &Path, course_id: &str, bytes: &[u8]) -> Result<(), CoverError> {
    let Some(destination) = cover_file(covers_dir, course_id) else {
        return Err(CoverError::Rejected);
    };
    if bytes.len() as u64 > MAX_COVER_BYTES || detect_image_type(bytes).is_none() {
        return Err(CoverError::Rejected);
    }
    fs::create_dir_all(covers_dir).map_err(|_| CoverError::WriteFailed)?;

    let temporary = covers_dir.join(format!(".{course_id}.{}.tmp", std::process::id()));
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

/// Lê a capa e redetecta o tipo pelos bytes.
///
/// Um caminho ausente ou que não seja um arquivo comum é ausência de capa; um arquivo comum
/// que não seja uma imagem suportada (ou acima do limite) é corrupção.
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

/// Remove a capa e informa se algo foi apagado.
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

/// Lê o arquivo escolhido no seletor nativo, recusando acima do limite antes de ler.
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

/// Segundos de modificação de um arquivo, quando representáveis.
fn modified_seconds(metadata: &Metadata) -> Option<i64> {
    let modified = metadata.modified().ok()?;
    let seconds = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}

/// Substitui o destino usando apenas APIs padrão.
///
/// Fora do Windows, `fs::rename` troca o nome no mesmo sistema de arquivos e substitui o
/// destino. No Windows, a API padrão não substitui um destino existente: removê-lo
/// imediatamente antes de `rename` deixa uma janela não atômica em que o destino pode não
/// existir.
#[cfg(windows)]
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    match fs::remove_file(dest) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(source, dest)
}

/// Substitui o destino usando apenas APIs padrão.
#[cfg(not(windows))]
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    fs::rename(source, dest)
}

/// Valida e normaliza a requisição crua, sem tocar o sistema de arquivos.
///
/// Recusa qualquer coisa que não seja exatamente o protocolo registrado para a janela
/// principal: outro esquema, outro host, query fora de `v=<dígitos>`, caminho com mais de
/// um segmento e identidades fora do alfabeto seguro.
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

/// A versão da capa é opcional e só pode chegar como `v=<dígitos>`.
///
/// A SPA usa o valor para trocar a URL do `<img>` quando a capa muda; o handler o ignora
/// na resolução e continua respondendo com `Cache-Control: no-store`. Qualquer outra
/// query é recusada em vez de silenciosamente descartada.
fn valid_cover_version(query: Option<&str>) -> bool {
    let Some(query) = query else {
        return true;
    };
    let Some(version) = query.strip_prefix("v=") else {
        return false;
    };
    !version.is_empty() && version.len() <= 20 && version.bytes().all(|byte| byte.is_ascii_digit())
}

/// Registra o protocolo de capas no builder do Tauri.
///
/// O protocolo é global, mas cada requisição é restrita à janela principal pelo rótulo do
/// webview, e o CORS é o mesmo da mídia: origem exata da janela, jamais curinga nem
/// credenciais. Nenhuma permissão de capability é concedida: o ACL continua deny-by-default.
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

/// Constrói a requisição crua a partir da requisição HTTP do Tauri.
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
    /// Entrega a capa autorizada pelo protocolo customizado.
    ///
    /// O parsing é puro e prefere a recusa estável; só depois de validada a requisição o
    /// registro de cursos e o diretório de capas são consultados. Um curso não registrado
    /// responde como uma capa ausente, sem revelar nada.
    pub fn serve_cover(&self, raw: &RawCoverRequest<'_>) -> MediaResponse {
        match parse_cover_request(raw) {
            Ok(request) => self.open_cover_request(&request),
            Err(rejection) => MediaResponse::empty(rejection.status()),
        }
    }

    /// Monta a resposta da capa de um curso registrado.
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
        // A versão na query é aceita pela SPA como cache-buster e não altera a resolução.
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

        // Um diretório escolhido não é uma imagem.
        assert!(read_picked_cover(temp.path()).is_err());
        assert!(read_picked_cover(&temp.path().join("nao-existe.png")).is_err());

        let text = temp.path().join("capa.png");
        std::fs::write(&text, b"nao e uma imagem").unwrap();
        assert!(read_picked_cover(&text).is_err());

        let png = temp.path().join("real.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
        assert_eq!(read_picked_cover(&png).unwrap(), b"\x89PNG\r\n\x1a\n");
    }
}
