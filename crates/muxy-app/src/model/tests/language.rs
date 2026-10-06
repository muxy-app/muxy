use super::extensions::finish_extension;
use super::*;
use crate::views::settings::Change;

const INFO: &str =
    "<plist><dict><key>CFBundleIdentifier</key><string>pack.de</string></dict></plist>";

fn language_pack() -> tempfile::TempDir {
    let package = tempfile::tempdir().expect("extension folder");
    let catalog = package.path().join("German.bundle/de.lproj");
    std::fs::create_dir_all(&catalog).expect("bundle");
    std::fs::write(
        package.path().join("package.json"),
        r#"{"name":"pack","version":"1.0.0","muxy":{"localizations":[
            {"id":"de","language":"de","title":"Deutsch","bundle":"German.bundle"}
        ]}}"#,
    )
    .expect("manifest");
    std::fs::write(package.path().join("German.bundle/Info.plist"), INFO).expect("info");
    std::fs::write(
        catalog.join("Localizable.strings"),
        "\"Close Tab\" = \"Tab schließen\";\n\"%lld changes\" = \"%lld Änderungen\";\n",
    )
    .expect("catalog");
    package
}

fn set_enabled(view: &Entity<AppModel>, enabled: bool, cx: &mut VisualTestContext) {
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("pack", enabled, cx)
        }),
        cx,
    )
    .expect("enable or disable");
    cx.run_until_parked();
}

#[gpui::test]
fn a_chosen_language_pack_translates_the_app_and_english_shows_while_it_is_unavailable(
    cx: &mut TestAppContext,
) {
    let package = language_pack();
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    finish_extension(
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        cx,
    )
    .expect("load");
    view.update(cx, |model, _| assert!(model.app_languages().is_empty()));
    set_enabled(&view, true, cx);
    view.update(cx, |model, cx| {
        assert_eq!(
            model.app_languages(),
            vec![("pack:de".to_owned(), "Deutsch — pack".to_owned())]
        );
        model.change_preference(Change::Language("pack:de".into()), cx);
    });
    cx.run_until_parked();
    assert_eq!(muxy_ui::tr!("Close Tab"), "Tab schließen");
    assert_eq!(muxy_ui::tr!("%lld changes", 3), "3 Änderungen");
    assert_eq!(muxy_ui::tr!("Close Pane"), "Close Pane");

    set_enabled(&view, false, cx);
    assert_eq!(muxy_ui::tr!("Close Tab"), "Close Tab");
    view.update(cx, |model, _| {
        assert_eq!(model.appearance.language, "pack:de");
    });

    set_enabled(&view, true, cx);
    assert_eq!(muxy_ui::tr!("Close Tab"), "Tab schließen");

    view.update(cx, |model, cx| {
        model.change_preference(Change::Language(String::new()), cx);
    });
    cx.run_until_parked();
    assert_eq!(muxy_ui::tr!("Close Tab"), "Close Tab");
}

#[gpui::test]
fn the_language_dropdown_lists_packs_keeps_an_unavailable_choice_and_browses_language_packs(
    cx: &mut TestAppContext,
) {
    use super::preferences::{click_preference, settings_window};
    let package = language_pack();
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.appearance.language = "gone:fr".into();
    let (view, cx) = settings_window(boot, cx);
    cx.simulate_resize(size(px(1200.0), px(900.0)));
    finish_extension(
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        cx,
    )
    .expect("load");
    set_enabled(&view, true, cx);
    click_preference(cx, "settings-category-Appearance");
    click_preference(cx, "settings-picker-app-language");
    for row in ["picker-row-", "picker-row-pack:de", "picker-row-gone:fr"] {
        assert!(cx.debug_bounds(row).is_some(), "{row} is missing");
    }
    click_preference(cx, "picker-row-gone:fr");
    view.read_with(cx, |model, _| {
        assert_eq!(model.appearance.language, "gone:fr");
    });
    click_preference(cx, "picker-row-pack:de");
    view.read_with(cx, |model, _| {
        assert_eq!(model.appearance.language, "pack:de");
        let saved =
            muxy_app_core::settings::Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("saved settings");
        assert_eq!(saved.appearance.language, "pack:de");
    });
    assert_eq!(muxy_ui::tr!("Close Tab"), "Tab schließen");

    click_preference(cx, "settings-browse-languages");
    assert!(cx.debug_bounds("extension-languages-only").is_some());
}
