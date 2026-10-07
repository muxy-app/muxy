use muxy_app_core::{AppState, TabCloseScope, TabId};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

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
