use super::*;

#[gpui::test]
#[ignore = "requires a built server and a fresh MUXY_DIR under /tmp/muxy-catalog-"]
fn quick_terminal_catalog_walkthrough(cx: &mut TestAppContext) {
    quick_catalog_walkthrough(cx).expect("Quick Terminal catalog walkthrough");
}

fn quick_catalog_walkthrough(cx: &mut TestAppContext) -> Result {
    let directory = PathBuf::from(std::env::var("MUXY_DIR")?);
    assert!(
        directory
            .to_string_lossy()
            .starts_with("/tmp/muxy-catalog-")
    );
    let (view, cx) =
        cx.add_window_view(|window, cx| AppModel::new(Boot::load().expect("boot"), window, cx));
    wait(cx, &view, |model, _| {
        model.servers.local.connection == ConnectionState::Ready
            && model.servers.local.catalog.restore.is_none()
    })?;
    let before = view.read_with(cx, |model, _| model.state.window().clone());
    let quick = view.update(cx, |model, cx| {
        let pane = model.state.ensure_quick_terminal();
        model.quick.visible = true;
        model.sync_visible(cx);
        model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
        pane
    });
    wait(cx, &view, |model, _| model.pane_session(quick).is_some())?;
    let session = view
        .read_with(cx, |model, _| model.pane_session(quick))
        .ok_or("session")?;
    let probe = Client::connect(&directory.join("server.sock"))?;
    let catalog = probe.catalog()?;
    let membership = probe.project_sessions(catalog.home, None, None)?;
    assert_eq!(membership.sessions[0].info.id, session);
    assert_eq!(membership.sessions[0].info.project, catalog.home);
    view.update(cx, |model, cx| {
        model.hide_quick_terminal(false, cx);
        assert_eq!(model.state.window(), &before);
        model.quick.visible = true;
        model.sync_visible(cx);
        model.start_attach(quick, Size { cols: 80, rows: 24 }, cx);
    });
    wait(cx, &view, |model, _| !model.pending.contains_key(&quick))?;
    assert_eq!(probe.list_sessions()?.len(), 1);
    view.update(cx, AppModel::close_quick_terminal);
    wait(cx, &view, |model, _| {
        model.state.pending_discards(ServerId::local()).is_empty()
            && model
                .state
                .pending_cancellations(ServerId::local())
                .is_empty()
    })?;
    assert!(probe.list_sessions()?.is_empty());
    report(
        "Quick Terminal belongs to server Home; hide/reattach preserves identity; close cleans session: PASS",
    )
}

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
        assert_eq!(model.pane_session(quick), Some(session));
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

#[gpui::test]
fn quick_terminal_exit_discards_only_its_session_and_next_show_gets_new_identity(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        model.new_tab(cx);
        let window = model.state.window().clone();
        let pane = model.state.ensure_quick_terminal();
        let session = SessionId::new(703).expect("session");
        model
            .state
            .set_pane_session(pane, Some(session))
            .expect("session");
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Event(ClientEvent::SessionEnded {
                    session,
                    reason: ExitReason::Exited(0),
                }),
            ),
            cx,
        );
        assert!(model.state.quick_terminal().is_none());
        assert_eq!(model.state.window(), &window);
        assert_eq!(model.state.home().tabs.len(), 1);
        assert_ne!(model.state.ensure_quick_terminal(), pane);
    });
}

#[gpui::test]
fn quick_terminal_focus_and_visibility_survive_workspace_overlays(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let quick = view.update(cx, |model, cx| {
        let quick = model.state.ensure_quick_terminal();
        model.quick.visible = true;
        model.sync_visible(cx);
        let pane = model.terminal(&quick).expect("quick terminal").view.clone();
        pane.update(cx, |pane, cx| pane.set_focused(true, cx));
        pane
    });
    cx.update(|window, cx| view.update(cx, |model, cx| model.open_theme_picker(window, cx)));
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_some()));
    quick.read_with(cx, |pane, _| {
        assert!(pane.focused);
        assert!(pane.native_visible);
    });
}

#[gpui::test]
fn quick_terminal_close_prompts_only_for_commands(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let pane = model.state.ensure_quick_terminal();
        model.quick.closing = Some((TabId::new(), pane));
        model.check_quick_close(
            Ok(Some(muxy_protocol::ForegroundProcess {
                name: "sleep".into(),
                is_shell: false,
            })),
            cx,
        );
        assert!(model.state.quick_terminal().is_some());
        assert!(model.quick.closing.is_some());
        model.check_quick_close(
            Ok(Some(muxy_protocol::ForegroundProcess {
                name: "zsh".into(),
                is_shell: true,
            })),
            cx,
        );
        assert!(model.state.quick_terminal().is_none());
        assert!(model.quick.closing.is_none());
    });
}
