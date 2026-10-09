use super::*;
use muxy_app_core::Direction;
use muxy_protocol::ChannelId;

fn split_state() -> (AppState, TabId, [PaneId; 3]) {
    let mut state = AppState::bootstrap().expect("state");
    let tab = state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    let third = state.split_pane(second, Direction::Down).expect("split");
    for (index, pane) in [first, second, third].into_iter().enumerate() {
        state
            .set_pane_session(pane, SessionId::new(100 + index as u64))
            .expect("session");
    }
    (state, tab, [first, second, third])
}

fn attach_panes(model: &mut AppModel, panes: &[PaneId], cx: &mut Context<AppModel>) {
    model.servers.local.connection = ConnectionState::Ready;
    for (index, pane) in panes.iter().enumerate() {
        let mut attachment = attachment();
        attachment.channel = ChannelId(u32::try_from(index + 1).expect("channel"));
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    pane: *pane,
                    session: model.pane_session(*pane).expect("session"),
                    attachment,
                    created: false,
                },
            ),
            cx,
        );
    }
}

#[gpui::test]
fn zoom_and_tab_switch_detach_only_hidden_leaves_and_restore_all(cx: &mut TestAppContext) {
    let (state, first_tab, panes) = split_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(model.grids.len(), 3);
        attach_panes(model, &panes, cx);
        let _ = requests.try_iter().collect::<Vec<_>>();
        model.toggle_zoom_pane(cx);
        assert_eq!(model.visible_panes(), [panes[2]]);
        assert_eq!(model.grids.len(), 1);
        let detached: Vec<_> = requests
            .try_iter()
            .filter_map(|(_, work)| match work {
                Work::Detach(channel) => Some(channel),
                _ => None,
            })
            .collect();
        assert_eq!(detached.len(), 2);
        assert!(detached.contains(&ChannelId(1)) && detached.contains(&ChannelId(2)));
        model.toggle_zoom_pane(cx);
        assert_eq!(model.grids.len(), 3);
        assert_eq!(
            model
                .terminal(&panes[2])
                .expect("terminal")
                .view
                .read(cx)
                .channel(),
            Some(ChannelId(3))
        );
        for pane in &panes {
            model
                .terminal(pane)
                .expect("terminal")
                .view
                .update(cx, |pane, cx| {
                    pane.set_viewport(Size { cols: 30, rows: 12 }, cx);
                });
        }
        model.ensure_visible(cx);
        let attached: Vec<_> = requests
            .try_iter()
            .filter_map(|(_, work)| match work {
                Work::Attach { pane, session, .. } => Some((pane, session)),
                _ => None,
            })
            .collect();
        assert_eq!(attached.len(), 2);
        assert!(attached.contains(&(panes[0], model.pane_session(panes[0]))));
        assert!(attached.contains(&(panes[1], model.pane_session(panes[1]))));
        attach_panes(model, &panes, cx);
        let _ = requests.try_iter().collect::<Vec<_>>();
        model.new_tab(cx);
        assert_eq!(model.grids.len(), 1);
        assert_eq!(
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::Detach(_)))
                .count(),
            3
        );
        model.select_tab(first_tab, cx);
        assert_eq!(model.grids.len(), 3);
        assert_eq!(model.active_pane(), Some(panes[2]));
    });
}

#[gpui::test]
fn closing_one_pane_discards_only_its_session_and_last_closes_tab(cx: &mut TestAppContext) {
    let (state, _, panes) = split_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        attach_panes(model, &panes, cx);
        let _ = requests.try_iter().collect::<Vec<_>>();
        model.close_pane(panes[1], cx);
        acknowledge_close(model, cx);
        assert_eq!(model.state.home().tabs[0].panes.len(), 2);
        assert_eq!(model.active_pane(), Some(panes[2]));
        assert_eq!(
            model.state.pending_discards(ServerId::local()),
            [SessionId::new(101).expect("session")]
        );
        let discarded: Vec<_> = requests
            .try_iter()
            .filter_map(|(_, work)| match work {
                Work::Discard(session, _) => Some(session),
                _ => None,
            })
            .collect();
        assert_eq!(discarded, model.state.pending_discards(ServerId::local()));
        let loaded = store::load(&model.path).expect("saved");
        assert_eq!(loaded.home().tabs[0].layout.leaves(), [panes[0], panes[2]]);
        model.close_pane(panes[2], cx);
        acknowledge_close(model, cx);
        assert_eq!(model.active_pane(), Some(panes[0]));
        model.close_pane(panes[0], cx);
        acknowledge_close(model, cx);
        assert!(model.state.home().tabs.is_empty());
        assert!(model.grids.is_empty());
    });
}

#[gpui::test]
fn closing_tab_checks_every_hidden_pane_and_cancel_keeps_all(cx: &mut TestAppContext) {
    let (state, tab, panes) = split_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        attach_panes(model, &panes, cx);
        model.toggle_zoom_pane(cx);
        model.close_tab(tab, cx);
        assert_eq!(model.pending_close, Some(tab));
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::CheckClose { session, .. } if Some(session) == SessionId::new(100))));
        model.receive_close_checked(tab, SessionId::new(100).expect("session"), Ok(None), cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::CheckClose { session, .. } if Some(session) == SessionId::new(101))));
        model.receive_close_checked(tab, SessionId::new(101).expect("session"), Ok(Some(muxy_protocol::ForegroundProcess { name: "vim".into(), is_shell: false })), cx);
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs[0].panes.len(), 3);
        assert!(model.state.pending_discards(ServerId::local()).is_empty());
    });
}

#[gpui::test]
fn split_directory_inherits_only_when_configured_and_falls_back_to_project(
    cx: &mut TestAppContext,
) {
    for (setting, reported, inherit) in [
        (
            muxy_app_core::settings::NewPaneDirectory::Project,
            b"/tmp".as_slice(),
            false,
        ),
        (
            muxy_app_core::settings::NewPaneDirectory::Current,
            b"/tmp".as_slice(),
            true,
        ),
        (
            muxy_app_core::settings::NewPaneDirectory::Current,
            b"".as_slice(),
            false,
        ),
        (
            muxy_app_core::settings::NewPaneDirectory::Current,
            b"relative".as_slice(),
            false,
        ),
    ] {
        let mut state = AppState::bootstrap().expect("state");
        state.open_terminal_tab(state.home().id).expect("tab");
        let original = state.home().tabs[0].panes[0].id;
        state
            .set_pane_session(original, SessionId::new(100))
            .expect("session");
        let (mut boot, requests) = stub_boot(state);
        boot.settings.panes.new_pane_directory = setting;
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            let mut data = attachment();
            data.directory = muxy_protocol::ServerPath(reported.to_vec());
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane: original,
                        session: SessionId::new(100).expect("session"),
                        attachment: data,
                        created: false,
                    },
                ),
                cx,
            );
            let _ = requests.try_iter().collect::<Vec<_>>();
            model.split_pane(Direction::Right, cx);
            let new = model.active_pane().expect("split");
            let directory = requests
                .try_iter()
                .find_map(|(_, work)| match work {
                    Work::Attach {
                        pane,
                        directory,
                        session: None,
                        ..
                    } if pane == new => Some(directory),
                    _ => None,
                })
                .expect("creation request");
            assert_eq!(
                directory,
                if inherit {
                    PathBuf::from("/tmp")
                } else {
                    model.state.current_project().directory.clone()
                }
            );
            model.close_pane(new, cx);
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane: new,
                        session: SessionId::new(101).expect("session"),
                        attachment: attachment(),
                        created: true,
                    },
                ),
                cx,
            );
            assert!(
                model
                    .state
                    .pending_discards(ServerId::local())
                    .contains(&SessionId::new(101).expect("session"))
            );
            assert!(model.initial_directories.is_empty());
        });
    }
}
