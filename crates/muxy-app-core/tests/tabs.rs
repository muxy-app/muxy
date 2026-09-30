use muxy_app_core::{AppState, Direction, TabCloseScope, TabId, TabSide};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn order(state: &AppState) -> Vec<TabId> {
    state.home().tabs.iter().map(|tab| tab.id).collect()
}

#[test]
fn customization_round_trips_and_reset_restores_live_pane_titles() -> Result {
    let mut state = AppState::bootstrap()?;
    let id = state.open_terminal_tab(state.home().id)?;
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right)?;
    state.set_tab_title(id, Some("  Build 日本語  ".into()))?;
    state.set_tab_color(id, Some("#7aa2f7".parse()?))?;
    state.toggle_tab_pin(id)?;
    state.set_pane_title(first, "shell")?;
    state.set_pane_title(second, "editor")?;
    let mut restored: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    assert_eq!(restored, state);
    assert_eq!(restored.home().tabs[0].title(Some(second)), "Build 日本語");
    restored.set_tab_title(id, None)?;
    assert_eq!(restored.home().tabs[0].title(Some(second)), "editor");
    assert_eq!(restored.home().tabs[0].title(None), "shell");
    restored.set_tab_title(id, Some(" \n ".into()))?;
    restored.set_tab_color(id, None)?;
    restored.toggle_tab_pin(id)?;
    assert!(restored.home().tabs[0].custom_title.is_none());
    assert!(restored.home().tabs[0].color.is_none());
    assert!(!restored.home().tabs[0].pinned);
    let mut legacy = serde_json::to_value(&state)?;
    let tab = &mut legacy["projects"][0]["tabs"][0];
    for field in ["custom_title", "color", "pinned"] {
        tab.as_object_mut().ok_or("tab object")?.remove(field);
    }
    let legacy: AppState = serde_json::from_value(legacy)?;
    assert!(legacy.home().tabs[0].custom_title.is_none());
    assert!(legacy.home().tabs[0].color.is_none());
    assert!(!legacy.home().tabs[0].pinned);
    Ok(())
}

#[test]
fn pinning_reordering_and_adjacent_creation_respect_the_pinned_boundary() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first = state.open_terminal_tab(home)?;
    let second = state.open_terminal_tab(home)?;
    let third = state.open_terminal_tab(home)?;
    let focus = state.window().clone();
    state.toggle_tab_pin(second)?;
    state.toggle_tab_pin(third)?;
    assert_eq!(order(&state), [second, third, first]);
    assert_eq!(state.window(), &focus);
    state.move_tab(home, 2, 0)?;
    assert_eq!(order(&state), [second, third, first]);
    state.move_tab(home, 0, 2)?;
    assert_eq!(order(&state), [third, second, first]);
    let fourth = state.open_terminal_tab_adjacent(home, third, TabSide::Left)?;
    assert_eq!(order(&state), [third, second, fourth, first]);
    let fifth = state.open_terminal_tab_adjacent(home, fourth, TabSide::Right)?;
    assert_eq!(order(&state), [third, second, fourth, fifth, first]);
    state.toggle_tab_pin(third)?;
    assert_eq!(order(&state), [second, third, fourth, fifth, first]);
    let before = state.clone();
    assert!(
        state
            .open_terminal_tab_adjacent(home, TabId::new(), TabSide::Right)
            .is_err()
    );
    assert_eq!(state, before);
    Ok(())
}

#[test]
fn filtered_tab_reordering_preserves_hidden_tabs_pins_and_selection() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let mut tabs = Vec::new();
    for _ in 0..7 {
        tabs.push(state.open_terminal_tab(home)?);
    }
    state.toggle_tab_pin(tabs[0])?;
    let before = state.clone();
    let visible = [tabs[0], tabs[1], tabs[3], tabs[5]];
    state.move_visible_tab(home, tabs[1], tabs[5], &visible)?;
    assert_eq!(
        order(&state),
        [
            tabs[0], tabs[3], tabs[2], tabs[5], tabs[4], tabs[1], tabs[6]
        ]
    );
    assert_eq!(state.window(), before.window());
    state.move_visible_tab(home, tabs[1], tabs[3], &visible)?;
    assert_eq!(state, before);
    for (from, to) in [
        (tabs[1], tabs[0]),
        (tabs[0], tabs[5]),
        (tabs[2], tabs[3]),
        (tabs[1], tabs[2]),
        (TabId::new(), tabs[1]),
        (tabs[1], TabId::new()),
    ] {
        state.move_visible_tab(home, from, to, &visible)?;
        assert_eq!(state, before);
    }
    Ok(())
}

#[test]
fn bulk_close_targets_follow_project_order_and_skip_pinned_tabs() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first = state.open_terminal_tab(home)?;
    let second = state.open_terminal_tab(home)?;
    let third = state.open_terminal_tab(home)?;
    let fourth = state.open_terminal_tab(home)?;
    state.toggle_tab_pin(first)?;
    assert_eq!(
        state.home().closable_tabs(third, TabCloseScope::Other),
        [second, fourth]
    );
    assert_eq!(
        state.home().closable_tabs(third, TabCloseScope::Left),
        [second]
    );
    assert_eq!(
        state.home().closable_tabs(third, TabCloseScope::Right),
        [fourth]
    );
    assert!(
        state
            .home()
            .closable_tabs(first, TabCloseScope::Left)
            .is_empty()
    );
    assert!(
        state
            .home()
            .closable_tabs(TabId::new(), TabCloseScope::Other)
            .is_empty()
    );
    Ok(())
}
