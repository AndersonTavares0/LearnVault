//! Visual cleanup of course, module and lesson titles.
//!
//! It removes only the typical noise of file names: resolution and encoding tags
//! (`1080p`, `x265`, `WEB-DL`), underscores used as spaces and repeated spaces. It does not
//! change the name on disk nor the path that identifies the node; it is presentation only.

/// Release and video-encoding tags that are not part of a lesson's title.
///
/// Audio (`aac`, `mp3`) and terms such as `4K` are left out on purpose: they may be part of a
/// lesson's subject.
const RELEASE_TAGS: &[&str] = &[
    "webrip", "web-dl", "webdl", "hdtv", "bluray", "brrip", "dvdrip", "hdrip", "hevc", "avc",
    "x264", "x265", "h264", "h265", "10bit", "8bit",
];

/// Title without release tags, with underscores swapped for spaces.
///
/// If the cleanup erases everything (for example, a name that is just `1080p`), it returns the original
/// input, so the node never ends up with no title.
pub fn presentable(title: &str) -> String {
    let mut spaced = title.replace('_', " ");
    // shortcut: a title with no spaces and 2+ dots is a release-style name ("Go.Golang.Course");
    // one dot ("Node.js") or a title with spaces keeps its dots.
    if !spaced.trim().contains(char::is_whitespace) && spaced.matches('.').count() >= 2 {
        spaced = spaced.replace('.', " ");
    }
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

/// Removes from the start of all titles the words they have in common, such as the name of the
/// course repeated on every lesson ("Rust Course – Ownership", "Rust Course – Borrowing").
///
/// It only acts with at least three titles, keeps at least one word in each and changes
/// nothing if some title would end up empty or with only separators.
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

/// True for a word that is only a release tag, even between brackets.
fn is_release_tag(token: &str) -> bool {
    let word = token
        .trim_matches(|character: char| "[](){}".contains(character))
        .to_ascii_lowercase();
    !word.is_empty() && (RELEASE_TAGS.contains(&word.as_str()) || is_resolution(&word))
}

/// `720p`, `1080p`, `2160p`: three or four digits followed by `p`.
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
    fn turns_the_dots_of_a_release_style_name_into_spaces() {
        assert_eq!(
            presentable("Algoritmos.e.Logica.de.Programacao-O.Curso.COMPLETO-CL"),
            "Algoritmos e Logica de Programacao-O Curso COMPLETO-CL"
        );
        assert_eq!(presentable("Node.js"), "Node.js");
        assert_eq!(presentable("Python 3.12.1 basics"), "Python 3.12.1 basics");
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
