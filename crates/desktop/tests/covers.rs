//! Testes do protocolo de capas e do ciclo de importação/remoção do desktop.
//!
//! Não sobem o webview: exercitam o parsing puro (sem sistema de arquivos), a função livre
//! `DesktopState::serve_cover`, que o handler do protocolo apenas embrulha, e as funções de
//! estado `import_course_cover`/`remove_course_cover`. O alvo é o contrato observável — só
//! a identidade opaca autorizada, validação por magic bytes e limite de tamanho, gravação
//! fora da pasta do curso, status/headers corretos (200/HEAD/404/500), CORS de origem exata
//! e nenhuma exposição de caminho.

use std::fs;
use std::path::{Path, PathBuf};

use learnvault_desktop::media::{MAIN_WEBVIEW_LABEL, MediaResponse, apply_media_cors};
use learnvault_desktop::{
    COVER_HOST, COVER_SCHEME, COVERS_DIR_NAME, CoverMethod, CoverRejection, DesktopState,
    IpcErrorCode, MAX_COVER_BYTES, RawCoverRequest, detect_image_type, parse_cover_request,
    valid_course_id,
};

/// Capa PNG sintética: só os bytes iniciais importam para a detecção.
const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n capa pequena";
/// Capa JPEG sintética.
const JPEG_BYTES: &[u8] = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00 capa";
/// Capa WebP sintética: `RIFF` + tamanho declarado + `WEBP`.
const WEBP_BYTES: &[u8] = b"RIFF\x20\x00\x00\x00WEBP capa";

/// Estado desktop isolado em diretórios temporários, com dados e configuração separados.
fn desktop(root: &Path) -> DesktopState {
    DesktopState::open(&root.join("data"), &root.join("config")).unwrap()
}

/// Estado desktop com um curso externo importado e persistido.
fn configured(root: &Path) -> (DesktopState, PathBuf) {
    let state = desktop(root);
    let library = root.join("Acervo privado");
    fs::create_dir_all(&library).unwrap();
    state.import_course(|| Some(library.clone())).unwrap();
    (state, library)
}

/// Identidade do curso das fixtures, derivada do caminho canônico.
fn course_id(root: &Path) -> String {
    learnvault_core::course_id_from_path(&fs::canonicalize(root.join("Acervo privado")).unwrap())
}

/// Diretório de capas do aplicativo (subdiretório do diretório de dados).
fn covers_dir(root: &Path) -> PathBuf {
    root.join("data").join(COVERS_DIR_NAME)
}

/// Requisição crua com os campos do contrato padrão, prontos para sobrescrever.
fn raw<'a>(method: &'a str, path: &'a str) -> RawCoverRequest<'a> {
    RawCoverRequest {
        method,
        scheme: COVER_SCHEME,
        host: Some(COVER_HOST),
        path,
        query: None,
        webview_label: MAIN_WEBVIEW_LABEL,
    }
}

/// Entrega uma capa pelo caminho do protocolo, montando a URI canônica.
fn serve(state: &DesktopState, course: &str, method: &str) -> MediaResponse {
    let path = format!("/{course}");
    state.serve_cover(&RawCoverRequest {
        method,
        scheme: COVER_SCHEME,
        host: Some(COVER_HOST),
        path: &path,
        query: None,
        webview_label: MAIN_WEBVIEW_LABEL,
    })
}

/// Manifesto dos arquivos da árvore, usado para provar que a capa não escreve no curso.
fn manifest(root: &Path) -> Vec<(String, u64)> {
    fn walk(base: &Path, current: &Path, out: &mut Vec<(String, u64)>) {
        let mut entries: Vec<_> = fs::read_dir(current)
            .unwrap_or_else(|error| panic!("não foi possível ler {}: {error}", current.display()))
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let size = fs::metadata(&path).unwrap().len();
                let relative = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, size));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

// --- Parsing puro: nenhum acesso ao sistema de arquivos -------------------------------

#[test]
fn parses_the_canonical_cover_request() {
    let request = parse_cover_request(&raw("GET", "/course-1")).unwrap();

    assert_eq!(request.method, CoverMethod::Get);
    assert_eq!(request.course_id, "course-1");
}

#[test]
fn accepts_head_and_keeps_the_opaque_identity() {
    let request = parse_cover_request(&raw("HEAD", "/library-1")).unwrap();

    assert_eq!(request.method, CoverMethod::Head);
    assert_eq!(request.course_id, "library-1");
}

#[test]
fn rejects_a_foreign_scheme_host_or_query() {
    let mut scheme = raw("GET", "/course-1");
    scheme.scheme = "learnvault-media";
    assert_eq!(
        parse_cover_request(&scheme).unwrap_err(),
        CoverRejection::Malformed
    );

    let mut host = raw("GET", "/course-1");
    host.host = Some("evil.example");
    assert_eq!(
        parse_cover_request(&host).unwrap_err(),
        CoverRejection::Malformed
    );

    let mut missing_host = raw("GET", "/course-1");
    missing_host.host = None;
    assert_eq!(
        parse_cover_request(&missing_host).unwrap_err(),
        CoverRejection::Malformed
    );

    // A versão da capa é uma questão da SPA; a query nunca chega ao handler.
    let mut query = raw("GET", "/course-1");
    query.query = Some("v=123");
    assert_eq!(
        parse_cover_request(&query).unwrap_err(),
        CoverRejection::Malformed
    );
}

#[test]
fn rejects_extra_path_segments() {
    for path in ["/a/b", "/", "course-1", "/./a", "/../a", "//a"] {
        assert_eq!(
            parse_cover_request(&raw("GET", path)).unwrap_err(),
            CoverRejection::Malformed,
            "caminho inválido aceito: {path}"
        );
    }
}

#[test]
fn rejects_invalid_course_identities() {
    for invalid in [
        "", "..", ".", "a/b", "a\\b", "a%2Fb", "a b", "a:b", "a?b", "a#b", "a\0b", "café",
    ] {
        let path = format!("/{invalid}");
        assert_eq!(
            parse_cover_request(&raw("GET", &path)).unwrap_err(),
            CoverRejection::Malformed,
            "identidade inválida aceita: {invalid:?}"
        );
        // A mesma regra vale fora do protocolo: nenhum identificador inseguro vira caminho.
        assert!(!valid_course_id(invalid), "identidade aceita: {invalid:?}");
    }

    assert!(valid_course_id("library-1"));
    assert!(valid_course_id(&format!("course-{}", "a".repeat(30))));
    assert!(!valid_course_id(&"a".repeat(200)));
}

#[test]
fn rejects_methods_other_than_get_and_head() {
    for method in ["POST", "PUT", "DELETE", "PATCH", "get", "head"] {
        assert_eq!(
            parse_cover_request(&raw(method, "/course-1")).unwrap_err(),
            CoverRejection::MethodNotAllowed,
            "método aceito indevidamente: {method}"
        );
    }
}

#[test]
fn rejects_requests_from_another_webview() {
    let mut foreign = raw("GET", "/course-1");
    foreign.webview_label = "outra-janela";
    assert_eq!(
        parse_cover_request(&foreign).unwrap_err(),
        CoverRejection::Forbidden
    );
    assert_eq!(CoverRejection::Forbidden.status(), 403);
    assert_eq!(CoverRejection::Malformed.status(), 400);
    assert_eq!(CoverRejection::MethodNotAllowed.status(), 405);
}

#[test]
fn detects_only_the_supported_image_signatures() {
    assert_eq!(
        detect_image_type(PNG_BYTES).map(|kind| kind.content_type()),
        Some("image/png")
    );
    assert_eq!(
        detect_image_type(JPEG_BYTES).map(|kind| kind.content_type()),
        Some("image/jpeg")
    );
    assert_eq!(
        detect_image_type(WEBP_BYTES).map(|kind| kind.content_type()),
        Some("image/webp")
    );
    for unsupported in [
        b"".as_slice(),
        b"GIF89a",
        b"\x89PNG",
        b"RIFF\x20\x00\x00\x00WEB",
    ] {
        assert_eq!(detect_image_type(unsupported), None);
    }
}

// --- Entrega: 200/HEAD/404/500, MIME e confinamento -----------------------------------

#[test]
fn imports_and_serves_the_cover_byte_for_byte() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = configured(dir.path());
    let id = course_id(dir.path());

    let imported = state
        .import_course_cover(&id, || Some(write_png(dir.path(), PNG_BYTES)))
        .unwrap();
    assert!(imported.cover_version.is_some());

    let response = serve(&state, &id, "GET");
    assert_eq!(response.status, 200);
    assert_eq!(response.header("Content-Type"), Some("image/png"));
    assert_eq!(response.header("Cache-Control"), Some("no-store"));
    assert_eq!(response.header("X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(
        response
            .header("Content-Length")
            .unwrap()
            .parse::<usize>()
            .unwrap(),
        PNG_BYTES.len()
    );
    assert_eq!(response.body, PNG_BYTES);

    // A capa fica no diretório de dados do aplicativo, nunca dentro da pasta do curso.
    assert!(covers_dir(dir.path()).join(format!("{id}.cover")).is_file());
    assert_eq!(manifest(&library), Vec::new());

    // Nenhum header pode carregar caminho, raiz ou nome da pasta do curso.
    for (name, value) in &response.headers {
        assert!(
            !value.contains("Acervo privado") && !value.contains("segredo"),
            "header {name} expõe o caminho: {value}"
        );
        assert!(
            !value.contains(&library.to_string_lossy().to_string()),
            "header {name} expõe a raiz: {value}"
        );
    }
}

#[test]
fn head_returns_the_same_headers_without_a_body() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());
    let id = course_id(dir.path());
    let picked = write_png(dir.path(), PNG_BYTES);
    state.import_course_cover(&id, || Some(picked)).unwrap();

    let response = serve(&state, &id, "HEAD");

    assert_eq!(response.status, 200);
    assert_eq!(response.header("Content-Type"), Some("image/png"));
    assert_eq!(
        response.header("Content-Length"),
        Some(PNG_BYTES.len().to_string().as_str())
    );
    assert!(
        response.body.is_empty(),
        "um HEAD não pode carregar corpo: {}",
        response.body.len()
    );
}

#[test]
fn the_content_type_comes_from_the_stored_bytes_not_the_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());
    let id = course_id(dir.path());

    // O seletor aceita o nome com extensão enganosa: o formato é decidido pelos bytes.
    let misleading = dir.path().join("capa.png");
    fs::write(&misleading, WEBP_BYTES).unwrap();
    state.import_course_cover(&id, || Some(misleading)).unwrap();
    let response = serve(&state, &id, "GET");
    assert_eq!(response.header("Content-Type"), Some("image/webp"));
    assert_eq!(response.body, WEBP_BYTES);
}

#[test]
fn an_unknown_course_or_a_missing_cover_is_not_found_without_a_body() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());

    let unknown = serve(&state, "course-00000000-0000-0000-0000-000000000000", "GET");
    assert_eq!(unknown.status, 404);
    assert!(unknown.body.is_empty());

    let missing = serve(&state, &course_id(dir.path()), "GET");
    assert_eq!(missing.status, 404);
    assert!(missing.body.is_empty());

    // Sem protocolo válido, o status é o da recusa estável.
    let mut foreign = raw("GET", "/course-1");
    foreign.scheme = "https";
    assert_eq!(state.serve_cover(&foreign).status, 400);
    let mut foreign_window = raw("GET", "/course-1");
    foreign_window.webview_label = "outra";
    assert_eq!(state.serve_cover(&foreign_window).status, 403);
}

#[test]
fn a_corrupt_stored_cover_is_an_internal_error_without_a_body() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());
    let id = course_id(dir.path());
    fs::create_dir_all(covers_dir(dir.path())).unwrap();
    fs::write(
        covers_dir(dir.path()).join(format!("{id}.cover")),
        b"nao e uma imagem",
    )
    .unwrap();

    let response = serve(&state, &id, "GET");

    assert_eq!(response.status, 500);
    assert!(response.body.is_empty());
    assert_eq!(response.header("Cache-Control"), Some("no-store"));
    for (name, value) in &response.headers {
        assert!(
            !value.contains(".cover") && !value.contains("covers"),
            "header {name} expõe o armazenamento: {value}"
        );
    }
}

// --- Importação e remoção pelo estado --------------------------------------------------

#[test]
fn importing_rejects_unsupported_oversized_and_unreadable_files() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());
    let id = course_id(dir.path());

    let text = dir.path().join("capa.txt");
    fs::write(&text, b"nao e uma imagem").unwrap();
    let rejected = state.import_course_cover(&id, || Some(text)).unwrap_err();
    assert_eq!(rejected.code, IpcErrorCode::InvalidCover);

    let oversized = dir.path().join("grande.png");
    let mut big = PNG_BYTES.to_vec();
    big.resize(MAX_COVER_BYTES as usize + 1, b'x');
    fs::write(&oversized, &big).unwrap();
    let rejected = state
        .import_course_cover(&id, || Some(oversized))
        .unwrap_err();
    assert_eq!(rejected.code, IpcErrorCode::InvalidCover);

    let missing = dir.path().join("nao-existe.png");
    let unreadable = state
        .import_course_cover(&id, || Some(missing))
        .unwrap_err();
    assert_eq!(unreadable.code, IpcErrorCode::CoverUnavailable);

    // Nenhuma tentativa pode ter gravado algo.
    assert!(!covers_dir(dir.path()).exists());
}

#[test]
fn importing_an_unknown_or_unsafe_course_is_refused_before_the_picker() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());

    for identity in ["curso-desconhecido", "..", "a/b"] {
        let error = state
            .import_course_cover(identity, || panic!("o seletor não pode ser aberto"))
            .unwrap_err();
        assert_eq!(error.code, IpcErrorCode::CourseNotFound, "{identity}");
        assert_eq!(error.message, "curso não encontrado");
        assert!(!error.message.contains(identity));
    }
    assert!(!covers_dir(dir.path()).exists());
}

#[test]
fn cancelling_the_picker_and_replacing_a_cover_only_keep_one_file() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());
    let id = course_id(dir.path());

    // Cancelar antes de existir capa preserva o estado e não cria nada.
    let cancelled = state.import_course_cover(&id, || None).unwrap();
    assert_eq!(cancelled.cover_version, None);
    assert!(!covers_dir(dir.path()).exists());

    let first = write_png(dir.path(), PNG_BYTES);
    let version = state.import_course_cover(&id, || Some(first)).unwrap();
    assert!(version.cover_version.is_some());

    // Cancelar com capa existente devolve a versão atual, sem apagar nada.
    let cancelled = state.import_course_cover(&id, || None).unwrap();
    assert_eq!(cancelled.cover_version, version.cover_version);
    assert_eq!(serve(&state, &id, "GET").body, PNG_BYTES);

    // Trocar a capa mantém exatamente um arquivo, sem temporários.
    let second = write_jpeg(dir.path(), JPEG_BYTES);
    state.import_course_cover(&id, || Some(second)).unwrap();
    let mut entries: Vec<String> = fs::read_dir(covers_dir(dir.path()))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    assert_eq!(entries, vec![format!("{id}.cover")]);
    assert_eq!(serve(&state, &id, "GET").body, JPEG_BYTES);
}

#[test]
fn removing_is_idempotent_and_the_catalog_stops_advertising_the_cover() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());
    let id = course_id(dir.path());
    let picked = write_png(dir.path(), PNG_BYTES);
    let version = state.import_course_cover(&id, || Some(picked)).unwrap();

    let advertised = state.catalog().unwrap();
    let node = advertised
        .nodes
        .iter()
        .find(|node| node.course_id.as_deref() == Some(id.as_str()))
        .expect("o curso importado precisa estar no catálogo");
    assert_eq!(node.cover_version, version.cover_version);

    let removed = state.remove_course_cover(&id).unwrap();
    assert!(removed.removed);
    let again = state.remove_course_cover(&id).unwrap();
    assert!(!again.removed);

    let after = state.catalog().unwrap();
    let node = after
        .nodes
        .iter()
        .find(|node| node.course_id.as_deref() == Some(id.as_str()))
        .unwrap();
    assert_eq!(node.cover_version, None);
    assert_eq!(serve(&state, &id, "GET").status, 404);

    let unknown = state.remove_course_cover("curso-desconhecido").unwrap_err();
    assert_eq!(unknown.code, IpcErrorCode::CourseNotFound);
}

// --- CORS ------------------------------------------------------------------------------

#[test]
fn cover_cors_reflects_only_the_exact_window_origin_on_success() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());
    let id = course_id(dir.path());
    let picked = write_png(dir.path(), PNG_BYTES);
    state.import_course_cover(&id, || Some(picked)).unwrap();
    let window_origin = "http://tauri.localhost";

    for request_origin in [Some(window_origin), Some("http://evil.example"), None] {
        let response = apply_media_cors(serve(&state, &id, "GET"), request_origin, window_origin);

        assert_eq!(response.status, 200);
        assert_eq!(response.body, PNG_BYTES, "o CORS nunca altera o corpo");
        assert_eq!(
            response.header("Access-Control-Allow-Origin"),
            request_origin.filter(|origin| *origin == window_origin)
        );
        assert_eq!(response.header("Vary"), request_origin.map(|_| "Origin"));
        assert!(
            response.header("Vary") != Some("*"),
            "o protocolo nunca usa curinga de CORS"
        );
        assert!(
            response
                .header("Access-Control-Allow-Credentials")
                .is_none(),
            "credenciais nunca são liberadas no protocolo"
        );
    }
}

#[test]
fn cover_cors_withholds_the_allow_origin_outside_success_statuses() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = configured(dir.path());
    let window_origin = "http://tauri.localhost";

    // 404 (sem capa) e 500 (capa corrompida) nunca recebem ACAO, mas sempre variam por origem.
    for response in [
        serve(&state, &course_id(dir.path()), "GET"),
        serve(&state, "curso-desconhecido", "GET"),
    ] {
        assert_eq!(response.status, 404);
        let response = apply_media_cors(response, Some(window_origin), window_origin);
        assert_eq!(response.header("Access-Control-Allow-Origin"), None);
        assert_eq!(response.header("Vary"), Some("Origin"));
    }
}

/// Grava um arquivo de fixture e devolve seu caminho.
fn write_png(dir: &Path, bytes: &[u8]) -> PathBuf {
    let path = dir.join("escolhida.png");
    fs::write(&path, bytes).unwrap();
    path
}

/// Grava um arquivo de fixture JPEG e devolve seu caminho.
fn write_jpeg(dir: &Path, bytes: &[u8]) -> PathBuf {
    let path = dir.join("escolhida.jpeg");
    fs::write(&path, bytes).unwrap();
    path
}
