//! Language packs: extensions that declare `muxy.localizations`, each a
//! resource-only `.bundle` with `<language>.lproj/Localizable.strings` and an
//! optional `Localizable.stringsdict`, in main's format.
//!
//! The selection is `<extension>:<localization>`, empty for built-in English.
//! An unavailable selection is kept and English shows until it returns.

mod catalog;
mod plist;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use muxy_core::l10n::{Language, PluralCategory};

use crate::extensions::{Extension, Registry};
use plist::Value;

const MAX_CATALOG_BYTES: u64 = 4 * 1024 * 1024;
const CATALOGS: [&str; 2] = ["Localizable.strings", "Localizable.stringsdict"];

/// A language an enabled extension provides.
#[derive(Clone, Debug, PartialEq)]
pub struct Provider {
    pub extension: String,
    pub id: String,
    pub language: String,
    pub title: String,
    pub bundle: PathBuf,
}

impl Provider {
    /// The settings value that selects this provider.
    pub fn selection(&self) -> String {
        format!("{}:{}", self.extension, self.id)
    }
}

/// Splits a selection into extension and localization ids.
pub fn parse_selection(value: &str) -> Option<(&str, &str)> {
    value
        .split_once(':')
        .filter(|(extension, id)| !extension.is_empty() && !id.is_empty())
}

/// Every language the enabled extensions provide, by extension then
/// declaration order.
pub fn providers(registry: &Registry) -> Vec<Provider> {
    registry.active().flat_map(providers_of).collect()
}

fn providers_of(extension: &Extension) -> Vec<Provider> {
    extension
        .manifest
        .localizations
        .iter()
        .filter_map(|localization| {
            Some(Provider {
                extension: extension.name.clone(),
                id: localization.id.clone(),
                language: localization.language.clone(),
                title: localization.title.clone(),
                bundle: extension.located(&localization.bundle).ok()?,
            })
        })
        .collect()
}

/// The provider a selection names, when its extension is enabled.
pub fn provider(registry: &Registry, selection: &str) -> Option<Provider> {
    let (extension, id) = parse_selection(selection)?;
    providers_of(registry.enabled(extension)?)
        .into_iter()
        .find(|provider| provider.id == id)
}

/// Reads and validates a provider's catalogs.
pub fn load(provider: &Provider) -> Result<Language, String> {
    let mut strings = HashMap::new();
    let mut plurals = HashMap::new();
    for found in catalogs(&provider.bundle, &provider.language)? {
        catalog::split(found, &mut strings, &mut plurals);
    }
    Ok(Language::new(strings, plurals, rules(&provider.language)))
}

/// Checks a bundle as main does before offering it: a resource-only
/// `Info.plist`, and catalogs under 4 MiB whose translations keep their
/// keys' placeholders.
pub(crate) fn validate_bundle(bundle: &Path, language: &str) -> Result<(), String> {
    catalogs(bundle, language).map(|_| ())
}

fn catalogs(bundle: &Path, language: &str) -> Result<Vec<BTreeMap<String, Value>>, String> {
    let root = bundle
        .canonicalize()
        .map_err(|_| "bundle was not found".to_owned())?;
    let info = read(&root, Path::new("Info.plist"))
        .and_then(|bytes| plist::dictionary(&bytes))
        .map_err(|error| format!("bundle Info.plist is invalid: {error}"))?;
    if info.contains_key("CFBundleExecutable") {
        return Err("bundle must not contain executable code".into());
    }
    let directory = PathBuf::from(format!("{language}.lproj"));
    let mut found = Vec::new();
    for name in CATALOGS {
        let relative = directory.join(name);
        if !root.join(&relative).exists() {
            continue;
        }
        let catalog = read(&root, &relative)
            .and_then(|bytes| plist::dictionary(&bytes))
            .map_err(|error| format!("catalog {name} is invalid: {error}"))?;
        if let Some(key) = catalog::incompatible_key(&catalog) {
            return Err(format!(
                "catalog {name} changes the format placeholders of '{key}'"
            ));
        }
        found.push(catalog);
    }
    if found.is_empty() {
        return Err(format!(
            "bundle has no Localizable.strings or Localizable.stringsdict for '{language}'"
        ));
    }
    Ok(found)
}

/// A regular file inside `root` of at most 4 MiB.
fn read(root: &Path, relative: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|_| format!("'{}' was not found", relative.display()))?;
    let too_large = || {
        format!(
            "'{}' must be a file of at most 4 MiB inside its bundle",
            relative.display()
        )
    };
    if !path.starts_with(root) || !path.is_file() {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|file| file.take(MAX_CATALOG_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_CATALOG_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

/// CLDR cardinal plural rules for a BCP 47 language; English-like `other`
/// when the language is unknown.
fn rules(language: &str) -> impl Fn(u64) -> PluralCategory + Send + Sync + 'static {
    use icu_plurals::{PluralCategory as Cldr, PluralRules};
    let rules = language
        .parse::<icu_locale_core::Locale>()
        .ok()
        .and_then(|locale| PluralRules::try_new_cardinal((&locale).into()).ok());
    move |count| match rules.as_ref().map(|rules| rules.category_for(count)) {
        Some(Cldr::Zero) => PluralCategory::Zero,
        Some(Cldr::One) => PluralCategory::One,
        Some(Cldr::Two) => PluralCategory::Two,
        Some(Cldr::Few) => PluralCategory::Few,
        Some(Cldr::Many) => PluralCategory::Many,
        Some(Cldr::Other) | None => PluralCategory::Other,
    }
}

#[cfg(test)]
mod tests;
