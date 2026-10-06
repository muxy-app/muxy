//! The English catalog translators start from: every `tr!` and `tr_key!`
//! literal in the app and every shortcut name, as `Localizable.strings`. Regenerate it with
//! `MUXY_UPDATE_LOCALIZATION=1 cargo test -p muxy-app --test localization`.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};

const CATALOG: &str = "localization/en.lproj/Localizable.strings";
const SOURCES: [&str; 4] = [
    "muxy-app/src",
    "muxy-ui/src",
    "muxy-core/src",
    "muxy-app-core/src",
];
const MACROS: [&str; 2] = ["tr!(", "tr_key!("];

#[test]
fn english_catalog_lists_every_translated_string() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut keys = BTreeSet::new();
    let mut problems = Vec::new();
    for source in SOURCES {
        for file in rust_files(&crates.join(source)) {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            extract(&text, &file, &mut keys, &mut problems);
        }
    }
    keys.extend(
        muxy_core::shortcuts::ALL
            .iter()
            .map(muxy_core::shortcuts::Shortcut::label),
    );
    for key in &keys {
        if key.contains('%') && muxy_core::l10n::format::tokens(key).is_none() {
            problems.push(format!(
                "'{key}' has a malformed placeholder; write %% for a percent sign"
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));

    let catalog = render(&keys);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(CATALOG);
    if std::env::var_os("MUXY_UPDATE_LOCALIZATION").is_some() {
        std::fs::create_dir_all(path.parent().unwrap_or(&path)).unwrap_or_default();
        std::fs::write(&path, &catalog).unwrap_or_default();
    }
    let saved = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        saved == catalog,
        "{CATALOG} is out of date. Run: MUXY_UPDATE_LOCALIZATION=1 cargo test -p muxy-app --test localization"
    );
}

fn rust_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if name != "tests" {
                    pending.push(path);
                }
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && name != "tests.rs"
                && !name.ends_with("_tests.rs")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Collects the literal of each macro call outside comments and macro definitions.
fn extract(text: &str, file: &Path, keys: &mut BTreeSet<String>, problems: &mut Vec<String>) {
    for (number, line) in text.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("//") || code.starts_with('$') || code.starts_with("macro_rules!") {
            continue;
        }
        for name in MACROS {
            let mut rest = line;
            while let Some(found) = rest.find(name) {
                let before = rest[..found].chars().next_back();
                let after = &rest[found + name.len()..];
                rest = after;
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let offset = line.len() - after.len();
                let start = text
                    .lines()
                    .take(number)
                    .map(|line| line.len() + 1)
                    .sum::<usize>()
                    + offset;
                if after.trim_start().starts_with('$') {
                    continue;
                }
                match literal(&text[start..]) {
                    Some(key) => {
                        keys.insert(key);
                    }
                    None => problems.push(format!(
                        "{}:{}: {name} needs a plain string literal",
                        file.display(),
                        number + 1
                    )),
                }
            }
        }
    }
}

/// The value of the Rust string literal at the start of `text`, after whitespace.
fn literal(text: &str) -> Option<String> {
    let mut chars = text.trim_start().strip_prefix('"')?.chars().peekable();
    let mut value = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(value),
            '\\' => match chars.next()? {
                'n' => value.push('\n'),
                't' => value.push('\t'),
                'r' => value.push('\r'),
                '0' => value.push('\0'),
                'x' => {
                    let code: String = chars.by_ref().take(2).collect();
                    value.push(char::from(u8::from_str_radix(&code, 16).ok()?));
                }
                'u' => {
                    let code: String = chars.by_ref().skip(1).take_while(|c| *c != '}').collect();
                    value.push(char::from_u32(u32::from_str_radix(&code, 16).ok()?)?);
                }
                '\n' => while chars.next_if(|c| c.is_whitespace()).is_some() {},
                other => value.push(other),
            },
            other => value.push(other),
        }
    }
    None
}

fn render(keys: &BTreeSet<String>) -> String {
    let mut catalog = String::from(
        "/* Muxy's English strings, the source for language packs. Translate the\n   right-hand side of each line and keep its placeholders. */\n\n",
    );
    for key in keys {
        let escaped = escape(key);
        let _ = writeln!(catalog, "\"{escaped}\" = \"{escaped}\";");
    }
    catalog
}

fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            other => escaped.push(other),
        }
    }
    escaped
}
