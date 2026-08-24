use anyhow::anyhow;
use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "icons/*.svg"]
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
