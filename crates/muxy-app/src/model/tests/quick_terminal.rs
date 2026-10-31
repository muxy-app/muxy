use super::*;
use crate::model::quick_terminal::QuickClose;

#[gpui::test]
fn quick_terminal_uses_home_without_changing_workspace_and_reattaches_after_hide(
    cx: &mut TestAppContext,
) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        model.new_tab(cx);
        let window = model.state.window().clone();
        let quick = model.state.ensure_quick_terminal();
        model.quick.visible = true;
        model.sync_visible(cx);
        model.start_attach(quick, Size { cols: 100, rows: 30 }, cx);
        let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
        assert!(work.iter().any(|work| matches!(work, Work::Attach { pane, session: None, directory, .. } if *pane == quick && *directory == model.state.home().directory)));
        assert_eq!(model.state.window(), &window);
        assert!(!model.visible_panes().contains(&quick));
        let session = SessionId::new(701).expect("session");
        model.state.set_pane_session(quick, Some(session)).expect("session");
        model.pending.remove(&quick);
        model.quick.visible = false;
        model.sync_visible(cx);
        assert!(!model.grids.contains_key(&quick));
        assert_eq!(model.state.pane_session(quick), Some((ServerId::local(), session)));
        assert!(!requests.try_iter().any(|(_, work)| matches!(work, Work::Discard(_, _))));
        model.quick.visible = true;
        model.sync_visible(cx);
        model.start_attach(quick, Size { cols: 100, rows: 30 }, cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::Attach { pane, session: Some(id), .. } if pane == quick && id == session)));
        assert_eq!(model.state.window(), &window);
    });
}

#[gpui::test]
fn quick_terminal_disable_queues_cleanup_offline_and_preserves_preferences(
    cx: &mut TestAppContext,
) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let pane = model.state.ensure_quick_terminal();
        let session = SessionId::new(702).expect("session");
        model
            .state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let mut settings = model.settings.quick_terminal.clone();
        settings.enabled = false;
        settings.width = 900;
        model.apply_quick_settings(settings, cx).expect("disable");
        assert!(model.state.quick_terminal().is_none());
        let restored = store::load(&model.path).expect("state");
        assert!(
            restored
                .pending_discards(ServerId::local())
                .contains(&session)
        );
        assert_eq!(model.settings.quick_terminal.width, 900);
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Discard(_, _)))
        );
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        assert!(
            requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
        );
    });
}

/// Asks to close the Quick Terminal and answers the server's check with
/// `name` as the foreground process.
fn close_quick_terminal_running(
    model: &mut AppModel,
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
    name: &str,
    cx: &mut Context<AppModel>,
) {
    let (_, session) = model
        .state
        .quick_terminal()
        .and_then(|pane| model.state.pane_session(pane.id))
        .expect("session");
    model.request_close_quick_terminal(cx);
    let Some(QuickClose::Checking(check)) = model.quick.closing else {
        panic!("the close was not checked");
    };
    assert!(requests.try_iter().any(|(_, work)| matches!(
        work,
        Work::CheckClose { check: sent, session: asked, .. } if sent == check && asked == session
    )));
    let result = Ok(Some(muxy_protocol::ForegroundProcess {
        name: name.into(),
        is_shell: name == "zsh",
    }));
    let checked = Update::CloseChecked {
        check,
        session,
        result,
    };
    model.receive(local(model.servers.local.generation, checked), cx);
}

#[gpui::test]
fn quick_terminal_close_prompts_only_for_commands_it_alone_shows(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        let session = SessionId::new(702).expect("session");
        let pane = model.state.ensure_quick_terminal();
        model
            .state
            .set_pane_session(pane, Some(session))
            .expect("session");

        close_quick_terminal_running(model, &requests, "sleep", cx);
        assert!(model.state.quick_terminal().is_some());
        assert_eq!(model.quick.closing, Some(QuickClose::Confirming));

        model.quick.closing = None;
        close_quick_terminal_running(model, &requests, "zsh", cx);
        assert!(model.state.quick_terminal().is_none());
        assert!(model.quick.closing.is_none());

        let pane = model.state.ensure_quick_terminal();
        model
            .state
            .set_pane_session(pane, Some(session))
            .expect("session");
        model
            .state
            .open_terminal_tab(model.state.home().id)
            .expect("tab");
        let tab_pane = model.state.home().tabs[0].panes[0].id;
        model
            .state
            .set_pane_session(tab_pane, Some(session))
            .expect("session");
        model.request_close_quick_terminal(cx);
        assert!(model.state.quick_terminal().is_none());
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::CheckClose { .. }))
        );
    });
}

#[gpui::test]
fn quick_terminal_pin_is_saved_and_survives_reopening(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let saved = |model: &AppModel| {
            muxy_app_core::settings::Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("settings")
                .quick_terminal
                .pinned
        };
        assert!(!model.settings.quick_terminal.pinned);
        model.toggle_quick_terminal_pin(cx);
        assert!(model.settings.quick_terminal.pinned);
        assert!(saved(model));
        model.hide_quick_terminal(false, cx);
        assert!(model.settings.quick_terminal.pinned);
        model.toggle_quick_terminal_pin(cx);
        assert!(!saved(model));
    });
}

#[gpui::test]
fn quick_terminal_closes_without_discarding_when_its_session_ends(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let pane = model.state.ensure_quick_terminal();
        let stale = SessionId::new(703).expect("session");
        model
            .state
            .set_pane_session(pane, Some(stale))
            .expect("session");
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        assert!(model.state.quick_terminal().is_none());

        let pane = model.state.ensure_quick_terminal();
        let ended = SessionId::new(704).expect("session");
        model
            .state
            .set_pane_session(pane, Some(ended))
            .expect("session");
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Event(ClientEvent::SessionEnded {
                    session: ended,
                    reason: ExitReason::Ended,
                }),
            ),
            cx,
        );
        assert!(model.state.quick_terminal().is_none());

        let pane = model.state.ensure_quick_terminal();
        let missing = SessionId::new(705).expect("session");
        model
            .state
            .set_pane_session(pane, Some(missing))
            .expect("session");
        model.receive(
            (
                ServerId::local(),
                1,
                Update::AttachFailed {
                    pane,
                    session: Some(missing),
                    created: false,
                    error: missing_session_error(),
                },
            ),
            cx,
        );
        assert!(model.state.quick_terminal().is_none());
        assert!(model.state.pending_discards(ServerId::local()).is_empty());
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Discard(_, _)))
        );
    });
}

#[gpui::test]
fn quick_terminal_theme_overrides_the_app_and_gives_its_session_colors(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        let pane = model.state.ensure_quick_terminal();
        let session = SessionId::new(703).expect("session");
        model
            .state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let sent = || {
            requests
                .try_iter()
                .filter_map(|(_, work)| match work {
                    Work::SessionColors(id, colors) => Some((id, colors)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let _ = sent();
        assert!(model.quick_theme().is_none());
        let app = model.palette.terminal_colors();
        let other = if model.dark { "Muxy Light" } else { "Muxy" };
        let mut settings = model.settings.quick_terminal.clone();
        settings.set_theme(model.dark, Some(other.into()));
        model
            .apply_quick_settings(settings.clone(), cx)
            .expect("theme");
        let (_, palette) = model.quick_theme().expect("own theme");
        let colors = palette.terminal_colors();
        assert_ne!(colors, app);
        assert_eq!(model.palette.terminal_colors(), app);
        assert_eq!(sent(), [(session, Some(colors))]);
        model.refresh_quick_terminal(cx);
        assert!(sent().is_empty());
        settings.set_theme(model.dark, None);
        model
            .apply_quick_settings(settings, cx)
            .expect("same as app");
        assert!(model.quick_theme().is_none());
        assert_eq!(sent(), [(session, None)]);
    });
}

#[gpui::test]
fn quick_terminal_session_starts_and_attaches_with_its_theme(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        let mut settings = model.settings.quick_terminal.clone();
        let other = if model.dark { "Muxy Light" } else { "Muxy" };
        settings.set_theme(model.dark, Some(other.into()));
        model.apply_quick_settings(settings, cx).expect("theme");
        let colors = model
            .quick_theme()
            .expect("own theme")
            .1
            .terminal_colors();
        let quick = model.state.ensure_quick_terminal();
        model.quick.visible = true;
        model.sync_visible(cx);
        model.start_attach(quick, Size { cols: 100, rows: 30 }, cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(
            work,
            Work::Attach { pane, session: None, colors: Some(sent), .. } if pane == quick && sent == colors
        )));
        let session = SessionId::new(704).expect("session");
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    pane: quick,
                    session,
                    attachment: attachment(),
                    created: true,
                },
            ),
            cx,
        );
        assert!(requests.try_iter().any(|(_, work)| matches!(
            work,
            Work::SessionColors(id, Some(sent)) if id == session && sent == colors
        )));
    });
}

#[gpui::test]
fn quick_terminal_takes_session_titles_and_marks_completions_only_while_hidden(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        let quick = model.state.ensure_quick_terminal();
        let session = SessionId::new(703).expect("session");
        model
            .state
            .set_pane_session(quick, Some(session))
            .expect("session");

        let metadata = muxy_protocol::SessionMetadata {
            title: "cargo build".into(),
            directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
            process: None,
        };
        model.receive_session_metadata(ServerId::local(), session, &metadata, cx);
        assert_eq!(
            model.state.pane(quick).map(|pane| pane.title.as_str()),
            Some("cargo build")
        );

        let finished = |completed| muxy_protocol::SessionProgress {
            progress: None,
            completed,
        };
        model.quick.visible = true;
        model.receive_progress(ServerId::local(), session, finished(1), cx);
        assert!(!model.completions.contains(&quick));
        model.quick.visible = false;
        model.receive_progress(ServerId::local(), session, finished(2), cx);
        assert!(model.completions.contains(&quick));
    });
}
