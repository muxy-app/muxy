use crate::resources::AppResources;
use muxy_core::environment::BuildMode;
use muxy_core::session::transition::DesiredSessionMode;
use muxy_core::store::Project;
use muxy_core::workspace::TabKind;
use muxy_core::workspace_store::WorkspaceStore;
use muxy_proto::session::{
    CreateSessionRequest, EnvironmentEntry, SessionDescriptor, SessionId, SessionOwner,
    SessionStatus, WindowSize, WorkspacePlacement,
};
use muxy_session::SessionClient;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const STARTUP_CONTROL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppliedSessionMode {
    Ordinary,
    Persistent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartupBarrier {
    Reconciling,
    Ready(AppliedSessionMode),
    Blocked(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkedTabState {
    Present,
    Missing,
    Ended,
    AttachmentFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingSessionLaunch {
    pub directory: PathBuf,
    pub command: Option<muxy_terminal::backend::LaunchCommand>,
}

#[derive(Clone)]
struct LaunchContract {
    socket_path: PathBuf,
    helper_path: PathBuf,
    resources: AppResources,
    shell: String,
    environment: Vec<EnvironmentEntry>,
    build_mode: muxy_proto::session::BuildMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EligibleTab {
    tab_id: String,
    owner: SessionOwner,
    placement: WorkspacePlacement,
    directory: PathBuf,
    command: Option<muxy_terminal::backend::LaunchCommand>,
    title: String,
    linked_session_id: Option<SessionId>,
}

pub struct SessionCoordinator {
    barrier: StartupBarrier,
    desired: DesiredSessionMode,
    mode: BuildMode,
    app_support: PathBuf,
    current_executable: PathBuf,
    main_socket_path: PathBuf,
    contract: Option<LaunchContract>,
    linked_tabs: HashMap<String, LinkedTabState>,
}

impl SessionCoordinator {
    pub fn start(
        mode: BuildMode,
        app_support: &Path,
        current_executable: &Path,
        main_socket_path: &Path,
        desired_persistent: bool,
        projects: &[Project],
        store: &mut WorkspaceStore,
    ) -> Self {
        let desired = if desired_persistent {
            DesiredSessionMode::Persistent
        } else {
            DesiredSessionMode::Ordinary
        };
        let mut coordinator = Self {
            barrier: StartupBarrier::Reconciling,
            desired,
            mode,
            app_support: app_support.to_path_buf(),
            current_executable: current_executable.to_path_buf(),
            main_socket_path: main_socket_path.to_path_buf(),
            contract: None,
            linked_tabs: HashMap::new(),
        };
        coordinator.run_startup(projects, store);
        coordinator
    }

    pub fn barrier(&self) -> &StartupBarrier {
        &self.barrier
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.barrier, StartupBarrier::Ready(_))
    }

    pub fn build_mode(&self) -> BuildMode {
        self.mode
    }

    pub fn applied_mode(&self) -> Option<AppliedSessionMode> {
        match self.barrier {
            StartupBarrier::Ready(mode) => Some(mode),
            StartupBarrier::Reconciling | StartupBarrier::Blocked(_) => None,
        }
    }

    pub fn desired_persistent(&self) -> bool {
        self.desired == DesiredSessionMode::Persistent
    }

    pub fn set_desired_persistent(&mut self, enabled: bool) {
        self.desired = if enabled {
            DesiredSessionMode::Persistent
        } else {
            DesiredSessionMode::Ordinary
        };
    }

    pub fn block(&mut self, error: String) {
        self.barrier = StartupBarrier::Blocked(error);
    }

    pub fn needs_reconcile(&self, projects: &[Project], store: &WorkspaceStore) -> bool {
        self.applied_mode() == Some(AppliedSessionMode::Persistent)
            && eligible_tabs(projects, store, &HashMap::new())
                .iter()
                .any(|tab| {
                    tab.linked_session_id.is_none() || !self.linked_tabs.contains_key(&tab.tab_id)
                })
    }

    pub fn socket_path(&self) -> Option<&Path> {
        self.contract
            .as_ref()
            .map(|contract| contract.socket_path.as_path())
    }

    pub fn helper_path(&self) -> Option<&Path> {
        self.contract
            .as_ref()
            .map(|contract| contract.helper_path.as_path())
    }

    pub fn active_session_count(&self) -> Result<usize, String> {
        let fallback;
        let contract = match self.contract.as_ref() {
            Some(contract) => contract,
            None => {
                let socket_path = runtime_socket_path(self.mode, &self.app_support);
                if !socket_path.exists() {
                    return Ok(0);
                }
                fallback = self.build_contract(socket_path)?;
                &fallback
            }
        };
        let mut control = FacadeControl::connect(contract, false).map_err(display_error)?;
        control
            .list()
            .map(|sessions| {
                sessions
                    .into_iter()
                    .filter(|session| session.status == SessionStatus::Running)
                    .count()
            })
            .map_err(display_error)
    }

    pub fn linked_tab_state(&self, tab_id: &str) -> Option<LinkedTabState> {
        self.linked_tabs.get(tab_id).copied()
    }

    pub fn attachment_command(&self, session_id: SessionId) -> Result<String, String> {
        let contract = self
            .contract
            .as_ref()
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        attachment_command(&contract.helper_path, &contract.socket_path, session_id)
    }

    pub fn retry(
        &mut self,
        desired_persistent: bool,
        projects: &[Project],
        store: &mut WorkspaceStore,
    ) {
        *self = Self::start(
            self.mode,
            &self.app_support,
            &self.current_executable,
            &self.main_socket_path,
            desired_persistent,
            projects,
            store,
        );
    }

    pub fn reconcile_new_sessions(
        &mut self,
        projects: &[Project],
        store: &mut WorkspaceStore,
        launches: &HashMap<String, PendingSessionLaunch>,
    ) -> Result<(), String> {
        if self.applied_mode() != Some(AppliedSessionMode::Persistent) {
            return Ok(());
        }
        let contract = self
            .contract
            .clone()
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        let mut control = FacadeControl::connect(&contract, true).map_err(display_error)?;
        let eligible = eligible_tabs(projects, store, launches);
        let statuses = reconcile_persistent(&mut control, &contract, store, &eligible)
            .map_err(display_error)?;
        self.linked_tabs.extend(statuses);
        Ok(())
    }

    pub fn mark_attachment_failed(&mut self, tab_id: &str) {
        if self.linked_tabs.contains_key(tab_id) {
            self.linked_tabs
                .insert(tab_id.to_owned(), LinkedTabState::AttachmentFailed);
        }
    }

    pub fn refresh_linked_tab(
        &mut self,
        tab_id: &str,
        session_id: SessionId,
    ) -> Result<LinkedTabState, String> {
        self.update_linked_tab(tab_id, session_id, LinkedTabState::AttachmentFailed)
    }

    pub fn retry_attachment(
        &mut self,
        tab_id: &str,
        session_id: SessionId,
    ) -> Result<LinkedTabState, String> {
        self.update_linked_tab(tab_id, session_id, LinkedTabState::Present)
    }

    fn update_linked_tab(
        &mut self,
        tab_id: &str,
        session_id: SessionId,
        running: LinkedTabState,
    ) -> Result<LinkedTabState, String> {
        let contract = self
            .contract
            .as_ref()
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        let mut control = FacadeControl::connect(contract, false).map_err(display_error)?;
        let state = query_linked_tab(&mut control, session_id, running).map_err(display_error)?;
        self.linked_tabs.insert(tab_id.to_owned(), state);
        Ok(state)
    }

    fn run_startup(&mut self, projects: &[Project], store: &mut WorkspaceStore) {
        if !store.session_link_errors().is_empty() {
            self.barrier = StartupBarrier::Blocked(
                "Workspace session links contain an invalid session identifier.".to_owned(),
            );
            return;
        }
        let result = match self.desired {
            DesiredSessionMode::Ordinary => self.start_ordinary(store),
            DesiredSessionMode::Persistent => self.start_persistent(projects, store),
        };
        if let Err(error) = result {
            self.barrier = StartupBarrier::Blocked(error);
        }
    }

    fn start_ordinary(&mut self, store: &mut WorkspaceStore) -> Result<(), String> {
        let socket_path = runtime_socket_path(self.mode, &self.app_support);
        let linked = linked_session_count(store);
        if linked == 0 && !socket_path.exists() {
            self.barrier = StartupBarrier::Ready(AppliedSessionMode::Ordinary);
            return Ok(());
        }
        let contract = self.build_contract(socket_path)?;
        let mut control = FacadeControl::connect(&contract, linked > 0).map_err(display_error)?;
        disable_persistent(&mut control, store).map_err(display_error)?;
        self.contract = Some(contract);
        self.linked_tabs.clear();
        self.barrier = StartupBarrier::Ready(AppliedSessionMode::Ordinary);
        Ok(())
    }

    fn start_persistent(
        &mut self,
        projects: &[Project],
        store: &mut WorkspaceStore,
    ) -> Result<(), String> {
        let socket_path = runtime_socket_path(self.mode, &self.app_support);
        let contract = self.build_contract(socket_path)?;
        let mut control = FacadeControl::connect(&contract, true).map_err(display_error)?;
        let eligible = eligible_tabs(projects, store, &HashMap::new());
        self.linked_tabs = reconcile_persistent(&mut control, &contract, store, &eligible)
            .map_err(display_error)?;
        self.contract = Some(contract);
        self.barrier = StartupBarrier::Ready(AppliedSessionMode::Persistent);
        Ok(())
    }

    fn build_contract(&self, socket_path: PathBuf) -> Result<LaunchContract, String> {
        let helper_path = muxy_session::sibling_helper(&self.current_executable)
            .and_then(std::fs::canonicalize)
            .map_err(display_error)?;
        let resources = AppResources::discover().map_err(|error| error.to_string())?;
        let shell = muxy_terminal::backend::user_shell();
        let shell = std::fs::canonicalize(&shell)
            .unwrap_or_else(|_| PathBuf::from(&shell))
            .to_str()
            .ok_or_else(|| "user shell path is not UTF-8".to_owned())?
            .to_owned();
        let mut environment = process_environment();
        set_environment(
            &mut environment,
            "MUXY_SOCKET_PATH",
            self.main_socket_path
                .to_str()
                .ok_or_else(|| "Muxy socket path is not UTF-8".to_owned())?,
        );
        Ok(LaunchContract {
            socket_path,
            helper_path,
            resources,
            shell,
            environment,
            build_mode: protocol_build_mode(self.mode),
        })
    }
}

fn protocol_build_mode(mode: BuildMode) -> muxy_proto::session::BuildMode {
    if mode.is_development() {
        muxy_proto::session::BuildMode::Development
    } else {
        muxy_proto::session::BuildMode::Production
    }
}

fn runtime_socket_path(mode: BuildMode, app_support: &Path) -> PathBuf {
    muxy_session::selected_socket_path(mode, app_support, std::env::temp_dir())
}

fn attachment_command(
    helper_path: &Path,
    socket_path: &Path,
    session_id: SessionId,
) -> Result<String, String> {
    let helper = path_argument(helper_path)?;
    let socket = path_argument(socket_path)?;
    Ok(format!(
        "{} attach --socket {} --session-id {}",
        muxy_terminal::backend::shell_escape(helper),
        muxy_terminal::backend::shell_escape(socket),
        session_id.uppercase()
    ))
}

fn path_argument(path: &Path) -> Result<&str, String> {
    let value = path
        .to_str()
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))?;
    if value.contains('\0') {
        return Err("path contains NUL".to_owned());
    }
    Ok(value)
}

fn process_environment() -> Vec<EnvironmentEntry> {
    let mut entries = std::env::vars()
        .filter(|(key, value)| {
            !key.is_empty()
                && !key.contains('=')
                && key.len() <= muxy_proto::session::MAX_ENVIRONMENT_KEY_BYTES
                && value.len() <= muxy_proto::session::MAX_VALUE_BYTES
                && !key.starts_with("MUXY_SESSION_")
        })
        .map(|(key, value)| EnvironmentEntry { key, value })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.key.cmp(&right.key));
    entries.truncate(muxy_proto::session::MAX_ENVIRONMENT_ENTRIES);
    entries
}

fn set_environment(entries: &mut Vec<EnvironmentEntry>, key: &str, value: &str) {
    entries.retain(|entry| entry.key != key);
    entries.push(EnvironmentEntry {
        key: key.to_owned(),
        value: value.to_owned(),
    });
}

fn linked_session_count(store: &WorkspaceStore) -> usize {
    store
        .states()
        .iter()
        .flat_map(|workspace| workspace.root.iter().flat_map(|root| root.tabs()))
        .filter(|tab| tab.kind == TabKind::Terminal && tab.session_id.is_some())
        .count()
}

fn disable_persistent<C: Control>(control: &mut C, store: &mut WorkspaceStore) -> io::Result<()> {
    control.end_all()?;
    let mut updated = store.clone();
    clear_session_links(&mut updated);
    updated.save()?;
    *store = updated;
    Ok(())
}

fn clear_session_links(store: &mut WorkspaceStore) {
    let ids = store
        .states()
        .iter()
        .flat_map(|workspace| workspace.root.iter().flat_map(|root| root.tabs()))
        .filter(|tab| tab.kind == TabKind::Terminal && tab.session_id.is_some())
        .map(|tab| tab.id.clone())
        .collect::<Vec<_>>();
    for workspace in store.states_mut() {
        for id in &ids {
            if let Some(tab) = workspace.tab_mut(id) {
                tab.session_id = None;
            }
        }
    }
}

fn eligible_tabs(
    projects: &[Project],
    store: &WorkspaceStore,
    launches: &HashMap<String, PendingSessionLaunch>,
) -> Vec<EligibleTab> {
    let local = projects
        .iter()
        .filter(|project| !project.is_remote())
        .map(|project| project.id.to_ascii_uppercase())
        .collect::<HashSet<_>>();
    let mut eligible = Vec::new();
    for workspace in store.states() {
        if !local.contains(&workspace.project_id.to_ascii_uppercase()) {
            continue;
        }
        let Some(worktree_id) = workspace.worktree_id.as_deref() else {
            continue;
        };
        let Some(root) = workspace.root.as_ref() else {
            continue;
        };
        for tab in root.tabs() {
            if tab.kind != TabKind::Terminal {
                continue;
            }
            let Some(area) = workspace.area_containing_tab(&tab.id) else {
                continue;
            };
            let directory = launches
                .get(&tab.id)
                .map(|launch| launch.directory.clone())
                .or_else(|| tab.project_path.as_ref().map(PathBuf::from))
                .or_else(|| workspace.worktree_path.as_ref().map(PathBuf::from));
            let Some(directory) = directory else {
                continue;
            };
            let owner = SessionOwner {
                project_id: workspace.project_id.to_ascii_uppercase(),
                worktree_id: worktree_id.to_ascii_uppercase(),
                original_tab_id: tab.id.to_ascii_uppercase(),
            };
            let placement = WorkspacePlacement {
                project_id: owner.project_id.clone(),
                worktree_id: owner.worktree_id.clone(),
                tab_id: owner.original_tab_id.clone(),
                area_id: area.id.to_ascii_uppercase(),
            };
            if owner.validate().is_err()
                || placement.validate().is_err()
                || !directory.is_absolute()
            {
                continue;
            }
            eligible.push(EligibleTab {
                tab_id: tab.id.clone(),
                owner,
                placement,
                directory,
                command: launches
                    .get(&tab.id)
                    .and_then(|launch| launch.command.clone()),
                title: tab.title().to_owned(),
                linked_session_id: tab.session_id,
            });
        }
    }
    eligible.sort_by_key(|tab| tab.tab_id.to_ascii_uppercase());
    eligible
}

trait Control {
    fn list(&mut self) -> io::Result<Vec<SessionDescriptor>>;
    fn get(&mut self, session_id: SessionId) -> io::Result<Option<SessionDescriptor>>;
    fn create(&mut self, request: CreateSessionRequest) -> io::Result<SessionDescriptor>;
    fn set_placement(
        &mut self,
        session_id: SessionId,
        placement: WorkspacePlacement,
    ) -> io::Result<()>;
    fn end_all(&mut self) -> io::Result<()>;
}

struct FacadeControl {
    client: SessionClient,
}

impl FacadeControl {
    fn connect(contract: &LaunchContract, spawn: bool) -> io::Result<Self> {
        let client = if spawn {
            SessionClient::connect_or_spawn(
                &contract.socket_path,
                &contract.helper_path,
                contract.build_mode,
            )?
        } else {
            SessionClient::connect(&contract.socket_path, contract.build_mode)?
        };
        client.set_read_timeout(Some(STARTUP_CONTROL_TIMEOUT))?;
        client.set_write_timeout(Some(STARTUP_CONTROL_TIMEOUT))?;
        Ok(Self { client })
    }
}

fn query_linked_tab<C: Control>(
    control: &mut C,
    session_id: SessionId,
    running: LinkedTabState,
) -> io::Result<LinkedTabState> {
    Ok(match control.get(session_id)? {
        None => LinkedTabState::Missing,
        Some(descriptor) => match descriptor.status {
            SessionStatus::Running => running,
            SessionStatus::Exited { .. } => LinkedTabState::Ended,
        },
    })
}

impl Control for FacadeControl {
    fn list(&mut self) -> io::Result<Vec<SessionDescriptor>> {
        self.client.list_sessions()
    }

    fn get(&mut self, session_id: SessionId) -> io::Result<Option<SessionDescriptor>> {
        self.client.get_session(session_id)
    }

    fn create(&mut self, request: CreateSessionRequest) -> io::Result<SessionDescriptor> {
        self.client.create_session(request)
    }

    fn set_placement(
        &mut self,
        session_id: SessionId,
        placement: WorkspacePlacement,
    ) -> io::Result<()> {
        self.client
            .set_workspace_placement(session_id, Some(placement))
    }

    fn end_all(&mut self) -> io::Result<()> {
        self.client.end_all_sessions()
    }
}

fn reconcile_persistent<C: Control>(
    control: &mut C,
    contract: &LaunchContract,
    store: &mut WorkspaceStore,
    eligible: &[EligibleTab],
) -> io::Result<HashMap<String, LinkedTabState>> {
    let current = control.list()?;
    let mut updated = store.clone();
    let mut changed = false;
    let mut statuses = HashMap::new();
    for tab in eligible {
        let state = if let Some(session_id) = tab.linked_session_id {
            let matches = current
                .iter()
                .filter(|descriptor| descriptor.session_id == session_id)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => LinkedTabState::Missing,
                [descriptor] if descriptor.owner != tab.owner => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("session owner differs for tab {}", tab.tab_id),
                    ));
                }
                [descriptor] => {
                    if descriptor.placement.as_ref() != Some(&tab.placement) {
                        control.set_placement(session_id, tab.placement.clone())?;
                    }
                    match descriptor.status {
                        SessionStatus::Running => LinkedTabState::Present,
                        SessionStatus::Exited { .. } => LinkedTabState::Ended,
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("duplicate session identifier for tab {}", tab.tab_id),
                    ));
                }
            }
        } else {
            let request = create_request(contract, tab)?;
            let descriptor = control.create(request)?;
            if descriptor.owner != tab.owner {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("created session owner differs for tab {}", tab.tab_id),
                ));
            }
            let linked = updated
                .states_mut()
                .iter_mut()
                .find_map(|workspace| workspace.tab_mut(&tab.tab_id));
            let Some(linked) = linked else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "workspace tab disappeared during reconciliation: {}",
                        tab.tab_id
                    ),
                ));
            };
            linked.session_id = Some(descriptor.session_id);
            changed = true;
            match descriptor.status {
                SessionStatus::Running => LinkedTabState::Present,
                SessionStatus::Exited { .. } => LinkedTabState::Ended,
            }
        };
        statuses.insert(tab.tab_id.clone(), state);
    }
    if changed {
        updated.save()?;
        *store = updated;
    }
    Ok(statuses)
}

fn create_request(
    contract: &LaunchContract,
    tab: &EligibleTab,
) -> io::Result<CreateSessionRequest> {
    let directory = tab.directory.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "working directory is not UTF-8",
        )
    })?;
    let startup = tab.command.as_ref().map(|command| command.command.clone());
    let request = CreateSessionRequest {
        session_id: SessionId::parse(&muxy_core::store::new_uuid())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?,
        owner: tab.owner.clone(),
        placement: Some(tab.placement.clone()),
        working_directory: directory.to_owned(),
        initial_size: WindowSize::new(80, 24),
        shell_executable: contract.shell.clone(),
        argv: Vec::new(),
        startup_command: startup,
        keep_shell_open: tab
            .command
            .as_ref()
            .is_some_and(|command| command.keeps_shell_open),
        environment: contract.environment.clone(),
        ghostty_resources: contract.resources.ghostty.to_string_lossy().into_owned(),
        terminfo: contract.resources.terminfo.to_string_lossy().into_owned(),
        terminal_type: "xterm-ghostty".to_owned(),
        color_terminal: "truecolor".to_owned(),
        title: tab.title.clone(),
    };
    request
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
    Ok(request)
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::workspace::Tab;
    use std::collections::VecDeque;

    fn session_id(value: &str) -> SessionId {
        SessionId::parse(value).unwrap()
    }

    fn project(id: &str, remote: bool) -> Project {
        let mut project = Project::new("Project".into(), "/tmp".into(), 0);
        project.id = id.into();
        if remote {
            project.remote_workspace_id = Some("remote".into());
        }
        project
    }

    fn store(path: &Path, project_id: &str, worktree_id: &str) -> WorkspaceStore {
        let mut store = WorkspaceStore::load_from(path.join("workspaces.json"));
        let workspace = store.ensure_worktree(project_id, worktree_id, path.to_str().unwrap());
        let mut hidden = Tab::new(TabKind::Terminal);
        hidden.project_path = Some(path.to_string_lossy().into_owned());
        workspace.new_top_level_tab(hidden);
        store
    }

    fn contract(path: &Path) -> LaunchContract {
        let resources_root = path.join("resources");
        std::fs::create_dir_all(resources_root.join("ghostty/shell-integration")).unwrap();
        std::fs::create_dir_all(resources_root.join("terminfo")).unwrap();
        LaunchContract {
            socket_path: path.join("session/control.sock"),
            helper_path: path.join("Muxy.app/Contents/MacOS/muxy-session"),
            resources: AppResources {
                root: resources_root.clone(),
                ghostty: resources_root.join("ghostty"),
                shell_integration: resources_root.join("ghostty/shell-integration"),
                defaults_config: resources_root.join("defaults"),
                transparent_surface_config: resources_root.join("transparent"),
                terminfo: resources_root.join("terminfo"),
            },
            shell: "/bin/sh".into(),
            environment: vec![EnvironmentEntry {
                key: "MUXY_SOCKET_PATH".into(),
                value: path
                    .join(
                        muxy_core::environment::RuntimePathPolicy::new(BuildMode::Production)
                            .main_socket_filename(),
                    )
                    .to_string_lossy()
                    .into_owned(),
            }],
            build_mode: muxy_proto::session::BuildMode::Development,
        }
    }

    #[derive(Default)]
    struct FakeControl {
        sessions: Vec<SessionDescriptor>,
        created: Vec<CreateSessionRequest>,
        placements: Vec<(SessionId, WorkspacePlacement)>,
        create_ids: VecDeque<SessionId>,
        ended: usize,
        end_fails: bool,
    }

    impl Control for FakeControl {
        fn list(&mut self) -> io::Result<Vec<SessionDescriptor>> {
            Ok(self.sessions.clone())
        }

        fn get(&mut self, session_id: SessionId) -> io::Result<Option<SessionDescriptor>> {
            Ok(self
                .sessions
                .iter()
                .find(|session| session.session_id == session_id)
                .cloned())
        }

        fn create(&mut self, mut request: CreateSessionRequest) -> io::Result<SessionDescriptor> {
            if let Some(existing) = self
                .sessions
                .iter()
                .find(|session| session.owner == request.owner)
                .cloned()
            {
                self.created.push(request);
                return Ok(existing);
            }
            if let Some(id) = self.create_ids.pop_front() {
                request.session_id = id;
            }
            let descriptor = descriptor(&request, SessionStatus::Running);
            self.created.push(request);
            self.sessions.push(descriptor.clone());
            Ok(descriptor)
        }

        fn set_placement(
            &mut self,
            session_id: SessionId,
            placement: WorkspacePlacement,
        ) -> io::Result<()> {
            self.placements.push((session_id, placement));
            Ok(())
        }

        fn end_all(&mut self) -> io::Result<()> {
            self.ended += 1;
            if self.end_fails {
                return Err(io::Error::other("end failed"));
            }
            self.sessions.clear();
            Ok(())
        }
    }

    fn descriptor(request: &CreateSessionRequest, status: SessionStatus) -> SessionDescriptor {
        SessionDescriptor {
            session_id: request.session_id,
            owner: request.owner.clone(),
            placement: request.placement.clone(),
            title: request.title.clone(),
            working_directory: request.working_directory.clone(),
            shell: muxy_proto::session::ProcessIdentity {
                process_id: 100,
                start_identity: 200,
            },
            process_session_id: 100,
            process_group_id: 100,
            tty_device: 1,
            created_at_milliseconds: 1,
            renderer_attached: false,
            status,
        }
    }

    #[test]
    fn sessions_enable_links_every_local_terminal_before_surface_materialization() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let tabs = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new());
        assert_eq!(tabs.len(), 2);
        let mut control = FakeControl::default();
        let states =
            reconcile_persistent(&mut control, &contract(temp.path()), &mut store, &tabs).unwrap();
        assert_eq!(control.created.len(), 2);
        assert!(
            states
                .values()
                .all(|state| *state == LinkedTabState::Present)
        );
        assert!(
            store.states()[0]
                .root
                .as_ref()
                .unwrap()
                .tabs()
                .iter()
                .all(|tab| tab.session_id.is_some())
        );
        let reloaded = WorkspaceStore::load_from(temp.path().join("workspaces.json"));
        assert!(
            reloaded.states()[0]
                .root
                .as_ref()
                .unwrap()
                .tabs()
                .iter()
                .all(|tab| tab.session_id.is_some())
        );
    }

    #[test]
    fn sessions_recover_exact_owner_and_surface_missing_or_ended_links() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let tabs = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new());
        let held_id = session_id("123E4567-E89B-12D3-A456-426614174000");
        store.states_mut()[0]
            .tab_mut(&tabs[0].tab_id)
            .unwrap()
            .session_id = Some(held_id);
        let ended_id = session_id("223E4567-E89B-12D3-A456-426614174000");
        store.states_mut()[0]
            .tab_mut(&tabs[1].tab_id)
            .unwrap()
            .session_id = Some(ended_id);
        let mut held_request = create_request(&contract(temp.path()), &tabs[0]).unwrap();
        held_request.session_id = held_id;
        let mut ended_request = create_request(&contract(temp.path()), &tabs[1]).unwrap();
        ended_request.session_id = ended_id;
        let mut control = FakeControl {
            sessions: vec![descriptor(
                &ended_request,
                SessionStatus::Exited { status: Some(0) },
            )],
            ..Default::default()
        };
        let linked = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new());
        let states =
            reconcile_persistent(&mut control, &contract(temp.path()), &mut store, &linked)
                .unwrap();
        assert_eq!(states[&tabs[0].tab_id], LinkedTabState::Missing);
        assert_eq!(states[&tabs[1].tab_id], LinkedTabState::Ended);
        assert!(control.created.is_empty());
    }

    #[test]
    fn sessions_attachment_retry_reuses_only_the_running_linked_session() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let store = store(temp.path(), &project_id, &worktree_id);
        let tab = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new())
            .into_iter()
            .next()
            .unwrap();
        let session_id = session_id("323E4567-E89B-12D3-A456-426614174000");
        let mut request = create_request(&contract(temp.path()), &tab).unwrap();
        request.session_id = session_id;
        let mut control = FakeControl {
            sessions: vec![descriptor(&request, SessionStatus::Running)],
            ..Default::default()
        };

        assert_eq!(
            query_linked_tab(&mut control, session_id, LinkedTabState::Present).unwrap(),
            LinkedTabState::Present
        );
        assert!(control.created.is_empty());
        assert_eq!(control.sessions.len(), 1);
    }

    #[test]
    fn sessions_exclude_remote_workspaces_and_escape_each_attach_argument() {
        let temp = tempfile::tempdir().unwrap();
        let local_id = muxy_core::store::new_uuid();
        let remote_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let store = store(temp.path(), &local_id, &worktree_id);
        let mut all = store.clone();
        all.ensure_worktree(&remote_id, &worktree_id, temp.path().to_str().unwrap());
        let tabs = eligible_tabs(
            &[project(&local_id, false), project(&remote_id, true)],
            &all,
            &HashMap::new(),
        );
        assert!(tabs.iter().all(|tab| tab.owner.project_id == local_id));

        let helper = Path::new("/tmp/Muxy's App/Contents/MacOS/muxy-session");
        let socket = Path::new("/tmp/Muxy's App/session socket/control.sock");
        let command = attachment_command(
            helper,
            socket,
            session_id("323E4567-E89B-12D3-A456-426614174000"),
        )
        .unwrap();
        assert_eq!(
            command,
            "'/tmp/Muxy'\\''s App/Contents/MacOS/muxy-session' attach --socket '/tmp/Muxy'\\''s App/session socket/control.sock' --session-id 323E4567-E89B-12D3-A456-426614174000"
        );
    }

    #[test]
    fn sessions_disable_acknowledges_cleanup_before_clearing_durable_links() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let ids = [
            session_id("423E4567-E89B-12D3-A456-426614174000"),
            session_id("523E4567-E89B-12D3-A456-426614174000"),
        ];
        let tab_ids = store.states()[0]
            .root
            .as_ref()
            .unwrap()
            .tabs()
            .iter()
            .map(|tab| tab.id.clone())
            .collect::<Vec<_>>();
        for (tab_id, session_id) in tab_ids.iter().zip(ids) {
            store.states_mut()[0].tab_mut(tab_id).unwrap().session_id = Some(session_id);
        }
        store.save().unwrap();
        let mut failing = FakeControl {
            end_fails: true,
            ..Default::default()
        };
        assert!(disable_persistent(&mut failing, &mut store).is_err());
        assert!(
            store.states()[0]
                .root
                .as_ref()
                .unwrap()
                .tabs()
                .iter()
                .all(|tab| tab.session_id.is_some())
        );
        let mut control = FakeControl::default();
        disable_persistent(&mut control, &mut store).unwrap();
        assert_eq!(control.ended, 1);
        assert!(
            store.states()[0]
                .root
                .as_ref()
                .unwrap()
                .tabs()
                .iter()
                .all(|tab| tab.session_id.is_none())
        );
        let reloaded = WorkspaceStore::load_from(temp.path().join("workspaces.json"));
        assert!(
            reloaded.states()[0]
                .root
                .as_ref()
                .unwrap()
                .tabs()
                .iter()
                .all(|tab| tab.session_id.is_none())
        );
    }

    #[test]
    fn sessions_startup_barrier_and_restart_mode_are_explicit() {
        assert!(
            !SessionCoordinator {
                barrier: StartupBarrier::Reconciling,
                desired: DesiredSessionMode::Persistent,
                mode: BuildMode::Development,
                app_support: PathBuf::new(),
                current_executable: PathBuf::new(),
                main_socket_path: PathBuf::new(),
                contract: None,
                linked_tabs: HashMap::new(),
            }
            .is_ready()
        );
        assert!(
            !SessionCoordinator {
                barrier: StartupBarrier::Blocked("retry".into()),
                desired: DesiredSessionMode::Ordinary,
                mode: BuildMode::Development,
                app_support: PathBuf::new(),
                current_executable: PathBuf::new(),
                main_socket_path: PathBuf::new(),
                contract: None,
                linked_tabs: HashMap::new(),
            }
            .is_ready()
        );
    }
}
