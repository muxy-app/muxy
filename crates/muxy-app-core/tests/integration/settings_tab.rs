use muxy_app_core::{AppState, Direction, PaneContent, ServerId, store};
use muxy_protocol::SessionId;

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn settings_reuses_one_tab_across_projects_and_restores_it() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let tab = state.show_settings_tab(home, "Settings")?;
    let pane = state.window().active_pane.ok_or("settings pane")?;
    assert!(state.is_settings_pane(pane));
    let directory = tempfile::tempdir()?;
    let project = state.add_project(ServerId::local(), directory.path().to_path_buf())?;
    state.open_terminal_tab(project)?;
    assert_eq!(state.show_settings_tab(project, "Preferences")?, tab);
    assert_eq!(state.current_project().id, home);
    assert_eq!(state.window().active_pane, Some(pane));
    assert_eq!(state.home().tabs[0].panes[0].title, "Preferences");
    let path = directory.path().join("state.json");
    store::save(&path, &state)?;
    let mut loaded = store::load(&path)?;
    assert_eq!(loaded, state);
    assert_eq!(loaded.settings_tab(), Some((home, tab)));
    loaded.close_tab(home, tab)?;
    assert!(loaded.pending_discards(ServerId::local()).is_empty());
    assert!(loaded.settings_tab().is_none());
    assert_ne!(loaded.show_settings_tab(home, "Settings")?, tab);
    Ok(())
}

#[test]
fn settings_from_a_remote_project_opens_in_local_home() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let remote = ServerId::new();
    let mut home = state.home().descriptor();
    home.id = muxy_app_core::ProjectId::new();
    let project = home.id;
    state.apply_catalog(
        remote,
        &muxy_protocol::CatalogPage {
            server: muxy_protocol::ServerIdentity::from_u128(2),
            home: project,
            revision: 1,
            projects: vec![home],
            next: None,
            legacy_home: None,
        },
    )?;
    state.select_project(project)?;
    let tab = state.show_settings_tab(project, "Settings")?;
    assert_eq!(state.settings_tab(), Some((state.home().id, tab)));
    assert!(
        state
            .project(project)
            .ok_or("remote project")?
            .tabs
            .is_empty()
    );
    Ok(())
}

#[test]
fn settings_recovers_from_a_missing_project_without_duplicates() -> TestResult {
    let directory = tempfile::tempdir()?;
    let folder = directory.path().join("project");
    std::fs::create_dir(&folder)?;
    let mut state = AppState::bootstrap()?;
    let project = state.add_project(ServerId::local(), folder.clone())?;
    let first = state.show_settings_tab(project, "Settings")?;
    let settings = state.window().active_pane.ok_or("settings pane")?;
    let terminal = state.split_pane(settings, Direction::Right)?;
    let session = SessionId::new(21).ok_or("session")?;
    state.set_pane_session(terminal, Some(session))?;
    std::fs::remove_dir(&folder)?;
    state.refresh_project_statuses();
    let second = state.show_settings_tab(project, "Settings")?;
    assert_ne!(first, second);
    assert_eq!(state.settings_tab(), Some((state.home().id, second)));
    std::fs::create_dir(&folder)?;
    state.refresh_project_statuses();
    let tabs = &state.project(project).ok_or("project")?.tabs;
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].panes.len(), 1);
    assert_eq!(tabs[0].panes[0].id, terminal);
    assert_eq!(
        state.pane_session(terminal),
        Some((ServerId::local(), session))
    );
    assert!(state.pending_discards(ServerId::local()).is_empty());
    Ok(())
}

#[test]
fn restoring_settings_preserves_terminal_splits_sessions_and_focus() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let tab = state.open_terminal_tab(home)?;
    let settings = state.window().active_pane.ok_or("pane")?;
    let terminal = state.split_pane(settings, Direction::Right)?;
    let session = SessionId::new(20).ok_or("session")?;
    state.set_pane_session(terminal, Some(session))?;
    state.focus_pane(settings)?;
    let mut stored = serde_json::to_value(&state)?;
    stored["projects"][0]["tabs"][0]["panes"][0]["content"] =
        serde_json::json!({"type": "settings"});
    let mut restored: AppState = serde_json::from_value(stored)?;
    assert_eq!(restored.settings_tab(), Some((home, tab)));
    assert_eq!(restored.window().active_pane, Some(settings));
    assert_eq!(restored.home().tabs[0].panes.len(), 2);
    assert_eq!(
        restored.home().tabs[0].panes[0].content,
        PaneContent::Settings
    );
    assert_eq!(
        restored.pane_session(terminal),
        Some((ServerId::local(), session))
    );
    assert!(restored.pending_discards(ServerId::local()).is_empty());
    restored.focus_pane(terminal)?;
    restored.toggle_zoom(terminal)?;
    assert_eq!(restored.show_settings_tab(home, "Settings")?, tab);
    assert_eq!(restored.window().active_pane, Some(settings));
    assert_eq!(restored.home().tabs[0].zoomed, Some(settings));
    Ok(())
}

#[test]
fn backup_restore_keeps_one_settings_pane_and_preserves_other_splits() -> TestResult {
    let mut current = AppState::bootstrap()?;
    let home = current.home().id;
    let original = current.show_settings_tab(home, "Settings")?;
    let backup = current.configuration_backup();
    current.close_tab(home, original)?;
    let directory = tempfile::tempdir()?;
    let project = current.add_project(ServerId::local(), directory.path().to_path_buf())?;
    current.show_settings_tab(project, "Settings")?;
    let settings = current.window().active_pane.ok_or("settings pane")?;
    let terminal = current.split_pane(settings, Direction::Right)?;
    let session = SessionId::new(22).ok_or("session")?;
    current.set_pane_session(terminal, Some(session))?;
    for restored in [
        backup.restore_configuration(&current)?,
        backup.merge_configuration(&current)?,
    ] {
        assert_eq!(
            restored
                .panes()
                .filter(|placed| placed.pane.content == PaneContent::Settings)
                .count(),
            1
        );
        assert_eq!(
            restored.pane_session(terminal),
            Some((ServerId::local(), session))
        );
        assert!(restored.pending_discards(ServerId::local()).is_empty());
        let loaded: AppState = serde_json::from_value(serde_json::to_value(&restored)?)?;
        assert_eq!(loaded, restored);
    }
    Ok(())
}

#[test]
fn loading_duplicate_settings_preserves_the_focused_one_and_terminal_splits() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    state.show_settings_tab(home, "Settings")?;
    let first = state.window().active_pane.ok_or("settings pane")?;
    let terminal = state.split_pane(first, Direction::Right)?;
    let session = SessionId::new(23).ok_or("session")?;
    state.set_pane_session(terminal, Some(session))?;
    state.open_terminal_tab(home)?;
    let active = state.window().active_pane.ok_or("active pane")?;
    let mut stored = serde_json::to_value(&state)?;
    stored["projects"][0]["tabs"][1]["panes"][0]["content"] =
        serde_json::json!({"type": "settings"});
    let loaded: AppState = serde_json::from_value(stored)?;
    assert_eq!(loaded.window().active_pane, Some(active));
    assert!(loaded.is_settings_pane(active));
    assert!(!loaded.is_settings_pane(first));
    assert_eq!(
        loaded.pane_session(terminal),
        Some((ServerId::local(), session))
    );
    assert_eq!(loaded.home().tabs[0].layout.leaves(), vec![terminal]);
    assert!(loaded.pending_discards(ServerId::local()).is_empty());
    Ok(())
}
