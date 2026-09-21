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
        model.connection = ConnectionState::Ready;
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
        assert!(model.state.pending_cancellations().is_empty());
        assert!(model.state.pending_discards().is_empty());
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
fn detach_save_failure_keeps_the_pane_and_sends_no_detach(cx: &mut TestAppContext) {
    let (state, pane) = terminal_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        attach_pane(model, pane, 1, cx);
        let previous = model.state.clone();
        let original_path = model.path.clone();
        let blocked = original_path.with_file_name("blocked");
        std::fs::write(&blocked, "file").expect("blocked directory");
        model.path = blocked.join("state.json");
        requests.try_iter().for_each(drop);
        model.detach_terminal(pane, cx);
        assert_eq!(model.state, previous);
        assert!(model.terminal(&pane).is_some());
        assert!(
            model
                .error
                .as_ref()
                .is_some_and(|error| error.contains("Could not save tabs"))
        );
        assert!(requests.try_iter().all(|(_, work)| !matches!(
            work,
            Work::Detach(_) | Work::References(_)
        ) && non_destructive(&work)));
        model.path = original_path;
    });
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
            model.connection = ConnectionState::Ready;
            let session = model.pane_session(pane).expect("session");
            model.pending.insert(pane);
            requests.try_iter().for_each(drop);
            model.detach_terminal(pane, cx);
            if succeeds {
                model.receive_attached(pane, session, attachment(), false, cx);
            } else {
                model.receive_attach_failed(
                    pane,
                    Some(session),
                    false,
                    &muxy_client::ClientError::Timeout,
                    cx,
                );
            }
            assert!(model.state.home().tabs.is_empty());
            assert!(model.state.pending_discards().is_empty());
            assert!(model.state.pending_cancellations().is_empty());
            let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
            assert!(work.iter().all(non_destructive));
            if succeeds {
                assert!(work.iter().any(|work| matches!(work, Work::Detach(_))));
            }
        });
    }
}

#[gpui::test]
fn detach_shortcut_is_unassigned_and_can_be_configured(cx: &mut TestAppContext) {
    let (state, pane) = terminal_state();
    let (boot, requests) = stub_boot(state);
    assert!(
        boot.settings
            .keymap
            .chord(muxy_core::shortcuts::ShortcutId::DetachTerminal)
            .is_none()
    );
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-shift-e");
    view.update(cx, |model, cx| {
        assert_eq!(model.active_pane(), Some(pane));
        model.change_preference(
            crate::views::settings::Change::Binding(
                "detach_terminal".into(),
                Some("cmd-shift-e".parse().expect("shortcut")),
            ),
            cx,
        );
    });
    cx.simulate_keystrokes("cmd-shift-e");
    view.read_with(cx, |model, _| {
        assert!(model.state.home().tabs.is_empty());
        assert!(model.state.pending_cancellations().is_empty());
        assert!(model.state.pending_discards().is_empty());
    });
    assert!(requests.try_iter().all(|(_, work)| non_destructive(&work)));
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
        !model.state.session_references().contains(&session)
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
        model.state.pending_discards().is_empty()
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
