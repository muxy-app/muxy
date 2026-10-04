use super::*;
use muxy_app_core::settings::AppLayout;
use muxy_protocol::{ProgressState, SessionProgress, TerminalProgress};

fn report(
    model: &mut AppModel,
    session: SessionId,
    state: Option<ProgressState>,
    completed: u64,
    cx: &mut Context<AppModel>,
) {
    model.receive_event(
        ClientEvent::Progress {
            session,
            progress: SessionProgress {
                progress: state.map(|state| TerminalProgress {
                    state,
                    percent: Some(42),
                }),
                completed,
            },
        },
        cx,
    );
}

#[gpui::test]
fn progress_survives_hidden_tabs_projects_and_zoom_in_both_layouts(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tab = state.open_terminal_tab(home).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let split = state.split_pane(pane, Direction::Right).expect("split");
    let session = SessionId::new(42).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let other = state.open_terminal_tab(home).expect("other tab");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.select_tab(home, other).expect("select");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    cx.run_until_parked();
    let selector = format!("tab-progress-{tab}").leak();
    for layout in [AppLayout::ProjectFocused, AppLayout::TabFocused] {
        view.update(cx, |model, cx| {
            model.appearance.layout = layout;
            model.appearance.sidebar_expanded = true;
            model.appearance.tab_focused_expanded.insert(home, true);
            model.select_tab(other, cx);
            assert!(model.terminal(&pane).is_none());
            report(model, session, Some(ProgressState::Indeterminate), 0, cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds(selector).is_some());
        view.update(cx, |model, cx| {
            report(model, session, None, 2, cx);
            assert!(model.completions.contains(&pane));
            report(model, session, Some(ProgressState::Running), 2, cx);
            assert!(model.completions.contains(&pane));
            model.select_project(project, cx);
            assert!(model.terminal(&pane).is_none());
            report(model, session, Some(ProgressState::Paused), 2, cx);
            assert_eq!(
                model.progress[&session].progress.expect("progress").state,
                ProgressState::Paused
            );
            model.select_project(home, cx);
            model.select_tab(tab, cx);
            model.focus_pane(pane, cx);
        });
        cx.run_until_parked();
        view.update(cx, |model, cx| {
            assert!(!model.completions.contains(&pane));
            model.focus_pane(split, cx);
            model.toggle_zoom_pane(cx);
            report(model, session, Some(ProgressState::Error), 2, cx);
            assert!(model.terminal(&pane).is_none());
            assert_eq!(
                model.progress[&session].progress.expect("progress").state,
                ProgressState::Error
            );
            model.toggle_zoom_pane(cx);
        });
    }
    view.update(cx, |model, cx| {
        model.disconnect(cx);
        assert!(
            model
                .progress
                .values()
                .all(|state| state.progress.is_none())
        );
        report(model, session, None, 2, cx);
        assert!(model.completions.is_empty());
    });
}
