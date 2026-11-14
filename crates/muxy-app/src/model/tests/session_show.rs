use super::*;
use muxy_app_core::browser::api;
use serde_json::{Value, json};

fn show(
    model: &mut AppModel,
    session: &str,
    project: ProjectId,
    cx: &mut Context<AppModel>,
) -> Result<Value, String> {
    let args = json!({"session": session, "project": project.to_string()});
    model.show_session(args.as_object().expect("args"), cx)
}

fn show_in_background(
    model: &mut AppModel,
    session: &str,
    project: ProjectId,
    cx: &mut Context<AppModel>,
) -> Result<Value, String> {
    let args = json!({"session": session, "project": project.to_string(), "background": true});
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
fn a_terminal_that_ended_before_showing_closes_its_tab(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let project = state.home().id;
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
        let tabs = tab_count(model, project);
        let session = SessionId::new(7).expect("session");
        show(model, "7", project, cx).expect("shown");
        let (_, _, pane) = model
            .session_pane(ServerId::local(), session)
            .expect("a pane shows the terminal");
        model.receive(
            (
                ServerId::local(),
                1,
                Update::AttachFailed {
                    pane,
                    session: Some(session),
                    created: false,
                    error: missing_session_error(),
                },
            ),
            cx,
        );
        assert!(model.session_pane(ServerId::local(), session).is_none());
        assert_eq!(tab_count(model, project), tabs);
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

#[gpui::test]
fn showing_a_terminal_leaves_the_app_in_the_background(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let project = state.home().id;
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let in_front = |cx: &mut VisualTestContext| cx.update(|window, _| window.is_window_active());
    cx.deactivate_window();
    cx.run_until_parked();
    assert!(!in_front(cx));

    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
        show(model, "7", project, cx).expect("shown");
        show(model, "7", project, cx).expect("selected");
    });
    cx.run_until_parked();
    assert!(!in_front(cx));
}

#[gpui::test]
fn showing_in_the_background_keeps_the_window_where_it_is(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    let home = state.home().id;
    state.open_terminal_tab(home).expect("home tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(tab_count(model, project), 0);
        let focused = model.active_pane();
        let selected =
            |model: &AppModel, project| model.state.window().selected_tab.get(&project).copied();
        let home_tab = selected(model, home);
        assert!(home_tab.is_some() && focused.is_some());
        let window_unchanged = |model: &AppModel| {
            assert_eq!(model.state.current_project().id, home);
            assert_eq!(selected(model, home), home_tab);
            assert_eq!(model.active_pane(), focused);
        };

        let pane = show_in_background(model, "7", project, cx).expect("shown");
        let (shown, first_tab, shown_pane) = model
            .session_pane(ServerId::local(), SessionId::new(7).expect("session"))
            .expect("a pane shows the terminal");
        assert_eq!(shown.id, project);
        assert_eq!(pane, json!(shown_pane.to_string()));
        assert_eq!(tab_count(model, project), 1);
        assert_eq!(selected(model, project), Some(first_tab));
        window_unchanged(model);

        assert_eq!(show_in_background(model, "7", project, cx), Ok(pane));
        assert_eq!(tab_count(model, project), 1);
        window_unchanged(model);

        show_in_background(model, "8", project, cx).expect("second tab");
        assert_eq!(tab_count(model, project), 2);
        assert_eq!(selected(model, project), Some(first_tab));
        window_unchanged(model);

        let home_tabs = tab_count(model, home);
        show_in_background(model, "9", home, cx).expect("shown in the current project");
        assert_eq!(tab_count(model, home), home_tabs + 1);
        window_unchanged(model);
    });
}
