use std::{process::Command, sync::OnceLock};

use gpui::SharedString;

pub const GENERIC_MONOSPACE: &str = "monospace";
const APPLE_MONOSPACE: &str = ".AppleSystemUIFontMonospaced";

pub fn family(name: &str) -> SharedString {
    if name == GENERIC_MONOSPACE {
        monospace()
    } else {
        name.to_owned().into()
    }
}

pub fn monospace() -> SharedString {
    static FAMILY: OnceLock<SharedString> = OnceLock::new();
    FAMILY
        .get_or_init(|| {
            if cfg!(target_os = "macos") {
                APPLE_MONOSPACE.into()
            } else {
                fontconfig_family(GENERIC_MONOSPACE)
                    .unwrap_or_else(|| GENERIC_MONOSPACE.to_owned())
                    .into()
            }
        })
        .clone()
}

fn fontconfig_family(pattern: &str) -> Option<String> {
    let output = Command::new("fc-match")
        .args(["--format", "%{family[0]}", pattern])
        .output()
        .ok()?;
    matched_family(&output.stdout)
}

fn matched_family(output: &[u8]) -> Option<String> {
    let family = std::str::from_utf8(output).ok()?.trim();
    (!family.is_empty()).then(|| family.to_owned())
}
