#![allow(clippy::unwrap_used, clippy::expect_used)]

use muxy_app_core::{AppState, Direction, TabId};

fn tabs() -> (AppState, [TabId; 3]) {
    let mut state = AppState::bootstrap().unwrap();
    let ids = std::array::from_fn(|_| state.open_terminal_tab(state.home().id).unwrap());
    (state, ids)
}

#[test]
fn pane_moves_and_center_swaps_never_change_pane_identity_or_content() {
    let (mut state, [a, _, _]) = tabs();
    state.select_tab(state.home().id, a).unwrap();
    let first = state.window().active_pane.unwrap();
    let second = state.split_pane(first, Direction::Right).unwrap();
    let third = state.split_pane(first, Direction::Down).unwrap();
    let before = state.home().tabs[0].panes.clone();
    for edge in [
        Some(Direction::Left),
        Some(Direction::Right),
        Some(Direction::Up),
        Some(Direction::Down),
        None,
    ] {
        state.move_pane(third, second, edge).unwrap();
        assert_eq!(state.home().tabs[0].panes, before);
        assert_eq!(state.window().active_pane, Some(third));
        assert_eq!(state.home().tabs[0].layout.leaves().len(), 3);
        serde_json::from_slice::<AppState>(&serde_json::to_vec(&state).unwrap()).unwrap();
    }
    let before = state.clone();
    assert!(state.move_pane(first, first, None).is_err());
    assert!(
        state
            .move_pane(
                first,
                state.home().tabs[1].panes[0].id,
                Some(Direction::Left)
            )
            .is_err()
    );
    assert_eq!(state, before);
}

#[test]
fn restarting_a_terminal_keeps_its_place_title_zoom_and_focus() {
    let (mut state, [a, _, _]) = tabs();
    state.select_tab(state.home().id, a).unwrap();
    let first = state.window().active_pane.unwrap();
    let second = state.split_pane(first, Direction::Right).unwrap();
    state.set_pane_title(second, "Build").unwrap();
    state.set_pane_proxy(second, true).unwrap();
    state.toggle_zoom(second).unwrap();
    let before = state.home().tabs[0].layout.rects();

    let restarted = state.restart_terminal(second).unwrap();

    let tab = &state.home().tabs[0];
    let pane = &tab.panes[1];
    assert_ne!(restarted, second);
    assert_eq!(pane.id, restarted);
    assert_eq!(pane.title, "Build");
    assert!(!pane.proxy);
    assert_eq!(
        pane.content,
        muxy_app_core::PaneContent::Terminal { session: None }
    );
    assert_eq!(tab.layout.leaves(), vec![first, restarted]);
    assert_eq!(
        tab.layout
            .rects()
            .iter()
            .map(|(_, rect)| *rect)
            .collect::<Vec<_>>(),
        before.iter().map(|(_, rect)| *rect).collect::<Vec<_>>()
    );
    assert_eq!(tab.zoomed, Some(restarted));
    assert_eq!(state.window().active_pane, Some(restarted));
    serde_json::from_slice::<AppState>(&serde_json::to_vec(&state).unwrap()).unwrap();
}

#[test]
fn only_proxied_panes_save_the_proxy_flag() {
    let (mut state, [a, _, _]) = tabs();
    state.select_tab(state.home().id, a).unwrap();
    let pane = state.window().active_pane.unwrap();
    let saved = serde_json::to_string(&state).unwrap();
    assert!(!saved.contains("\"proxy\""));
    state.set_pane_proxy(pane, true).unwrap();
    let restored: AppState = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    assert!(restored.home().tabs[0].panes[0].proxy);
    let older: AppState = serde_json::from_str(&saved).unwrap();
    assert!(!older.home().tabs[0].panes[0].proxy);
}
