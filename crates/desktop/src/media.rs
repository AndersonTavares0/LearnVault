//! Entrega de materiais e mídia do curso pelo protocolo customizado do webview.
//!
//! A entrega usa um protocolo `URI scheme` próprio do Tauri 2,
//! `learnvault-media://localhost/<media-id>`. Materiais continuam limitados a respostas
//! pequenas; vídeo, áudio e legendas usam a mesma resolução segura por identificador opaco e
//! suportam faixas únicas, como requerem elementos HTML de mídia.
//!
//! # Contrato de segurança
//!
//! - o webview só informa um **identificador opaco** (`media_id`); nenhum caminho cruza a
//!   fronteira. O parsing é estrito: esquema e host exatamente os registrados, método
//!   `GET`/`HEAD`, caminho com um único segmento opaco, sem query, sem fragmento, sem
//!   segmentos extras e com identificador de caracteres seguros;
//! - o `media_id` é resolvido **somente** pelo catálogo persistido, e a mídia é aberta
//!   exatamente uma vez por [`learnvault_core::open_media`], que revalida a raiz, a
//!   disponibilidade, o confinamento sob a raiz, o componente final (link simbólico) e a
//!   identidade do descritor aberto. O [`std::fs::File`] já aberto é o que é lido;
//! - materiais têm o limite explícito de [`MAX_SMALL_FILE_BYTES`]; mídia de curso é entregue
//!   por faixas, com cada resposta limitada por [`MAX_MEDIA_RESPONSE_BYTES`];
//! - só extensões e tipos de conteúdo em allowlist (materiais, vídeo, áudio e legendas) são
//!   servidos;
//! - nenhum corpo ou header carrega caminho do sistema de arquivos nem mensagem de erro de
//!   IO: as falhas viram apenas o status, com `Cache-Control: no-store`;
//! - a entrega é **somente leitura**: a única operação sobre a biblioteca é abrir e ler o
//!   arquivo.
//!
//! # Escopo da resposta
//!
//! `200`/`HEAD`/`206`/`416` são suportados com `Range` de faixa única. Um pedido multi-faixa
//! ou insatisfazível vira `416` com `Content-Range: bytes */<tamanho>`. O `Content-Range` de
//! sucesso é `bytes <início>-<fim>/<tamanho>`.

use std::borrow::Cow;
use std::io::{Read, Seek, SeekFrom};

use learnvault_core::{CourseRoot, Error, MediaKind, OpenMedia};
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, Url};

use crate::state::DesktopState;

/// Esquema do protocolo customizado de mídia do desktop.
pub const MEDIA_SCHEME: &str = "learnvault-media";

/// Host canônico do protocolo customizado.
///
/// Em todas as plataformas suportadas o handler recebe a URI já revertida para
/// `learnvault-media://localhost/...`: no Windows/Android o webview usa o prefixo
/// `http://learnvault-media.localhost`, que o runtime reverte antes de chamar o handler.
pub const MEDIA_HOST: &str = "localhost";

/// Rótulo da única janela que pode requisitar o protocolo.
pub const MAIN_WEBVIEW_LABEL: &str = "main";

/// Limite explícito de resposta para materiais pequenos.
///
/// A resposta é bufferizada em memória pela API do Tauri; arquivos acima deste limite
/// (ou uma faixa que o exceda) são recusados sem serem lidos por inteiro. Mídia de curso
/// (vídeo, áudio e legendas) usa o limite de resposta [`MAX_MEDIA_RESPONSE_BYTES`].
pub const MAX_SMALL_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Limite de bytes materializados em uma única resposta de mídia.
///
/// O protocolo do Tauri recebe o corpo como `Vec<u8>`, portanto tanto uma resposta completa
/// quanto uma faixa explícita precisam ser limitadas antes da alocação.
pub const MAX_MEDIA_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

/// Comprimento máximo aceito para o identificador opaco de mídia.
const MAX_MEDIA_ID_LEN: usize = 128;

/// Métodos HTTP aceitos pelo protocolo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMethod {
    /// Requisição com corpo.
    Get,
    /// Requisição somente de cabeçalhos.
    Head,
}

/// Requisição crua do handler, já normalizada em campos simples e testáveis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMediaRequest<'a> {
    /// Método como enviado pelo webview.
    pub method: &'a str,
    /// Esquema da URI.
    pub scheme: &'a str,
    /// Host da URI, se houver.
    pub host: Option<&'a str>,
    /// Caminho da URI, começando por `/`.
    pub path: &'a str,
    /// Query da URI, se houver (sempre recusada).
    pub query: Option<&'a str>,
    /// Cabeçalho `Range`, se houver.
    pub range: Option<&'a str>,
    /// Rótulo do webview que originou a requisição.
    pub webview_label: &'a str,
}

/// Requisição validada: método, identificador opaco e faixa opcional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRequest {
    /// Método validado.
    pub method: MediaMethod,
    /// Identificador opaco autorizado, sem caminho.
    pub media_id: String,
    /// `Range` repassado verbatim para a validação de faixa.
    pub range: Option<String>,
}

/// Recusa de parsing, antes de qualquer acesso ao sistema de arquivos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaRejection {
    /// A requisição não veio da janela principal do aplicativo.
    Forbidden,
    /// Esquema, host, caminho, query ou identificador inválidos.
    Malformed,
    /// Método diferente de `GET`/`HEAD`.
    MethodNotAllowed,
}

impl MediaRejection {
    /// Status HTTP estável da recusa.
    pub fn status(self) -> u16 {
        match self {
            MediaRejection::Forbidden => status::FORBIDDEN,
            MediaRejection::Malformed => status::BAD_REQUEST,
            MediaRejection::MethodNotAllowed => status::METHOD_NOT_ALLOWED,
        }
    }
}

/// Resposta pronta do protocolo, sem conhecer o runtime do Tauri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaResponse {
    /// Status HTTP.
    pub status: u16,
    /// Headers, na ordem de emissão.
    pub headers: Vec<(String, String)>,
    /// Corpo da resposta.
    pub body: Vec<u8>,
}

impl MediaResponse {
    /// Resposta sem corpo e sem headers.
    fn bare(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Resposta de erro estável: apenas o status e `Cache-Control: no-store`.
    fn empty(status: u16) -> Self {
        let mut response = Self::bare(status);
        response.push("Cache-Control", "no-store");
        response
    }

    /// Acrescenta um header.
    fn push(&mut self, name: &str, value: impl Into<String>) {
        self.headers.push((name.to_owned(), value.into()));
    }

    /// Valor de um header, comparando o nome sem diferenciar maiúsculas.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Adiciona CORS apenas para a origem exata da janela que solicitou a mídia.
///
/// CORS controla a leitura dos pixels pelo canvas; não é autorização da mídia e, por isso,
/// uma origem ausente ou não correspondente não altera status nem corpo da resposta.
///
/// Nenhum `Access-Control-Allow-Origin` herdado é refletido: o cabeçalho só é emitido na
/// correspondência exata. Toda requisição que traz `Origin` marca a resposta com
/// `Vary: Origin`, porque o cabeçalho CORS (e portanto a representação) depende da origem
/// mesmo quando o ACAO não é emitido.
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

/// Valida uma origem serializada com o parser URL e retorna sua forma canônica.
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

/// Extrai a origem da URL da janela sem depender de uma origem fixa por plataforma.
fn window_url_origin(url: &Url) -> Option<String> {
    let serialized = url.origin().ascii_serialization();
    if serialized != "null" {
        return Some(serialized);
    }
    let host = url.host_str()?;
    (host.eq_ignore_ascii_case("localhost") && url.port().is_none())
        .then(|| format!("{}://{}", url.scheme(), host.to_ascii_lowercase()))
}

/// Status HTTP usados pelo protocolo.
mod status {
    /// Requisição inválida.
    pub const BAD_REQUEST: u16 = 400;
    /// Requisição de outra janela.
    pub const FORBIDDEN: u16 = 403;
    /// Mídia ausente, indisponível, removida ou sem raiz configurada.
    pub const NOT_FOUND: u16 = 404;
    /// Método não suportado.
    pub const METHOD_NOT_ALLOWED: u16 = 405;
    /// Arquivo acima do limite de resposta.
    pub const PAYLOAD_TOO_LARGE: u16 = 413;
    /// Tipo de conteúdo fora da allowlist.
    pub const UNSUPPORTED_MEDIA_TYPE: u16 = 415;
    /// Faixa insatisfazível.
    pub const RANGE_NOT_SATISFIABLE: u16 = 416;
    /// Falha interna de leitura ou de estado.
    pub const INTERNAL_SERVER_ERROR: u16 = 500;
    /// Resposta completa.
    pub const OK: u16 = 200;
    /// Resposta parcial.
    pub const PARTIAL_CONTENT: u16 = 206;
}

/// Allowlist de extensões e seus tipos de conteúdo.
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
    ("webm", "video/webm"),
    ("ogv", "video/ogg"),
    ("mkv", "video/x-matroska"),
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
];

/// Valida e normaliza a requisição crua, sem tocar o sistema de arquivos.
///
/// Recusa qualquer coisa que não seja exatamente o protocolo registrado para a janela
/// principal: outro esquema, outro host, query, fragmento (ausente do caminho), caminho
/// com mais de um segmento e identificadores fora do conjunto de caracteres seguro.
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

/// Um identificador opaco válido é um único segmento de caracteres seguros.
///
/// O identificador real é um UUID gerado pelo domínio, mas a validação aceita o
/// alfabeto seguro sem decodificar percent-encoding: um `%` indicaria uma tentativa de
/// escapar a forma canônica e é recusado, nunca decodificado.
fn valid_media_id(media_id: &str) -> bool {
    !media_id.is_empty()
        && media_id.len() <= MAX_MEDIA_ID_LEN
        && media_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

/// Registra o protocolo de mídia no builder do Tauri.
///
/// O protocolo é global, mas cada requisição é restrita à janela principal pelo rótulo do
/// webview — o handler recusa qualquer outra origem. Nenhuma permissão de capability é
/// concedida: o ACL continua deny-by-default.
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

/// Constrói a requisição crua a partir da requisição HTTP do Tauri.
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

/// Converte a resposta do protocolo na resposta HTTP do runtime.
fn http_response(response: MediaResponse) -> Response<Cow<'static, [u8]>> {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = Response::builder().status(status);
    for (name, value) in &response.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }
    builder
        .body(Cow::Owned(response.body))
        .unwrap_or_else(|_| fallback_response())
}

/// Resposta mínima usada apenas se a construção de headers falhar (nunca acontece com
/// valores próprios, mas mantém o handler livre de `panic`).
fn fallback_response() -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Cow::Owned(Vec::new()))
        .expect("uma resposta de erro vazia é sempre construível")
}

impl DesktopState {
    /// Entrega uma mídia autorizada pelo protocolo customizado.
    ///
    /// O parsing é puro e prefere a recusa estável; só depois de validada a requisição a
    /// raiz configurada e o catálogo são consultados. Sem raiz configurada, a resposta é
    /// `404`, sem expor estado algum.
    pub fn serve_media(&self, raw: &RawMediaRequest<'_>) -> MediaResponse {
        match parse_media_request(raw) {
            Ok(request) => self.open_media_request(&request),
            Err(rejection) => MediaResponse::empty(rejection.status()),
        }
    }

    /// Abre a mídia autorizada uma única vez e monta a resposta.
    fn open_media_request(&self, request: &MediaRequest) -> MediaResponse {
        let Ok(database) = self.lock_database() else {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        };
        // A identidade opaca liga a mídia à biblioteca do curso registrado; sem curso
        // correspondente, a resposta é `404`, sem expor estado algum.
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
        // A raiz persistida é revalidada imediatamente antes da abertura, preservando a
        // semântica da importação: uma pasta removida, inacessível ou substituída por
        // link/junction é recusada, sem seguir o alvo.
        let Ok(root) = CourseRoot::resolve(course.root.as_path()) else {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        };

        // A abertura única revalida raiz, disponibilidade, confinamento, link simbólico e
        // identidade do descritor. O `File` já aberto é o que será lido.
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

/// Monta a resposta de sucesso (`200`/`206`) a partir da mídia já aberta.
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

/// Valida uma faixa `Range` de faixa única.
///
/// Devolve `Ok(None)` quando não há `Range`, `Ok(Some((início, fim)))` para uma faixa
/// válida e `Err(())` quando a faixa é multi-faixa, vazia, malformada ou insatisfazível.
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

/// Resposta `416` estável, com a sonda de tamanho total e sem corpo.
fn range_not_satisfiable(size: u64) -> MediaResponse {
    let mut response = MediaResponse::empty(status::RANGE_NOT_SATISFIABLE);
    response.push("Content-Range", format!("bytes */{size}"));
    response
}

/// Traduz o erro do núcleo em um status estável, sem vazar a causa.
///
/// Ausência legítima vira `404`; integridade e falhas operacionais viram `500`, como no
/// adaptador HTTP, para não confundir ausência real com catálogo/sistema inconsistente.
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

/// Tipo de conteúdo permitido para um nome de arquivo, ou `None` se fora da allowlist.
fn content_type(name: &str) -> Option<&'static str> {
    let (_, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    ALLOWED_TYPES
        .iter()
        .find(|(candidate, _)| *candidate == extension.as_str())
        .map(|(_, mime)| *mime)
}
