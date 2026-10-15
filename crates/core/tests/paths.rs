//! Testes de normalização e resolução de caminhos relativos da biblioteca.
//!
//! Todos os casos usam dados em memória ou diretórios temporários: nenhuma
//! biblioteca real é acessada.

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
fn empty_and_malformed_paths_are_rejected() {
    for path in ["", ".", "./curso", "curso//aula.mp4"] {
        assert!(
            relative_path_key(path, KeyMode::Sensitive).is_err(),
            "caminho inválido foi aceito: {path}"
        );
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
fn resolve_within_refuses_escapes_and_absolute_paths() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();

    for relative in [
        "../fora.txt",
        "/etc/passwd",
        "C:/Users/aluno",
        "curso/../../fora.txt",
        "",
    ] {
        assert!(
            resolve_within(root, relative).is_err(),
            "escape foi aceito: {relative}"
        );
    }
}
