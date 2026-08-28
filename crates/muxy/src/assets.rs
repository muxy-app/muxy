use anyhow::anyhow;
use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "icons/*.svg"]
#[include = "icons/providers/*.svg"]
#[include = "themes/*"]
pub struct Assets;

impl Assets {
    pub fn theme(name: &str) -> Option<String> {
        let file = Self::get(&format!("themes/{name}"))?;
        String::from_utf8(file.data.into_owned()).ok()
    }

    pub fn theme_names() -> Vec<String> {
        Self::iter()
            .filter_map(|entry| entry.strip_prefix("themes/").map(str::to_owned))
            .filter(|name| !name.is_empty())
            .collect()
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Self::get(path)
            .map(|file| Some(file.data))
            .ok_or_else(|| anyhow!("asset not found: {path}"))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter(|entry| entry.starts_with(path))
            .map(|entry| SharedString::from(entry.to_string()))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::Assets;
    use muxy_core::repository_ai::PROVIDERS;

    #[test]
    fn every_provider_uses_an_exact_embedded_legacy_icon() {
        let legacy = [
            (
                "claude",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/claude.svg").as_slice(),
            ),
            (
                "opencode",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/opencode.svg").as_slice(),
            ),
            (
                "codex",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/codex.svg").as_slice(),
            ),
            (
                "cursor",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/cursor.svg").as_slice(),
            ),
            (
                "copilot",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/copilot.svg").as_slice(),
            ),
            (
                "factory",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/factory.svg").as_slice(),
            ),
            (
                "pi",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/pi.svg").as_slice(),
            ),
            (
                "grok",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/grok.svg").as_slice(),
            ),
            (
                "kiro",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/kiro.svg").as_slice(),
            ),
            (
                "xal",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/xal.svg").as_slice(),
            ),
            (
                "antigravity",
                include_bytes!("../../../Muxy/Resources/ProviderIcons/antigravity.svg").as_slice(),
            ),
        ];

        for provider in PROVIDERS {
            let expected = legacy
                .iter()
                .find_map(|(key, bytes)| (*key == provider.icon_key).then_some(*bytes))
                .unwrap();
            let embedded =
                Assets::get(&format!("icons/providers/{}.svg", provider.icon_key)).unwrap();
            assert_eq!(embedded.data.as_ref(), expected);
        }
    }
}
