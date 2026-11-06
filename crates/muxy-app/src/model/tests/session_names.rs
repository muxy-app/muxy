use super::*;

type Asked = (SessionId, Option<String>, Option<String>);

fn named(requests: &std::sync::mpsc::Receiver<(u64, Work)>) -> Vec<Asked> {
    requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::SessionName {
                session,
                name,
                replacing,
            } => Some((session, name, replacing)),
            _ => None,
        })
        .collect()
}

fn answer(
    model: &mut AppModel,
    (session, name, _): Asked,
    result: Result<(), muxy_client::ClientError>,
    cx: &mut Context<AppModel>,
) {
    let named = Update::SessionNamed {
        session,
        name,
        result,
    };
    model.receive((ServerId::local(), 1, named), cx);
}

fn reconnect(model: &mut AppModel, sessions: &[SessionId], cx: &mut Context<AppModel>) {
    let home = model.state.home().id;
    let listed = sessions
        .iter()
        .map(|id| SessionInfo {
            project: home,
            id: *id,
            directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
        })
        .collect();
    model.receive((ServerId::local(), 1, Update::Connected(listed)), cx);
    acknowledge_catalog(model, cx);
    model.sync_visible(cx);
}

#[gpui::test]
fn renaming_a_tab_names_its_sessions_and_clears_only_what_the_server_took(cx: &mut TestAppContext) {
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
        let given = |model: &AppModel| model.state.given_session_names(ServerId::local());
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
        let asked = named(&requests);
        assert_eq!(asked, [(renamed, Some("API server".into()), None)]);
        assert!(
            given(model).is_empty(),
            "nothing is kept before the server takes it"
        );
        answer(model, asked[0].clone(), Ok(()), cx);
        assert_eq!(
            given(model),
            BTreeMap::from([(renamed, "API server".to_owned())])
        );
        assert!(model.edit_tab(
            |state| state.set_tab_title(tab, Some("API server".into())),
            cx
        ));
        model.sync_visible(cx);
        assert_eq!(named(&requests), [], "a name is asked once a connection");

        reconnect(model, &[renamed, other], cx);
        assert_eq!(
            named(&requests),
            [(renamed, Some("API server".into()), None)],
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

        reconnect(model, &[renamed, other], cx);
        let clear = (renamed, None, Some("API server".to_owned()));
        assert_eq!(
            named(&requests),
            std::slice::from_ref(&clear),
            "a title removed while disconnected clears only that name on reconnect"
        );
        answer(
            model,
            clear.clone(),
            Err(muxy_client::ClientError::Disconnected),
            cx,
        );
        assert_eq!(
            given(model),
            BTreeMap::from([(renamed, "API server".to_owned())]),
            "a lost clear is kept"
        );
        model.sync_visible(cx);
        assert_eq!(
            named(&requests),
            [],
            "a failure waits for the next connection"
        );

        reconnect(model, &[renamed, other], cx);
        assert_eq!(named(&requests), std::slice::from_ref(&clear));
        answer(model, clear, Ok(()), cx);
        assert!(given(model).is_empty());
        model.sync_visible(cx);
        assert_eq!(named(&requests), []);
    });
}
