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

/// Remove do início de todos os títulos as palavras que eles têm em comum, como o nome do
/// curso repetido em cada aula ("Rust Course – Ownership", "Rust Course – Borrowing").
///
/// Só age com pelo menos três títulos, mantém ao menos uma palavra em cada um e não muda
/// nada se algum título ficaria vazio ou só com separadores.
pub fn strip_shared_prefix(titles: &mut [String]) {
    if titles.len() < 3 {
        return;
    }
    let words: Vec<Vec<&str>> = titles
        .iter()
        .map(|title| title.split_whitespace().collect())
        .collect();
    let first = &words[0];
    let mut shared = first.len();
    for other in &words[1..] {
        shared = shared.min(other.iter().zip(first).take_while(|(a, b)| a == b).count());
    }
    let shortest = words.iter().map(Vec::len).min().unwrap_or(0);
    shared = shared.min(shortest.saturating_sub(1));
    if shared == 0 {
        return;
    }
    let rests: Vec<String> = words
        .iter()
        .map(|words| {
            words[shared..]
                .join(" ")
                .trim_start_matches(|c: char| "-–—:|·•/".contains(c))
                .trim()
                .to_owned()
        })
        .collect();
    if rests.iter().any(String::is_empty) {
        return;
    }
    for (title, rest) in titles.iter_mut().zip(rests) {
        *title = rest;
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
    fn removes_a_prefix_shared_by_every_title() {
        let mut titles: Vec<String> = [
            "Rust Course – Ownership",
            "Rust Course – Borrowing",
            "Rust Course – Lifetimes",
        ]
        .iter()
        .map(|title| (*title).to_owned())
        .collect();
        super::strip_shared_prefix(&mut titles);
        assert_eq!(titles, vec!["Ownership", "Borrowing", "Lifetimes"]);
    }

    #[test]
    fn leaves_titles_alone_with_fewer_than_three_or_no_shared_words() {
        let mut two: Vec<String> = vec!["Rust Ownership".into(), "Rust Borrowing".into()];
        super::strip_shared_prefix(&mut two);
        assert_eq!(two, vec!["Rust Ownership", "Rust Borrowing"]);

        let mut none: Vec<String> =
            vec!["Ownership".into(), "Borrowing".into(), "Lifetimes".into()];
        super::strip_shared_prefix(&mut none);
        assert_eq!(none, vec!["Ownership", "Borrowing", "Lifetimes"]);
    }

    #[test]
    fn never_empties_a_title() {
        let mut separators: Vec<String> = vec![
            "Rust –".into(),
            "Rust – Borrowing".into(),
            "Rust – Lifetimes".into(),
        ];
        super::strip_shared_prefix(&mut separators);
        assert_eq!(
            separators,
            vec!["Rust –", "Rust – Borrowing", "Rust – Lifetimes"]
        );

        let mut kept: Vec<String> = vec![
            "Rust Course".into(),
            "Rust Course – Borrowing".into(),
            "Rust Course –".into(),
        ];
        super::strip_shared_prefix(&mut kept);
        assert_eq!(kept, vec!["Course", "Course – Borrowing", "Course –"]);
    }

    #[test]
    fn falls_back_to_the_input_when_nothing_is_left() {
        assert_eq!(presentable("1080p"), "1080p");
        assert_eq!(presentable("[x265]"), "[x265]");
    }
}
