use super::*;
use gpui::{MouseButton, point};
use muxy_protocol::ChannelId;

mod close_behavior;

fn terminal_state() -> (AppState, PaneId) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    state.prepare_creation(pane, std::path::Path::new("/tmp"));
    state
        .set_pane_session(pane, SessionId::new(71))
        .expect("session");
    (state, pane)
}

fn attach_pane(model: &mut AppModel, pane: PaneId, channel: u32, cx: &mut Context<AppModel>) {
    let mut attached = attachment();
    attached.channel = ChannelId(channel);
    attached.process = Some(muxy_protocol::ForegroundProcess {
        name: "sleep".into(),
        is_shell: false,
    });
    model.receive_attached(
        ServerId::local(),
        pane,
        model.pane_session(pane).expect("session"),
        attached,
        false,
        cx,
    );
}

fn non_destructive(work: &Work) -> bool {
    !matches!(
        work,
        Work::Discard(..) | Work::CancelCreation(_) | Work::CheckClose { .. } | Work::EndAll(_)
    )
}

#[gpui::test]
fn terminal_context_menu_detaches_clicked_split_and_last_pane_without_closing_sessions(
    cx: &mut TestAppContext,
) {
    let (mut state, first) = terminal_state();
    let second = state.split_pane(first, Direction::Right).expect("split");
    state
        .set_pane_session(second, SessionId::new(72))
        .expect("session");
    let (boot, requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        attach_pane(model, first, 1, cx);
        attach_pane(model, second, 2, cx);
    });
    cx.run_until_parked();
    let position = view.read_with(cx, |model, cx| {
        let pane = model.terminal(&first).expect("terminal").view.read(cx);
        let (bounds, cell) = pane.geometry.expect("geometry");
        bounds.origin + point(cell.width, cell.height / 2.0)
    });
    requests.try_iter().for_each(drop);
    cx.simulate_mouse_move(position, None, Modifiers::default());
    cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    let detach = cx
        .debug_bounds("menu-label-Detach Terminal")
        .expect("Detach Terminal");
    cx.simulate_click(detach.center(), Modifiers::default());
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(model.active_pane(), Some(second));
        assert_eq!(model.state.home().tabs[0].panes.len(), 1);
        assert!(
            model
                .state
                .pending_cancellations(ServerId::local())
                .is_empty()
        );
        assert!(model.state.pending_discards(ServerId::local()).is_empty());
        assert!(model.close_prompt.is_none());
        model.detach_terminal(second, cx);
        assert!(model.state.home().tabs.is_empty());
        assert!(
            store::load(&model.path)
                .expect("persisted layout")
                .home()
                .tabs
                .is_empty()
        );
    });
    let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
    assert!(work.iter().all(non_destructive));
    assert!(
        work.iter()
            .any(|work| matches!(work, Work::Detach(ChannelId(1))))
    );
    assert!(
        work.iter()
            .any(|work| matches!(work, Work::Detach(ChannelId(2))))
    );
    assert!(
        work.iter()
            .any(|work| matches!(work, Work::References(references) if references.is_empty()))
    );
}

#[gpui::test]
fn late_attachment_replies_after_detach_never_discard_the_existing_session(
    cx: &mut TestAppContext,
) {
    for succeeds in [false, true] {
        let (state, pane) = terminal_state();
        let (boot, requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            let session = model.pane_session(pane).expect("session");
            model.pending.insert(pane, ServerId::local());
            requests.try_iter().for_each(drop);
            model.detach_terminal(pane, cx);
            if succeeds {
                model.receive_attached(ServerId::local(), pane, session, attachment(), false, cx);
            } else {
                model.receive_attach_failed(
                    ServerId::local(),
                    pane,
                    Some(session),
                    false,
                    &muxy_client::ClientError::Timeout,
                    cx,
                );
            }
            assert!(model.state.home().tabs.is_empty());
            assert!(model.state.pending_discards(ServerId::local()).is_empty());
            assert!(
                model
                    .state
                    .pending_cancellations(ServerId::local())
                    .is_empty()
            );
            let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
            assert!(work.iter().all(non_destructive));
            if succeeds {
                assert!(work.iter().any(|work| matches!(work, Work::Detach(_))));
            }
        });
    }
}

pub(super) fn verify_live_detach(
    cx: &mut VisualTestContext,
    view: &Entity<AppModel>,
    probe: &Client,
) -> Result {
    verify_live_detach_mode(cx, view, probe, false)?;
    verify_live_detach_mode(cx, view, probe, true)
}

fn verify_live_detach_mode(
    cx: &mut VisualTestContext,
    view: &Entity<AppModel>,
    probe: &Client,
    close_tab: bool,
) -> Result {
    view.update(cx, AppModel::new_tab);
    wait_live(cx, view)?;
    shell(cx, "printf '\\nDETACH_READY\\n'");
    wait_text(cx, view, "DETACH_READY")?;
    let session = active_session(view, cx)?;
    let project = view.read_with(cx, |model, _| model.state.current_project().id);
    let observer = probe.identify(muxy_protocol::ClientKind::Tui)?;
    let attached = probe.attach(session, Size { cols: 80, rows: 24 })?;
    shell(cx, "sleep 30");
    wait(cx, view, |model, cx| {
        model
            .active_pane()
            .and_then(|pane| model.terminal(&pane))
            .and_then(|pane| pane.view.read(cx).process.as_ref())
            .is_some_and(|process| process.name == "sleep")
    })?;
    view.update(cx, |model, cx| {
        if close_tab {
            model.settings.window.close_behavior = muxy_app_core::settings::CloseBehavior::Detach;
            model.close_tab(model.active_tab().expect("live tab"), cx);
        } else {
            model.detach_terminal(model.active_pane().expect("live terminal"), cx);
        }
    });
    wait(cx, view, |model, _| {
        !model
            .state
            .session_references(ServerId::local())
            .contains(&session)
            && model.existing_terminal_count() > 0
            && probe
                .project_sessions(project, None, None)
                .is_ok_and(|page| {
                    page.sessions
                        .iter()
                        .any(|entry| entry.info.id == session && entry.owner == Some(observer))
                })
    })?;
    probe.detach(attached.channel)?;
    wait(cx, view, |_, _| {
        probe
            .project_sessions(project, None, None)
            .is_ok_and(|page| {
                page.sessions
                    .iter()
                    .any(|entry| entry.info.id == session && entry.owner.is_none())
            })
    })?;
    let available = probe
        .available_project_sessions(project)?
        .sessions
        .into_iter()
        .find(|entry| entry.info.id == session)
        .ok_or("detached terminal missing")?;
    view.update(cx, |model, cx| {
        model.open_existing_session(project, &available, cx);
    });
    wait_live(cx, view)?;
    assert_eq!(active_session(view, cx)?, session);
    let running = probe.attach(session, Size { cols: 80, rows: 24 })?;
    assert!(
        running
            .process
            .is_some_and(|process| process.name == "sleep")
    );
    probe.send_input(running.channel, b"\x03")?;
    probe.detach(running.channel)?;
    shell(cx, "printf '\\nDETACH_REATTACHED\\n'");
    wait_text(cx, view, "DETACH_REATTACHED")?;
    view.update(cx, |model, cx| {
        model.settings.window.close_behavior = muxy_app_core::settings::CloseBehavior::CloseSession;
        model.close_tab(model.active_tab().expect("reattached tab"), cx);
    });
    wait(cx, view, |model, _| {
        model.state.pending_discards(ServerId::local()).is_empty()
            && probe
                .list_sessions()
                .is_ok_and(|sessions| !sessions.iter().any(|info| info.id == session))
    })?;
    report(if close_tab {
        "Configured tab close detaches, transfers ownership, and reopens the running session: PASS"
    } else {
        "Desktop detach preserves a running program, transfers ownership, remains discoverable, and reattaches to the same session: PASS"
    })
}
