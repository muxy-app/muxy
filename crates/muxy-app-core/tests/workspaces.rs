use std::os::unix::ffi::OsStrExt;

use muxy_app_core::{AppState, ProjectId, ProjectKind, WorkspaceId, store};
use muxy_protocol::{CatalogPage, ServerIdentity, ServerPath};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn catalog(state: &AppState) -> CatalogPage {
    CatalogPage {
        server: ServerIdentity::from_u128(1),
        home: state.home().id,
        revision: 1,
        projects: state
            .projects()
            .iter()
            .map(muxy_app_core::Project::descriptor)
            .collect(),
        next: None,
        legacy_home: None,
    }
}

fn acknowledge(state: &mut AppState) -> Result {
    while let Some(intent) = state.project_intents().first().cloned() {
        state.complete_project_intent(intent.operation)?;
    }
    Ok(())
}

fn with_worktree(
    state: &mut AppState,
    parent: ProjectId,
    directory: &tempfile::TempDir,
) -> Result<ProjectId> {
    std::fs::write(directory.path().join(".git"), "gitdir: elsewhere")?;
    let mut page = catalog(state);
    let mut child = state.project(parent).ok_or("parent")?.descriptor();
    child.id = ProjectId::new();
    child.name = "feature".into();
    child.directory = ServerPath(directory.path().as_os_str().as_bytes().into());
    child.kind = Some(ProjectKind::Worktree);
    child.parent_id = Some(parent);
    page.projects.push(child.clone());
    state.apply_catalog(&page)?;
    Ok(child.id)
}

fn listed(state: &AppState) -> Vec<ProjectId> {
    state
        .projects()
        .iter()
        .filter(|project| state.is_listed(project))
        .map(|project| project.id)
        .collect()
}

#[test]
fn overlapping_workspaces_filter_top_level_projects_and_their_worktrees() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let shared = state.add_project(std::env::temp_dir())?;
    let personal = state.add_project(std::env::temp_dir())?;
    acknowledge(&mut state)?;
    let checkout = tempfile::tempdir()?;
    let worktree = with_worktree(&mut state, shared, &checkout)?;
    let work = state.create_workspace("  Work  ")?;
    let hobby = state.create_workspace("Personal")?;
    state.set_workspace_member(work, shared, true)?;
    state.set_workspace_member(hobby, shared, true)?;
    state.set_workspace_member(hobby, personal, true)?;
    assert_eq!(state.workspace(work).ok_or("work")?.name, "Work");
    assert_eq!(listed(&state), [home, shared, personal, worktree]);

    state.select_workspace(Some(work))?;
    assert_eq!(state.active_workspace().map(|w| w.id), Some(work));
    assert_eq!(listed(&state), [home, shared, worktree]);
    state.select_workspace(Some(hobby))?;
    assert_eq!(listed(&state), [home, shared, personal, worktree]);

    assert!(state.set_workspace_member(work, home, true).is_err());
    assert!(state.set_workspace_member(work, worktree, true).is_err());
    assert!(state.create_workspace("   ").is_err());
    assert!(state.rename_workspace(work, "").is_err());
    state.rename_workspace(work, "Client")?;
    assert_eq!(state.workspace(work).ok_or("work")?.name, "Client");

    state.set_workspace_member(hobby, shared, false)?;
    state.delete_workspace(hobby)?;
    assert!(state.active_workspace().is_none());
    assert_eq!(state.workspaces().len(), 1);
    assert_eq!(listed(&state), [home, shared, personal, worktree]);
    assert!(state.select_workspace(Some(hobby)).is_err());

    state.set_workspace_member(work, shared, false)?;
    state.select_workspace(Some(work))?;
    state.join_active_workspace(worktree);
    state.join_active_workspace(home);
    assert_eq!(
        state
            .workspace(work)
            .ok_or("work")?
            .projects
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [shared],
        "opening a worktree adds its parent"
    );
    Ok(())
}

#[test]
fn opened_projects_join_the_active_workspace_and_deleted_projects_leave_it() -> Result {
    let mut state = AppState::bootstrap()?;
    let work = state.create_workspace("Work")?;
    let outside = state.add_project(std::env::temp_dir())?;
    state.select_workspace(Some(work))?;
    let inside = state.add_project(std::env::temp_dir())?;
    let remote = state.add_project(std::env::temp_dir())?;
    state.join_active_workspace(state.home().id);
    let members = |state: &AppState| -> Vec<ProjectId> {
        state.workspaces()[0].projects.iter().copied().collect()
    };
    let mut expected = vec![inside, remote];
    expected.sort();
    assert_eq!(members(&state), expected);
    assert!(!state.workspaces()[0].projects.contains(&outside));

    state.remove_project(inside)?;
    assert_eq!(members(&state), [remote]);

    acknowledge(&mut state)?;
    let mut page = catalog(&state);
    page.projects.retain(|project| project.id != remote);
    state.apply_catalog(&page)?;
    assert!(members(&state).is_empty());
    assert_eq!(state.active_workspace().map(|w| w.id), Some(work));
    Ok(())
}

#[test]
fn navigating_outside_the_filter_reveals_a_workspace_that_lists_the_project() -> Result {
    let mut state = AppState::bootstrap()?;
    let first = state.add_project(std::env::temp_dir())?;
    let second = state.add_project(std::env::temp_dir())?;
    let loose = state.add_project(std::env::temp_dir())?;
    acknowledge(&mut state)?;
    let checkout = tempfile::tempdir()?;
    let worktree = with_worktree(&mut state, first, &checkout)?;
    let work = state.create_workspace("Work")?;
    let other = state.create_workspace("Other")?;
    state.set_workspace_member(work, first, true)?;
    state.set_workspace_member(other, second, true)?;
    state.select_workspace(Some(work))?;

    state.select_project(state.home().id)?;
    state.reveal_current_project();
    assert_eq!(
        state.window().workspace,
        Some(work),
        "Home is always listed"
    );

    state.select_project(second)?;
    state.reveal_current_project();
    assert_eq!(state.window().workspace, Some(other));

    state.select_project(worktree)?;
    state.reveal_current_project();
    assert_eq!(
        state.window().workspace,
        Some(work),
        "worktrees follow their parent"
    );

    state.select_project(loose)?;
    state.reveal_current_project();
    assert_eq!(state.window().workspace, None);
    Ok(())
}

#[test]
fn workspaces_persist_and_older_or_stale_state_files_still_load() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("desktop-state.json");
    let mut state = AppState::bootstrap()?;
    let untouched = serde_json::to_value(&state)?;
    assert!(untouched.get("workspaces").is_none());
    assert!(untouched["window"].get("workspace").is_none());

    let project = state.add_project(std::env::temp_dir())?;
    let work = state.create_workspace("Work")?;
    state.set_workspace_member(work, project, true)?;
    state.select_workspace(Some(work))?;
    store::save(&path, &state)?;
    assert_eq!(store::load(&path)?, state);

    let mut dangling = serde_json::to_value(&state)?;
    dangling["workspaces"][0]["projects"] = serde_json::json!([
        project.to_string(),
        state.home().id.to_string(),
        ProjectId::new().to_string(),
    ]);
    dangling["window"]["workspace"] = serde_json::json!(WorkspaceId::new().to_string());
    let loaded: AppState = serde_json::from_value(dangling)?;
    assert_eq!(
        loaded.workspaces()[0]
            .projects
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [project]
    );
    assert!(loaded.active_workspace().is_none());

    let mut older = serde_json::to_value(&state)?;
    older.as_object_mut().ok_or("state")?.remove("workspaces");
    older["window"]
        .as_object_mut()
        .ok_or("window")?
        .remove("workspace");
    let loaded: AppState = serde_json::from_value(older)?;
    assert!(loaded.workspaces().is_empty());
    assert!(loaded.active_workspace().is_none());
    Ok(())
}
