//! Testes do protocolo de mídia de arquivos pequenos do desktop.
//!
//! Não sobem o webview: exercitam o parsing puro (sem sistema de arquivos) e a função
//! livre `DesktopState::serve_media`, que o handler do protocolo apenas embrulha. O alvo é
//! o contrato observável — somente `media_id` autorizado, sem caminho vindo do webview,
//! somente arquivos pequenos/imagens/materiais, status/headers corretos (200/HEAD/206/416),
//! limite explícito de resposta e nenhuma exposição de caminho ou erro de IO.

use std::fs;
use std::path::{Path, PathBuf};

use learnvault_desktop::DesktopState;
use learnvault_desktop::command::LIBRARY_ID;
use learnvault_desktop::media::{
    MAIN_WEBVIEW_LABEL, MAX_SMALL_FILE_BYTES, MEDIA_HOST, MEDIA_SCHEME, MediaMethod,
    MediaRejection, MediaResponse, RawMediaRequest, parse_media_request,
};

/// Conteúdo sintético pequeno com extensão de imagem para as fixtures.
const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n pequeno";

/// Estado desktop isolado em diretórios temporários, com dados e configuração separados.
fn desktop(root: &Path) -> DesktopState {
    DesktopState::open(&root.join("data"), &root.join("config")).unwrap()
}

/// Estado desktop com uma biblioteca externa escolhida e persistida.
fn configured(root: &Path) -> (DesktopState, PathBuf) {
    let state = desktop(root);
    let library = root.join("Acervo privado");
    fs::create_dir_all(&library).unwrap();
    state.select_library(|| Some(library.clone())).unwrap();
    (state, library)
}

/// Biblioteca indexada com uma imagem pequena, um material pequeno e arquivos fora do escopo.
fn library(root: &Path) -> (DesktopState, PathBuf) {
    let (state, library) = configured(root);
    fs::create_dir_all(library.join("Curso")).unwrap();
    fs::write(library.join("Curso").join("foto.png"), PNG_BYTES).unwrap();
    fs::write(library.join("Curso").join("material.pdf"), b"pdf pequeno").unwrap();
    fs::write(library.join("Curso").join("aula.mp4"), b"video grande").unwrap();
    fs::write(library.join("Curso").join("legenda.vtt"), b"WEBVTT\n").unwrap();
    state.rescan().unwrap();
    (state, library)
}

/// Caminho do banco local usado pelo estado.
fn database_path(root: &Path) -> PathBuf {
    root.join("data").join("learnvault.sqlite")
}

/// Identidade autorizada de um arquivo indexado.
fn media_id(root: &Path, relative_path: &str) -> String {
    let database = learnvault_core::Database::open(database_path(root)).unwrap();
    learnvault_core::find_media_by_path(&database, LIBRARY_ID, relative_path)
        .unwrap()
        .unwrap_or_else(|| panic!("mídia não indexada: {relative_path}"))
        .media_id
}

/// Requisição crua com os campos do contrato padrão, prontos para sobrescrever.
fn raw<'a>(method: &'a str, path: &'a str) -> RawMediaRequest<'a> {
    RawMediaRequest {
        method,
        scheme: MEDIA_SCHEME,
        host: Some(MEDIA_HOST),
        path,
        query: None,
        range: None,
        webview_label: MAIN_WEBVIEW_LABEL,
    }
}

/// Entrega uma mídia pelo caminho do protocolo, montando a URI canônica.
fn serve(state: &DesktopState, media_id: &str, method: &str, range: Option<&str>) -> MediaResponse {
    let path = format!("/{media_id}");
    state.serve_media(&RawMediaRequest {
        method,
        scheme: MEDIA_SCHEME,
        host: Some(MEDIA_HOST),
        path: &path,
        query: None,
        range,
        webview_label: MAIN_WEBVIEW_LABEL,
    })
}

/// Manifesto dos arquivos da árvore, usado para provar que a entrega não escreve.
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

/// Cria um symlink de arquivo apontando para `target`.
///
/// No Windows criar um symlink exige privilégio; quando a plataforma recusa, o teste é
/// pulado com registro, como nos demais testes de link do repositório.
#[cfg(unix)]
fn link_file(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn link_file(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}

// --- Parsing puro: nenhum acesso ao sistema de arquivos -------------------------------

#[test]
fn parses_the_canonical_small_file_request() {
    let request = parse_media_request(&raw("GET", "/media-1")).unwrap();

    assert_eq!(request.method, MediaMethod::Get);
    assert_eq!(request.media_id, "media-1");
    assert_eq!(request.range, None);
}

#[test]
fn accepts_head_and_preserves_a_single_range_verbatim() {
    let mut head = raw("HEAD", "/media-1");
    head.range = Some("bytes=0-10");
    let request = parse_media_request(&head).unwrap();

    assert_eq!(request.method, MediaMethod::Head);
    assert_eq!(request.range.as_deref(), Some("bytes=0-10"));
}

#[test]
fn rejects_a_foreign_scheme_or_host() {
    let mut scheme = raw("GET", "/media-1");
    scheme.scheme = "https";
    assert_eq!(
        parse_media_request(&scheme).unwrap_err(),
        MediaRejection::Malformed
    );

    let mut host = raw("GET", "/media-1");
    host.host = Some("evil.example");
    assert_eq!(
        parse_media_request(&host).unwrap_err(),
        MediaRejection::Malformed
    );

    let mut missing_host = raw("GET", "/media-1");
    missing_host.host = None;
    assert_eq!(
        parse_media_request(&missing_host).unwrap_err(),
        MediaRejection::Malformed
    );
}

#[test]
fn rejects_any_query_and_extra_path_segments() {
    let mut query = raw("GET", "/media-1");
    query.query = Some("path=../segredo");
    assert_eq!(
        parse_media_request(&query).unwrap_err(),
        MediaRejection::Malformed
    );

    for path in ["/a/b", "/", "media-1", "/./a", "/../a"] {
        assert_eq!(
            parse_media_request(&raw("GET", path)).unwrap_err(),
            MediaRejection::Malformed,
            "caminho inválido aceito: {path}"
        );
    }
}

#[test]
fn rejects_invalid_media_ids() {
    for invalid in [
        "", "..", ".", "a/b", "a\\b", "a%2Fb", "a b", "a:b", "a?b", "a#b", "a\0b", "café",
    ] {
        let path = format!("/{invalid}");
        assert_eq!(
            parse_media_request(&raw("GET", &path)).unwrap_err(),
            MediaRejection::Malformed,
            "identificador inválido aceito: {invalid:?}"
        );
    }

    let long = "a".repeat(200);
    let path = format!("/{long}");
    assert_eq!(
        parse_media_request(&raw("GET", &path)).unwrap_err(),
        MediaRejection::Malformed
    );
}

#[test]
fn rejects_methods_other_than_get_and_head() {
    for method in ["POST", "PUT", "DELETE", "PATCH", "get", "head"] {
        assert_eq!(
            parse_media_request(&raw(method, "/media-1")).unwrap_err(),
            MediaRejection::MethodNotAllowed,
            "método aceito indevidamente: {method}"
        );
    }
}

#[test]
fn rejects_requests_from_another_webview() {
    let mut foreign = raw("GET", "/media-1");
    foreign.webview_label = "outra-janela";
    assert_eq!(
        parse_media_request(&foreign).unwrap_err(),
        MediaRejection::Forbidden
    );
}

// --- Entrega: 200/HEAD/206/416, limite, MIME e confinamento ---------------------------

#[test]
fn serves_a_small_image_with_200_and_safe_headers() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/foto.png");

    let response = serve(&state, &id, "GET", None);

    assert_eq!(response.status, 200);
    assert_eq!(response.header("Content-Type"), Some("image/png"));
    assert_eq!(response.header("Accept-Ranges"), Some("bytes"));
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
    assert!(
        response.header("Content-Range").is_none(),
        "uma resposta completa não pode declarar Content-Range"
    );

    // Nenhum header pode carregar caminho, raiz ou nome da pasta da biblioteca.
    for (name, value) in &response.headers {
        assert!(
            !value.contains("Curso") && !value.contains("foto.png"),
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
    let (state, _library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/foto.png");

    let response = serve(&state, &id, "HEAD", None);

    assert_eq!(response.status, 200);
    assert_eq!(response.header("Content-Type"), Some("image/png"));
    assert_eq!(
        response
            .header("Content-Length")
            .unwrap()
            .parse::<usize>()
            .unwrap(),
        PNG_BYTES.len()
    );
    assert!(
        response.body.is_empty(),
        "um HEAD não pode carregar corpo: {}",
        response.body.len()
    );
}

#[test]
fn serves_a_single_range_with_206_and_content_range() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/foto.png");

    let response = serve(&state, &id, "GET", Some("bytes=1-3"));

    assert_eq!(response.status, 206);
    let expected_range = format!("bytes 1-3/{}", PNG_BYTES.len());
    assert_eq!(
        response.header("Content-Range"),
        Some(expected_range.as_str())
    );
    assert_eq!(response.header("Content-Length"), Some("3"));
    assert_eq!(response.body, &PNG_BYTES[1..4]);
}

#[test]
fn a_suffix_and_an_open_range_are_supported() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/foto.png");

    let suffix = serve(&state, &id, "GET", Some("bytes=-2"));
    assert_eq!(suffix.status, 206);
    assert_eq!(suffix.body, &PNG_BYTES[PNG_BYTES.len() - 2..]);

    let open = serve(&state, &id, "GET", Some("bytes=2-"));
    assert_eq!(open.status, 206);
    assert_eq!(open.body, &PNG_BYTES[2..]);
}

#[test]
fn an_unsatisfiable_range_is_416_with_a_content_range_probe() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/foto.png");

    let response = serve(&state, &id, "GET", Some("bytes=999999-"));

    assert_eq!(response.status, 416);
    let expected = format!("bytes */{}", PNG_BYTES.len());
    assert_eq!(response.header("Content-Range"), Some(expected.as_str()));
    assert!(response.body.is_empty());
}

#[test]
fn a_multi_range_request_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/foto.png");

    let response = serve(&state, &id, "GET", Some("bytes=0-1,3-4"));

    assert_eq!(response.status, 416);
    assert!(response.body.is_empty());
}

#[test]
fn a_video_or_subtitle_is_never_served_by_the_protocol() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = library(dir.path());

    let video = media_id(dir.path(), "Curso/aula.mp4");
    assert_eq!(
        serve(&state, &video, "GET", None).status,
        415,
        "vídeo/áudio não pertence a esta fatia"
    );

    let subtitle = media_id(dir.path(), "Curso/legenda.vtt");
    assert_eq!(
        serve(&state, &subtitle, "GET", None).status,
        415,
        "legendas continuam HTTP-only"
    );
}

#[test]
fn an_extension_outside_the_allowlist_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = library(dir.path());
    fs::write(library.join("Curso").join("extra.zip"), b"zip").unwrap();
    state.rescan().unwrap();
    let id = media_id(dir.path(), "Curso/extra.zip");

    let response = serve(&state, &id, "GET", None);

    assert_eq!(response.status, 415);
    assert!(response.body.is_empty());
}

#[test]
fn a_file_above_the_small_file_cap_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = library(dir.path());
    let big = vec![b'a'; (MAX_SMALL_FILE_BYTES + 1) as usize];
    fs::write(library.join("Curso").join("grande.pdf"), &big).unwrap();
    state.rescan().unwrap();
    let id = media_id(dir.path(), "Curso/grande.pdf");

    let response = serve(&state, &id, "GET", None);

    assert_eq!(response.status, 413);
    assert!(
        response.body.is_empty(),
        "um arquivo acima do cap nunca pode ser carregado"
    );
}

#[test]
fn an_unknown_media_id_is_not_found_without_a_body() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = library(dir.path());

    let response = serve(&state, "00000000-0000-0000-0000-000000000000", "GET", None);

    assert_eq!(response.status, 404);
    assert!(response.body.is_empty());
}

#[test]
fn a_removed_media_is_no_longer_served() {
    let dir = tempfile::tempdir().unwrap();
    let (state, _library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/material.pdf");
    let mut database = learnvault_core::Database::open(database_path(dir.path())).unwrap();
    learnvault_core::remove_media(&mut database, LIBRARY_ID, &id, false).unwrap();

    let response = serve(&state, &id, "GET", None);

    assert_eq!(response.status, 404);
    assert!(response.body.is_empty());
}

#[test]
fn without_a_configured_root_the_request_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let state = desktop(dir.path());

    let response = serve(&state, "media-1", "GET", None);

    assert_eq!(response.status, 404);
    assert!(response.body.is_empty());
    assert!(
        !response
            .headers
            .iter()
            .any(|(_, value)| value.contains(&dir.path().to_string_lossy().to_string())),
        "nenhum header pode expor a raiz"
    );
}

#[test]
fn a_final_component_replaced_by_a_symlink_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/foto.png");

    let outside = dir.path().join("segredo.png");
    fs::write(&outside, b"x").unwrap();
    fs::remove_file(library.join("Curso").join("foto.png")).unwrap();
    if !link_file(&outside, &library.join("Curso").join("foto.png")) {
        eprintln!("pulado: criar symlink exige privilégio nesta plataforma");
        return;
    }

    let response = serve(&state, &id, "GET", None);

    assert_eq!(response.status, 500);
    assert!(response.body.is_empty());
    assert!(
        !response
            .headers
            .iter()
            .any(|(_, value)| value.contains("segredo")),
        "nenhum header pode expor o alvo do link"
    );
}

#[test]
fn serving_small_files_never_writes_inside_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/foto.png");
    let before = manifest(&library);

    let _ = serve(&state, &id, "GET", None);
    let _ = serve(&state, &id, "GET", Some("bytes=0-2"));
    let _ = serve(&state, &id, "HEAD", None);

    assert_eq!(
        manifest(&library),
        before,
        "a biblioteca externa permanece somente leitura"
    );
}

#[test]
fn a_file_replaced_in_place_is_served_without_a_path_leak() {
    let dir = tempfile::tempdir().unwrap();
    let (state, library) = library(dir.path());
    let id = media_id(dir.path(), "Curso/material.pdf");

    fs::write(library.join("Curso").join("material.pdf"), b"conteudo novo").unwrap();

    let response = serve(&state, &id, "GET", None);

    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"conteudo novo");
    let rendered = format!("{:?}", response.headers);
    assert!(
        !rendered.contains("Curso") && !rendered.contains("material.pdf"),
        "a resposta não pode expor o caminho: {rendered}"
    );
}
