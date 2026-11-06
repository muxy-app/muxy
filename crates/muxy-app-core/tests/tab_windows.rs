use std::error::Error;

use muxy_app_core::{
    AppState, Direction, PaneDrop, PaneId, ProjectId, ServerId, SessionMove, TabId, TabPlace,
    WindowBounds,
};
use muxy_protocol::{CatalogPage, ProjectDescriptor, ServerIdentity, ServerPath, SessionId};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const BOUNDS: WindowBounds = WindowBounds {
    x: 40.0,
    y: 60.0,
    width: 800.0,
    height: 500.0,
};

/// Fails unless the state survives being saved and read back.
fn reload(state: &AppState) -> TestResult<AppState> {
    Ok(serde_json::from_slice(&serde_json::to_vec(state)?)?)
}

fn session(id: u64) -> TestResult<SessionId> {
    Ok(SessionId::new(id).ok_or("session IDs are nonzero")?)
}

fn first_pane(state: &AppState, tab: TabId) -> TestResult<PaneId> {
    state
        .projects()
        .iter()
        .flat_map(|project| &project.tabs)
        .find(|candidate| candidate.id == tab)
        .and_then(|tab| tab.layout.leaves().first().copied())
        .ok_or_else(|| "tab has no pane".into())
}

fn listed(state: &AppState, project: ProjectId) -> Vec<TabId> {
    state
        .project(project)
        .map(|project| project.listed_tabs().map(|tab| tab.id).collect())
        .unwrap_or_default()
}

/// Home and one more local project, each with tabs.
fn two_projects() -> TestResult<(AppState, ProjectId, ProjectId)> {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let other = state.add_project(ServerId::local(), std::env::temp_dir())?;
    Ok((state, home, other))
}

#[test]
fn a_detached_tab_leaves_the_main_window_and_survives_a_restart() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    let b_pane = first_pane(&state, b)?;
    let window = state.move_tab_to_window(b, Some(BOUNDS))?;
    assert_eq!(listed(&state, home), [a]);
    assert_eq!(state.window().selected_tab.get(&home), Some(&a));
    assert_eq!(state.window().active_pane, Some(first_pane(&state, a)?));
    assert_eq!(state.pane_window(b_pane), Some(window));
    assert!(state.select_tab(home, b).is_err());
    let restored = reload(&state)?;
    let (_, tab, saved) = restored.tab_window(window).ok_or("window")?;
    assert_eq!((tab.id, saved.bounds), (b, Some(BOUNDS)));
    assert_eq!(saved.active_pane(), Some(b_pane));
    Ok(())
}

#[test]
fn detaching_the_only_tab_leaves_the_project_without_a_selection() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    state.move_tab_to_window(a, None)?;
    assert!(listed(&state, home).is_empty());
    assert_eq!(state.window().selected_tab.get(&home), None);
    assert_eq!(state.window().active_pane, None);
    reload(&state)?;
    Ok(())
}

#[test]
fn focus_inside_a_tab_window_never_moves_the_main_windows_focus() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    let window = state.move_tab_to_window(b, None)?;
    let b_first = first_pane(&state, b)?;
    let b_second = state.split_pane(b_first, Direction::Right)?;
    let main = state.window().active_pane;
    assert_eq!(main, Some(first_pane(&state, a)?));
    let active = |state: &AppState| {
        state
            .tab_window(window)
            .and_then(|(_, _, window)| window.active_pane())
    };
    assert_eq!(active(&state), Some(b_second));
    state.focus_pane(b_first)?;
    assert_eq!(active(&state), Some(b_first));
    state.close_pane(b_first)?;
    assert_eq!(active(&state), Some(b_second));
    assert_eq!(state.window().active_pane, main);
    reload(&state)?;
    Ok(())
}

#[test]
fn a_tab_window_returns_to_any_place_in_any_project() -> TestResult {
    let (mut state, home, other) = two_projects()?;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    let c = state.open_terminal_tab(other)?;
    let pane = first_pane(&state, b)?;
    state.set_pane_session(pane, Some(session(7)?))?;
    let window = state.move_tab_to_window(b, None)?;
    let moves = state.return_tab(
        window,
        TabPlace {
            project: other,
            before: Some(c),
        },
    )?;
    assert_eq!(
        moves,
        [SessionMove {
            server: ServerId::local(),
            session: session(7)?,
            project: other,
        }]
    );
    assert_eq!(listed(&state, home), [a]);
    assert_eq!(listed(&state, other), [b, c]);
    assert_eq!(state.window().current_project, other);
    assert_eq!(state.window().selected_tab.get(&other), Some(&b));
    assert_eq!(state.window().active_pane, Some(pane));
    assert!(state.tab_window(window).is_none());
    reload(&state)?;
    Ok(())
}

#[test]
fn a_tab_moves_between_projects_and_the_old_project_selects_its_neighbor() -> TestResult {
    let (mut state, home, other) = two_projects()?;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    let moves = state.move_tab_to(b, TabPlace::end(other))?;
    assert!(moves.is_empty());
    assert_eq!(listed(&state, home), [a]);
    assert_eq!(state.window().selected_tab.get(&home), Some(&a));
    assert_eq!(listed(&state, other), [b]);
    assert_eq!(state.window().current_project, other);
    assert!(
        state
            .move_tab_to(
                a,
                TabPlace {
                    project: other,
                    before: Some(a),
                }
            )
            .is_err()
    );
    reload(&state)?;
    Ok(())
}

#[test]
fn a_pane_becomes_a_new_tab_or_window_without_cancelling_its_terminal() -> TestResult {
    let (mut state, home, other) = two_projects()?;
    let a = state.open_terminal_tab(home)?;
    let first = first_pane(&state, a)?;
    let second = state.split_pane(first, Direction::Right)?;
    let third = state.split_pane(first, Direction::Down)?;
    state.plan_creation(second, Some(&std::env::temp_dir()), Some(true));
    let (tab, moves) = state.move_pane_to_new_tab(second, TabPlace::end(other))?;
    assert!(moves.is_empty());
    assert_eq!(state.starting_proxy(second), Some(true));
    assert!(state.pending_cancellations(ServerId::local()).is_empty());
    assert_eq!(listed(&state, other), [tab]);
    assert_eq!(state.window().active_pane, Some(second));
    let window = state.move_pane_to_window(third, None)?;
    let (_, detached, _) = state.tab_window(window).ok_or("window")?;
    assert_eq!(detached.layout.leaves(), [third]);
    assert_eq!(listed(&state, home), [a]);
    assert_eq!(
        state
            .project(home)
            .and_then(|project| project.tabs.iter().find(|tab| tab.id == a))
            .map(|tab| tab.layout.leaves()),
        Some(vec![first])
    );
    reload(&state)?;
    Ok(())
}

#[test]
fn a_pane_docks_into_another_tab_and_its_emptied_tab_closes() -> TestResult {
    let (mut state, home, other) = two_projects()?;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(other)?;
    let a_pane = first_pane(&state, a)?;
    let b_pane = first_pane(&state, b)?;
    state.set_pane_session(a_pane, Some(session(3)?))?;
    let moves = state.move_pane_to(
        a_pane,
        PaneDrop::Dock {
            anchor: b_pane,
            edge: Direction::Left,
            level: 0,
        },
    )?;
    assert_eq!(moves.len(), 1);
    assert!(listed(&state, home).is_empty());
    let tab = state
        .project(other)
        .and_then(|project| project.tabs.iter().find(|tab| tab.id == b))
        .ok_or("tab")?;
    assert_eq!(tab.layout.leaves(), [a_pane, b_pane]);
    assert_eq!(state.window().active_pane, Some(a_pane));
    reload(&state)?;
    Ok(())
}

#[test]
fn a_pane_swaps_with_a_pane_in_a_tab_window() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    let a_first = first_pane(&state, a)?;
    let a_second = state.split_pane(a_first, Direction::Right)?;
    let window = state.move_tab_to_window(b, None)?;
    let b_pane = first_pane(&state, b)?;
    state.focus_pane(a_second)?;
    state.move_pane_to(a_second, PaneDrop::Swap(b_pane))?;
    let (_, detached, saved) = state.tab_window(window).ok_or("window")?;
    assert_eq!(detached.layout.leaves(), [a_second]);
    assert_eq!(saved.active_pane(), Some(a_second));
    assert_eq!(state.window().active_pane, Some(b_pane));
    assert_eq!(
        state
            .home()
            .tabs
            .iter()
            .find(|tab| tab.id == a)
            .map(|tab| tab.layout.leaves()),
        Some(vec![a_first, b_pane])
    );
    reload(&state)?;
    Ok(())
}

#[test]
fn the_last_pane_leaving_a_tab_window_closes_it() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    let window = state.move_tab_to_window(b, None)?;
    let b_pane = first_pane(&state, b)?;
    state.move_pane_to(
        b_pane,
        PaneDrop::Dock {
            anchor: first_pane(&state, a)?,
            edge: Direction::Right,
            level: 0,
        },
    )?;
    assert!(state.tab_window(window).is_none());
    assert_eq!(listed(&state, home), [a]);
    reload(&state)?;
    Ok(())
}

#[test]
fn tab_groups_ignore_tabs_in_their_own_window() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    let c = state.open_terminal_tab(home)?;
    state.split_tab(b, a, Direction::Right)?;
    let window = state.move_tab_to_window(c, None)?;
    assert!(state.split_tab(c, a, Direction::Left).is_err());
    let groups = state.home().groups().ok_or("groups")?;
    assert!(groups.group_of(c).is_none());
    reload(&state)?;
    state.return_tab(
        window,
        TabPlace {
            project: home,
            before: Some(a),
        },
    )?;
    let groups = state.home().groups().ok_or("groups")?;
    assert_eq!(
        groups.group_of(c).map(|group| group.tabs().to_vec()),
        Some(vec![c, a])
    );
    reload(&state)?;
    Ok(())
}

#[test]
fn closing_a_tab_windows_tab_keeps_the_main_window_as_it_was() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    state.move_tab_to_window(b, None)?;
    let main = state.window().clone();
    state.close_tab(home, b)?;
    assert_eq!(state.window(), &main);
    assert_eq!(listed(&state, home), [a]);
    reload(&state)?;
    Ok(())
}

#[test]
fn moves_between_servers_are_refused() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    let remote = ServerId::new();
    let remote_home = ProjectId::new();
    state.apply_catalog(
        remote,
        &CatalogPage {
            server: ServerIdentity::from_u128(2),
            home: remote_home,
            revision: 1,
            projects: vec![ProjectDescriptor {
                id: remote_home,
                home: true,
                name: "Home".into(),
                icon: None,
                logo: None,
                color: "#808080".into(),
                directory: ServerPath::from("/home/dev".to_owned()),
                kind: None,
                parent_id: None,
            }],
            next: None,
            legacy_home: None,
        },
    )?;
    let before = state.clone();
    assert!(!state.can_move_into(home, remote_home));
    assert!(state.move_tab_to(a, TabPlace::end(remote_home)).is_err());
    assert!(
        state
            .move_pane_to_new_tab(first_pane(&state, a)?, TabPlace::end(remote_home))
            .is_err()
    );
    assert_eq!(state, before);
    Ok(())
}

#[test]
fn a_tab_window_shows_its_own_focused_pane() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    state.open_terminal_tab(home)?;
    let b = state.open_terminal_tab(home)?;
    let first = first_pane(&state, b)?;
    state.move_tab_to_window(b, None)?;
    let second = state.split_pane(first, Direction::Right)?;
    state.focus_pane(first)?;
    state.focus_pane(second)?;
    let tab = state
        .projects()
        .iter()
        .flat_map(|project| &project.tabs)
        .find(|tab| tab.id == b)
        .ok_or("tab")?;
    assert_eq!(state.shown_pane(tab), Some(second));
    Ok(())
}

#[test]
fn a_pane_stays_put_when_its_destination_is_gone() -> TestResult {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let a = state.open_terminal_tab(home)?;
    let first = first_pane(&state, a)?;
    let second = state.split_pane(first, Direction::Right)?;
    let before = state.clone();
    let place = TabPlace {
        project: home,
        before: Some(TabId::new()),
    };
    assert!(state.move_pane_to_new_tab(second, place).is_err());
    assert_eq!(state, before);
    Ok(())
}
