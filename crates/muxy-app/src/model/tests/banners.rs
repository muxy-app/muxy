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
fn toast_overlays_both_layouts_without_moving_content_or_focus(cx: &mut TestAppContext) {
    use muxy_app_core::settings::AppLayout;
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(600.0), px(400.0)));
    for (layout, selector) in [
        (AppLayout::ProjectFocused, "tab-strip"),
        (AppLayout::TabFocused, "project-titlebar"),
    ] {
        view.update(cx, |model, cx| {
            model.appearance.layout = layout;
            model.set_configuration_error(None);
            model.set_banner_error(None);
            cx.notify();
        });
        cx.run_until_parked();
        let titlebar = cx.debug_bounds(selector).expect("titlebar");
        let content = cx.debug_bounds("workspace-content").expect("content");
        let focus = cx.update(|window, app| window.focused(app));
        view.update(cx, |model, cx| {
            model.fail("A long diagnostic message ".repeat(100), cx);
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(crate::model::banners::TOAST_TRANSITION);
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let toast = cx.debug_bounds("workspace-toast").expect("toast");
        assert_eq!(cx.debug_bounds(selector), Some(titlebar));
        assert_eq!(cx.debug_bounds("workspace-content"), Some(content));
        assert!(cx.debug_bounds("dismiss-workspace-toast").is_some());
        assert_eq!(toast.top(), px(40.0));
        assert_eq!(toast.center().x, px(300.0));
        assert!(toast.size.height <= px(80.0));
        assert!(toast.size.width <= px(560.0));
        assert_eq!(cx.update(|window, app| window.focused(app)), focus);
    }
}

#[gpui::test]
fn problems_stay_until_dismissed_while_notices_expire_beside_them(cx: &mut TestAppContext) {
    use muxy_ui::toast::ToastKind;
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.set_configuration_error(None);
        model.fail("Server unavailable".into(), cx);
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_secs(10));
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        let problem = model.problem_toast.as_ref().expect("problem stays");
        assert_eq!(problem.kind, ToastKind::Error);
        model.show_notice("Saved".into(), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.error.as_deref(), Some("Server unavailable"));
        assert!(model.problem_toast.is_some());
        assert_eq!(
            model.notice_toast.as_ref().map(|toast| toast.kind),
            Some(ToastKind::Success)
        );
    });
    cx.executor().advance_clock(Duration::from_secs(3));
    cx.run_until_parked();
    cx.executor()
        .advance_clock(crate::model::banners::TOAST_TRANSITION);
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert!(model.notice_toast.is_none());
        assert!(model.problem_toast.is_some());
        model.dismiss_problem(cx);
    });
    cx.executor()
        .advance_clock(crate::model::banners::TOAST_TRANSITION);
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert!(model.problem_toast.is_none());
        model.fail("Server unavailable".into(), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(
            model.problem_toast.is_none(),
            "dismissed background errors stay quiet"
        );
    });
}

#[gpui::test]
fn failed_actions_show_their_details_every_time_they_fail(cx: &mut TestAppContext) {
    use muxy_ui::toast::ToastKind;
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for _ in 0..2 {
        view.update(cx, |model, cx| {
            model.set_configuration_error(None);
            model.fail_detail(
                "Committed abc1234 on feature, but couldn't push".into(),
                "network down. The commit is saved locally.",
                cx,
            );
        });
        cx.run_until_parked();
        view.update(cx, |model, cx| {
            let toast = model.problem_toast.as_ref().expect("failure shown");
            assert_eq!(toast.kind, ToastKind::Error);
            assert_eq!(
                toast.body.as_deref(),
                Some("network down. The commit is saved locally.")
            );
            model.dismiss_problem(cx);
        });
        cx.executor()
            .advance_clock(crate::model::banners::TOAST_TRANSITION);
        cx.run_until_parked();
    }
    view.update(cx, |model, cx| {
        model.set_configuration_error(Some("Unknown setting".into()));
        cx.notify();
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.problem_toast.as_ref().map(|toast| toast.kind),
            Some(ToastKind::Warning)
        );
    });
}

#[gpui::test]
fn toast_body_is_bounded_and_clears_for_a_title_only_notice(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(600.0), px(400.0)));
    view.update(cx, |model, cx| {
        model.set_configuration_error(None);
        model.show_toast(
            "Extension message".into(),
            Some("A long line of body text\n".repeat(100)),
            cx,
        );
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(crate::model::banners::TOAST_TRANSITION);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let body = cx.debug_bounds("workspace-toast-body").expect("body");
    let toast = cx.debug_bounds("workspace-toast").expect("toast");
    assert!(body.size.height <= px(28.0));
    assert!(toast.size.height <= px(64.0));
    assert!(toast.size.width <= px(420.0));
    view.update(cx, |model, cx| model.show_notice("Saved".into(), cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.notice_toast.as_ref().expect("toast").body.is_none());
    });
}
