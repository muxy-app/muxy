use super::*;
use gpui::{MouseButton, point};
use muxy_app_core::{Direction, FloatScope, TabWindowId};

use crate::model::tab_windows::Surface;
use crate::model::tab_windows::floating::panel;

fn windows(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| cx.windows().len())
}

fn listed(model: &AppModel, project: ProjectId) -> Vec<TabId> {
    model
        .state
        .project(project)
        .map(|project| project.listed_tabs().map(|tab| tab.id).collect())
        .unwrap_or_default()
}

fn float_of(model: &AppModel, pane: PaneId) -> TabWindowId {
    model.state.pane_window(pane).expect("floating tab")
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Home with a tab of two terminals, the second floated, and another
/// project with a tab of its own.
fn floated(
    cx: &mut TestAppContext,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    ProjectId,
    ProjectId,
    PaneId,
    PaneId,
) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tab = state.open_terminal_tab(home).expect("tab");
    let main = state
        .project(home)
        .and_then(|project| project.tabs.iter().find(|candidate| candidate.id == tab))
        .map(|tab| tab.panes[0].id)
        .expect("pane");
    let pane = state.split_pane(main, Direction::Right).expect("split");
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.open_terminal_tab(other).expect("tab");
    let _ = state.select_project(home);
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update(cx, |model, cx| {
        model.float_pane(pane, FloatScope::Project, cx);
    });
    redraw(cx);
    (view, cx, home, other, main, pane)
}

#[gpui::test]
fn a_floating_pane_takes_the_keyboard_and_splits_inside_itself(cx: &mut TestAppContext) {
    let (view, cx, home, _, main, pane) = floated(cx);
    assert_eq!(windows(cx), 1);
    view.update(cx, |model, cx| {
        let id = float_of(model, pane);
        let tab = model.window_tab(id).expect("tab");
        assert_eq!(model.key_float(), Some(id));
        assert_eq!(model.active_tab(), Some(tab));
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(listed(model, home).len(), 1);
        assert!(model.floating_members().contains(&panel(id)));
        assert!(model.tab_windows.handle(id).is_none());
        model.split_pane(Direction::Down, cx);
        let split = model.active_pane().expect("split");
        assert_eq!(model.tab(tab).expect("tab").layout.leaves(), [pane, split]);
        assert!(model.attached_panes().contains(&split));
        assert!(model.attached_panes().contains(&main));
        model.enter_surface(Surface::Main);
        assert_eq!(model.active_pane(), Some(main));
    });
    assert!(cx.debug_bounds("floating-tab").is_some());
}

#[gpui::test]
fn a_project_float_shows_only_over_its_project(cx: &mut TestAppContext) {
    let (view, cx, home, other, _, pane) = floated(cx);
    view.update(cx, |model, cx| {
        let id = float_of(model, pane);
        model.select_project(other, cx);
        assert!(!model.float_shown(id));
        assert_eq!(model.key_float(), None);
        assert!(!model.attached_panes().contains(&pane));
        assert!(!model.floating_members().contains(&panel(id)));
        model.select_project(home, cx);
        assert!(model.float_shown(id));
        model.set_float_scope(id, FloatScope::AllProjects, cx);
        model.select_project(other, cx);
        assert!(model.float_shown(id));
        assert!(model.attached_panes().contains(&pane));
    });
    redraw(cx);
    assert!(cx.debug_bounds("floating-tab").is_some());
}

#[gpui::test]
fn toggling_hides_every_float_and_hands_the_keyboard_back(cx: &mut TestAppContext) {
    let (view, cx, _, _, main, pane) = floated(cx);
    view.update(cx, |model, cx| {
        let id = float_of(model, pane);
        model.toggle_floats(cx);
        assert!(model.state.floats_hidden());
        assert!(!model.float_shown(id));
        assert_eq!(model.key_float(), None);
        assert_eq!(model.active_pane(), Some(main));
        assert!(!model.attached_panes().contains(&pane));
        model.toggle_floats(cx);
        assert!(model.float_shown(id));
        assert_eq!(model.key_float(), Some(id));
        assert!(model.attached_panes().contains(&pane));
    });
}

#[gpui::test]
fn showing_a_pane_in_a_hidden_float_reveals_it_over_its_project(cx: &mut TestAppContext) {
    let (view, cx, home, other, _, pane) = floated(cx);
    view.update(cx, |model, cx| {
        let id = float_of(model, pane);
        model.toggle_floats(cx);
        model.select_project(other, cx);
        assert!(model.raise_pane_window(pane, cx));
        assert!(model.float_shown(id));
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(model.key_float(), Some(id));
    });
}

fn bounds(cx: &mut VisualTestContext, selector: String) -> gpui::Bounds<gpui::Pixels> {
    cx.debug_bounds(selector.leak()).expect("drawn")
}

#[gpui::test]
fn the_pane_header_moves_the_float_and_holds_its_scope_and_dock_buttons(cx: &mut TestAppContext) {
    let (view, cx, home, _, _, pane) = floated(cx);
    let id = view.read_with(cx, |model, _| float_of(model, pane));
    assert!(cx.debug_bounds("floating-tab-bar").is_none());
    let frame = |cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| {
            model
                .state
                .tab_window(id)
                .and_then(|(_, _, window)| window.floating())
                .and_then(|floating| floating.frame)
                .expect("placed")
        })
    };
    let before = frame(cx);
    let header = bounds(cx, format!("pane-header-{pane}"));
    let start = point(
        header.origin.x + header.size.width * 0.55,
        header.origin.y + header.size.height / 2.0,
    );
    let by = point(px(60.0), px(40.0));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(start + by, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(start + by, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let after = frame(cx);
    assert!(after[0] > before[0] && after[1] > before[1]);
    assert_eq!((after[2], after[3]), (before[2], before[3]));
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .tab(model.window_tab(id).expect("tab"))
                .expect("tab")
                .panes
                .len(),
            1
        );
    });
    redraw(cx);
    let scope = bounds(cx, format!("pane-float-scope-{pane}")).center();
    cx.simulate_click(scope, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.float_scope(id), Some(FloatScope::AllProjects));
    });
    redraw(cx);
    let dock = bounds(cx, format!("pane-return-{pane}")).center();
    cx.simulate_click(dock, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(listed(model, home).len(), 2);
        assert!(model.state.floating_tabs().next().is_none());
        assert_eq!(model.key_float(), None);
        assert_eq!(model.active_pane(), Some(pane));
    });
}

#[gpui::test]
fn a_float_shows_a_bar_of_its_own_without_pane_headers(cx: &mut TestAppContext) {
    let (view, cx, _, _, _, pane) = floated(cx);
    view.update(cx, |model, cx| {
        model.appearance.pane_headers = false;
        cx.notify();
    });
    redraw(cx);
    assert!(cx.debug_bounds("floating-tab-bar").is_some());
    let id = view.read_with(cx, |model, _| float_of(model, pane));
    let scope = cx
        .debug_bounds("floating-tab-scope")
        .expect("scope")
        .center();
    cx.simulate_click(scope, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.float_scope(id), Some(FloatScope::AllProjects));
    });
}

#[gpui::test]
fn a_float_becomes_a_window_of_its_own_and_back(cx: &mut TestAppContext) {
    let (view, cx, _, _, _, pane) = floated(cx);
    view.update(cx, |model, cx| {
        model.move_pane_to_window(pane, None, None, cx);
    });
    cx.run_until_parked();
    assert_eq!(windows(cx), 2);
    let id = view.read_with(cx, |model, _| {
        let id = model.state.pane_window(pane).expect("window");
        assert!(!model.is_float(id));
        assert!(model.tab_windows.handle(id).is_some());
        id
    });
    view.update(cx, |model, cx| {
        model.float_pane(pane, FloatScope::Project, cx);
    });
    cx.run_until_parked();
    assert_eq!(windows(cx), 1);
    view.read_with(cx, |model, _| {
        assert!(model.is_float(id));
        assert_eq!(model.key_float(), Some(id));
    });
}

#[gpui::test]
fn a_pane_dropped_over_a_float_lands_in_the_float(cx: &mut TestAppContext) {
    let (view, cx, _, _, _, pane) = floated(cx);
    let center = cx
        .debug_bounds("floating-tab-content")
        .expect("content")
        .center();
    let here = cx.update(|window, _| window.window_handle().window_id());
    view.read_with(cx, |model, _| {
        let float = model.window_tab(float_of(model, pane)).expect("tab");
        let placement = model.layout_drag.geometry.at(here, center).expect("placed");
        assert_eq!(placement.tab, float);
    });
}

#[gpui::test]
fn the_command_palette_acts_on_the_float_that_opened_it(cx: &mut TestAppContext) {
    let (view, cx, home, _, _, pane) = floated(cx);
    let id = view.read_with(cx, |model, _| float_of(model, pane));
    view.update_in(cx, |model, window, cx| {
        model.enter_surface(Surface::Main);
        model.focus_pane(pane, cx);
        assert_eq!(model.key_float(), Some(id));
        model.toggle_command_palette(window, cx);
        assert!(model.overlay.is_some());
        assert_eq!(model.key_float(), Some(id));
        assert_eq!(model.active_pane(), Some(pane));
    });
    redraw(cx);
    view.update(cx, |model, cx| {
        assert_eq!(model.key_float(), Some(id));
        model.dismiss_overlay(cx);
        model.return_tab_window(id, None, cx);
        assert_eq!(listed(model, home).len(), 2);
        assert_eq!(model.key_float(), None);
    });
}

#[gpui::test]
fn the_float_holding_the_keyboard_stays_the_surface_shortcuts_act_on(cx: &mut TestAppContext) {
    let (view, cx, _, _, main, pane) = floated(cx);
    let id = view.read_with(cx, |model, _| float_of(model, pane));
    cx.update(|window, _| window.activate_window());
    view.update(cx, |model, _| model.enter_surface(Surface::Main));
    redraw(cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.key_float(), Some(id));
        assert_eq!(model.active_pane(), Some(pane));
    });
    view.update_in(cx, |model, window, cx| {
        model.enter_surface(Surface::Main);
        model.focus_pane(main, cx);
        model.focus_active(window, cx);
    });
    redraw(cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.key_float(), None);
        assert_eq!(model.active_pane(), Some(main));
    });
}

#[gpui::test]
fn floating_a_tab_windows_only_pane_gives_the_float_the_keyboard(cx: &mut TestAppContext) {
    let (view, cx, _, _, _, pane) = floated(cx);
    view.update(cx, |model, cx| {
        model.move_pane_to_window(pane, None, None, cx);
    });
    cx.run_until_parked();
    let id = view.read_with(cx, |model, _| {
        model.state.pane_window(pane).expect("window")
    });
    view.update(cx, |model, cx| {
        model.enter_surface(Surface::Tab(id));
        model.float_pane(pane, FloatScope::Project, cx);
        assert_eq!(model.key_float(), Some(id));
    });
    cx.run_until_parked();
    assert_eq!(windows(cx), 1);
    view.read_with(cx, |model, _| assert_eq!(model.key_float(), Some(id)));
}

#[gpui::test]
fn a_press_just_inside_the_corner_resizes_the_float(cx: &mut TestAppContext) {
    let (view, cx, _, _, _, pane) = floated(cx);
    let id = view.read_with(cx, |model, _| float_of(model, pane));
    let card = bounds(cx, format!("floating-tab-{id}"));
    let start = card.bottom_right() - point(px(3.0), px(3.0));
    let size = |cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| {
            let frame = model
                .state
                .tab_window(id)
                .and_then(|(_, _, window)| window.floating())
                .and_then(|floating| floating.frame)
                .expect("placed");
            (frame[2], frame[3])
        })
    };
    let before = size(cx);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        start + point(px(60.0), px(40.0)),
        Some(MouseButton::Left),
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        start + point(px(60.0), px(40.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    let after = size(cx);
    assert!(
        after.0 > before.0 && after.1 > before.1,
        "{before:?} -> {after:?}"
    );
}

#[gpui::test]
fn the_pane_header_floats_a_terminal_in_place_of_detaching_it(cx: &mut TestAppContext) {
    let (view, cx, _, _, main, _) = floated(cx);
    let float = bounds(cx, format!("pane-float-{main}")).center();
    cx.simulate_click(float, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.pane_floats(main));
        assert_eq!(model.key_float(), model.state.pane_window(main));
        assert!(model.state.pane(main).is_some());
    });
}

#[gpui::test]
fn clicking_the_main_window_takes_the_keyboard_from_a_float_at_once(cx: &mut TestAppContext) {
    let (view, cx, _, _, main, pane) = floated(cx);
    let focused = |cx: &mut VisualTestContext, id: PaneId| {
        view.read_with(cx, |model, cx| {
            model
                .terminal(&id)
                .is_some_and(|terminal| terminal.view.read(cx).focused)
        })
    };
    assert!(focused(cx, pane));
    assert!(!focused(cx, main));
    let header = bounds(cx, format!("pane-header-{main}"));
    let below = point(
        header.origin.x + px(40.0),
        header.origin.y + header.size.height + px(30.0),
    );
    cx.simulate_click(below, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.key_float(), None);
        assert_eq!(model.active_pane(), Some(main));
    });
    assert!(focused(cx, main));
    assert!(!focused(cx, pane));
}

#[gpui::test]
fn showing_floats_again_puts_the_one_with_the_keyboard_in_front(cx: &mut TestAppContext) {
    let (view, cx, _, _, main, pane) = floated(cx);
    view.update(cx, |model, cx| {
        model.float_pane(main, FloatScope::Project, cx);
    });
    redraw(cx);
    view.update(cx, |model, cx| model.toggle_floats(cx));
    redraw(cx);
    view.update(cx, |model, cx| {
        model.toggle_floats(cx);
        let key = model.key_float().expect("a float has the keyboard");
        assert_eq!(model.floating.stack.frontmost(), Some(&panel(key)));
        assert!(model.float_shown(float_of(model, pane)));
    });
    redraw(cx);
    view.read_with(cx, |model, _| {
        let key = model.key_float().expect("a float has the keyboard");
        assert_eq!(model.floating.stack.frontmost(), Some(&panel(key)));
    });
}

#[gpui::test]
fn floating_a_tab_windows_terminal_shows_it_over_its_own_project(cx: &mut TestAppContext) {
    let (view, cx, home, other, _, pane) = floated(cx);
    view.update(cx, |model, cx| {
        model.move_pane_to_window(pane, None, None, cx);
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.select_project(other, cx);
        let id = model.state.pane_window(pane).expect("window");
        model.enter_surface(Surface::Tab(id));
        model.float_pane(pane, FloatScope::Project, cx);
        assert_eq!(model.state.current_project().id, home);
        assert!(model.float_shown(id));
        assert_eq!(model.key_float(), Some(id));
    });
}

#[gpui::test]
fn selecting_the_shown_tab_from_a_float_gives_the_main_pane_the_keyboard(cx: &mut TestAppContext) {
    let (view, cx, _, _, main, pane) = floated(cx);
    let tab = view.read_with(cx, |model, _| model.state.pane_tab(main).expect("tab"));
    view.update(cx, |model, cx| {
        assert!(model.key_float().is_some());
        model.select_tab(tab, cx);
        assert_eq!(model.key_float(), None);
    });
    redraw(cx);
    view.read_with(cx, |model, cx| {
        assert_eq!(model.active_pane(), Some(main));
        assert!(
            model
                .terminal(&main)
                .is_some_and(|t| t.view.read(cx).focused)
        );
        assert!(
            !model
                .terminal(&pane)
                .is_some_and(|t| t.view.read(cx).focused)
        );
    });
}
