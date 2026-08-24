use gpui::{App, WindowAppearance};
use muxy_core::prefs::Prefs;
use muxy_core::shortcuts::ShortcutMap;
use muxy_core::store::{CommandShortcuts, Project, Workspace, Worktree};
use muxy_core::workspace::WorkspaceState;
use muxy_core::workspace_store::WorkspaceStore;
use muxy_ui::theme::{Appearance, Metrics, Theme};
use std::collections::HashMap;

pub struct AppState {
    pub prefs: Prefs,
    pub theme: Theme,
    pub metrics: Metrics,
    pub workspace: Workspace,
    pub tab_workspaces: WorkspaceStore,
    pub shortcuts: ShortcutMap,
    pub command_shortcuts: CommandShortcuts,
    pub worktrees: HashMap<String, Vec<Worktree>>,
    pub active_project_id: Option<String>,
    pub ide_name: Option<String>,
    pub appearance: Appearance,
}

impl AppState {
    pub fn load(cx: &App) -> Self {
        let prefs = Prefs::load();
        let appearance = match cx.window_appearance() {
            WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
            _ => Appearance::Dark,
        };
        let theme = match appearance {
            Appearance::Light => crate::themes::load(&prefs.light_theme, "Muxy Light"),
            Appearance::Dark => crate::themes::load(&prefs.dark_theme, "Muxy"),
        };
        let workspace = Workspace::load(&prefs);
        let mut tab_workspaces = WorkspaceStore::load();
        for project in &workspace.projects {
            tab_workspaces.ensure_project(project.id.clone(), project.path.clone());
        }
        let shortcuts = ShortcutMap::load();
        let command_shortcuts = CommandShortcuts::load();
        let active_project_id = workspace.resolve_active(&prefs);
        let ide_name = prefs
            .ide_bundle_identifier
            .as_deref()
            .and_then(muxy_api::ide::display_name);

        Self {
            metrics: Metrics::new(prefs.scale.multiplier()),
            prefs,
            theme,
            workspace,
            tab_workspaces,
            shortcuts,
            command_shortcuts,
            worktrees: HashMap::new(),
            active_project_id,
            ide_name,
            appearance,
        }
    }

    pub fn apply_truth(&mut self, truth: Vec<muxy_api::truth::ProjectTruth>) {
        let mut active_worktrees_changed = false;
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
            if !entry.worktrees.is_empty()
                && self
                    .prefs
                    .active_worktree_ids
                    .get(&entry.project_id)
                    .is_some_and(|selected| {
                        !entry
                            .worktrees
                            .iter()
                            .any(|worktree| worktree.id.eq_ignore_ascii_case(selected))
                    })
            {
                self.prefs.active_worktree_ids.remove(&entry.project_id);
                active_worktrees_changed = true;
            }
            self.worktrees.insert(entry.project_id, entry.worktrees);
        }
        if active_worktrees_changed {
            Prefs::store_active_worktree_ids(&self.prefs.active_worktree_ids);
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

    pub fn active_tab_workspace(&self) -> Option<&WorkspaceState> {
        let project = self.active_project()?;
        self.tab_workspaces
            .active(&project.id, &self.active_worktree_path(project))
    }

    pub fn active_tab_workspace_mut(&mut self) -> Option<&mut WorkspaceState> {
        let project = self.active_project()?;
        let id = project.id.clone();
        let path = self.active_worktree_path(project);
        self.tab_workspaces.active_mut(&id, &path)
    }

    pub fn layouts(&self) -> Vec<muxy_api::layouts::Descriptor> {
        self.active_project()
            .map(|project| muxy_api::layouts::discover(&self.active_worktree_path(project)))
            .unwrap_or_default()
    }

    pub fn select_worktree(&mut self, project_id: &str, worktree_id: &str) {
        let Some(worktree) = self
            .worktrees
            .get(project_id)
            .and_then(|list| {
                list.iter()
                    .find(|worktree| worktree.id.eq_ignore_ascii_case(worktree_id))
            })
            .cloned()
        else {
            return;
        };
        self.tab_workspaces
            .ensure_worktree(project_id, &worktree.id, &worktree.path);
        if let Err(error) = self.tab_workspaces.save() {
            log::warn!("failed to save selected worktree workspace: {error}");
            return;
        }
        self.prefs
            .active_worktree_ids
            .insert(project_id.to_owned(), worktree.id.clone());
        Prefs::store_active_worktree_ids(&self.prefs.active_worktree_ids);
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
        self.select_project(project_id);
    }

    pub fn save_tab_workspaces(&self) {
        let _ = self.tab_workspaces.save();
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
        self.select_project(&id);
        Some(id)
    }

    pub fn select_project_index(&mut self, index: usize) -> Option<String> {
        let id = self
            .workspace
            .visible_projects()
            .get(index)
            .map(|project| project.id.clone())?;
        self.select_project(&id);
        Some(id)
    }

    pub fn select_project(&mut self, id: &str) {
        if let Some(project) = self.workspace.project(id) {
            let project_id = project.id.clone();
            self.tab_workspaces
                .ensure_project(project_id.clone(), project.path.clone());
            self.active_project_id = Some(project_id.clone());
            self.workspace.activate_group_for_project(&project_id);
        }
    }
}
