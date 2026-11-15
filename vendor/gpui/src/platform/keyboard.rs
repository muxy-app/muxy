use collections::HashMap;

use crate::{KeybindingKeystroke, Keystroke};

pub trait PlatformKeyboardLayout {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
}

pub trait PlatformKeyboardMapper {
    fn map_key_equivalent(
        &self,
        keystroke: Keystroke,
        use_key_equivalents: bool,
    ) -> KeybindingKeystroke;
    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>>;
}

pub struct DummyKeyboardMapper;

impl PlatformKeyboardMapper for DummyKeyboardMapper {
    fn map_key_equivalent(
        &self,
        keystroke: Keystroke,
        _use_key_equivalents: bool,
    ) -> KeybindingKeystroke {
        KeybindingKeystroke::from_keystroke(keystroke)
    }

    fn get_key_equivalents(&self) -> Option<&HashMap<char, char>> {
        None
    }
}
