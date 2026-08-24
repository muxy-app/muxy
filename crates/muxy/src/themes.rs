use crate::assets::Assets;
use muxy_ui::theme::{ColorScheme, Theme};

pub fn load(name: &str, fallback: &str) -> Theme {
    let source = Assets::theme(name).or_else(|| Assets::theme(fallback));
    let scheme = source
        .map(|source| ColorScheme::parse(&source))
        .unwrap_or_default();
    Theme::from_scheme(&scheme)
}

const PINNED: [&str; 2] = ["Muxy", "Muxy Light"];

pub struct ThemeEntry {
    pub name: String,
    pub scheme: ColorScheme,
}

pub fn catalog() -> Vec<ThemeEntry> {
    let mut entries: Vec<ThemeEntry> = Assets::theme_names()
        .into_iter()
        .filter_map(|name| {
            let source = Assets::theme(&name)?;
            Some(ThemeEntry {
                scheme: ColorScheme::parse(&source),
                name,
            })
        })
        .collect();
    entries.sort_by(|left, right| {
        let left_pinned = PINNED.contains(&left.name.as_str());
        let right_pinned = PINNED.contains(&right.name.as_str());
        match (left_pinned, right_pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => left.name.cmp(&right.name),
            (false, false) => left
                .name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.name.cmp(&right.name)),
        }
    });
    entries
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_catalog_pins_the_two_muxy_themes_and_sorts_the_rest_case_insensitively() {
        let catalog = super::catalog();
        assert!(catalog.len() > 2);
        assert_eq!(catalog[0].name, "Muxy");
        assert_eq!(catalog[1].name, "Muxy Light");

        let smallest = catalog[2..]
            .iter()
            .map(|entry| entry.name.to_lowercase())
            .min()
            .expect("a third theme");
        assert_eq!(catalog[2].name.to_lowercase(), smallest);
    }
}
