use super::*;

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let bounds = cx.debug_bounds(selector).expect("control");
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn server_popover_is_always_available_beside_updates_and_dismisses(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        cx.notify();
    });
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    click(cx, "project-connection-status");
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Server)));
        assert!(model.settings_window.is_none());
    });
    assert!(cx.debug_bounds("beta-update-status").is_none());
    for (width, height) in [(1000.0, 700.0), (600.0, 400.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
        let panel = cx.debug_bounds("server-popover").expect("popover");
        let status = cx
            .debug_bounds("project-connection-status")
            .expect("status");
        assert!(panel.bottom() <= status.top());
        assert!(panel.left() >= px(0.0) && panel.right() <= px(width));
        assert!(panel.top() >= px(0.0));
        assert!(cx.debug_bounds("restart-project-server").is_some());
        assert!(cx.debug_bounds("stop-project-server").is_some());
    }
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    view.update(cx, |model, cx| {
        model.updates.ready = Some(crate::updater::PreparedUpdate::fixture().expect("update"));
        cx.notify();
    });
    cx.run_until_parked();
    let update = cx.debug_bounds("beta-update-status").expect("update");
    let server = cx
        .debug_bounds("project-connection-status")
        .expect("server");
    assert!(update.right() < server.left());
    click(cx, "beta-update-status");
    assert!(cx.debug_bounds("update-popover").is_some());
    cx.simulate_keystrokes("escape");
    click(cx, "project-connection-status");
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Server)));
    });
    cx.simulate_click(gpui::point(px(500.0), px(50.0)), Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    click(cx, "project-connection-status");
    view.update(cx, |model, cx| {
        model.appearance.status_bar_visible = false;
        cx.notify();
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
}

#[gpui::test]
fn server_popover_confirms_stop_and_restart_and_only_reconnects_for_restart(
    cx: &mut TestAppContext,
) {
    for restart in [false, true] {
        let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            cx.notify();
        });
        let action = if restart {
            "restart-project-server"
        } else {
            "stop-project-server"
        };
        click(cx, "project-connection-status");
        click(cx, action);
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::StopServer { .. }))
        );
        click(cx, "project-connection-status");
        click(cx, action);
        cx.simulate_prompt_answer(if restart { "Restart" } else { "Stop" });
        cx.run_until_parked();
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::StopServer { restart: actual, .. } if actual == restart)));
        view.update(cx, |model, cx| {
            assert!(model.settings_window.is_none());
            assert_eq!(
                model.server_status(),
                if restart {
                    ServerStatus::Restarting
                } else {
                    ServerStatus::Stopping
                }
            );
            assert!(!model.server_control_enabled());
            model.receive(
                (
                    1,
                    Update::ServerStopped {
                        restart,
                        result: Ok(()),
                    },
                ),
                cx,
            );
            assert_eq!(
                model.connection,
                if restart {
                    ConnectionState::Connecting
                } else {
                    ConnectionState::Disconnected
                }
            );
        });
        assert_eq!(
            requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Connect)),
            restart
        );
    }
}

#[gpui::test]
fn disconnected_server_popover_connects_once_and_tracks_connection_changes(
    cx: &mut TestAppContext,
) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, AppModel::disconnect);
    click(cx, "project-connection-status");
    assert!(cx.debug_bounds("restart-project-server").is_none());
    click(cx, "connect-server");
    view.read_with(cx, |model, _| {
        assert_eq!(model.server_status(), ServerStatus::Connecting);
        assert!(!model.server_connect_enabled());
    });
    click(cx, "connect-server");
    assert_eq!(
        requests
            .try_iter()
            .filter(|(_, work)| matches!(work, Work::Connect))
            .count(),
        1
    );
    view.update(cx, |model, cx| {
        model.receive(
            (
                model.generation,
                Update::ConnectFailed("unavailable".into()),
            ),
            cx,
        );
        assert_eq!(model.server_status(), ServerStatus::Disconnected);
        assert!(model.server_connect_enabled());
        model.connection = ConnectionState::Ready;
        cx.notify();
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.server_status(), ServerStatus::Connected);
    });
    assert!(cx.debug_bounds("restart-project-server").is_some());
}

#[gpui::test]
fn server_controls_respect_busy_state_and_surface_stop_errors_without_settings(
    cx: &mut TestAppContext,
) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.server_preferences.busy = true;
        cx.notify();
    });
    click(cx, "project-connection-status");
    click(cx, "stop-project-server");
    assert!(!cx.has_pending_prompt());
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::StopServer { .. }))
    );
    view.update(cx, |model, cx| {
        model.server_preferences.busy = false;
        model.server_preferences.control_busy = true;
        model.receive(
            (
                1,
                Update::ServerStopped {
                    restart: false,
                    result: Err(io::Error::other("stop failed").into()),
                },
            ),
            cx,
        );
        assert_eq!(model.server_status(), ServerStatus::Connected);
        assert!(model.server_control_enabled());
        assert!(
            model
                .error
                .as_ref()
                .is_some_and(|error| error.contains("stop failed"))
        );
        assert!(model.settings_window.is_none());
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Connect))
    );
}
