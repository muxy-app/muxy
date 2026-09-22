use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use muxy_app_core::{Branch, Direction, Layout, PaneId};
use muxy_protocol::{CatalogPage, OperationId, ProjectId, ServerIdentity, ServerPath, SessionId};
use serde::{Deserialize, Serialize};

pub(crate) type Result<T = ()> = std::result::Result<T, String>;
pub(crate) const MAX_PANES: usize = 256;
const MAX_TABS: usize = 64;
const MAX_SPLIT_PANES: usize = 16;
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
    pub(crate) fn load(profile: &Path, catalog: &CatalogPage) -> Result<Self> {
        let path = profile.join("tui-state.json");
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

    #[test]
    fn automatic_close_overflow_survives_relaunch_and_retries_when_cleanup_has_space() -> Result {
        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let catalog = catalog();
        let mut store = Store::load(directory.path(), &catalog)?;
        let sessions: BTreeSet<_> = [10_000, 10_001]
            .into_iter()
            .map(|id| SessionId::new(id).ok_or_else(|| "session".to_string()))
            .collect::<Result<_>>()?;
        store.change(|state| {
            state.reconcile(&catalog)?;
            let mut sessions = sessions.iter().copied();
            let first = state.tab().ok_or("tab")?.focus;
            let pane = state.pane_mut(first).ok_or("pane")?;
            pane.session = sessions.next();
            pane.creation = None;
            state.new_pane(None, ServerPath(b"/tmp".to_vec()), sessions.next())?;
            state.discards = (1..MAX_PANES)
                .map(|id| {
                    SessionId::new(id as u64)
                        .map(Discard::Session)
                        .ok_or_else(|| "session".to_string())
                })
                .collect::<Result<_>>()?;
            Ok(())
        })?;
        let closed = store.change(|state| Ok(state.close_ended_sessions(&sessions)))?;
        assert_eq!(closed.len(), 1);
        assert_eq!(store.state.discards.len(), MAX_PANES);
        let pending: BTreeSet<_> = sessions.difference(&closed).copied().collect();
        let mut visible = store.state.clone();
        visible.close_sessions(&pending);
        assert!(visible.tab().is_none());

        let mut restored = Store::load(directory.path(), &catalog)?;
        assert_eq!(
            restored.state.session_references(),
            pending.iter().copied().collect::<Vec<_>>()
        );
        assert!(
            restored
                .change(|state| Ok(state.close_ended_sessions(&pending)))?
                .is_empty()
        );
        restored.change(|state| {
            state.discards.remove(0);
            Ok(())
        })?;
        assert_eq!(
            restored.change(|state| Ok(state.close_ended_sessions(&pending)))?,
            pending
        );
        let saved = Store::load(directory.path(), &catalog)?;
        assert!(saved.state.tab().is_none());
        assert_eq!(saved.state.discards.len(), MAX_PANES);
        for session in sessions {
            assert!(saved.state.discards.contains(&Discard::Session(session)));
            assert!(saved.state.close_operations.contains_key(&session));
        }
        Ok(())
    }

    #[test]
    fn dead_sessions_close_across_projects_without_changing_live_focus() -> Result {
        let catalog = catalog();
        let mut state = State::new(&catalog);
        state.reconcile(&catalog)?;
        let directory = ServerPath(b"/tmp".to_vec());
        let dead = SessionId::new(1).ok_or("session")?;
        let live = SessionId::new(2).ok_or("session")?;
        let first = state.tab().ok_or("tab")?.focus;
        let pane = state.pane_mut(first).ok_or("pane")?;
        pane.session = Some(dead);
        pane.creation = None;
        state.new_pane(None, directory.clone(), Some(dead))?;
        let focused = state.new_pane(Some(Direction::Right), directory.clone(), Some(live))?;
        state.tab_mut().ok_or("tab")?.zoom = true;
        let hidden = ProjectId::new();
        state.projects.insert(hidden, Project::default());
        state.active = hidden;
        state.new_pane(None, directory, Some(dead))?;
        state.active = catalog.home;
        state.close_sessions(&BTreeSet::from([dead]));
        assert_eq!(state.active, catalog.home);
        assert!(state.projects[&hidden].tabs.is_empty());
        assert_eq!(state.projects[&catalog.home].tabs.len(), 1);
        assert_eq!(state.tab().ok_or("tab")?.focus, focused);
        assert!(state.tab().ok_or("tab")?.zoom);
        assert_eq!(state.tab().ok_or("tab")?.layout.leaves(), vec![focused]);
        state.validate()?;
        state.close_sessions(&BTreeSet::from([live]));
        assert!(state.tab().is_none());
        state.validate()
    }

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
    fn first_launch_token_survives_relaunch_and_an_explicitly_empty_home_stays_empty() -> Result {
        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let catalog = catalog();
        let mut store = Store::load(directory.path(), &catalog)?;
        store.change(|state| state.reconcile(&catalog))?;
        let first = store.state.tab().ok_or("tab")?.focus;
        let token = store.state.tab().ok_or("tab")?.panes[&first].creation;
        let mut restored = Store::load(directory.path(), &catalog)?;
        restored.change(|state| state.reconcile(&catalog))?;
        assert_eq!(store.state, restored.state);
        restored.change(|state| state.close(first))?;
        assert_eq!(
            restored.state.discards,
            token.map(Discard::Creation).into_iter().collect::<Vec<_>>()
        );
        restored.change(|state| {
            state.discards.clear();
            Ok(())
        })?;
        let mut again = Store::load(directory.path(), &catalog)?;
        again.change(|state| state.reconcile(&catalog))?;
        assert!(again.state.tab().is_none());
        assert!(again.state.discards.is_empty());
        Ok(())
    }

    #[test]
    fn failed_close_save_retains_the_pane_and_failed_ack_save_retains_the_intent() -> Result {
        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let catalog = catalog();
        let mut store = Store::load(directory.path(), &catalog)?;
        store.change(|state| state.reconcile(&catalog))?;
        let id = store.state.tab().ok_or("tab")?.focus;
        let before = store.state.clone();
        let path = directory.path().join("tui-state.json");
        let backup = directory.path().join("saved.json");
        let block = || -> Result {
            fs::rename(&path, &backup).map_err(|error| error.to_string())?;
            fs::create_dir(&path).map_err(|error| error.to_string())
        };
        let unblock = || -> Result {
            fs::remove_dir(&path).map_err(|error| error.to_string())?;
            fs::rename(&backup, &path).map_err(|error| error.to_string())
        };
        block()?;
        assert!(store.change(|state| state.close(id)).is_err());
        assert_eq!(store.state, before);
        assert!(store.ready().is_err());
        unblock()?;
        let mut store = Store::load(directory.path(), &catalog)?;
        store.change(|state| state.close(id))?;
        let closed = store.state.clone();
        assert!(!closed.discards.is_empty());
        block()?;
        assert!(
            store
                .change(|state| {
                    state.discards.remove(0);
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(store.state, closed);
        unblock()?;
        assert_eq!(Store::load(directory.path(), &catalog)?.state, closed);
        Ok(())
    }

    #[test]
    fn split_focus_resize_zoom_and_close_preserve_valid_layouts() -> Result {
        let catalog = catalog();
        let mut state = State::new(&catalog);
        state.reconcile(&catalog)?;
        let left = state.tab().ok_or("tab")?.focus;
        let right = state.new_pane(Some(Direction::Right), ServerPath(b"/tmp".to_vec()), None)?;
        state.focus(Direction::Left);
        assert_eq!(state.tab().ok_or("tab")?.focus, left);
        state.resize(Direction::Right)?;
        assert!(
            matches!(&state.tab().ok_or("tab")?.layout, Layout::Split { ratio, .. } if *ratio > 0.5)
        );
        state.tab_mut().ok_or("tab")?.zoom = true;
        state.close(right)?;
        assert!(!state.tab().ok_or("tab")?.zoom);
        assert_eq!(state.tab().ok_or("tab")?.layout.leaves(), vec![left]);
        state.validate()
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
