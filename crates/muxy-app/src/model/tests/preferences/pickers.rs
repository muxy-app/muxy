use super::*;
use crate::views::settings::window::SettingsOverlay as Overlay;

fn load_languages(view: &Entity<AppModel>, cx: &mut VisualTestContext) {
    let picker = view.read_with(cx, |model, cx| {
        let Some(Overlay::Languages { picker, .. }) = &settings_root(model, cx).overlay else {
            panic!("language dropdown missing");
        };
        picker.clone()
    });
    picker.update(cx, |picker, cx| {
        picker.set_languages(
            vec![
                muxy_ui::voice::Language {
                    id: "en-US".into(),
                    name: "English (United States)".into(),
                    preferred: true,
                },
                muxy_ui::voice::Language {
                    id: "fr-FR".into(),
                    name: "French (France)".into(),
                    preferred: false,
                },
            ],
            cx,
        );
    });
    cx.run_until_parked();
}

#[gpui::test]
fn language_dropdown_stays_in_settings_filters_saves_and_cancels(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    click_preference(cx, "settings-category-Composer");
    click_preference(cx, "settings-disclosure-Composer");
    click_preference(cx, "settings-subcategory-Voice");
    click_preference(cx, "settings-picker-composer-language");
    cx.simulate_input("fr");
    load_languages(&view, cx);
    let trigger = cx
        .debug_bounds("settings-picker-composer-language")
        .expect("language trigger");
    let dropdown = cx
        .debug_bounds("language-browser")
        .expect("language dropdown");
    assert!(dropdown.top() >= trigger.bottom() || dropdown.bottom() <= trigger.top());
    assert!(dropdown.left() <= trigger.right() && dropdown.right() >= trigger.left());
    assert_eq!(dropdown.size.width, Metrics::new(1.15).scaled(260.0));
    assert!(
        (cx.debug_bounds("picker-row-fr-FR")
            .expect("language row")
            .size
            .height
            - Metrics::new(1.15).scaled(24.0))
        .abs()
            <= px(0.5)
    );
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(model.overlay.is_none());
        assert!(settings_root(model, cx).overlay.is_none());
        assert_eq!(model.settings.composer.language, "fr-FR");
        assert_eq!(settings_view(model).read(cx).dictation_language(), "fr-FR");
        let saved =
            muxy_app_core::settings::Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("saved settings");
        assert_eq!(saved.composer.language, "fr-FR");
    });
    let settings = view.read_with(cx, |model, _| settings_view(model));
    cx.update(|window, cx| assert!(settings.read(cx).focus.is_focused(window)));
    click_preference(cx, "settings-picker-composer-language");
    load_languages(&view, cx);
    cx.simulate_keystrokes("z z z enter");
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_some());
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
        assert_eq!(model.settings.composer.language, "fr-FR");
    });
    click_preference(cx, "settings-picker-composer-language");
    load_languages(&view, cx);
    cx.simulate_input("system");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
        assert!(model.settings.composer.language.is_empty());
    });
}

#[gpui::test]
fn language_dropdown_reports_failed_saves_without_changing_the_preference(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    click_preference(cx, "settings-category-Composer");
    click_preference(cx, "settings-disclosure-Composer");
    click_preference(cx, "settings-subcategory-Voice");
    click_preference(cx, "settings-picker-composer-language");
    load_languages(&view, cx);
    view.read_with(cx, |model, _| {
        let path = model.path.with_file_name("settings.toml");
        if path.is_file() {
            std::fs::remove_file(&path).expect("remove fixture settings");
        }
        std::fs::create_dir(&path).expect("block settings path");
    });
    cx.simulate_input("French");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(model.settings.composer.language.is_empty());
        assert!(
            settings_view(model)
                .read(cx)
                .errors
                .contains_key("composer-language")
        );
    });
}

#[gpui::test]
fn font_dropdown_filters_saves_and_cancels_without_expanding_the_settings_rows(
    cx: &mut TestAppContext,
) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    click_preference(cx, "settings-category-Terminal");
    let section = cx
        .debug_bounds("settings-section-Terminal")
        .expect("terminal section");
    let settings = view.read_with(cx, |model, _| settings_view(model));
    click_preference(cx, "settings-field-font-size");
    cx.simulate_keystrokes("cmd-a 2 1");
    settings.read_with(cx, |pane, cx| {
        assert_eq!(pane.field_value("font-size", cx), "21");
    });
    click_preference(cx, "settings-picker-font-family");
    view.read_with(cx, |model, cx| {
        assert!(matches!(
            settings_root(model, cx).overlay,
            Some(Overlay::Fonts { .. })
        ));
        assert_eq!(model.terminal.font_size, 21.0);
    });
    assert_eq!(cx.debug_bounds("settings-section-Terminal"), Some(section));
    let dropdown = cx.debug_bounds("font-browser").expect("font dropdown");
    assert_eq!(dropdown.size.width, Metrics::new(1.15).scaled(260.0));
    assert!(dropdown.size.height <= Metrics::new(1.15).scaled(260.0));
    assert!(
        (cx.debug_bounds("picker-row-font-0")
            .expect("font row")
            .size
            .height
            - Metrics::new(1.15).scaled(24.0))
        .abs()
            <= px(0.5)
    );
    let font = cx
        .text_system()
        .all_font_names()
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .max_by_key(String::len)
        .expect("available font");
    cx.simulate_input(&font);
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("settings-section-Terminal"), Some(section));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
        assert_eq!(
            model.terminal.font_families.first().map(String::as_str),
            Some(font.as_str())
        );
        let saved = muxy_app_core::settings::TerminalSettings::load_with_seed(
            &model.path.with_file_name("ghostty.conf"),
            None,
        )
        .expect("saved font");
        assert_eq!(saved.font_families, model.terminal.font_families);
    });
    cx.update(|window, cx| assert!(settings.read(cx).focus.is_focused(window)));
    click_preference(cx, "settings-picker-font-family");
    cx.simulate_keystrokes("z z z z z z enter");
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_some());
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
        assert_eq!(
            model.terminal.font_families.first().map(String::as_str),
            Some(font.as_str())
        );
    });
    let mut names = cx.text_system().all_font_names();
    names.retain(|name| !name.starts_with('.'));
    names.sort_unstable_by_key(|name| name.to_lowercase());
    names.dedup();
    click_preference(cx, "settings-picker-font-family");
    cx.simulate_keystrokes("down down up enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
        assert_eq!(model.terminal.font_families, [names[1].clone()]);
    });
}

#[gpui::test]
fn settings_theme_dropdowns_are_compact_anchored_and_update_only_the_selected_mode(
    cx: &mut TestAppContext,
) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _requests) = stub_boot(state);
    let themes = boot.state_path.with_file_name("themes");
    std::fs::create_dir_all(&themes).expect("themes directory");
    std::fs::write(
        themes.join("Picker Fixture.conf"),
        "background = 123456\nforeground = abcdef\n",
    )
    .expect("test theme");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    click_preference(cx, "settings-category-Appearance");
    for (dark, selector) in [
        (true, "settings-picker-dark-theme"),
        (false, "settings-picker-light-theme"),
    ] {
        let before = view.read_with(cx, |model, _| model.appearance.clone());
        click_preference(cx, selector);
        let palette = cx.debug_bounds("command-palette").expect("theme palette");
        let trigger = cx.debug_bounds(selector).expect("theme trigger");
        assert!(palette.top() >= trigger.bottom() || palette.bottom() <= trigger.top());
        assert!(palette.left() <= trigger.right() && palette.right() >= trigger.left());
        assert_eq!(palette.size.width, Metrics::new(1.15).scaled(260.0));
        assert!(palette.size.height <= Metrics::new(1.15).scaled(260.0));
        assert!(cx.debug_bounds("picker-back").is_none());
        assert!(cx.debug_bounds("settings-dropdown").is_some());
        view.read_with(cx, |model, cx| {
            let Some(Overlay::Themes { source, .. }) = &settings_root(model, cx).overlay else {
                panic!("settings theme picker")
            };
            assert_eq!(source.kind, crate::views::settings::PickerKind::Theme(dark));
        });
        cx.simulate_keystrokes("p i c k e r space f i x t u r e enter");
        cx.run_until_parked();
        view.read_with(cx, |model, cx| {
            assert!(settings_root(model, cx).overlay.is_some());
            assert_eq!(
                if dark {
                    &model.appearance.dark_theme
                } else {
                    &model.appearance.light_theme
                },
                "Picker Fixture"
            );
            if dark {
                assert_eq!(model.appearance.light_theme, before.light_theme);
            } else {
                assert_eq!(model.appearance.dark_theme, before.dark_theme);
            }
        });
        assert!(cx.debug_bounds("picker-title-Picker Fixture").is_some());
        cx.simulate_keystrokes("cmd-a m u x y enter");
        cx.run_until_parked();
        view.read_with(cx, |model, cx| {
            assert!(settings_root(model, cx).overlay.is_some());
            assert_eq!(model.themes.active_name(&model.appearance, dark), "Muxy");
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        view.read_with(cx, |model, cx| {
            assert!(settings_root(model, cx).overlay.is_none());
        });
    }
}

#[gpui::test]
fn theme_palettes_sync_current_theme_between_windows_and_when_reentering_a_page(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    click_preference(cx, "settings-category-Appearance");
    let (workspace, dark) = view.read_with(cx, |model, _| (model.window, model.dark));
    click_preference(
        cx,
        if dark {
            "settings-picker-dark-theme"
        } else {
            "settings-picker-light-theme"
        },
    );
    let settings_palette = view.read_with(cx, |model, cx| {
        let Some(Overlay::Themes { picker, .. }) = &settings_root(model, cx).overlay else {
            panic!("settings theme palette")
        };
        picker.clone()
    });
    {
        let main = VisualTestContext::from_window(workspace, cx).into_mut();
        main.simulate_keystrokes("cmd-shift-p");
        main.simulate_input("change theme");
        main.simulate_keystrokes("enter");
        main.simulate_input("Dracula");
        main.simulate_keystrokes("enter");
        main.run_until_parked();
    }
    cx.run_until_parked();
    settings_palette.read_with(cx, |palette, _| {
        assert_eq!(palette.current_id(), Some("Dracula"));
    });
    cx.simulate_input("Gruvbox Dark");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let Some(crate::views::overlays::Overlay::Commands { palette, .. }) = &model.overlay else {
            panic!("workspace theme palette")
        };
        assert_eq!(palette.read(cx).current_id(), Some("Gruvbox Dark"));
    });
    {
        let main = VisualTestContext::from_window(workspace, cx).into_mut();
        main.simulate_keystrokes("escape");
        main.run_until_parked();
    }
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("Atom One Dark");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    {
        let main = VisualTestContext::from_window(workspace, cx).into_mut();
        main.simulate_keystrokes("enter");
        main.run_until_parked();
    }
    view.read_with(cx, |model, cx| {
        let Some(crate::views::overlays::Overlay::Commands { palette, .. }) = &model.overlay else {
            panic!("workspace theme palette")
        };
        assert_eq!(palette.read(cx).current_id(), Some("Atom One Dark"));
        assert_eq!(
            settings_palette.read(cx).current_id(),
            Some("Atom One Dark")
        );
    });
}

#[gpui::test]
fn settings_dropdowns_follow_their_trigger_on_resize_and_fit_the_window(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = settings_window(boot, cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    click_preference(cx, "settings-category-Terminal");
    click_preference(cx, "settings-picker-font-family");
    let settings = view.read_with(cx, |model, _| settings_view(model));
    for (width, height) in [
        (1200.0, 800.0),
        (650.0, 650.0),
        (460.0, 500.0),
        (1400.0, 900.0),
    ] {
        let renders = settings.read_with(cx, |pane, _| pane.render_count);
        cx.simulate_resize(size(px(width), px(height)));
        cx.run_until_parked();
        settings.read_with(cx, |pane, _| assert_eq!(pane.render_count - renders, 1));
        let trigger = cx
            .debug_bounds("settings-picker-font-family")
            .expect("font trigger");
        let dropdown = cx.debug_bounds("settings-dropdown").expect("font dropdown");
        view.read_with(cx, |model, cx| {
            let source = settings_root(model, cx)
                .overlay
                .as_ref()
                .map(Overlay::source)
                .expect("open dropdown");
            assert_eq!(source.anchor.get(), Some(trigger));
        });
        assert!(dropdown.left() >= px(0.0) && dropdown.right() <= px(width));
        assert!(dropdown.top() >= px(0.0) && dropdown.bottom() <= px(height));
        assert!(dropdown.left() <= trigger.right() && dropdown.right() >= trigger.left());
    }
    view.update(cx, AppModel::new_tab);
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_some());
    });
}

#[gpui::test]
fn settings_pickers_leave_the_workspace_focus_and_overlays_untouched(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("terminal");
    let (boot, _) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    let before = view.read_with(cx, |model, _| model.state.clone());
    click_preference(cx, "settings-category-Appearance");
    click_preference(cx, "settings-picker-dark-theme");
    view.read_with(cx, |model, cx| {
        assert!(model.overlay.is_none());
        assert!(settings_root(model, cx).overlay.is_some());
        assert_eq!(model.state, before);
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let settings = view.read_with(cx, |model, _| settings_view(model));
    cx.update(|window, cx| assert!(settings.read(cx).focus.is_focused(window)));
}

#[gpui::test]
fn font_dropdown_mouse_selection_reports_save_failure_and_outside_click_does_not_reopen(
    cx: &mut TestAppContext,
) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _requests) = stub_boot(state);
    let config = boot.state_path.with_file_name("ghostty.conf");
    std::fs::create_dir_all(&config).expect("blocked config path");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    click_preference(cx, "settings-category-Terminal");
    let settings = view.read_with(cx, |model, _| settings_view(model));
    let previous = view.read_with(cx, |model, _| model.terminal.font_families.clone());
    let font = cx
        .text_system()
        .all_font_names()
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .max_by_key(String::len)
        .expect("available font");
    click_preference(cx, "settings-picker-font-family");
    cx.simulate_input(&font);
    cx.run_until_parked();
    let dropdown = cx.debug_bounds("settings-dropdown").expect("font dropdown");
    cx.simulate_click(
        gpui::point(dropdown.center().x, dropdown.bottom() - px(16.0)),
        Modifiers::default(),
    );
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
        assert_eq!(model.terminal.font_families, previous);
    });
    settings.read_with(cx, |pane, _| {
        assert!(pane.errors.contains_key("font-family"));
    });
    std::fs::remove_dir(&config).expect("unblock config path");
    click_preference(cx, "settings-picker-font-family");
    cx.simulate_input(&font);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
        assert_eq!(model.terminal.font_families, std::slice::from_ref(&font));
    });
    settings.read_with(cx, |pane, _| {
        assert!(!pane.errors.contains_key("font-family"));
    });
    click_preference(cx, "settings-picker-font-family");
    click_preference(cx, "settings-picker-font-family");
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
    });
    cx.update(|window, cx| assert!(settings.read(cx).focus.is_focused(window)));
}

#[gpui::test]
fn search_result_dropdown_uses_the_visible_field_and_closes_when_scrolled_out(
    cx: &mut TestAppContext,
) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    cx.simulate_resize(size(px(1200.0), px(1200.0)));
    click_preference(cx, "settings-search");
    cx.simulate_input("t");
    let settings = view.read_with(cx, |model, _| settings_view(model));
    settings.update(cx, |pane, cx| {
        pane.results_state().scroll_to(gpui::ListOffset {
            item_ix: 4,
            offset_in_item: px(0.0),
        });
        cx.notify();
    });
    click_preference(cx, "settings-picker-font-family");
    let trigger = cx
        .debug_bounds("settings-picker-font-family")
        .expect("font trigger");
    view.read_with(cx, |model, cx| {
        let source = settings_root(model, cx)
            .overlay
            .as_ref()
            .map(Overlay::source)
            .expect("source");
        assert_eq!(source.anchor.get(), Some(trigger));
    });
    let settings = view.read_with(cx, |model, _| settings_view(model));
    settings.update(cx, |pane, cx| {
        let list = pane.results_state();
        list.scroll_to(gpui::ListOffset {
            item_ix: list.item_count() - 2,
            offset_in_item: px(0.0),
        });
        cx.notify();
    });
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(settings_root(model, cx).overlay.is_none());
    });
    cx.update(|window, cx| assert!(settings.read(cx).focus.is_focused(window)));
    click_preference(cx, "settings-search");
    cx.simulate_keystrokes("cmd-a f o n t");
    cx.simulate_resize(size(px(650.0), px(650.0)));
    settings.update(cx, |pane, cx| {
        pane.results_state().scroll_to(gpui::ListOffset {
            item_ix: 1,
            offset_in_item: px(0.0),
        });
        cx.notify();
    });
    click_preference(cx, "settings-picker-font-family");
    let trigger = cx
        .debug_bounds("settings-picker-font-family")
        .expect("font trigger");
    let dropdown = cx.debug_bounds("settings-dropdown").expect("font dropdown");
    assert!(dropdown.top() >= trigger.bottom() || dropdown.bottom() <= trigger.top());
    assert_eq!(dropdown.left(), trigger.left());
}
