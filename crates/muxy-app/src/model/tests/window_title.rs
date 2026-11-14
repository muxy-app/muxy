use super::*;

#[gpui::test]
fn window_title_follows_the_active_tab_and_project(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tab = state.open_terminal_tab(home).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let project = view.read_with(cx, |model, _| model.state.home().name.clone());
    assert_eq!(cx.window_title(), Some(format!("Terminal — {project}")));

    view.update(cx, |model, cx| {
        model
            .state
            .set_tab_title(tab, Some("deploy".into()))
            .expect("title");
        model.state.rename_project(home, "blog").expect("rename");
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(cx.window_title().as_deref(), Some("deploy — blog"));
}
