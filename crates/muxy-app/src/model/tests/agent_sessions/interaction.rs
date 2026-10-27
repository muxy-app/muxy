use super::*;
use gpui::Focusable;
use std::time::{Duration, SystemTime};

fn populate(model: &mut AppModel, directory: &std::path::Path, cx: &mut Context<AppModel>) {
    model.agent_sessions.roots = Some(Roots::from_home(directory));
    for (index, (provider, title, days)) in [
        (AgentProvider::Claude, "Fix regression in session discovery across all supported providers without installing any hooks", 0),
        (AgentProvider::Codex, "Fix regression in terminal search", 2),
        (AgentProvider::Xal, "Improve settings navigation", 10),
    ].into_iter().enumerate() {
        let mut session = session(model, directory);
        session.provider = provider;
        session.id = format!("session-{index}");
        session.title = title.into();
        session.modified = SystemTime::now() - Duration::from_secs(days * 86_400);
        model.agent_sessions.listing.sessions.push(session);
    }
    model.filter_agent_sessions(cx);
}

#[gpui::test]
fn first_open_completes_all_batches_without_retoggling(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("directory");
    let state = AppState::bootstrap().expect("state");
    let cwd = &state.current_project().directory;
    let claude = directory.path().join(".claude/projects/project");
    std::fs::create_dir_all(&claude).expect("claude directory");
    for index in 0..10 {
        let path = claude.join(format!("{index}.jsonl"));
        std::fs::write(
            &path,
            format!(
                "{}\n",
                serde_json::json!({"type":"user","sessionId":format!("claude-{index}"),"cwd":cwd})
            ),
        )
        .expect("fixture");
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("file")
            .set_len(1024 * 1024)
            .expect("large history");
    }
    for (path, metadata) in [
        (
            ".codex/sessions/2026/01/01/session.jsonl",
            serde_json::json!({"type":"session_meta","payload":{"id":"codex-1","cwd":cwd}}),
        ),
        (
            ".xal/sessions/project/session.jsonl",
            serde_json::json!({"type":"meta","meta":{"id":"xal-1","cwd":cwd}}),
        ),
    ] {
        let path = directory.path().join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        std::fs::write(path, format!("{metadata}\n")).expect("fixture");
    }
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.agent_sessions.roots = Some(Roots::from_home(directory.path()));
        model.toggle_agent_sessions(window, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.agent_sessions.listing.sessions.len(), 12);
        assert_eq!(model.agent_sessions.filtered.len(), 12);
        assert!(!model.agent_sessions.listing.pending);
        assert!(model.agent_sessions.task.is_none());
        assert_eq!(model.agent_sessions.list.counts[0], 10);
        assert_eq!(model.agent_sessions.list.counts[1], 1);
        assert_eq!(model.agent_sessions.list.counts[6], 1);
    });
}

#[gpui::test]
fn provider_filters_recover_from_empty_results(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("directory");
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update_in(cx, |model, window, cx| {
        model.toggle_agent_sessions(window, cx);
        populate(model, directory.path(), cx);
    });
    click(cx, "session-provider-filter");
    click(cx, "session-select-none");
    click(cx, "session-provider-1");
    view.read_with(cx, |model, _| {
        assert_eq!(model.agent_sessions.filtered, [1]);
        assert_eq!(&model.agent_sessions.list.counts[..2], &[1, 1]);
    });
    cx.simulate_keystrokes("escape");
    cx.simulate_input("missing");
    click(cx, "session-provider-filter");
    click(cx, "session-select-none");
    view.read_with(cx, |model, _| {
        assert!(model.appearance.agent_sessions_providers.is_empty());
        assert!(
            model
                .agent_sessions
                .list
                .counts
                .iter()
                .all(|count| *count == 0)
        );
    });
    click(cx, "session-select-all");
    click(cx, "session-select-none");
    click(cx, "session-provider-0");
    click(cx, "session-provider-1");
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.appearance.agent_sessions_providers,
            [AgentProvider::Claude, AgentProvider::Codex]
        );
        let saved =
            muxy_app_core::settings::Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("settings");
        assert_eq!(
            saved.appearance.agent_sessions_providers,
            model.appearance.agent_sessions_providers
        );
    });
    cx.simulate_keystrokes("escape");
    click(cx, "reset-session-filters");
    view.read_with(cx, |model, cx| {
        assert_eq!(model.agent_sessions.filtered, [0, 1, 2]);
        assert!(
            model
                .agent_sessions
                .input
                .as_ref()
                .expect("input")
                .read(cx)
                .text()
                .is_empty()
        );
    });
}

#[gpui::test]
fn keyboard_resume_keeps_its_identity_when_results_arrive(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("directory");
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update_in(cx, |model, window, cx| {
        model.toggle_agent_sessions(window, cx);
        populate(model, directory.path(), cx);
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("tab");
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::SessionProviders)));
    });
    cx.simulate_keystrokes("escape");
    cx.simulate_input("regression");
    cx.simulate_keystrokes("down");
    view.update(cx, |model, cx| {
        assert_eq!(model.agent_sessions.list.selected_index(), Some(1));
        let mut added = model.agent_sessions.listing.sessions[0].clone();
        added.id = "newly-discovered".into();
        model.agent_sessions.listing.sessions.insert(0, added);
        model.filter_agent_sessions(cx);
        assert_eq!(model.agent_sessions.list.selected_index(), Some(2));
    });
    cx.simulate_keystrokes("enter");
    view.update_in(cx, |model, window, cx| {
        assert_eq!(
            model
                .state
                .startup_command(model.active_pane().expect("pane")),
            Some("codex resume session-1")
        );
        model
            .agent_sessions
            .input
            .as_ref()
            .expect("input")
            .focus_handle(cx)
            .focus(window);
        model.focus_requested = false;
    });
    cx.simulate_keystrokes("escape");
    view.update_in(cx, |model, window, cx| {
        let input = model.agent_sessions.input.as_ref().expect("input");
        assert!(input.read(cx).text().is_empty());
        assert!(input.focus_handle(cx).is_focused(window));
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.update_in(cx, |model, window, cx| {
        assert!(
            !model
                .agent_sessions
                .input
                .as_ref()
                .expect("input")
                .focus_handle(cx)
                .is_focused(window)
        );
    });
}
