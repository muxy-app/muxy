use crate::resources::AppResources;
use muxy_core::environment::BuildMode;
use muxy_core::resources::ProcessIdentity as ResourceProcessIdentity;
use muxy_core::session::transition::DesiredSessionMode;
use muxy_core::store::Project;
use muxy_core::workspace::{CloseMode, Tab, TabKind, WorkspaceState};
use muxy_core::workspace_store::WorkspaceStore;
use muxy_proto::session::{
    CreateSessionRequest, EnvironmentEntry, ProcessIdentity, SessionDescriptor, SessionId,
    SessionOwner, SessionStatus, WindowSize, WorkspacePlacement,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionCloseCandidate {
    pub tab_id: String,
    pub session_id: SessionId,
    pub owner: SessionOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionClosePlan {
    pub project_id: String,
    pub worktree_id: Option<String>,
    pub tab_ids: Vec<String>,
    pub persistent: Vec<SessionCloseCandidate>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionOwnerScope {
    pub project_id: String,
    pub worktree_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionOwnerCleanupCandidate {
    pub session_id: SessionId,
    pub owner: SessionOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionOwnerCleanupPlan {
    pub scope: SessionOwnerScope,
    pub candidates: Vec<SessionOwnerCleanupCandidate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedSessionState {
    Workspace,
    Background,
    Missing,
    Ended,
    AttachmentFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedSession {
    pub session_id: SessionId,
    pub owner: SessionOwner,
    pub placement: Option<WorkspacePlacement>,
    pub shell: Option<ProcessIdentity>,
    pub title: String,
    pub working_directory: String,
    pub state: ManagedSessionState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionResourceRoots {
    pub identities: Vec<ResourceProcessIdentity>,
    pub session_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionReattachOutcome {
    Focused(WorkspacePlacement),
    Reattached(WorkspacePlacement),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionEndCandidate {
    pub session_id: SessionId,
    pub owner: SessionOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionEndAllPlan {
    pub candidates: Vec<SessionEndCandidate>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnerObservation {
    generation: u64,
    request_id: u64,
    missing_count: u8,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkedSession {
    tab_id: String,
    session_id: SessionId,
    owner: SessionOwner,
    placement: WorkspacePlacement,
    title: String,
    working_directory: String,
}

#[derive(Clone)]
pub struct SessionCoordinator {
    barrier: StartupBarrier,
    desired: DesiredSessionMode,
    mode: BuildMode,
    app_support: PathBuf,
    current_executable: PathBuf,
    main_socket_path: PathBuf,
    contract: Option<LaunchContract>,
    linked_tabs: HashMap<String, LinkedTabState>,
    owner_observations: HashMap<SessionOwnerScope, OwnerObservation>,
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
            owner_observations: HashMap::new(),
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

    pub fn close_plan(
        workspace: &WorkspaceState,
        tab_id: &str,
        mode: CloseMode,
    ) -> SessionClosePlan {
        let mut candidate = workspace.clone();
        let tab_ids = candidate.close_tab(tab_id, mode);
        let persistent = tab_ids
            .iter()
            .filter_map(|tab_id| {
                let tab = workspace.tab(tab_id)?;
                let session_id = tab.session_id?;
                let worktree_id = workspace.worktree_id.as_ref()?;
                let owner = SessionOwner {
                    project_id: workspace.project_id.to_ascii_uppercase(),
                    worktree_id: worktree_id.to_ascii_uppercase(),
                    original_tab_id: tab.id.to_ascii_uppercase(),
                };
                owner.validate().ok()?;
                Some(SessionCloseCandidate {
                    tab_id: tab.id.clone(),
                    session_id,
                    owner,
                })
            })
            .collect();
        SessionClosePlan {
            project_id: workspace.project_id.clone(),
            worktree_id: workspace.worktree_id.clone(),
            tab_ids,
            persistent,
        }
    }

    pub fn end_close_plan(&mut self, plan: &SessionClosePlan) -> Result<(), String> {
        if plan.persistent.is_empty() {
            return Ok(());
        }
        let mut control = self
            .existing_control()?
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        end_close_candidates(&mut control, &plan.persistent).map_err(display_error)?;
        for candidate in &plan.persistent {
            self.linked_tabs
                .insert(candidate.tab_id.clone(), LinkedTabState::Ended);
        }
        Ok(())
    }

    pub fn managed_sessions(&self, store: &WorkspaceStore) -> Result<Vec<ManagedSession>, String> {
        let linked = linked_sessions(store);
        let descriptors = match self.existing_control()? {
            Some(mut control) => control.list().map_err(display_error)?,
            None if linked.is_empty() => Vec::new(),
            None => return Err("persistent session runtime is unavailable".to_owned()),
        };
        let descriptor_ids = descriptors
            .iter()
            .map(|descriptor| descriptor.session_id)
            .collect::<HashSet<_>>();
        let mut sessions = descriptors
            .into_iter()
            .filter_map(|descriptor| {
                let linked_state = descriptor
                    .placement
                    .as_ref()
                    .and_then(|placement| self.linked_tabs.get(&placement.tab_id));
                let linked_matches = linked
                    .iter()
                    .filter(|candidate| candidate.session_id == descriptor.session_id)
                    .collect::<Vec<_>>();
                if matches!(descriptor.status, SessionStatus::Exited { .. })
                    && linked_matches.is_empty()
                {
                    return None;
                }
                let invalid_owner = linked_matches.len() > 1
                    || linked_matches
                        .first()
                        .is_some_and(|linked| linked.owner != descriptor.owner);
                let state = match descriptor.status {
                    SessionStatus::Exited { .. } => ManagedSessionState::Ended,
                    SessionStatus::Running
                        if invalid_owner
                            || linked_state == Some(&LinkedTabState::AttachmentFailed) =>
                    {
                        ManagedSessionState::AttachmentFailed
                    }
                    SessionStatus::Running if descriptor.placement.is_some() => {
                        ManagedSessionState::Workspace
                    }
                    SessionStatus::Running => ManagedSessionState::Background,
                };
                Some(ManagedSession {
                    session_id: descriptor.session_id,
                    owner: descriptor.owner,
                    placement: descriptor.placement,
                    shell: Some(descriptor.shell),
                    title: descriptor.title,
                    working_directory: descriptor.working_directory,
                    state,
                })
            })
            .collect::<Vec<_>>();
        sessions.extend(linked.into_iter().filter_map(|linked| {
            if descriptor_ids.contains(&linked.session_id) {
                return None;
            }
            let state = match self.linked_tabs.get(&linked.tab_id) {
                Some(LinkedTabState::Ended) => ManagedSessionState::Ended,
                Some(LinkedTabState::AttachmentFailed) => ManagedSessionState::AttachmentFailed,
                Some(LinkedTabState::Present | LinkedTabState::Missing) | None => {
                    ManagedSessionState::Missing
                }
            };
            Some(ManagedSession {
                session_id: linked.session_id,
                owner: linked.owner,
                placement: Some(linked.placement),
                shell: None,
                title: linked.title,
                working_directory: linked.working_directory,
                state,
            })
        }));
        sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(sessions)
    }

    pub fn live_session_descriptors(&self) -> Result<Vec<SessionDescriptor>, String> {
        let mut control = self
            .existing_control()?
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        let mut sessions = control
            .list()
            .map_err(display_error)?
            .into_iter()
            .filter(|session| session.status == SessionStatus::Running)
            .collect::<Vec<_>>();
        sessions.sort_by_key(|session| session.session_id);
        Ok(sessions)
    }

    pub fn end_session(
        &mut self,
        session_id: SessionId,
        expected_owner: &SessionOwner,
        store: &mut WorkspaceStore,
    ) -> Result<(), String> {
        let mut control = self
            .existing_control()?
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        let linked_tab = end_exact_session(&mut control, store, session_id, expected_owner)?;
        if let Some(tab_id) = linked_tab {
            self.linked_tabs.remove(&tab_id);
        }
        Ok(())
    }

    pub fn end_session_by_id(
        &mut self,
        session_id: SessionId,
        store: &mut WorkspaceStore,
    ) -> Result<(), String> {
        let descriptor = self
            .live_session_descriptors()?
            .into_iter()
            .find(|descriptor| descriptor.session_id == session_id)
            .ok_or_else(|| format!("pane not found {session_id}"))?;
        self.end_session(session_id, &descriptor.owner, store)
    }

    pub fn remove_stale_session(
        &mut self,
        tab_id: &str,
        session_id: SessionId,
        expected_owner: &SessionOwner,
        store: &mut WorkspaceStore,
    ) -> Result<(), String> {
        let linked = linked_sessions(store)
            .into_iter()
            .filter(|linked| linked.tab_id.eq_ignore_ascii_case(tab_id))
            .collect::<Vec<_>>();
        let [linked] = linked.as_slice() else {
            return Err("workspace session link is missing or duplicated".to_owned());
        };
        if linked.session_id != session_id {
            return Err("workspace session link changed".to_owned());
        }
        if let Some(mut control) = self.existing_control()? {
            match control.get(session_id).map_err(display_error)? {
                Some(descriptor) => {
                    if descriptor.owner != *expected_owner {
                        return Err(format!("session owner mismatch {session_id}"));
                    }
                    if descriptor.status == SessionStatus::Running {
                        return Err("the linked background session is still running".to_owned());
                    }
                }
                None if linked.owner != *expected_owner => {
                    return Err("workspace session link changed".to_owned());
                }
                None => {}
            }
        } else if linked.owner != *expected_owner {
            return Err("workspace session link changed".to_owned());
        }
        remove_stale_workspace_link(store, tab_id)?;
        self.linked_tabs.remove(tab_id);
        Ok(())
    }

    pub fn end_all_plan(&self) -> Result<SessionEndAllPlan, String> {
        let candidates = self
            .live_session_descriptors()?
            .into_iter()
            .map(|descriptor| SessionEndCandidate {
                session_id: descriptor.session_id,
                owner: descriptor.owner,
            })
            .collect();
        Ok(SessionEndAllPlan { candidates })
    }

    pub fn end_all_plan_sessions(
        &mut self,
        plan: &SessionEndAllPlan,
        store: &mut WorkspaceStore,
    ) -> Result<(), String> {
        if plan.candidates.is_empty() {
            if self.end_all_plan()?.candidates.is_empty() {
                return Ok(());
            }
            return Err("the active session set changed after confirmation".to_owned());
        }
        let mut control = self
            .existing_control()?
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        let linked_tabs = end_session_plan(&mut control, store, plan)?;
        for tab_id in linked_tabs {
            self.linked_tabs.remove(&tab_id);
        }
        Ok(())
    }

    pub fn send_to_background(
        &mut self,
        tab_id: &str,
        session_id: SessionId,
        owner: &SessionOwner,
    ) -> Result<(), String> {
        let mut control = self
            .existing_control()?
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        let descriptor = exact_running_session(&mut control, session_id, owner)?;
        control
            .clear_placement(descriptor.session_id)
            .map_err(display_error)?;
        self.linked_tabs.remove(tab_id);
        Ok(())
    }

    pub fn reattach(
        &mut self,
        session_id: SessionId,
        store: &mut WorkspaceStore,
    ) -> Result<SessionReattachOutcome, String> {
        let mut control = self
            .existing_control()?
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        let descriptor = control
            .get(session_id)
            .map_err(display_error)?
            .ok_or_else(|| "background session is missing".to_owned())?;
        if descriptor.status != SessionStatus::Running {
            return Err("background session has ended".to_owned());
        }
        let outcome = reattach_session(&mut control, store, &descriptor).map_err(display_error)?;
        let placement = match &outcome {
            SessionReattachOutcome::Focused(placement)
            | SessionReattachOutcome::Reattached(placement) => placement,
        };
        self.linked_tabs
            .insert(placement.tab_id.clone(), LinkedTabState::Present);
        Ok(outcome)
    }

    pub fn owner_cleanup_plan(
        &self,
        scope: &SessionOwnerScope,
    ) -> Result<SessionOwnerCleanupPlan, String> {
        let candidates = match self.existing_control()? {
            Some(mut control) => {
                owner_cleanup_candidates(&mut control, scope).map_err(display_error)?
            }
            None => Vec::new(),
        };
        Ok(SessionOwnerCleanupPlan {
            scope: scope.clone(),
            candidates,
        })
    }

    pub fn owner_session_count(&self, scope: &SessionOwnerScope) -> Result<usize, String> {
        self.owner_cleanup_plan(scope)
            .map(|plan| plan.candidates.len())
    }

    pub fn end_owner_cleanup_plan(
        &mut self,
        plan: &SessionOwnerCleanupPlan,
    ) -> Result<Vec<SessionId>, String> {
        if plan.candidates.is_empty() {
            let current = self.owner_cleanup_plan(&plan.scope)?;
            if current.candidates.is_empty() {
                return Ok(Vec::new());
            }
            return Err("the background session set changed after confirmation".to_owned());
        }
        let mut control = self
            .existing_control()?
            .ok_or_else(|| "persistent session runtime is unavailable".to_owned())?;
        end_owner_cleanup_candidates(&mut control, plan).map_err(display_error)
    }

    pub fn end_owner_sessions(
        &mut self,
        scope: &SessionOwnerScope,
    ) -> Result<Vec<SessionId>, String> {
        let plan = self.owner_cleanup_plan(scope)?;
        self.end_owner_cleanup_plan(&plan)
    }

    pub fn resource_roots(&self) -> Result<SessionResourceRoots, String> {
        let Some(mut control) = self.existing_control()? else {
            return Ok(SessionResourceRoots {
                identities: Vec::new(),
                session_count: 0,
            });
        };
        let daemon = control.daemon_identity();
        let descriptors = control.list().map_err(display_error)?;
        let mut shells = descriptors
            .iter()
            .filter(|descriptor| descriptor.status == SessionStatus::Running)
            .map(|descriptor| descriptor.shell)
            .collect::<Vec<_>>();
        shells.sort_by_key(|identity| (identity.process_id, identity.start_identity));
        shells.dedup();
        shells.retain(|identity| *identity != daemon);
        let mut identities = Vec::with_capacity(shells.len() + 1);
        identities.push(daemon);
        identities.extend(shells);
        Ok(SessionResourceRoots {
            identities: identities
                .into_iter()
                .map(|identity| ResourceProcessIdentity {
                    process_id: identity.process_id,
                    start_identity: identity.start_identity,
                })
                .collect(),
            session_count: descriptors
                .iter()
                .filter(|descriptor| descriptor.status == SessionStatus::Running)
                .count(),
        })
    }

    pub fn runtime_process_identities(&self) -> Result<Vec<ProcessIdentity>, String> {
        self.resource_roots().map(|roots| {
            roots
                .identities
                .into_iter()
                .map(|identity| ProcessIdentity {
                    process_id: identity.process_id,
                    start_identity: identity.start_identity,
                })
                .collect()
        })
    }

    pub fn end_all_sessions(&mut self) -> Result<(), String> {
        let Some(mut control) = self.existing_control()? else {
            return Ok(());
        };
        control.end_all().map_err(display_error)?;
        for state in self.linked_tabs.values_mut() {
            *state = LinkedTabState::Ended;
        }
        Ok(())
    }

    pub fn start_new_terminal(
        &mut self,
        tab_id: &str,
        session_id: SessionId,
        store: &mut WorkspaceStore,
    ) -> Result<(), String> {
        if let Some(mut control) = self.existing_control()?
            && let Some(descriptor) = control.get(session_id).map_err(display_error)?
            && descriptor.status == SessionStatus::Running
        {
            return Err("the linked background session is still running".to_owned());
        }
        let mut updated = store.clone();
        let tab = updated
            .states_mut()
            .iter_mut()
            .find_map(|workspace| workspace.tab_mut(tab_id))
            .ok_or_else(|| "workspace tab no longer exists".to_owned())?;
        if tab.session_id != Some(session_id) {
            return Err("workspace session link changed".to_owned());
        }
        tab.session_id = None;
        updated.save().map_err(display_error)?;
        *store = updated;
        self.linked_tabs.remove(tab_id);
        Ok(())
    }

    pub fn reconcile_owner_existence(
        &mut self,
        facts: &[muxy_api::truth::OwnerExistenceFact],
        store: &WorkspaceStore,
    ) -> Result<Vec<SessionId>, String> {
        let scopes = confirmed_missing_scopes(&mut self.owner_observations, facts);
        if scopes.is_empty() {
            return Ok(Vec::new());
        }
        let Some(mut control) = self.existing_control()? else {
            return Ok(Vec::new());
        };
        let ended = end_owner_sessions(&mut control, &scopes).map_err(display_error)?;
        for linked in linked_sessions(store) {
            if ended.contains(&linked.session_id) {
                self.linked_tabs
                    .insert(linked.tab_id, LinkedTabState::Ended);
            }
        }
        Ok(ended)
    }

    fn existing_control(&self) -> Result<Option<FacadeControl>, String> {
        let fallback;
        let contract = match self.contract.as_ref() {
            Some(contract) => contract,
            None => {
                let socket_path = runtime_socket_path(self.mode, &self.app_support);
                if !socket_path.exists() {
                    return Ok(None);
                }
                fallback = self.build_contract(socket_path)?;
                &fallback
            }
        };
        FacadeControl::connect(contract, false)
            .map(Some)
            .map_err(display_error)
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
    linked_sessions(store).len()
}

fn linked_sessions(store: &WorkspaceStore) -> Vec<LinkedSession> {
    let mut linked = Vec::new();
    for workspace in store.states() {
        let Some(worktree_id) = workspace.worktree_id.as_deref() else {
            continue;
        };
        let Some(root) = workspace.root.as_ref() else {
            continue;
        };
        for tab in root.tabs() {
            let Some(session_id) = tab.session_id else {
                continue;
            };
            let Some(area) = workspace.area_containing_tab(&tab.id) else {
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
            if owner.validate().is_err() || placement.validate().is_err() {
                continue;
            }
            linked.push(LinkedSession {
                tab_id: tab.id.clone(),
                session_id,
                owner,
                placement,
                title: tab.title().to_owned(),
                working_directory: tab
                    .project_path
                    .clone()
                    .or_else(|| workspace.worktree_path.clone())
                    .unwrap_or_default(),
            });
        }
    }
    linked
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
    fn clear_placement(&mut self, session_id: SessionId) -> io::Result<()>;
    fn end(&mut self, session_id: SessionId) -> io::Result<()>;
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

    fn daemon_identity(&self) -> ProcessIdentity {
        self.client.daemon_identity()
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

fn reattach_session<C: Control>(
    control: &mut C,
    store: &mut WorkspaceStore,
    descriptor: &SessionDescriptor,
) -> io::Result<SessionReattachOutcome> {
    let existing = linked_sessions(store)
        .into_iter()
        .filter(|linked| linked.session_id == descriptor.session_id)
        .collect::<Vec<_>>();
    if let [linked] = existing.as_slice() {
        if linked.owner != descriptor.owner {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "workspace session owner differs",
            ));
        }
        let mut updated = store.clone();
        let workspace = updated
            .states_mut()
            .iter_mut()
            .find(|workspace| {
                workspace
                    .project_id
                    .eq_ignore_ascii_case(&linked.owner.project_id)
                    && workspace.worktree_id.as_deref().is_some_and(|worktree_id| {
                        worktree_id.eq_ignore_ascii_case(&linked.owner.worktree_id)
                    })
            })
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "original workspace is missing")
            })?;
        if !workspace.select_tab(&linked.placement.area_id, &linked.tab_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "workspace session placement is invalid",
            ));
        }
        updated.save()?;
        *store = updated;
        control.set_placement(descriptor.session_id, linked.placement.clone())?;
        return Ok(SessionReattachOutcome::Focused(linked.placement.clone()));
    }
    if !existing.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "workspace contains duplicate session links",
        ));
    }
    if linked_sessions(store).iter().any(|linked| {
        linked
            .tab_id
            .eq_ignore_ascii_case(&descriptor.owner.original_tab_id)
    }) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "original session tab identifier is already in use",
        ));
    }
    let mut updated = store.clone();
    let workspace = updated
        .states_mut()
        .iter_mut()
        .find(|workspace| {
            workspace
                .project_id
                .eq_ignore_ascii_case(&descriptor.owner.project_id)
                && workspace.worktree_id.as_deref().is_some_and(|worktree_id| {
                    worktree_id.eq_ignore_ascii_case(&descriptor.owner.worktree_id)
                })
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "original workspace is missing"))?;
    let mut tab = Tab::new(TabKind::Terminal);
    tab.id = descriptor.owner.original_tab_id.clone();
    tab.session_id = Some(descriptor.session_id);
    tab.project_path = Some(descriptor.working_directory.clone());
    tab.pane_title = Some(descriptor.title.clone());
    let tab_id = workspace.new_top_level_tab(tab).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "original session tab identifier is already in use",
        )
    })?;
    let area_id = workspace
        .area_containing_tab(&tab_id)
        .map(|area| area.id.to_ascii_uppercase())
        .ok_or_else(|| io::Error::other("reattached tab has no workspace area"))?;
    let placement = WorkspacePlacement {
        project_id: descriptor.owner.project_id.clone(),
        worktree_id: descriptor.owner.worktree_id.clone(),
        tab_id,
        area_id,
    };
    placement
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    updated.save()?;
    *store = updated;
    control.set_placement(descriptor.session_id, placement.clone())?;
    Ok(SessionReattachOutcome::Reattached(placement))
}

fn remove_stale_workspace_link(store: &mut WorkspaceStore, tab_id: &str) -> Result<(), String> {
    remove_workspace_links(store, &[tab_id.to_owned()])
}

fn remove_workspace_links(store: &mut WorkspaceStore, tab_ids: &[String]) -> Result<(), String> {
    let updated = store_without_workspace_links(store, tab_ids)?;
    updated.save().map_err(display_error)?;
    *store = updated;
    Ok(())
}

fn store_without_workspace_links(
    store: &WorkspaceStore,
    tab_ids: &[String],
) -> Result<WorkspaceStore, String> {
    let mut updated = store.clone();
    for tab_id in tab_ids {
        let removed = updated
            .states_mut()
            .iter_mut()
            .find(|workspace| workspace.tab(tab_id).is_some())
            .map(|workspace| workspace.close_tab(tab_id, CloseMode::Single))
            .unwrap_or_default();
        if removed != [tab_id.to_owned()] {
            return Err("workspace session removal changed".to_owned());
        }
    }
    Ok(updated)
}

fn exact_session_link(
    store: &WorkspaceStore,
    descriptor: &SessionDescriptor,
    expected_owner: &SessionOwner,
) -> Result<Option<String>, String> {
    if &descriptor.owner != expected_owner {
        return Err(format!("session owner mismatch {}", descriptor.session_id));
    }
    let linked = linked_sessions(store)
        .into_iter()
        .filter(|linked| linked.session_id == descriptor.session_id)
        .collect::<Vec<_>>();
    if linked.len() > 1 {
        return Err(format!(
            "duplicate workspace session link {}",
            descriptor.session_id
        ));
    }
    Ok(linked.first().map(|linked| linked.tab_id.clone()))
}

fn end_exact_session<C: Control>(
    control: &mut C,
    store: &mut WorkspaceStore,
    session_id: SessionId,
    expected_owner: &SessionOwner,
) -> Result<Option<String>, String> {
    let descriptor = control
        .get(session_id)
        .map_err(display_error)?
        .ok_or_else(|| format!("pane not found {session_id}"))?;
    let linked_tab = exact_session_link(store, &descriptor, expected_owner)?;
    let updated = linked_tab
        .as_ref()
        .map(|tab_id| store_without_workspace_links(store, std::slice::from_ref(tab_id)))
        .transpose()?;
    if descriptor.status == SessionStatus::Running {
        control.end(session_id).map_err(display_error)?;
    }
    if let Some(updated) = updated {
        updated.save().map_err(display_error)?;
        *store = updated;
    }
    Ok(linked_tab)
}

fn end_session_plan<C: Control>(
    control: &mut C,
    store: &mut WorkspaceStore,
    plan: &SessionEndAllPlan,
) -> Result<Vec<String>, String> {
    let mut descriptors = control
        .list()
        .map_err(display_error)?
        .into_iter()
        .filter(|descriptor| descriptor.status == SessionStatus::Running)
        .collect::<Vec<_>>();
    descriptors.sort_by_key(|descriptor| descriptor.session_id);
    let current = SessionEndAllPlan {
        candidates: descriptors
            .iter()
            .map(|descriptor| SessionEndCandidate {
                session_id: descriptor.session_id,
                owner: descriptor.owner.clone(),
            })
            .collect(),
    };
    if current != *plan {
        return Err("the active session set changed after confirmation".to_owned());
    }
    let mut linked_tabs = Vec::new();
    for (descriptor, candidate) in descriptors.iter().zip(&plan.candidates) {
        if let Some(tab_id) = exact_session_link(store, descriptor, &candidate.owner)? {
            linked_tabs.push(tab_id);
        }
    }
    let updated = if linked_tabs.is_empty() {
        None
    } else {
        Some(store_without_workspace_links(store, &linked_tabs)?)
    };
    for candidate in &plan.candidates {
        control.end(candidate.session_id).map_err(display_error)?;
    }
    if let Some(updated) = updated {
        updated.save().map_err(display_error)?;
        *store = updated;
    }
    Ok(linked_tabs)
}

fn exact_running_session<C: Control>(
    control: &mut C,
    session_id: SessionId,
    owner: &SessionOwner,
) -> Result<SessionDescriptor, String> {
    let descriptor = control
        .get(session_id)
        .map_err(display_error)?
        .ok_or_else(|| "background session is missing".to_owned())?;
    if &descriptor.owner != owner {
        return Err("background session owner changed".to_owned());
    }
    if descriptor.status != SessionStatus::Running {
        return Err("background session has ended".to_owned());
    }
    Ok(descriptor)
}

fn end_close_candidates<C: Control>(
    control: &mut C,
    candidates: &[SessionCloseCandidate],
) -> io::Result<()> {
    let descriptors = control.list()?;
    let mut running = Vec::new();
    for candidate in candidates {
        let matches = descriptors
            .iter()
            .filter(|descriptor| descriptor.session_id == candidate.session_id)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {}
            [descriptor] if descriptor.owner != candidate.owner => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("session owner differs for tab {}", candidate.tab_id),
                ));
            }
            [descriptor] if descriptor.status == SessionStatus::Running => {
                running.push(candidate.session_id);
            }
            [_] => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("duplicate session identifier for tab {}", candidate.tab_id),
                ));
            }
        }
    }
    for session_id in running {
        control.end(session_id)?;
    }
    Ok(())
}

fn owner_matches_scope(owner: &SessionOwner, scope: &SessionOwnerScope) -> bool {
    owner.project_id.eq_ignore_ascii_case(&scope.project_id)
        && scope
            .worktree_id
            .as_deref()
            .is_none_or(|worktree_id| owner.worktree_id.eq_ignore_ascii_case(worktree_id))
}

fn owner_cleanup_candidates<C: Control>(
    control: &mut C,
    scope: &SessionOwnerScope,
) -> io::Result<Vec<SessionOwnerCleanupCandidate>> {
    let mut candidates = control
        .list()?
        .into_iter()
        .filter(|descriptor| {
            descriptor.status == SessionStatus::Running
                && owner_matches_scope(&descriptor.owner, scope)
        })
        .map(|descriptor| SessionOwnerCleanupCandidate {
            session_id: descriptor.session_id,
            owner: descriptor.owner,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.session_id.cmp(&right.session_id));
    Ok(candidates)
}

fn end_owner_cleanup_candidates<C: Control>(
    control: &mut C,
    plan: &SessionOwnerCleanupPlan,
) -> io::Result<Vec<SessionId>> {
    if owner_cleanup_candidates(control, &plan.scope)? != plan.candidates {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the background session set changed after confirmation",
        ));
    }
    let mut ended = Vec::with_capacity(plan.candidates.len());
    for candidate in &plan.candidates {
        control.end(candidate.session_id)?;
        ended.push(candidate.session_id);
    }
    Ok(ended)
}

fn end_owner_sessions<C: Control>(
    control: &mut C,
    scopes: &[SessionOwnerScope],
) -> io::Result<Vec<SessionId>> {
    let mut ended = Vec::new();
    for descriptor in control.list()? {
        if descriptor.status == SessionStatus::Running
            && scopes
                .iter()
                .any(|scope| owner_matches_scope(&descriptor.owner, scope))
            && !ended.contains(&descriptor.session_id)
        {
            control.end(descriptor.session_id)?;
            ended.push(descriptor.session_id);
        }
    }
    Ok(ended)
}

fn confirmed_missing_scopes(
    observations: &mut HashMap<SessionOwnerScope, OwnerObservation>,
    facts: &[muxy_api::truth::OwnerExistenceFact],
) -> Vec<SessionOwnerScope> {
    let mut confirmed = Vec::new();
    for fact in facts {
        let scope = SessionOwnerScope {
            project_id: fact.project_id.to_ascii_uppercase(),
            worktree_id: fact.worktree_id.as_ref().map(|id| id.to_ascii_uppercase()),
        };
        let previous = observations.get(&scope).copied();
        if previous.is_some_and(|previous| {
            previous.generation > fact.generation
                || (previous.generation == fact.generation
                    && previous.request_id >= fact.request_id)
        }) {
            continue;
        }
        let missing_count = match fact.existence {
            muxy_api::truth::OwnerExistence::Missing
                if previous.is_some_and(|previous| {
                    previous.generation == fact.generation && previous.missing_count > 0
                }) =>
            {
                2
            }
            muxy_api::truth::OwnerExistence::Missing => 1,
            muxy_api::truth::OwnerExistence::Present | muxy_api::truth::OwnerExistence::Unknown => {
                0
            }
        };
        observations.insert(
            scope.clone(),
            OwnerObservation {
                generation: fact.generation,
                request_id: fact.request_id,
                missing_count,
            },
        );
        if missing_count == 2 {
            confirmed.push(scope);
        }
    }
    confirmed
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

    fn clear_placement(&mut self, session_id: SessionId) -> io::Result<()> {
        self.client.set_workspace_placement(session_id, None)
    }

    fn end(&mut self, session_id: SessionId) -> io::Result<()> {
        self.client.end_session(session_id)
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
        cleared_placements: Vec<SessionId>,
        ended_ids: Vec<SessionId>,
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
            self.placements.push((session_id, placement.clone()));
            if let Some(session) = self
                .sessions
                .iter_mut()
                .find(|session| session.session_id == session_id)
            {
                session.placement = Some(placement);
            }
            Ok(())
        }

        fn clear_placement(&mut self, session_id: SessionId) -> io::Result<()> {
            self.cleared_placements.push(session_id);
            if let Some(session) = self
                .sessions
                .iter_mut()
                .find(|session| session.session_id == session_id)
            {
                session.placement = None;
            }
            Ok(())
        }

        fn end(&mut self, session_id: SessionId) -> io::Result<()> {
            if self.end_fails {
                return Err(io::Error::other("end failed"));
            }
            self.ended_ids.push(session_id);
            if let Some(session) = self
                .sessions
                .iter_mut()
                .find(|session| session.session_id == session_id)
            {
                session.status = SessionStatus::Exited { status: Some(0) };
            }
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
    fn session_lifecycle_close_plan_ends_persistent_backing_before_workspace_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let eligible = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new());
        let closing = &eligible[0];
        let held_id = session_id("623E4567-E89B-12D3-A456-426614174000");
        store.states_mut()[0]
            .tab_mut(&closing.tab_id)
            .unwrap()
            .session_id = Some(held_id);
        let plan =
            SessionCoordinator::close_plan(&store.states()[0], &closing.tab_id, CloseMode::Single);
        assert_eq!(plan.tab_ids, vec![closing.tab_id.clone()]);
        assert_eq!(plan.persistent.len(), 1);
        assert!(store.states()[0].tab(&closing.tab_id).is_some());

        let mut request = create_request(&contract(temp.path()), closing).unwrap();
        request.session_id = held_id;
        let mut control = FakeControl {
            sessions: vec![descriptor(&request, SessionStatus::Running)],
            ..Default::default()
        };
        end_close_candidates(&mut control, &plan.persistent).unwrap();
        assert_eq!(control.ended_ids, vec![held_id]);
        assert!(store.states()[0].tab(&closing.tab_id).is_some());
        let removed = store.states_mut()[0].close_tab(&closing.tab_id, CloseMode::Single);
        assert_eq!(removed, plan.tab_ids);
    }

    #[test]
    fn session_lifecycle_all_close_modes_partition_mixed_tabs_and_validate_before_cleanup() {
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut workspace =
            WorkspaceState::with_worktree(&project_id, &worktree_id, "/tmp/project");
        let mut first = Tab::new(TabKind::Terminal);
        first.session_id = Some(session_id("923E4567-E89B-12D3-A456-426614174000"));
        let first_id = workspace.new_top_level_tab(first).unwrap();
        let mut child = Tab::new(TabKind::Terminal);
        child.session_id = Some(session_id("A23E4567-E89B-12D3-A456-426614174000"));
        let child_id = child.id.clone();
        workspace
            .split_focused_area(muxy_core::workspace::Edge::Right, child)
            .unwrap();
        let second_id = workspace
            .new_top_level_tab(Tab::new(TabKind::Terminal))
            .unwrap();
        let browser_id = workspace
            .new_top_level_tab(Tab::new(TabKind::Browser))
            .unwrap();
        let extension_id = workspace
            .new_top_level_tab(Tab::new(TabKind::ExtensionWebView))
            .unwrap();

        let single = SessionCoordinator::close_plan(&workspace, &first_id, CloseMode::Single);
        assert_eq!(single.tab_ids, vec![first_id.clone(), child_id.clone()]);
        assert_eq!(single.persistent.len(), 2);
        let others = SessionCoordinator::close_plan(&workspace, &second_id, CloseMode::Others);
        assert_eq!(
            others.tab_ids,
            vec![
                first_id.clone(),
                child_id.clone(),
                browser_id.clone(),
                extension_id.clone()
            ]
        );
        assert_eq!(others.persistent.len(), 2);
        let left = SessionCoordinator::close_plan(&workspace, &browser_id, CloseMode::ToLeft);
        assert_eq!(
            left.tab_ids,
            vec![first_id.clone(), child_id, second_id.clone()]
        );
        assert_eq!(left.persistent.len(), 2);
        let right = SessionCoordinator::close_plan(&workspace, &second_id, CloseMode::ToRight);
        assert_eq!(right.tab_ids, vec![browser_id, extension_id]);
        assert!(right.persistent.is_empty());

        let requests = single
            .persistent
            .iter()
            .map(|candidate| CreateSessionRequest {
                session_id: candidate.session_id,
                owner: candidate.owner.clone(),
                placement: None,
                working_directory: "/tmp".into(),
                initial_size: WindowSize::new(80, 24),
                shell_executable: "/bin/sh".into(),
                argv: Vec::new(),
                startup_command: None,
                keep_shell_open: false,
                environment: Vec::new(),
                ghostty_resources: "/tmp/resources".into(),
                terminfo: "/tmp/terminfo".into(),
                terminal_type: "xterm-ghostty".into(),
                color_terminal: "truecolor".into(),
                title: "Terminal".into(),
            })
            .collect::<Vec<_>>();
        let mut descriptors = requests
            .iter()
            .map(|request| descriptor(request, SessionStatus::Running))
            .collect::<Vec<_>>();
        descriptors[1].owner.original_tab_id = muxy_core::store::new_uuid();
        let mut control = FakeControl {
            sessions: descriptors,
            ..Default::default()
        };
        assert!(end_close_candidates(&mut control, &single.persistent).is_err());
        assert!(control.ended_ids.is_empty());
    }

    #[test]
    fn session_manager_end_validates_exact_owner_before_signaling() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let tab = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new()).remove(0);
        let id = session_id("B23E4567-E89B-12D3-A456-426614174000");
        store.states_mut()[0]
            .tab_mut(&tab.tab_id)
            .unwrap()
            .session_id = Some(id);
        store.save().unwrap();
        let mut request = create_request(&contract(temp.path()), &tab).unwrap();
        request.session_id = id;
        let descriptor = descriptor(&request, SessionStatus::Running);
        let mut control = FakeControl {
            sessions: vec![descriptor.clone()],
            ..Default::default()
        };
        let mut changed_owner = descriptor.owner.clone();
        changed_owner.worktree_id = muxy_core::store::new_uuid();

        assert_eq!(
            end_exact_session(&mut control, &mut store, id, &changed_owner),
            Err(format!("session owner mismatch {id}"))
        );
        assert!(control.ended_ids.is_empty());
        assert_eq!(
            end_exact_session(&mut control, &mut store, id, &descriptor.owner).unwrap(),
            Some(tab.tab_id.clone())
        );
        assert_eq!(control.ended_ids, [id]);
        assert!(store.states()[0].tab(&tab.tab_id).is_none());
        let reloaded = WorkspaceStore::load_from(temp.path().join("workspaces.json"));
        assert!(reloaded.states()[0].tab(&tab.tab_id).is_none());
    }

    #[test]
    fn session_manager_end_cleans_owner_changed_daemon_and_workspace_link() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let tab = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new()).remove(0);
        let id = session_id("E23E4567-E89B-12D3-A456-426614174000");
        store.states_mut()[0]
            .tab_mut(&tab.tab_id)
            .unwrap()
            .session_id = Some(id);
        store.save().unwrap();
        let mut request = create_request(&contract(temp.path()), &tab).unwrap();
        request.session_id = id;
        request.owner.worktree_id = muxy_core::store::new_uuid();
        let descriptor = descriptor(&request, SessionStatus::Running);
        let mut control = FakeControl {
            sessions: vec![descriptor.clone()],
            ..Default::default()
        };

        assert_eq!(
            end_exact_session(&mut control, &mut store, id, &descriptor.owner).unwrap(),
            Some(tab.tab_id.clone())
        );
        assert_eq!(control.ended_ids, [id]);
        assert!(store.states()[0].tab(&tab.tab_id).is_none());
    }

    #[test]
    fn session_manager_end_all_rejects_a_changed_confirmed_set_before_signaling() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let tab = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new()).remove(0);
        let mut request = create_request(&contract(temp.path()), &tab).unwrap();
        request.session_id = session_id("C23E4567-E89B-12D3-A456-426614174000");
        let first = descriptor(&request, SessionStatus::Running);
        let plan = SessionEndAllPlan {
            candidates: vec![SessionEndCandidate {
                session_id: first.session_id,
                owner: first.owner.clone(),
            }],
        };
        let mut added_request = request;
        added_request.session_id = session_id("D23E4567-E89B-12D3-A456-426614174000");
        added_request.owner.original_tab_id = muxy_core::store::new_uuid();
        let mut control = FakeControl {
            sessions: vec![first, descriptor(&added_request, SessionStatus::Running)],
            ..Default::default()
        };

        assert_eq!(
            end_session_plan(&mut control, &mut store, &plan),
            Err("the active session set changed after confirmation".to_owned())
        );
        assert!(control.ended_ids.is_empty());
    }

    #[test]
    fn session_manager_remove_closes_only_the_selected_stale_link() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let tab_ids = store.states()[0]
            .root
            .as_ref()
            .unwrap()
            .tabs()
            .iter()
            .map(|tab| tab.id.clone())
            .collect::<Vec<_>>();

        remove_stale_workspace_link(&mut store, &tab_ids[0]).unwrap();
        assert!(store.states()[0].tab(&tab_ids[0]).is_none());
        assert!(store.states()[0].tab(&tab_ids[1]).is_some());
        let reloaded = WorkspaceStore::load_from(temp.path().join("workspaces.json"));
        assert!(reloaded.states()[0].tab(&tab_ids[0]).is_none());
        assert!(reloaded.states()[0].tab(&tab_ids[1]).is_some());
    }

    #[test]
    fn session_lifecycle_background_and_reattach_preserve_identity_and_startup_command() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let mut store = store(temp.path(), &project_id, &worktree_id);
        let mut launches = HashMap::new();
        let tab_id = store.states()[0].root.as_ref().unwrap().tabs()[0]
            .id
            .clone();
        launches.insert(
            tab_id.clone(),
            PendingSessionLaunch {
                directory: temp.path().to_path_buf(),
                command: Some(muxy_terminal::backend::LaunchCommand {
                    command: "printf once".to_owned(),
                    keeps_shell_open: true,
                }),
            },
        );
        let tab = eligible_tabs(&[project(&project_id, false)], &store, &launches)
            .into_iter()
            .find(|tab| tab.tab_id == tab_id)
            .unwrap();
        let request = create_request(&contract(temp.path()), &tab).unwrap();
        assert_eq!(request.startup_command.as_deref(), Some("printf once"));
        let descriptor = descriptor(&request, SessionStatus::Running);
        let mut control = FakeControl {
            sessions: vec![descriptor.clone()],
            ..Default::default()
        };

        exact_running_session(&mut control, descriptor.session_id, &descriptor.owner).unwrap();
        control.clear_placement(descriptor.session_id).unwrap();
        assert!(control.sessions[0].placement.is_none());
        assert_eq!(
            store.states_mut()[0].close_tab(&tab_id, CloseMode::Single),
            vec![tab_id.clone()]
        );
        store.save().unwrap();

        let outcome = reattach_session(&mut control, &mut store, &descriptor).unwrap();
        let SessionReattachOutcome::Reattached(placement) = outcome else {
            panic!("detached session was not reattached");
        };
        assert_eq!(placement.tab_id, descriptor.owner.original_tab_id);
        assert_eq!(control.sessions[0].placement, Some(placement.clone()));
        assert_eq!(
            store.states()[0]
                .tab(&placement.tab_id)
                .and_then(|tab| tab.session_id),
            Some(descriptor.session_id)
        );
        assert!(matches!(
            reattach_session(&mut control, &mut store, &descriptor).unwrap(),
            SessionReattachOutcome::Focused(focused) if focused == placement
        ));
        assert!(control.created.is_empty());
    }

    #[test]
    fn session_lifecycle_external_owner_truth_requires_two_fresh_missing_observations() {
        use muxy_api::truth::{OwnerExistence, OwnerExistenceFact};

        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let scope = SessionOwnerScope {
            project_id: project_id.clone(),
            worktree_id: Some(worktree_id.clone()),
        };
        let fact = |request_id, existence| OwnerExistenceFact {
            project_id: project_id.clone(),
            worktree_id: Some(worktree_id.clone()),
            path: "/tmp/missing".to_owned(),
            generation: 7,
            request_id,
            existence,
        };
        let mut observations = HashMap::new();
        assert!(
            confirmed_missing_scopes(&mut observations, &[fact(1, OwnerExistence::Missing)])
                .is_empty()
        );
        assert_eq!(
            confirmed_missing_scopes(&mut observations, &[fact(2, OwnerExistence::Missing)]),
            vec![scope.clone()]
        );
        assert!(
            confirmed_missing_scopes(&mut observations, &[fact(1, OwnerExistence::Missing)])
                .is_empty()
        );
        assert!(
            confirmed_missing_scopes(&mut observations, &[fact(3, OwnerExistence::Unknown)])
                .is_empty()
        );
        assert!(
            confirmed_missing_scopes(&mut observations, &[fact(4, OwnerExistence::Missing)])
                .is_empty()
        );
        assert!(
            confirmed_missing_scopes(&mut observations, &[fact(5, OwnerExistence::Present)])
                .is_empty()
        );
    }

    #[test]
    fn session_lifecycle_exact_owner_cleanup_never_ends_unrelated_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let other_worktree_id = muxy_core::store::new_uuid();
        let store = store(temp.path(), &project_id, &worktree_id);
        let mut tabs = eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new());
        let first = tabs.remove(0);
        let mut first_request = create_request(&contract(temp.path()), &first).unwrap();
        first_request.session_id = session_id("723E4567-E89B-12D3-A456-426614174000");
        let mut other_request = first_request.clone();
        other_request.session_id = session_id("823E4567-E89B-12D3-A456-426614174000");
        other_request.owner.worktree_id = other_worktree_id;
        let mut control = FakeControl {
            sessions: vec![
                descriptor(&first_request, SessionStatus::Running),
                descriptor(&other_request, SessionStatus::Running),
            ],
            ..Default::default()
        };
        let ended = end_owner_sessions(
            &mut control,
            &[SessionOwnerScope {
                project_id,
                worktree_id: Some(worktree_id),
            }],
        )
        .unwrap();
        assert_eq!(ended, vec![first_request.session_id]);
        assert_eq!(control.ended_ids, vec![first_request.session_id]);
        assert_eq!(control.sessions[1].status, SessionStatus::Running);
    }

    #[test]
    fn session_lifecycle_owner_cleanup_rejects_candidates_added_after_confirmation() {
        let temp = tempfile::tempdir().unwrap();
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let store = store(temp.path(), &project_id, &worktree_id);
        let first =
            eligible_tabs(&[project(&project_id, false)], &store, &HashMap::new()).remove(0);
        let mut first_request = create_request(&contract(temp.path()), &first).unwrap();
        first_request.session_id = session_id("923E4567-E89B-12D3-A456-426614174000");
        let mut control = FakeControl {
            sessions: vec![descriptor(&first_request, SessionStatus::Running)],
            ..Default::default()
        };
        let scope = SessionOwnerScope {
            project_id,
            worktree_id: Some(worktree_id),
        };
        let plan = SessionOwnerCleanupPlan {
            scope: scope.clone(),
            candidates: owner_cleanup_candidates(&mut control, &scope).unwrap(),
        };
        let mut added_request = first_request.clone();
        added_request.session_id = session_id("A23E4567-E89B-12D3-A456-426614174000");
        added_request.owner.original_tab_id = muxy_core::store::new_uuid();
        control
            .sessions
            .push(descriptor(&added_request, SessionStatus::Running));
        assert!(end_owner_cleanup_candidates(&mut control, &plan).is_err());
        assert!(control.ended_ids.is_empty());
        assert!(
            control
                .sessions
                .iter()
                .all(|session| session.status == SessionStatus::Running)
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
                owner_observations: HashMap::new(),
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
                owner_observations: HashMap::new(),
            }
            .is_ready()
        );
    }
}
