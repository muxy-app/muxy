use super::*;
use muxy_app_core::settings::Settings;

#[gpui::test]
fn sidebar_vibrancy_controls_apply_save_and_validate(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = settings_window(boot, cx);
    click_preference(cx, "settings-search");
    cx.simulate_keystrokes("s i d e b a r space v i b r a n c y");
    cx.run_until_parked();
    click_preference(cx, "settings-toggle-sidebar-vibrancy");
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.sidebar_vibrancy);
        assert!(!model.appearance.sidebar_expanded);
    });
    click_preference(cx, "settings-field-sidebar-vibrancy-level");
    cx.simulate_keystrokes("cmd-a 3 5 enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).expect("settings");
        assert!(!saved.appearance.sidebar_vibrancy);
        assert_eq!(saved.appearance.sidebar_vibrancy_level, 35);
        assert_eq!(model.appearance.sidebar_vibrancy_level, 35);
    });
    for invalid in ["-1", "101", "256", "NaN", "12.5"] {
        view.update(cx, |model, cx| {
            model.change_preference(Change::Field("sidebar-vibrancy-level", invalid.into()), cx);
            assert_eq!(model.appearance.sidebar_vibrancy_level, 35);
            assert!(
                settings_view(model)
                    .read(cx)
                    .errors
                    .contains_key("sidebar-vibrancy-level")
            );
        });
    }
    for level in [0, 100] {
        view.update(cx, |model, cx| {
            model.change_preference(
                Change::Field("sidebar-vibrancy-level", level.to_string()),
                cx,
            );
            assert_eq!(model.appearance.sidebar_vibrancy_level, level);
            assert!(
                !settings_view(model)
                    .read(cx)
                    .errors
                    .contains_key("sidebar-vibrancy-level")
            );
        });
    }
    click_preference(cx, "settings-toggle-sidebar-vibrancy");
    view.update(cx, |model, cx| {
        assert!(model.appearance.sidebar_vibrancy);
        let path = model.path.clone();
        let blocked = path.with_file_name("vibrancy-blocked");
        std::fs::write(&blocked, "file").expect("blocked directory");
        model.path = blocked.join("state.json");
        model.change_preference(Change::SidebarVibrancy(false), cx);
        assert!(model.appearance.sidebar_vibrancy);
        assert!(
            settings_view(model)
                .read(cx)
                .errors
                .contains_key("sidebar-vibrancy")
        );
        model.path = path;
        model.reload_configuration(cx);
        assert!(model.appearance.sidebar_vibrancy);
        assert_eq!(model.appearance.sidebar_vibrancy_level, 100);
    });
}
