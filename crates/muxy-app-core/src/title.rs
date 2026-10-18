use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
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
    let directory = Path::new(OsStr::from_bytes(&directory.0));
    let basename = directory.file_name().unwrap_or(directory.as_os_str());
    sanitize_or_default(&basename.to_string_lossy())
}
