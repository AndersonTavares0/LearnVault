//! Entrega de arquivos pequenos pelo protocolo customizado do webview.
//!
//! Esta fatia entrega, no desktop, **somente arquivos pequenos** — imagens/thumbnails e
//! materiais pequenos — por um protocolo `URI scheme` próprio do Tauri 2,
//! `learnvault-media://localhost/<media-id>`. A entrega de vídeo e áudio permanece fora
//! deste protocolo por causa da incompatibilidade de reprodução parcial documentada no
//! webview do Linux (`WebKitGTK`): o streaming de vídeo depende de `Range` contínuo com
//! resposta progressiva, que esta fatia não implementa, e o `Player` mantém vídeo/áudio do
//! desktop explicitamente indisponíveis. Legendas continuam no caminho HTTP (a conversão
//! de SRT sob demanda é `fetch` e só existe no build HTTP). Materiais e downloads grandes
//! ficam fora do limite de resposta.
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
//! - a resposta é **bufferizada** pela API do Tauri, então há um limite explícito de
//!   [`MAX_SMALL_FILE_BYTES`]: um arquivo ou uma faixa acima do limite é recusado sem ser
//!   lido por inteiro, e a leitura nunca excede o limite;
//! - só um **tipo de conteúdo em allowlist** (imagens e materiais pequenos) é servido;
//!   vídeo, áudio e legendas são recusados com erro estável;
//! - nenhum corpo ou header carrega caminho do sistema de arquivos nem mensagem de erro de
//!   IO: as falhas viram apenas o status, com `Cache-Control: no-store`;
//! - a entrega é **somente leitura**: a única operação sobre a biblioteca é abrir e ler o
//!   arquivo.
//!
//! # Escopo da resposta
//!
//! `200`/`HEAD`/`206`/`416` são suportados para arquivos pequenos, com `Range` de faixa
//! única. Um pedido multi-faixa ou insatisfazível vira `416` com `Content-Range: bytes
//! */<tamanho>`. O `Content-Range` de sucesso é `bytes <início>-<fim>/<tamanho>`.

use std::borrow::Cow;
use std::io::{Read, Seek, SeekFrom};

use learnvault_core::{Error, MediaKind, OpenMedia};
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime};

use crate::command::LIBRARY_ID;
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

/// Limite explícito de resposta para arquivos pequenos.
///
/// A resposta é bufferizada em memória pela API do Tauri; arquivos acima deste limite
/// (ou uma faixa que o exceda) são recusados sem serem lidos por inteiro. Materiais e
/// downloads grandes ficam fora desta fatia.
pub const MAX_SMALL_FILE_BYTES: u64 = 8 * 1024 * 1024;

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
///
/// Somente imagens e materiais pequenos. Vídeo, áudio, legendas e qualquer outra extensão
/// ficam de fora: o protocolo não é uma superfície genérica de leitura da biblioteca.
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
        http_response(state.serve_media(&raw))
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
    /// Entrega uma mídia de arquivo pequeno pelo protocolo customizado.
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
        let Some(root) = self.library_root() else {
            return MediaResponse::empty(status::NOT_FOUND);
        };
        let Ok(database) = self.lock_database() else {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        };

        // A abertura única revalida raiz, disponibilidade, confinamento, link simbólico e
        // identidade do descritor. O `File` já aberto é o que será lido.
        let opened = match learnvault_core::open_media(
            &database,
            LIBRARY_ID,
            &request.media_id,
            root.as_path(),
        ) {
            Ok(opened) => opened,
            Err(error) => return MediaResponse::empty(media_error_status(&error)),
        };

        // O protocolo não é uma superfície genérica: vídeo/áudio e legendas não entram.
        if opened.kind != MediaKind::Material {
            return MediaResponse::empty(status::UNSUPPORTED_MEDIA_TYPE);
        }
        let Some(content_type) = content_type(&opened.name) else {
            return MediaResponse::empty(status::UNSUPPORTED_MEDIA_TYPE);
        };
        if opened.size > MAX_SMALL_FILE_BYTES {
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
    let (start, end) = range.unwrap_or((0, size.saturating_sub(1)));
    let length = if size == 0 { 0 } else { end - start + 1 };

    let mut body = Vec::new();
    if request.method == MediaMethod::Get && length > 0 {
        let mut file = opened.file;
        if file.seek(SeekFrom::Start(start)).is_err() {
            return MediaResponse::empty(status::INTERNAL_SERVER_ERROR);
        }
        let mut buffer = vec![0u8; length as usize];
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
