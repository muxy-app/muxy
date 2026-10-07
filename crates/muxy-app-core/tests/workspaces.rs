use muxy_app_core::{AppState, ProjectId, ServerId, WorkspaceId, store};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn workspaces_persist_and_older_or_stale_state_files_still_load() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("desktop-state.json");
    let mut state = AppState::bootstrap()?;
    let untouched = serde_json::to_value(&state)?;
    assert!(untouched.get("workspaces").is_none());
    assert!(untouched["window"].get("workspace").is_none());

    let project = state.add_project(ServerId::local(), std::env::temp_dir())?;
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
