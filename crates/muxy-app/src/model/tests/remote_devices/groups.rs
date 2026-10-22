use super::*;

pub(super) fn grouped(id: ServerId, name: &str, group: Option<&str>) -> ServerEntry {
    ServerEntry {
        group: group.map(str::to_owned),
        ..entry(id, name)
    }
}

pub(super) fn bounds(cx: &mut VisualTestContext, selector: String) -> gpui::Point<gpui::Pixels> {
    cx.run_until_parked();
    cx.debug_bounds(selector.leak()).expect("shown").center()
}

pub(super) fn press(cx: &mut VisualTestContext, selector: String) {
    press_with(cx, selector, Modifiers::none());
}

pub(super) fn press_with(cx: &mut VisualTestContext, selector: String, modifiers: Modifiers) {
    let at = bounds(cx, selector);
    cx.simulate_click(at, modifiers);
    cx.run_until_parked();
}

pub(super) fn right_click(cx: &mut VisualTestContext, selector: String) {
    let at = bounds(cx, selector);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, gpui::MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, gpui::MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
}

pub(super) fn open_submenu(cx: &mut VisualTestContext, selector: String) {
    let at = bounds(cx, selector);
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.run_until_parked();
}

pub(super) fn groups(path: &std::path::Path) -> Vec<(String, Option<String>)> {
    listed(path)
        .into_iter()
        .map(|entry| (entry.name, entry.group))
        .collect()
}

pub(super) fn in_group(name: &str, group: &str) -> (String, Option<String>) {
    (name.to_owned(), Some(group.to_owned()))
}

#[test]
fn servers_are_listed_by_group_in_their_order_with_the_ungrouped_last() {
    let servers = [
        grouped(ServerId::new(), "loose", None),
        grouped(ServerId::new(), "web", Some("production")),
        grouped(ServerId::new(), "ci", Some("Build")),
        grouped(ServerId::new(), "api", Some("production")),
        grouped(ServerId::new(), "other", None),
    ];
    let listed: Vec<_> = by_group(&servers, |entry| entry)
        .into_iter()
        .map(|(group, entries)| {
            let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
            (group, names)
        })
        .collect();
    assert_eq!(
        listed,
        [
            (Some("production"), vec!["web", "api"]),
            (Some("Build"), vec!["ci"]),
            (None, vec!["loose", "other"]),
        ]
    );
}

#[gpui::test]
fn a_server_removed_from_the_popover_menu_takes_only_its_projects(cx: &mut TestAppContext) {
    let (api, devbox) = (ServerId::new(), ServerId::new());
    let (boot, _, _) = boot_with(vec![
        grouped(api, "api", Some("Production")),
        grouped(devbox, "devbox", None),
    ]);
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    let (service, tools) = (ProjectId::new(), ProjectId::new());
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        connect_remote(model, api, vec![], &[(service, "service")], cx);
        model.receive((devbox, 1, Update::Connected(vec![])), cx);
        let catalog = page(3, ProjectId::new(), &[(tools, "tools")]);
        model.receive((devbox, 1, Update::Catalog(Ok(catalog))), cx);
        model.disconnect_remote_servers(&[devbox], cx);
    });
    press(cx, "remote-servers-section".into());
    right_click(cx, format!("remote-server-{devbox}"));
    press(cx, "menu-item-5".into());
    cx.simulate_prompt_answer("Forget");
    cx.run_until_parked();
    assert_eq!(groups(&path), [in_group("api", "Production")]);
    view.read_with(cx, |model, _| {
        assert!(model.state.project(tools).is_none());
        assert!(model.state.project(service).is_some());
        assert!(
            matches!(model.overlay, Some(Overlay::RemoteServers)),
            "the popover shows what is left"
        );
    });
}

#[gpui::test]
fn the_ungrouped_heading_removes_only_ungrouped_servers(cx: &mut TestAppContext) {
    let (team, loose, other) = (ServerId::new(), ServerId::new(), ServerId::new());
    let (boot, _, _) = boot_with(vec![
        grouped(loose, "loose", None),
        grouped(team, "team", Some("Team")),
        grouped(other, "other", None),
    ]);
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        model.disconnect_remote_servers(&[team, loose, other], cx);
    });
    press(cx, "remote-servers-section".into());
    right_click(cx, "remote-group-".into());
    press(cx, "menu-item-1".into());
    cx.simulate_prompt_answer("Forget");
    cx.run_until_parked();
    assert_eq!(groups(&path), [in_group("team", "Team")]);
}

#[gpui::test]
fn group_headings_rename_and_delete_and_rows_start_new_groups(cx: &mut TestAppContext) {
    let (api, db, devbox) = (ServerId::new(), ServerId::new(), ServerId::new());
    let (boot, _, _) = boot_with(vec![
        grouped(api, "api", Some("Production")),
        grouped(db, "db", Some("Production")),
        grouped(devbox, "devbox", None),
    ]);
    let path = boot.state_path.with_file_name("settings.toml");
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&boot.settings.keymap, cx);
        cx.bind_keys(muxy_ui::text_input::key_bindings());
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        model.disconnect_remote_servers(&[api, db, devbox], cx);
    });

    press(cx, "remote-servers-section".into());
    right_click(cx, "remote-group-Production".into());
    press(cx, "menu-item-1".into());
    cx.simulate_input("Prod");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        groups(&path)[..2],
        [in_group("api", "Prod"), in_group("db", "Prod")]
    );

    press(cx, "remote-servers-section".into());
    right_click(cx, format!("remote-server-{devbox}"));
    open_submenu(cx, "menu-item-4".into());
    press(cx, "menu-item-1-2".into());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |model, _| model.error.is_some()),
        "a new group needs a name"
    );
    cx.simulate_input("Personal");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(groups(&path)[2], in_group("devbox", "Personal"));

    press(cx, "remote-servers-section".into());
    right_click(cx, "remote-group-Prod".into());
    press(cx, "menu-item-2".into());
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Delete");
    cx.run_until_parked();
    assert_eq!(
        groups(&path),
        [
            ("api".to_owned(), None),
            ("db".to_owned(), None),
            in_group("devbox", "Personal"),
        ],
        "deleting a group keeps its servers"
    );

    view.update(cx, |model, cx| {
        let typed = DeviceForm {
            group: " Build ".into(),
            ..form("ci", "ci@build")
        };
        model.save_remote_server(None, &typed, cx).expect("added");
        assert!(
            model
                .move_remote_servers(&[api], Some("\u{7}"), cx)
                .is_err()
        );
    });
    assert_eq!(groups(&path)[3], in_group("ci", "Build"));
}

#[gpui::test]
fn a_rows_edit_button_opens_its_form(cx: &mut TestAppContext) {
    let devbox = ServerId::new();
    let (boot, _, _) = boot_with(vec![grouped(devbox, "devbox", None)]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    view.update(cx, connect_local);
    press(cx, "remote-servers-section".into());
    press(cx, format!("remote-server-edit-{devbox}"));
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::ServerForm(_))));
    });
}

#[gpui::test]
fn connecting_several_servers_asks_only_for_the_first_missing_password(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let (open, first, second) = (ServerId::new(), ServerId::new(), ServerId::new());
    let asking = |id, name| ServerEntry {
        password_login: true,
        ..entry(id, name)
    };
    let (boot, _, _) = boot_with(vec![
        asking(first, "first"),
        entry(open, "open"),
        asking(second, "second"),
    ]);
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&boot.settings.keymap, cx);
        cx.bind_keys(muxy_ui::text_input::key_bindings());
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.reconnect_remote_servers(&[first, open, second], cx);
        assert_eq!(model.connection(open), ConnectionState::Connecting);
        assert!(matches!(model.overlay, Some(Overlay::Password(_))));
    });
    cx.run_until_parked();
    cx.simulate_input("hunter2");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.connection(first), ConnectionState::Connecting);
        assert_eq!(model.connection(second), ConnectionState::Disconnected);
        assert!(model.overlay.is_none(), "the second one is left for later");
    });
}
