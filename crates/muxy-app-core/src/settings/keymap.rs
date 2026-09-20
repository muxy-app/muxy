use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::settings::{Error, KeyChord, Result};
use muxy_core::shortcuts::{self, Shortcut, ShortcutId, ShortcutSettings};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(into = "BTreeMap<String, KeyChord>")]
pub struct Keymap(BTreeMap<String, KeyChord>, BTreeMap<String, String>);

impl Default for Keymap {
    fn default() -> Self {
        Self(
            shortcuts::ALL
                .iter()
                .filter_map(|shortcut| {
                    shortcut
                        .keys
                        .first()
                        .map(|key| (shortcut.id.to_owned(), KeyChord::default_binding(key)))
                })
                .collect(),
            BTreeMap::new(),
        )
    }
}

impl Keymap {
    pub const ACTIONS: [ShortcutId; 49] = [
        ShortcutId::OpenSettings,
        ShortcutId::NewHomeTab,
        ShortcutId::ToggleSidebar,
        ShortcutId::ToggleFullScreen,
        ShortcutId::ToggleThemePicker,
        ShortcutId::ToggleCommandPalette,
        ShortcutId::NavigateBack,
        ShortcutId::NavigateForward,
        ShortcutId::Quit,
        ShortcutId::HideApp,
        ShortcutId::HideOthers,
        ShortcutId::Minimize,
        ShortcutId::NewTab,
        ShortcutId::ExistingTerminals,
        ShortcutId::DetachTerminal,
        ShortcutId::CloseTab,
        ShortcutId::SplitRight,
        ShortcutId::SplitDown,
        ShortcutId::FocusPaneLeft,
        ShortcutId::FocusPaneRight,
        ShortcutId::FocusPaneUp,
        ShortcutId::FocusPaneDown,
        ShortcutId::ToggleZoomPane,
        ShortcutId::ClosePane,
        ShortcutId::NextTab,
        ShortcutId::PreviousTab,
        ShortcutId::PreviousProject,
        ShortcutId::NextProject,
        ShortcutId::AddProject,
        ShortcutId::SelectTab1,
        ShortcutId::SelectTab2,
        ShortcutId::SelectTab3,
        ShortcutId::SelectTab4,
        ShortcutId::SelectTab5,
        ShortcutId::SelectTab6,
        ShortcutId::SelectTab7,
        ShortcutId::SelectTab8,
        ShortcutId::SelectTab9,
        ShortcutId::Copy,
        ShortcutId::Paste,
        ShortcutId::Find,
        ShortcutId::FindNext,
        ShortcutId::FindPrevious,
        ShortcutId::ScrollToBottom,
        ShortcutId::PreviousPrompt,
        ShortcutId::NextPrompt,
        ShortcutId::SelectCommandOutput,
        ShortcutId::IncreaseFontSize,
        ShortcutId::DecreaseFontSize,
    ];

    pub fn chord(&self, action: ShortcutId) -> Option<&KeyChord> {
        self.0.get(action.name())
    }

    pub fn action(&self, chord: &KeyChord) -> Option<ShortcutId> {
        Self::ACTIONS
            .into_iter()
            .find(|action| self.chord(*action) == Some(chord))
    }

    pub fn binding(&self, id: &str) -> Option<&KeyChord> {
        self.0.get(id)
    }

    pub fn with_binding(&self, id: &str, chord: Option<KeyChord>) -> Result<Self> {
        if shortcuts::find(id).is_none() && !extension_action(id) {
            return Err(Error::new("keymap", "unknown action"));
        }
        let reset = chord.is_none();
        let mut overrides = self.1.clone();
        if let Some(chord) = chord {
            overrides.insert(id.into(), chord.to_string());
        } else {
            overrides.remove(id);
        }
        let resolved = Self::from_overrides(overrides)?;
        if reset && !extension_action(id) && resolved.binding(id) != Self::default().binding(id) {
            return Err(Error::new(
                format!("keymap.{id}"),
                "the default shortcut is assigned to another action; reset that action first",
            ));
        }
        Ok(resolved)
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        crate::settings::appearance::replace_section(path, "keymap", &self.1)
            .map_err(|error| Error::new("keymap", error))
    }

    fn from_overrides(overrides: BTreeMap<String, String>) -> Result<Self> {
        let mut keymap = Self(Self::default().0, overrides.clone());
        let mut explicit = BTreeMap::new();
        for (name, value) in overrides {
            let key = format!("keymap.{name}");
            if shortcuts::find(&name).is_none() && !extension_action(&name) {
                return Err(Error::new(&key, "unknown action"));
            }
            let chord = value
                .parse()
                .map_err(|error: Error| Error::new(&key, error))?;
            explicit.insert(name, chord);
        }
        for action in [
            ShortcutId::OpenSettings,
            ShortcutId::NewHomeTab,
            ShortcutId::ToggleSidebar,
            ShortcutId::ToggleFullScreen,
            ShortcutId::ToggleThemePicker,
            ShortcutId::ToggleCommandPalette,
            ShortcutId::NavigateBack,
            ShortcutId::NavigateForward,
            ShortcutId::Quit,
            ShortcutId::HideApp,
            ShortcutId::HideOthers,
            ShortcutId::Minimize,
            ShortcutId::PreviousPrompt,
            ShortcutId::NextPrompt,
            ShortcutId::Find,
            ShortcutId::FindNext,
            ShortcutId::FindPrevious,
            ShortcutId::PreviousProject,
            ShortcutId::NextProject,
            ShortcutId::AddProject,
            ShortcutId::CloseTab,
            ShortcutId::SplitRight,
            ShortcutId::SplitDown,
            ShortcutId::FocusPaneLeft,
            ShortcutId::FocusPaneRight,
            ShortcutId::FocusPaneUp,
            ShortcutId::FocusPaneDown,
            ShortcutId::ToggleZoomPane,
            ShortcutId::ClosePane,
        ] {
            if !explicit.contains_key(action.name())
                && explicit.iter().any(|(id, chord)| {
                    Some(chord) == keymap.chord(action) && same_scope(id, action.name())
                })
            {
                keymap.0.remove(action.name());
            }
        }
        keymap.0.extend(explicit);
        let bindings: Vec<_> = keymap.0.iter().collect();
        for (index, (id, chord)) in bindings.iter().enumerate() {
            for (other, other_chord) in &bindings[..index] {
                if chord == other_chord && same_scope(id, other) {
                    return Err(Error::new(
                        format!("keymap.{id}"),
                        format!("{chord} is also bound to keymap.{other}"),
                    ));
                }
            }
        }
        let defaults = Self::default();
        keymap
            .1
            .retain(|id, chord| chord.parse::<KeyChord>().ok().as_ref() != defaults.0.get(id));
        Ok(keymap)
    }
}

impl<'de> Deserialize<'de> for Keymap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Self::from_overrides(BTreeMap::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

fn overlaps(left: &Shortcut, right: &Shortcut) -> bool {
    left.contexts.iter().any(|context| {
        right
            .contexts
            .iter()
            .any(|other| context_overlap(*context, *other))
    })
}

fn extension_action(id: &str) -> bool {
    id.starts_with("extension.") && id.len() <= 300 && id.split('.').count() >= 3
}

fn same_scope(left: &str, right: &str) -> bool {
    if extension_action(left) || extension_action(right) {
        return true;
    }
    shortcuts::find(left)
        .zip(shortcuts::find(right))
        .is_some_and(|(left, right)| overlaps(left, right))
}

impl ShortcutSettings for Keymap {
    fn keys(&self, id: &str, context: Option<&str>) -> Vec<String> {
        let Some(primary) = self.0.get(id) else {
            return Vec::new();
        };
        let Some(shortcut) = shortcuts::find(id) else {
            return if extension_action(id) {
                vec![primary.as_str().to_owned()]
            } else {
                Vec::new()
            };
        };
        let is_default = shortcut.keys.first().is_some_and(|key| {
            key.parse::<KeyChord>()
                .is_ok_and(|default| default == *primary)
        });
        if !is_default {
            return vec![primary.as_str().to_owned()];
        }
        shortcut
            .keys
            .iter()
            .zip(shortcut.key_contexts)
            .filter(|(_, scopes)| scopes.contains(&context))
            .filter_map(|(key, _)| key.parse::<KeyChord>().ok())
            .filter(|key| {
                !self.0.iter().any(|(other, chord)| {
                    other != id && chord == key && primary_applies(other, chord, context)
                })
            })
            .map(|key| key.as_str().to_owned())
            .collect()
    }
}

fn context_overlap(left: Option<&str>, right: Option<&str>) -> bool {
    fn scope(context: Option<&str>) -> Option<&str> {
        context.map(|context| {
            if context == shortcuts::WORKSPACE_CLIPBOARD_CONTEXT {
                "WorkspaceTabs"
            } else {
                context
            }
        })
    }
    left.is_none() || right.is_none() || scope(left) == scope(right)
}

fn primary_applies(id: &str, chord: &KeyChord, context: Option<&str>) -> bool {
    let Some(shortcut) = shortcuts::find(id) else {
        return false;
    };
    let is_default = shortcut.keys.first().is_some_and(|key| {
        key.parse::<KeyChord>()
            .is_ok_and(|default| default == *chord)
    });
    let contexts = if is_default {
        shortcut.key_contexts[0]
    } else {
        shortcut.contexts
    };
    contexts
        .iter()
        .any(|other| context_overlap(context, *other))
}

impl From<Keymap> for BTreeMap<String, KeyChord> {
    fn from(keymap: Keymap) -> Self {
        keymap.0
    }
}
