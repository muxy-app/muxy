use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use muxy_app_core::{Branch, Direction, Layout, PaneId};
use muxy_protocol::{CatalogPage, OperationId, ProjectId, ServerIdentity, ServerPath, SessionId};
use serde::{Deserialize, Serialize};

pub(crate) type Result<T = ()> = std::result::Result<T, String>;
pub(crate) const MAX_PANES: usize = 256;
const MAX_TABS: usize = 64;
const MAX_SPLIT_PANES: usize = 16;
const MAX_TITLE_CHARS: usize = 64;
const MAX_STATE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct State {
    version: u8,
    #[serde(default)]
    pub catalog_revision: u64,
    #[serde(default)]
    pub layout_id: Option<OperationId>,
    #[serde(default)]
    pub reference_revision: u64,
    pub server: ServerIdentity,
    pub active: ProjectId,
    pub projects: BTreeMap<ProjectId, Project>,
    pub discards: Vec<Discard>,
    #[serde(default)]
    pub close_operations: BTreeMap<SessionId, OperationId>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Project {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Tab {
    pub layout: Layout,
    pub focus: PaneId,
    pub zoom: bool,
    pub panes: BTreeMap<PaneId, Pane>,
    /// A name given in place of the terminal's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct Pane {
    pub session: Option<SessionId>,
    pub creation: Option<OperationId>,
    pub directory: ServerPath,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) enum Discard {
    Session(SessionId),
    Creation(OperationId),
}

impl State {
    pub(crate) fn new(catalog: &CatalogPage) -> Self {
        Self {
            version: 1,
            catalog_revision: 0,
            layout_id: Some(OperationId::new()),
            reference_revision: 0,
            server: catalog.server,
            active: catalog.home,
            projects: BTreeMap::new(),
            discards: Vec::new(),
            close_operations: BTreeMap::new(),
        }
    }

    pub(crate) fn reconcile(&mut self, catalog: &CatalogPage) -> Result {
        if self.server != catalog.server {
            return Err("This TUI layout belongs to a different server. Move tui-state.json aside to start a new layout.".into());
        }
        if catalog.revision < self.catalog_revision {
            return Ok(());
        }
        self.catalog_revision = catalog.revision;
        self.projects
            .retain(|id, _| catalog.projects.iter().any(|project| project.id == *id));
        if !catalog
            .projects
            .iter()
            .any(|project| project.id == self.active)
        {
            self.active = catalog.home;
        }
        self.open(self.active, catalog)
    }

    pub(crate) fn open(&mut self, id: ProjectId, catalog: &CatalogPage) -> Result {
        let project = catalog
            .projects
            .iter()
            .find(|project| project.id == id)
            .ok_or("Project is no longer available")?;
        self.active = id;
        if let std::collections::btree_map::Entry::Vacant(entry) = self.projects.entry(id) {
            entry.insert(Project::default());
            self.new_pane(None, project.directory.clone(), None)?;
        }
        Ok(())
    }

    pub(crate) fn tab(&self) -> Option<&Tab> {
        let project = self.projects.get(&self.active)?;
        project.tabs.get(project.active)
    }

    pub(crate) fn tab_mut(&mut self) -> Option<&mut Tab> {
        let project = self.projects.get_mut(&self.active)?;
        project.tabs.get_mut(project.active)
    }

    pub(crate) fn selection(&self) -> (ProjectId, Option<PaneId>) {
        (self.active, self.tab().map(|tab| tab.focus))
    }

    pub(crate) fn tab_selection(&self, index: usize) -> Option<(ProjectId, Option<PaneId>)> {
        Some((
            self.active,
            Some(self.projects.get(&self.active)?.tabs.get(index)?.focus),
        ))
    }

    pub(crate) fn cycle_selection(&self, forward: bool) -> Option<(ProjectId, Option<PaneId>)> {
        let project = self.projects.get(&self.active)?;
        let count = project.tabs.len();
        if count == 0 {
            return None;
        }
        self.tab_selection((project.active + if forward { 1 } else { count - 1 }) % count)
    }

    pub(crate) fn select(&mut self, (id, pane): (ProjectId, Option<PaneId>)) -> Result {
        let project = self
            .projects
            .get_mut(&id)
            .ok_or("Project is no longer available")?;
        if let Some(pane) = pane {
            let index = project
                .tabs
                .iter()
                .position(|tab| tab.panes.contains_key(&pane))
                .ok_or("Pane was closed in another instance")?;
            project.active = index;
            project.tabs[index].focus = pane;
        }
        self.active = id;
        Ok(())
    }

    pub(crate) fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.projects
            .values_mut()
            .flat_map(|project| &mut project.tabs)
            .find_map(|tab| tab.panes.get_mut(&id))
    }

    pub(crate) fn new_pane(
        &mut self,
        split: Option<Direction>,
        directory: ServerPath,
        session: Option<SessionId>,
    ) -> Result<PaneId> {
        if self
            .projects
            .values()
            .flat_map(|project| &project.tabs)
            .map(|tab| tab.panes.len())
            .sum::<usize>()
            >= MAX_PANES
        {
            return Err("TUI pane limit reached".into());
        }
        let id = PaneId::new();
        let pane = Pane {
            session,
            creation: session.is_none().then(|| id.creation_token()),
            directory,
            error: None,
        };
        let project = self
            .projects
            .get_mut(&self.active)
            .ok_or("No active project")?;
        if let Some(direction) = split {
            let tab = project
                .tabs
                .get_mut(project.active)
                .ok_or("Create a tab before splitting")?;
            if tab.panes.len() >= MAX_SPLIT_PANES {
                return Err("Split pane limit reached".into());
            }
            tab.layout.split(tab.focus, id, direction);
            tab.panes.insert(id, pane);
            tab.focus = id;
            tab.zoom = false;
        } else {
            if project.tabs.len() >= MAX_TABS {
                return Err("Tab limit reached".into());
            }
            project.tabs.push(Tab {
                layout: Layout::Leaf(id),
                focus: id,
                zoom: false,
                panes: BTreeMap::from([(id, pane)]),
                title: None,
            });
            project.active = project.tabs.len() - 1;
        }
        Ok(id)
    }

    pub(crate) fn close(&mut self, id: PaneId) -> Result {
        if self.discards.len() >= MAX_PANES {
            return Err("Pending terminal closures must finish before closing another pane".into());
        }
        let tab = self
            .tab_mut()
            .filter(|tab| tab.panes.contains_key(&id))
            .ok_or("Pane is no longer active")?;
        let pane = tab.panes.remove(&id).ok_or("Pane is no longer available")?;
        tab.zoom = false;
        if let Some(next) = tab.panes.keys().next().copied() {
            tab.layout.remove(id);
            if tab.focus == id {
                tab.focus = next;
            }
        } else if let Some(project) = self.projects.get_mut(&self.active) {
            project.tabs.remove(project.active);
            project.active = project.active.min(project.tabs.len().saturating_sub(1));
        }
        let discard = pane
            .session
            .map(Discard::Session)
            .or_else(|| pane.creation.map(Discard::Creation));
        if let Some(discard) = discard
            && !self.discards.contains(&discard)
            && !matches!(discard, Discard::Session(session) if self.session_references().contains(&session))
        {
            self.discards.push(discard);
        }
        Ok(())
    }

    /// Closes every pane of the current project's tab that holds `pane`.
    pub(crate) fn close_tab(&mut self, pane: PaneId) -> Result {
        let index = self.tab_index(pane)?;
        let project = self
            .projects
            .get_mut(&self.active)
            .ok_or("No active project")?;
        project.active = index;
        let panes: Vec<_> = project.tabs[index].panes.keys().copied().collect();
        for id in panes {
            self.close(id)?;
        }
        Ok(())
    }

    /// Moves the tab holding `pane` one place earlier or later.
    pub(crate) fn move_tab(&mut self, pane: PaneId, forward: bool) -> Result {
        let index = self.tab_index(pane)?;
        let project = self
            .projects
            .get_mut(&self.active)
            .ok_or("No active project")?;
        let Some(other) = (if forward {
            index
                .checked_add(1)
                .filter(|other| *other < project.tabs.len())
        } else {
            index.checked_sub(1)
        }) else {
            return Ok(());
        };
        project.tabs.swap(index, other);
        if project.active == index {
            project.active = other;
        } else if project.active == other {
            project.active = index;
        }
        Ok(())
    }

    /// Names the tab holding `pane`; no name shows its terminal's title.
    pub(crate) fn rename_tab(&mut self, pane: PaneId, title: Option<String>) -> Result {
        let index = self.tab_index(pane)?;
        let title = title
            .map(|title| {
                title
                    .chars()
                    .filter(|character| !character.is_control())
                    .take(MAX_TITLE_CHARS)
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .filter(|title| !title.is_empty());
        if let Some(project) = self.projects.get_mut(&self.active) {
            project.tabs[index].title = title;
        }
        Ok(())
    }

    fn tab_index(&self, pane: PaneId) -> Result<usize> {
        self.projects
            .get(&self.active)
            .and_then(|project| {
                project
                    .tabs
                    .iter()
                    .position(|tab| tab.panes.contains_key(&pane))
            })
            .ok_or_else(|| "Tab was closed in another instance".into())
    }

    pub(crate) fn session_references(&self) -> Vec<SessionId> {
        self.projects
            .values()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| tab.panes.values())
            .filter_map(|pane| pane.session)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(crate) fn close_sessions(&mut self, sessions: &BTreeSet<SessionId>) {
        for project in self.projects.values_mut() {
            for index in (0..project.tabs.len()).rev() {
                let tab = &mut project.tabs[index];
                let removed: Vec<_> = tab
                    .panes
                    .iter()
                    .filter(|(_, pane)| {
                        pane.session
                            .is_some_and(|session| sessions.contains(&session))
                    })
                    .map(|(id, _)| *id)
                    .collect();
                for id in removed {
                    let neighbor = [
                        Direction::Right,
                        Direction::Left,
                        Direction::Down,
                        Direction::Up,
                    ]
                    .into_iter()
                    .find_map(|direction| tab.layout.neighbor(id, direction));
                    tab.panes.remove(&id);
                    tab.layout.remove(id);
                    if tab.focus == id {
                        tab.focus = neighbor
                            .or_else(|| tab.panes.keys().next().copied())
                            .unwrap_or(id);
                        tab.zoom = false;
                    }
                }
                if tab.panes.is_empty() {
                    project.tabs.remove(index);
                    if index < project.active {
                        project.active -= 1;
                    }
                }
            }
            project.active = project.active.min(project.tabs.len().saturating_sub(1));
        }
    }

    pub(crate) fn close_ended_sessions(
        &mut self,
        ended: &BTreeSet<SessionId>,
    ) -> BTreeSet<SessionId> {
        let references: BTreeSet<_> = self.session_references().into_iter().collect();
        let mut closed = BTreeSet::new();
        for session in ended {
            let discard = Discard::Session(*session);
            if !references.contains(session) || self.discards.contains(&discard) {
                closed.insert(*session);
            } else if self.discards.len() < MAX_PANES {
                self.discards.push(discard);
                closed.insert(*session);
            }
        }
        self.close_sessions(&closed);
        closed
    }

    fn prepare_closes(&mut self) {
        self.close_operations
            .retain(|session, _| self.discards.contains(&Discard::Session(*session)));
        for discard in &self.discards {
            if let Discard::Session(session) = discard {
                self.close_operations.entry(*session).or_default();
            }
        }
    }

    pub(crate) fn focus(&mut self, direction: Direction) {
        if let Some(tab) = self.tab_mut()
            && let Some(next) = tab.layout.neighbor(tab.focus, direction)
        {
            tab.focus = next;
        }
    }

    pub(crate) fn cycle_pane(&mut self, forward: bool) {
        if let Some(tab) = self.tab_mut() {
            let leaves = tab.layout.leaves();
            if let Some(index) = leaves.iter().position(|id| *id == tab.focus) {
                let count = leaves.len();
                tab.focus = leaves[(index + if forward { 1 } else { count - 1 }) % count];
            }
        }
    }

    pub(crate) fn resize(&mut self, direction: Direction) -> Result {
        let tab = self.tab_mut().ok_or("No pane to resize")?;
        let Some((path, ratio)) = resize_path(&tab.layout, tab.focus, direction, &mut Vec::new())
        else {
            return Ok(());
        };
        tab.layout
            .set_ratio(&path, ratio)
            .map_err(|error| error.to_string())
    }

    /// Sets the split that divides `first` from `second`, as a mouse drag
    /// does. Naming panes instead of a path keeps it right if another
    /// instance changed the layout meanwhile.
    pub(crate) fn set_ratio(&mut self, first: PaneId, second: PaneId, ratio: f32) -> Result {
        let tab = self.tab_mut().ok_or("No pane to resize")?;
        let path = split_between(&tab.layout, first, second, &mut Vec::new())
            .ok_or("Split is no longer available")?;
        tab.layout
            .set_ratio(&path, ratio)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn validate(&self) -> Result {
        if self.version != 1
            || self.projects.len() > muxy_protocol::MAX_PROJECTS
            || self.discards.len() > MAX_PANES
        {
            return Err("Invalid TUI state version or size".into());
        }
        let mut ids = BTreeSet::new();
        let mut sessions = BTreeSet::new();
        let mut creations = BTreeSet::new();
        for project in self.projects.values() {
            if project.tabs.len() > MAX_TABS || project.active >= project.tabs.len().max(1) {
                return Err("Invalid TUI tabs".into());
            }
            for tab in &project.tabs {
                if tab.title.as_ref().is_some_and(|title| {
                    title.is_empty()
                        || title.chars().count() > MAX_TITLE_CHARS
                        || title.chars().any(char::is_control)
                }) {
                    return Err("Invalid TUI tab title".into());
                }
                let leaves = tab.layout.leaves();
                if leaves.len() > MAX_SPLIT_PANES
                    || tab.panes.len() != leaves.len()
                    || !tab.panes.contains_key(&tab.focus)
                    || !leaves
                        .iter()
                        .all(|id| ids.insert(*id) && tab.panes.contains_key(id))
                {
                    return Err("Invalid TUI pane layout".into());
                }
                tab.layout.validate().map_err(|error| error.to_string())?;
                for pane in tab.panes.values() {
                    sessions.extend(pane.session);
                    if pane.session.is_some() && pane.creation.is_some()
                        || pane.creation.is_some_and(|id| !creations.insert(id))
                        || (pane.session.is_none()
                            && pane.creation.is_none()
                            && pane.error.is_none())
                        || pane.error.as_ref().is_some_and(|error| error.len() > 4096)
                    {
                        return Err("Invalid TUI session reference".into());
                    }
                    muxy_protocol::validate_path(&pane.directory)
                        .map_err(|error| format!("Invalid TUI directory: {error:?}"))?;
                }
            }
        }
        if ids.len() > MAX_PANES {
            return Err("TUI pane limit exceeded".into());
        }
        for (index, discard) in self.discards.iter().enumerate() {
            let visible = match discard {
                Discard::Session(id) => sessions.contains(id),
                Discard::Creation(id) => creations.contains(id),
            };
            if visible || self.discards[..index].contains(discard) {
                return Err("Invalid pending TUI closure".into());
            }
        }
        Ok(())
    }
}

fn resize_path(
    layout: &Layout,
    pane: PaneId,
    direction: Direction,
    path: &mut Vec<Branch>,
) -> Option<(Vec<Branch>, f32)> {
    let Layout::Split {
        axis,
        ratio,
        first,
        second,
    } = layout
    else {
        return None;
    };
    let (branch, child) = if first.contains(pane) {
        (Branch::First, first)
    } else {
        (Branch::Second, second)
    };
    path.push(branch);
    let nested = resize_path(child, pane, direction, path);
    path.pop();
    nested.or_else(|| {
        (*axis == direction.axis()).then(|| {
            (
                path.clone(),
                ratio
                    + if matches!(direction, Direction::Left | Direction::Up) {
                        -0.05
                    } else {
                        0.05
                    },
            )
        })
    })
}

fn split_between(
    layout: &Layout,
    first: PaneId,
    second: PaneId,
    path: &mut Vec<Branch>,
) -> Option<Vec<Branch>> {
    let Layout::Split {
        first: a,
        second: b,
        ..
    } = layout
    else {
        return None;
    };
    if a.contains(first) && b.contains(second) {
        return Some(path.clone());
    }
    let (branch, child) = if a.contains(first) {
        (Branch::First, a)
    } else {
        (Branch::Second, b)
    };
    path.push(branch);
    let found = split_between(child, first, second, path);
    path.pop();
    found
}

pub(crate) struct Store {
    path: PathBuf,
    pub state: State,
    blocked: bool,
    saved: bool,
}

impl Store {
    pub(crate) fn ready(&self) -> Result {
        if self.blocked {
            Err("TUI storage needs repair before further session actions".into())
        } else {
            Ok(())
        }
    }
    /// Loads the layout kept in `directory`, creating the folder, private to
    /// this user, if it is missing.
    pub(crate) fn load(directory: &Path, catalog: &CatalogPage) -> Result<Self> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|error| format!("Cannot create {}: {error}", directory.display()))?;
        let path = directory.join("tui-state.json");
        let _guard = state_lock(&path)?;
        let saved = read_state(&path)?;
        Ok(Self {
            path,
            saved: saved.is_some(),
            state: saved.unwrap_or_else(|| State::new(catalog)),
            blocked: false,
        })
    }

    fn read_latest(&mut self) -> Result {
        let latest = match read_state(&self.path) {
            Ok(Some(state)) => state,
            Ok(None) if !self.saved => return Ok(()),
            Ok(None) => {
                self.blocked = true;
                return Err(
                    "The shared TUI state disappeared; restore it before continuing".into(),
                );
            }
            Err(error) => {
                self.blocked = true;
                return Err(error);
            }
        };
        if latest.server != self.state.server {
            self.blocked = true;
            return Err("The shared TUI state belongs to another server".into());
        }
        self.state = latest;
        self.saved = true;
        Ok(())
    }

    pub(crate) fn change<T>(&mut self, change: impl FnOnce(&mut State) -> Result<T>) -> Result<T> {
        if self.blocked {
            return Err("TUI state could not be restored after a failed save. Detach and repair its storage before continuing.".into());
        }
        let _guard = state_lock(&self.path)?;
        self.read_latest()?;
        let mut next = self.state.clone();
        let result = change(&mut next)?;
        next.layout_id.get_or_insert_default();
        if next.session_references() != self.state.session_references() {
            next.reference_revision = next
                .reference_revision
                .checked_add(1)
                .ok_or("TUI reference revisions exhausted")?;
        }
        next.prepare_closes();
        next.validate()?;
        if next != self.state || !self.path.exists() {
            if let Err(error) = persist(&self.path, &next) {
                // A directory sync can fail after rename. Restore the old document
                // before allowing any further mutation or server-side effect.
                if persist(&self.path, &self.state).is_err() {
                    self.blocked = true;
                }
                return Err(format!("Could not save TUI state: {error}"));
            }
            self.state = next;
            self.saved = true;
        }
        Ok(result)
    }
}

fn state_lock(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path.with_file_name("tui.lock"))
        .map_err(|error| error.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(error) if std::time::Instant::now() >= deadline => {
                return Err(format!("Shared TUI state is busy: {error}"));
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
        }
    }
}

fn read_state(path: &Path) -> Result<Option<State>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("TUI state must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_STATE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err("TUI state file is too large".into());
    }
    let state: State = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Cannot read tui-state.json: {error}"))?;
    state.validate()?;
    Ok(Some(state))
}

fn persist(path: &Path, state: &State) -> io::Result<()> {
    let bytes = serde_json::to_vec(state)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(io::Error::other("TUI state file is too large"));
    }
    let temporary = path.with_file_name(format!(".tui-state-{}.tmp", OperationId::new()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    let result = file
        .write_all(&bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&temporary, path))
        .and_then(|()| File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all());
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::ProjectDescriptor;

    fn catalog() -> CatalogPage {
        let home = ProjectId::new();
        CatalogPage {
            server: ServerIdentity::new(),
            home,
            revision: 0,
            next: None,
            legacy_home: None,
            projects: vec![ProjectDescriptor {
                id: home,
                home: true,
                name: "Home".into(),
                icon: None,
                logo: None,
                color: "#808080".into(),
                directory: ServerPath(b"/tmp".to_vec()),
                kind: None,
                parent_id: None,
            }],
        }
    }

    #[test]
    fn invalid_state_is_reported_without_overwriting_it() -> Result {
        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let path = directory.path().join("tui-state.json");
        fs::write(&path, b"not json").map_err(|error| error.to_string())?;
        assert!(Store::load(directory.path(), &catalog()).is_err());
        assert_eq!(
            fs::read(&path).map_err(|error| error.to_string())?,
            b"not json"
        );
        Ok(())
    }

    #[test]
    fn stale_instances_merge_changes_and_retries_do_not_remove_another_close_intent() -> Result {
        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let catalog = catalog();
        let mut first = Store::load(directory.path(), &catalog)?;
        let mut second = Store::load(directory.path(), &catalog)?;
        first.change(|state| state.reconcile(&catalog))?;
        second.change(|state| state.reconcile(&catalog))?;
        assert_eq!(first.state, second.state);
        let original = first.state.selection();
        let added =
            first.change(|state| state.new_pane(None, ServerPath(b"/tmp".to_vec()), None))?;
        let split = second.change(|state| {
            state.select(original)?;
            state.new_pane(Some(Direction::Right), ServerPath(b"/tmp".to_vec()), None)
        })?;
        let mut saved = Store::load(directory.path(), &catalog)?;
        assert_eq!(saved.state.projects[&catalog.home].tabs.len(), 2);
        assert_eq!(saved.state.tab().ok_or("tab")?.panes.len(), 2);
        first.change(|state| {
            state.select((catalog.home, Some(added)))?;
            state.close(added)
        })?;
        second.change(|state| {
            state.select((catalog.home, Some(split)))?;
            state.close(split)
        })?;
        saved.change(|_| Ok(()))?;
        let first_discard = saved.state.discards[0];
        let second_discard = saved.state.discards[1];
        for store in [&mut first, &mut second] {
            store.change(|state| {
                state.discards.retain(|pending| *pending != first_discard);
                Ok(())
            })?;
        }
        saved.change(|_| Ok(()))?;
        assert_eq!(saved.state.discards, vec![second_discard]);
        Ok(())
    }
}
