//! Limpeza visual de títulos de cursos, módulos e aulas.
//!
//! Remove só o ruído típico de nomes de arquivo: tags de resolução e de codificação
//! (`1080p`, `x265`, `WEB-DL`), sublinhados usados como espaço e espaços repetidos. Não
//! altera o nome no disco nem o caminho que identifica o nó; é apenas apresentação.

/// Tags de release e codificação de vídeo que não fazem parte do título de uma aula.
///
/// Áudio (`aac`, `mp3`) e termos como `4K` ficam de fora de propósito: podem ser parte do
/// assunto de uma aula.
const RELEASE_TAGS: &[&str] = &[
    "webrip", "web-dl", "webdl", "hdtv", "bluray", "brrip", "dvdrip", "hdrip", "hevc", "avc",
    "x264", "x265", "h264", "h265", "10bit", "8bit",
];

/// Título sem tags de release, com sublinhados trocados por espaços.
///
/// Se a limpeza apagar tudo (por exemplo, um nome que é só `1080p`), devolve a entrada
/// original, para que o nó nunca fique sem título.
pub fn presentable(title: &str) -> String {
    let spaced = title.replace('_', " ");
    let kept: Vec<&str> = spaced
        .split_whitespace()
        .filter(|token| !is_release_tag(token))
        .collect();
    if kept.is_empty() {
        title.to_owned()
    } else {
        kept.join(" ")
    }
}

/// Verdadeiro para uma palavra que é só uma tag de release, mesmo entre colchetes.
fn is_release_tag(token: &str) -> bool {
    let word = token
        .trim_matches(|character: char| "[](){}".contains(character))
        .to_ascii_lowercase();
    !word.is_empty() && (RELEASE_TAGS.contains(&word.as_str()) || is_resolution(&word))
}

/// `720p`, `1080p`, `2160p`: três ou quatro dígitos seguidos de `p`.
fn is_resolution(word: &str) -> bool {
    word.strip_suffix('p').is_some_and(|digits| {
        matches!(digits.len(), 3 | 4) && digits.bytes().all(|byte| byte.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::presentable;

    #[test]
    fn removes_resolution_and_codec_tags() {
        assert_eq!(presentable("Ownership 1080p x265"), "Ownership");
        assert_eq!(presentable("Borrowing [720p] WEB-DL"), "Borrowing");
        assert_eq!(presentable("Traits (2160p) HEVC"), "Traits");
    }

    #[test]
    fn turns_underscores_into_spaces_and_collapses_spaces() {
        assert_eq!(presentable("Intro_to__Rust"), "Intro to Rust");
        assert_eq!(presentable("  Lifetimes   basics "), "Lifetimes basics");
    }

    #[test]
    fn keeps_words_that_only_look_like_tags() {
        assert_eq!(presentable("Node.js 4K editing"), "Node.js 4K editing");
        assert_eq!(presentable("AAC audio basics"), "AAC audio basics");
        assert_eq!(presentable("1080 steps"), "1080 steps");
    }

    #[test]
    fn falls_back_to_the_input_when_nothing_is_left() {
        assert_eq!(presentable("1080p"), "1080p");
        assert_eq!(presentable("[x265]"), "[x265]");
    }
}
