use super::{CellHeight, Error, Result, parse_height};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FontOptions {
    pub bold: Vec<String>,
    pub italic: Vec<String>,
    pub bold_italic: Vec<String>,
    pub features: Vec<(String, u32)>,
    pub codepoints: Vec<FontMap>,
    pub cell_width: CellHeight,
    pub thicken: bool,
    pub thicken_strength: u8,
}

impl Default for FontOptions {
    fn default() -> Self {
        Self {
            bold: Vec::new(),
            italic: Vec::new(),
            bold_italic: Vec::new(),
            features: Vec::new(),
            codepoints: Vec::new(),
            cell_width: CellHeight::Natural,
            thicken: false,
            thicken_strength: 255,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontMap {
    pub start: u32,
    pub end: u32,
    pub family: String,
}

pub(super) const KEYS: &[&str] = &[
    "font-family-bold",
    "font-family-italic",
    "font-family-bold-italic",
    "font-feature",
    "font-codepoint-map",
    "font-thicken",
    "font-thicken-strength",
    "adjust-cell-width",
];

impl FontOptions {
    pub(super) fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (key, values) in [
            ("font-family-bold", &self.bold),
            ("font-family-italic", &self.italic),
            ("font-family-bold-italic", &self.bold_italic),
        ] {
            lines.push(format!("{key} ="));
            lines.extend(values.iter().map(|value| format!("{key} = \"{value}\"")));
        }
        lines.push("font-feature =".into());
        lines.extend(
            self.features
                .iter()
                .map(|(key, value)| format!("font-feature = {key}={value}")),
        );
        lines.push("font-codepoint-map =".into());
        lines.extend(self.codepoints.iter().map(|map| {
            format!(
                "font-codepoint-map = U+{:X}-U+{:X}={}",
                map.start, map.end, map.family
            )
        }));
        lines.push(format!("font-thicken = {}", self.thicken));
        lines.push(format!("font-thicken-strength = {}", self.thicken_strength));
        lines.push(format!("adjust-cell-width = {}", self.cell_width));
        lines
    }

    pub(super) fn read(&mut self, key: &str, value: &str) -> Result<bool> {
        let families = match key {
            "font-family-bold" => Some(&mut self.bold),
            "font-family-italic" => Some(&mut self.italic),
            "font-family-bold-italic" => Some(&mut self.bold_italic),
            _ => None,
        };
        if let Some(families) = families {
            if value.is_empty() {
                families.clear();
            } else {
                families.push(value.into());
            }
            return Ok(true);
        }
        match key {
            "font-feature" => {
                if value.is_empty() {
                    self.features.clear();
                    return Ok(true);
                }
                for feature in value.split(',') {
                    let feature = feature.trim();
                    let (name, enabled) = if let Some(name) = feature.strip_prefix('-') {
                        (name, 0)
                    } else if let Some((name, number)) = feature.split_once('=') {
                        (
                            name.trim(),
                            number.trim().parse().map_err(|e| Error::new(key, e))?,
                        )
                    } else if let Some((name, number)) = feature.split_once(char::is_whitespace) {
                        (name, number.trim().parse().map_err(|e| Error::new(key, e))?)
                    } else {
                        (feature.trim_start_matches('+'), 1)
                    };
                    let name = name.trim_matches(['\'', '"']);
                    if name.len() != 4 || !name.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                        return Err(Error::new(key, "expected a four-character OpenType tag"));
                    }
                    self.features.retain(|(old, _)| old != name);
                    self.features.push((name.into(), enabled));
                }
            }
            "font-codepoint-map" => {
                if value.is_empty() {
                    self.codepoints.clear();
                    return Ok(true);
                }
                let (ranges, family) = value
                    .split_once('=')
                    .ok_or_else(|| Error::new(key, "expected U+start-U+end=family"))?;
                let family = family.trim();
                if family.is_empty() {
                    return Err(Error::new(key, "font family is empty"));
                }
                for range in ranges.split(',') {
                    let (start, end) = range
                        .trim()
                        .split_once('-')
                        .unwrap_or((range.trim(), range.trim()));
                    let code = |value: &str| {
                        u32::from_str_radix(value.trim().trim_start_matches("U+"), 16)
                            .map_err(|e| Error::new(key, e))
                    };
                    let start = code(start)?;
                    let end = code(end)?;
                    if start > end || end > 0x10_ffff {
                        return Err(Error::new(key, "invalid Unicode range"));
                    }
                    self.codepoints.push(FontMap {
                        start,
                        end,
                        family: family.into(),
                    });
                }
            }
            "adjust-cell-width" => self.cell_width = parse_height(value)?,
            "font-thicken" => {
                self.thicken = if value.is_empty() {
                    false
                } else {
                    value.parse().map_err(|e| Error::new(key, e))?
                }
            }
            "font-thicken-strength" => {
                self.thicken_strength = if value.is_empty() {
                    255
                } else {
                    value.parse().map_err(|e| Error::new(key, e))?
                };
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
