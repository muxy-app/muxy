mod interaction;
mod refresh;

use super::*;
use muxy_app_core::agent_sessions::{Roots, Session};
use muxy_core::shortcuts::ShortcutId;
use muxy_protocol::AgentProvider;
use muxy_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition};

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let point = cx.debug_bounds(selector).expect(selector).center();
    click_at(cx, point);
}

fn click_at(cx: &mut VisualTestContext, point: gpui::Point<gpui::Pixels>) {
    cx.simulate_mouse_down(point, gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(point, gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn session(model: &AppModel, directory: &std::path::Path) -> Session {
    let path = directory.join("session.jsonl");
    std::fs::write(&path, "").expect("fixture");
    Session {
        provider: AgentProvider::Codex,
        id: "session-123".into(),
        title: "Fix regression".into(),
        directory: model.state.current_project().directory.clone(),
        path,
        modified: std::time::SystemTime::UNIX_EPOCH,
    }
}

#[gpui::test]
fn filters_persist_and_sessions_share_the_panel_host(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("directory");
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.toggle_agent_sessions(window, cx);
        assert!(
            model
                .panels
                .placement(&PanelId::new(super::super::agent_sessions::PANEL))
                .is_some()
        );
        let session = session(model, directory.path());
        model.agent_sessions.listing.sessions.push(session);
        model.filter_agent_sessions(cx);
        assert_eq!(model.agent_sessions.filtered, [0]);
        model.toggle_session_provider(AgentProvider::Codex, cx);
        assert!(model.agent_sessions.filtered.is_empty());
        assert!(
            !muxy_app_core::settings::Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("settings")
                .appearance
                .agent_sessions_providers
                .contains(&AgentProvider::Codex)
        );
        model.toggle_session_provider(AgentProvider::Codex, cx);
        let input = model.agent_sessions.input.clone().expect("input");
        input.update(cx, |input, cx| input.set_text("missing", cx));
        model.filter_agent_sessions(cx);
        assert!(model.agent_sessions.filtered.is_empty());
        input.update(cx, |input, cx| input.set_text("REGRESSION codex", cx));
        model.filter_agent_sessions(cx);
        assert_eq!(model.agent_sessions.filtered, [0]);
        model.toggle_session_providers(window, cx);
        model.place_panel(
            PanelPlacement::new("extension", PanelPosition::Right, PanelMode::Pinned),
            cx,
        );
        assert!(!model.appearance.agent_sessions_visible);
        assert!(model.overlay.is_none());
        assert!(model.agent_sessions.listing.sessions.is_empty());
        model.toggle_agent_sessions(window, cx);
        assert_eq!(model.panels.len(), 1);
        model.set_agents_panel_dock(muxy_app_core::settings::AgentsPanelDock::Right, window, cx);
        assert!(!model.appearance.agent_sessions_visible);
        assert!(model.agents_panel_shown());
    });
}

#[gpui::test]
fn resume_is_transactional_and_uses_the_selected_project(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("directory");
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.toggle_agent_sessions(window, cx);
        let session = session(model, directory.path());
        model.agent_sessions.listing.sessions.push(session.clone());
        let tabs = model.state.current_project().tabs.len();
        let path = model.path.clone();
        model.path = directory.path().to_path_buf();
        model.resume_agent_session(&session, cx);
        assert_eq!(model.state.current_project().tabs.len(), tabs);
        model.path = path;
        model.resume_agent_session(&session, cx);
        assert_eq!(model.state.current_project().tabs.len(), tabs + 1);
        let pane = model.active_pane().expect("pane");
        assert_eq!(
            model.state.startup_command(pane),
            Some("codex resume session-123")
        );
        assert_eq!(
            model.initial_directories.get(&pane),
            Some(&model.state.current_project().directory)
        );
        model.agent_sessions.scope = None;
        model.resume_agent_session(&session, cx);
        assert_eq!(
            model.state.current_project().tabs.len(),
            tabs + 1,
            "stale rows cannot resume"
        );
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Input(..))),
        "no command runs before terminal attachment"
    );
}

#[gpui::test]
fn background_discovery_does_not_publish_after_hiding(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("directory");
    let state = AppState::bootstrap().expect("state");
    let sessions = directory.path().join(".xal/sessions/project");
    std::fs::create_dir_all(&sessions).expect("sessions");
    std::fs::write(sessions.join("session.jsonl"), format!("{}\n", serde_json::json!({"type":"meta","meta":{"id":"session-123","cwd":state.current_project().directory}}))).expect("fixture");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.agent_sessions.roots = Some(Roots::from_home(directory.path()));
        model.toggle_agent_sessions(window, cx);
        assert!(model.agent_sessions.task.is_some());
        model.toggle_agent_sessions(window, cx);
    });
    cx.run_until_parked();
    view.update_in(cx, |model, window, cx| {
        assert!(model.agent_sessions.listing.sessions.is_empty());
        model.toggle_agent_sessions(window, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.agent_sessions.listing.sessions.len(), 1);
        assert_eq!(model.agent_sessions.filtered, [0]);
    });
}

#[gpui::test]
fn switching_projects_during_discovery_and_remote_paths_do_not_leak_history(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().expect("directory");
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let project = state
        .add_project(ServerId::local(), directory.path().to_path_buf())
        .expect("project");
    let sessions = directory.path().join(".xal/sessions/project");
    std::fs::create_dir_all(&sessions).expect("sessions");
    for (name, cwd) in [
        ("home", state.home().directory.clone()),
        ("project", directory.path().to_path_buf()),
    ] {
        std::fs::write(
            sessions.join(format!("{name}.jsonl")),
            format!(
                "{}\n",
                serde_json::json!({"type":"meta","meta":{"id":name,"cwd":cwd}})
            ),
        )
        .expect("fixture");
    }
    state.select_project(home).expect("home");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.agent_sessions.roots = Some(Roots::from_home(directory.path()));
        model.toggle_agent_sessions(window, cx);
        model.select_project(project, cx);
        assert!(model.agent_sessions.listing.sessions.is_empty());
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(model.agent_sessions.listing.sessions.len(), 1);
        assert_eq!(model.agent_sessions.listing.sessions[0].id, "project");
        let remote = ServerId::new();
        let remote_home = ProjectId::new();
        let mut page = servers::page(2, remote_home, &[]);
        page.projects[0].directory = muxy_protocol::ServerPath::from(directory.path());
        model
            .state
            .apply_catalog(remote, &page)
            .expect("remote catalog");
        model
            .state
            .select_project(remote_home)
            .expect("remote home");
        model.sync_agent_sessions(cx);
        assert!(model.agent_sessions.scope.is_none());
        assert!(model.agent_sessions.listing.sessions.is_empty());
        assert!(
            model.agent_sessions.task.is_none(),
            "remote directories never scan the local disk"
        );
    });
}

#[gpui::test]
fn titlebar_toggle_and_resize_persist_without_saving_during_drag(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();
    let button = cx
        .debug_bounds("toggle-agent-sessions")
        .expect("titlebar toggle")
        .center();
    cx.simulate_mouse_down(button, gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(button, gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let start = cx
        .debug_bounds("panel-resize-builtin:agent-sessions")
        .expect("resize grip")
        .center();
    let end = start - gpui::point(px(80.0), px(0.0));
    cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), Modifiers::none());
    view.read_with(cx, |model, _| {
        assert!(model.settings.appearance.agent_sessions_width.is_none());
    });
    cx.simulate_mouse_up(end, gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let appearance =
            muxy_app_core::settings::Appearance::load(&model.path.with_file_name("settings.toml"))
                .expect("saved appearance");
        assert!(appearance.agent_sessions_visible);
        assert!((appearance.agent_sessions_width.expect("saved width") - 400.0).abs() < 1.0);
    });
}

#[gpui::test]
fn search_is_clickable_and_filters_after_a_project_switch(cx: &mut TestAppContext) {
    use gpui::Focusable;
    let directory = tempfile::tempdir().expect("directory");
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), directory.path().to_path_buf())
        .expect("project");
    state.select_project(state.home().id).expect("home");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update_in(cx, |model, window, cx| {
        model.toggle_agent_sessions(window, cx);
        model.select_project(project, cx);
        model
            .agent_sessions
            .listing
            .sessions
            .push(session(model, directory.path()));
        model.filter_agent_sessions(cx);
    });
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds("agent-session-search-input")
        .expect("search input");
    assert!(
        bounds.size.width > px(100.0) && bounds.size.height > px(12.0),
        "search input has usable bounds: {bounds:?}"
    );
    cx.simulate_mouse_down(bounds.center(), gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(bounds.center(), gpui::MouseButton::Left, Modifiers::none());
    view.update_in(cx, |model, window, cx| {
        assert!(
            model
                .agent_sessions
                .input
                .as_ref()
                .expect("input")
                .focus_handle(cx)
                .is_focused(window)
        );
    });
    cx.simulate_input("missing");
    view.read_with(cx, |model, cx| {
        assert_eq!(
            model
                .agent_sessions
                .input
                .as_ref()
                .expect("input")
                .read(cx)
                .text(),
            "missing"
        );
        assert!(model.agent_sessions.filtered.is_empty());
    });
}

#[gpui::test]
fn a_user_shortcut_toggles_the_panel(cx: &mut TestAppContext) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    assert!(
        boot.settings
            .keymap
            .chord(ShortcutId::ToggleAgentSessions)
            .is_none()
    );
    boot.settings.keymap = boot
        .settings
        .keymap
        .with_binding(
            ShortcutId::ToggleAgentSessions.name(),
            Some("ctrl-alt-shift-y".parse().expect("chord")),
        )
        .expect("binding");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-alt-shift-y");
    view.read_with(cx, |model, _| {
        assert!(model.appearance.agent_sessions_visible);
    });
    cx.simulate_keystrokes("ctrl-alt-shift-y");
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.agent_sessions_visible);
    });
}

#[gpui::test]
fn header_moves_pins_and_floats_the_panel(cx: &mut TestAppContext) {
    use muxy_app_core::settings::{AgentSessionsPosition, Appearance};
    let panel = PanelId::new(super::super::agent_sessions::PANEL);
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update_in(cx, |model, window, cx| {
        model.toggle_agent_sessions(window, cx);
    });
    let saved = |model: &AppModel| {
        Appearance::load(&model.path.with_file_name("settings.toml")).expect("saved appearance")
    };

    click(cx, "agent-sessions-position");
    view.read_with(cx, |model, _| {
        let placement = model.panels.placement(&panel).expect("placement");
        assert_eq!(placement.position, PanelPosition::Bottom);
        assert_eq!(placement.mode, PanelMode::Pinned);
        assert_eq!(
            saved(model).agent_sessions_position,
            AgentSessionsPosition::Bottom
        );
    });
    let start = cx
        .debug_bounds("panel-resize-builtin:agent-sessions")
        .expect("resize grip")
        .center();
    let end = start - gpui::point(px(0.0), px(60.0));
    cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(gpui::MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let appearance = saved(model);
        assert!((appearance.agent_sessions_height.expect("saved height") - 280.0).abs() < 1.0);
        assert!(appearance.agent_sessions_width.is_none());
    });

    click(cx, "agent-sessions-pin");
    view.read_with(cx, |model, _| {
        let placement = model.panels.placement(&panel).expect("placement");
        assert_eq!(placement.mode, PanelMode::Floating);
        assert!(!saved(model).agent_sessions_pinned);
    });
    click_at(cx, gpui::point(px(600.0), px(200.0)));
    view.read_with(cx, |model, _| {
        assert!(
            !model.appearance.agent_sessions_visible,
            "outside click hides it"
        );
        assert!(model.panels.placement(&panel).is_none());
    });

    click(cx, "toggle-agent-sessions");
    view.read_with(cx, |model, _| {
        let placement = model.panels.placement(&panel).expect("reopened");
        assert_eq!(placement.position, PanelPosition::Bottom);
        assert_eq!(placement.mode, PanelMode::Floating);
    });
    click(cx, "toggle-agent-sessions");
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.agent_sessions_visible);
    });
}
