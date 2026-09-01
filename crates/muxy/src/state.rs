use crate::socket::ingress::IngressQueues;
use gpui::{App, WindowAppearance};
use muxy_core::composer::ComposerStore;
use muxy_core::navigation::{Direction, NavigationEntry, NavigationHistory};
use muxy_core::notifications::{NotificationStore, NotificationTarget};
use muxy_core::prefs::Prefs;
use muxy_core::shortcuts::ShortcutMap;
use muxy_core::store::{CommandShortcuts, Project, Workspace, Worktree};
use muxy_core::workspace::WorkspaceState;
use muxy_core::workspace_store::WorkspaceStore;
use muxy_ui::theme::{Appearance, Metrics, Theme};
use std::collections::HashMap;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum NavigationApplyError {
    WorkspacePersistence(io::Error),
    PreferencePersistence(io::Error),
}

pub struct CreationEffects {
    pub warnings: Vec<muxy_api::worktree_lifecycle::LifecycleWarning>,
    pub navigation_recorded: bool,
}

pub struct RemovalEffects {
    pub tab_ids: Vec<String>,
    pub warnings: Vec<muxy_api::worktree_lifecycle::LifecycleWarning>,
    pub navigation_recorded: bool,
}

impl std::fmt::Display for NavigationApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WorkspacePersistence(error) => write!(formatter, "{error}"),
            Self::PreferencePersistence(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for NavigationApplyError {}

fn load_notification_store() -> NotificationStore {
    load_notification_store_from(muxy_core::prefs::app_support_dir().join("notifications.json"))
}

fn load_notification_store_from(path: impl Into<std::path::PathBuf>) -> NotificationStore {
    let mut store = NotificationStore::load_from(path);
    if store.mark_all_read()
        && let Err(error) = store.flush()
    {
        log::warn!("failed to clear retained notification unread state: {error}");
    }
    store
}

pub(crate) fn load_composer_store() -> ComposerStore {
    load_composer_store_from(muxy_core::prefs::app_support_dir())
}

fn load_composer_store_from(path: impl Into<std::path::PathBuf>) -> ComposerStore {
    let store = ComposerStore::load_from(path);
    for warning in &store.load_status().warnings {
        log::warn!("{warning}");
    }
    if !store.load_status().malformed_keys.is_empty() {
        log::warn!(
            "malformed Composer drafts preserved: {}",
            store.load_status().malformed_keys.join(", ")
        );
    }
    store
}

fn p7_composer_status_path(
    is_test_process: bool,
    case_name: Option<&str>,
    app_support: &Path,
    injected_app_support: Option<&Path>,
    home: &Path,
) -> Option<std::path::PathBuf> {
    if !is_test_process
        || !matches!(case_name, Some("phase-2" | "persistence"))
        || injected_app_support != Some(app_support)
        || !app_support.is_absolute()
        || app_support.starts_with(home)
        || !std::fs::symlink_metadata(app_support)
            .ok()
            .is_some_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        return None;
    }
    Some(app_support.join(".muxy-p7-composer-status.json"))
}

fn current_p7_composer_status_path() -> Option<std::path::PathBuf> {
    let app_support = muxy_core::prefs::app_support_dir();
    let injected =
        std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").map(std::path::PathBuf::from);
    let case_name = std::env::var("MUXY_TEST_P7_COMPOSER_CASE").ok();
    p7_composer_status_path(
        muxy_core::prefs::is_test_process(),
        case_name.as_deref(),
        &app_support,
        injected.as_deref(),
        &muxy_core::prefs::home_dir(),
    )
}

pub(crate) fn p7_composer_status_enabled() -> bool {
    current_p7_composer_status_path().is_some()
}

pub(crate) fn write_p7_composer_status(store: &ComposerStore) {
    let Some(path) = current_p7_composer_status_path() else {
        return;
    };
    let image_files = store
        .image_storage()
        .and_then(|storage| storage.regular_file_names().ok())
        .unwrap_or_default();
    let value = serde_json::json!({
        "draftCount": store.drafts().count(),
        "overwriteBlocked": store.load_status().overwrite_blocked,
        "malformedKeys": store.load_status().malformed_keys,
        "warnings": store.load_status().warnings,
        "imageFiles": image_files,
    });
    match serde_json::to_vec_pretty(&value) {
        Ok(contents) => {
            if let Err(error) = muxy_core::store::write_private(&path, &contents) {
                log::warn!("failed to write P7 Composer status: {error}");
            }
        }
        Err(error) => log::warn!("failed to encode P7 Composer status: {error}"),
    }
}

pub(crate) fn appearance_for_window(appearance: WindowAppearance) -> Appearance {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
        _ => Appearance::Dark,
    }
}

pub struct AppState {
    pub prefs: Prefs,
    pub theme: Theme,
    pub metrics: Metrics,
    pub workspace: Workspace,
    pub tab_workspaces: WorkspaceStore,
    pub shortcuts: ShortcutMap,
    pub command_shortcuts: CommandShortcuts,
    pub worktrees: HashMap<String, Vec<Worktree>>,
    pub project_operations: crate::project_operations::ProjectOperations,
    pub navigation: NavigationHistory,
    pub(crate) navigation_recording_suppressed: bool,
    pub socket_ingress: IngressQueues,
    pub notification_store: NotificationStore,
    pub active_project_id: Option<String>,
    pub ide_name: Option<String>,
    pub appearance: Appearance,
}

impl AppState {
    pub fn load(cx: &App) -> Self {
        let mut prefs = Prefs::load();
        let appearance = appearance_for_window(cx.window_appearance());
        let theme = match appearance {
            Appearance::Light => crate::themes::load(&prefs.light_theme, "Muxy Light"),
            Appearance::Dark => crate::themes::load(&prefs.dark_theme, "Muxy"),
        };
        let workspace = Workspace::load(&prefs);
        let previous_active_worktree_ids = prefs.active_worktree_ids.clone();
        let mut tab_workspaces = WorkspaceStore::load();
        let previous_tab_workspaces = tab_workspaces.clone();
        let mut worktrees = HashMap::new();
        let mut active_worktrees_changed = false;
        for project in &workspace.projects {
            let loaded = (!project.is_remote())
                .then(|| {
                    muxy_api::worktrees::load_or_create_primary(
                        &project.id,
                        &project.name,
                        &project.path,
                    )
                })
                .flatten();
            let Some(loaded) = loaded else {
                tab_workspaces.ensure_project(project.id.clone(), project.path.clone());
                continue;
            };
            let selected = prefs
                .active_worktree_ids
                .get(&project.id)
                .and_then(|id| {
                    loaded
                        .iter()
                        .find(|worktree| worktree.id.eq_ignore_ascii_case(id))
                })
                .or_else(|| loaded.iter().find(|worktree| worktree.is_primary))
                .or_else(|| loaded.first());
            if let Some(selected) = selected {
                tab_workspaces.ensure_worktree(&project.id, &selected.id, &selected.path);
                if prefs.active_worktree_ids.get(&project.id) != Some(&selected.id) {
                    prefs
                        .active_worktree_ids
                        .insert(project.id.clone(), selected.id.clone());
                    active_worktrees_changed = true;
                }
            } else {
                tab_workspaces.ensure_project(project.id.clone(), project.path.clone());
            }
            worktrees.insert(project.id.clone(), loaded);
        }
        let startup_persistence_succeeded = match tab_workspaces.save() {
            Ok(()) if active_worktrees_changed => {
                Prefs::store_active_worktree_ids(&prefs.active_worktree_ids);
                true
            }
            Ok(()) => true,
            Err(error) => {
                log::warn!("failed to save startup worktree workspaces: {error}");
                prefs.active_worktree_ids = previous_active_worktree_ids;
                tab_workspaces = previous_tab_workspaces;
                false
            }
        };
        let shortcuts = ShortcutMap::load();
        let command_shortcuts = CommandShortcuts::load();
        let notification_store = load_notification_store();
        let active_project_id = workspace.resolve_active(&prefs);
        let ide_name = prefs
            .ide_bundle_identifier
            .as_deref()
            .and_then(muxy_api::ide::display_name);

        let mut state = Self {
            metrics: Metrics::new(prefs.scale.multiplier()),
            prefs,
            theme,
            workspace,
            tab_workspaces,
            shortcuts,
            command_shortcuts,
            worktrees,
            project_operations: crate::project_operations::ProjectOperations::default(),
            navigation: NavigationHistory::default(),
            navigation_recording_suppressed: false,
            socket_ingress: IngressQueues::default(),
            notification_store,
            active_project_id,
            ide_name,
            appearance,
        };
        if startup_persistence_succeeded && let Some(entry) = state.current_navigation_entry() {
            state.navigation.record(entry);
        }
        state
    }

    pub fn apply_truth(&mut self, truth: Vec<muxy_api::truth::ProjectTruth>) -> io::Result<()> {
        let mut active_worktrees_changed = false;
        let mut tab_workspaces_changed = false;
        let previous_workspace = self.workspace.clone();
        let previous_worktrees = self.worktrees.clone();
        let previous_tab_workspaces = self.tab_workspaces.clone();
        let previous_active_worktree_ids = self.prefs.active_worktree_ids.clone();
        for entry in truth {
            let Some(project) = self
                .workspace
                .projects
                .iter_mut()
                .find(|project| project.id == entry.project_id)
            else {
                continue;
            };
            project.is_git_repo = entry.is_git_repo;
            project.worktree_label = entry.worktree_label;
            let Some(worktrees) = entry.worktrees else {
                continue;
            };
            let selected = self
                .prefs
                .active_worktree_ids
                .get(&entry.project_id)
                .and_then(|id| {
                    worktrees
                        .iter()
                        .find(|worktree| worktree.id.eq_ignore_ascii_case(id))
                })
                .or_else(|| worktrees.iter().find(|worktree| worktree.is_primary))
                .or_else(|| worktrees.first());
            match selected {
                Some(selected) => {
                    self.tab_workspaces.ensure_worktree(
                        &entry.project_id,
                        &selected.id,
                        &selected.path,
                    );
                    tab_workspaces_changed = true;
                    if self.prefs.active_worktree_ids.get(&entry.project_id) != Some(&selected.id) {
                        self.prefs
                            .active_worktree_ids
                            .insert(entry.project_id.clone(), selected.id.clone());
                        active_worktrees_changed = true;
                    }
                }
                None => {
                    if self
                        .prefs
                        .active_worktree_ids
                        .remove(&entry.project_id)
                        .is_some()
                    {
                        active_worktrees_changed = true;
                    }
                }
            }
            self.worktrees.insert(entry.project_id, worktrees);
        }
        if tab_workspaces_changed && let Err(error) = self.persist_tab_workspaces() {
            self.workspace = previous_workspace;
            self.worktrees = previous_worktrees;
            self.tab_workspaces = previous_tab_workspaces;
            self.prefs.active_worktree_ids = previous_active_worktree_ids;
            return Err(error);
        }
        if active_worktrees_changed {
            Prefs::store_active_worktree_ids(&self.prefs.active_worktree_ids);
        }
        Ok(())
    }

    pub fn apply_created_worktree(
        &mut self,
        project_id: &str,
        outcome: muxy_api::worktree_lifecycle::CreateWorktreeOutcome,
    ) -> CreationEffects {
        self.apply_created_worktree_with(project_id, outcome, |active_worktrees| {
            Prefs::try_store_active_worktree_ids(active_worktrees)
        })
    }

    pub fn apply_removed_worktree(
        &mut self,
        project_id: &str,
        outcome: muxy_api::worktree_lifecycle::RemoveWorktreeOutcome,
    ) -> RemovalEffects {
        self.apply_removed_worktree_with(
            project_id,
            outcome,
            |store| store.save(),
            Prefs::try_store_active_worktree_ids,
        )
    }

    fn apply_removed_worktree_with<SaveWorkspaces, SavePreferences>(
        &mut self,
        project_id: &str,
        outcome: muxy_api::worktree_lifecycle::RemoveWorktreeOutcome,
        save_workspaces: SaveWorkspaces,
        save_preferences: SavePreferences,
    ) -> RemovalEffects
    where
        SaveWorkspaces: FnOnce(&WorkspaceStore) -> io::Result<()>,
        SavePreferences: FnOnce(&HashMap<String, String>) -> io::Result<()>,
    {
        let mut warnings = outcome.warnings;
        let removed = outcome.removed;
        let tab_ids = self
            .tab_workspaces
            .worktree(project_id, &removed.id)
            .and_then(|workspace| workspace.root.as_ref())
            .map(|root| root.tabs().iter().map(|tab| tab.id.clone()).collect())
            .unwrap_or_default();
        self.tab_workspaces.remove_worktree(project_id, &removed.id);
        self.prune_navigation_worktree(project_id, &removed.id);
        self.worktrees
            .insert(project_id.to_owned(), outcome.worktrees.clone());

        let selected = self.prefs.active_worktree_ids.get(project_id).cloned();
        let replacement = selected
            .as_ref()
            .and_then(|selected| {
                outcome
                    .worktrees
                    .iter()
                    .find(|worktree| worktree.id.eq_ignore_ascii_case(selected))
            })
            .or_else(|| {
                outcome
                    .worktrees
                    .iter()
                    .find(|worktree| worktree.is_primary)
            })
            .or_else(|| outcome.worktrees.first())
            .cloned();
        match &replacement {
            Some(worktree) => {
                self.tab_workspaces
                    .ensure_worktree(project_id, &worktree.id, &worktree.path);
                self.prefs
                    .active_worktree_ids
                    .insert(project_id.to_owned(), worktree.id.clone());
            }
            None => {
                self.prefs.active_worktree_ids.remove(project_id);
            }
        }
        if let Some(project) = self
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id.eq_ignore_ascii_case(project_id))
        {
            project.worktree_label = replacement.as_ref().map(|worktree| {
                if worktree.is_primary {
                    "primary".to_owned()
                } else {
                    worktree.name.clone()
                }
            });
        }

        let navigation_recorded = match save_workspaces(&self.tab_workspaces) {
            Ok(()) => {
                self.prune_navigation_history();
                if !self.navigation_recording_suppressed
                    && let Some(entry) = self.current_navigation_entry()
                {
                    self.navigation.record(entry);
                }
                true
            }
            Err(error) => {
                warnings.push(
                    muxy_api::worktree_lifecycle::LifecycleWarning::WorkspacePersistence(
                        error.to_string(),
                    ),
                );
                false
            }
        };
        if let Err(error) = save_preferences(&self.prefs.active_worktree_ids) {
            warnings.push(
                muxy_api::worktree_lifecycle::LifecycleWarning::ActivePreferencePersistence(
                    error.to_string(),
                ),
            );
        }
        RemovalEffects {
            tab_ids,
            warnings,
            navigation_recorded,
        }
    }

    fn apply_created_worktree_with<F>(
        &mut self,
        project_id: &str,
        outcome: muxy_api::worktree_lifecycle::CreateWorktreeOutcome,
        store_active_worktrees: F,
    ) -> CreationEffects
    where
        F: FnOnce(&HashMap<String, String>) -> io::Result<()>,
    {
        let mut warnings = outcome.warnings;
        let created = outcome.worktree;
        self.worktrees
            .insert(project_id.to_owned(), outcome.worktrees);
        self.tab_workspaces
            .ensure_worktree(project_id, &created.id, &created.path);
        self.prefs
            .active_worktree_ids
            .insert(project_id.to_owned(), created.id.clone());
        self.active_project_id = Some(project_id.to_owned());
        self.prefs.active_project_id = Some(project_id.to_owned());
        if let Some(project) = self
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id.eq_ignore_ascii_case(project_id))
        {
            project.worktree_label = Some(created.name.clone());
        }
        self.workspace.activate_group_for_project(project_id);
        let navigation_recorded = match self.persist_tab_workspaces() {
            Ok(()) => true,
            Err(error) => {
                warnings.push(
                    muxy_api::worktree_lifecycle::LifecycleWarning::WorkspacePersistence(
                        error.to_string(),
                    ),
                );
                false
            }
        };
        if let Err(error) = store_active_worktrees(&self.prefs.active_worktree_ids) {
            warnings.push(
                muxy_api::worktree_lifecycle::LifecycleWarning::ActivePreferencePersistence(
                    error.to_string(),
                ),
            );
        }
        Prefs::store_default("muxy.activeProjectID", Some(project_id));
        CreationEffects {
            warnings,
            navigation_recorded,
        }
    }

    pub fn active_project(&self) -> Option<&Project> {
        let id = self.active_project_id.as_ref()?;
        self.workspace
            .projects
            .iter()
            .find(|project| &project.id == id)
    }

    pub fn active_worktree_path(&self, project: &Project) -> String {
        let Some(worktree_id) = self.prefs.active_worktree_ids.get(&project.id) else {
            return project.path.clone();
        };
        if let Some(worktrees) = self.worktrees.get(&project.id) {
            return worktrees
                .iter()
                .find(|worktree| {
                    worktree.id.eq_ignore_ascii_case(worktree_id) && !worktree.path.is_empty()
                })
                .map(|worktree| worktree.path.clone())
                .unwrap_or_else(|| project.path.clone());
        }
        self.tab_workspaces
            .states()
            .iter()
            .find(|workspace| {
                workspace.project_id.eq_ignore_ascii_case(&project.id)
                    && workspace
                        .worktree_id
                        .as_deref()
                        .is_some_and(|id| id.eq_ignore_ascii_case(worktree_id))
            })
            .and_then(|workspace| workspace.worktree_path.clone())
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| project.path.clone())
    }

    pub(crate) fn active_repository_key(&self) -> Option<crate::repository::RepositoryKey> {
        let project = self.active_project()?;
        if project.is_home() || project.is_remote() || !project.is_git_repo {
            return None;
        }
        let path = self.active_worktree_path(project);
        let path = Path::new(&path);
        if !muxy_api::git::is_repository_path(path) {
            return None;
        }
        let normalized_path = std::fs::canonicalize(path).ok()?;
        let worktree_id = self
            .prefs
            .active_worktree_ids
            .get(&project.id)
            .cloned()
            .or_else(|| {
                self.worktrees.get(&project.id).and_then(|worktrees| {
                    worktrees
                        .iter()
                        .find(|worktree| worktree.is_primary)
                        .map(|worktree| worktree.id.clone())
                })
            })
            .unwrap_or_else(|| format!("primary:{}", project.id));
        Some(crate::repository::RepositoryKey {
            project_id: project.id.clone(),
            worktree_id,
            normalized_path,
        })
    }

    pub fn remote_project_ids(&self) -> std::collections::HashSet<String> {
        self.workspace
            .projects
            .iter()
            .filter(|project| project.is_remote())
            .map(|project| project.id.to_ascii_uppercase())
            .collect()
    }

    pub fn active_tab_workspace(&self) -> Option<&WorkspaceState> {
        let project = self.active_project()?;
        if let Some(worktree_id) = self.prefs.active_worktree_ids.get(&project.id) {
            return self.tab_workspaces.worktree(&project.id, worktree_id);
        }
        self.tab_workspaces
            .active(&project.id, &self.active_worktree_path(project))
    }

    pub fn active_tab_workspace_mut(&mut self) -> Option<&mut WorkspaceState> {
        let project = self.active_project()?;
        let id = project.id.clone();
        if let Some(worktree_id) = self.prefs.active_worktree_ids.get(&id).cloned() {
            return self.tab_workspaces.worktree_mut(&id, &worktree_id);
        }
        let path = self.active_worktree_path(project);
        self.tab_workspaces.active_mut(&id, &path)
    }

    pub(crate) fn notification_target_for_pane(&self, pane_id: &str) -> Option<NotificationTarget> {
        let pane_id = muxy_core::notifications::canonical_uuid(pane_id)?;
        self.tab_workspaces.states().iter().find_map(|workspace| {
            workspace
                .tab(&pane_id)
                .and_then(|_| self.notification_target_in_workspace(workspace, &pane_id))
        })
    }

    pub(crate) fn notification_target_is_focused(&self, target: &NotificationTarget) -> bool {
        let Some(workspace) = self.active_tab_workspace() else {
            return false;
        };
        if !workspace
            .project_id
            .eq_ignore_ascii_case(&target.project_id)
            || workspace
                .worktree_id
                .as_deref()
                .is_none_or(|id| !id.eq_ignore_ascii_case(&target.worktree_id))
            || workspace.focused_area_id.as_deref() != Some(target.area_id.as_str())
            || workspace
                .area(&target.area_id)
                .and_then(|area| area.active_tab_id.as_deref())
                != Some(target.pane_id.as_str())
            || workspace.root_id_for_tab(&target.pane_id) != Some(target.tab_id.as_str())
        {
            return false;
        }
        workspace
            .visible_area_tabs()
            .iter()
            .any(|(area_id, tab_id)| {
                area_id.eq_ignore_ascii_case(&target.area_id)
                    && tab_id.eq_ignore_ascii_case(&target.pane_id)
            })
    }

    pub(crate) fn active_notification_target(&self) -> Option<NotificationTarget> {
        let workspace = self.active_tab_workspace()?;
        let area_id = workspace.focused_area_id.as_deref()?;
        let pane_id = workspace.area(area_id)?.active_tab_id.as_deref()?;
        self.notification_target_in_workspace(workspace, pane_id)
    }

    pub(crate) fn active_first_terminal_notification_target(&self) -> Option<NotificationTarget> {
        let workspace = self.active_tab_workspace()?;
        let pane_id = workspace
            .root
            .as_ref()?
            .tabs()
            .into_iter()
            .find(|tab| tab.kind == muxy_core::workspace::TabKind::Terminal)?
            .id
            .clone();
        self.notification_target_in_workspace(workspace, &pane_id)
    }

    fn notification_target_in_workspace(
        &self,
        workspace: &WorkspaceState,
        pane_id: &str,
    ) -> Option<NotificationTarget> {
        let project = self
            .workspace
            .projects
            .iter()
            .find(|project| project.id.eq_ignore_ascii_case(&workspace.project_id))?;
        let worktree_id = workspace.worktree_id.as_deref().or_else(|| {
            self.prefs
                .active_worktree_ids
                .get(&workspace.project_id)
                .map(String::as_str)
        })?;
        let worktree = self
            .worktrees
            .get(&project.id)?
            .iter()
            .find(|worktree| worktree.id.eq_ignore_ascii_case(worktree_id))?;
        let tab = workspace.tab(pane_id)?;
        if tab.kind != muxy_core::workspace::TabKind::Terminal {
            return None;
        }
        let area = workspace.area_containing_tab(pane_id)?;
        NotificationTarget::new(
            pane_id,
            &project.id,
            &worktree.id,
            &area.id,
            tab.root_id(),
            &worktree.path,
        )
    }

    pub fn layouts(&self) -> Vec<muxy_api::layouts::Descriptor> {
        self.active_project()
            .map(|project| muxy_api::layouts::discover(&self.active_worktree_path(project)))
            .unwrap_or_default()
    }

    pub fn try_select_worktree(&mut self, project_id: &str, worktree_id: &str) -> bool {
        let Some(worktree) = self
            .worktrees
            .get(project_id)
            .and_then(|list| {
                list.iter()
                    .find(|worktree| worktree.id.eq_ignore_ascii_case(worktree_id))
            })
            .cloned()
        else {
            return false;
        };
        let previous_tab_workspaces = self.tab_workspaces.clone();
        let previous_workspace = self.workspace.clone();
        let previous_active_project_id = self.active_project_id.clone();
        let previous_prefs_active_project_id = self.prefs.active_project_id.clone();
        let previous_active_worktree_ids = self.prefs.active_worktree_ids.clone();
        self.tab_workspaces
            .ensure_worktree(project_id, &worktree.id, &worktree.path);
        self.prefs
            .active_worktree_ids
            .insert(project_id.to_owned(), worktree.id.clone());
        self.active_project_id = Some(project_id.to_owned());
        self.prefs.active_project_id = Some(project_id.to_owned());
        if let Some(project) = self
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id.eq_ignore_ascii_case(project_id))
        {
            project.worktree_label = Some(if worktree.is_primary {
                "primary".to_owned()
            } else {
                worktree.name.clone()
            });
        }
        self.workspace.activate_group_for_project(project_id);
        if let Err(error) = self.persist_tab_workspaces() {
            log::warn!("failed to save selected worktree workspace: {error}");
            self.tab_workspaces = previous_tab_workspaces;
            self.workspace = previous_workspace;
            self.active_project_id = previous_active_project_id;
            self.prefs.active_project_id = previous_prefs_active_project_id;
            self.prefs.active_worktree_ids = previous_active_worktree_ids;
            return false;
        }
        Prefs::store_active_worktree_ids(&self.prefs.active_worktree_ids);
        Prefs::store_default("muxy.activeProjectID", Some(project_id));
        true
    }

    pub fn select_worktree(&mut self, project_id: &str, worktree_id: &str) {
        self.try_select_worktree(project_id, worktree_id);
    }

    pub fn current_navigation_entry(&self) -> Option<NavigationEntry> {
        let project = self.active_project()?;
        let worktree_id = self.prefs.active_worktree_ids.get(&project.id)?;
        let workspace = self.tab_workspaces.worktree(&project.id, worktree_id)?;
        let area_id = workspace.focused_area_id.as_ref()?;
        let area = workspace.area(area_id)?;
        if area
            .active_tab_id
            .as_deref()
            .is_some_and(|tab_id| !area.tabs.iter().any(|tab| tab.id == tab_id))
        {
            return None;
        }
        Some(NavigationEntry {
            project_id: project.id.clone(),
            worktree_id: worktree_id.clone(),
            area_id: area_id.clone(),
            tab_id: area.active_tab_id.clone(),
        })
    }

    pub fn persist_tab_workspaces(&mut self) -> io::Result<()> {
        self.tab_workspaces.save()?;
        self.prune_navigation_history();
        if !self.navigation_recording_suppressed
            && let Some(entry) = self.current_navigation_entry()
        {
            self.navigation.record(entry);
        }
        Ok(())
    }

    fn prune_navigation_history(&mut self) {
        let tab_workspaces = &self.tab_workspaces;
        let workspace = &self.workspace;
        self.navigation
            .prune(|entry| navigation_entry_is_live(workspace, tab_workspaces, entry));
    }

    pub fn can_navigate(&self, direction: Direction) -> bool {
        self.navigation.can_navigate(direction, |entry| {
            navigation_entry_is_live(&self.workspace, &self.tab_workspaces, entry)
        })
    }

    pub fn navigate(&mut self, direction: Direction) -> Result<bool, NavigationApplyError> {
        self.prune_navigation_history();
        let Some(target) = self.navigation.target(direction, |entry| {
            navigation_entry_is_live(&self.workspace, &self.tab_workspaces, entry)
        }) else {
            return Ok(false);
        };
        let entry = target.entry.clone();
        let index = target.index;
        if !self.apply_navigation_entry(&entry)? {
            return Ok(false);
        }
        self.navigation.commit_target(index);
        Ok(true)
    }

    pub(crate) fn apply_navigation_entry(
        &mut self,
        entry: &NavigationEntry,
    ) -> Result<bool, NavigationApplyError> {
        let previous_tab_workspaces = self.tab_workspaces.clone();
        let previous_workspace = self.workspace.clone();
        let previous_active_project_id = self.active_project_id.clone();
        let previous_prefs_active_project_id = self.prefs.active_project_id.clone();
        let previous_active_worktree_ids = self.prefs.active_worktree_ids.clone();
        let previous_suppression = self.navigation_recording_suppressed;

        self.active_project_id = Some(entry.project_id.clone());
        self.prefs.active_project_id = Some(entry.project_id.clone());
        self.prefs
            .active_worktree_ids
            .insert(entry.project_id.clone(), entry.worktree_id.clone());
        let applied = self
            .tab_workspaces
            .worktree_mut(&entry.project_id, &entry.worktree_id)
            .is_some_and(|workspace| match entry.tab_id.as_deref() {
                Some(tab_id) => workspace.select_tab(&entry.area_id, tab_id),
                None => {
                    let exists = workspace.area(&entry.area_id).is_some();
                    if exists {
                        workspace.focus_area(Some(&entry.area_id));
                    }
                    exists
                }
            });
        if !applied {
            self.tab_workspaces = previous_tab_workspaces;
            self.workspace = previous_workspace;
            self.active_project_id = previous_active_project_id;
            self.prefs.active_project_id = previous_prefs_active_project_id;
            self.prefs.active_worktree_ids = previous_active_worktree_ids;
            return Ok(false);
        }

        self.navigation_recording_suppressed = true;
        if let Err(error) = self.persist_tab_workspaces() {
            self.tab_workspaces = previous_tab_workspaces;
            self.workspace = previous_workspace;
            self.active_project_id = previous_active_project_id;
            self.prefs.active_project_id = previous_prefs_active_project_id;
            self.prefs.active_worktree_ids = previous_active_worktree_ids;
            self.navigation_recording_suppressed = previous_suppression;
            return Err(NavigationApplyError::WorkspacePersistence(error));
        }
        if let Err(error) = Prefs::try_store_active_worktree_ids(&self.prefs.active_worktree_ids) {
            self.tab_workspaces = previous_tab_workspaces;
            self.workspace = previous_workspace;
            self.active_project_id = previous_active_project_id;
            self.prefs.active_project_id = previous_prefs_active_project_id;
            self.prefs.active_worktree_ids = previous_active_worktree_ids;
            let _ = self.tab_workspaces.save();
            self.navigation_recording_suppressed = previous_suppression;
            return Err(NavigationApplyError::PreferencePersistence(error));
        }
        Prefs::store_default("muxy.activeProjectID", Some(entry.project_id.as_str()));
        if let Some(worktree) = self.worktrees.get(&entry.project_id).and_then(|worktrees| {
            worktrees
                .iter()
                .find(|worktree| worktree.id.eq_ignore_ascii_case(&entry.worktree_id))
        }) && let Some(project) = self
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id.eq_ignore_ascii_case(&entry.project_id))
        {
            project.worktree_label = Some(if worktree.is_primary {
                "primary".to_owned()
            } else {
                worktree.name.clone()
            });
        }
        self.workspace.activate_group_for_project(&entry.project_id);
        self.navigation_recording_suppressed = previous_suppression;
        Ok(true)
    }

    pub fn prune_navigation_project(&mut self, project_id: &str) {
        let worktree_ids: Vec<String> = self
            .navigation
            .entries()
            .iter()
            .filter(|entry| entry.project_id.eq_ignore_ascii_case(project_id))
            .map(|entry| entry.worktree_id.clone())
            .collect();
        for worktree_id in worktree_ids {
            self.prune_navigation_worktree(project_id, &worktree_id);
        }
    }

    pub fn prune_navigation_worktree(&mut self, project_id: &str, worktree_id: &str) {
        self.navigation.prune(|entry| {
            !entry.project_id.eq_ignore_ascii_case(project_id)
                || !entry.worktree_id.eq_ignore_ascii_case(worktree_id)
        });
    }

    pub fn remove_project(&mut self, project_id: &str) -> bool {
        if self.project_operations.is_mutating(project_id) {
            return false;
        }
        if !self.workspace.remove(project_id) {
            return false;
        }
        self.project_operations.project_removed(project_id);
        self.tab_workspaces.remove_project(project_id);
        self.prune_navigation_project(project_id);
        if self.active_project_id.as_deref() == Some(project_id) {
            let replacement = self
                .workspace
                .visible_projects()
                .first()
                .map(|project| project.id.clone());
            self.active_project_id = replacement.clone();
            self.prefs.active_project_id = replacement.clone();
            if let Some(replacement) = replacement.as_deref() {
                self.workspace.activate_group_for_project(replacement);
            }
        }
        if let Err(error) = self.persist_tab_workspaces() {
            log::warn!("failed to save workspaces after project removal: {error}");
        }
        Prefs::store_default(
            "muxy.activeProjectID",
            self.prefs.active_project_id.as_deref(),
        );
        true
    }

    pub fn is_active(&self, project: &Project) -> bool {
        self.active_project_id.as_deref() == Some(project.id.as_str())
    }

    pub fn select_project_relative(&mut self, delta: i32) -> Option<String> {
        let ids: Vec<String> = self
            .workspace
            .visible_projects()
            .iter()
            .map(|project| project.id.clone())
            .collect();
        if ids.len() < 2 {
            return None;
        }
        let current = self
            .active_project_id
            .as_ref()
            .and_then(|active| ids.iter().position(|id| id == active))?;
        let index = (current as i32 + delta).rem_euclid(ids.len() as i32) as usize;
        let id = ids[index].clone();
        self.try_select_project(&id).then_some(id)
    }

    pub fn select_project_index(&mut self, index: usize) -> Option<String> {
        let id = self
            .workspace
            .visible_projects()
            .get(index)
            .map(|project| project.id.clone())?;
        self.try_select_project(&id).then_some(id)
    }

    pub fn select_project(&mut self, id: &str) {
        self.try_select_project(id);
    }

    fn try_select_project(&mut self, id: &str) -> bool {
        if let Some(project) = self.workspace.project(id) {
            let project_id = project.id.clone();
            let project_path = project.path.clone();
            let previous_tab_workspaces = self.tab_workspaces.clone();
            let previous_workspace = self.workspace.clone();
            let previous_active_project_id = self.active_project_id.clone();
            let previous_prefs_active_project_id = self.prefs.active_project_id.clone();
            if let Some(worktree_id) = self.prefs.active_worktree_ids.get(&project_id).cloned()
                && let Some(worktree) = self.worktrees.get(&project_id).and_then(|worktrees| {
                    worktrees
                        .iter()
                        .find(|worktree| worktree.id.eq_ignore_ascii_case(&worktree_id))
                })
            {
                self.tab_workspaces
                    .ensure_worktree(&project_id, &worktree.id, &worktree.path);
            } else {
                self.tab_workspaces
                    .ensure_project(project_id.clone(), project_path);
            }
            self.active_project_id = Some(project_id.clone());
            self.prefs.active_project_id = Some(project_id.clone());
            self.workspace.activate_group_for_project(&project_id);
            if let Err(error) = self.persist_tab_workspaces() {
                log::warn!("failed to save selected project workspace: {error}");
                self.tab_workspaces = previous_tab_workspaces;
                self.workspace = previous_workspace;
                self.active_project_id = previous_active_project_id;
                self.prefs.active_project_id = previous_prefs_active_project_id;
                return false;
            }
            Prefs::store_default("muxy.activeProjectID", Some(project_id.as_str()));
            return true;
        }
        false
    }
}

fn navigation_entry_is_live(
    workspace: &Workspace,
    tab_workspaces: &WorkspaceStore,
    entry: &NavigationEntry,
) -> bool {
    if workspace.project(&entry.project_id).is_none() {
        return false;
    }
    let Some(worktree) = tab_workspaces.worktree(&entry.project_id, &entry.worktree_id) else {
        return false;
    };
    let Some(area) = worktree.area(&entry.area_id) else {
        return false;
    };
    entry
        .tab_id
        .as_deref()
        .is_none_or(|tab_id| area.tabs.iter().any(|tab| tab.id == tab_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::navigation::{Direction, NavigationEntry, NavigationHistory};
    use muxy_core::store::worktrees;
    use muxy_ui::theme::ColorScheme;

    #[test]
    fn window_appearance_selects_the_matching_light_or_dark_theme() {
        assert_eq!(
            appearance_for_window(gpui::WindowAppearance::Light),
            muxy_ui::theme::Appearance::Light
        );
        assert_eq!(
            appearance_for_window(gpui::WindowAppearance::VibrantLight),
            muxy_ui::theme::Appearance::Light
        );
        assert_eq!(
            appearance_for_window(gpui::WindowAppearance::Dark),
            muxy_ui::theme::Appearance::Dark
        );
    }

    #[test]
    fn remove_worktree_contract_requires_post_disk_exact_cleanup_effects() {
        let _: Option<RemovalEffects> = None;
        let _ = AppState::apply_removed_worktree;
    }

    #[test]
    fn notifications_startup_retains_rows_marks_them_read_and_flushes_immediately() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notifications.json");
        let target = NotificationTarget::new(
            "11111111-2222-4333-8444-555555555555",
            "22222222-3333-4444-8555-666666666666",
            "33333333-4444-4555-8666-777777777777",
            "44444444-5555-4666-8777-888888888888",
            "55555555-6666-4777-8888-999999999999",
            "/tmp/worktree",
        )
        .unwrap();
        let mut store = NotificationStore::empty_at(&path);
        store.insert(
            muxy_core::notifications::NotificationRecord::new(
                target,
                muxy_core::notifications::NotificationSource::Socket,
                "Title",
                "Body",
                1.0,
            )
            .unwrap(),
        );
        store.flush().unwrap();

        let loaded = load_notification_store_from(&path);
        assert_eq!(loaded.records().len(), 1);
        assert!(loaded.records()[0].is_read);
        assert!(!loaded.needs_flush());
        let persisted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted[0]["isRead"], true);
    }

    #[test]
    fn composer_store_staged_status_requires_test_identity_case_and_injected_root() {
        let directory = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(directory.path()).unwrap();
        let support = root.join("support");
        let home = root.join("home");
        std::fs::create_dir(&support).unwrap();
        std::fs::create_dir(&home).unwrap();
        assert!(
            p7_composer_status_path(false, Some("phase-2"), &support, Some(&support), &home)
                .is_none()
        );
        assert!(p7_composer_status_path(true, None, &support, Some(&support), &home).is_none());
        assert!(
            p7_composer_status_path(true, Some("other"), &support, Some(&support), &home).is_none()
        );
        assert!(
            p7_composer_status_path(true, Some("phase-2"), &support, Some(&root), &home).is_none()
        );
        assert!(
            p7_composer_status_path(true, Some("phase-2"), &home, Some(&home), &home).is_none()
        );
        assert_eq!(
            p7_composer_status_path(true, Some("phase-2"), &support, Some(&support), &home),
            Some(support.join(".muxy-p7-composer-status.json"))
        );
    }

    #[test]
    fn composer_store_startup_exposes_valid_and_recoverable_load_status() {
        let directory = tempfile::tempdir().unwrap();
        let id = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE:11111111-2222-4333-8444-555555555555";
        std::fs::write(
            directory.path().join("rich-input-drafts.json"),
            serde_json::to_vec(&serde_json::json!({
                id: {"text": "restored"},
                "malformed": {"text": 7}
            }))
            .unwrap(),
        )
        .unwrap();
        let store = load_composer_store_from(directory.path());
        assert_eq!(store.drafts().count(), 1);
        assert_eq!(store.load_status().malformed_keys, ["malformed"]);
        assert!(!store.load_status().overwrite_blocked);
    }

    #[test]
    fn notifications_startup_empty_cutover_does_not_create_or_rewrite_a_file() {
        let directory = tempfile::tempdir().unwrap();
        let absent = directory.path().join("absent.json");
        let loaded = load_notification_store_from(&absent);
        assert!(loaded.records().is_empty());
        assert!(!absent.exists());

        let invalid = directory.path().join("invalid.json");
        std::fs::write(&invalid, b"not-json").unwrap();
        let before = std::fs::read(&invalid).unwrap();
        let loaded = load_notification_store_from(&invalid);
        assert!(loaded.records().is_empty());
        assert_eq!(std::fs::read(&invalid).unwrap(), before);
    }

    fn secondary(id: &str, name: &str, path: &str) -> Worktree {
        Worktree {
            id: id.into(),
            name: name.into(),
            path: path.into(),
            branch: Some(name.into()),
            source: muxy_core::store::worktrees::Source::Muxy,
            is_primary: false,
            created_at: 1.0,
            last_active_at: None,
        }
    }

    fn project(id: &str, name: &str, path: &str) -> Project {
        let mut project = Project::new(name.into(), path.into(), 0);
        project.id = id.into();
        project
    }

    fn state_at(path: &std::path::Path) -> AppState {
        let prefs = Prefs::default();
        let mut workspace = Workspace::load(&prefs);
        workspace.projects = vec![
            project("project-one", "One", "/one"),
            project("project-two", "Two", "/two"),
        ];
        let one = worktrees::primary("One", "/one");
        let two = worktrees::primary("Two", "/two");
        let mut tab_workspaces = WorkspaceStore::load_from(path);
        tab_workspaces.ensure_worktree("project-one", &one.id, &one.path);
        tab_workspaces.ensure_worktree("project-two", &two.id, &two.path);
        AppState {
            metrics: Metrics::new(1.0),
            prefs,
            theme: Theme::from_scheme(&ColorScheme::default()),
            workspace,
            tab_workspaces,
            shortcuts: ShortcutMap::load(),
            command_shortcuts: CommandShortcuts::default(),
            worktrees: HashMap::from([
                ("project-one".into(), vec![one]),
                ("project-two".into(), vec![two]),
            ]),
            project_operations: crate::project_operations::ProjectOperations::default(),
            navigation: NavigationHistory::default(),
            navigation_recording_suppressed: false,
            socket_ingress: IngressQueues::default(),
            notification_store: NotificationStore::empty_at(
                path.with_file_name("notifications.json"),
            ),
            active_project_id: None,
            ide_name: None,
            appearance: Appearance::Dark,
        }
    }

    fn select_in_memory(state: &mut AppState, project_id: &str) -> NavigationEntry {
        let worktree = state.worktrees[project_id][0].clone();
        state.active_project_id = Some(project_id.into());
        state.prefs.active_project_id = Some(project_id.into());
        state
            .prefs
            .active_worktree_ids
            .insert(project_id.into(), worktree.id.clone());
        state.current_navigation_entry().unwrap()
    }

    #[test]
    fn navigation_capture_includes_complete_home_state_and_honors_suppression() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state_at(&directory.path().join("workspaces.json"));
        let expected = select_in_memory(&mut state, "project-one");
        state.persist_tab_workspaces().unwrap();
        assert_eq!(state.navigation.current(), Some(&expected));

        state.navigation_recording_suppressed = true;
        select_in_memory(&mut state, "project-two");
        state.persist_tab_workspaces().unwrap();
        assert_eq!(state.navigation.current(), Some(&expected));

        let home = muxy_core::store::home_project();
        let home_worktree = worktrees::primary(&home.name, &home.path);
        state.workspace.projects.push(home.clone());
        state
            .tab_workspaces
            .ensure_worktree(&home.id, &home_worktree.id, &home_worktree.path);
        state
            .prefs
            .active_worktree_ids
            .insert(home.id.clone(), home_worktree.id.clone());
        state.active_project_id = Some(home.id.clone());
        let home_entry = state.current_navigation_entry().unwrap();
        assert_eq!(home_entry.project_id, muxy_core::store::HOME_PROJECT_ID);
        assert_eq!(home_entry.worktree_id, home_worktree.id);
        state.active_project_id = Some("project-one".into());
        let worktree_id = state.worktrees["project-one"][0].id.clone();
        state
            .tab_workspaces
            .worktree_mut("project-one", &worktree_id)
            .unwrap()
            .root = None;
        assert!(state.current_navigation_entry().is_none());

        let mut state = state_at(&directory.path().join("stale-tab.json"));
        select_in_memory(&mut state, "project-one");
        let worktree_id = state.worktrees["project-one"][0].id.clone();
        let workspace = state
            .tab_workspaces
            .worktree_mut("project-one", &worktree_id)
            .unwrap();
        let area_id = workspace.focused_area_id.clone().unwrap();
        workspace.area_mut(&area_id).unwrap().active_tab_id = Some("missing-tab".into());
        assert!(state.current_navigation_entry().is_none());
    }

    #[test]
    fn navigation_applies_complete_targets_and_prunes_removed_worktrees() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state_at(&directory.path().join("workspaces.json"));
        let one = select_in_memory(&mut state, "project-one");
        state.persist_tab_workspaces().unwrap();
        let two = select_in_memory(&mut state, "project-two");
        state.persist_tab_workspaces().unwrap();

        assert!(state.can_navigate(Direction::Back));
        assert!(!state.can_navigate(Direction::Forward));
        assert!(state.navigate(Direction::Back).unwrap());
        assert_eq!(state.active_project_id.as_deref(), Some("project-one"));
        assert_eq!(state.navigation.current(), Some(&one));
        assert!(state.can_navigate(Direction::Forward));
        assert!(state.navigate(Direction::Forward).unwrap());
        assert_eq!(state.navigation.current(), Some(&two));

        state.prune_navigation_worktree(&one.project_id, &one.worktree_id);
        assert_eq!(state.navigation.entries(), &[two]);
    }

    #[test]
    fn failed_navigation_persistence_does_not_advance_the_cursor_or_selection() {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("blocker");
        std::fs::write(&blocker, b"blocked").unwrap();
        let mut state = state_at(&blocker.join("workspaces.json"));
        let one = select_in_memory(&mut state, "project-one");
        let two = select_in_memory(&mut state, "project-two");
        state.navigation.record(one);
        state.navigation.record(two.clone());

        assert!(state.navigate(Direction::Back).is_err());
        assert_eq!(state.navigation.current(), Some(&two));
        assert_eq!(state.active_project_id.as_deref(), Some("project-two"));
    }

    #[test]
    fn create_worktree_application_keeps_authoritative_state_after_persistence_failures() {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("blocker");
        std::fs::write(&blocker, b"blocked").unwrap();
        let mut state = state_at(&blocker.join("workspaces.json"));
        let created = Worktree {
            id: "CREATED-ID".into(),
            name: "Feature".into(),
            path: "/feature".into(),
            branch: Some("feature".into()),
            source: muxy_core::store::worktrees::Source::Muxy,
            is_primary: false,
            created_at: 1.0,
            last_active_at: None,
        };
        let outcome = muxy_api::worktree_lifecycle::CreateWorktreeOutcome {
            worktree: created.clone(),
            worktrees: vec![state.worktrees["project-one"][0].clone(), created.clone()],
            warnings: Vec::new(),
        };

        let effects = state.apply_created_worktree_with("project-one", outcome, |_| Ok(()));

        assert_eq!(
            state.prefs.active_worktree_ids.get("project-one"),
            Some(&created.id)
        );
        assert!(
            state
                .tab_workspaces
                .worktree("project-one", &created.id)
                .is_some()
        );
        assert!(effects.warnings.iter().any(|warning| matches!(
            warning,
            muxy_api::worktree_lifecycle::LifecycleWarning::WorkspacePersistence(_)
        )));
        assert!(!effects.navigation_recorded);

        let mut state = state_at(&directory.path().join("workspaces-ok.json"));
        let outcome = muxy_api::worktree_lifecycle::CreateWorktreeOutcome {
            worktree: created.clone(),
            worktrees: vec![state.worktrees["project-one"][0].clone(), created],
            warnings: Vec::new(),
        };
        let effects = state.apply_created_worktree_with("project-one", outcome, |_| {
            Err(std::io::Error::other("preference failure"))
        });
        assert!(effects.navigation_recorded);
        assert!(effects.warnings.iter().any(|warning| matches!(
            warning,
            muxy_api::worktree_lifecycle::LifecycleWarning::ActivePreferencePersistence(_)
        )));
        assert_eq!(
            state
                .prefs
                .active_worktree_ids
                .get("project-one")
                .map(String::as_str),
            Some("CREATED-ID")
        );
    }

    #[test]
    fn remove_worktree_application_collects_tabs_prunes_exact_state_and_keeps_cleanup_on_save_failures()
     {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state_at(&directory.path().join("workspaces.json"));
        let primary = state.worktrees["project-one"][0].clone();
        let removed = secondary("REMOVED", "Removed", "/removed");
        state
            .worktrees
            .get_mut("project-one")
            .unwrap()
            .push(removed.clone());
        state
            .tab_workspaces
            .ensure_worktree("project-one", &removed.id, &removed.path);
        state.active_project_id = Some("project-one".into());
        state.prefs.active_project_id = Some("project-one".into());
        state
            .prefs
            .active_worktree_ids
            .insert("project-one".into(), removed.id.clone());
        let removed_entry = state.current_navigation_entry().unwrap();
        state.navigation.record(removed_entry);
        let outcome = muxy_api::worktree_lifecycle::RemoveWorktreeOutcome {
            removed: removed.clone(),
            worktrees: vec![primary.clone()],
            files_preserved: false,
            warnings: Vec::new(),
        };

        let effects =
            state.apply_removed_worktree_with("project-one", outcome, |_| Ok(()), |_| Ok(()));

        assert!(!effects.tab_ids.is_empty());
        assert!(effects.navigation_recorded);
        assert!(effects.warnings.is_empty());
        assert!(
            state
                .tab_workspaces
                .worktree("project-one", &removed.id)
                .is_none()
        );
        assert_eq!(
            state.prefs.active_worktree_ids.get("project-one"),
            Some(&primary.id)
        );
        assert!(
            state
                .navigation
                .entries()
                .iter()
                .all(|entry| { !entry.worktree_id.eq_ignore_ascii_case(&removed.id) })
        );
        assert_eq!(
            state
                .workspace
                .project("project-one")
                .and_then(|project| project.worktree_label.as_deref()),
            Some("primary")
        );

        let retained = secondary("RETAINED", "Retained", "/retained");
        let doomed = secondary("DOOMED", "Doomed", "/doomed");
        state.worktrees.insert(
            "project-one".into(),
            vec![primary, retained.clone(), doomed.clone()],
        );
        state
            .tab_workspaces
            .ensure_worktree("project-one", &retained.id, &retained.path);
        state
            .tab_workspaces
            .ensure_worktree("project-one", &doomed.id, &doomed.path);
        state
            .prefs
            .active_worktree_ids
            .insert("project-one".into(), retained.id.clone());
        let outcome = muxy_api::worktree_lifecycle::RemoveWorktreeOutcome {
            removed: doomed.clone(),
            worktrees: state.worktrees["project-one"]
                .iter()
                .filter(|worktree| worktree.id != doomed.id)
                .cloned()
                .collect(),
            files_preserved: false,
            warnings: Vec::new(),
        };
        let effects = state.apply_removed_worktree_with(
            "project-one",
            outcome,
            |_| Err(io::Error::other("workspace failure")),
            |_| Err(io::Error::other("preference failure")),
        );
        assert_eq!(
            state.prefs.active_worktree_ids.get("project-one"),
            Some(&retained.id)
        );
        assert!(
            state
                .tab_workspaces
                .worktree("project-one", &doomed.id)
                .is_none()
        );
        assert!(!effects.navigation_recorded);
        assert!(effects.warnings.iter().any(|warning| matches!(
            warning,
            muxy_api::worktree_lifecycle::LifecycleWarning::WorkspacePersistence(_)
        )));
        assert!(effects.warnings.iter().any(|warning| matches!(
            warning,
            muxy_api::worktree_lifecycle::LifecycleWarning::ActivePreferencePersistence(_)
        )));
    }

    #[test]
    fn navigation_commit_survives_pruning_before_a_forward_target() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state_at(&directory.path().join("workspaces.json"));
        let one = select_in_memory(&mut state, "project-one");
        let two = select_in_memory(&mut state, "project-two");
        let dead = NavigationEntry {
            project_id: one.project_id.clone(),
            worktree_id: one.worktree_id.clone(),
            area_id: "missing-area".into(),
            tab_id: None,
        };
        state.navigation.record(one.clone());
        state.navigation.record(dead);
        state.navigation.record(two.clone());
        state.navigation.commit_target(0);
        select_in_memory(&mut state, "project-one");

        assert!(state.navigate(Direction::Forward).unwrap());
        assert_eq!(state.active_project_id.as_deref(), Some("project-two"));
        assert_eq!(state.navigation.current(), Some(&two));
    }

    #[test]
    fn navigation_records_the_replacement_after_active_project_removal() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state_at(&directory.path().join("workspaces.json"));
        let one = select_in_memory(&mut state, "project-one");
        state.persist_tab_workspaces().unwrap();
        let two_id = state.worktrees["project-two"][0].id.clone();
        state
            .prefs
            .active_worktree_ids
            .insert("project-two".into(), two_id);

        assert!(state.remove_project("project-one"));
        assert_ne!(state.navigation.current(), Some(&one));
        assert_eq!(
            state
                .navigation
                .current()
                .map(|entry| entry.project_id.as_str()),
            Some("project-two")
        );
    }

    #[test]
    fn project_operation_blocks_project_removal_until_mutation_finishes() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = state_at(&directory.path().join("workspaces.json"));
        let operation = state
            .project_operations
            .begin_operation(
                "project-one",
                crate::project_operations::ProjectOperationKind::Remove,
            )
            .unwrap();

        assert!(!state.remove_project("project-one"));
        assert!(state.workspace.project("project-one").is_some());

        state
            .project_operations
            .finish_operation(&operation)
            .unwrap();
        assert!(state.remove_project("project-one"));
    }

    #[test]
    fn failed_truth_workspace_persistence_rolls_back_all_applied_state() {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("blocker");
        std::fs::write(&blocker, b"blocked").unwrap();
        let mut state = state_at(&blocker.join("workspaces.json"));
        let previous_project = state.workspace.project("project-one").unwrap().clone();
        let previous_worktrees = state.worktrees["project-one"].clone();
        let previous_active_worktree_ids = state.prefs.active_worktree_ids.clone();
        let previous_tab_workspaces = state.tab_workspaces.clone();
        let mut refreshed = worktrees::primary("One", "/refreshed");
        refreshed.id = "REFRESHED".into();

        let result = state.apply_truth(vec![muxy_api::truth::ProjectTruth {
            project_id: "project-one".into(),
            generation: 1,
            request_id: 2,
            is_git_repo: true,
            worktree_label: Some("refreshed".into()),
            worktrees: Some(vec![refreshed]),
            candidate: None,
        }]);

        assert!(result.is_err());
        let project = state.workspace.project("project-one").unwrap();
        assert_eq!(project.is_git_repo, previous_project.is_git_repo);
        assert_eq!(project.worktree_label, previous_project.worktree_label);
        assert_eq!(state.worktrees["project-one"], previous_worktrees);
        assert_eq!(
            state.prefs.active_worktree_ids,
            previous_active_worktree_ids
        );
        assert_eq!(
            state.tab_workspaces.states(),
            previous_tab_workspaces.states()
        );
    }

    #[test]
    fn active_repository_key_uses_selected_worktree_and_excludes_nonlocal_candidates() {
        let directory = tempfile::tempdir().unwrap();
        let repository = directory.path().join("repository");
        let secondary = directory.path().join("secondary");
        std::fs::create_dir_all(repository.join(".git")).unwrap();
        std::fs::create_dir_all(&secondary).unwrap();
        std::fs::write(
            secondary.join(".git"),
            format!(
                "gitdir: {}/worktrees/secondary\n",
                repository.join(".git").display()
            ),
        )
        .unwrap();
        let mut state = state_at(&directory.path().join("workspaces.json"));
        let project = state
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id == "project-one")
            .unwrap();
        project.path = repository.to_string_lossy().into_owned();
        project.is_git_repo = true;
        let primary = &mut state.worktrees.get_mut("project-one").unwrap()[0];
        primary.id = "PRIMARY".to_owned();
        primary.path = repository.to_string_lossy().into_owned();
        let mut linked = primary.clone();
        linked.id = "SECONDARY".to_owned();
        linked.name = "feature".to_owned();
        linked.path = secondary.to_string_lossy().into_owned();
        linked.is_primary = false;
        state.worktrees.get_mut("project-one").unwrap().push(linked);
        state.active_project_id = Some("project-one".to_owned());
        state
            .prefs
            .active_worktree_ids
            .insert("project-one".to_owned(), "PRIMARY".to_owned());

        let primary_key = state.active_repository_key().unwrap();
        assert_eq!(primary_key.worktree_id, "PRIMARY");
        assert_eq!(
            primary_key.normalized_path,
            std::fs::canonicalize(&repository).unwrap()
        );

        state
            .prefs
            .active_worktree_ids
            .insert("project-one".to_owned(), "SECONDARY".to_owned());
        let secondary_key = state.active_repository_key().unwrap();
        assert_eq!(secondary_key.worktree_id, "SECONDARY");
        assert_eq!(
            secondary_key.normalized_path,
            std::fs::canonicalize(&secondary).unwrap()
        );
        let focused_terminal_cwd = directory.path().join("unrelated-terminal-cwd");
        std::fs::create_dir_all(&focused_terminal_cwd).unwrap();
        assert_ne!(secondary_key.normalized_path, focused_terminal_cwd);
        assert_eq!(state.active_repository_key().unwrap(), secondary_key);

        let other_repository = directory.path().join("other-repository");
        std::fs::create_dir_all(other_repository.join(".git")).unwrap();
        let mut other_project = state
            .workspace
            .projects
            .iter()
            .find(|project| project.id == "project-one")
            .unwrap()
            .clone();
        other_project.id = "repository-switch".to_owned();
        other_project.path = other_repository.to_string_lossy().into_owned();
        other_project.is_git_repo = true;
        other_project.remote_workspace_id = None;
        other_project.remote_device_id = None;
        let mut other_worktree = state.worktrees["project-one"][0].clone();
        other_worktree.id = "OTHER_PRIMARY".to_owned();
        other_worktree.path = other_repository.to_string_lossy().into_owned();
        state.workspace.projects.push(other_project);
        state
            .worktrees
            .insert("repository-switch".to_owned(), vec![other_worktree]);
        state
            .prefs
            .active_worktree_ids
            .insert("repository-switch".to_owned(), "OTHER_PRIMARY".to_owned());
        state.active_project_id = Some("repository-switch".to_owned());
        assert_eq!(
            state.active_project().unwrap().path,
            other_repository.to_string_lossy()
        );
        assert_eq!(
            state.active_worktree_path(state.active_project().unwrap()),
            other_repository.to_string_lossy()
        );
        assert!(muxy_api::git::is_repository_path(&other_repository));
        let other_key = state.active_repository_key().unwrap();
        assert_eq!(other_key.project_id, "repository-switch");
        assert_eq!(other_key.worktree_id, "OTHER_PRIMARY");
        assert_eq!(
            other_key.normalized_path,
            std::fs::canonicalize(&other_repository).unwrap()
        );
        state.active_project_id = Some("project-one".to_owned());

        state
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id == "project-one")
            .unwrap()
            .is_git_repo = false;
        assert!(state.active_repository_key().is_none());
        state
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id == "project-one")
            .unwrap()
            .is_git_repo = true;
        state
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id == "project-one")
            .unwrap()
            .remote_workspace_id = Some("remote".to_owned());
        assert!(state.active_repository_key().is_none());
        state
            .workspace
            .projects
            .iter_mut()
            .find(|project| project.id == "project-one")
            .unwrap()
            .remote_workspace_id = None;
        state.active_project_id = Some(muxy_core::store::HOME_PROJECT_ID.to_owned());
        assert!(state.active_repository_key().is_none());
    }
}
