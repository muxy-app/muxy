#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Palette {
    pub(crate) background: u32,
    pub(crate) foreground: u32,
    pub(crate) cursor: u32,
    pub(crate) cursor_color: Option<muxy_app_core::settings::TerminalColor>,
    colors: [u32; 256],
    pub(crate) cursor_style: Option<muxy_protocol::CursorShape>,
    pub(crate) cursor_blink: Option<bool>,
    pub(crate) cursor_opacity: f32,
    pub(crate) cursor_text: Option<muxy_app_core::settings::TerminalColor>,
    pub(crate) selection_foreground: Option<muxy_app_core::settings::TerminalColor>,
    pub(crate) selection_background: Option<muxy_app_core::settings::TerminalColor>,
    bold_is_bright: bool,
}

impl Palette {
    pub(crate) fn terminal_colors(&self) -> muxy_protocol::TerminalColors {
        let rgb = |color: u32| {
            let [_, r, g, b] = color.to_be_bytes();
            [r, g, b]
        };
        muxy_protocol::TerminalColors {
            foreground: rgb(self.foreground),
            background: rgb(self.background),
            cursor: rgb(self.cursor),
            ansi: std::array::from_fn(|index| rgb(self.colors[index])),
            palette: (16..=255)
                .map(|index| (index, rgb(self.colors[usize::from(index)])))
                .collect(),
            cursor_style: self.cursor_style,
            cursor_blink: self.cursor_blink,
        }
    }

    pub(crate) fn from_scheme(scheme: &muxy_ui::theme::ColorScheme, dark: bool) -> Self {
        let mut palette = Self::new(dark);
        if let Some(color) = scheme.background {
            palette.background = u32::from(color) >> 8;
        }
        if let Some(color) = scheme.foreground {
            palette.foreground = u32::from(color) >> 8;
        }
        let color = |color| match color {
            muxy_ui::theme::CellColor::Rgb(value) => {
                muxy_app_core::settings::TerminalColor::Rgb(u32::from(value) >> 8)
            }
            muxy_ui::theme::CellColor::Foreground => {
                muxy_app_core::settings::TerminalColor::CellForeground
            }
            muxy_ui::theme::CellColor::Background => {
                muxy_app_core::settings::TerminalColor::CellBackground
            }
        };
        palette.cursor_color = scheme.cursor_color.map(color);
        palette.cursor = palette.cursor_color.map_or(palette.foreground, |color| {
            color.resolve(palette.foreground, palette.background)
        });
        palette.cursor_text = scheme.cursor_text.map(color);
        palette.selection_foreground = scheme.selection_foreground.map(color);
        palette.selection_background = scheme.selection_background.map(color);
        for (index, color) in palette.colors.iter_mut().enumerate() {
            if let Some(value) = scheme.palette_color(index) {
                *color = u32::from(value) >> 8;
            }
        }
        palette
    }

    pub(crate) fn resolve(&self, color: muxy_protocol::Color, default: u32) -> u32 {
        match color {
            muxy_protocol::Color::Default => default,
            muxy_protocol::Color::Indexed(index) => self.indexed(index),
            muxy_protocol::Color::Rgb(red, green, blue) => {
                (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
            }
        }
    }

    pub(crate) fn style(&self, style: muxy_protocol::Style) -> (u32, u32) {
        let fg = match style.fg {
            muxy_protocol::Color::Indexed(index @ 0..=7) if style.bold && self.bold_is_bright => {
                muxy_protocol::Color::Indexed(index + 8)
            }
            other => other,
        };
        let foreground = self.resolve(fg, self.foreground);
        let background = self.resolve(style.bg, self.background);
        if style.inverse {
            (background, foreground)
        } else {
            (foreground, background)
        }
    }

    pub(crate) const fn new(dark: bool) -> Self {
        if dark {
            Self {
                background: 0x19_17_1f,
                foreground: 0xc9_c2_d9,
                cursor: 0xc9_c2_d9,
                colors: default_colors([
                    0x46_40_56, 0xec_48_99, 0x34_d3_99, 0xe0_af_68, 0xc3_70_d3, 0x63_66_f1,
                    0x22_d3_ee, 0xa9_b1_d6, 0x7c_73_93, 0xf4_72_b6, 0x6e_e7_b7, 0xfb_bf_24,
                    0xd9_9b_e5, 0x81_8c_f8, 0x67_e8_f9, 0xc9_c2_d9,
                ]),
                cursor_color: None,
                cursor_style: None,
                cursor_blink: None,
                cursor_opacity: 1.0,
                cursor_text: None,
                selection_foreground: None,
                selection_background: None,
                bold_is_bright: false,
            }
        } else {
            Self {
                background: 0xf0_f0_f5,
                foreground: 0x1e_1e_2e,
                cursor: 0x1e_1e_2e,
                colors: default_colors([
                    0xd5_d6_db, 0xa3_2d_68, 0x1a_7a_4e, 0x9a_70_24, 0x47_96_f0, 0x7c_3a_ed,
                    0x0b_71_89, 0x3b_3f_5c, 0x7a_7e_94, 0xec_48_99, 0x34_d3_99, 0xe0_af_68,
                    0x6b_ab_f5, 0xa7_8b_fa, 0x22_d3_ee, 0x1e_1e_2e,
                ]),
                cursor_color: None,
                cursor_style: None,
                cursor_blink: None,
                cursor_opacity: 1.0,
                cursor_text: None,
                selection_foreground: None,
                selection_background: None,
                bold_is_bright: false,
            }
        }
    }

    pub(crate) fn indexed(&self, index: u8) -> u32 {
        self.colors[usize::from(index)]
    }

    pub(crate) fn with_options(
        mut self,
        options: &muxy_app_core::settings::TerminalOptions,
    ) -> Self {
        if let Some(value) = options.background {
            self.background = value;
        }
        if let Some(value) = options.foreground {
            self.foreground = value;
        }
        self.cursor_color = options.cursor_color.or(self.cursor_color);
        if let Some(value) = self.cursor_color {
            self.cursor = value.resolve(self.foreground, self.background);
        }
        for (&index, &color) in &options.palette {
            self.colors[usize::from(index)] = color;
        }
        self.cursor_style = options.cursor_style;
        self.cursor_blink = options.cursor_blink;
        self.cursor_opacity = options.cursor_opacity;
        self.cursor_text = options.cursor_text.or(self.cursor_text);
        self.selection_foreground = options.selection_foreground.or(self.selection_foreground);
        self.selection_background = options.selection_background.or(self.selection_background);
        self.bold_is_bright = options.bold_is_bright;
        self
    }
}

const fn default_colors(ansi: [u32; 16]) -> [u32; 256] {
    let mut colors = [0; 256];
    let mut index = 0;
    while index < 16 {
        colors[index] = ansi[index];
        index += 1;
    }
    let mut cube = 0u32;
    while cube < 216 {
        let r = cube / 36;
        let g = (cube / 6) % 6;
        let b = cube % 6;
        let r = if r == 0 { 0 } else { 55 + 40 * r };
        let g = if g == 0 { 0 } else { 55 + 40 * g };
        let b = if b == 0 { 0 } else { 55 + 40 * b };
        colors[index] = (r << 16) | (g << 8) | b;
        index += 1;
        cube += 1;
    }
    let mut level = 8;
    while index < 256 {
        colors[index] = (level << 16) | (level << 8) | level;
        level += 10;
        index += 1;
    }
    colors
}
