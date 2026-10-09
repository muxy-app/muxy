use std::path::Path;

use super::{CellHeight, Error, Result, TerminalBindings, TerminalColor, TerminalSettings};

impl TerminalSettings {
    pub fn load_native(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(source) => Self::from_native_source(&source),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let legacy = path.with_file_name("ghostty.conf");
                let settings = if legacy.try_exists().map_err(|e| Error::new("terminal", e))? {
                    Self::resolve(&legacy)?.0
                } else {
                    Self::default()
                };
                settings.save_native(path)?;
                Ok(settings)
            }
            Err(error) => Err(Error::new("terminal.toml", error)),
        }
    }

    pub fn from_native_source(source: &str) -> Result<Self> {
        let settings: Self = toml::from_str(source).map_err(|e| Error::new("terminal.toml", e))?;
        settings.validate_native()?;
        Ok(settings)
    }

    pub fn native_source(&self) -> Result<String> {
        self.validate_native()?;
        let source = toml::to_string_pretty(self).map_err(|e| Error::new("terminal.toml", e))?;
        Self::from_native_source(&source)?;
        Ok(source)
    }

    pub fn save_native(&self, path: &Path) -> Result<()> {
        let source = self.native_source()?;
        match std::fs::read_to_string(path) {
            Ok(previous) => {
                Self::from_native_source(&previous)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::new("terminal.toml", error)),
        }
        crate::settings::appearance::atomic_write(path, &source)
            .map_err(|error| Error::new("terminal.toml", error))
    }

    pub fn from_legacy_source(source: &str) -> Result<Self> {
        let mut settings = Self::default();
        let mut families = Vec::new();
        if !settings
            .read_source(source, Path::new("ghostty.conf"), &mut families, None)?
            .is_empty()
        {
            return Err(Error::new(
                "terminal",
                "imports cannot contain config-file references",
            ));
        }
        if !families.is_empty() {
            settings.font_families = families;
        }
        settings.validate_native()?;
        Ok(settings)
    }

    pub fn legacy_source(&self) -> String {
        let mut lines = vec![
            "font-family =".into(),
            format!("font-size = {}", self.font_size),
            format!("adjust-cell-height = {}", self.cell_height),
            format!("macos-option-as-alt = {}", self.macos_option_as_alt),
        ];
        lines.extend(
            self.font_families
                .iter()
                .map(|name| format!("font-family = \"{name}\"")),
        );
        lines.extend(self.font.lines());
        lines.extend(
            self.options
                .values()
                .into_iter()
                .map(|(key, value)| format!("{key} = {value}")),
        );
        lines.extend(
            self.keybindings
                .lines()
                .into_iter()
                .map(|value| format!("keybind = {value}")),
        );
        lines.join("\n") + "\n"
    }

    fn validate_native(&self) -> Result<()> {
        bounded("font size", self.font_size, 1.0, 256.0)?;
        for adjustment in [
            self.cell_height,
            self.font.cell_width,
            self.options.cursor_thickness,
        ] {
            adjustment.to_string().parse::<CellHeight>()?;
        }
        if self.font_families.is_empty() {
            return Err(Error::new("font family", "at least one font is required"));
        }
        for name in self
            .font_families
            .iter()
            .chain(&self.font.bold)
            .chain(&self.font.italic)
            .chain(&self.font.bold_italic)
            .chain(self.font.codepoints.iter().map(|map| &map.family))
        {
            if name.trim().is_empty() || name.chars().any(|ch| ch.is_control() || ch == '"') {
                return Err(Error::new(
                    "font family",
                    "enter a font name without quotes or control characters",
                ));
            }
        }
        if self
            .font
            .features
            .iter()
            .any(|(tag, _)| tag.len() != 4 || !tag.bytes().all(|b| b.is_ascii_alphanumeric()))
            || self
                .font
                .codepoints
                .iter()
                .any(|map| map.start > map.end || map.end > 0x10_ffff)
        {
            return Err(Error::new("font", "invalid font feature or Unicode range"));
        }
        bounded(
            "background vibrancy",
            f32::from(self.options.background_vibrancy),
            0.0,
            100.0,
        )?;
        bounded("cursor opacity", self.options.cursor_opacity, 0.0, 1.0)?;
        if let Some(opacity) = self.options.background_opacity {
            bounded("background opacity", opacity, 0.0, 1.0)?;
        }
        for amount in self
            .options
            .padding_x
            .into_iter()
            .chain(self.options.padding_y)
        {
            bounded("padding", amount, 0.0, 4096.0)?;
        }
        for amount in [self.options.scroll_precision, self.options.scroll_discrete] {
            bounded("scroll speed", amount, 0.01, 10_000.0)?;
        }
        if self
            .options
            .background
            .into_iter()
            .chain(self.options.foreground)
            .chain(self.options.palette.values().copied())
            .any(|color| color > 0xff_ffff)
            || matches!(
                self.options.cursor_style,
                Some(muxy_protocol::CursorShape::Unrecognized(_))
            )
        {
            return Err(Error::new("terminal", "invalid color or cursor shape"));
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub fn set_preference(&mut self, id: &str, value: &str) -> Result<()> {
        let mut next = self.clone();
        let value = value.trim();
        let flag = || value.parse::<bool>().map_err(|e| Error::new(id, e));
        let number = || value.parse::<f32>().map_err(|e| Error::new(id, e));
        match id {
            "font-family" => {
                if let Some(family) = next.font_families.first_mut() {
                    *family = value.into();
                } else {
                    next.font_families.push(value.into());
                }
            }
            "font-size" => next.font_size = number()?,
            "adjust-cell-height" => next.cell_height = value.parse()?,
            "adjust-cell-width" => next.font.cell_width = value.parse()?,
            "font-thicken" => next.font.thicken = flag()?,
            "font-ligatures" => {
                let enabled = flag()?;
                next.font
                    .features
                    .retain(|(name, _)| !matches!(name.as_str(), "calt" | "liga" | "dlig"));
                if !enabled {
                    next.font
                        .features
                        .extend(["calt", "liga", "dlig"].map(|name| (name.into(), 0)));
                }
            }
            "background-transparency" => {
                let percent = number()?;
                bounded(id, percent, 0.0, 100.0)?;
                next.options.background_opacity = Some(1.0 - percent / 100.0);
            }
            "background-vibrancy" => {
                next.options.background_vibrancy = value.parse().map_err(|e| Error::new(id, e))?;
            }
            "padding-left" => next.options.padding_x[0] = number()?,
            "padding-right" => next.options.padding_x[1] = number()?,
            "padding-top" => next.options.padding_y[0] = number()?,
            "padding-bottom" => next.options.padding_y[1] = number()?,
            "background-opacity" => {
                let percent = number()?;
                bounded(id, percent, 0.0, 100.0)?;
                next.options.background_opacity = Some(percent / 100.0);
            }
            "background-opacity-cells" => next.options.background_opacity_cells = flag()?,
            "cursor-style" | "cursor-style-blink" | "window-padding-x" | "window-padding-y" => {
                next.options.read(id, value)?;
            }
            "adjust-cursor-thickness" => next.options.cursor_thickness = value.parse()?,
            "window-padding-balance" => next.options.padding_balance = flag()?,
            "copy-on-select" => next.options.copy_on_select = Some(flag()?),
            "selection-clear-on-typing" => next.options.selection_clear_on_typing = flag()?,
            "selection-clear-on-copy" => next.options.selection_clear_on_copy = flag()?,
            "bold-is-bright" => next.options.bold_is_bright = flag()?,
            "mouse-reporting" => next.options.mouse_reporting = flag()?,
            "scroll-on-keystroke" => next.options.scroll_on_keystroke = flag()?,
            "scroll-on-output" => next.options.scroll_on_output = flag()?,
            "scroll-precision" => next.options.scroll_precision = number()?,
            "scroll-discrete" => next.options.scroll_discrete = number()?,
            "macos-option-as-alt" => next.macos_option_as_alt = value.parse()?,
            _ => return Err(Error::new(id, "unknown terminal setting")),
        }
        next.validate_native()?;
        *self = next;
        Ok(())
    }

    pub fn ligatures_enabled(&self) -> bool {
        !self
            .font
            .features
            .iter()
            .any(|(name, value)| matches!(name.as_str(), "calt" | "liga" | "dlig") && *value == 0)
    }
}

fn bounded(name: &str, value: f32, min: f32, max: f32) -> Result<()> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(Error::new(
            name,
            format!("enter a number between {min} and {max}"),
        ))
    }
}

impl TryFrom<String> for CellHeight {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

impl From<CellHeight> for String {
    fn from(value: CellHeight) -> Self {
        value.to_string()
    }
}

impl TryFrom<String> for TerminalColor {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        super::options::color(&value)
    }
}

impl From<TerminalColor> for String {
    fn from(value: TerminalColor) -> Self {
        match value {
            TerminalColor::Rgb(color) => format!("{color:06x}"),
            TerminalColor::CellForeground => "cell-foreground".into(),
            TerminalColor::CellBackground => "cell-background".into(),
        }
    }
}

impl TryFrom<Vec<String>> for TerminalBindings {
    type Error = Error;
    fn try_from(values: Vec<String>) -> Result<Self> {
        let mut bindings = Self::default();
        for value in values {
            if let Some(warning) = bindings.read(&value)?
                && value != "clear"
            {
                return Err(Error::new("terminal keybindings", warning));
            }
        }
        Ok(bindings)
    }
}

impl From<TerminalBindings> for Vec<String> {
    fn from(value: TerminalBindings) -> Self {
        value.lines()
    }
}
