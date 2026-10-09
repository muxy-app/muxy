use std::error::Error;

use muxy_app_core::{AppState, Color, PaneContent, ProjectId, ServerId};
use muxy_protocol::SessionId;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn legacy_home_migrates_and_renamed_home_wins_over_another_home_name() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let tab = state.open_terminal_tab(home)?;
    let mut legacy = serde_json::to_value(&state)?;
    legacy["projects"][0]
        .as_object_mut()
        .ok_or("project object")?
        .remove("home");
    let mut migrated: AppState = serde_json::from_value(legacy)?;
    assert!(migrated.home().home);
    assert_eq!(migrated.home().id, home);
    assert_eq!(migrated.home().tabs[0].id, tab);
    migrated.rename_project(home, "Personal")?;
    let other = migrated.add_project(ServerId::local(), std::env::temp_dir())?;
    migrated.rename_project(other, "Home")?;
    let restored: AppState = serde_json::from_value(serde_json::to_value(&migrated)?)?;
    assert_eq!(restored.home().id, home);
    assert_eq!(restored.home().name, "Personal");
    assert_eq!(restored.current_project().id, other);
    assert_eq!(restored, migrated);
    Ok(())
}

#[test]
fn missing_projects_preserve_tabs_and_only_allow_removal() -> TestResult {
    let directory = std::env::temp_dir().join(format!("muxy-project-{}", ProjectId::new()));
    std::fs::create_dir(&directory)?;
    let marker = directory.join("keep.txt");
    std::fs::write(&marker, "keep")?;
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let project = state.add_project(ServerId::local(), directory.clone())?;
    let tab = state.open_terminal_tab(project)?;
    let session = SessionId::new(24).ok_or("session")?;
    state.set_pane_session(state.current_project().tabs[0].panes[0].id, Some(session))?;
    let moved = directory.with_extension("moved");
    std::fs::rename(&directory, &moved)?;
    state.refresh_project_statuses();
    assert_eq!(
        state.current_project().status(),
        muxy_app_core::ProjectStatus::Missing
    );
    assert!(state.open_terminal_tab(project).is_err());
    assert!(state.select_project(project).is_err());
    assert!(state.select_tab(project, tab).is_err());
    assert!(state.rename_project(project, "Gone").is_err());
    assert!(state.set_project_icon(project, None).is_err());
    assert!(state.set_project_color(project, Color::default()).is_err());
    assert!(state.move_project(project, 1).is_err());
    let json = serde_json::to_value(&state)?;
    assert!(json["projects"][1].get("status").is_none());
    let mut loaded: AppState = serde_json::from_value(json)?;
    assert_eq!(
        loaded.current_project().status(),
        muxy_app_core::ProjectStatus::Missing
    );
    assert_eq!(loaded.remove_project(project)?, [session]);
    assert_eq!(loaded.current_project().id, home);
    assert!(!loaded.window().selected_tab.contains_key(&project));
    assert_eq!(std::fs::read_to_string(moved.join("keep.txt"))?, "keep");
    std::fs::remove_dir_all(moved)?;
    Ok(())
}

#[test]
fn legacy_settings_are_removed_without_losing_terminal_splits_or_sessions() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    state.open_terminal_tab(home)?;
    let legacy = state.window().active_pane.ok_or("pane")?;
    let terminal = state.split_pane(legacy, muxy_app_core::Direction::Right)?;
    let session = SessionId::new(20).ok_or("session")?;
    state.set_pane_session(terminal, Some(session))?;
    state.focus_pane(legacy)?;
    state.toggle_zoom(legacy)?;
    let mut stored = serde_json::to_value(&state)?;
    stored["projects"][0]["tabs"][0]["panes"][0]["content"] =
        serde_json::json!({"type": "settings"});
    let restored: AppState = serde_json::from_value(stored)?;
    assert_eq!(restored.home().tabs.len(), 1);
    let tab = &restored.home().tabs[0];
    assert_eq!(tab.panes.len(), 1);
    assert_eq!(tab.panes[0].id, terminal);
    assert_eq!(
        tab.panes[0].content,
        PaneContent::Terminal {
            session: Some(session)
        }
    );
    assert_eq!(tab.layout.leaves(), vec![terminal]);
    assert_eq!(tab.zoomed, None);
    assert_eq!(restored.window().active_pane, Some(terminal));
    assert!(
        !serde_json::to_value(restored.window())?["focus_history"]
            .as_array()
            .ok_or("focus history")?
            .contains(&serde_json::to_value(legacy)?)
    );
    assert!(restored.pending_discards(ServerId::local()).is_empty());
    assert_eq!(
        serde_json::from_value::<AppState>(serde_json::to_value(&restored)?)?,
        restored
    );
    Ok(())
}
