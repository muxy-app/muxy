use crate::shortcuts::{KeyCombo, canonical_key, legacy_virtual_key_code};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum QuickTerminalShortcut {
    #[serde(rename = "unassigned")]
    Unassigned,
    #[serde(rename = "doubleShift")]
    DoubleShift,
    #[serde(rename = "keyCombo")]
    KeyCombo {
        #[serde(rename = "keyCombo")]
        key_combo: KeyCombo,
        #[serde(rename = "virtualKeyCode")]
        virtual_key_code: u16,
    },
}

impl<'de> Deserialize<'de> for QuickTerminalShortcut {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "type")]
        enum StoredShortcut {
            #[serde(rename = "unassigned")]
            Unassigned,
            #[serde(rename = "doubleShift")]
            DoubleShift,
            #[serde(rename = "keyCombo")]
            KeyCombo {
                #[serde(rename = "keyCombo")]
                key_combo: KeyCombo,
                #[serde(default, rename = "virtualKeyCode")]
                virtual_key_code: Option<u16>,
            },
        }

        match StoredShortcut::deserialize(deserializer)? {
            StoredShortcut::Unassigned => Ok(Self::Unassigned),
            StoredShortcut::DoubleShift => Ok(Self::DoubleShift),
            StoredShortcut::KeyCombo {
                key_combo,
                virtual_key_code,
            } => {
                let virtual_key_code = virtual_key_code
                    .or_else(|| legacy_virtual_key_code(&key_combo.key))
                    .ok_or_else(|| serde::de::Error::custom("unsupported virtual key code"))?;
                Ok(Self::KeyCombo {
                    key_combo,
                    virtual_key_code,
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistrationIdentity {
    pub modifiers: u64,
    pub virtual_key_code: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConflictCandidate {
    pub label: String,
    pub combo: KeyCombo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutConflict {
    pub label: String,
}

impl QuickTerminalShortcut {
    pub fn canonicalized(
        &self,
        mut key_resolver: impl FnMut(u16) -> Option<String>,
    ) -> Option<Self> {
        match self {
            Self::Unassigned => Some(Self::Unassigned),
            Self::DoubleShift => Some(Self::DoubleShift),
            Self::KeyCombo {
                virtual_key_code, ..
            } => {
                let identity = self.registration_identity()?;
                let key = canonical_key(&key_resolver(*virtual_key_code)?);
                let key_combo = KeyCombo::new(&key, identity.modifiers);
                key_combo.is_supported_shortcut().then_some(Self::KeyCombo {
                    key_combo,
                    virtual_key_code: *virtual_key_code,
                })
            }
        }
    }

    pub fn registration_identity(&self) -> Option<RegistrationIdentity> {
        let Self::KeyCombo {
            key_combo,
            virtual_key_code,
        } = self
        else {
            return None;
        };
        (key_combo.is_supported_shortcut() && *virtual_key_code <= 127).then_some(
            RegistrationIdentity {
                modifiers: key_combo.modifiers,
                virtual_key_code: *virtual_key_code,
            },
        )
    }

    pub fn key_combo(&self) -> Option<&KeyCombo> {
        match self {
            Self::KeyCombo { key_combo, .. } => Some(key_combo),
            Self::Unassigned | Self::DoubleShift => None,
        }
    }

    pub fn find_conflict(
        &self,
        candidates: &[ConflictCandidate],
        key_resolver: impl FnMut(u16) -> Option<String>,
    ) -> Option<ShortcutConflict> {
        let shortcut = self.canonicalized(key_resolver)?;
        let combo = shortcut.key_combo()?;
        candidates
            .iter()
            .find(|candidate| combo.conflicts_with(&candidate.combo))
            .map(|candidate| ShortcutConflict {
                label: candidate.label.clone(),
            })
    }
}

pub fn load_from(
    path: &Path,
    key_resolver: impl FnMut(u16) -> Option<String>,
) -> QuickTerminalShortcut {
    std::fs::read(path)
        .ok()
        .and_then(|contents| serde_json::from_slice::<QuickTerminalShortcut>(&contents).ok())
        .and_then(|shortcut| shortcut.canonicalized(key_resolver))
        .unwrap_or(QuickTerminalShortcut::Unassigned)
}

pub fn save_to(path: &Path, shortcut: &QuickTerminalShortcut) -> std::io::Result<()> {
    if matches!(shortcut, QuickTerminalShortcut::KeyCombo { .. })
        && shortcut.registration_identity().is_none()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid Quick Terminal shortcut",
        ));
    }
    let value = serde_json::to_value(shortcut)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let contents = crate::prefs::settings::to_foundation_json(&value, true, false);
    crate::store::write_private(path, contents.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{ConflictCandidate, QuickTerminalShortcut, load_from, save_to};
    use crate::shortcuts::{COMMAND, KeyCombo, SHIFT};
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;

    fn resolver(code: u16) -> Option<String> {
        match code {
            0 => Some("a".to_owned()),
            36 => Some("return".to_owned()),
            49 => Some("space".to_owned()),
            123 => Some("leftarrow".to_owned()),
            _ => None,
        }
    }

    #[test]
    fn quick_terminal_shortcut_exact_variants_round_trip() {
        let cases = [
            (
                QuickTerminalShortcut::Unassigned,
                json!({ "type": "unassigned" }),
            ),
            (
                QuickTerminalShortcut::DoubleShift,
                json!({ "type": "doubleShift" }),
            ),
            (
                QuickTerminalShortcut::KeyCombo {
                    key_combo: KeyCombo::new("space", COMMAND),
                    virtual_key_code: 49,
                },
                json!({
                    "type": "keyCombo",
                    "keyCombo": { "key": "space", "modifiers": COMMAND },
                    "virtualKeyCode": 49
                }),
            ),
        ];
        for (shortcut, expected) in cases {
            let encoded = serde_json::to_value(&shortcut).unwrap();
            assert_eq!(encoded, expected);
            assert_eq!(
                serde_json::from_value::<QuickTerminalShortcut>(encoded).unwrap(),
                shortcut
            );
        }
    }

    #[test]
    fn quick_terminal_legacy_combo_derives_the_physical_key_code() {
        let value = json!({
            "type": "keyCombo",
            "keyCombo": { "key": "space", "modifiers": COMMAND }
        });
        let shortcut: QuickTerminalShortcut = serde_json::from_value(value).unwrap();
        assert_eq!(
            shortcut.registration_identity().unwrap().virtual_key_code,
            49
        );
    }

    #[test]
    fn quick_terminal_physical_identity_replaces_a_mismatched_display_key() {
        let shortcut = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("x", COMMAND),
            virtual_key_code: 49,
        };
        let canonical = shortcut.canonicalized(resolver).unwrap();
        assert_eq!(canonical.key_combo().unwrap().key, "space");
        assert_eq!(
            canonical.registration_identity(),
            shortcut.registration_identity()
        );
    }

    #[test]
    fn quick_terminal_rejects_noncanonical_modifiers_and_unsupported_keys() {
        let extra = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("a", COMMAND | (1 << 30)),
            virtual_key_code: 0,
        };
        assert!(extra.canonicalized(resolver).is_none());
        let unsupported = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("escape", COMMAND),
            virtual_key_code: 53,
        };
        assert!(
            unsupported
                .canonicalized(|_| Some("escape".to_owned()))
                .is_none()
        );
        let shift_only = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("a", SHIFT),
            virtual_key_code: 0,
        };
        assert!(shift_only.canonicalized(resolver).is_none());
    }

    #[test]
    fn quick_terminal_accepts_named_keys_and_preserves_registration_on_layout_refresh() {
        for (key, code) in [("return", 36), ("leftarrow", 123)] {
            let shortcut = QuickTerminalShortcut::KeyCombo {
                key_combo: KeyCombo::new(key, COMMAND),
                virtual_key_code: code,
            };
            assert!(shortcut.canonicalized(resolver).is_some());
        }
        let unicode = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("é", COMMAND),
            virtual_key_code: 0,
        };
        assert!(unicode.canonicalized(|_| Some("é".to_owned())).is_some());
        let shortcut = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("a", COMMAND),
            virtual_key_code: 0,
        };
        let refreshed = shortcut
            .canonicalized(|code| (code == 0).then(|| "q".to_owned()))
            .unwrap();
        assert_eq!(refreshed.key_combo().unwrap().key, "q");
        assert_eq!(
            refreshed.registration_identity(),
            shortcut.registration_identity()
        );
    }

    #[test]
    fn quick_terminal_invalid_file_falls_back_without_rewrite() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quick-terminal-shortcut.json");
        let invalid = br#"{"type":"unknown","held":true}"#;
        std::fs::write(&path, invalid).unwrap();
        assert_eq!(
            load_from(&path, resolver),
            QuickTerminalShortcut::Unassigned
        );
        assert_eq!(std::fs::read(&path).unwrap(), invalid);
        assert_eq!(
            load_from(&directory.path().join("missing.json"), resolver),
            QuickTerminalShortcut::Unassigned
        );
    }

    #[test]
    fn quick_terminal_save_is_atomic_private_and_foundation_compatible() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quick-terminal-shortcut.json");
        let shortcut = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("space", COMMAND),
            virtual_key_code: 49,
        };
        save_to(&path, &shortcut).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(stored["virtualKeyCode"], 49);
    }

    #[test]
    fn quick_terminal_conflicts_use_the_canonical_logical_combo() {
        let shortcut = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("x", COMMAND),
            virtual_key_code: 49,
        };
        let candidates = [ConflictCandidate {
            label: "Open Project".to_owned(),
            combo: KeyCombo::new("space", COMMAND),
        }];
        assert_eq!(
            shortcut.find_conflict(&candidates, resolver).unwrap().label,
            "Open Project"
        );
    }
}
