pub mod client;
pub mod recovery;

use crate::resources::AppResources;
use crate::state::AppState;
use muxy_core::environment::{BuildMode, RuntimePathPolicy};
use muxy_core::workspace::{CloseMode, Tab, TabKind};
use muxy_core::workspace_store::TerminalIdentityProblem;
use muxy_proto::session::SessionDescriptor;
use muxy_terminal::offline::RecoveryState;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const PERSISTENT_SESSION_SETTING: &str = "muxy.terminalPersistentSession.enabled";
const TEST_SESSION_SOCKET_PATH: &str = "MUXY_TEST_P8_SESSION_SOCKET_PATH";

#[derive(Clone, Debug)]
pub struct SessionConfiguration {
    pub executable: PathBuf,
    pub resources_directory: PathBuf,
    pub socket_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct PersistentStartup {
    pub enabled: bool,
    pub configuration: Option<SessionConfiguration>,
    pub client: Option<client::SessionClient>,
    pub eligible_tabs: HashSet<String>,
    pub remote_projects: HashSet<String>,
    pub recovery: HashMap<String, RecoveryState>,
}

impl PersistentStartup {
    pub fn disabled(remote_projects: HashSet<String>) -> Self {
        Self {
            enabled: false,
            configuration: None,
            client: None,
            eligible_tabs: HashSet::new(),
            remote_projects,
            recovery: HashMap::new(),
        }
    }
}

pub fn prepare_startup(state: &mut AppState, mode: BuildMode) -> PersistentStartup {
    let remote_projects = state
        .workspace
        .projects
        .iter()
        .filter(|project| project.is_remote())
        .map(|project| project.id.to_ascii_uppercase())
        .collect::<HashSet<_>>();
    if !muxy_core::prefs::settings::bool_value(PERSISTENT_SESSION_SETTING, false) {
        return PersistentStartup::disabled(remote_projects);
    }
    let configuration =
        match SessionConfiguration::resolve(mode, &muxy_core::prefs::app_support_dir()) {
            Ok(configuration) => configuration,
            Err(error) => return unavailable_startup(state, error),
        };
    let client = client::SessionClient::new(configuration.socket_path.clone());
    let inventory = client.recover();
    let descriptors = match &inventory {
        client::InventoryOutcome::Available(descriptors) => descriptors.clone(),
        client::InventoryOutcome::Unreachable(_) => Vec::new(),
    };
    reconcile_ownerless(state, &descriptors);
    let inventory_available = matches!(inventory, client::InventoryOutcome::Available(_));
    let found = descriptors
        .iter()
        .map(|descriptor| descriptor.session_id.clone())
        .collect::<HashSet<_>>();
    let blocked = blocked_identities(state);
    let mut eligible_tabs = HashSet::new();
    let mut recovery = HashMap::new();
    for workspace in state.tab_workspaces.states() {
        let remote = state
            .workspace
            .project(&workspace.project_id)
            .is_some_and(|project| project.is_remote());
        for tab in workspace
            .root
            .as_ref()
            .map(|root| root.tabs())
            .unwrap_or_default()
        {
            if tab.kind != TabKind::Terminal || remote {
                continue;
            }
            if let Some(reason) = blocked.get(&tab.id) {
                recovery.insert(tab.id.clone(), recovery::blocked_identity(reason.clone()));
                continue;
            }
            eligible_tabs.insert(tab.id.clone());
            recovery.insert(
                tab.id.clone(),
                recovery::startup_state(
                    tab.rust_persistent_session,
                    inventory_available,
                    found.contains(&tab.id),
                ),
            );
        }
    }
    PersistentStartup {
        enabled: true,
        configuration: Some(configuration),
        client: Some(client),
        eligible_tabs,
        remote_projects,
        recovery,
    }
}

fn unavailable_startup(state: &AppState, error: String) -> PersistentStartup {
    let mut recovery = HashMap::new();
    let mut eligible_tabs = HashSet::new();
    let blocked = blocked_identities(state);
    for workspace in state.tab_workspaces.states() {
        let remote = state
            .workspace
            .project(&workspace.project_id)
            .is_some_and(|project| project.is_remote());
        if remote {
            continue;
        }
        for tab in workspace
            .root
            .as_ref()
            .map(|root| root.tabs())
            .unwrap_or_default()
        {
            if tab.kind == TabKind::Terminal {
                if let Some(reason) = blocked.get(&tab.id) {
                    recovery.insert(tab.id.clone(), recovery::blocked_identity(reason.clone()));
                } else {
                    eligible_tabs.insert(tab.id.clone());
                    recovery.insert(tab.id.clone(), RecoveryState::Unreachable);
                }
            }
        }
    }
    log::warn!("persistent session startup unavailable: {error}");
    PersistentStartup {
        enabled: true,
        configuration: None,
        client: None,
        eligible_tabs,
        remote_projects: state
            .workspace
            .projects
            .iter()
            .filter(|project| project.is_remote())
            .map(|project| project.id.to_ascii_uppercase())
            .collect(),
        recovery,
    }
}

fn blocked_identities(state: &AppState) -> HashMap<String, String> {
    let mut blocked = HashMap::new();
    for issue in state.tab_workspaces.terminal_identity_issues() {
        let problem = match issue.problem {
            TerminalIdentityProblem::Malformed => "is not a canonical uppercase UUID",
            TerminalIdentityProblem::Duplicate => "is duplicated across terminal tabs",
        };
        blocked.insert(
            issue.tab_id.clone(),
            format!("Terminal ID {} {problem}.", issue.tab_id),
        );
    }
    blocked
}

fn reconcile_ownerless(state: &mut AppState, descriptors: &[SessionDescriptor]) {
    let existing = state
        .tab_workspaces
        .states()
        .iter()
        .flat_map(|workspace| {
            workspace
                .root
                .as_ref()
                .map(|root| root.tabs())
                .unwrap_or_default()
        })
        .map(|tab| tab.id.clone())
        .collect::<HashSet<_>>();
    let ownerless = descriptors
        .iter()
        .filter(|descriptor| !existing.contains(&descriptor.session_id))
        .cloned()
        .collect::<Vec<_>>();
    if ownerless.is_empty() {
        return;
    }
    let workspace_snapshot = state.workspace.clone();
    let tab_snapshot = state.tab_workspaces.clone();
    for descriptor in ownerless {
        insert_recovered_tab(state, descriptor);
    }
    if let Err(error) = state.persist_tab_workspaces() {
        state.workspace = workspace_snapshot;
        state.tab_workspaces = tab_snapshot;
        log::warn!("failed to publish recovered terminal owners: {error}");
    }
}

fn insert_recovered_tab(state: &mut AppState, descriptor: SessionDescriptor) {
    let target = recovery_target(state, &descriptor);
    if target
        .0
        .eq_ignore_ascii_case(muxy_core::store::HOME_PROJECT_ID)
    {
        state.workspace.ensure_home();
    }
    let existed = match target.1.as_deref() {
        Some(worktree_id) => state
            .tab_workspaces
            .worktree(&target.0, worktree_id)
            .is_some(),
        None => state.tab_workspaces.project(&target.0).is_some(),
    };
    let workspace = match target.1.as_deref() {
        Some(worktree_id) => {
            state
                .tab_workspaces
                .ensure_worktree(&target.0, worktree_id, &target.2)
        }
        None => state
            .tab_workspaces
            .ensure_project(target.0.clone(), target.2.clone()),
    };
    if !existed {
        let roots = workspace.root_tab_ids();
        for root in roots {
            workspace.close_tab(&root, CloseMode::Single);
        }
    }
    let mut tab = Tab::new(TabKind::Terminal);
    tab.id = descriptor.session_id;
    tab.project_path = Some(target.2);
    tab.pane_title = (!descriptor.owner.title.is_empty()).then_some(descriptor.owner.title);
    tab.rust_persistent_session = true;
    tab.terminal_resume_directory = absolute_directory(&descriptor.working_directory);
    workspace.new_top_level_tab(tab);
}

fn recovery_target(
    state: &AppState,
    descriptor: &SessionDescriptor,
) -> (String, Option<String>, String) {
    let project = state
        .workspace
        .project(&descriptor.owner.project_id)
        .filter(|project| !project.is_remote());
    if let Some(project) = project {
        if let Some(worktree_id) = descriptor.owner.worktree_id.as_deref()
            && let Some(workspace) = state
                .tab_workspaces
                .worktree(&descriptor.owner.project_id, worktree_id)
        {
            return (
                workspace.project_id.clone(),
                workspace.worktree_id.clone(),
                workspace
                    .worktree_path
                    .clone()
                    .unwrap_or_else(|| project.path.clone()),
            );
        }
        if let Some(workspace) = state.tab_workspaces.project(&descriptor.owner.project_id) {
            return (
                workspace.project_id.clone(),
                workspace.worktree_id.clone(),
                workspace
                    .worktree_path
                    .clone()
                    .unwrap_or_else(|| project.path.clone()),
            );
        }
        return (project.id.clone(), None, project.path.clone());
    }
    (
        muxy_core::store::HOME_PROJECT_ID.to_owned(),
        None,
        muxy_core::prefs::home_dir().to_string_lossy().into_owned(),
    )
}

fn absolute_directory(directory: &str) -> Option<String> {
    Path::new(directory)
        .is_absolute()
        .then(|| directory.to_owned())
}

impl SessionConfiguration {
    pub fn resolve(mode: BuildMode, app_support: &Path) -> Result<Self, String> {
        let resources = AppResources::discover().map_err(|error| error.to_string())?;
        let executable = resources
            .session_executable()
            .map_err(|error| error.to_string())?;
        let policy = RuntimePathPolicy::new(mode);
        let injected_socket = (muxy_core::prefs::is_test_process()
            && std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").is_some())
        .then(|| std::env::var_os(TEST_SESSION_SOCKET_PATH))
        .flatten()
        .map(PathBuf::from);
        let preferred = policy.preferred_session_socket_path(app_support);
        let socket_path = if let Some(injected) = injected_socket {
            if !injected.is_absolute() || !unix_socket_path_fits(&injected) {
                return Err("injected session socket path is invalid".to_owned());
            }
            injected
        } else if unix_socket_path_fits(&preferred) {
            preferred
        } else {
            let fallback_root = std::env::temp_dir()
                .canonicalize()
                .map_err(|error| format!("failed to resolve temporary directory: {error}"))?;
            let uid = unsafe { libc::geteuid() };
            let fallback = policy.fallback_session_socket_path(fallback_root, uid);
            if !unix_socket_path_fits(&fallback) {
                return Err(format!(
                    "session socket path exceeds the platform limit: {}",
                    fallback.display()
                ));
            }
            fallback
        };
        Ok(Self {
            executable,
            resources_directory: resources.root,
            socket_path,
        })
    }
}

fn unix_socket_path_fits(path: &Path) -> bool {
    os_bytes(path.as_os_str()).len() < unix_socket_limit()
}

fn os_bytes(value: &OsStr) -> &[u8] {
    value.as_bytes()
}

const fn unix_socket_limit() -> usize {
    if cfg!(target_os = "macos") { 104 } else { 108 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_boundary_is_strict() {
        let limit = unix_socket_limit();
        let fitting = PathBuf::from(format!("/{}", "a".repeat(limit - 2)));
        let oversized = PathBuf::from(format!("/{}", "a".repeat(limit - 1)));
        assert!(unix_socket_path_fits(&fitting));
        assert!(!unix_socket_path_fits(&oversized));
    }
}
