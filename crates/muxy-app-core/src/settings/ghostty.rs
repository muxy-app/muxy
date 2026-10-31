use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use crate::settings::{Error, Result};

mod bindings;
mod fonts;
mod native;
mod options;
pub use bindings::{TerminalAction, TerminalBindings};
pub use fonts::{FontMap, FontOptions};
pub use native::TerminalEdit;
pub use options::{PaddingColor, TerminalColor, TerminalOptions};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TerminalSettings {
    pub font_families: Vec<String>,
    pub font_size: f32,
    pub cell_height: CellHeight,
    pub font: FontOptions,
    pub macos_option_as_alt: OptionAsAlt,
    pub options: TerminalOptions,
    pub keybindings: TerminalBindings,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionAsAlt {
    #[default]
    True,
    False,
    Left,
    Right,
}

impl OptionAsAlt {
    pub fn enabled(self, left: bool, right: bool) -> bool {
        match self {
            Self::True => true,
            Self::False => false,
            Self::Left => left,
            Self::Right => right,
        }
    }
}

impl std::str::FromStr for OptionAsAlt {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "" | "true" => Ok(Self::True),
            "false" => Ok(Self::False),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            _ => Err(Error::new(
                "macos-option-as-alt",
                "expected true, false, left, or right",
            )),
        }
    }
}

impl std::fmt::Display for OptionAsAlt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::True => "true",
            Self::False => "false",
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum CellHeight {
    #[default]
    Natural,
    Pixels(i16),
    Percent(f32),
}

impl CellHeight {
    pub fn apply(self, natural: f32, scale: f32) -> f32 {
        let physical = natural * scale;
        let adjusted = match self {
            Self::Natural => physical,
            Self::Pixels(amount) => physical.round() + f32::from(amount),
            Self::Percent(amount) => physical * (1.0 + amount / 100.0),
        };
        adjusted.round().max(1.0) / scale
    }
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font_families: vec![
                if cfg!(target_os = "macos") {
                    "Menlo"
                } else {
                    "monospace"
                }
                .into(),
            ],
            font_size: 13.0,
            cell_height: CellHeight::Natural,
            font: FontOptions::default(),
            macos_option_as_alt: OptionAsAlt::default(),
            options: TerminalOptions::default(),
            keybindings: TerminalBindings::default(),
            diagnostics: Vec::new(),
        }
    }
}

impl TerminalSettings {
    pub fn backup_source(path: &Path) -> Result<String> {
        let mut pending = VecDeque::from([(path.to_owned(), false, 0)]);
        let mut loaded = HashSet::new();
        let mut settings = Self::default();
        let mut families = Vec::new();
        let mut output = String::new();
        while let Some((path, optional, depth)) = pending.pop_front() {
            if optional
                && !path
                    .try_exists()
                    .map_err(|error| Error::new("backup", error))?
            {
                continue;
            }
            let canonical = fs::canonicalize(&path).map_err(|error| Error::new("backup", error))?;
            if !loaded.insert(canonical) || depth >= 32 {
                return Err(Error::new(
                    "backup",
                    "config-file cycle or nesting exceeds 32 files",
                ));
            }
            if fs::metadata(&path)
                .map_err(|error| Error::new("backup", error))?
                .len()
                > 8 * 1024 * 1024
            {
                return Err(Error::new("backup", "Ghostty configuration exceeds 8 MiB"));
            }
            let source = fs::read_to_string(&path).map_err(|error| Error::new("backup", error))?;
            for line in source.lines().filter(|line| {
                line.split('=')
                    .next()
                    .is_none_or(|key| key.trim() != "config-file")
            }) {
                output.push_str(line);
                output.push('\n');
            }
            if output.len() > 8 * 1024 * 1024 {
                return Err(Error::new(
                    "backup",
                    "Combined Ghostty configuration exceeds 8 MiB",
                ));
            }
            pending.extend(
                settings
                    .read(&path, &mut families)?
                    .into_iter()
                    .map(|(path, optional)| (path, optional, depth + 1)),
            );
        }
        Ok(output)
    }

    pub fn import_supported_source(source: &str) -> (String, Vec<String>) {
        let mut output = String::new();
        let mut skipped = Vec::new();
        for line in source.lines() {
            let key = line.split('=').next().unwrap_or_default().trim();
            if key.is_empty() || key.starts_with('#') {
                continue;
            }
            let supported = matches!(
                key,
                "font-family"
                    | "font-size"
                    | "adjust-cell-height"
                    | "font-family-bold"
                    | "font-family-italic"
                    | "font-family-bold-italic"
                    | "font-feature"
                    | "font-codepoint-map"
                    | "font-thicken"
                    | "font-thicken-strength"
                    | "adjust-cell-width"
                    | "macos-option-as-alt"
            ) || options::KEYS.contains(&key);
            let binding = key == "keybind"
                && line.split_once('=').is_some_and(|(_, value)| {
                    TerminalBindings::default()
                        .read(value.trim())
                        .is_ok_and(|warning| warning.is_none())
                });
            if supported || binding {
                output.push_str(line);
                output.push('\n');
            } else {
                skipped.push(format!("ghostty.conf: {key}"));
            }
        }
        (output, skipped)
    }

    fn resolve(path: &Path) -> Result<Self> {
        let mut settings = Self::default();
        let mut families = Vec::new();
        let mut pending = VecDeque::from([(path.to_owned(), false, 0)]);
        let mut loaded = HashSet::new();
        while let Some((path, optional, depth)) = pending.pop_front() {
            if optional && path.try_exists().is_ok_and(|exists| !exists) {
                continue;
            }
            let canonical = fs::canonicalize(&path)
                .map_err(|error| Error::new(path.display().to_string(), error))?;
            if !loaded.insert(canonical) || depth >= 32 {
                return Err(Error::new(
                    path.display().to_string(),
                    "config-file cycle or nesting exceeds 32 files",
                ));
            }
            pending.extend(
                settings
                    .read(&path, &mut families)?
                    .into_iter()
                    .map(|(path, optional)| (path, optional, depth + 1)),
            );
        }
        if !families.is_empty() {
            settings.font_families = families;
        }
        Ok(settings)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Keep config dispatch and source diagnostics together"
    )]
    fn read(&mut self, path: &Path, families: &mut Vec<String>) -> Result<Vec<(PathBuf, bool)>> {
        let source = fs::read_to_string(path)
            .map_err(|error| Error::new(path.display().to_string(), error))?;
        self.read_source(&source, path, families)
    }

    #[allow(clippy::too_many_lines)]
    fn read_source(
        &mut self,
        source: &str,
        path: &Path,
        families: &mut Vec<String>,
    ) -> Result<Vec<(PathBuf, bool)>> {
        let mut includes = Vec::new();
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        for (index, line) in source.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let context = format!("{}:{}", path.display(), index + 1);
            let note = format!("{file}:{}", index + 1);
            let (key, value) = line
                .split_once('=')
                .map_or((line, None), |(key, value)| (key, Some(value)));
            let key = key.trim();
            if !matches!(
                key,
                "font-family"
                    | "font-size"
                    | "adjust-cell-height"
                    | "config-file"
                    | "font-family-bold"
                    | "font-family-italic"
                    | "font-family-bold-italic"
                    | "font-feature"
                    | "font-codepoint-map"
                    | "font-thicken"
                    | "font-thicken-strength"
                    | "adjust-cell-width"
                    | "macos-option-as-alt"
            ) && !options::KEYS.contains(&key)
                && key != "keybind"
            {
                self.diagnostics
                    .push(format!("{note}: {key} is not supported by Muxy"));
                continue;
            }
            let context = format!("{context} {key}");
            let value = value
                .or_else(|| {
                    (matches!(key, "font-thicken" | "macos-option-as-alt")
                        || TerminalOptions::boolean(key))
                    .then_some("true")
                })
                .ok_or_else(|| Error::new(&context, "expected key = value"))?
                .trim();
            let optional = key == "config-file" && value.starts_with('?');
            let value = if optional { &value[1..] } else { value };
            let value = match key {
                "font-feature" => value,
                "font-family"
                | "font-family-bold"
                | "font-family-italic"
                | "font-family-bold-italic"
                | "font-codepoint-map" => config_value(value).unwrap_or(value),
                _ => config_value(value).map_err(|error| Error::new(&context, error))?,
            };
            match key {
                key if self
                    .font
                    .read(key, value)
                    .map_err(|error| Error::new(&context, error))? => {}
                key if self
                    .options
                    .read(key, value)
                    .map_err(|error| Error::new(&context, error))? => {}
                "keybind" => {
                    if let Some(warning) = self
                        .keybindings
                        .read(value)
                        .map_err(|error| Error::new(&context, error))?
                    {
                        self.diagnostics.push(format!("{note} {key}: {warning}"));
                    }
                }
                "font-family" if value.is_empty() => families.clear(),
                "font-family" => families.push(value.into()),
                "font-size" => {
                    let size = if value.is_empty() {
                        13.0
                    } else {
                        value
                            .parse::<f32>()
                            .map_err(|error| Error::new(&context, error))?
                    };
                    if !size.is_finite() || !(1.0..=256.0).contains(&size) {
                        return Err(Error::new(context, "must be between 1 and 256 points"));
                    }
                    self.font_size = size;
                }
                "adjust-cell-height" => {
                    self.cell_height =
                        parse_height(value).map_err(|error| Error::new(&context, error))?;
                }
                "macos-option-as-alt" => {
                    self.macos_option_as_alt =
                        value.parse().map_err(|error| Error::new(&context, error))?;
                }
                "config-file" if value.is_empty() => includes.clear(),
                "config-file" => {
                    let target = if let Some(relative) = value.strip_prefix("~/") {
                        PathBuf::from(
                            std::env::var_os("HOME")
                                .ok_or_else(|| Error::new(&context, "HOME is not set"))?,
                        )
                        .join(relative)
                    } else {
                        path.parent().unwrap_or_else(|| Path::new(".")).join(value)
                    };
                    includes.push((target, optional));
                }
                _ => {}
            }
        }
        Ok(includes)
    }

    pub fn zoom(&mut self, delta: f32) {
        self.font_size = (self.font_size + delta).clamp(1.0, 256.0);
    }
}

fn config_value(value: &str) -> Result<&str> {
    if let Some(quoted) = value.strip_prefix('"') {
        quoted
            .strip_suffix('"')
            .ok_or_else(|| Error::new("value", "unterminated quote"))
    } else {
        Ok(value)
    }
}

pub(crate) fn parse_height(value: &str) -> Result<CellHeight> {
    if value.is_empty() || value == "0" {
        return Ok(CellHeight::Natural);
    }
    if let Some(percentage) = value.strip_suffix('%') {
        let amount = percentage
            .parse::<f32>()
            .map_err(|error| Error::new("adjust-cell-height", error))?;
        if !amount.is_finite() || !(-99.0..=1000.0).contains(&amount) {
            return Err(Error::new(
                "adjust-cell-height",
                "percentage is outside the supported range",
            ));
        }
        return Ok(CellHeight::Percent(amount));
    }
    let amount = value
        .parse::<i16>()
        .map_err(|error| Error::new("adjust-cell-height", error))?;
    if !(-4096..=4096).contains(&amount) {
        return Err(Error::new(
            "adjust-cell-height",
            "adjustment is outside the supported range",
        ));
    }
    Ok(CellHeight::Pixels(amount))
}

impl std::fmt::Display for CellHeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Natural => f.write_str("0"),
            Self::Pixels(value) => write!(f, "{value}"),
            Self::Percent(value) => write!(f, "{value}%"),
        }
    }
}

impl std::str::FromStr for CellHeight {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        parse_height(value.trim())
    }
}
