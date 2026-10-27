use super::groups::{grouped, groups, in_group, press, press_with, right_click};
use super::*;

fn open_tab(view: &Entity<AppModel>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |model, cx| model.open_servers_tab(window, cx));
    });
    cx.run_until_parked();
}

fn tab_window(
    servers: Vec<ServerEntry>,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext, PathBuf, Remotes) {
    let (boot, _, remotes) = boot_with(servers);
    let path = boot.state_path.with_file_name("settings.toml");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(900.0)));
    view.update(cx, connect_local);
    (view, cx, path, remotes)
}

#[gpui::test]
fn manage_servers_opens_one_remote_servers_tab(cx: &mut TestAppContext) {
    let devbox = ServerId::new();
    let (view, cx, _, _) = tab_window(vec![grouped(devbox, "devbox", None)], cx);
    press(cx, "remote-servers-section".into());
    press(cx, "remote-manage-servers".into());
    open_tab(&view, cx);
    view.read_with(cx, |model, _| {
        let pane = model.active_pane().expect("pane");
        assert!(model.state.is_servers_pane(pane));
        let tabs: usize = model
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .filter(|tab| {
                tab.panes
                    .iter()
                    .any(|pane| model.state.is_servers_pane(pane.id))
            })
            .count();
        assert_eq!(tabs, 1, "opening it again shows the same tab");
        assert!(model.overlay.is_none());
    });
    assert!(cx.debug_bounds("servers-tab").is_some());
}

#[gpui::test]
fn boxes_and_select_all_choose_what_the_bar_acts_on(cx: &mut TestAppContext) {
    let (api, db, devbox) = (ServerId::new(), ServerId::new(), ServerId::new());
    let (view, cx, path, _) = tab_window(
        vec![
            grouped(api, "api", Some("Production")),
            grouped(db, "db", Some("Production")),
            grouped(devbox, "devbox", None),
        ],
        cx,
    );
    open_tab(&view, cx);
    let selection =
        |cx: &mut VisualTestContext| view.read_with(cx, |model, _| model.remote_selection.clone());
    press(cx, format!("servers-row-{api}"));
    assert!(selection(cx).is_empty(), "clicking a row selects nothing");

    press(cx, format!("checkbox-server-{api}"));
    press(cx, format!("checkbox-server-{devbox}"));
    assert_eq!(selection(cx), [api, devbox]);
    press(cx, "checkbox-servers-select-all".into());
    assert_eq!(
        selection(cx),
        [api, devbox, db],
        "Select all ticks the rest"
    );
    press(cx, "checkbox-servers-select-all".into());
    assert!(
        selection(cx).is_empty(),
        "and clears them once all are ticked"
    );
    press(cx, "servers-select-all-label".into());
    assert_eq!(selection(cx).len(), 3, "so does its label");
    press(cx, "checkbox-servers-select-all".into());

    press(cx, format!("checkbox-server-{db}"));
    press_with(cx, format!("checkbox-server-{devbox}"), Modifiers::shift());
    assert_eq!(selection(cx), [db, devbox], "a range in shown order");
    press(cx, "servers-remove".into());
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Forget");
    cx.run_until_parked();
    assert_eq!(groups(&path), [in_group("api", "Production")]);
}

#[gpui::test]
fn right_click_acts_on_the_server_or_the_selection_without_changing_it(cx: &mut TestAppContext) {
    let (api, db, devbox) = (ServerId::new(), ServerId::new(), ServerId::new());
    let (view, cx, path, _) = tab_window(
        vec![
            grouped(api, "api", Some("Production")),
            grouped(db, "db", Some("Production")),
            grouped(devbox, "devbox", None),
        ],
        cx,
    );
    view.update(cx, |model, cx| {
        model.disconnect_remote_servers(&[api, db, devbox], cx);
    });
    open_tab(&view, cx);
    press(cx, format!("checkbox-server-{api}"));
    right_click(cx, format!("servers-row-{devbox}"));
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Menu(_))));
        assert_eq!(model.remote_selection, [api], "right-click selects nothing");
    });
    press(cx, "menu-item-5".into());
    cx.simulate_prompt_answer("Forget");
    cx.run_until_parked();
    assert_eq!(
        groups(&path),
        [in_group("api", "Production"), in_group("db", "Production")],
        "an unselected server's menu acts on that server only"
    );

    press(cx, format!("checkbox-server-{db}"));
    right_click(cx, format!("servers-row-{db}"));
    view.read_with(cx, |model, _| assert_eq!(model.remote_selection, [api, db]));
    press(cx, "menu-item-2".into());
    cx.simulate_prompt_answer("Forget");
    cx.run_until_parked();
    assert!(
        groups(&path).is_empty(),
        "a selected server's menu acts on all"
    );
}

#[gpui::test]
fn a_disconnected_server_waits_for_connect_or_its_project(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (view, cx, _, remotes) = tab_window(vec![grouped(remote, "box", None)], cx);
    let project = ProjectId::new();
    view.update(cx, |model, cx| {
        connect_remote(model, remote, vec![], &[(project, "api")], cx);
    });
    let worker = remotes.borrow_mut().remove(&remote).expect("worker");
    work(&worker);
    view.update(cx, |model, cx| {
        model.disconnect_remote_servers(&[remote], cx);
        assert_eq!(model.connection(remote), ConnectionState::Disconnected);
        let dropped = Update::ConnectFailed("Connection closed".into(), None);
        model.receive((remote, 1, dropped), cx);
    });
    assert!(
        work(&worker)
            .iter()
            .any(|work| matches!(work, Work::Disconnect))
    );
    cx.executor().advance_clock(Duration::from_secs(120));
    cx.run_until_parked();
    assert!(
        !work(&worker)
            .iter()
            .any(|work| matches!(work, Work::Connect)),
        "it doesn't retry by itself"
    );
    view.update(cx, |model, cx| model.select_project(project, cx));
    assert!(
        work(&worker)
            .iter()
            .any(|work| matches!(work, Work::Connect)),
        "opening its project connects it"
    );
}

#[gpui::test]
fn the_search_takes_typing_and_drops_hidden_servers_from_the_selection(cx: &mut TestAppContext) {
    let (api, db) = (ServerId::new(), ServerId::new());
    let (view, cx, _, _) = tab_window(
        vec![
            grouped(api, "api", Some("Production")),
            grouped(db, "db", Some("Production")),
        ],
        cx,
    );
    open_tab(&view, cx);
    press(cx, format!("checkbox-server-{db}"));
    press(cx, "servers-search".into());
    cx.simulate_input("api");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let search = model.servers_tab.search_input().expect("search");
        assert_eq!(search.read(cx).text(), "api");
        assert!(
            model.remote_selection.is_empty(),
            "a server the search hides is no longer selected"
        );
    });
}

#[gpui::test]
fn disconnecting_a_connecting_server_tells_its_worker(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (view, cx, _, remotes) = tab_window(vec![grouped(remote, "box", None)], cx);
    let worker = remotes.borrow_mut().remove(&remote).expect("worker");
    view.update(cx, |model, cx| {
        model.reconnect_remote_server(remote, cx);
        assert_eq!(model.connection(remote), ConnectionState::Connecting);
        model.disconnect_remote_servers(&[remote], cx);
        assert_eq!(model.connection(remote), ConnectionState::Disconnected);
    });
    assert!(
        work(&worker)
            .iter()
            .any(|work| matches!(work, Work::Disconnect))
    );
}

#[gpui::test]
fn opened_from_another_computers_project_the_tab_goes_to_home(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (view, cx, _, _) = tab_window(vec![grouped(remote, "box", None)], cx);
    let project = ProjectId::new();
    view.update(cx, |model, cx| {
        connect_remote(model, remote, vec![], &[(project, "api")], cx);
        model.select_project(project, cx);
    });
    open_tab(&view, cx);
    view.read_with(cx, |model, _| {
        let (owner, _) = model.state.servers_tab().expect("tab");
        assert_eq!(owner, model.state.home().id);
    });
}
