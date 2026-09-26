use std::collections::BTreeMap;

use crate::settings::{Error, Result};

pub(super) const KEYS: &[&str] = &[
    "theme",
    "background",
    "foreground",
    "palette",
    "cursor-color",
    "cursor-text",
    "cursor-opacity",
    "cursor-style",
    "cursor-style-blink",
    "selection-foreground",
    "selection-background",
    "selection-clear-on-typing",
    "selection-clear-on-copy",
    "background-opacity",
    "background-opacity-cells",
    "bold-is-bright",
    "copy-on-select",
    "mouse-reporting",
    "mouse-scroll-multiplier",
    "scroll-to-bottom",
    "window-padding-x",
    "window-padding-y",
    "window-padding-balance",
    "window-padding-color",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalColor {
    Rgb(u32),
    CellForeground,
    CellBackground,
}

impl TerminalColor {
    pub fn resolve(self, foreground: u32, background: u32) -> u32 {
        match self {
            Self::Rgb(color) => color,
            Self::CellForeground => foreground,
            Self::CellBackground => background,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PaddingColor {
    Background,
    #[default]
    Extend,
    ExtendAlways,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq)]
pub struct TerminalOptions {
    pub theme: Option<String>,
    pub background: Option<u32>,
    pub foreground: Option<u32>,
    pub palette: BTreeMap<u8, u32>,
    pub cursor_color: Option<TerminalColor>,
    pub cursor_text: Option<TerminalColor>,
    pub cursor_opacity: f32,
    pub cursor_style: Option<muxy_protocol::CursorShape>,
    pub cursor_blink: Option<bool>,
    pub selection_foreground: Option<TerminalColor>,
    pub selection_background: Option<TerminalColor>,
    pub selection_clear_on_typing: bool,
    pub selection_clear_on_copy: bool,
    pub background_opacity: Option<f32>,
    pub background_opacity_cells: bool,
    pub bold_is_bright: bool,
    pub copy_on_select: Option<bool>,
    pub mouse_reporting: bool,
    pub scroll_precision: f32,
    pub scroll_discrete: f32,
    pub scroll_on_keystroke: bool,
    pub scroll_on_output: bool,
    pub padding_x: [f32; 2],
    pub padding_y: [f32; 2],
    pub padding_balance: bool,
    pub padding_color: PaddingColor,
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            theme: None,
            background: None,
            foreground: None,
            palette: BTreeMap::new(),
            cursor_color: None,
            cursor_text: None,
            cursor_opacity: 1.0,
            cursor_style: None,
            cursor_blink: None,
            selection_foreground: None,
            selection_background: None,
            selection_clear_on_typing: false,
            selection_clear_on_copy: false,
            background_opacity: None,
            background_opacity_cells: false,
            bold_is_bright: false,
            copy_on_select: None,
            mouse_reporting: true,
            scroll_precision: 1.0,
            scroll_discrete: 1.0,
            scroll_on_keystroke: true,
            scroll_on_output: false,
            padding_x: [2.0; 2],
            padding_y: [2.0; 2],
            padding_balance: false,
            padding_color: PaddingColor::default(),
        }
    }
}

impl TerminalOptions {
    #[allow(
        clippy::too_many_lines,
        reason = "The configuration key table stays together for auditing"
    )]
    pub(super) fn values(&self) -> BTreeMap<&'static str, String> {
        let rgb =
            |value: Option<u32>| value.map_or_else(String::new, |color| format!("{color:06x}"));
        let color = |value: Option<TerminalColor>| match value {
            Some(TerminalColor::Rgb(value)) => format!("{value:06x}"),
            Some(TerminalColor::CellForeground) => "cell-foreground".into(),
            Some(TerminalColor::CellBackground) => "cell-background".into(),
            None => String::new(),
        };
        BTreeMap::from([
            ("theme", self.theme.clone().unwrap_or_default()),
            ("background", rgb(self.background)),
            ("foreground", rgb(self.foreground)),
            (
                "palette",
                self.palette
                    .iter()
                    .map(|(index, color)| format!("{index}={color:06x}"))
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            ("cursor-color", color(self.cursor_color)),
            ("cursor-text", color(self.cursor_text)),
            ("cursor-opacity", self.cursor_opacity.to_string()),
            (
                "cursor-style",
                self.cursor_style
                    .map_or("", |style| match style {
                        muxy_protocol::CursorShape::Block
                        | muxy_protocol::CursorShape::Unrecognized(_) => "block",
                        muxy_protocol::CursorShape::Bar => "bar",
                        muxy_protocol::CursorShape::Underline => "underline",
                        muxy_protocol::CursorShape::Hollow => "block_hollow",
                    })
                    .into(),
            ),
            (
                "cursor-style-blink",
                self.cursor_blink
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            ("selection-foreground", color(self.selection_foreground)),
            ("selection-background", color(self.selection_background)),
            (
                "selection-clear-on-typing",
                self.selection_clear_on_typing.to_string(),
            ),
            (
                "selection-clear-on-copy",
                self.selection_clear_on_copy.to_string(),
            ),
            (
                "background-opacity",
                self.background_opacity
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            (
                "background-opacity-cells",
                self.background_opacity_cells.to_string(),
            ),
            ("bold-is-bright", self.bold_is_bright.to_string()),
            (
                "copy-on-select",
                self.copy_on_select
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            ("mouse-reporting", self.mouse_reporting.to_string()),
            (
                "mouse-scroll-multiplier",
                format!(
                    "precision:{},discrete:{}",
                    self.scroll_precision, self.scroll_discrete
                ),
            ),
            (
                "scroll-to-bottom",
                format!(
                    "{},{}",
                    if self.scroll_on_keystroke {
                        "keystroke"
                    } else {
                        "no-keystroke"
                    },
                    if self.scroll_on_output {
                        "output"
                    } else {
                        "no-output"
                    }
                ),
            ),
            (
                "window-padding-x",
                format!("{},{}", self.padding_x[0], self.padding_x[1]),
            ),
            (
                "window-padding-y",
                format!("{},{}", self.padding_y[0], self.padding_y[1]),
            ),
            ("window-padding-balance", self.padding_balance.to_string()),
            (
                "window-padding-color",
                match self.padding_color {
                    PaddingColor::Background => "background",
                    PaddingColor::Extend => "extend",
                    PaddingColor::ExtendAlways => "extend-always",
                }
                .into(),
            ),
        ])
    }

    pub(super) fn boolean(key: &str) -> bool {
        matches!(
            key,
            "cursor-style-blink"
                | "selection-clear-on-typing"
                | "selection-clear-on-copy"
                | "background-opacity-cells"
                | "bold-is-bright"
                | "copy-on-select"
                | "mouse-reporting"
                | "window-padding-balance"
        )
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Keep the terminal option grammar in one match"
    )]
    pub(super) fn read(&mut self, key: &str, value: &str) -> Result<bool> {
        match key {
            "theme" => self.theme = (!value.is_empty()).then(|| value.to_owned()),
            "background" => self.background = optional(value, rgb)?,
            "foreground" => self.foreground = optional(value, rgb)?,
            "palette" if value.is_empty() => self.palette.clear(),
            "palette" => {
                for entry in value.split(',') {
                    let (index, color) = entry
                        .split_once('=')
                        .ok_or_else(|| Error::new(key, "expected index=color"))?;
                    let index = index
                        .trim()
                        .parse::<u8>()
                        .map_err(|error| Error::new(key, error))?;
                    self.palette.insert(index, rgb(color.trim())?);
                }
            }
            "cursor-color" => self.cursor_color = optional(value, color)?,
            "cursor-text" => self.cursor_text = optional(value, color)?,
            "cursor-opacity" => {
                self.cursor_opacity = if value.is_empty() {
                    1.0
                } else {
                    number(value)?.clamp(0.0, 1.0)
                }
            }
            "cursor-style" => {
                self.cursor_style = optional(value, |value| match value {
                    "block" => Ok(muxy_protocol::CursorShape::Block),
                    "bar" => Ok(muxy_protocol::CursorShape::Bar),
                    "underline" => Ok(muxy_protocol::CursorShape::Underline),
                    "block_hollow" => Ok(muxy_protocol::CursorShape::Hollow),
                    _ => Err(Error::new(
                        key,
                        "expected block, bar, underline, or block_hollow",
                    )),
                })?;
            }
            "cursor-style-blink" => {
                self.cursor_blink = optional(value, |value| boolean(value, true))?;
            }
            "selection-foreground" => self.selection_foreground = optional(value, color)?,
            "selection-background" => self.selection_background = optional(value, color)?,
            "selection-clear-on-typing" => self.selection_clear_on_typing = boolean(value, false)?,
            "selection-clear-on-copy" => self.selection_clear_on_copy = boolean(value, false)?,
            "background-opacity" => {
                self.background_opacity =
                    optional(value, |value| Ok(number(value)?.clamp(0.0, 1.0)))?;
            }
            "background-opacity-cells" => self.background_opacity_cells = boolean(value, false)?,
            "bold-is-bright" => self.bold_is_bright = boolean(value, false)?,
            "copy-on-select" => {
                self.copy_on_select = optional(value, |value| {
                    if value == "clipboard" {
                        Ok(true)
                    } else {
                        boolean(value, false)
                    }
                })?;
            }
            "mouse-reporting" => self.mouse_reporting = boolean(value, true)?,
            "mouse-scroll-multiplier" => self.read_scroll(value)?,
            "scroll-to-bottom" => {
                self.scroll_on_keystroke = true;
                self.scroll_on_output = false;
                for flag in value
                    .split(',')
                    .map(str::trim)
                    .filter(|flag| !flag.is_empty())
                {
                    match flag {
                        "keystroke" => self.scroll_on_keystroke = true,
                        "no-keystroke" => self.scroll_on_keystroke = false,
                        "output" => self.scroll_on_output = true,
                        "no-output" => self.scroll_on_output = false,
                        _ => {
                            return Err(Error::new(
                                key,
                                "expected keystroke, no-keystroke, output, or no-output",
                            ));
                        }
                    }
                }
            }
            "window-padding-x" => self.padding_x = padding(value)?,
            "window-padding-y" => self.padding_y = padding(value)?,
            "window-padding-balance" => self.padding_balance = boolean(value, false)?,
            "window-padding-color" => {
                self.padding_color = match value {
                    "background" => PaddingColor::Background,
                    "" | "extend" => PaddingColor::Extend,
                    "extend-always" => PaddingColor::ExtendAlways,
                    _ => {
                        return Err(Error::new(
                            key,
                            "expected background, extend, or extend-always",
                        ));
                    }
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn read_scroll(&mut self, value: &str) -> Result<()> {
        self.scroll_precision = 1.0;
        self.scroll_discrete = 1.0;
        for part in value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let (kind, value) = part.split_once(':').unwrap_or(("both", part));
            let amount = number(value.trim())?.clamp(0.01, 10_000.0);
            match kind.trim() {
                "both" => {
                    self.scroll_precision = amount;
                    self.scroll_discrete = amount;
                }
                "precision" => self.scroll_precision = amount,
                "discrete" => self.scroll_discrete = amount,
                _ => {
                    return Err(Error::new(
                        "mouse-scroll-multiplier",
                        "expected precision or discrete",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn optional<T>(value: &str, parse: impl FnOnce(&str) -> Result<T>) -> Result<Option<T>> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse(value).map(Some)
    }
}

fn boolean(value: &str, default: bool) -> Result<bool> {
    if value.is_empty() {
        Ok(default)
    } else {
        value.parse().map_err(|error| Error::new("boolean", error))
    }
}

fn number(value: &str) -> Result<f32> {
    let number = value
        .parse::<f32>()
        .map_err(|error| Error::new("number", error))?;
    if number.is_finite() {
        Ok(number)
    } else {
        Err(Error::new("number", "must be finite"))
    }
}

fn padding(value: &str) -> Result<[f32; 2]> {
    if value.is_empty() {
        return Ok([2.0; 2]);
    }
    let (a, b) = value.split_once(',').unwrap_or((value, value));
    let result = [number(a.trim())?, number(b.trim())?];
    if result.iter().any(|value| !(0.0..=4096.0).contains(value)) {
        return Err(Error::new("padding", "must be between 0 and 4096 points"));
    }
    Ok(result)
}

fn color(value: &str) -> Result<TerminalColor> {
    match value {
        "cell-foreground" => Ok(TerminalColor::CellForeground),
        "cell-background" => Ok(TerminalColor::CellBackground),
        _ => rgb(value).map(TerminalColor::Rgb),
    }
}

pub(super) fn rgb(value: &str) -> Result<u32> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() == 6
        && let Ok(color) = u32::from_str_radix(hex, 16)
    {
        return Ok(color);
    }
    match value.to_ascii_lowercase().as_str() {
        "black" => Ok(0),
        "white" => Ok(0xff_ffff),
        "red" => Ok(0xff_0000),
        "green" => Ok(0x00_ff00),
        "blue" => Ok(0x00_00ff),
        "yellow" => Ok(0xff_ff00),
        "magenta" => Ok(0xff_00ff),
        "cyan" => Ok(0x00_ffff),
        "gray" | "grey" => Ok(0xbe_bebe),
        _ => Err(Error::new(
            "color",
            "expected RRGGBB, #RRGGBB, or a supported color name",
        )),
    }
}
