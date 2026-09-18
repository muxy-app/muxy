use super::{Error, Result};
use crate::composer::submission::ImageSubmissionStrategy;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComposerPosition {
    Right,
    #[default]
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComposerPresentation {
    #[default]
    Panel,
    Floating,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent composer preferences"
)]
pub struct ComposerSettings {
    pub presentation: ComposerPresentation,
    pub floating_width: f32,
    pub floating_height: f32,
    pub expanded: bool,
    pub font_size: f32,
    pub position: ComposerPosition,
    pub pinned: bool,
    pub width: f32,
    pub height: f32,
    pub broadcast: bool,
    pub clear_after_sending: bool,
    pub clear_on_close: bool,
    pub image_strategy: ImageSubmissionStrategy,
    pub font_family: String,
    pub line_height: f32,
    pub language: String,
    pub voice_auto_send: bool,
}

impl Default for ComposerSettings {
    fn default() -> Self {
        Self {
            presentation: ComposerPresentation::Panel,
            floating_width: 570.0,
            floating_height: 236.0,
            expanded: false,
            font_size: 13.0,
            position: ComposerPosition::Right,
            pinned: false,
            width: 380.0,
            height: 220.0,
            broadcast: false,
            clear_after_sending: false,
            clear_on_close: false,
            image_strategy: ImageSubmissionStrategy::Clipboard,
            font_family: "SF Mono".into(),
            line_height: 1.2,
            language: String::new(),
            voice_auto_send: false,
        }
    }
}

impl ComposerSettings {
    pub fn validate(&self) -> Result<()> {
        for (name, value, min, max) in [
            ("width", self.width, 280.0, 800.0),
            ("floating_width", self.floating_width, 1.0, 16384.0),
            ("floating_height", self.floating_height, 1.0, 16384.0),
            ("font_size", self.font_size, 9.0, 32.0),
            ("height", self.height, 120.0, 600.0),
            ("line_height", self.line_height, 1.1, 2.0),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(Error::new(
                    format!("composer.{name}"),
                    format!("must be between {min} and {max}"),
                ));
            }
        }
        if self.font_family.len() > 200 || self.language.len() > 200 {
            return Err(Error::new(
                "composer",
                "Font and language names must be at most 200 bytes",
            ));
        }
        Ok(())
    }
}
