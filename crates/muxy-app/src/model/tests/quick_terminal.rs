use super::*;

#[gpui::test]
fn native_terminal_material_survives_pane_recreation_without_entering_quick_terminal(
    cx: &mut TestAppContext,
) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.terminal.options.background_vibrancy = 70;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.new_tab(cx);
        let first = model.active_pane().expect("main pane");
        model.set_terminal_vibrancy_available(true, cx);
        assert!(model.grids[&first].view.read(cx).native_vibrancy);

        let quick = model.state.ensure_quick_terminal();
        model.quick.visible = true;
        model.sync_visible(cx);
        assert!(!model.grids[&quick].view.read(cx).native_vibrancy);

        model.new_tab(cx);
        let next = model.active_pane().expect("new main pane");
        assert_ne!(next, first);
        assert!(model.grids[&next].view.read(cx).native_vibrancy);
        assert!(!model.grids[&quick].view.read(cx).native_vibrancy);

        model.set_terminal_vibrancy_available(false, cx);
        assert!(!model.grids[&next].view.read(cx).native_vibrancy);
        model.set_terminal_vibrancy_available(true, cx);
        assert!(model.grids[&next].view.read(cx).native_vibrancy);
        assert!(!model.grids[&quick].view.read(cx).native_vibrancy);
    });
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
