use super::*;
use muxy_app_core::settings::Settings;

#[gpui::test]
fn later_appearance_edits_preserve_another_instances_saved_sidebar_choice(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _requests) = stub_boot(state.clone());
    let path = boot.state_path.clone();
    boot.settings
        .appearance
        .save(&path.with_file_name("settings.toml"))
        .expect("initial appearance");
    let (mut other_boot, _requests) = stub_boot(state);
    other_boot.state_path = path.clone();
    other_boot.settings =
        Settings::load(&path.with_file_name("settings.toml")).expect("initial settings");
    let (first, first_window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let (second, second_window) =
        first_window.add_window_view(|window, cx| AppModel::new(other_boot, window, cx));
    first.update(second_window, |model, cx| {
        model.appearance.sidebar_expanded = !model.appearance.sidebar_expanded;
        model.save_appearance(cx);
    });
    second.update(second_window, |model, cx| {
        model.change_preference(crate::views::settings::Change::StatusBar(false), cx);
    });
    let saved = Settings::load(&path.with_file_name("settings.toml")).expect("saved settings");
    assert!(saved.appearance.sidebar_expanded);
    assert!(!saved.appearance.status_bar_visible);
    first.update(second_window, |model, cx| {
        model.appearance.sidebar_expanded = !model.appearance.sidebar_expanded;
        model.save_appearance(cx);
    });
    second.update(second_window, |model, cx| {
        model.appearance.dark_theme = "Dracula".into();
        model.save_appearance(cx);
    });
    let saved = Settings::load(&path.with_file_name("settings.toml")).expect("restart settings");
    assert!(!saved.appearance.sidebar_expanded);
    assert_eq!(saved.appearance.dark_theme, "Dracula");
    assert!(!saved.appearance.status_bar_visible);
}
