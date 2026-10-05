use super::super::preferences::{click_preference, settings_window};
use super::*;
use crate::model::tips::{HIDE_ACTION, TIPS, TipPlacement};
use crate::views::settings::Change;
use muxy_app_core::settings::{AppLayout, Settings, SidebarCollapsedStyle};

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let bounds = cx.debug_bounds(selector).expect(selector);
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

fn position(view: &Entity<AppModel>, cx: &VisualTestContext) -> usize {
    view.read_with(cx, |model, _| model.tips.position())
}

fn following(position: usize) -> usize {
    position % TIPS.len() + 1
}

#[gpui::test]
fn expanded_sidebar_card_steps_through_tips_in_both_layouts(cx: &mut TestAppContext) {
    for layout in [AppLayout::ProjectFocused, AppLayout::TabFocused] {
        let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
        boot.settings.appearance.layout = layout;
        boot.settings.appearance.sidebar_expanded = true;
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.run_until_parked();
        let sidebar = cx.debug_bounds("workspace-sidebar").expect("sidebar");
        let card = cx.debug_bounds("sidebar-tip").expect("tip card");
        assert!(sidebar.contains(&card.origin) && card.right() <= sidebar.right());
        let remote = cx.debug_bounds("remote-servers-section").expect("Remote");
        assert!(card.bottom() <= remote.top() && remote.top() - card.bottom() < px(20.0));
        assert!(
            remote.bottom() <= sidebar.bottom() && sidebar.bottom() - remote.bottom() < px(20.0)
        );
        assert!(cx.debug_bounds("sidebar-tip-button").is_none());
        let start = position(&view, cx);
        click(cx, "next-tip");
        assert_eq!(position(&view, cx), following(start));
        click(cx, "previous-tip");
        click(cx, "previous-tip");
        assert_eq!(following(following(position(&view, cx))), following(start));
        view.read_with(cx, |model, _| {
            assert_eq!(model.tips.current(), TIPS[model.tips.position() - 1]);
        });
    }
}

#[gpui::test]
fn hiding_tips_asks_first_saves_the_choice_and_settings_bring_them_back(cx: &mut TestAppContext) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.appearance.sidebar_expanded = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    click(cx, "hide-tips");
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.tip_placement(), Some(TipPlacement::Card));
    });
    click(cx, "hide-tips");
    cx.simulate_prompt_answer(HIDE_ACTION);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.tip_placement(), None);
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).expect("settings");
        assert!(!saved.appearance.tips_visible);
    });
    view.update(cx, |model, cx| {
        model.change_preference(Change::Tips(true), cx);
        assert_eq!(model.tip_placement(), Some(TipPlacement::Card));
    });
}

#[gpui::test]
fn collapsed_sidebar_opens_the_tip_in_a_popover_above_its_button(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar-tip").is_none());
    click(cx, "sidebar-tip-button");
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Tip)));
    });
    let button = cx.debug_bounds("sidebar-tip-button").expect("button");
    let panel = cx.debug_bounds("tip-popover").expect("popover");
    assert!(panel.bottom() <= button.top());
    assert!(panel.left() >= px(0.0) && panel.top() >= px(0.0));
    let start = position(&view, cx);
    click(cx, "next-tip");
    assert_eq!(position(&view, cx), following(start));
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Tip)));
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    click(cx, "sidebar-tip-button");
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Tip)));
    });
    view.update(cx, |model, cx| {
        model.change_preference(Change::Tips(false), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.tip_placement(), None);
    });
}

#[gpui::test]
fn tips_show_only_where_the_built_in_sidebar_is_visible(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        assert_eq!(model.tip_placement(), Some(TipPlacement::Button));
        model.appearance.sidebar_collapsed_style = SidebarCollapsedStyle::Hidden;
        assert_eq!(model.tip_placement(), None);
        model.appearance.sidebar_collapsed_style = SidebarCollapsedStyle::Icons;
        model.appearance.layout = AppLayout::TabFocused;
        assert_eq!(model.tip_placement(), None);
        model.appearance.sidebar_expanded = true;
        assert_eq!(model.tip_placement(), Some(TipPlacement::Card));
        model.appearance.extension_sidebar = "example.sidebar".into();
        assert_eq!(
            model.extension_sidebar_active(),
            model.tip_placement().is_none()
        );
        model.appearance.extension_sidebar.clear();
        model.appearance.tips_visible = false;
        assert_eq!(model.tip_placement(), None);
    });
}

#[gpui::test]
fn settings_toggle_hides_and_shows_tips(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = settings_window(boot, cx);
    click_preference(cx, "settings-search");
    cx.simulate_keystrokes("s h o w space t i p s");
    cx.run_until_parked();
    click_preference(cx, "settings-toggle-tips");
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.tips_visible);
        assert_eq!(model.tip_placement(), None);
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).expect("settings");
        assert!(!saved.appearance.tips_visible);
    });
    click_preference(cx, "settings-toggle-tips");
    view.read_with(cx, |model, _| assert!(model.appearance.tips_visible));
}
