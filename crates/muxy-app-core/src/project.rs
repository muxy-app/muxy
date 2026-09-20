use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    #[serde(default)]
    pub home: bool,
    pub name: String,
    pub icon: Option<String>,
    pub color: Color,
    pub server_id: ServerId,
    #[serde(with = "crate::catalog::directory")]
    pub directory: PathBuf,
    pub kind: Option<ProjectKind>,
    pub parent_id: Option<ProjectId>,
    pub tabs: Vec<Tab>,
    #[serde(skip)]
    pub(crate) status: ProjectStatus,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProjectStatus {
    #[default]
    Available,
    Missing,
}

pub const PROJECT_COLORS: [(&str, &str); 12] = [
    ("Red", "#e5484d"),
    ("Orange", "#f76b15"),
    ("Amber", "#f5a623"),
    ("Yellow", "#ebcb00"),
    ("Lime", "#9bcd1e"),
    ("Green", "#30a46c"),
    ("Teal", "#12a594"),
    ("Cyan", "#05a2c2"),
    ("Blue", "#3e63dd"),
    ("Indigo", "#5b5bd6"),
    ("Violet", "#8e4ec6"),
    ("Pink", "#d6409f"),
];

impl Project {
    pub fn status(&self) -> ProjectStatus {
        self.status
    }

    pub fn initial(&self) -> &str {
        self.name.graphemes(true).next().unwrap_or("?")
    }

    pub(crate) fn refresh_status(&mut self) {
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

pub(crate) fn validate_icon(icon: Option<&str>) -> Result<(), AppError> {
    if icon.is_some_and(|icon| icon.trim().is_empty() || icon.graphemes(true).count() != 1) {
        Err(AppError::InvalidState(
            "project icon must be one character or empty".into(),
        ))
    } else {
        Ok(())
    }
}

pub use muxy_protocol::ProjectKind;
