use super::*;
use muxy_app_core::Direction;
use muxy_app_core::settings::ProxySettings;
use muxy_protocol::{ChannelId, Feature};

fn proxy(global: bool) -> ProxySettings {
    ProxySettings {
        host: "127.0.0.1".into(),
        port: 8080,
        global,
        ..ProxySettings::default()
    }
}

fn connect(model: &mut AppModel, features: Vec<Feature>, cx: &mut Context<AppModel>) {
    model.servers.local.connection = ConnectionState::Ready;
    model.receive(
        local(
            1,
            Update::ServerInfo(muxy_protocol::ServerInfo::current(), features),
        ),
        cx,
    );
}

fn attach(model: &mut AppModel, pane: PaneId, session: u64, cx: &mut Context<AppModel>) {
    let mut attachment = attachment();
    attachment.channel = ChannelId(u32::try_from(session).expect("channel"));
    model.receive(
        local(
            1,
            Update::Attached {
                pane,
                session: SessionId::new(session).expect("session"),
                attachment,
                created: false,
            },
        ),
        cx,
    );
}

/// The environment each new terminal was asked to start with.
fn new_terminals(
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
) -> Vec<(PaneId, BTreeMap<String, String>)> {
    requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::Attach {
                pane,
                session: None,
                environment,
                ..
            } => Some((pane, environment)),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn applying_and_removing_the_proxy_restarts_the_terminal_in_its_place(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    for (pane, session) in [(first, 100), (second, 101)] {
        state
            .set_pane_session(pane, SessionId::new(session))
            .expect("session");
    }
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, vec![Feature::SESSION_ENVIRONMENT], cx);
        attach(model, first, 100, cx);
        attach(model, second, 101, cx);
        assert!(!model.can_restart_terminal(second, true));
        model.set_proxy(Some(proxy(false)), cx).expect("saved");
        assert!(model.can_restart_terminal(second, true));
        let _ = requests.try_iter().count();

        model.restart_terminal(second, true, cx);
        acknowledge_close(model, cx);

        let tab = &model.state.home().tabs[0];
        let restarted = tab.panes[1].id;
        assert_ne!(restarted, second);
        assert!(tab.panes[1].proxy);
        assert!(tab.uses_proxy());
        assert_eq!(tab.layout.leaves(), [first, restarted]);
        assert_eq!(model.active_pane(), Some(restarted));
        assert_eq!(
            model.state.pending_discards(ServerId::local()),
            [SessionId::new(101).expect("session")]
        );
        let created = new_terminals(&requests);
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].0, restarted);
        assert_eq!(created[0].1["http_proxy"], "http://127.0.0.1:8080");
        assert!(
            store::load(&model.path).expect("saved").home().tabs[0].panes[1].proxy,
            "the flag is saved with the layout"
        );

        attach(model, restarted, 102, cx);
        model.restart_terminal(restarted, false, cx);
        acknowledge_close(model, cx);
        let tab = &model.state.home().tabs[0];
        assert!(!tab.panes[1].proxy);
        assert!(!tab.uses_proxy());
        let created = new_terminals(&requests);
        assert_eq!(created.len(), 1);
        assert!(created[0].1.is_empty());
    });
}

#[gpui::test]
fn a_whole_app_proxy_reaches_new_terminals_and_their_splits(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, vec![Feature::SESSION_ENVIRONMENT], cx);
        model.set_proxy(Some(proxy(true)), cx).expect("saved");
        let _ = requests.try_iter().count();
        model.new_tab(cx);
        let pane = model.state.home().tabs[0].panes[0].id;
        model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
        assert!(model.state.home().tabs[0].uses_proxy());
        let created = new_terminals(&requests);
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].1["all_proxy"], "http://127.0.0.1:8080");
        assert_eq!(created[0].1["no_proxy"], "");
        assert_eq!(created[0].1["NO_PROXY"], "");

        model.set_proxy(Some(proxy(false)), cx).expect("saved");
        attach(model, pane, 100, cx);
        model.split_pane(Direction::Right, cx);
        let split = new_terminals(&requests);
        assert_eq!(split.len(), 1);
        assert_eq!(split[0].1["http_proxy"], "http://127.0.0.1:8080");
    });
}

#[gpui::test]
fn servers_that_cannot_take_an_environment_get_no_proxy(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, Vec::new(), cx);
        model.set_proxy(Some(proxy(true)), cx).expect("saved");
        let _ = requests.try_iter().count();
        model.new_tab(cx);
        let pane = model.state.home().tabs[0].panes[0].id;
        model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
        assert!(!model.state.home().tabs[0].uses_proxy());
        let created = new_terminals(&requests);
        assert_eq!(created.len(), 1);
        assert!(created[0].1.is_empty());
        attach(model, pane, 100, cx);
        assert!(!model.can_restart_terminal(pane, true));
        assert!(model.can_restart_terminal(pane, false));
    });
}

#[gpui::test]
fn the_proxy_is_saved_privately_and_removed(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let path = model.path.with_file_name("proxy.toml");
        model.set_proxy(Some(proxy(true)), cx).expect("saved");
        assert_eq!(ProxySettings::load(&path).expect("load"), Some(proxy(true)));
        assert!(model.proxy.global());
        model.set_proxy(None, cx).expect("removed");
        assert!(!path.exists());
        assert!(model.proxy.settings.is_none());
        assert!(
            model
                .set_proxy(
                    Some(ProxySettings {
                        port: 0,
                        ..proxy(true)
                    }),
                    cx
                )
                .is_err()
        );
        assert!(model.proxy.settings.is_none());
    });
}

#[gpui::test]
fn the_proxy_can_be_configured_saved_and_removed_with_only_the_keyboard(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    view.update_in(cx, AppModel::open_proxy_popover);
    cx.run_until_parked();
    let host = view.update_in(cx, |_, window, cx| window.focused(cx).expect("host focus"));
    for direction in ["tab", "shift-tab"] {
        let mut visited = Vec::new();
        for _ in 0..10 {
            cx.simulate_keystrokes(direction);
            let focused = view.update_in(cx, |model, window, cx| {
                assert!(model.overlay_focus.contains_focused(window, cx));
                window.focused(cx).expect("form focus")
            });
            assert!(!visited.contains(&focused));
            visited.push(focused);
        }
        assert_eq!(visited.last(), Some(&host));
    }
    cx.simulate_keystrokes("shift-tab space tab");
    cx.simulate_input("127.0.0.1");
    cx.simulate_keystrokes("tab");
    cx.simulate_input("1080");
    cx.simulate_keystrokes("tab");
    cx.simulate_input("review");
    cx.simulate_keystrokes("tab");
    cx.simulate_input("test password");
    cx.simulate_keystrokes("tab tab space tab space");
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        let settings = model.proxy.settings.as_ref().expect("saved proxy");
        assert_eq!(settings.kind, muxy_app_core::settings::ProxyKind::Socks5);
        assert_eq!(settings.host, "127.0.0.1");
        assert_eq!(settings.port, 1080);
        assert_eq!(settings.username, "review");
        assert_eq!(settings.password, "test password");
        assert!(settings.global);
        assert_eq!(
            ProxySettings::load(&model.path.with_file_name("proxy.toml")).expect("load"),
            Some(settings.clone())
        );
    });
    view.update_in(cx, AppModel::open_proxy_popover);
    cx.run_until_parked();
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab shift-tab shift-tab space");
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.proxy.settings.is_none());
        assert!(!model.path.with_file_name("proxy.toml").exists());
    });
    view.update_in(cx, AppModel::open_proxy_popover);
    cx.run_until_parked();
    cx.simulate_keystrokes("shift-tab shift-tab shift-tab space");
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
}

#[gpui::test]
fn a_pinned_tab_with_one_terminal_can_restart_with_the_proxy(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let tab = state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    state
        .set_pane_session(pane, SessionId::new(100))
        .expect("session");
    state.toggle_tab_pin(tab).expect("pin");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, Vec::new(), cx);
        assert!(model.proxy_needs_newer_server());
        connect(model, vec![Feature::SESSION_ENVIRONMENT], cx);
        assert!(!model.proxy_needs_newer_server());
        attach(model, pane, 100, cx);
        model.set_proxy(Some(proxy(false)), cx).expect("saved");
        let _ = requests.try_iter().count();
        model.restart_terminal(pane, true, cx);
        acknowledge_close(model, cx);
        let tab = &model.state.home().tabs[0];
        assert!(tab.pinned);
        assert_ne!(tab.panes[0].id, pane);
        assert!(tab.uses_proxy());
        assert_eq!(new_terminals(&requests).len(), 1);
    });
}
