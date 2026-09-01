use crate::confirmation::{ConfirmationId, ConfirmationKind};
use crate::scrollbar::ScrollbarMetrics;
use muxy_core::shortcuts::KeyCombo;
pub use muxy_core::terminal_launch::{shell_escape, startup_shell_command, user_shell};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchTotals {
    pub active: bool,
    pub total: Option<usize>,
    pub selected: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceProgressKind {
    Set,
    Error,
    Indeterminate,
    Paused,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceProgress {
    pub kind: Option<SurfaceProgressKind>,
    pub value: Option<f32>,
}

impl SurfaceProgress {
    pub fn is_active(self) -> bool {
        self.kind.is_some()
    }

    pub fn fraction(self) -> f32 {
        self.value.unwrap_or(1.0).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfaceMetadata {
    pub title: Option<String>,
    pub working_directory: Option<String>,
    pub bell_generation: u64,
    pub progress: SurfaceProgress,
    pub search_totals: SearchTotals,
    pub scrollbar: ScrollbarMetrics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SurfaceAction {
    Copy,
    Paste,
    ClipboardDecision { id: ConfirmationId, approved: bool },
    SearchStart,
    SearchQuery(String),
    SearchNext,
    SearchPrevious,
    SearchEnd,
    ScrollToRow(u64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceSignal {
    Data(Vec<u8>),
    DataGap,
    Metadata(SurfaceMetadata),
    DesktopNotification {
        title: String,
        body: String,
    },
    Exited,
    Confirm {
        id: ConfirmationId,
        kind: ConfirmationKind,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    Other,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub platform: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerInput {
    Moved {
        x: f64,
        y: f64,
        modifiers: PointerModifiers,
    },
    Down {
        x: f64,
        y: f64,
        button: PointerButton,
        modifiers: PointerModifiers,
        click_count: usize,
    },
    Up {
        x: f64,
        y: f64,
        button: PointerButton,
        modifiers: PointerModifiers,
    },
}

pub trait TerminalSurfaceHandle {
    fn set_focused(&self, focused: bool);
    fn set_occluded(&self, occluded: bool);
    fn set_pointer_inside(&self, inside: bool);
    fn set_input_transaction_active(&self, _active: bool) {}
    fn cancel_input_transaction(&self) {}
    fn has_native_scrollbar(&self) -> bool {
        false
    }
    fn has_selection(&self) -> bool;
    fn send_text(&self, _text: &str) -> bool {
        false
    }
    fn send_bytes(&self, _bytes: &[u8]) -> bool {
        false
    }
    fn read_screen_text(&self, _last_lines: usize) -> Option<String> {
        None
    }
    fn foreground_pid(&self) -> Option<u64> {
        None
    }
    fn is_alternate_screen(&self) -> Option<bool> {
        None
    }
    fn metadata(&self) -> &SurfaceMetadata;
    fn perform(&self, action: SurfaceAction) -> bool;
    fn forward_pointer(&self, input: PointerInput) -> bool;
    fn apply(&mut self, signal: SurfaceSignal) -> bool;
    fn refresh(&mut self) -> bool;
    fn needs_confirm_close(&self) -> bool;
    fn request_close(&self);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalDrop {
    pub file_values: Vec<String>,
    pub plain_text: Option<String>,
}

impl ExternalDrop {
    pub fn is_empty(&self) -> bool {
        self.file_values.is_empty() && self.plain_text.as_deref().is_none_or(str::is_empty)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchCommand {
    pub command: String,
    pub keeps_shell_open: bool,
}

pub const NS_UP_ARROW: u32 = 0xF700;
pub const NS_DOWN_ARROW: u32 = 0xF701;
pub const NS_LEFT_ARROW: u32 = 0xF702;
pub const NS_RIGHT_ARROW: u32 = 0xF703;

pub fn normalize_key(characters: &str, key_code: u16) -> Option<String> {
    let mut chars = characters.chars();
    let first = chars.next();
    if let Some(character) = first
        && chars.next().is_none()
    {
        let code = character as u32;
        let named = match code {
            NS_UP_ARROW => Some("uparrow"),
            NS_DOWN_ARROW => Some("downarrow"),
            NS_LEFT_ARROW => Some("leftarrow"),
            NS_RIGHT_ARROW => Some("rightarrow"),
            0x0D | 0x03 => Some("return"),
            0x09 | 0x19 => Some("tab"),
            0x1B => Some("escape"),
            0x20 => Some("space"),
            0x7F | 0x08 => Some("delete"),
            _ => None,
        };
        if let Some(named) = named {
            return Some(named.to_owned());
        }
        if (0x21..=0x7E).contains(&code) {
            return Some(character.to_lowercase().to_string());
        }
    }
    key_name_for_code(key_code).map(str::to_owned)
}

fn key_name_for_code(key_code: u16) -> Option<&'static str> {
    Some(match key_code {
        0x24 => "return",
        0x30 => "tab",
        0x31 => "space",
        0x33 => "delete",
        0x35 => "escape",
        0x7B => "leftarrow",
        0x7C => "rightarrow",
        0x7D => "downarrow",
        0x7E => "uparrow",
        _ => return None,
    })
}

pub struct ShortcutGate {
    combos: Vec<KeyCombo>,
}

impl ShortcutGate {
    pub fn new(combos: Vec<KeyCombo>) -> Self {
        Self {
            combos: combos
                .into_iter()
                .filter(|combo| combo.is_assigned())
                .collect(),
        }
    }

    pub fn declines(&self, characters: &str, key_code: u16, modifiers: u64) -> bool {
        let Some(key) = normalize_key(characters, key_code) else {
            return false;
        };
        self.combos
            .iter()
            .any(|combo| combo.modifiers == modifiers && combo.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::shortcuts::{COMMAND, OPTION, SHIFT};

    #[test]
    fn resolves_function_keys_and_lowercases_shifted_letters() {
        let left = char::from_u32(NS_LEFT_ARROW).unwrap().to_string();
        assert_eq!(normalize_key(&left, 0x7B).as_deref(), Some("leftarrow"));
        assert_eq!(normalize_key("D", 0x02).as_deref(), Some("d"));
        assert_eq!(normalize_key("d", 0x02).as_deref(), Some("d"));
        assert_eq!(normalize_key("\r", 0x24).as_deref(), Some("return"));
    }

    #[test]
    fn falls_back_to_the_keycode_table_for_unprintable_characters() {
        assert_eq!(normalize_key("", 0x7E).as_deref(), Some("uparrow"));
        assert_eq!(normalize_key("", 0x30).as_deref(), Some("tab"));
        assert_eq!(normalize_key("", 0x00), None);
    }

    #[test]
    fn declines_assigned_combos_after_normalisation() {
        let gate = ShortcutGate::new(vec![
            KeyCombo {
                key: "leftarrow".to_owned(),
                modifiers: COMMAND | OPTION,
            },
            KeyCombo {
                key: "d".to_owned(),
                modifiers: COMMAND | SHIFT,
            },
        ]);
        let left = char::from_u32(NS_LEFT_ARROW).unwrap().to_string();
        assert!(gate.declines(&left, 0x7B, COMMAND | OPTION));
        assert!(gate.declines("D", 0x02, COMMAND | SHIFT));
        assert!(!gate.declines("d", 0x02, COMMAND));
        assert!(!gate.declines("c", 0x08, COMMAND));
    }

    #[test]
    fn ignores_unassigned_combos() {
        let gate = ShortcutGate::new(vec![KeyCombo {
            key: String::new(),
            modifiers: 0,
        }]);
        assert!(!gate.declines("", 0x00, 0));
    }

    #[test]
    fn external_drop_is_neutral_and_distinguishes_empty_content() {
        assert!(
            ExternalDrop {
                file_values: Vec::new(),
                plain_text: None,
            }
            .is_empty()
        );
        assert!(
            !ExternalDrop {
                file_values: vec!["file:///tmp/a".to_owned()],
                plain_text: Some("fallback".to_owned()),
            }
            .is_empty()
        );
    }

    #[test]
    fn desktop_notification_signal_is_owned_and_separate_from_persistent_metadata() {
        let metadata = SurfaceMetadata::default();
        let signal = SurfaceSignal::DesktopNotification {
            title: "Done".to_owned(),
            body: "Ready".to_owned(),
        };
        assert!(matches!(
            signal,
            SurfaceSignal::DesktopNotification { title, body }
                if title == "Done" && body == "Ready"
        ));
        assert_eq!(metadata, SurfaceMetadata::default());
    }

    #[test]
    fn progress_distinguishes_absent_indeterminate_and_bounded_values() {
        assert!(!SurfaceProgress::default().is_active());
        assert_eq!(SurfaceProgress::default().fraction(), 1.0);
        assert!(
            SurfaceProgress {
                kind: Some(SurfaceProgressKind::Indeterminate),
                value: None,
            }
            .is_active()
        );
        assert_eq!(
            SurfaceProgress {
                kind: Some(SurfaceProgressKind::Set),
                value: Some(1.5),
            }
            .fraction(),
            1.0
        );
    }
}
