use std::time::Duration;

use gpui::{Modifiers, TestAppContext, VisualTestContext, px, size};
use muxy_app_core::settings::{AppLayout, SidebarCollapsedStyle};

use super::*;
use crate::model::banners::TOAST_TRANSITION;
use crate::model::tips::{FIRST_TIP_DELAY, TIP_DURATION, TIP_INTERVAL, TIP_RETRY};

fn open(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (view, cx)
}

fn advance(cx: &mut VisualTestContext, by: Duration) {
    cx.executor().advance_clock(by);
    cx.run_until_parked();
}

fn press(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx.debug_bounds(selector).expect(selector);
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

fn tip_on_screen(view: &Entity<AppModel>, cx: &VisualTestContext) -> bool {
    view.read_with(cx, |model, _| model.tips.toast.is_some())
}

#[gpui::test]
fn a_tip_appears_in_the_bottom_right_corner_after_a_while(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    assert!(!tip_on_screen(&view, cx));
    assert!(cx.debug_bounds("tip-toast").is_none());
    advance(cx, FIRST_TIP_DELAY);
    let toast = cx.debug_bounds("tip-toast").expect("toast");
    assert!(toast.right() <= px(1200.0));
    assert!(toast.right() > px(1200.0) - toast.size.width);
    assert!(toast.bottom() <= px(800.0));
    assert!(toast.top() > px(400.0));
    assert!(view.read_with(cx, |model, _| model.overlay.is_none()));
}

#[gpui::test]
fn got_it_hides_the_tip_and_another_comes_later(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    advance(cx, FIRST_TIP_DELAY);
    let first = view.read_with(cx, |model, _| model.tips.current());
    press(cx, "tip-got-it");
    advance(cx, TOAST_TRANSITION);
    assert!(!tip_on_screen(&view, cx));
    assert!(view.read_with(cx, |model, _| model.appearance.tips_visible));
    advance(cx, TIP_INTERVAL);
    assert!(tip_on_screen(&view, cx));
    assert_ne!(view.read_with(cx, |model, _| model.tips.current()), first);
}

#[gpui::test]
fn dont_show_tips_stops_them_and_says_where_to_turn_them_back_on(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    advance(cx, FIRST_TIP_DELAY);
    press(cx, "tip-turn-off");
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.tips_visible);
        assert_eq!(model.notice.as_deref(), Some("Tips turned off"));
    });
    advance(cx, TOAST_TRANSITION);
    assert!(!tip_on_screen(&view, cx));
    advance(cx, TIP_INTERVAL);
    assert!(!tip_on_screen(&view, cx));
}

#[gpui::test]
fn an_ignored_tip_leaves_on_its_own_unless_the_pointer_is_on_it(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    advance(cx, FIRST_TIP_DELAY);
    view.update(cx, |model, cx| model.hover_tip(true, cx));
    advance(cx, TIP_DURATION * 2);
    assert!(tip_on_screen(&view, cx));
    view.update(cx, |model, cx| model.hover_tip(false, cx));
    advance(cx, TIP_DURATION / 2);
    assert!(tip_on_screen(&view, cx));
    advance(cx, TIP_DURATION / 2);
    advance(cx, TOAST_TRANSITION);
    assert!(!tip_on_screen(&view, cx));
}

#[gpui::test]
fn the_tip_buttons_leave_the_keyboard_where_it_was(cx: &mut TestAppContext) {
    for button in ["tip-got-it", "tip-turn-off"] {
        let (view, cx) = open(cx);
        advance(cx, FIRST_TIP_DELAY);
        let held = view.read_with(cx, |model, _| model.overlay_focus.clone());
        cx.update(|window, _| held.focus(window));
        cx.run_until_parked();
        press(cx, button);
        cx.update(|window, _| assert!(held.is_focused(window), "{button}"));
    }
}

#[gpui::test]
fn a_tip_waits_for_an_open_menu_to_close(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update_in(cx, AppModel::toggle_remote_popover);
    cx.run_until_parked();
    advance(cx, FIRST_TIP_DELAY);
    assert!(!tip_on_screen(&view, cx));
    view.update(cx, AppModel::dismiss_overlay);
    advance(cx, TIP_RETRY);
    assert!(tip_on_screen(&view, cx));
}

#[gpui::test]
fn turning_tips_off_in_settings_hides_the_one_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    advance(cx, FIRST_TIP_DELAY);
    assert!(tip_on_screen(&view, cx));
    view.update(cx, |model, cx| {
        model.change_preference(crate::model::preferences::Change::Tips(false), cx);
    });
    advance(cx, TOAST_TRANSITION);
    assert!(!tip_on_screen(&view, cx));
}

#[gpui::test]
fn settings_moves_to_the_top_bar_only_while_the_footer_is_away(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    for (layout, expanded, style, footer) in [
        (
            AppLayout::ProjectFocused,
            true,
            SidebarCollapsedStyle::Icons,
            true,
        ),
        (
            AppLayout::ProjectFocused,
            false,
            SidebarCollapsedStyle::Icons,
            true,
        ),
        (
            AppLayout::ProjectFocused,
            false,
            SidebarCollapsedStyle::Hidden,
            false,
        ),
        (
            AppLayout::TabFocused,
            true,
            SidebarCollapsedStyle::Icons,
            true,
        ),
        (
            AppLayout::TabFocused,
            false,
            SidebarCollapsedStyle::Icons,
            false,
        ),
        (
            AppLayout::AgentsFocused,
            false,
            SidebarCollapsedStyle::Icons,
            false,
        ),
    ] {
        view.update(cx, |model, cx| {
            model.appearance.layout = layout;
            model.appearance.sidebar_expanded = expanded;
            model.appearance.sidebar_collapsed_style = style;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model.sidebar_footer_shown()),
            footer
        );
        assert_settings_button(cx, footer, (layout, expanded, style));
    }
}

fn assert_settings_button(cx: &mut VisualTestContext, in_footer: bool, case: impl std::fmt::Debug) {
    let button = cx.debug_bounds("settings-button").expect("settings button");
    let actions = cx
        .debug_bounds("titlebar-actions")
        .expect("titlebar actions");
    assert_eq!(actions.contains(&button.center()), !in_footer, "{case:?}");
}
