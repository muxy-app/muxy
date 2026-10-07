use super::*;
use muxy_app_core::{TabCloseScope, settings::CloseBehavior};

fn fixture() -> (AppState, [TabId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let ids = std::array::from_fn(|_| state.open_terminal_tab(state.home().id).expect("tab"));
    state.select_tab(state.home().id, ids[2]).expect("select");
    (state, ids)
}

#[gpui::test]
fn bulk_close_is_project_scoped_and_honors_pins_focus_and_detach(cx: &mut TestAppContext) {
    for (scope, remaining) in [
        (TabCloseScope::Left, vec![0, 2, 3]),
        (TabCloseScope::Right, vec![0, 1, 2]),
        (TabCloseScope::Other, vec![0, 2]),
    ] {
        for behavior in [CloseBehavior::CloseSession, CloseBehavior::Detach] {
            let (mut state, ids) = fixture();
            state.toggle_tab_pin(ids[0]).expect("pin");
            let other = state
                .add_project(ServerId::local(), std::env::temp_dir())
                .expect("project");
            let other_tab = state.open_terminal_tab(other).expect("other tab");
            let (mut boot, _requests) = stub_boot(state);
            boot.settings.window.close_behavior = behavior;
            let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
            view.update(cx, |model, cx| {
                model.close_tabs(ids[2], scope, cx);
                assert_eq!(
                    model
                        .state
                        .home()
                        .tabs
                        .iter()
                        .map(|tab| tab.id)
                        .collect::<Vec<_>>(),
                    remaining
                        .iter()
                        .map(|index| ids[*index])
                        .collect::<Vec<_>>()
                );
                assert_eq!(model.active_tab(), Some(other_tab));
                assert!(model.close_request.is_none());
                assert_eq!(store::load(&model.path).expect("saved"), model.state);
            });
        }
    }
}

#[gpui::test]
fn bulk_close_confirms_once_and_cancellation_preserves_every_target(cx: &mut TestAppContext) {
    let (mut state, ids) = fixture();
    for index in [0, 1, 3] {
        let pane = state.home().tabs[index].panes[0].id;
        state
            .set_pane_session(pane, SessionId::new(index as u64 + 10))
            .expect("session");
    }
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for answer in ["Cancel", "Close"] {
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            model.close_tabs(ids[2], TabCloseScope::Other, cx);
            let request = model.close_request.as_ref().expect("request");
            let tab = request.tab;
            let session = model.pane_session(request.panes[0]).expect("session");
            model.receive_close_checked(
                tab,
                session,
                Ok(Some(muxy_protocol::ForegroundProcess {
                    name: "vim".into(),
                    is_shell: false,
                })),
                cx,
            );
        });
        cx.run_until_parked();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer(answer);
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        view.read_with(cx, |model, _| {
            assert_eq!(
                model.state.home().tabs.len(),
                if answer == "Cancel" { 4 } else { 1 }
            );
            assert_eq!(model.active_tab(), Some(ids[2]));
            assert!(model.close_request.is_none());
        });
    }
}

#[gpui::test]
fn tab_edits_and_bulk_close_roll_back_on_save_failure(cx: &mut TestAppContext) {
    let (state, ids) = fixture();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let previous = model.state.clone();
        let path = model.path.clone();
        let blocked = path.with_file_name("blocked");
        std::fs::create_dir_all(blocked.parent().expect("parent")).expect("directory");
        std::fs::write(&blocked, "file").expect("blocked");
        model.path = blocked.join("state.json");
        assert!(!model.edit_tab(|state| state.toggle_tab_pin(ids[0]), cx));
        assert_eq!(model.state, previous);
        model.close_tabs(ids[2], TabCloseScope::Other, cx);
        assert_eq!(model.state, previous);
        model.path = path;
    });
}
