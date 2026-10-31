//! Bytes a terminal receives when the user pastes text or drops files.

use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use muxy_protocol::Modes;

/// Pasted text with newlines sent as Return and escape sequences removed, so
/// the text can't end a bracketed paste early or act as keystrokes.
pub fn paste(text: &str, modes: Modes) -> Vec<u8> {
    let text = text.replace("\r\n", "\n").replace('\n', "\r");
    let text = text.replace(['\x1b', '\u{009b}'], "");
    bracket(text.into_bytes(), modes)
}

/// Absolute paths quoted for a shell and separated by spaces, as a paste.
/// Returns `None` for a relative path or one containing a NUL byte.
pub fn paths(paths: &[PathBuf], modes: Modes) -> Option<Vec<u8>> {
    let mut text = Vec::new();
    for path in paths {
        let bytes = path.as_os_str().as_bytes();
        if !path.is_absolute() || bytes.contains(&0) {
            return None;
        }
        if !text.is_empty() {
            text.push(b' ');
        }
        let ansi = bytes.iter().any(u8::is_ascii_control);
        if ansi {
            text.push(b'$');
        }
        text.push(b'\'');
        for &byte in bytes {
            match (ansi, byte) {
                (true, byte) if byte.is_ascii_control() => {
                    let hex = b"0123456789abcdef";
                    text.extend_from_slice(&[
                        b'\\',
                        b'x',
                        hex[usize::from(byte >> 4)],
                        hex[usize::from(byte & 15)],
                    ]);
                }
                (true, b'\'' | b'\\') => text.extend_from_slice(&[b'\\', byte]),
                (false, b'\'') => text.extend_from_slice(b"'\\''"),
                _ => text.push(byte),
            }
        }
        text.push(b'\'');
    }
    Some(bracket(text, modes))
}

fn bracket(text: Vec<u8>, modes: Modes) -> Vec<u8> {
    if modes.bracketed_paste && !text.is_empty() {
        [b"\x1b[200~".as_slice(), &text, b"\x1b[201~"].concat()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modes(bracketed_paste: bool) -> Modes {
        Modes {
            bracketed_paste,
            ..Modes::default()
        }
    }

    #[test]
    fn paste_normalizes_newlines_and_removes_embedded_escape_sequences() {
        let text = "first\r\nsecond\n\x1b[201~\rthird\u{009b}201~界\t";
        for bracketed_paste in [false, true] {
            let bytes = paste(text, modes(bracketed_paste));
            let expected = "first\rsecond\r[201~\rthird201~界\t";
            let expected = if bracketed_paste {
                format!("\x1b[200~{expected}\x1b[201~")
            } else {
                expected.to_owned()
            };
            assert_eq!(bytes, expected.as_bytes());
        }
        assert!(paste("", modes(true)).is_empty());
    }

    #[test]
    fn paths_quote_for_a_shell() {
        let dropped = [PathBuf::from("/tmp/it's"), PathBuf::from("/tmp/a\x1bb")];
        assert_eq!(
            paths(&dropped, modes(false)).as_deref(),
            Some(b"'/tmp/it'\\''s' $'/tmp/a\\x1bb'".as_slice())
        );
        assert_eq!(paths(&[PathBuf::from("relative")], modes(false)), None);
    }
}
