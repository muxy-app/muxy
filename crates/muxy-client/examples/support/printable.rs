//! Shared by the examples: text from a server, shown on this terminal.

/// `text` with every control character shown as an escape, as the terminal UI
/// refuses to draw them. Cell text, paths and messages come from the server,
/// so printed raw they could drive this terminal, such as setting its
/// clipboard.
pub(crate) fn printable(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control() {
            shown.extend(character.escape_default());
        } else {
            shown.push(character);
        }
    }
    shown
}
