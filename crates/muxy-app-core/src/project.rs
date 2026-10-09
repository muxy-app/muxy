use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use muxy_core::tr_key;
use muxy_protocol::PROJECT_COLORS as PALETTE;
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::{AppError, ProjectId, ServerId, Tab};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color(String);

impl Color {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Color {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() == 7
            && value.starts_with('#')
            && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        {
            Ok(Self(value.to_ascii_lowercase()))
        } else {
            Err(AppError::InvalidState(
                "project color must be a #RRGGBB hex string".into(),
            ))
        }
    }
}

impl TryFrom<String> for Color {
    type Error = AppError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Color> for String {
    fn from(color: Color) -> Self {
        color.0
    }
}

impl Default for Color {
    fn default() -> Self {
        Self("#808080".into())
    }
}

impl fmt::Display for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The first character of a name, which stands for it on a project's tile.
pub fn initial(name: &str) -> &str {
    name.graphemes(true).next().unwrap_or("?")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    #[serde(default)]
    pub home: bool,
    pub name: String,
    pub icon: Option<String>,
    #[serde(default)]
    pub logo: Option<std::sync::Arc<[u8]>>,
    pub color: Color,
    pub server_id: ServerId,
    #[serde(with = "crate::catalog::directory")]
    pub directory: PathBuf,
    pub kind: Option<ProjectKind>,
    pub parent_id: Option<ProjectId>,
    pub tabs: Vec<Tab>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) groups: Option<crate::TabGroups>,
    #[serde(skip)]
    pub(crate) status: ProjectStatus,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProjectStatus {
    #[default]
    Available,
    Missing,
}

/// The shared palette, by name. Names are English keys, translated where
/// shown.
pub const PROJECT_COLORS: [(&str, &str); 12] = [
    (tr_key!("Red"), PALETTE[0]),
    (tr_key!("Orange"), PALETTE[1]),
    (tr_key!("Amber"), PALETTE[2]),
    (tr_key!("Yellow"), PALETTE[3]),
    (tr_key!("Lime"), PALETTE[4]),
    (tr_key!("Green"), PALETTE[5]),
    (tr_key!("Teal"), PALETTE[6]),
    (tr_key!("Cyan"), PALETTE[7]),
    (tr_key!("Blue"), PALETTE[8]),
    (tr_key!("Indigo"), PALETTE[9]),
    (tr_key!("Violet"), PALETTE[10]),
    (tr_key!("Pink"), PALETTE[11]),
];

impl Project {
    pub fn status(&self) -> ProjectStatus {
        self.status
    }

    /// Another computer's Home, which stands for its server.
    pub fn is_remote_home(&self) -> bool {
        self.home && !self.server_id.is_local()
    }

    /// Only this computer's folders can be checked here. Another computer's
    /// projects keep what their server last said.
    pub(crate) fn refresh_status(&mut self) {
        if !self.server_id.is_local() {
            return;
        }
        self.status = if self.directory.is_dir()
            && (self.kind != Some(ProjectKind::Worktree) || self.directory.join(".git").is_file())
        {
            ProjectStatus::Available
        } else {
            ProjectStatus::Missing
        };
    }

    pub(crate) fn require_available(&self) -> Result<(), AppError> {
        if self.status == ProjectStatus::Missing {
            Err(AppError::InvalidState("project folder is missing".into()))
        } else {
            Ok(())
        }
    }
}

pub use muxy_protocol::ProjectKind;
