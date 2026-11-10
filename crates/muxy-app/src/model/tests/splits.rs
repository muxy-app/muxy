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
        let attachment = attachment_on(u32::try_from(index + 1).expect("channel"));
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    pane: *pane,
                    session: model.state.pane_session(*pane).expect("session").1,
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
        for pane in &panes[..2] {
            let session = model.state.pane_session(*pane).map(|(_, session)| session);
            assert!(attached.contains(&(*pane, session)));
        }
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
fn equalize_splits_evens_resized_panes_from_the_shortcut_and_the_terminal_menu(
    cx: &mut TestAppContext,
) {
    let (state, tab, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    let resize = |model: &mut AppModel, cx: &mut Context<AppModel>| {
        model.state.set_ratio(tab, &[], 0.8).expect("ratio");
        model
            .state
            .set_ratio(tab, &[muxy_app_core::Branch::Second], 0.2)
            .expect("ratio");
        model.save_split_resize(cx);
    };
    let assert_even_and_saved = |model: &AppModel| {
        assert_eq!(
            model.state.home().tabs[0].layout.rects(),
            [
                (panes[0], [0.0, 0.0, 0.5, 1.0]),
                (panes[1], [0.5, 0.0, 0.5, 0.5]),
                (panes[2], [0.5, 0.5, 0.5, 0.5]),
            ]
        );
        assert_eq!(model.active_pane(), Some(panes[2]));
        assert_eq!(store::load(&model.path).expect("saved"), model.state);
    };
    view.update(cx, |model, cx| resize(model, cx));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-ctrl-="
    } else {
        "ctrl-alt-="
    });
    view.read_with(cx, |model, _| assert_even_and_saved(model));

    view.update(cx, |model, cx| {
        resize(model, cx);
        model.terminal_menu(panes[0], gpui::point(px(100.0), px(100.0)), cx);
    });
    cx.run_until_parked();
    let equalize = cx.debug_bounds("menu-item-7").expect("Equalize Splits");
    cx.simulate_click(equalize.center(), Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert_even_and_saved(model));
}

fn scroll_first_pane(
    model: &mut AppModel,
    panes: &[PaneId],
    rows: f32,
    cx: &mut Context<AppModel>,
) {
    let scrolled = model.terminal(&panes[0]).expect("terminal").view.clone();
    scrolled.update(cx, |pane, cx| {
        pane.set_viewport(attachment().terminal.grid.size, cx);
    });
    attach_panes(model, panes, cx);
    scrolled.update(cx, |pane, cx| {
        let grid = pane.grid.as_mut().expect("grid");
        grid.history = (0..10)
            .map(|index| muxy_protocol::Row {
                index,
                runs: vec![muxy_protocol::Run {
                    text: index.to_string(),
                    width: 1,
                    style: muxy_protocol::Style::default(),
                }],
            })
            .collect();
        grid.history_total = 10;
        pane.scroll_rows(rows, cx);
    });
}

#[gpui::test]
fn a_hidden_tab_comes_back_scrolled_where_it_was_left(cx: &mut TestAppContext) {
    let (state, first_tab, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let top_row = |model: &AppModel, pane: PaneId, cx: &gpui::App| {
            let pane = model.terminal(&pane).expect("terminal").view.read(cx);
            let grid = pane.displayed_grid().expect("grid");
            grid.content_row(pane.visible_start(grid))
                .expect("row")
                .to_vec()
        };
        scroll_first_pane(model, &panes, 4.0, cx);
        let left = top_row(model, panes[0], cx);
        assert_eq!(left[0].text, "6");
        model.new_tab(cx);
        assert_eq!(model.grids.len(), 1);
        model.select_tab(first_tab, cx);
        assert_eq!(top_row(model, panes[0], cx), left);
        attach_panes(model, &panes, cx);
        assert_eq!(top_row(model, panes[0], cx), left);
        assert_eq!(top_row(model, panes[1], cx)[0].text, "final marker");
        assert!(!model.has_saved_scroll());
    });
}

#[gpui::test]
fn a_tab_hidden_just_above_the_bottom_comes_back_at_the_bottom(cx: &mut TestAppContext) {
    let (state, first_tab, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        scroll_first_pane(model, &panes, 0.25, cx);
        let pane = model.terminal(&panes[0]).expect("terminal").view.read(cx);
        assert!(pane.scroll.view.is_some());
        model.new_tab(cx);
        assert!(!model.has_saved_scroll());
        model.select_tab(first_tab, cx);
        attach_panes(model, &panes, cx);
        let pane = model.terminal(&panes[0]).expect("terminal").view.read(cx);
        assert!(pane.scroll.view.is_none());
    });
}

#[gpui::test]
fn scroll_on_output_notices_output_attached_while_the_tab_was_hidden_again(
    cx: &mut TestAppContext,
) {
    let (state, first_tab, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        scroll_first_pane(model, &panes, 4.0, cx);
        model.terminal.options.scroll_on_output = true;
        model.new_tab(cx);
        let (_, session) = model.state.pane_session(panes[0]).expect("session");
        let mut output = attachment();
        output.terminal.grid.rows[1] = output.terminal.grid.rows[0].clone();
        let attached = || Update::Attached {
            pane: panes[0],
            session,
            attachment: output.clone(),
            created: false,
        };
        model.receive((ServerId::local(), 1, attached()), cx);
        model.select_tab(first_tab, cx);
        model.receive((ServerId::local(), 1, attached()), cx);
        let pane = model.terminal(&panes[0]).expect("terminal").view.read(cx);
        assert!(pane.channel().is_some());
        assert!(pane.scroll.view.is_none());
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
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::CheckClose { session, .. } if Some(session) == SessionId::new(100))));
        model.receive_close_checked(model.pending_close.expect("close check"), SessionId::new(100).expect("session"), Ok(None), cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::CheckClose { session, .. } if Some(session) == SessionId::new(101))));
        model.receive_close_checked(model.pending_close.expect("close check"), SessionId::new(101).expect("session"), Ok(Some(muxy_protocol::ForegroundProcess { name: "vim".into(), is_shell: false })), cx);
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
            assert_eq!(model.state.starting_proxy(new), None);
            let mut after_close = model.state.clone();
            let folder = after_close.prepare_creation(new, std::path::Path::new("/closed"));
            assert_eq!(folder.0, b"/closed");
        });
    }
}

#[gpui::test]
fn a_panes_record_ends_with_the_pane_or_when_it_has_nothing_to_keep(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let first_tab = state.open_terminal_tab(state.home().id).expect("tab");
    let hidden = state.home().tabs[0].panes[0].id;
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.set_pane_completed(hidden, true);
        model.set_pane_zoom(hidden, Some(20.0));
        assert_eq!(model.pane_runtime.len(), 1);
        model.set_pane_completed(hidden, false);
        model.set_pane_zoom(hidden, None);
        assert!(model.pane_runtime.is_empty());

        model.set_pane_zoom(hidden, Some(20.0));
        let home = model.state.home().id;
        model.state.close_tab(home, first_tab).expect("close");
        model.sync_visible(cx);
        assert!(model.pane_runtime.is_empty());
    });
}

fn click_selector(cx: &mut VisualTestContext, selector: &str) {
    cx.run_until_parked();
    let point = cx
        .debug_bounds(selector.to_owned().leak())
        .unwrap_or_else(|| panic!("{selector}"))
        .center();
    cx.simulate_click(point, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn every_pane_has_a_header_whose_buttons_act_on_that_pane(cx: &mut TestAppContext) {
    let (state, _, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update(cx, |model, cx| attach_panes(model, &panes, cx));
    cx.run_until_parked();
    for pane in panes {
        for part in [
            "header",
            "split-right",
            "split-down",
            "float",
            "maximize",
            "close",
        ] {
            assert!(
                cx.debug_bounds(format!("pane-{part}-{pane}").leak())
                    .is_some(),
                "{part} of {pane}"
            );
        }
    }
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(panes[2]));
    });

    click_selector(cx, &format!("pane-split-right-{}", panes[0]));
    let added = view.read_with(cx, |model, _| {
        let added = model.active_pane().expect("active pane");
        assert!(!panes.contains(&added));
        assert_eq!(model.state.home().tabs[0].panes.len(), 4);
        assert_eq!(
            model.state.neighbor(panes[0], Direction::Right),
            Some(added)
        );
        added
    });

    click_selector(cx, &format!("pane-maximize-{}", panes[1]));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs[0].zoomed, Some(panes[1]));
        assert_eq!(model.active_pane(), Some(panes[1]));
    });
    click_selector(cx, &format!("pane-restore-{}", panes[1]));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs[0].zoomed, None);
    });

    click_selector(cx, &format!("pane-close-{}", panes[0]));
    view.update(cx, acknowledge_close);
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.home().tabs[0].layout.leaves(),
            [added, panes[1], panes[2]]
        );
    });
}

fn shown(cx: &mut VisualTestContext, part: &str, pane: PaneId) -> bool {
    cx.debug_bounds(format!("pane-{part}-{pane}").leak())
        .is_some()
}

#[gpui::test]
fn a_lone_pane_keeps_its_header_without_the_maximize_button(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let (boot, _requests) = stub_boot(state);
    let (_view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert!(shown(cx, "header", pane));
    assert!(shown(cx, "split-right", pane));
    assert!(shown(cx, "close", pane));
    assert!(!shown(cx, "maximize", pane));
    assert!(!shown(cx, "detach", pane));
    assert!(!shown(cx, "dim", pane));
}

#[gpui::test]
fn closing_a_pane_from_its_header_keeps_focus_on_the_active_pane(cx: &mut TestAppContext) {
    let (state, _, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update(cx, |model, cx| attach_panes(model, &panes, cx));
    cx.run_until_parked();
    for (pane, dimmed) in [(panes[0], true), (panes[1], true), (panes[2], false)] {
        assert_eq!(shown(cx, "dim", pane), dimmed, "{pane}");
    }
    assert!(
        cx.debug_bounds("maximize-pane").is_none(),
        "the title bar leaves maximizing to the pane headers"
    );
    click_selector(cx, &format!("pane-close-{}", panes[0]));
    view.update(cx, acknowledge_close);
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.home().tabs[0].layout.leaves(),
            [panes[1], panes[2]]
        );
        assert_eq!(model.active_pane(), Some(panes[2]));
    });
}

#[gpui::test]
fn a_pinned_tabs_last_pane_offers_no_close(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let tab = state.open_terminal_tab(state.home().id).expect("tab");
    state.toggle_tab_pin(tab).expect("pin");
    let pane = state.home().tabs[0].panes[0].id;
    let (boot, _requests) = stub_boot(state);
    let (_view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert!(shown(cx, "split-right", pane));
    assert!(!shown(cx, "close", pane));
}

#[gpui::test]
fn a_pane_running_an_agent_shows_the_agents_icon_in_its_header(cx: &mut TestAppContext) {
    use muxy_protocol::{ActivitySnapshot, AgentActivity, AgentProvider, AgentState};
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    state.open_terminal_tab(home).expect("tab");
    let agent = state.window().active_pane.expect("pane");
    let shell = state.split_pane(agent, Direction::Right).expect("split");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let local = ServerId::local();
        model.servers.local.connection = ConnectionState::Ready;
        for (index, pane) in [agent, shell].into_iter().enumerate() {
            let session = SessionId::new(index as u64 + 1).expect("session");
            model.receive_attached(local, pane, session, attachment(), true, cx);
        }
        model.servers.local.activity.loaded = true;
        model.receive_activity(
            local,
            Ok(ActivitySnapshot {
                agents: vec![AgentActivity {
                    session: SessionId::new(1).expect("session"),
                    project: home,
                    provider: AgentProvider::OpenCode,
                    state: AgentState::Idle,
                }],
                ..ActivitySnapshot::default()
            }),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(format!("pane-provider-{agent}-OpenCode").leak())
            .is_some()
    );
    assert!(
        cx.debug_bounds(format!("pane-provider-{shell}-OpenCode").leak())
            .is_none()
    );
}

#[gpui::test]
fn a_pane_header_names_the_shell_while_its_tab_keeps_its_title(cx: &mut TestAppContext) {
    use muxy_protocol::MetadataEvent;
    let (state, tab, panes) = split_state();
    let pane = panes[0];
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let report = |event: MetadataEvent, cx: &mut VisualTestContext| {
        view.update(cx, |model, cx| {
            let terminal = model.terminal(&pane).expect("terminal").view.clone();
            terminal.update(cx, |terminal, cx| terminal.metadata(event, cx));
        });
        cx.run_until_parked();
    };
    let tab_title = |cx: &mut VisualTestContext| {
        view.update(cx, |model, cx| {
            let tab = model.tab(tab).expect("tab");
            model.webview_title(tab, cx).to_owned()
        })
    };
    view.update(cx, |model, cx| {
        attach_panes(model, &panes, cx);
        model.focus_pane(pane, cx);
    });
    report(MetadataEvent::Directory("/home/dev/muxy".into()), cx);
    assert!(
        cx.debug_bounds(format!("pane-title-{pane}-sh").leak())
            .is_some()
    );
    let before = tab_title(cx);
    assert_eq!(before, "muxy");

    report(
        MetadataEvent::ForegroundProcess {
            name: "-zsh".into(),
            is_shell: true,
        },
        cx,
    );
    assert!(
        cx.debug_bounds(format!("pane-title-{pane}-zsh").leak())
            .is_some()
    );
    assert_eq!(tab_title(cx), before);
}

#[gpui::test]
fn a_pane_header_shows_its_folder_unless_the_title_already_does(cx: &mut TestAppContext) {
    use muxy_protocol::MetadataEvent;
    let (state, _, panes) = split_state();
    let pane = panes[0];
    let home = state.home().directory.to_string_lossy().into_owned();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let report = |event: MetadataEvent, cx: &mut VisualTestContext| {
        view.update(cx, |model, cx| {
            let terminal = model.terminal(&pane).expect("terminal").view.clone();
            terminal.update(cx, |terminal, cx| terminal.metadata(event, cx));
        });
        cx.run_until_parked();
    };
    let directory = |name: &str, cx: &mut VisualTestContext| {
        cx.debug_bounds(format!("pane-directory-{pane}-{name}").leak())
            .is_some()
    };
    view.update(cx, |model, cx| attach_panes(model, &panes, cx));
    report(
        MetadataEvent::ForegroundProcess {
            name: "-zsh".into(),
            is_shell: true,
        },
        cx,
    );
    report(
        MetadataEvent::Directory(format!("{home}/muxy/crates").into()),
        cx,
    );
    assert!(
        cx.debug_bounds(format!("pane-title-{pane}-zsh").leak())
            .is_some()
    );
    assert!(directory("~/muxy/crates", cx));

    for (title, folder) in [("~/muxy", "muxy"), ("dev@host: ~/api", "api")] {
        report(MetadataEvent::Title(title.into()), cx);
        report(
            MetadataEvent::Directory(format!("{home}/{folder}").into()),
            cx,
        );
        assert!(
            cx.debug_bounds(format!("pane-title-{pane}-{title}").leak())
                .is_some()
        );
        assert!(!directory(&format!("~/{folder}"), cx), "{title}");
    }
}

#[gpui::test]
fn a_narrow_pane_header_cuts_its_folder_before_its_title(cx: &mut TestAppContext) {
    use muxy_protocol::MetadataEvent;
    let (state, _, panes) = split_state();
    let pane = panes[0];
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update(cx, |model, cx| attach_panes(model, &panes, cx));
    let folder = format!("/srv/{}", "deep/".repeat(40));
    let folder = folder.trim_end_matches('/').to_owned();
    for event in [
        MetadataEvent::ForegroundProcess {
            name: "a-long-running-program-name".into(),
            is_shell: false,
        },
        MetadataEvent::Directory(folder.clone().into()),
    ] {
        view.update(cx, |model, cx| {
            let terminal = model.terminal(&pane).expect("terminal").view.clone();
            terminal.update(cx, |terminal, cx| terminal.metadata(event, cx));
        });
    }
    cx.run_until_parked();
    let title = |cx: &mut VisualTestContext| {
        cx.debug_bounds(format!("pane-title-{pane}-a-long-running-program-name").leak())
            .expect("title")
            .size
            .width
    };
    let directory = |cx: &mut VisualTestContext| {
        cx.debug_bounds(format!("pane-directory-{pane}-{folder}").leak())
            .expect("folder")
            .size
            .width
    };
    let wide = title(cx);
    let wide_directory = directory(cx);
    cx.simulate_resize(size(px(700.0), px(800.0)));
    cx.run_until_parked();
    assert_eq!(title(cx), wide);
    assert!(directory(cx) < wide_directory);
}

#[gpui::test]
fn with_pane_headers_off_panes_have_no_header_but_stay_dimmed(cx: &mut TestAppContext) {
    let (state, _, panes) = split_state();
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.pane_headers = false;
    let (_view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();
    for pane in panes {
        assert!(!shown(cx, "header", pane), "{pane}");
        assert!(!shown(cx, "split-right", pane), "{pane}");
    }
    assert!(shown(cx, "dim", panes[0]));
    assert!(!shown(cx, "dim", panes[2]));
    assert!(cx.debug_bounds("maximize-pane").is_some());
}

#[gpui::test]
fn the_pane_headers_setting_is_saved(cx: &mut TestAppContext) {
    let (state, _, _) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert!(model.appearance.pane_headers);
        model.change_preference(crate::model::preferences::Change::PaneHeaders(false), cx);
        assert!(!model.appearance.pane_headers);
        let saved =
            muxy_app_core::settings::Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("settings");
        assert!(!saved.appearance.pane_headers);
    });
}

#[gpui::test]
fn a_narrow_pane_keeps_its_actions_ahead_of_its_icon_and_title(cx: &mut TestAppContext) {
    let (mut state, tab, panes) = split_state();
    state.set_ratio(tab, &[], 0.15).expect("ratio");
    state.focus_pane(panes[0]).expect("focus");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| attach_panes(model, &panes, cx));
    cx.run_until_parked();
    let pane = cx
        .debug_bounds(format!("pane-{}", panes[0]).leak())
        .expect("pane");
    assert!(pane.size.width < px(200.0), "{pane:?}");
    for part in ["split-right", "close"] {
        let button = cx
            .debug_bounds(format!("pane-{part}-{}", panes[0]).leak())
            .expect(part);
        assert!(
            button.left() >= pane.left() && button.right() <= pane.right(),
            "{part} {button:?} outside {pane:?}"
        );
    }
    view.update(cx, |model, cx| {
        model.state.set_ratio(tab, &[], 0.07).expect("ratio");
        cx.notify();
    });
    cx.simulate_resize(size(px(900.0), px(600.0)));
    cx.run_until_parked();
    let narrow = cx
        .debug_bounds(format!("pane-{}", panes[0]).leak())
        .expect("pane");
    let close = cx
        .debug_bounds(format!("pane-close-{}", panes[0]).leak())
        .expect("close");
    assert!(close.right() <= narrow.right(), "{close:?} in {narrow:?}");
    let first = cx
        .debug_bounds(format!("pane-split-right-{}", panes[0]).leak())
        .expect("split right");
    assert!(
        first.left() >= narrow.left() || first.top() >= close.bottom(),
        "a button that doesn't fit moves out of the row instead of being cut: {first:?} in {narrow:?}"
    );
}

#[gpui::test]
fn only_the_focused_tab_is_underlined(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let shown = state.open_terminal_tab(home).expect("tab");
    let hidden = state.open_terminal_tab(home).expect("tab");
    state.select_tab(home, shown).expect("select");
    let (boot, _requests) = stub_boot(state);
    let (_view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(format!("tab-focused-{shown}").leak())
            .is_some()
    );
    assert!(
        cx.debug_bounds(format!("tab-focused-{hidden}").leak())
            .is_none()
    );
}

#[gpui::test]
fn only_terminal_panes_get_a_header(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state
        .show_settings_tab(state.home().id, "Settings")
        .expect("settings");
    let settings = state.window().active_pane.expect("settings pane");
    let terminal = state.split_pane(settings, Direction::Right).expect("split");
    let (boot, _requests) = stub_boot(state);
    let (_view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();
    assert!(shown(cx, "header", terminal));
    assert!(!shown(cx, "header", settings));
}

fn attach_reporting(
    model: &mut AppModel,
    pane: PaneId,
    reported: &[u8],
    cx: &mut Context<AppModel>,
) {
    let mut data = attachment();
    data.directory = muxy_protocol::ServerPath(reported.to_vec());
    model.receive(
        (
            ServerId::local(),
            1,
            Update::Attached {
                pane,
                session: SessionId::new(100).expect("session"),
                attachment: data,
                created: false,
            },
        ),
        cx,
    );
}

fn starts_in(model: &AppModel, pane: PaneId) -> Vec<u8> {
    model
        .state
        .clone()
        .prepare_creation(pane, std::path::Path::new("/fallback"))
        .0
}

#[gpui::test]
fn new_tabs_start_in_the_focused_panes_directory_only_when_configured_and_in_its_project(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::settings::NewPaneDirectory;
    let other_folder = tempfile::tempdir().expect("folder");
    for (setting, reported, inherit) in [
        (NewPaneDirectory::Project, b"/tmp".as_slice(), false),
        (NewPaneDirectory::Current, b"/tmp".as_slice(), true),
        (NewPaneDirectory::Current, b"".as_slice(), false),
    ] {
        let mut state = AppState::bootstrap().expect("state");
        let other = state
            .add_project(ServerId::local(), other_folder.path().to_owned())
            .expect("project");
        state.select_project(state.home().id).expect("home");
        let anchor = state.open_terminal_tab(state.home().id).expect("tab");
        let original = state.home().tabs[0].panes[0].id;
        state
            .set_pane_session(original, SessionId::new(100))
            .expect("session");
        let (mut boot, _requests) = stub_boot(state);
        boot.settings.panes.new_pane_directory = setting;
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            let expected: &[u8] = if inherit { b"/tmp" } else { b"/fallback" };

            attach_reporting(model, original, reported, cx);
            model.new_tab(cx);
            let pane = model.active_pane().expect("new tab");
            assert_ne!(pane, original);
            assert_eq!(starts_in(model, pane), expected);

            model.select_tab(anchor, cx);
            attach_reporting(model, original, reported, cx);
            assert_eq!(model.active_pane(), Some(original));
            model.new_tab_adjacent(anchor, muxy_app_core::TabSide::Right, cx);
            let pane = model.active_pane().expect("adjacent tab");
            assert_eq!(starts_in(model, pane), expected);

            model.select_tab(anchor, cx);
            attach_reporting(model, original, reported, cx);
            model.new_tab_in(other, cx);
            let pane = model.active_pane().expect("other project's tab");
            assert_eq!(starts_in(model, pane), b"/fallback");
        });
    }
}

#[gpui::test]
fn a_new_tab_from_a_tab_window_starts_in_that_windows_pane_directory(cx: &mut TestAppContext) {
    use crate::model::tab_windows::Surface;
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    state.open_terminal_tab(home).expect("tab");
    let main = state.home().tabs[0].panes[0].id;
    let detached = state.open_terminal_tab(home).expect("tab");
    let pane = state.home().tabs[1].panes[0].id;
    state
        .set_pane_session(main, SessionId::new(100))
        .expect("session");
    state
        .set_pane_session(pane, SessionId::new(101))
        .expect("session");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.panes.new_pane_directory = muxy_app_core::settings::NewPaneDirectory::Current;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.move_tab_to_window(detached, None, None, cx);
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        for (pane, session, directory) in [
            (main, 100, b"/main".as_slice()),
            (pane, 101, b"/detached".as_slice()),
        ] {
            let mut data = attachment();
            data.directory = muxy_protocol::ServerPath(directory.to_vec());
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane,
                        session: SessionId::new(session).expect("session"),
                        attachment: data,
                        created: false,
                    },
                ),
                cx,
            );
        }
        let id = model.state.tab_windows().next().expect("window").2.id;
        model.enter_surface(Surface::Tab(id));
        assert_eq!(model.active_pane(), Some(pane));
        model.new_tab(cx);
        let opened = model.state.window().active_pane.expect("new tab");
        assert_ne!(opened, main);
        assert_eq!(starts_in(model, opened), b"/detached");
    });
}
