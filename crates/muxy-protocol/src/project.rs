use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(#[serde(with = "uuid::serde::simple")] Uuid);

        impl $name {
            pub const fn from_u128(value: u128) -> Self {
                Self(Uuid::from_u128(value))
            }

            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.simple().fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

id!(ProjectId);
id!(OperationId);
id!(ClientId);
id!(ServerIdentity);

use crate::{ErrorCode, ServerPath, SessionId, SessionInfo};
use unicode_segmentation::UnicodeSegmentation;

pub const CATALOG_PAGE_SIZE: usize = 128;
pub const MAX_PROJECTS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Worktree,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectDescriptor {
    pub id: ProjectId,
    pub home: bool,
    pub directory: ServerPath,
    pub name: String,
    pub icon: Option<String>,
    #[serde(default)]
    pub logo: Option<std::sync::Arc<[u8]>>,
    pub color: String,
    pub kind: Option<ProjectKind>,
    pub parent_id: Option<ProjectId>,
}

impl ProjectDescriptor {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        validate_name(&self.name)?;
        validate_icon(self.icon.as_deref())?;
        validate_color(&self.color)?;
        validate_logo(self.logo.as_deref())?;
        if !self.directory.0.starts_with(b"/")
            || self.directory.0.len() > 4096
            || self.directory.0.contains(&0)
            || self.parent_id == Some(self.id)
            || (self.kind == Some(ProjectKind::Worktree) && self.parent_id.is_none())
            || (self.home && (self.kind.is_some() || self.parent_id.is_some()))
        {
            return Err(ErrorCode::BadRequest);
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<(), ErrorCode> {
    if name.trim().is_empty() || name.len() > 1024 {
        Err(ErrorCode::BadRequest)
    } else {
        Ok(())
    }
}

fn validate_icon(icon: Option<&str>) -> Result<(), ErrorCode> {
    if icon.is_some_and(|value| {
        value.len() > 128
            || value.trim().is_empty()
            || (value.graphemes(true).count() != 1 && !is_project_symbol(value))
    }) {
        Err(ErrorCode::BadRequest)
    } else {
        Ok(())
    }
}

/// SF Symbol names are tagged so existing one-grapheme icons keep their meaning.
pub fn is_project_symbol(value: &str) -> bool {
    value.strip_prefix("sf:").is_some_and(|name| {
        !name.is_empty()
            && name.len() <= 125
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.')
    })
}

pub const MAX_PROJECT_LOGO_BYTES: usize = 300 * 1024;

fn validate_logo(logo: Option<&[u8]>) -> Result<(), ErrorCode> {
    let Some(bytes) = logo else {
        return Ok(());
    };
    if bytes.len() > MAX_PROJECT_LOGO_BYTES
        || bytes.len() < 33
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR")
    {
        return Err(ErrorCode::BadRequest);
    }
    let width = u32::from_be_bytes(
        bytes[16..20]
            .try_into()
            .map_err(|_| ErrorCode::BadRequest)?,
    );
    let height = u32::from_be_bytes(
        bytes[20..24]
            .try_into()
            .map_err(|_| ErrorCode::BadRequest)?,
    );
    if width == 0 || width > 256 || height != width {
        return Err(ErrorCode::BadRequest);
    }
    Ok(())
}

fn validate_color(color: &str) -> Result<(), ErrorCode> {
    if color.len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    {
        Ok(())
    } else {
        Err(ErrorCode::BadRequest)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProjectPatch {
    Name(String),
    Icon(Option<String>),
    Color(String),
    Logo(Option<std::sync::Arc<[u8]>>),
}

impl ProjectPatch {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Name(name) => validate_name(name),
            Self::Icon(icon) => validate_icon(icon.as_deref()),
            Self::Color(color) => validate_color(color),
            Self::Logo(logo) => validate_logo(logo.as_deref()),
        }
    }
    pub fn apply(&self, project: &mut ProjectDescriptor) {
        match self {
            Self::Name(name) => project.name = name.trim().into(),
            Self::Icon(icon) => project.icon.clone_from(icon),
            Self::Color(color) => project.color = color.to_ascii_lowercase(),
            Self::Logo(logo) => project.logo.clone_from(logo),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProjectMutation {
    Create(ProjectDescriptor),
    Patch {
        project: ProjectId,
        patch: ProjectPatch,
    },
    Delete(ProjectId),
}

impl ProjectMutation {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Create(project) if project.home => Err(ErrorCode::BadRequest),
            Self::Create(project) => project.validate(),
            Self::Patch { patch, .. } => patch.validate(),
            Self::Delete(_) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectIntent {
    pub operation: OperationId,
    pub mutation: ProjectMutation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CatalogPage {
    pub server: ServerIdentity,
    pub home: ProjectId,
    pub revision: u64,
    pub projects: Vec<ProjectDescriptor>,
    pub next: Option<ProjectId>,
    pub legacy_home: Option<ProjectId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SessionStatus {
    Starting,
    Live,
    Ended,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectSession {
    pub info: SessionInfo,
    pub status: SessionStatus,
    pub owner: Option<crate::SessionClient>,
    pub attached: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectSessions {
    pub revision: u64,
    pub sessions: Vec<ProjectSession>,
    pub next: Option<SessionId>,
}

impl CatalogPage {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.projects.len() > CATALOG_PAGE_SIZE
            || self
                .projects
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
            || self.next.is_some_and(|next| {
                self.projects
                    .last()
                    .is_none_or(|project| project.id != next)
            })
            || self
                .projects
                .iter()
                .any(|project| project.home != (project.id == self.home))
        {
            return Err(ErrorCode::BadRequest);
        }
        for project in &self.projects {
            project.validate()?;
        }
        Ok(())
    }
}

impl ProjectSessions {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.sessions.len() > CATALOG_PAGE_SIZE
            || self
                .sessions
                .windows(2)
                .any(|pair| pair[0].info.id >= pair[1].info.id)
            || self.next.is_some_and(|next| {
                self.sessions
                    .last()
                    .is_none_or(|session| session.info.id != next)
            })
        {
            return Err(ErrorCode::BadRequest);
        }
        for session in &self.sessions {
            crate::validate_path(&session.info.directory)?;
        }
        Ok(())
    }
}
