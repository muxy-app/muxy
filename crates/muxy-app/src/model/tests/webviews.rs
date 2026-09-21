use super::*;
use muxy_app_core::webview::WebviewDescriptor;

#[gpui::test]
fn rapid_editor_and_project_switches_preserve_terminal_attachment_and_input(
    cx: &mut TestAppContext,
) {
    let (mut state, first, second, first_pane, second_pane) = projects::two_projects();
    let mut targets = Vec::new();
    for (project, pane, id) in [(first, first_pane, 41), (second, second_pane, 42)] {
        let terminal = state.project(project).expect("project").tabs[0].id;
        let session = SessionId::new(id).expect("session");
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let (editor, _) = state
            .open_webview(
                project,
                WebviewDescriptor {
                    owner: "not-installed".into(),
                    kind: "editor".into(),
                    data: serde_json::json!({"path":"file.txt"}),
                },
                "Editor",
                false,
            )
            .expect("editor");
        targets.push((project, terminal, editor, pane, session));
    }
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    for round in 0..20 {
        let (project, terminal, editor, pane, session) = targets[round % targets.len()];
        view.update(cx, |model, cx| {
            model.select_project(project, cx);
            model.select_tab(terminal, cx);
        });
        cx.run_until_parked();
        view.update(cx, |model, cx| model.select_tab(editor, cx));
        let mut late = attachment();
        late.channel = muxy_protocol::ChannelId(u32::try_from(round * 2 + 1).unwrap());
        view.update(cx, |model, cx| {
            model.receive_attached(pane, session, late, false, cx);
            assert!(model.terminal(&pane).is_none());
            model.select_tab(terminal, cx);
        });
        cx.run_until_parked();
        let channel = muxy_protocol::ChannelId(u32::try_from(round * 2 + 2).unwrap());
        view.update(cx, |model, cx| {
            let mut current = attachment();
            current.channel = channel;
            model.receive_attached(pane, session, current, false, cx);
        });
        cx.run_until_parked();
        requests.try_iter().for_each(drop);
        cx.simulate_keystrokes("x");
        cx.run_until_parked();
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::Input(actual, bytes) if actual == channel && bytes == b"x")));
        view.read_with(cx, |model, cx| {
            assert_eq!(model.active_pane(), Some(pane));
            assert_eq!(
                model.terminal(&pane).unwrap().view.read(cx).channel(),
                Some(channel)
            );
            assert!(!model.pending.contains(&pane));
        });
    }
}

#[gpui::test]
fn forwarded_webview_shortcut_can_close_tabs_including_the_last(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    for file in ["first.txt", "last.txt"] {
        state
            .open_webview(
                home,
                WebviewDescriptor {
                    owner: "not-installed".into(),
                    kind: "editor".into(),
                    data: serde_json::json!({"path":file}),
                },
                file,
                false,
            )
            .expect("editor tab");
    }
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    for remaining in [1, 0] {
        let closing = view.read_with(cx, |model, _| model.active_tab().expect("active tab"));
        cx.update(|window, cx| {
            view.update(cx, |_, cx| {
                crate::model::webviews::dispatch_shortcut(
                    Keystroke::parse("cmd-w").expect("shortcut"),
                    window,
                    cx,
                );
            });
        });
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert!(model.tab(closing).is_none());
            assert_eq!(model.state.home().tabs.len(), remaining);
            assert!(model.close_request.is_none());
        });
    }
}

#[gpui::test]
fn unavailable_webview_is_client_owned_and_can_close_while_offline(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let (tab, pane) = state
        .open_webview(
            home,
            WebviewDescriptor {
                owner: "not-installed".into(),
                kind: "editor".into(),
                data: serde_json::json!({"draft":"retained"}),
            },
            "Editor",
            false,
        )
        .expect("webview");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(pane));
        assert!(model.grids.is_empty());
        assert!(model.webviews.panes.is_empty());
        assert!(model.state.session_references().is_empty());
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Disconnected;
        model.close_tab(tab, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.state.home().tabs.is_empty()));
}

#[gpui::test]
fn unavailable_webview_click_selects_and_closes_only_its_own_split(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let (_, webview) = state
        .open_webview(
            state.home().id,
            WebviewDescriptor {
                owner: "not-installed".into(),
                kind: "editor".into(),
                data: serde_json::Value::Null,
            },
            "Editor",
            false,
        )
        .expect("webview");
    let terminal = state
        .split_pane(webview, Direction::Right)
        .expect("terminal split");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(terminal));
    });
    let bounds = cx.debug_bounds("webview-placeholder").expect("placeholder");
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(model.active_pane(), Some(webview));
        model.connection = ConnectionState::Disconnected;
        model.close_pane(model.active_pane().expect("selected pane"), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(terminal));
        assert_eq!(model.state.home().tabs[0].panes.len(), 1);
        assert_eq!(model.state.home().tabs[0].panes[0].id, terminal);
    });
}

#[gpui::test]
fn composer_and_webview_panels_replace_each_other_without_losing_the_draft(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::settings::ComposerPresentation;
    use muxy_ui::panel::{PanelId, PanelPlacement};
    let state = AppState::bootstrap().expect("state");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.composer.presentation = ComposerPresentation::Panel;
    boot.settings.composer.clear_on_close = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_keystrokes("cmd-i");
    cx.simulate_input("keep this draft");
    let webview = PanelId::new("webview:foundation:right");
    view.update(cx, |model, cx| {
        let composer = model
            .composer
            .view
            .as_ref()
            .expect("composer")
            .read(cx)
            .placement();
        model.place_panel(
            PanelPlacement::new(
                webview.clone(),
                composer.position.moved(),
                composer.mode.toggled(),
            ),
            cx,
        );
        assert!(model.composer.view.is_none());
        assert!(model.panels.occupant(composer.slot()).is_none());
        assert!(model.panels.placement(&webview).is_some());
        assert_eq!(model.panels.len(), 1);
        assert!(
            model
                .panels
                .placement(&PanelId::new(crate::views::composer::PANEL))
                .is_none()
        );
    });
    cx.simulate_keystrokes("cmd-i");
    view.read_with(cx, |model, cx| {
        let composer = model
            .composer
            .view
            .as_ref()
            .expect("reopened composer")
            .read(cx);
        assert_eq!(composer.draft.text, "keep this draft");
        assert!(model.panels.placement(&webview).is_none());
        assert_eq!(model.panels.len(), 1);
    });
}
