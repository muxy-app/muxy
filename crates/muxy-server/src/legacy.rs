//! One-time input adapter; presentation stays in the desktop's original file.
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use muxy_protocol::{ProjectDescriptor, ProjectId, ProjectKind, ServerPath, SessionId};
use muxy_server::LegacyImport;
use serde::Deserialize;

#[cfg(test)]
mod tests;

#[derive(Deserialize)]
struct Snapshot {
    version: u32,
    projects: Vec<Project>,
    #[serde(default)]
    quick_terminal: Option<Pane>,
    #[serde(default)]
    pending_discards: Vec<SessionId>,
}

#[derive(Deserialize)]
struct Project {
    id: ProjectId,
    server_id: ProjectId,
    #[serde(default)]
    home: bool,
    name: String,
    icon: Option<String>,
    color: String,
    directory: Directory,
    kind: Option<ProjectKind>,
    parent_id: Option<ProjectId>,
    tabs: Vec<Tab>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Directory {
    Text(String),
    Bytes(Vec<u8>),
}
impl Directory {
    fn path(self) -> ServerPath {
        ServerPath(match self {
            Self::Text(text) => text.into_bytes(),
            Self::Bytes(bytes) => bytes,
        })
    }
}

#[derive(Deserialize)]
struct Tab {
    panes: Vec<Pane>,
}
#[derive(Deserialize)]
struct Pane {
    id: ProjectId,
    content: Content,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Content {
    Terminal { session: Option<SessionId> },
    Settings,
}

pub(crate) fn read(profile: &Path) -> io::Result<LegacyImport> {
    // Every committed catalog is an import receipt, including an empty-profile import.
    if profile.join("sessions/catalog.json").try_exists()? {
        return Ok(LegacyImport::default());
    }
    let file = match File::open(profile.join("state.json")) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(LegacyImport::default()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take(32 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(io::Error::other(
            "legacy state exceeds import limit; original state preserved",
        ));
    }
    parse(&bytes)
}

fn parse(bytes: &[u8]) -> io::Result<LegacyImport> {
    let snapshot: Snapshot = serde_json::from_slice(bytes)?;
    if snapshot.version != 1 {
        return Err(io::Error::other("unsupported legacy desktop state version"));
    }
    let home = snapshot
        .projects
        .iter()
        .find(|project| project.home)
        .or_else(|| {
            snapshot.projects.iter().find(|project| {
                project.name == "Home" && project.kind.is_none() && project.parent_id.is_none()
            })
        })
        .map(|project| project.id);
    let mut result = LegacyImport::default();
    let mut panes = HashSet::new();
    for project in snapshot.projects {
        if project.server_id != ProjectId::from_u128(0x9eed_d633_57b5_4359_b76e_762c_6bc2_775c) {
            return Err(io::Error::other(
                "legacy project selects an unsupported server; original state preserved",
            ));
        }
        for pane in project.tabs.into_iter().flat_map(|tab| tab.panes) {
            import_pane(&pane, project.id, &mut result.sessions, &mut panes)?;
        }
        result.projects.push(ProjectDescriptor {
            id: project.id,
            home: project.home || Some(project.id) == home,
            name: project.name,
            icon: project.icon,
            logo: None,
            color: project.color,
            directory: project.directory.path(),
            kind: project.kind,
            parent_id: project.parent_id,
        });
    }
    if let Some(pane) = snapshot.quick_terminal {
        let owner = ensure_home(&mut result, home)?;
        import_pane(&pane, owner, &mut result.sessions, &mut panes)?;
    }
    if !snapshot.pending_discards.is_empty() {
        let owner = ensure_home(&mut result, home)?;
        for session in snapshot.pending_discards {
            result.sessions.entry(session).or_insert(owner);
        }
    }
    Ok(result)
}

fn ensure_home(result: &mut LegacyImport, home: Option<ProjectId>) -> io::Result<ProjectId> {
    if let Some(home) = home.or_else(|| {
        result
            .projects
            .iter()
            .find(|project| project.home)
            .map(|project| project.id)
    }) {
        return Ok(home);
    }
    let directory =
        std::env::home_dir().ok_or_else(|| io::Error::other("home directory unavailable"))?;
    let id = ProjectId::new();
    result.projects.push(ProjectDescriptor {
        id,
        home: true,
        name: "Home".into(),
        icon: None,
        logo: None,
        color: "#808080".into(),
        directory: ServerPath(directory.as_os_str().as_bytes().into()),
        kind: None,
        parent_id: None,
    });
    Ok(id)
}

fn import_pane(
    pane: &Pane,
    owner: ProjectId,
    sessions: &mut BTreeMap<SessionId, ProjectId>,
    panes: &mut HashSet<ProjectId>,
) -> io::Result<()> {
    if !panes.insert(pane.id) {
        return Err(io::Error::other(
            "duplicate legacy pane identity; original state preserved",
        ));
    }
    if let Content::Terminal {
        session: Some(session),
    } = pane.content
        && let Some(previous) = sessions.insert(session, owner)
        && previous != owner
    {
        return Err(io::Error::other(
            "legacy session belongs to conflicting projects; original state preserved",
        ));
    }
    Ok(())
}
