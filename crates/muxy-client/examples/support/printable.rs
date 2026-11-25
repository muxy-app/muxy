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
