#[cfg(windows)]
use std::path::Path;
#[cfg(not(windows))]
use std::process::Command;
use std::sync::OnceLock;

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
                system_family().into()
            }
        })
        .clone()
}

#[cfg(not(windows))]
fn system_family() -> String {
    fontconfig_family(GENERIC_MONOSPACE).unwrap_or_else(|| GENERIC_MONOSPACE.to_owned())
}

#[cfg(windows)]
fn system_family() -> String {
    let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    windows_family(&Path::new(&windows).join("Fonts")).to_owned()
}

/// Cascadia Mono where Windows has it, as Windows 11 does, else Consolas,
/// which every Windows since Vista has.
#[cfg(windows)]
fn windows_family(fonts: &Path) -> &'static str {
    if fonts.join("CascadiaMono.ttf").is_file() {
        "Cascadia Mono"
    } else {
        "Consolas"
    }
}

#[cfg(not(windows))]
fn fontconfig_family(pattern: &str) -> Option<String> {
    let output = Command::new("fc-match")
        .args(["--format", "%{family[0]}", pattern])
        .output()
        .ok()?;
    matched_family(&output.stdout)
}

#[cfg(not(windows))]
fn matched_family(output: &[u8]) -> Option<String> {
    let family = std::str::from_utf8(output).ok()?.trim();
    (!family.is_empty()).then(|| family.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_generic_name_is_replaced_by_the_system_family() {
        assert_eq!(family("Noto Mono"), "Noto Mono");
        assert_eq!(family(GENERIC_MONOSPACE), monospace());
        assert!(!monospace().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn windows_prefers_cascadia_mono_and_falls_back_to_consolas() -> std::io::Result<()> {
        let fonts = std::env::temp_dir().join(format!("muxy-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&fonts)?;
        assert_eq!(windows_family(&fonts), "Consolas");
        std::fs::write(fonts.join("CascadiaMono.ttf"), b"")?;
        assert_eq!(windows_family(&fonts), "Cascadia Mono");
        std::fs::remove_dir_all(&fonts)
    }

    #[cfg(not(windows))]
    #[test]
    fn a_fontconfig_answer_without_a_family_is_ignored() {
        assert_eq!(
            matched_family(b"DejaVu Sans Mono\n").as_deref(),
            Some("DejaVu Sans Mono")
        );
        assert_eq!(matched_family(b" \n"), None);
        assert_eq!(matched_family(&[0xff]), None);
    }
}
