#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use muxy_app_core::{AppState, Axis, Layout, PaneContent, ServerId, project_layouts::Config};
use muxy_protocol::SessionId;

fn state() -> AppState {
    AppState::bootstrap().expect("state")
}

#[test]
fn yaml_and_json_build_the_same_nested_layout_and_commands() {
    let yaml = Config::parse("layout: horizontal\npanes:\n  - tab:\n      name: ' editor '\n      command: [ ' cd src ', '', 'npm test' ]\n  - layout: vertical\n    panes:\n      - tab: top\n      - tab: {}\n").expect("yaml");
    let json = Config::parse(r#"{"layout":"horizontal","panes":[{"tab":{"name":"editor","command":["cd src","npm test"]}},{"layout":"vertical","panes":[{"tab":"top"},{"tab":{}}]}]}"#).expect("json");
    assert_eq!(yaml, json);
    let mut state = state();
    let panes = state
        .apply_project_layout(state.home().id, &yaml)
        .expect("layout");
    let tab = &state.home().tabs[0];
    assert_eq!(tab.custom_title.as_deref(), Some("editor"));
    assert_eq!(
        tab.panes
            .iter()
            .map(|pane| pane.title.as_str())
            .collect::<Vec<_>>(),
        ["editor", "top", "Terminal"]
    );
    assert_eq!(tab.layout.leaves(), panes);
    assert_eq!(state.window().active_pane, Some(panes[0]));
    assert_eq!(state.startup_command(panes[0]), Some("cd src && npm test"));
    assert_eq!(state.startup_command(panes[1]), Some("top"));
    assert_eq!(state.startup_command(panes[2]), None);
    assert!(
        matches!(&tab.layout, Layout::Split { axis: Axis::Horizontal, second, .. } if matches!(second.as_ref(), Layout::Split { axis: Axis::Vertical, .. }))
    );
    let restored: AppState =
        serde_json::from_str(&serde_json::to_string(&state).expect("encode")).expect("decode");
    assert_eq!(restored, state);
}

#[test]
fn legacy_extra_tabs_keep_depth_first_order_and_separate_startup_commands() {
    let config =
        Config::parse("panes:\n - tabs: [nvim, 'npm test']\n - tabs: [top, 'git status', {}]\n")
            .expect("config");
    let mut state = state();
    state
        .apply_project_layout(state.home().id, &config)
        .expect("apply");
    let tabs = &state.home().tabs;
    assert_eq!(tabs.len(), 4);
    assert_eq!(tabs[0].panes.len(), 2);
    assert_eq!(
        tabs.iter()
            .map(|tab| tab.custom_title.as_deref())
            .collect::<Vec<_>>(),
        [Some("nvim"), Some("npm"), Some("git"), Some("Terminal")]
    );
    assert_eq!(state.startup_command(tabs[1].panes[0].id), Some("npm test"));
    assert!(tabs.iter().skip(1).all(|tab| tab.panes.len() == 1));
}

#[test]
fn branches_take_precedence_and_many_siblings_have_valid_balanced_ratios() {
    let mut state = state();
    let config = Config::parse(&format!(
        "tab: ignored\npanes:\n{}",
        " - tab: {}\n".repeat(64)
    ))
    .expect("64 panes");
    let panes = state
        .apply_project_layout(state.home().id, &config)
        .expect("apply");
    assert_eq!(panes.len(), 64);
    assert!(state.home().tabs[0].layout.validate().is_ok());
    let config = Config::parse("panes: [{tab: left}, {tab: middle}, {tab: right}]").expect("three");
    state
        .apply_project_layout(state.home().id, &config)
        .expect("apply");
    let Layout::Split { ratio, .. } = state.home().tabs[0].layout else {
        panic!("split");
    };
    assert!((ratio - 1.0 / 3.0).abs() < f32::EPSILON);
}

#[test]
fn rejects_invalid_or_unbounded_layouts_instead_of_partially_replacing_tabs() {
    for text in [
        "",
        "null",
        "tabs: []",
        "panes: []",
        "tab: ''",
        "panes: [{tab: ok}, broken]",
        "tab: {command: [ok, 42]}",
        "tab: [",
        "panes: wrong",
    ] {
        assert!(Config::parse(text).is_err(), "{text}");
    }
    assert!(Config::parse(&" ".repeat(256 * 1024 + 1)).is_err());
    assert!(Config::parse(&format!("panes:\n{}", " - tab: {}\n".repeat(65))).is_err());
    let deep = format!(
        "{}{{\"tab\":{{}}}}{}",
        "{\"panes\":[".repeat(34),
        "]}".repeat(34)
    );
    assert!(Config::parse(&deep).is_err());
}

#[test]
fn replacement_closes_only_target_references_and_cancels_pending_creations() {
    let mut state = state();
    let target = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    let old = state.open_terminal_tab(target).expect("tab");
    let pane = state.project(target).expect("project").tabs[0].panes[0].id;
    let session = SessionId::new(91).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    state.toggle_tab_pin(old).expect("pin");
    state.prepare_creation(pane, &std::env::temp_dir());
    state.open_terminal_tab(target).expect("pending tab");
    let pending = state.window().active_pane.expect("pending pane");
    state.prepare_creation(pending, &std::env::temp_dir());
    let home_tab = state.open_terminal_tab(state.home().id).expect("home");
    let home_pane = state.home().tabs[0].panes[0].id;
    state
        .set_pane_session(home_pane, Some(session))
        .expect("shared reference");
    state = serde_json::from_str(&serde_json::to_string(&state).expect("encode")).expect("restore");
    let config = Config::parse("tab: echo hello").expect("layout");
    let panes = state.apply_project_layout(target, &config).expect("apply");
    assert_eq!(state.home().tabs[0].id, home_tab);
    assert!(state.pending_discards(ServerId::local()).is_empty());
    assert_eq!(
        state.pending_cancellations(ServerId::local()),
        [pending.creation_token()]
    );
    assert!(matches!(
        state.project(target).expect("target").tabs[0].panes[0].content,
        PaneContent::Terminal { session: None }
    ));
    assert_ne!(panes[0], pane);
    let command = state.take_startup_command(panes[0]);
    assert_eq!(command.as_deref(), Some("echo hello"));
    assert!(state.take_startup_command(panes[0]).is_none());
    state
        .apply_project_layout(state.home().id, &config)
        .expect("close last reference");
    assert_eq!(state.pending_discards(ServerId::local()), [session]);
}
