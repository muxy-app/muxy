use muxy_app_core::{AppState, Direction, PaneContent, restore, webview::WebviewDescriptor};
use serde_json::json;

fn descriptor() -> WebviewDescriptor {
    WebviewDescriptor {
        owner: "test".into(),
        kind: "editor".into(),
        data: json!({"file":"main.rs"}),
    }
}

#[test]
fn webview_round_trip_singleton_and_terminal_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let (tab, pane) = state.open_webview(home, descriptor(), "Editor", false)?;
    let terminal = state.split_pane(pane, Direction::Right)?;
    assert_ne!(pane, terminal);
    let plan = restore::plan(&state, &[]);
    assert_eq!(plan.create, vec![terminal]);
    assert!(plan.attach.is_empty());
    assert!(state.session_references().is_empty());
    let mut updated = descriptor();
    updated.data = json!({"file":"other.rs"});
    assert_eq!(
        state.open_webview(home, updated.clone(), "Editor", true)?,
        (tab, pane)
    );
    assert_eq!(state.window().active_pane, Some(pane));
    let saved = serde_json::to_vec(&state)?;
    let mut restored: AppState = serde_json::from_slice(&saved)?;
    assert_eq!(restored, state);
    restored.clear_terminal_panes()?;
    assert_eq!(restored.home().tabs.len(), 1);
    assert_eq!(restored.home().tabs[0].panes.len(), 1);
    assert_eq!(
        restored.home().tabs[0].panes[0].content,
        PaneContent::Webview(updated)
    );
    assert!(restore::plan(&restored, &[]).create.is_empty());
    restored.close_pane(pane)?;
    assert!(restored.home().tabs.is_empty());
    Ok(())
}

#[test]
fn independent_instances_and_project_scoped_singletons() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first = state.open_webview(home, descriptor(), "Editor", false)?;
    let second = state.open_webview(home, descriptor(), "Editor", false)?;
    assert_ne!(first, second);
    let other = state.add_project(std::env::temp_dir())?;
    let third = state.open_webview(other, descriptor(), "Editor", true)?;
    assert_ne!(first, third);
    assert_eq!(state.project(other).ok_or("project")?.tabs.len(), 1);
    state.close_pane(first.1)?;
    assert_eq!(state.window().active_pane, Some(third.1));
    Ok(())
}
