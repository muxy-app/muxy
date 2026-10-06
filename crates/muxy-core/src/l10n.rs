//! The app language. English text is the key, as in main's
//! `Localizable.strings`; the active language may translate it. Language
//! packs are loaded by `muxy_app_core::localization`.

pub mod format;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

pub use format::Arg;
use format::Token;

/// Translates an English literal through the active language.
///
/// `tr!("Settings")` and `tr!("%lld changes", count)` both return a
/// `Cow<'static, str>`. Placeholders follow main's catalogs: `%@` for text,
/// `%lld` for integers, `%1$@` to reorder, and `%%` for a percent sign.
#[macro_export]
macro_rules! tr {
    ($key:literal $(,)?) => {
        $crate::l10n::translate($key)
    };
    ($key:literal, $($arg:expr),+ $(,)?) => {
        ::std::borrow::Cow::<'static, str>::Owned($crate::l10n::format(
            $key,
            &[$($crate::l10n::Arg::from($arg)),+],
        ))
    };
}

/// Marks an English literal kept for later `translate` calls, such as a label
/// in a static table, so it is extracted into the English catalog.
#[macro_export]
macro_rules! tr_key {
    ($key:literal $(,)?) => {
        $key
    };
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

/// One `Localizable.stringsdict` entry: a format whose `%#@name@`
/// references pick a form by the plural category of their argument.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plural {
    pub format: String,
    pub variables: HashMap<String, HashMap<PluralCategory, String>>,
}

type Rules = dyn Fn(u64) -> PluralCategory + Send + Sync;

/// A loaded translation: string and plural catalogs plus the language's
/// plural rules.
pub struct Language {
    strings: HashMap<String, String>,
    plurals: HashMap<String, Plural>,
    rules: Box<Rules>,
}

impl std::fmt::Debug for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Language")
            .field("strings", &self.strings.len())
            .field("plurals", &self.plurals.len())
            .finish_non_exhaustive()
    }
}

const MAX_PLURAL_DEPTH: usize = 8;

impl Language {
    pub fn new(
        strings: HashMap<String, String>,
        plurals: HashMap<String, Plural>,
        rules: impl Fn(u64) -> PluralCategory + Send + Sync + 'static,
    ) -> Self {
        Self {
            strings,
            plurals,
            rules: Box::new(rules),
        }
    }

    /// The translation of `key`, or `key` when this language lacks it.
    pub fn translate<'a>(&'a self, key: &'a str) -> &'a str {
        self.strings.get(key).map_or(key, String::as_str)
    }

    /// Formats the translation of `key`. A plural entry wins over a plain
    /// string, as in Foundation.
    pub fn format(&self, key: &str, args: &[Arg<'_>]) -> String {
        let mut out = String::new();
        let rendered = match self.plurals.get(key) {
            Some(plural) => self.write(&mut out, &plural.format, 1, args, Some(plural), 0),
            None => self.write(&mut out, self.translate(key), 1, args, None, 0),
        };
        if rendered.is_none() {
            out.clear();
            write_english(&mut out, key, args);
        }
        out
    }

    fn write(
        &self,
        out: &mut String,
        format: &str,
        first: usize,
        args: &[Arg<'_>],
        plural: Option<&Plural>,
        depth: usize,
    ) -> Option<()> {
        let mut next = first;
        for token in format::tokens(format)? {
            match token {
                Token::Text(text) => out.push_str(text),
                Token::Percent => out.push('%'),
                Token::Argument(spec) => {
                    let position = spec.position.unwrap_or(next);
                    next = next.max(position + 1);
                    let arg = args.get(position - 1);
                    debug_assert!(arg.is_some(), "'{format}' reads a missing argument");
                    format::write_argument(out, &spec, arg);
                }
                Token::Variable(name) => {
                    let forms = plural?.variables.get(name)?;
                    if depth >= MAX_PLURAL_DEPTH {
                        return None;
                    }
                    let position = next;
                    next += 1;
                    let category = self.category(args.get(position - 1), forms);
                    let form = forms
                        .get(&category)
                        .or_else(|| forms.get(&PluralCategory::Other))?;
                    self.write(out, form, position, args, plural, depth + 1)?;
                }
            }
        }
        Some(())
    }

    /// Foundation uses a `zero` form for 0 in every language that defines it.
    fn category(
        &self,
        arg: Option<&Arg<'_>>,
        forms: &HashMap<PluralCategory, String>,
    ) -> PluralCategory {
        let count = match arg {
            Some(Arg::Int(value)) => value.unsigned_abs(),
            Some(Arg::UInt(value)) => *value,
            _ => return PluralCategory::Other,
        };
        if count == 0 && forms.contains_key(&PluralCategory::Zero) {
            PluralCategory::Zero
        } else {
            (self.rules)(count)
        }
    }
}

fn write_english(out: &mut String, key: &str, args: &[Arg<'_>]) {
    let english = Language::new(HashMap::new(), HashMap::new(), |_| PluralCategory::Other);
    if english.write(out, key, 1, args, None, 0).is_none() {
        out.clear();
        out.push_str(key);
    }
}

#[cfg(not(any(test, feature = "test-support")))]
mod active {
    use std::sync::{Arc, RwLock};

    static ACTIVE: RwLock<Option<Arc<super::Language>>> = RwLock::new(None);

    pub(super) fn get() -> Option<Arc<super::Language>> {
        ACTIVE.read().ok().and_then(|active| active.clone())
    }

    pub(super) fn set(language: Option<Arc<super::Language>>) {
        if let Ok(mut active) = ACTIVE.write() {
            *active = language;
        }
    }
}

/// Tests run in parallel threads, so each sees only the language it sets.
#[cfg(any(test, feature = "test-support"))]
mod active {
    use std::cell::RefCell;
    use std::sync::Arc;

    thread_local! {
        static ACTIVE: RefCell<Option<Arc<super::Language>>> = const { RefCell::new(None) };
    }

    pub(super) fn get() -> Option<Arc<super::Language>> {
        ACTIVE.with(|active| active.borrow().clone())
    }

    pub(super) fn set(language: Option<Arc<super::Language>>) {
        ACTIVE.with(|active| *active.borrow_mut() = language);
    }
}

/// Makes `language` the app language; `None` is English.
pub fn set_language(language: Option<Arc<Language>>) {
    active::set(language);
}

pub fn language() -> Option<Arc<Language>> {
    active::get()
}

/// The active translation of `key`. A key that is a format without
/// arguments, such as `"100%%"`, reads `%%` as a percent sign; any other
/// text, such as a user's `"50% done"`, is never read as a format.
pub fn translate(key: &str) -> Cow<'_, str> {
    if key.contains('%') && takes_no_arguments(key) {
        return Cow::Owned(format(key, &[]));
    }
    match active::get() {
        Some(language) => match language.strings.get(key) {
            Some(text) => Cow::Owned(text.clone()),
            None => Cow::Borrowed(key),
        },
        None => Cow::Borrowed(key),
    }
}

fn takes_no_arguments(key: &str) -> bool {
    format::tokens(key).is_some_and(|tokens| {
        tokens
            .iter()
            .all(|token| matches!(token, Token::Text(_) | Token::Percent))
    })
}

/// Formats the active translation of `key` with `args`.
pub fn format(key: &str, args: &[Arg<'_>]) -> String {
    if let Some(language) = active::get() {
        return language.format(key, args);
    }
    let mut out = String::new();
    write_english(&mut out, key, args);
    out
}

/// Lowercased text to search for `key`: its English source and, when
/// different, its translation, so both find it.
pub fn searchable(key: &str) -> String {
    let translated = translate(key);
    if translated == key {
        key.to_lowercase()
    } else {
        format!("{key} {translated}").to_lowercase()
    }
}

#[cfg(test)]
mod tests;
