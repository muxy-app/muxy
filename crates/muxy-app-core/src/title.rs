use std::sync::LazyLock;

use muxy_protocol::{ForegroundProcess, ServerPath};
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

#[allow(
    clippy::expect_used,
    reason = "The Unicode pattern is a tested constant"
)]
static DECORATIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"[\p{So}\p{Co}\p{Emoji_Presentation}\u{20e3}]|\p{Extended_Pictographic}\u{fe0f}|^[\u{200d}\u{fe0e}\u{fe0f}\u{e0020}-\u{e007f}]+$",
    )
    .expect("valid title decoration pattern")
});

pub(crate) fn sanitize(title: &str) -> String {
    let title = title.trim();
    if title.is_ascii() || !DECORATIONS.is_match(title) {
        return title.to_owned();
    }
    let title: String = title
        .graphemes(true)
        .filter(|grapheme| !DECORATIONS.is_match(grapheme))
        .collect();
    title.trim().to_owned()
}

pub(crate) fn sanitize_or_default(title: &str) -> String {
    let title = sanitize(title);
    if title.is_empty() {
        "Terminal".into()
    } else {
        title
    }
}

pub fn derive(title: &str, process: Option<&ForegroundProcess>, directory: &ServerPath) -> String {
    let title = process
        .filter(|process| matches!(process.name.as_str(), "omp" | "oh-my-pi"))
        .and_then(|_| title.strip_prefix("π "))
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(title);
    let title = sanitize(title);
    if !title.is_empty() {
        return title;
    }
    if let Some(process) = process
        && !process.is_shell
    {
        let title = sanitize(&process.name);
        if !title.is_empty() {
            return title;
        }
    }
    sanitize_or_default(&folder_name(directory))
}

/// The last component of a Unix or a Windows server's path, or the whole
/// path for a root such as `/`.
fn folder_name(directory: &ServerPath) -> String {
    let path = String::from_utf8_lossy(&directory.0);
    let windows = matches!(directory.0.as_slice(), [_, b':', ..] | [b'\\', b'\\', ..]);
    let separator = |c: char| c == '/' || (windows && c == '\\');
    match path.trim_end_matches(separator).rsplit(separator).next() {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => path.into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(path: &str) -> String {
        derive("", None, &ServerPath(path.as_bytes().to_vec()))
    }

    #[test]
    fn a_terminal_without_a_title_is_named_after_its_folder_on_any_server() {
        assert_eq!(folder("/Users/me/src/muxy"), "muxy");
        assert_eq!(folder("/Users/me/src/muxy/"), "muxy");
        assert_eq!(folder(r"C:\Users\Me\src\muxy"), "muxy");
        assert_eq!(folder(r"C:\Users\Me\src\muxy\"), "muxy");
        assert_eq!(folder("C:/Users/Me/src"), "src");
        assert_eq!(folder(r"\\server\share\docs"), "docs");
        assert_eq!(folder(r"/home/me/a\b"), r"a\b");
        assert_eq!(folder("/"), "/");
        assert_eq!(folder(r"C:\"), "C:");
        assert_eq!(folder(""), "Terminal");
    }
}
