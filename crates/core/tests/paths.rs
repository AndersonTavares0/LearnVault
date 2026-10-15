//! Testes de normalização e resolução de caminhos relativos da biblioteca.
//!
//! Todos os casos usam dados em memória ou diretórios temporários: nenhuma
//! biblioteca real é acessada.

use learnvault_core::Error;
use learnvault_core::paths::{KeyMode, relative_path_key, resolve_within};

#[test]
fn separators_are_normalized_to_forward_slashes() {
    assert_eq!(
        relative_path_key("curso\\modulo\\aula.mp4", KeyMode::Sensitive).unwrap(),
        "curso/modulo/aula.mp4"
    );
    assert_eq!(
        relative_path_key("curso/modulo\\aula.mp4", KeyMode::Sensitive).unwrap(),
        "curso/modulo/aula.mp4"
    );
}

#[test]
fn insensitive_mode_folds_ascii_case_and_sensitive_mode_preserves_it() {
    let original = "Curso/MODULO/Aula.MP4";
    assert_eq!(
        relative_path_key(original, KeyMode::Insensitive).unwrap(),
        "curso/modulo/aula.mp4"
    );
    assert_eq!(
        relative_path_key(original, KeyMode::Sensitive).unwrap(),
        original
    );
}

#[test]
fn absolute_paths_are_rejected() {
    for path in [
        "/etc/passwd",
        "\\windows\\system32",
        "C:/Users/aluno",
        "C:\\Users\\aluno",
    ] {
        assert!(
            relative_path_key(path, KeyMode::Sensitive).is_err(),
            "caminho absoluto foi aceito: {path}"
        );
    }
}

#[test]
fn drive_prefixed_components_are_rejected_even_in_the_middle() {
    // No Windows, empilhar um componente com prefixo próprio descarta a raiz acumulada.
    for path in ["curso/C:/Windows", "x/C:", "C:"] {
        assert!(
            relative_path_key(path, KeyMode::Sensitive).is_err(),
            "componente com prefixo de unidade foi aceito: {path}"
        );
    }
}

#[test]
fn parent_components_are_rejected() {
    for path in [
        "../segredo.txt",
        "curso/../fora.txt",
        "curso/..",
        "curso/sub/../../fora.txt",
    ] {
        assert!(
            relative_path_key(path, KeyMode::Sensitive).is_err(),
            "componente .. foi aceito: {path}"
        );
    }
}

#[test]
fn empty_malformed_and_control_character_paths_are_rejected() {
    for path in [
        "",
        ".",
        "./curso",
        "curso//aula.mp4",
        "curso/",
        "aula\0.mp4",
    ] {
        assert!(
            relative_path_key(path, KeyMode::Sensitive).is_err(),
            "caminho inválido foi aceito: {path}"
        );
    }
}

#[test]
fn rejected_paths_report_the_invalid_relative_path_error() {
    for path in ["../fora.txt", "/etc/passwd", "", "curso/C:"] {
        match relative_path_key(path, KeyMode::Sensitive) {
            Err(Error::InvalidRelativePath { path: reported }) => assert_eq!(reported, path),
            other => panic!("erro inesperado para {path}: {other:?}"),
        }
    }
}

#[test]
fn resolve_within_accepts_valid_relative_paths() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    let resolved = resolve_within(root, "curso/modulo\\aula.mp4").unwrap();

    assert_eq!(resolved, root.join("curso").join("modulo").join("aula.mp4"));
}

#[test]
fn resolve_within_always_stays_under_the_root() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    let resolved = resolve_within(root, "curso/modulo/aula.mp4").unwrap();

    assert!(
        resolved.starts_with(root),
        "o caminho resolvido escapou da raiz: {}",
        resolved.display()
    );
}

#[test]
fn resolve_within_refuses_escapes_and_absolute_paths() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    for relative in [
        "../fora.txt",
        "/etc/passwd",
        "C:/Users/aluno",
        "curso/C:/Windows",
        "curso/../../fora.txt",
        "",
    ] {
        match resolve_within(root, relative) {
            Err(Error::InvalidRelativePath { .. }) => {}
            other => panic!("esperava Error::InvalidRelativePath para {relative}: {other:?}"),
        }
    }
}

#[test]
fn windows_mode_rejects_colons_because_they_open_alternate_data_streams() {
    for path in [
        "Curso/Aula/aula.mp4:fluxo",
        "Curso/Aula:oculto/aula.mp4",
        "Curso/Dia 1: Introdução/aula.mp4",
        "aula.mp4:",
    ] {
        assert!(
            relative_path_key(path, KeyMode::Insensitive).is_err(),
            "componente com `:` foi aceito no modo do Windows: {path}"
        );
    }
}

#[test]
fn sensitive_mode_keeps_colons_because_unix_allows_them_in_names() {
    for path in [
        "Curso/Aula/aula.mp4:fluxo",
        "Curso/Dia 1: Introdução/aula.mp4",
        "aula.mp4:",
    ] {
        assert_eq!(
            relative_path_key(path, KeyMode::Sensitive).unwrap(),
            path,
            "o modo Unix deve preservar nomes com `:`, que são legítimos: {path}"
        );
    }
}
