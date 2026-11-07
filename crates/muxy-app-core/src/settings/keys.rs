//! Every app setting by its dotted key, as `settings.toml` and `terminal.toml`
//! spell it: `appearance.dark_theme`, or `terminal.font_size` for the terminal
//! file. Keys are read from the files' own formats, so a new setting needs no
//! change here.

use std::path::PathBuf;

use super::{Error, Result, Settings, TerminalSettings};

/// The first key segment that names `terminal.toml` instead of `settings.toml`.
pub const TERMINAL: &str = "terminal";
/// Remote servers belong to Settings → Devices, which also connects them.
const READ_ONLY: &[&str] = &["servers"];

/// The settings files of one profile folder.
#[derive(Clone, Debug)]
pub struct Profile {
    directory: PathBuf,
}

/// Where a key lives: `settings.toml`, or `terminal.toml` without its prefix.
enum Scope {
    App(Vec<String>),
    Terminal(Vec<String>),
}

impl Profile {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    pub fn settings_path(&self) -> PathBuf {
        self.directory.join("settings.toml")
    }

    pub fn terminal_path(&self) -> PathBuf {
        self.directory.join("terminal.toml")
    }

    /// Every setting the app uses, defaults included, as one table with the
    /// terminal's under [`TERMINAL`].
    pub fn effective(&self) -> Result<toml::Table> {
        let settings = Settings::load(&self.settings_path())?;
        let terminal = TerminalSettings::load_native(&self.terminal_path())?;
        let mut table = to_table(&settings, "settings.toml")?;
        table.insert(
            TERMINAL.into(),
            toml::Value::Table(to_table(&terminal, "terminal.toml")?),
        );
        Ok(table)
    }

    /// Each setting's key and value, tables flattened to their leaves.
    pub fn list(&self) -> Result<Vec<(String, toml::Value)>> {
        let mut entries = Vec::new();
        flatten(&mut Vec::new(), &self.effective()?, &mut entries);
        Ok(entries)
    }

    /// The value at `key`, which may be a whole table.
    pub fn get(&self, key: &str) -> Result<toml::Value> {
        let segments = parse_key(key)?;
        let effective = toml::Value::Table(self.effective()?);
        lookup(&effective, &segments)
            .cloned()
            .ok_or_else(|| unknown(key))
    }

    /// Sets `key` to `raw`, read as a TOML value or else as text, and saves
    /// it only if the app would still start with it.
    pub fn set(&self, key: &str, raw: &str) -> Result<toml::Value> {
        let segments = parse_key(key)?;
        refuse_read_only(key, &segments)?;
        let current = self.get(key).ok();
        let value = parse_value(raw, current.as_ref());
        match scope(segments)? {
            Scope::App(segments) => {
                let terminal = TerminalSettings::load_native(&self.terminal_path())?;
                self.edit_app(key, &segments, &terminal, |document| {
                    insert(document, &segments, value.clone()).map_err(|error| error.to_string())
                })?;
            }
            Scope::Terminal(segments) => {
                let settings = Settings::load(&self.settings_path())?;
                self.edit_terminal(key, &segments, &settings, |table| {
                    insert(table, &segments, value.clone())
                })?;
            }
        }
        Ok(self.get(key).unwrap_or(value))
    }

    /// Returns `key` to its default by removing it from its file.
    pub fn unset(&self, key: &str) -> Result<()> {
        let segments = parse_key(key)?;
        refuse_read_only(key, &segments)?;
        let known = self.get(key).is_ok();
        match scope(segments)? {
            Scope::App(segments) => {
                let terminal = TerminalSettings::load_native(&self.terminal_path())?;
                self.edit_app(key, &segments, &terminal, |document| {
                    if remove(document, &segments) || known {
                        Ok(())
                    } else {
                        Err(UNKNOWN.to_owned())
                    }
                })
            }
            Scope::Terminal(segments) => {
                let settings = Settings::load(&self.settings_path())?;
                self.edit_terminal(key, &segments, &settings, |table| {
                    if remove(table, &segments) || known {
                        Ok(())
                    } else {
                        Err(unknown(key))
                    }
                })
            }
        }
    }

    fn edit_app<T>(
        &self,
        key: &str,
        segments: &[String],
        terminal: &TerminalSettings,
        change: impl FnOnce(&mut toml::Table) -> std::result::Result<T, String>,
    ) -> Result<T> {
        super::appearance::edit_document(&self.settings_path(), |document| {
            let changed = change(document)?;
            let settings: Settings = toml::Value::Table(document.clone()).try_into()?;
            settings.validate()?;
            settings.validate_command_shortcuts(terminal)?;
            check_kept(segments, &to_table(&settings, "settings.toml")?, document)?;
            Ok(changed)
        })
        .map_err(|error| Error::new(key, error))
    }

    fn edit_terminal<T>(
        &self,
        key: &str,
        segments: &[String],
        settings: &Settings,
        change: impl FnOnce(&mut toml::Table) -> Result<T>,
    ) -> Result<T> {
        let mut changed = None;
        TerminalSettings::edit_native(&self.terminal_path(), |terminal| {
            let mut table = to_table(&*terminal, "terminal.toml")?;
            changed = Some(change(&mut table)?);
            let next: TerminalSettings = toml::Value::Table(table.clone())
                .try_into()
                .map_err(|error| Error::new(key, error))?;
            next.native_source()?;
            settings.validate_command_shortcuts(&next)?;
            check_kept(segments, &to_table(&next, "terminal.toml")?, &table)
                .map_err(|error| Error::new(key, error))?;
            *terminal = next;
            Ok(())
        })?;
        changed.ok_or_else(|| Error::new(key, "was not changed"))
    }
}

/// Splits `key` at dots, keeping a dot inside double quotes:
/// `keymap."command.build"`.
pub fn parse_key(key: &str) -> Result<Vec<String>> {
    let mut segments = Vec::new();
    let mut segment = String::new();
    let mut quoted = false;
    let mut characters = key.trim().chars();
    while let Some(character) = characters.next() {
        match character {
            '\\' if quoted => {
                let escaped = characters
                    .next()
                    .ok_or_else(|| Error::new(key, "ends inside an escape"))?;
                segment.push(escaped);
            }
            '"' => quoted = !quoted,
            '.' if !quoted => segments.push(std::mem::take(&mut segment)),
            character => segment.push(character),
        }
    }
    if quoted {
        return Err(Error::new(key, "has an unclosed quote"));
    }
    segments.push(segment);
    if segments.iter().any(String::is_empty) {
        return Err(Error::new(key, "is not a setting key"));
    }
    Ok(segments)
}

/// Writes `segments` back as a key that [`parse_key`] reads.
pub fn render_key(segments: &[String]) -> String {
    segments
        .iter()
        .map(|segment| {
            if !segment.is_empty()
                && segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "_-".contains(character))
            {
                segment.clone()
            } else {
                format!("\"{}\"", segment.replace('\\', "\\\\").replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// How a value prints for a person: text without quotes, the rest as TOML.
pub fn display(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Table(table) => toml::to_string_pretty(table)
            .unwrap_or_default()
            .trim_end()
            .to_owned(),
        value => value.to_string(),
    }
}

fn scope(mut segments: Vec<String>) -> Result<Scope> {
    if segments.first().map(String::as_str) != Some(TERMINAL) {
        return Ok(Scope::App(segments));
    }
    segments.remove(0);
    if segments.is_empty() {
        return Err(Error::new(
            TERMINAL,
            "name a terminal setting, such as terminal.font_size",
        ));
    }
    Ok(Scope::Terminal(segments))
}

fn refuse_read_only(key: &str, segments: &[String]) -> Result<()> {
    if segments
        .first()
        .is_some_and(|first| READ_ONLY.contains(&first.as_str()))
    {
        return Err(Error::new(
            key,
            "is managed in Settings → Devices and can't be changed here",
        ));
    }
    Ok(())
}

const UNKNOWN: &str =
    "is not a setting, or has no value; run `muxy config list` to see every setting";

fn unknown(key: &str) -> Error {
    Error::new(key, UNKNOWN)
}

fn to_table(value: &impl serde::Serialize, file: &str) -> Result<toml::Table> {
    match toml::Value::try_from(value).map_err(|error| Error::new(file, error))? {
        toml::Value::Table(table) => Ok(table),
        _ => Err(Error::new(file, "is not a table")),
    }
}

/// Text that isn't a TOML value is taken as text, so `Solarized Dark` needs no
/// quotes. A whole number for a decimal setting is taken as decimal, and any
/// value for a text setting as its text.
fn parse_value(raw: &str, current: Option<&toml::Value>) -> toml::Value {
    let parsed = toml::from_str::<toml::Table>(&format!("value = {raw}"))
        .ok()
        .filter(|table| table.len() == 1)
        .and_then(|mut table| table.remove("value"));
    match (parsed, current) {
        (Some(toml::Value::Integer(number)), Some(toml::Value::Float(_))) =>
        {
            #[allow(clippy::cast_precision_loss, reason = "settings hold small numbers")]
            toml::Value::Float(number as f64)
        }
        (Some(toml::Value::String(text)), _) => toml::Value::String(text),
        (Some(_), Some(toml::Value::String(_))) | (None, _) => toml::Value::String(raw.to_owned()),
        (Some(value), _) => value,
    }
}

fn lookup<'a>(value: &'a toml::Value, segments: &[String]) -> Option<&'a toml::Value> {
    segments
        .iter()
        .try_fold(value, |value, segment| value.as_table()?.get(segment))
}

fn insert(table: &mut toml::Table, segments: &[String], value: toml::Value) -> Result<()> {
    let Some((last, parents)) = segments.split_last() else {
        return Err(Error::new("", "is not a setting key"));
    };
    let mut target = table;
    for (index, segment) in parents.iter().enumerate() {
        let entry = target
            .entry(segment.clone())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        target = entry.as_table_mut().ok_or_else(|| {
            Error::new(
                render_key(&segments[..=index]),
                "holds a value, not other settings",
            )
        })?;
    }
    target.insert(last.clone(), value);
    Ok(())
}

/// Removes the key, then any table it leaves empty. Whether it was there.
fn remove(table: &mut toml::Table, segments: &[String]) -> bool {
    let Some((first, rest)) = segments.split_first() else {
        return false;
    };
    if rest.is_empty() {
        return table.remove(first).is_some();
    }
    let Some(child) = table.get_mut(first).and_then(toml::Value::as_table_mut) else {
        return false;
    };
    let removed = remove(child, rest);
    if removed && child.is_empty() {
        table.remove(first);
    }
    removed
}

/// A key the format ignores, or one whose value it drops, disappears when the
/// settings are written back, so that is how an unknown key shows. An empty
/// list or table disappears too where the format leaves empty ones out.
fn check_kept(
    segments: &[String],
    saved: &toml::Table,
    edited: &toml::Table,
) -> std::result::Result<(), String> {
    let Some(value) = lookup_table(edited, segments) else {
        return Ok(());
    };
    let empty = match value {
        toml::Value::Array(items) => items.is_empty(),
        toml::Value::Table(table) => table.is_empty(),
        _ => false,
    };
    if !empty && lookup_table(saved, segments).is_none() {
        return Err(
            "is not a setting, or that value isn't allowed; run `muxy config list` to see every setting"
                .to_owned(),
        );
    }
    Ok(())
}

fn lookup_table<'a>(table: &'a toml::Table, segments: &[String]) -> Option<&'a toml::Value> {
    let (first, rest) = segments.split_first()?;
    lookup(table.get(first)?, rest)
}

fn flatten(
    prefix: &mut Vec<String>,
    table: &toml::Table,
    entries: &mut Vec<(String, toml::Value)>,
) {
    for (key, value) in table {
        prefix.push(key.clone());
        match value {
            toml::Value::Table(child) if !child.is_empty() => flatten(prefix, child, entries),
            value => entries.push((render_key(prefix), value.clone())),
        }
        prefix.pop();
    }
}
