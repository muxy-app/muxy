use super::*;
use muxy_app_core::settings::{AgentsPanelDock, Appearance};
use muxy_ui::panel::PanelId;

fn window(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    (view, cx)
}

fn sessions() -> PanelId {
    PanelId::new(super::super::agent_sessions::PANEL)
}

fn agents() -> PanelId {
    PanelId::new(super::super::agents_panel::PANEL)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let point = cx.debug_bounds(selector).expect(selector).center();
    cx.simulate_mouse_down(point, gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(point, gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn drag(cx: &mut VisualTestContext, selector: &'static str, by: gpui::Point<gpui::Pixels>) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let start = cx.debug_bounds(selector).expect(selector).center();
    cx.simulate_mouse_down(start, gpui::MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(start + by, Some(gpui::MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(start + by, gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn saved(model: &AppModel) -> Appearance {
    Appearance::load(&model.path.with_file_name("settings.toml")).expect("saved appearance")
}

#[gpui::test]
fn sessions_pop_out_beside_a_docked_panel_and_dock_back(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |model, window, cx| {
        model.set_agents_panel_dock(AgentsPanelDock::Right, window, cx);
        model.toggle_agent_sessions(window, cx);
    });
    view.read_with(cx, |model, _| {
        assert!(model.panels.placement(&sessions()).is_some());
        assert!(
            model.panels.placement(&agents()).is_none(),
            "one docked panel at a time"
        );
    });
    click(cx, "agent-sessions-detach");
    view.update_in(cx, |model, window, cx| {
        assert!(model.appearance.agent_sessions_detached);
        assert!(saved(model).agent_sessions_detached);
        assert!(model.panels.placement(&sessions()).is_none());
        model.set_agents_panel_dock(AgentsPanelDock::Right, window, cx);
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(
            model.panels.placement(&agents()).is_some(),
            "the docked slot is free for another panel"
        );
        assert!(
            model.agent_sessions_detached_shown(),
            "the detached panel stays open beside it"
        );
        assert_eq!(model.floating_members(), [sessions()]);
    });
    assert!(cx.debug_bounds("floating-agent-sessions").is_some());

    click(cx, "agent-sessions-detach");
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.agent_sessions_detached);
        assert!(
            model.panels.placement(&sessions()).is_some(),
            "docked again"
        );
        assert!(model.floating_members().is_empty());
    });
}

#[gpui::test]
fn floating_panels_stack_and_the_clicked_one_comes_to_the_front(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |model, window, cx| {
        model.set_agents_panel_detached(true, window, cx);
        model.appearance.agents_panel_frame = Some([100.0, 100.0, 300.0, 300.0]);
        model.set_agent_sessions_detached(true, window, cx);
        model.appearance.agent_sessions_frame = Some([500.0, 100.0, 360.0, 400.0]);
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.floating_members(),
            [agents(), sessions()],
            "the last one opened is in front"
        );
    });
    click(cx, "agents-panel-header");
    view.read_with(cx, |model, _| {
        assert_eq!(model.floating_members(), [sessions(), agents()]);
    });
    drag(
        cx,
        "floating-agent-sessions-resize-left",
        gpui::point(px(-40.0), px(0.0)),
    );
    view.read_with(cx, |model, _| {
        assert_eq!(model.floating_members(), [agents(), sessions()]);
        let [x, y, width, height] = saved(model).agent_sessions_frame.expect("saved frame");
        assert!((x - 460.0).abs() < 0.5, "the left edge moves: {x}");
        assert!((width - 400.0).abs() < 0.5, "the right edge stays: {width}");
        assert!((y - 100.0).abs() < 0.5 && (height - 400.0).abs() < 0.5);
    });
}

#[gpui::test]
fn a_detached_panel_moves_within_the_window_and_keeps_its_minimum(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |model, window, cx| {
        model.set_agent_sessions_detached(true, window, cx);
        model.appearance.agent_sessions_frame = Some([100.0, 100.0, 360.0, 400.0]);
        model.begin_floating_drag(
            &sessions(),
            None,
            gpui::point(px(10.0), px(10.0)),
            window,
            cx,
        );
        model.move_floating_drag(gpui::point(px(5000.0), px(-500.0)), window, cx);
        model.finish_floating_drag(cx);
        let [x, y, width, height] = saved(model).agent_sessions_frame.expect("frame");
        assert!(
            (x - 840.0).abs() < 0.5 && y.abs() < 0.5,
            "it stops at the window's edges: {x},{y}"
        );
        assert!((width - 360.0).abs() < 0.5 && (height - 400.0).abs() < 0.5);
        model.begin_floating_drag(
            &sessions(),
            Some(muxy_ui::floating::Edges::BOTTOM_RIGHT),
            gpui::point(px(10.0), px(10.0)),
            window,
            cx,
        );
        model.move_floating_drag(gpui::point(px(-5000.0), px(-5000.0)), window, cx);
        model.finish_floating_drag(cx);
        let [_, _, width, height] = saved(model).agent_sessions_frame.expect("frame");
        assert!(
            (width - 260.0).abs() < 0.5 && (height - 200.0).abs() < 0.5,
            "it keeps its minimum size: {width}x{height}"
        );
    });
}

#[gpui::test]
fn hiding_a_floating_panel_drops_it_from_the_stack(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update_in(cx, |model, window, cx| {
        model.set_agent_sessions_detached(true, window, cx);
        model.set_agents_panel_detached(true, window, cx);
        model.sync_floating_stack();
        model.begin_floating_drag(
            &sessions(),
            None,
            gpui::point(px(10.0), px(10.0)),
            window,
            cx,
        );
        model.toggle_agent_sessions(window, cx);
        model.sync_floating_stack();
        assert_eq!(model.floating_members(), [agents()]);
        assert!(model.floating.drag.is_none(), "its drag ends with it");
        assert_eq!(model.floating.stack.iter().collect::<Vec<_>>(), [&agents()]);
    });
}
