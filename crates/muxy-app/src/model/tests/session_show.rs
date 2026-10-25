use super::*;
use muxy_app_core::browser::api;
use serde_json::{Value, json};

fn show(
    model: &mut AppModel,
    session: &str,
    project: ProjectId,
    cx: &mut Context<AppModel>,
) -> std::result::Result<Value, String> {
    let args = json!({"session": session, "project": project.to_string()});
    model.show_session(args.as_object().expect("args"), cx)
}

fn tab_count(model: &AppModel, project: ProjectId) -> usize {
    model
        .state
        .projects()
        .iter()
        .find(|candidate| candidate.id == project)
        .map_or(0, |candidate| candidate.tabs.len())
}

#[gpui::test]
fn showing_a_terminal_opens_one_tab_and_then_selects_it(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let project = state.home().id;
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
        let tabs = tab_count(model, project);
        let pane = show(model, "7", project, cx).expect("shown");
        let session = SessionId::new(7).expect("session");
        let (shown, _, shown_pane) = model
            .session_pane(ServerId::local(), session)
            .expect("a pane shows the terminal");
        assert_eq!(shown.id, project);
        assert_eq!(pane, json!(shown_pane.to_string()));
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.active_pane(), Some(shown_pane));
        assert_eq!(tab_count(model, project), tabs + 1);

        assert_eq!(show(model, "7", project, cx), Ok(pane));
        assert_eq!(tab_count(model, project), tabs + 1);
    });
}

#[gpui::test]
fn showing_a_terminal_reveals_its_project_workspace(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    let work = state.create_workspace("Work").expect("workspace");
    state
        .set_workspace_member(work, project, true)
        .expect("member");
    let other = state.create_workspace("Other").expect("workspace");
    state
        .select_workspace(Some(other))
        .expect("other workspace");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
        show(model, "7", project, cx).expect("shown");
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.state.window().workspace, Some(work));

        let home = model.state.home().id;
        model.select_project(home, cx);
        model
            .state
            .select_workspace(Some(other))
            .expect("other workspace");
        show(model, "7", project, cx).expect("shown again");
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.state.window().workspace, Some(work));
    });
}

#[gpui::test]
fn showing_waits_for_the_server_and_rejects_bad_ids(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let project = state.home().id;
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(
            show(model, "7", project, cx),
            Err(api::NOT_READY.to_owned())
        );
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
        assert_eq!(
            show(model, "7", ProjectId::new(), cx),
            Err(api::NOT_READY.to_owned())
        );
        assert_eq!(
            show(model, "0", project, cx),
            Err("invalid session ID".to_owned())
        );
        assert!(
            model
                .session_pane(ServerId::local(), SessionId::new(7).expect("session"))
                .is_none()
        );
    });
}
