use super::session::InputMode;

pub enum KeycapPart {
    Symbol(&'static str),
    Text(&'static str),
}

pub struct Shortcut {
    pub parts: &'static [KeycapPart],
    pub label: String,
}

const NAVIGATE: &[KeycapPart] = &[
    KeycapPart::Symbol("arrow.up"),
    KeycapPart::Symbol("arrow.down"),
];
const TAB: &[KeycapPart] = &[KeycapPart::Text("Tab")];
const RETURN: &[KeycapPart] = &[KeycapPart::Symbol("return")];
const COMMAND_RETURN: &[KeycapPart] =
    &[KeycapPart::Symbol("command"), KeycapPart::Symbol("return")];
const ESCAPE: &[KeycapPart] = &[KeycapPart::Text("Esc")];
const OPTION_DELETE: &[KeycapPart] = &[
    KeycapPart::Symbol("option"),
    KeycapPart::Symbol("delete.left"),
];

pub fn ordered(mode: InputMode, action_title: &str) -> Vec<Shortcut> {
    let shortcut = |parts: &'static [KeycapPart], label: &str| Shortcut {
        parts,
        label: label.to_owned(),
    };

    match mode {
        InputMode::FolderSearch => vec![
            shortcut(NAVIGATE, "Navigate"),
            shortcut(RETURN, action_title),
            shortcut(TAB, "Use Path"),
            shortcut(ESCAPE, "Close"),
        ],
        InputMode::Path => vec![
            shortcut(NAVIGATE, "Navigate"),
            shortcut(TAB, "Autocomplete"),
            shortcut(RETURN, "Open"),
            shortcut(COMMAND_RETURN, action_title),
            shortcut(OPTION_DELETE, "Go back"),
            shortcut(ESCAPE, "Close"),
        ],
    }
}
