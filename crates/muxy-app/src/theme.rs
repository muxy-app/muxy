use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use gpui::{Window, WindowAppearance};
use muxy_app_core::settings::Appearance;
use muxy_ui::theme::{ColorScheme, Theme};

use crate::views::terminal::colors::Palette;

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) scheme: ColorScheme,
}

#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) entries: Vec<Entry>,
    pub(crate) errors: Vec<String>,
}

impl Catalog {
    pub(crate) fn load(directory: &Path) -> Self {
        let mut entries: BTreeMap<_, _> = muxy_ui::assets::Assets::themes()
            .map(|(name, source)| (name.to_owned(), ColorScheme::parse(source)))
            .collect();
        let mut errors = Vec::new();
        if let Err(error) = Self::read_directory(directory, &mut entries, &mut errors) {
            errors.push(format!(
                "Could not load themes from {}: {error}",
                directory.display()
            ));
        }
        let mut entries: Vec<_> = entries
            .into_iter()
            .map(|(name, scheme)| Entry { name, scheme })
            .collect();
        entries.sort_by_key(|entry| {
            (
                !matches!(entry.name.as_str(), "Muxy" | "Muxy Light"),
                entry.name.to_lowercase(),
                entry.name.clone(),
            )
        });
        Self { entries, errors }
    }

    fn read_directory(
        directory: &Path,
        entries: &mut BTreeMap<String, ColorScheme>,
        errors: &mut Vec<String>,
    ) -> std::io::Result<()> {
        fs::create_dir_all(directory)?;
        let mut paths = fs::read_dir(directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.sort();
        for path in paths {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with('.') || !path.is_file() {
                continue;
            }
            match fs::read_to_string(&path) {
                Ok(source) => {
                    let scheme = ColorScheme::parse(&source);
                    if scheme.background.is_none() || scheme.foreground.is_none() {
                        errors.push(format!(
                            "Theme {name} needs valid background and foreground colors"
                        ));
                        continue;
                    }
                    let name = name
                        .strip_suffix(".conf")
                        .or_else(|| name.strip_suffix(".theme"))
                        .unwrap_or(name);
                    entries.insert(name.to_owned(), scheme);
                }
                Err(error) => errors.push(format!("Could not read theme {name}: {error}")),
            }
        }
        Ok(())
    }

    fn selected(&self, appearance: &Appearance, dark: bool) -> Option<&Entry> {
        let name = if dark {
            &appearance.dark_theme
        } else {
            &appearance.light_theme
        };
        let fallback = if dark { "Muxy" } else { "Muxy Light" };
        self.entries
            .iter()
            .find(|entry| &entry.name == name)
            .or_else(|| self.entries.iter().find(|entry| entry.name == fallback))
    }

    pub(crate) fn terminal_palette(
        &self,
        fallback: &Palette,
        options: &muxy_app_core::settings::TerminalOptions,
        dark: bool,
        directory: &Path,
    ) -> Result<Palette, String> {
        let Some(value) = &options.theme else {
            return Ok(fallback.with_options(options));
        };
        let name = terminal_theme_name(value, dark)?;
        let palette = if let Some(entry) = self.entries.iter().find(|entry| entry.name == name) {
            Palette::from_scheme(&entry.scheme, dark)
        } else {
            let path = if let Some(rest) = name.strip_prefix("~/") {
                std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .ok_or("Cannot resolve theme without a home directory")?
                    .join(rest)
            } else {
                directory.join(name)
            };
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("Could not load terminal theme {name:?}: {error}"))?;
            let scheme = ColorScheme::parse(&source);
            if scheme.background.is_none() || scheme.foreground.is_none() {
                return Err(format!(
                    "Terminal theme {name:?} needs valid background and foreground colors"
                ));
            }
            Palette::from_scheme(&scheme, dark)
        };
        Ok(palette.with_options(options))
    }

    pub(crate) fn active_name(&self, appearance: &Appearance, dark: bool) -> String {
        self.selected(appearance, dark)
            .map_or_else(String::new, |entry| entry.name.clone())
    }

    pub(crate) fn resolve(&self, appearance: &Appearance, dark: bool) -> (Theme, Palette) {
        let scheme = self
            .selected(appearance, dark)
            .map_or_else(ColorScheme::default, |entry| entry.scheme.clone());
        (
            Theme::from_scheme(&scheme),
            Palette::from_scheme(&scheme, dark),
        )
    }
}

fn terminal_theme_name(value: &str, dark: bool) -> Result<&str, String> {
    let value = value.trim();
    let name = if value.starts_with("dark:") || value.starts_with("light:") {
        let prefix = if dark { "dark:" } else { "light:" };
        value
            .split(',')
            .map(str::trim)
            .find_map(|part| part.strip_prefix(prefix))
            .ok_or_else(|| {
                format!(
                    "theme needs a {} entry",
                    if dark { "dark" } else { "light" }
                )
            })?
    } else {
        value
    }
    .trim();
    let name = name
        .strip_prefix('"')
        .and_then(|name| name.strip_suffix('"'))
        .unwrap_or(name);
    if name.is_empty() || name.contains('"') {
        return Err("Invalid terminal theme name".into());
    }
    Ok(name)
}

pub(crate) fn is_dark(window: &Window) -> bool {
    matches!(
        window.appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_theme_selection_and_color_overrides_follow_appearance() {
        let catalog = Catalog {
            entries: vec![
                Entry {
                    name: "Dark".into(),
                    scheme: ColorScheme::parse("background = 123456\nforeground = abcdef"),
                },
                Entry {
                    name: "Light".into(),
                    scheme: ColorScheme::parse("background = fedcba\nforeground = 654321"),
                },
            ],
            errors: Vec::new(),
        };
        let mut options = muxy_app_core::settings::TerminalOptions {
            theme: Some("dark:\"Dark\",light:\"Light\"".into()),
            ..Default::default()
        };
        for (dark, background) in [(true, 0x12_34_56), (false, 0xfe_dc_ba)] {
            assert_eq!(
                catalog
                    .terminal_palette(&Palette::new(dark), &options, dark, Path::new("."))
                    .expect("palette")
                    .background,
                background
            );
        }
        options.background = Some(0x11_22_33);
        options.palette.insert(196, 0x23_45_67);
        let palette = catalog
            .terminal_palette(&Palette::new(true), &options, true, Path::new("."))
            .expect("palette");
        assert_eq!(palette.background, 0x11_22_33);
        assert_eq!(palette.indexed(196), 0x23_45_67);
    }

    #[test]
    fn themes_are_discovered_reloaded_and_override_bundled_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = std::env::temp_dir().join(format!("muxy-themes-{}", std::process::id()));
        fs::create_dir_all(&directory)?;
        let original = Catalog::load(&directory);
        assert!(original.errors.is_empty());
        assert_eq!(original.entries.len(), 490);
        assert!(original.entries.iter().all(|entry| entry.scheme.background.is_some() && entry.scheme.foreground.is_some()));
        fs::write(
            directory.join("Custom.conf"),
            "background = 123456\nforeground = abcdef\npalette = 4=112233\n",
        )?;
        fs::write(
            directory.join("Muxy"),
            "background = 234567\nforeground = abcdef\n",
        )?;
        fs::write(directory.join("Broken"), "background = invalid\n")?;
        fs::create_dir_all(directory.join("Subdirectory"))?;
        let catalog = Catalog::load(&directory);
        assert_eq!(catalog.errors.len(), 1);
        assert_eq!(catalog.entries.len(), original.entries.len() + 1);
        let appearance = Appearance {
            dark_theme: "Custom".into(),
            ..Appearance::default()
        };
        assert_eq!(catalog.resolve(&appearance, true).1.background, 0x12_34_56);
        assert_eq!(
            catalog.resolve(&Appearance::default(), true).1.background,
            0x23_45_67
        );
        fs::write(
            directory.join("Custom.conf"),
            "background = 345678\nforeground = abcdef\n",
        )?;
        assert_eq!(
            Catalog::load(&directory)
                .resolve(&appearance, true)
                .1
                .background,
            0x34_56_78
        );
        fs::remove_file(directory.join("Custom.conf"))?;
        assert_eq!(
            Catalog::load(&directory)
                .resolve(&appearance, true)
                .1
                .background,
            0x23_45_67
        );
        fs::remove_dir_all(directory)?;
        Ok(())
    }
}
