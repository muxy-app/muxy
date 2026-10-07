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
        model.servers.local.connection = ConnectionState::Ready;
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
            model.receive_attached(ServerId::local(), pane, session, late, false, cx);
            assert!(model.terminal(&pane).is_none());
            model.select_tab(terminal, cx);
        });
        cx.run_until_parked();
        let channel = muxy_protocol::ChannelId(u32::try_from(round * 2 + 2).unwrap());
        view.update(cx, |model, cx| {
            let mut current = attachment();
            current.channel = channel;
            model.receive_attached(ServerId::local(), pane, session, current, false, cx);
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
            assert!(!model.pending.contains_key(&pane));
        });
    }
}
