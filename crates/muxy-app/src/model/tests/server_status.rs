use super::*;

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let bounds = cx.debug_bounds(selector).expect("control");
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn server_popover_confirms_stop_and_restart_and_only_reconnects_for_restart(
    cx: &mut TestAppContext,
) {
    for restart in [false, true] {
        let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
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
                    ServerId::local(),
                    1,
                    Update::ServerStopped {
                        restart,
                        result: Ok(()),
                    },
                ),
                cx,
            );
            assert_eq!(
                model.servers.local.connection,
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
