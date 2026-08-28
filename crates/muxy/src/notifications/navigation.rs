use crate::state::{AppState, NavigationApplyError};
use muxy_core::navigation::NavigationEntry;
use muxy_core::notifications::NotificationRecord;
use muxy_core::workspace::TabKind;

#[derive(Default)]
pub struct NavigationOutcome {
    pub navigated: bool,
    pub read_changed: bool,
    pub error: Option<NavigationApplyError>,
}

impl NavigationOutcome {
    pub fn changed(&self) -> bool {
        self.navigated || self.read_changed
    }
}

pub fn mark_active_tab_read(state: &mut AppState, window_active: bool) -> bool {
    if !window_active {
        return false;
    }
    let Some(target) = state.active_notification_target() else {
        return false;
    };
    state
        .notification_store
        .mark_tab_read(&target.project_id, &target.worktree_id, &target.tab_id)
}

pub fn navigate(state: &mut AppState, notification_id: &str) -> NavigationOutcome {
    let Some(record) = state.notification_store.get(notification_id).cloned() else {
        return NavigationOutcome::default();
    };
    let entry = live_navigation_entry(state, &record);
    let (navigated, error) = match entry {
        Some(entry) => match state.apply_navigation_entry(&entry) {
            Ok(navigated) => (navigated, None),
            Err(error) => (false, Some(error)),
        },
        None => (false, None),
    };
    let read_changed = state.notification_store.mark_read(notification_id);
    NavigationOutcome {
        navigated,
        read_changed,
        error,
    }
}

fn live_navigation_entry(state: &AppState, record: &NotificationRecord) -> Option<NavigationEntry> {
    state
        .workspace
        .project(&record.project_id)
        .filter(|project| project.id.eq_ignore_ascii_case(&record.project_id))?;
    state
        .worktrees
        .get(&record.project_id)?
        .iter()
        .find(|worktree| worktree.id.eq_ignore_ascii_case(&record.worktree_id))?;
    let workspace = state
        .tab_workspaces
        .worktree(&record.project_id, &record.worktree_id)?;
    let area = workspace.area(&record.area_id)?;
    let tab = area.tab(&record.tab_id)?;
    let pane = area.tab(&record.pane_id)?;
    if tab.kind != TabKind::Terminal
        || pane.kind != TabKind::Terminal
        || !pane.root_id().eq_ignore_ascii_case(&record.tab_id)
    {
        return None;
    }
    Some(NavigationEntry {
        project_id: record.project_id.clone(),
        worktree_id: record.worktree_id.clone(),
        area_id: record.area_id.clone(),
        tab_id: Some(record.tab_id.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::ingress::IngressQueues;
    use muxy_core::navigation::NavigationHistory;
    use muxy_core::notifications::{NotificationSource, NotificationStore, NotificationTarget};
    use muxy_core::prefs::Prefs;
    use muxy_core::shortcuts::ShortcutMap;
    use muxy_core::store::{CommandShortcuts, Project, Workspace, worktrees};
    use muxy_core::workspace::{Edge, Tab, TabKind};
    use muxy_core::workspace_store::WorkspaceStore;
    use muxy_ui::theme::{Appearance, ColorScheme, Metrics, Theme};
    use std::collections::HashMap;

    fn fixture(path: &std::path::Path) -> (AppState, NotificationTarget) {
        let project_id = muxy_core::store::new_uuid();
        let mut project = Project::new(
            "Project".to_owned(),
            "/tmp/notification-project".to_owned(),
            0,
        );
        project.id = project_id.clone();
        let mut worktree = worktrees::primary("Project", "/tmp/notification-project");
        worktree.id = muxy_core::store::new_uuid();
        let prefs = Prefs {
            active_project_id: Some(project_id.clone()),
            active_worktree_ids: HashMap::from([(project_id.clone(), worktree.id.clone())]),
            ..Prefs::default()
        };
        let mut workspace = Workspace::load(&prefs);
        workspace.projects = vec![project];
        let mut tab_workspaces = WorkspaceStore::load_from(path);
        let terminal = tab_workspaces.ensure_worktree(&project_id, &worktree.id, &worktree.path);
        let pane = terminal.root.as_ref().unwrap().tabs()[0];
        let area = terminal.area_containing_tab(&pane.id).unwrap();
        let target = NotificationTarget::new(
            &pane.id,
            &project_id,
            &worktree.id,
            &area.id,
            pane.root_id(),
            &worktree.path,
        )
        .unwrap();
        let notification_store =
            NotificationStore::empty_at(path.with_file_name("notifications.json"));
        (
            AppState {
                prefs,
                theme: Theme::from_scheme(&ColorScheme::default()),
                metrics: Metrics::new(1.0),
                workspace,
                tab_workspaces,
                shortcuts: ShortcutMap::load(),
                command_shortcuts: CommandShortcuts::default(),
                worktrees: HashMap::from([(project_id.clone(), vec![worktree])]),
                project_operations: crate::project_operations::ProjectOperations::default(),
                navigation: NavigationHistory::default(),
                navigation_recording_suppressed: false,
                socket_ingress: IngressQueues::default(),
                notification_store,
                active_project_id: Some(project_id),
                ide_name: None,
                appearance: Appearance::Dark,
            },
            target,
        )
    }

    fn insert(state: &mut AppState, target: NotificationTarget) -> String {
        let record = NotificationRecord::new(
            target,
            NotificationSource::Socket,
            "Task completed!",
            "Finished",
            1.0,
        )
        .unwrap();
        let id = record.id.clone();
        state.notification_store.insert(record);
        id
    }

    #[test]
    fn desktop_notification_target_resolution_returns_complete_live_and_focused_snapshots() {
        let directory = tempfile::tempdir().unwrap();
        let (state, target) = fixture(&directory.path().join("workspaces.json"));

        assert_eq!(
            state.notification_target_for_pane(&target.pane_id),
            Some(target.clone())
        );
        assert_eq!(state.active_notification_target(), Some(target.clone()));
        assert_eq!(
            state.active_first_terminal_notification_target(),
            Some(target.clone())
        );
        assert!(state.notification_target_is_focused(&target));
        let mut other_pane = target.clone();
        other_pane.pane_id = muxy_core::store::new_uuid();
        assert!(!state.notification_target_is_focused(&other_pane));
        assert!(
            state
                .notification_target_for_pane("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE")
                .is_none()
        );
        assert!(state.notification_target_for_pane("invalid").is_none());
    }

    #[test]
    fn desktop_notification_focus_rejects_other_split_hidden_tab_project_and_worktree() {
        let directory = tempfile::tempdir().unwrap();
        let (mut state, target) = fixture(&directory.path().join("workspaces.json"));
        let workspace = state
            .tab_workspaces
            .worktree_mut(&target.project_id, &target.worktree_id)
            .unwrap();
        let split_pane = Tab::new(TabKind::Terminal);
        let split_pane_id = split_pane.id.clone();
        workspace
            .split_area(&target.area_id, Edge::Right, split_pane)
            .unwrap();
        let split_target = state.notification_target_for_pane(&split_pane_id).unwrap();
        assert!(!state.notification_target_is_focused(&target));
        assert!(state.notification_target_is_focused(&split_target));

        let workspace = state
            .tab_workspaces
            .worktree_mut(&target.project_id, &target.worktree_id)
            .unwrap();
        workspace.focus_area(Some(&target.area_id));
        assert!(state.notification_target_is_focused(&target));
        assert!(!state.notification_target_is_focused(&split_target));

        let workspace = state
            .tab_workspaces
            .worktree_mut(&target.project_id, &target.worktree_id)
            .unwrap();
        workspace
            .new_top_level_tab(Tab::new(TabKind::Terminal))
            .unwrap();
        assert!(!state.notification_target_is_focused(&target));

        let mut other_project = target.clone();
        other_project.project_id = muxy_core::store::new_uuid();
        assert!(!state.notification_target_is_focused(&other_project));
        let mut other_worktree = target;
        other_worktree.worktree_id = muxy_core::store::new_uuid();
        assert!(!state.notification_target_is_focused(&other_worktree));
    }

    #[test]
    fn notification_navigation_active_tab_read_sync_is_inactive_safe_and_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let (mut state, target) = fixture(&directory.path().join("workspaces.json"));
        let current_id = insert(&mut state, target.clone());
        let mut other_target = target;
        other_target.tab_id = muxy_core::store::new_uuid();
        let other_id = insert(&mut state, other_target);
        let revision = state.notification_store.dirty_revision();

        assert!(!mark_active_tab_read(&mut state, false));
        assert_eq!(state.notification_store.dirty_revision(), revision);
        assert!(mark_active_tab_read(&mut state, true));
        assert!(state.notification_store.get(&current_id).unwrap().is_read);
        assert!(!state.notification_store.get(&other_id).unwrap().is_read);
        let synced_revision = state.notification_store.dirty_revision();
        assert!(!mark_active_tab_read(&mut state, true));
        assert_eq!(state.notification_store.dirty_revision(), synced_revision);
    }

    #[test]
    fn notification_navigation_live_target_selects_exact_identity_and_marks_read() {
        let directory = tempfile::tempdir().unwrap();
        let (mut state, target) = fixture(&directory.path().join("workspaces.json"));
        let id = insert(&mut state, target.clone());

        let outcome = navigate(&mut state, &id);

        assert!(outcome.navigated);
        assert!(outcome.read_changed);
        assert!(outcome.error.is_none());
        assert!(state.notification_store.get(&id).unwrap().is_read);
        assert_eq!(
            state.active_project_id.as_deref(),
            Some(target.project_id.as_str())
        );
        assert_eq!(
            state.prefs.active_worktree_ids.get(&target.project_id),
            Some(&target.worktree_id)
        );
        let workspace = state
            .tab_workspaces
            .worktree(&target.project_id, &target.worktree_id)
            .unwrap();
        assert_eq!(
            workspace.focused_area_id.as_deref(),
            Some(target.area_id.as_str())
        );
        assert_eq!(
            workspace
                .area(&target.area_id)
                .unwrap()
                .active_tab_id
                .as_deref(),
            Some(target.tab_id.as_str())
        );
    }

    #[test]
    fn notification_navigation_stale_target_marks_read_without_recreation() {
        let directory = tempfile::tempdir().unwrap();
        let (mut state, mut target) = fixture(&directory.path().join("workspaces.json"));
        target.area_id = muxy_core::store::new_uuid();
        let id = insert(&mut state, target.clone());
        let workspace_count = state.tab_workspaces.states().len();

        let outcome = navigate(&mut state, &id);

        assert!(!outcome.navigated);
        assert!(outcome.read_changed);
        assert!(outcome.error.is_none());
        assert!(state.notification_store.get(&id).unwrap().is_read);
        assert_eq!(state.tab_workspaces.states().len(), workspace_count);
        assert!(
            state
                .tab_workspaces
                .states()
                .iter()
                .all(|workspace| workspace.area(&target.area_id).is_none())
        );
    }

    #[test]
    fn notification_navigation_uses_stable_ids_not_stored_worktree_path() {
        let directory = tempfile::tempdir().unwrap();
        let (mut state, mut target) = fixture(&directory.path().join("workspaces.json"));
        target.worktree_path = "/stale/context/path".to_owned();
        let id = insert(&mut state, target);

        let outcome = navigate(&mut state, &id);

        assert!(outcome.navigated);
        assert!(outcome.read_changed);
        assert!(outcome.error.is_none());
    }

    #[test]
    fn notification_navigation_unknown_id_is_ignored() {
        let directory = tempfile::tempdir().unwrap();
        let (mut state, target) = fixture(&directory.path().join("workspaces.json"));
        let id = insert(&mut state, target);
        let revision = state.notification_store.dirty_revision();

        let outcome = navigate(&mut state, "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE");

        assert!(!outcome.changed());
        assert!(outcome.error.is_none());
        assert!(!state.notification_store.get(&id).unwrap().is_read);
        assert_eq!(state.notification_store.dirty_revision(), revision);
    }
}
