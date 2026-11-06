use super::*;

fn named(requests: &std::sync::mpsc::Receiver<(u64, Work)>) -> Vec<(SessionId, Option<String>)> {
    requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::SessionName(session, name) => Some((session, name)),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn renaming_a_tab_names_its_sessions_on_the_server_once(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        let home = model.state.home().id;
        let (renamed, other) = (
            SessionId::new(701).expect("session"),
            SessionId::new(702).expect("session"),
        );
        let tab = model.state.open_terminal_tab(home).expect("tab");
        let pane = model.state.home().tabs[0].panes[0].id;
        model
            .state
            .set_pane_session(pane, Some(renamed))
            .expect("session");
        model.state.open_terminal_tab(home).expect("tab");
        let plain = model.state.home().tabs[1].panes[0].id;
        model
            .state
            .set_pane_session(plain, Some(other))
            .expect("session");
        model.sync_visible(cx);
        assert_eq!(named(&requests), [], "unnamed tabs clear nothing");

        assert!(model.edit_tab(
            |state| state.set_tab_title(tab, Some("API server".into())),
            cx
        ));
        assert_eq!(named(&requests), [(renamed, Some("API server".into()))]);
        assert!(model.edit_tab(
            |state| state.set_tab_title(tab, Some("API server".into())),
            cx
        ));
        model.sync_visible(cx);
        assert_eq!(named(&requests), [], "an unchanged name is sent once");

        let listed = [renamed, other]
            .map(|id| SessionInfo {
                project: home,
                id,
                directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
            })
            .to_vec();
        model.receive((ServerId::local(), 1, Update::Connected(listed)), cx);
        acknowledge_catalog(model, cx);
        model.sync_visible(cx);
        assert_eq!(
            named(&requests),
            [(renamed, Some("API server".into()))],
            "a new connection hears the name again"
        );

        model.receive(
            (
                ServerId::local(),
                1,
                Update::Event(ClientEvent::Disconnected),
            ),
            cx,
        );
        assert!(model.edit_tab(|state| state.set_tab_title(tab, None), cx));
        assert_eq!(named(&requests), [], "nothing is sent while disconnected");
        let saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved.given_session_names(ServerId::local()),
            BTreeMap::from([(renamed, "API server".to_owned())]),
            "the app remembers the name it gave across a restart"
        );

        let listed = [renamed, other]
            .map(|id| SessionInfo {
                project: home,
                id,
                directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
            })
            .to_vec();
        model.receive((ServerId::local(), 1, Update::Connected(listed)), cx);
        acknowledge_catalog(model, cx);
        model.sync_visible(cx);
        assert_eq!(
            named(&requests),
            [(renamed, None)],
            "a title removed while disconnected clears the name on reconnect"
        );
        assert!(
            model
                .state
                .given_session_names(ServerId::local())
                .is_empty()
        );
        model.sync_visible(cx);
        assert_eq!(named(&requests), []);
    });
}
