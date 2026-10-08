use muxy_app_core::{AppState, Direction, PaneId, TabCloseScope, TabId};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn tabs(
    state: &mut AppState,
    count: usize,
) -> std::result::Result<Vec<TabId>, Box<dyn std::error::Error>> {
    let home = state.home().id;
    (0..count)
        .map(|_| Ok(state.open_terminal_tab(home)?))
        .collect()
}

fn group_tabs(state: &AppState) -> Vec<Vec<TabId>> {
    state.home().groups().map_or_else(Vec::new, |groups| {
        groups
            .layout()
            .leaves()
            .into_iter()
            .filter_map(|id| groups.group(id))
            .map(|group| group.tabs().to_vec())
            .collect()
    })
}

fn first_pane(state: &AppState, tab: TabId) -> Option<PaneId> {
    state
        .home()
        .tabs
        .iter()
        .find(|candidate| candidate.id == tab)
        .and_then(|tab| tab.layout.leaves().first().copied())
}

#[test]
fn a_groups_only_tab_cannot_split_off_itself() -> Result {
    let mut state = AppState::bootstrap()?;
    let [a] = tabs(&mut state, 1)?[..] else {
        return Err("tabs".into());
    };
    assert!(state.split_tab(a, a, Direction::Left).is_err());
    assert!(state.home().groups().is_none());
    let [b] = tabs(&mut state, 1)?[..] else {
        return Err("tabs".into());
    };
    state.split_tab(b, a, Direction::Left)?;
    assert!(state.split_tab(b, b, Direction::Down).is_err());
    assert_eq!(group_tabs(&state), [vec![b], vec![a]]);
    Ok(())
}

#[test]
fn new_tabs_open_in_the_focused_or_anchor_group() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let [a, b] = tabs(&mut state, 2)?[..] else {
        return Err("tabs".into());
    };
    state.split_tab(b, a, Direction::Right)?;
    let c = state.open_terminal_tab(home)?;
    assert_eq!(group_tabs(&state), [vec![a], vec![b, c]]);
    let d = state.open_terminal_tab_adjacent(home, a, muxy_app_core::TabSide::Right)?;
    assert_eq!(group_tabs(&state), [vec![a, d], vec![b, c]]);
    assert_eq!(state.visible_tabs(home), [d, c]);
    Ok(())
}

#[test]
fn closing_a_groups_last_tab_removes_it_and_focuses_the_neighbor() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let [a, b, c] = tabs(&mut state, 3)?[..] else {
        return Err("tabs".into());
    };
    state.split_tab(b, a, Direction::Right)?;
    state.split_tab(c, b, Direction::Down)?;
    let a_pane = first_pane(&state, a).ok_or("pane")?;
    let a_split = state.split_pane(a_pane, Direction::Right)?;
    state.select_tab(home, b)?;
    state.select_tab(home, c)?;
    state.close_tab(home, c)?;
    assert_eq!(group_tabs(&state), [vec![a], vec![b]]);
    assert_eq!(state.window().selected_tab.get(&home), Some(&a));
    assert_eq!(state.window().active_pane, Some(a_split));
    state.close_tab(home, a)?;
    assert!(state.home().groups().is_none());
    assert_eq!(state.window().selected_tab.get(&home), Some(&b));
    assert_eq!(state.window().active_pane, first_pane(&state, b));
    Ok(())
}

#[test]
fn moving_into_a_group_places_the_tab_and_dissolves_an_emptied_group() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let [a, b, c] = tabs(&mut state, 3)?[..] else {
        return Err("tabs".into());
    };
    state.split_tab(c, a, Direction::Right)?;
    state.move_tab_to_group(b, c, Some(c))?;
    assert_eq!(group_tabs(&state), [vec![a], vec![b, c]]);
    assert_eq!(state.window().selected_tab.get(&home), Some(&b));
    state.move_tab_to_group(a, c, None)?;
    assert!(state.home().groups().is_none());
    let order: Vec<_> = state.home().tabs.iter().map(|tab| tab.id).collect();
    assert_eq!(order, [b, c, a]);
    assert_eq!(state.window().selected_tab.get(&home), Some(&a));
    Ok(())
}

#[test]
fn reordering_and_pinning_keep_group_order_in_step_with_the_project() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let [a, b, c, d] = tabs(&mut state, 4)?[..] else {
        return Err("tabs".into());
    };
    state.split_tab(b, a, Direction::Right)?;
    state.move_tab_to_group(d, b, None)?;
    assert_eq!(group_tabs(&state), [vec![a, c], vec![b, d]]);
    let from = state
        .home()
        .tabs
        .iter()
        .position(|tab| tab.id == d)
        .ok_or("tab")?;
    state.move_tab(home, from, 0)?;
    assert_eq!(group_tabs(&state), [vec![a, c], vec![d, b]]);
    state.toggle_tab_pin(c)?;
    assert_eq!(group_tabs(&state), [vec![c, a], vec![d, b]]);
    assert_eq!(
        state.home().closable_tabs(a, TabCloseScope::Other),
        Vec::<TabId>::new()
    );
    assert_eq!(state.home().closable_tabs(b, TabCloseScope::Left), [d]);
    assert_eq!(state.home().group_tabs(d), [d, b]);
    Ok(())
}

#[test]
fn groups_persist_and_invalid_saved_groups_fall_back_to_one() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let [a, b] = tabs(&mut state, 2)?[..] else {
        return Err("tabs".into());
    };
    let legacy = serde_json::to_value(&state)?;
    assert!(legacy["projects"][0].get("groups").is_none());
    state.split_tab(b, a, Direction::Down)?;
    state.set_group_ratio(home, &[], 0.3)?;
    let restored: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    assert_eq!(restored, state);
    let mut broken = serde_json::to_value(&state)?;
    broken["projects"][0]["groups"]["groups"][1]["tabs"] = serde_json::json!([]);
    let repaired: AppState = serde_json::from_value(broken)?;
    assert!(repaired.home().groups().is_none());
    assert_eq!(repaired.home().tabs.len(), 2);
    let mut unknown = serde_json::to_value(&state)?;
    unknown["projects"][0]["groups"]["groups"][0]["tabs"] =
        serde_json::json!([a.to_string(), TabId::new().to_string()]);
    let pruned: AppState = serde_json::from_value(unknown)?;
    assert_eq!(group_tabs(&pruned), [vec![a], vec![b]]);
    Ok(())
}
