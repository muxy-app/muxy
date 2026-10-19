//! The names Windows file systems refuse, or read as something else.

/// Characters Windows refuses in a name. `:` would name an alternate data
/// stream, and `\` and `/` separate components.
const RESERVED_CHARACTERS: &[char] = &['<', '>', ':', '"', '|', '?', '*', '\\', '/'];

/// Device names Windows opens instead of a file, with any extension.
const DEVICES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$", "COM0", "COM1", "COM2", "COM3",
    "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³",
];

/// Why Windows can't hold `name` as one path component, if it can't.
pub(crate) fn windows_refusal(name: &str) -> Option<&'static str> {
    if name.is_empty() || name == "." || name == ".." {
        return Some("Invalid file name");
    }
    if name
        .chars()
        .any(|character| character.is_control() || RESERVED_CHARACTERS.contains(&character))
    {
        return Some("File names can't contain control characters or < > : \" | ? * \\ /");
    }
    if name.ends_with(['.', ' ']) {
        return Some("File names can't end with a dot or a space");
    }
    if is_device(name) {
        return Some("This name is reserved for a device on Windows");
    }
    None
}

/// `name` changed as little as possible so Windows can hold it.
pub(crate) fn windows_safe(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|character| {
            if character.is_control() || RESERVED_CHARACTERS.contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect();
    let trimmed = replaced.trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        "_".into()
    } else if is_device(trimmed) {
        format!("_{trimmed}")
    } else {
        trimmed.into()
    }
}

fn is_device(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    DEVICES.iter().any(|device| {
        device
            .chars()
            .flat_map(char::to_uppercase)
            .eq(stem.chars().flat_map(char::to_uppercase))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_refuses_streams_devices_trailing_dots_and_reserved_characters() {
        for refused in [
            "",
            ".",
            "..",
            "a:b",
            "file.txt:stream",
            "trailing.",
            "trailing ",
            "NUL",
            "nul.txt",
            "Com1.tar.gz",
            "CONIN$",
            "lpt¹",
            "a<b",
            "a|b",
            "a?b",
            "a*b",
            "a\"b",
            "a\\b",
            "a/b",
            "tab\there",
        ] {
            assert!(windows_refusal(refused).is_some(), "{refused:?}");
        }
        for allowed in [
            "notes.txt",
            ".env",
            "NULL",
            "COM10",
            "con-fig",
            "a b",
            "é.png",
        ] {
            assert_eq!(windows_refusal(allowed), None, "{allowed:?}");
        }
    }

    #[test]
    fn unsafe_names_become_the_nearest_safe_ones() {
        assert_eq!(windows_safe("shot:1.png"), "shot_1.png");
        assert_eq!(windows_safe("notes. . "), "notes");
        assert_eq!(windows_safe("aux.txt"), "_aux.txt");
        assert_eq!(windows_safe("..."), "_");
        assert_eq!(windows_safe("fine.png"), "fine.png");
        for name in ["shot:1.png", "notes. . ", "aux.txt", "...", "a\u{7}b?"] {
            assert_eq!(windows_refusal(&windows_safe(name)), None, "{name:?}");
        }
    }
}
