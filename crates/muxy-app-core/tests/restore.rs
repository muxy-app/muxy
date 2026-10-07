use std::error::Error;
use std::fs;

use muxy_app_core::{AppState, ProjectId, ServerId, restore, store};
use muxy_protocol::{ServerPath, SessionId, SessionInfo};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn closing_offline_persists_cleanup_without_restoring_the_tab() -> TestResult {
    let path = std::env::temp_dir().join(format!("muxy-close-{}.json", ProjectId::new()));
    let mut state = AppState::bootstrap()?;
    let tab = state.open_terminal_tab(state.home().id)?;
    let session = SessionId::new(42).ok_or("zero ID")?;
    state.set_pane_session(state.home().tabs[0].panes[0].id, Some(session))?;
    state.queue_discard(ServerId::local(), session);
    state.queue_discard(ServerId::local(), session);
    state.close_tab(state.home().id, tab)?;
    store::save(&path, &state)?;
    let mut loaded = store::load(&path)?;
    assert!(loaded.home().tabs.is_empty());
    assert_eq!(loaded.pending_discards(ServerId::local()), &[session]);
    assert_eq!(
        restore::plan(&loaded, ServerId::local(), &[]),
        restore::RestorePlan::default()
    );
    loaded.complete_discard(ServerId::local(), session);
    store::save(&path, &loaded)?;
    assert!(
        store::load(&path)?
            .pending_discards(ServerId::local())
            .is_empty()
    );
    fs::remove_file(path)?;
    Ok(())
}

#[test]
fn mixed_restore_identifies_dead_panes_without_mutating_selection() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    for _ in 0..3 {
        state.open_terminal_tab(home)?;
    }
    let panes: Vec<_> = state
        .home()
        .tabs
        .iter()
        .map(|tab| tab.panes[0].id)
        .collect();
    let live = SessionId::new(11).ok_or("zero ID")?;
    let missing = SessionId::new(22).ok_or("zero ID")?;
    state.set_pane_session(panes[0], Some(live))?;
    state.set_pane_session(panes[1], Some(missing))?;
    state.select_tab(home, state.home().tabs[1].id)?;
    let before = state.clone();
    let plan = restore::plan(
        &state,
        ServerId::local(),
        &[SessionInfo {
            sandbox: None,
            project: state.home().id,
            id: live,
            directory: ServerPath(b"/tmp".to_vec()),
        }],
    );
    assert_eq!(plan.attach, vec![(panes[0], live)]);
    assert_eq!(plan.close, vec![(panes[1], missing)]);
    assert_eq!(plan.create, vec![panes[2]]);
    assert_eq!(state, before);
    let all_missing = restore::plan(&state, ServerId::local(), &[]);
    assert_eq!(
        all_missing.close,
        vec![(panes[0], live), (panes[1], missing)]
    );
    assert!(all_missing.attach.is_empty());
    assert_eq!(state.home().tabs.len(), 3);
    Ok(())
}

#[test]
fn empty_restore_does_not_create_a_tab_or_adopt_an_unreferenced_session() -> TestResult {
    let state = AppState::bootstrap()?;
    let live = SessionInfo {
        sandbox: None,
        project: state.home().id,
        id: SessionId::new(1).ok_or("zero ID")?,
        directory: ServerPath(b"/tmp".to_vec()),
    };
    assert_eq!(
        restore::plan(&state, ServerId::local(), &[live]),
        restore::RestorePlan::default()
    );
    assert!(state.home().tabs.is_empty());
    Ok(())
}

#[test]
fn restore_covers_hidden_projects_without_creating_tabs_for_empty_projects() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let live = SessionId::new(24).ok_or("session")?;
    let ended = SessionId::new(25).ok_or("session")?;
    let mut panes = Vec::new();
    for session in [Some(live), Some(ended), None] {
        let project = state.add_project(ServerId::local(), std::env::temp_dir())?;
        state.open_terminal_tab(project)?;
        let pane = state.current_project().tabs[0].panes[0].id;
        state.set_pane_session(pane, session)?;
        panes.push(pane);
    }
    state.add_project(ServerId::local(), std::env::temp_dir())?;
    let loaded: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    let plan = restore::plan(
        &loaded,
        ServerId::local(),
        &[SessionInfo {
            sandbox: None,
            project: loaded.projects()[1].id,
            id: live,
            directory: ServerPath(b"/tmp".to_vec()),
        }],
    );
    assert_eq!(plan.attach, [(panes[0], live)]);
    assert_eq!(plan.close, [(panes[1], ended)]);
    assert_eq!(plan.create, [panes[2]]);
    assert!(loaded.current_project().tabs.is_empty());
    Ok(())
}
