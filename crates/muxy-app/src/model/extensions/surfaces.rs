//! Runtime changes to topbar and status-bar items (`topbar.set`,
//! `statusbar.set`) and runtime shortcuts (`shortcuts.*`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use muxy_app_core::extensions::Icon;
use muxy_app_core::settings::KeyChord;
use muxy_core::shortcuts::ShortcutSettings;
use muxy_ui::popover::PopoverAnchor;
use serde_json::{Value, json};

use super::{AppModel, Call};

/// A session-only change to one item; unset fields keep the manifest value.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Override {
    pub icon: Option<Icon>,
    pub text: Option<String>,
    pub visible: Option<bool>,
}

#[derive(Default)]
pub(crate) struct Items {
    pub(crate) topbar: HashMap<(String, String), Override>,
    pub(crate) status: HashMap<(String, String), Override>,
    anchors: RefCell<HashMap<String, PopoverAnchor>>,
    /// Template SVGs read from extension folders, keyed by resolved path.
    pub(crate) icons: HashMap<PathBuf, Arc<[u8]>>,
}

impl Items {
    pub(super) fn clear(&mut self, owner: Option<&str>) {
        let keep = |key: &(String, String)| owner.is_some_and(|owner| key.0 != owner);
        self.topbar.retain(|key, _| keep(key));
        self.status.retain(|key, _| keep(key));
    }

    /// The popover anchor an item records its bounds into while it renders.
    pub(crate) fn anchor(&self, key: &str) -> PopoverAnchor {
        self.anchors
            .borrow_mut()
            .entry(key.to_owned())
            .or_default()
            .clone()
    }
}

pub(super) struct Shortcut {
    pub(super) owner: String,
    pub(super) id: String,
    pub(super) chord: KeyChord,
}

const MODIFIERS: [(&str, &str); 4] = [
    ("cmd", "cmd"),
    ("shift", "shift"),
    ("ctrl", "ctrl"),
    ("opt", "alt"),
];

/// Parses main's combo grammar (`cmd+shift+e`, `opt+left`, …) into a chord and
/// main's canonical token string. Needs at least one of cmd, ctrl, or opt.
pub(super) fn parse_combo(combo: &str) -> Option<(KeyChord, String)> {
    let tokens: Vec<String> = combo
        .to_lowercase()
        .split('+')
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
        .collect();
    let (key, modifiers) = tokens.split_last()?;
    let mut used = [false; 4];
    for modifier in modifiers {
        let index = match modifier.as_str() {
            "cmd" | "command" | "meta" | "super" => 0,
            "shift" => 1,
            "ctrl" | "control" => 2,
            "opt" | "option" | "alt" => 3,
            _ => return None,
        };
        used[index] = true;
    }
    if !(used[0] || used[2] || used[3]) {
        return None;
    }
    let key = match key.as_str() {
        "left" | "leftarrow" => "left",
        "right" | "rightarrow" => "right",
        "up" | "uparrow" => "up",
        "down" | "downarrow" => "down",
        "tab" => "tab",
        "return" | "enter" => "return",
        "space" => "space",
        key if key.chars().count() == 1 => key,
        _ => return None,
    };
    let enabled = || MODIFIERS.iter().zip(used).filter(|(_, on)| *on);
    let chord = enabled()
        .map(|((_, gpui_name), _)| *gpui_name)
        .chain([if key == "return" { "enter" } else { key }])
        .collect::<Vec<_>>()
        .join("-")
        .parse()
        .ok()?;
    let token = enabled()
        .map(|((main_name, _), _)| *main_name)
        .chain([key])
        .collect::<Vec<_>>()
        .join("+");
    Some((chord, token))
}

/// main's token string for a chord: `cmd`, `shift`, `ctrl`, `opt`, then the key.
fn chord_token(chord: &KeyChord) -> String {
    let mut rest = chord.as_str();
    let mut used = [false; 4];
    while let Some(index) = ["cmd-", "shift-", "ctrl-", "alt-"]
        .iter()
        .position(|prefix| rest.starts_with(prefix) && rest.len() > prefix.len())
    {
        used[index] = true;
        rest = &rest[["cmd-", "shift-", "ctrl-", "alt-"][index].len()..];
    }
    let key = if rest == "enter" { "return" } else { rest };
    MODIFIERS
        .iter()
        .zip(used)
        .filter(|(_, on)| *on)
        .map(|((name, _), _)| *name)
        .chain([key])
        .collect::<Vec<_>>()
        .join("+")
}

fn title(id: &str) -> String {
    id.split('_')
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl AppModel {
    /// Runtime shortcut IDs make `command.<id>` subscribable while registered.
    pub(super) fn runtime_command_event(&self, owner: &str, event: &str) -> bool {
        event.strip_prefix("command.").is_some_and(|id| {
            self.extensions
                .shortcuts
                .iter()
                .any(|shortcut| shortcut.owner == owner && shortcut.id == id)
        })
    }

    pub(super) fn runtime_bindings(&self) -> impl Iterator<Item = (&str, &str, &KeyChord)> {
        self.extensions.shortcuts.iter().map(|shortcut| {
            (
                shortcut.owner.as_str(),
                shortcut.id.as_str(),
                &shortcut.chord,
            )
        })
    }

    fn shortcut_conflict(&self, owner: &str, id: &str, chord: &KeyChord) -> Option<String> {
        for shortcut in muxy_core::shortcuts::ALL {
            if shortcut.contexts.iter().any(|context| {
                self.settings
                    .keymap
                    .keys(shortcut.id, *context)
                    .iter()
                    .any(|key| key == chord.as_str())
            }) {
                return Some(format!("Conflicts with \"{}\"", title(shortcut.id)));
            }
        }
        self.extension_bindings()
            .iter()
            .any(|(binding, key)| {
                key == chord && !(binding.owner == owner && binding.command == id)
            })
            .then(|| "Conflicts with another extension shortcut".into())
    }

    pub(super) fn shortcuts_call(
        &mut self,
        call: &Call,
        cx: &mut gpui::Context<Self>,
    ) -> Result<Value, String> {
        let id = call.args["id"].as_str();
        match call.verb.as_str() {
            "shortcuts.register" => {
                let id = id.ok_or("missing argument 'id'")?;
                let extension = self
                    .extensions
                    .registry
                    .enabled(&call.owner)
                    .ok_or("extension is disabled or unloaded")?;
                if extension.manifest.command(id).is_some() {
                    return Err(format!(
                        "'{id}' is a manifest command; bind it via defaultShortcut"
                    ));
                }
                let combo = call.args["combo"]
                    .as_str()
                    .ok_or("missing argument 'combo'")?;
                if id.is_empty() {
                    return Err("shortcut id must not be empty".into());
                }
                let (chord, _) =
                    parse_combo(combo).ok_or_else(|| format!("invalid shortcut '{combo}'"))?;
                if let Some(conflict) = self.shortcut_conflict(&call.owner, id, &chord) {
                    return Ok(json!({"ok": false, "conflict": conflict}));
                }
                self.extensions
                    .shortcuts
                    .retain(|shortcut| !(shortcut.owner == call.owner && shortcut.id == id));
                self.extensions.shortcuts.push(Shortcut {
                    owner: call.owner.clone(),
                    id: id.into(),
                    chord,
                });
                self.invalidate_webview_shortcuts();
                self.bind_extension_keys(cx);
                Ok(json!({"ok": true}))
            }
            "shortcuts.unregister" => {
                let id = id.ok_or("missing argument 'id'")?;
                self.extensions
                    .shortcuts
                    .retain(|shortcut| !(shortcut.owner == call.owner && shortcut.id == id));
                self.invalidate_webview_shortcuts();
                self.bind_extension_keys(cx);
                Ok(Value::Null)
            }
            _ => {
                let bindings = self.extension_bindings();
                let mut list: Vec<Value> = self
                    .extensions
                    .registry
                    .enabled(&call.owner)
                    .into_iter()
                    .flat_map(|extension| &extension.manifest.commands)
                    .filter(|command| command.default_shortcut.is_some())
                    .map(|command| {
                        let combo = bindings
                            .iter()
                            .find(|(binding, _)| {
                                binding.owner == call.owner && binding.command == command.id
                            })
                            .map_or_else(String::new, |(_, chord)| chord_token(chord));
                        json!({"id": command.id, "combo": combo, "source": "manifest"})
                    })
                    .collect();
                list.extend(
                    self.extensions
                        .shortcuts
                        .iter()
                        .filter(|shortcut| shortcut.owner == call.owner)
                        .map(|shortcut| {
                            json!({
                                "id": shortcut.id,
                                "combo": chord_token(&shortcut.chord),
                                "source": "runtime",
                            })
                        }),
                );
                Ok(Value::Array(list))
            }
        }
    }

    /// `topbar.set` / `statusbar.set`: session-only icon, text, and visibility.
    pub(super) fn set_item(
        &mut self,
        call: &Call,
        cx: &mut gpui::Context<Self>,
    ) -> Result<Value, String> {
        let status = call.verb == "statusbar.set";
        let id = call.args["id"].as_str().ok_or("missing argument 'id'")?;
        let declared = self
            .extensions
            .registry
            .enabled(&call.owner)
            .is_some_and(|extension| {
                if status {
                    extension
                        .manifest
                        .status_bar_items
                        .iter()
                        .any(|item| item.id == id)
                } else {
                    extension
                        .manifest
                        .topbar_items
                        .iter()
                        .any(|item| item.id == id)
                }
            });
        if !declared {
            return Err(if status {
                format!("unknown status bar item '{id}'")
            } else {
                format!("unknown topbar item '{id}'")
            });
        }
        let items = if status {
            &mut self.extensions.items.status
        } else {
            &mut self.extensions.items.topbar
        };
        let entry = items.entry((call.owner.clone(), id.into())).or_default();
        if let Some(icon) = Icon::parse(&call.args["icon"]) {
            entry.icon = Some(icon);
        }
        if let Some(visible) = call.args["visible"].as_bool() {
            entry.visible = Some(visible);
        }
        if status && call.args.get("text").is_some() {
            entry.text = call.args["text"]
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned);
        }
        if *entry == Override::default() {
            items.remove(&(call.owner.clone(), id.into()));
        }
        self.load_extension_icons(cx);
        self.close_hidden_popover(cx);
        cx.notify();
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos_follow_main_grammar() {
        let (chord, token) = parse_combo(" Command + Option + K ").expect("combo");
        assert_eq!((chord.as_str(), token.as_str()), ("cmd-alt-k", "cmd+opt+k"));
        let (chord, token) = parse_combo("ctrl+shift+Return").expect("named key");
        assert_eq!(
            (chord.as_str(), token.as_str()),
            ("ctrl-shift-enter", "shift+ctrl+return")
        );
        assert!(parse_combo("shift+k").is_none());
        assert!(parse_combo("cmd+hyper+k").is_none());
        assert!(parse_combo("cmd+f13").is_none());
        assert_eq!(chord_token(&chord), "shift+ctrl+return");
        assert_eq!(title("toggle_command_palette"), "Toggle Command Palette");
    }
}
