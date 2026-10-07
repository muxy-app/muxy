use muxy_app_core::{AppState, Direction, ServerId, store};
use muxy_protocol::{
    SandboxInfo, SandboxNetwork, SandboxPolicy, SandboxSpec, ServerPath, SessionId, SessionInfo,
};

#[test]
fn splits_and_saved_panes_keep_their_sandbox_policy() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let pane = state.home().tabs[0].panes[0].id;
    let spec = SandboxSpec {
        workspace: ServerPath(b"/workspace".to_vec()),
        policy: SandboxPolicy::default(),
    };
    state.set_sandbox(pane, Some(spec.clone()))?;
    let split = state.split_pane(pane, Direction::Right)?;
    assert_eq!(state.sandbox(split), Some(&spec));
    let path = std::env::temp_dir().join(format!(
        "muxy-sandbox-state-{}.json",
        muxy_protocol::OperationId::new()
    ));
    store::save(&path, &state)?;
    let loaded = store::load(&path)?;
    std::fs::remove_file(path)?;
    assert_eq!(loaded.sandbox(pane), Some(&spec));
    assert_eq!(loaded.sandbox(split), Some(&spec));
    Ok(())
}

#[test]
fn server_policy_replaces_a_stale_client_copy() -> Result<(), Box<dyn std::error::Error>> {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let pane = state.home().tabs[0].panes[0].id;
    let id = SessionId::new(42).ok_or("session ID")?;
    state.set_pane_session(pane, Some(id))?;
    let spec = SandboxSpec {
        workspace: ServerPath(b"/workspace".to_vec()),
        policy: SandboxPolicy {
            network: SandboxNetwork::Domains,
            domains: vec!["example.com".into()],
            ..Default::default()
        },
    };
    state.sync_sandboxes(
        ServerId::local(),
        &[SessionInfo {
            id,
            project: state.home().id,
            directory: spec.workspace.clone(),
            sandbox: Some(SandboxInfo {
                spec: spec.clone(),
                backend_version: "0.79.0".into(),
            }),
        }],
    );
    assert_eq!(state.sandbox(pane), Some(&spec));
    state.sync_sandboxes(
        ServerId::local(),
        &[SessionInfo {
            id,
            project: state.home().id,
            directory: spec.workspace,
            sandbox: None,
        }],
    );
    assert!(state.sandbox(pane).is_none());
    Ok(())
}
