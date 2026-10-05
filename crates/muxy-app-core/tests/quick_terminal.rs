use muxy_app_core::{AppState, ServerId};
use muxy_protocol::SessionId;

#[test]
fn quick_terminal_is_lazy_persistent_and_does_not_own_workspace_tabs()
-> Result<(), Box<dyn std::error::Error>> {
    let mut state = AppState::bootstrap()?;
    assert!(state.quick_terminal().is_none());
    let window = state.window().clone();
    let pane = state.ensure_quick_terminal();
    assert_eq!(state.ensure_quick_terminal(), pane);
    state.set_pane_session(pane, SessionId::new(99))?;
    let restored: AppState = serde_json::from_str(&serde_json::to_string(&state)?)?;
    assert_eq!(restored.quick_terminal(), state.quick_terminal());
    assert_eq!(restored.window(), &window);
    assert!(restored.home().tabs.is_empty());
    state.close_quick_terminal();
    assert!(state.quick_terminal().is_none());
    assert_eq!(
        state.pending_discards(ServerId::local()),
        &[SessionId::new(99).ok_or("session")?]
    );
    Ok(())
}
