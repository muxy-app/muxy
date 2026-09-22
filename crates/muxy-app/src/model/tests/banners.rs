use super::*;
use crate::model::banners::BannerKind;

#[gpui::test]
fn opening_commands_does_not_repeat_dismissed_theme_errors(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let themes = boot.state_path.with_file_name("themes");
    std::fs::create_dir_all(&themes).expect("themes directory");
    let file = themes.join("broken.theme");
    std::fs::write(&file, "not a theme").expect("invalid theme");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.command_registry(model.dark, cx);
        assert!(model.visible_banner().is_some());
        model.dismiss_banner(BannerKind::Error);
        for _ in 0..3 {
            model.command_registry(model.dark, cx);
            assert!(model.visible_banner().is_none());
        }
        std::fs::write(&file, "background = #101010\nforeground = #eeeeee").expect("valid theme");
        model.command_registry(model.dark, cx);
        assert!(model.error.is_none());
        std::fs::write(&file, "not a theme").expect("invalid again");
        model.command_registry(model.dark, cx);
        assert!(model.visible_banner().is_some());
    });
}

#[gpui::test]
fn dismissed_banners_stay_quiet_until_the_message_changes_or_recovers(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.set_configuration_error(None);
        model.fail("Server unavailable".into(), cx);
        assert_eq!(
            model.visible_banner(),
            Some((BannerKind::Error, "Server unavailable"))
        );
        model.dismiss_banner(BannerKind::Error);
        for _ in 0..3 {
            model.fail("Server unavailable".into(), cx);
            assert!(model.visible_banner().is_none());
        }
        model.fail("Could not save project".into(), cx);
        assert_eq!(
            model.visible_banner(),
            Some((BannerKind::Error, "Could not save project"))
        );
        model.dismiss_banner(BannerKind::Error);
        model.set_banner_error(None);
        model.fail("Could not save project".into(), cx);
        assert!(model.visible_banner().is_some());
        model.set_banner_error(Some(" \n".into()));
        assert!(model.visible_banner().is_none());
    });
}

#[gpui::test]
fn banners_show_one_problem_at_a_time_and_preserve_dismissed_diagnostics(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.set_configuration_error(Some("Unknown setting".into()));
        model.fail("Server unavailable".into(), cx);
        assert_eq!(
            model.visible_banner().map(|(kind, _)| kind),
            Some(BannerKind::Error)
        );
        model.dismiss_banner(BannerKind::Error);
        assert_eq!(
            model.visible_banner(),
            Some((BannerKind::Configuration, "Unknown setting"))
        );
        model.dismiss_banner(BannerKind::Configuration);
        model.set_configuration_error(Some("Unknown setting".into()));
        assert!(model.visible_banner().is_none());
        assert_eq!(
            model.configuration_error.as_deref(),
            Some("Unknown setting")
        );
        model.set_configuration_error(None);
        model.set_configuration_error(Some("Unknown setting".into()));
        assert!(model.visible_banner().is_some());
        model.dismiss_banner(BannerKind::Configuration);
        model.set_configuration_error(Some("Invalid color".into()));
        assert_eq!(
            model.visible_banner(),
            Some((BannerKind::Configuration, "Invalid color"))
        );
    });
}

#[gpui::test]
fn banner_close_control_stays_visible_for_long_messages_in_both_layouts(cx: &mut TestAppContext) {
    use muxy_app_core::settings::AppLayout;
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(600.0), px(400.0)));
    for layout in [AppLayout::ProjectFocused, AppLayout::TabFocused] {
        view.update(cx, |model, cx| {
            model.appearance.layout = layout;
            model.set_configuration_error(None);
            model.set_banner_error(None);
            model.fail("A long diagnostic message\n".repeat(100), cx);
        });
        cx.run_until_parked();
        let banner = cx.debug_bounds("workspace-banner").expect("banner");
        let close = cx
            .debug_bounds("dismiss-workspace-banner")
            .expect("close button");
        assert!(banner.size.height <= px(100.0));
        assert!(close.left() >= banner.left() && close.right() <= banner.right());
        assert!(close.top() >= banner.top() && close.bottom() <= banner.bottom());
        cx.simulate_event(gpui::MouseDownEvent {
            position: close.center(),
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        });
        cx.simulate_event(gpui::MouseUpEvent {
            position: close.center(),
            button: gpui::MouseButton::Left,
            click_count: 1,
            ..Default::default()
        });
        cx.run_until_parked();
        view.read_with(cx, |model, _| assert!(model.visible_banner().is_none()));
    }
}
