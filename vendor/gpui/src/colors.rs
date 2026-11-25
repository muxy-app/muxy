use crate::{App, Global, Rgba, Window, WindowAppearance, rgb};
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Colors {
    pub text: Rgba,
    pub selected_text: Rgba,
    pub background: Rgba,
    pub disabled: Rgba,
    pub selected: Rgba,
    pub border: Rgba,
    pub separator: Rgba,
    pub container: Rgba,
}

impl Default for Colors {
    fn default() -> Self {
        Self::light()
    }
}

impl Colors {
    pub fn for_appearance(window: &Window) -> Self {
        match window.appearance() {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::light(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::dark(),
        }
    }

    pub fn dark() -> Self {
        Self {
            text: rgb(0xffffff),
            selected_text: rgb(0xffffff),
            disabled: rgb(0x565656),
            selected: rgb(0x2457ca),
            background: rgb(0x222222),
            border: rgb(0x000000),
            separator: rgb(0xd9d9d9),
            container: rgb(0x262626),
        }
    }

    pub fn light() -> Self {
        Self {
            text: rgb(0x252525),
            selected_text: rgb(0xffffff),
            background: rgb(0xffffff),
            disabled: rgb(0xb0b0b0),
            selected: rgb(0x2a63d9),
            border: rgb(0xd9d9d9),
            separator: rgb(0xe6e6e6),
            container: rgb(0xf4f5f5),
        }
    }

    pub fn get_global(cx: &App) -> &Arc<Colors> {
        &cx.global::<GlobalColors>().0
    }
}

#[derive(Clone, Debug)]
pub struct GlobalColors(pub Arc<Colors>);

impl Deref for GlobalColors {
    type Target = Arc<Colors>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Global for GlobalColors {}

pub trait DefaultColors {
    fn default_colors(&self) -> &Arc<Colors>;
}

impl DefaultColors for App {
    fn default_colors(&self) -> &Arc<Colors> {
        &self.global::<GlobalColors>().0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DefaultAppearance {
    #[default]
    Light,
    Dark,
}

impl From<WindowAppearance> for DefaultAppearance {
    fn from(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self::Light,
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
        }
    }
}
