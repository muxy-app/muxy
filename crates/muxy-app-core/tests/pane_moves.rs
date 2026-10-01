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
