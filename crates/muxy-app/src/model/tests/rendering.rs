use super::*;

#[gpui::test]
fn dismissing_overlay_restarts_idle_cursor_blink(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (model, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        AppModel::new(boot, window, cx)
    });
    let terminal = model.read_with(cx, |model, _| {
        model.grids[&model.active_pane().expect("pane")]
            .view
            .clone()
    });
    terminal.update(cx, |pane, cx| {
        let mut attachment = attachment();
        attachment.grid.cursor.visible = true;
        pane.attach(attachment, cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(530));
    cx.run_until_parked();
    assert!(!terminal.read_with(cx, |pane, _| pane.cursor_blink.visible));

    cx.update(|window, cx| {
        model.update(cx, |model, cx| model.toggle_notifications(window, cx));
    });
    cx.run_until_parked();
    assert!(terminal.read_with(cx, |pane, _| pane.cursor_blink.visible));
    model.update(cx, AppModel::dismiss_overlay);
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(530));
    cx.run_until_parked();
    assert!(!terminal.read_with(cx, |pane, _| pane.cursor_blink.visible));
}

#[gpui::test]
fn directional_focus_redraws_both_cached_split_panes(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    let (boot, _requests) = stub_boot(state);
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let panes = model.read_with(cx, |model, _| {
        [first, second].map(|id| model.grids[&id].view.clone())
    });
    let counts = panes
        .each_ref()
        .map(|pane| pane.read_with(cx, |pane, _| pane.render_count));
    model.update(cx, |model, cx| model.focus_direction(Direction::Left, cx));
    cx.run_until_parked();
    for (index, pane) in panes.iter().enumerate() {
        pane.read_with(cx, |pane, _| {
            assert_eq!(pane.focus_border.is_some(), index == 0);
            assert!(
                pane.render_count > counts[index],
                "pane {index} rendered {} times before and {} after",
                counts[index],
                pane.render_count
            );
        });
    }
}

#[gpui::test]
fn terminal_redraws_preserve_sidebar_and_hover_preserves_terminal(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let (sidebar, terminal) = model.read_with(cx, |model, _| {
        (
            model.sidebar_view.clone(),
            model.grids[&model.active_pane().expect("pane")]
                .view
                .clone(),
        )
    });
    let count = sidebar.read_with(cx, |sidebar, _| sidebar.render_count);
    let terminal_count = terminal.read_with(cx, |terminal, _| terminal.render_count);
    terminal.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(
        sidebar.read_with(cx, |sidebar, _| sidebar.render_count),
        count
    );
    assert!(terminal.read_with(cx, |terminal, _| terminal.render_count) > terminal_count);

    let count = terminal.read_with(cx, |terminal, _| terminal.render_count);
    let sidebar_count = sidebar.read_with(cx, |sidebar, _| sidebar.render_count);
    let row = cx.debug_bounds("project-row-0").expect("project row");
    cx.simulate_event(gpui::MouseMoveEvent {
        position: row.center(),
        ..Default::default()
    });
    cx.run_until_parked();
    assert_eq!(
        terminal.read_with(cx, |terminal, _| terminal.render_count),
        count
    );
    assert!(sidebar.read_with(cx, |sidebar, _| sidebar.render_count) > sidebar_count);

    let count = sidebar.read_with(cx, |sidebar, _| sidebar.render_count);
    let terminal_count = terminal.read_with(cx, |terminal, _| terminal.render_count);
    model.update(cx, AppModel::refresh_theme);
    cx.run_until_parked();
    assert!(sidebar.read_with(cx, |sidebar, _| sidebar.render_count) > count);
    assert!(terminal.read_with(cx, |terminal, _| terminal.render_count) > terminal_count);
}
