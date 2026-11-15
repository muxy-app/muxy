use std::path::Path;
use std::sync::LazyLock;

use crate::{ForegroundProcess, ServerPath};
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

pub fn sanitize(title: &str) -> String {
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

pub fn sanitize_or_default(title: &str) -> String {
    let title = sanitize(title);
    if title.is_empty() {
        "Terminal".into()
    } else {
        title
    }
}

pub fn derive(title: &str, process: Option<&ForegroundProcess>, directory: &ServerPath) -> String {
    derive_with(title, process, directory, false)
}

pub fn derive_for_header(
    title: &str,
    process: Option<&ForegroundProcess>,
    directory: &ServerPath,
) -> String {
    derive_with(title, process, directory, true)
}

pub fn directory_for_header(directory: &ServerPath, home: Option<&Path>) -> Option<String> {
    let directory = directory.as_path();
    if directory.as_os_str().is_empty() {
        return None;
    }
    let relative = home
        .filter(|home| home.parent().is_some())
        .and_then(|home| directory.strip_prefix(home).ok());
    Some(match relative {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => rest
            .components()
            .fold(String::from("~"), |mut shown, part| {
                shown.push(std::path::MAIN_SEPARATOR);
                shown.push_str(&part.as_os_str().to_string_lossy());
                shown
            }),
        None => directory.to_string_lossy().into_owned(),
    })
}

fn derive_with(
    title: &str,
    process: Option<&ForegroundProcess>,
    directory: &ServerPath,
    shell: bool,
) -> String {
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
        && (shell || !process.is_shell)
    {
        let name = if process.is_shell {
            process.name.trim_start_matches('-')
        } else {
            &process.name
        };
        let title = sanitize(name);
        if !title.is_empty() {
            return title;
        }
    }
    sanitize_or_default(&folder_name(directory))
}

pub(crate) fn folder_name(directory: &ServerPath) -> String {
    let path = String::from_utf8_lossy(&directory.0);
    let windows = matches!(directory.0.as_slice(), [_, b':', ..] | [b'\\', b'\\', ..]);
    let separator = |c: char| c == '/' || (windows && c == '\\');
    match path.trim_end_matches(separator).rsplit(separator).next() {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => path.into_owned(),
    }
}
