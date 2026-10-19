use super::extensions::finish_extension;
use super::*;
use crate::views::settings::Change;
use muxy_app_core::composer::submission::ImageSubmissionStrategy;
use muxy_app_core::settings::Settings;
use muxy_app_core::webview::WebviewDescriptor;
use muxy_core::capability::{Capability, Feature};
use muxy_core::shortcuts::ShortcutId;

const INFO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.example.muxy-german.localization</string>
    <key>CFBundleDevelopmentRegion</key>
    <string>de</string>
</dict>
</plist>"#;

fn without(model: &mut AppModel, features: &[Feature]) {
    for feature in features {
        model.capabilities.set(*feature, Capability::Unsupported);
    }
}

fn reason(feature: Feature) -> String {
    feature.unsupported_reason().into_owned()
}

#[gpui::test]
fn commands_without_a_backend_report_why_and_start_nothing(cx: &mut TestAppContext) {
    let (mut boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.quick_terminal.enabled = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        model.new_tab(cx);
        without(
            model,
            &[
                Feature::Voice,
                Feature::QuickTerminal,
                Feature::CommandLineTool,
                Feature::ExtensionWebviews,
            ],
        );
        let tabs = model.state.home().tabs.len();
        let _ = requests.try_iter().count();

        model.toggle_voice(window, cx);
        assert_eq!(model.error, Some(reason(Feature::Voice)));

        model.toggle_quick_terminal(cx);
        assert!(model.quick.panel.is_none());
        assert!(!model.quick.visible);
        assert!(model.state.quick_terminal().is_none());
        assert_eq!(model.error, Some(reason(Feature::QuickTerminal)));

        model.install_command_line(cx);
        assert_eq!(model.error, Some(reason(Feature::CommandLineTool)));

        assert_eq!(
            model
                .open_webview_tab(
                    WebviewDescriptor {
                        owner: "viewer".into(),
                        kind: "page".into(),
                        data: serde_json::Value::Null,
                    },
                    false,
                    cx,
                )
                .err(),
            Some(reason(Feature::ExtensionWebviews))
        );
        assert_eq!(model.state.home().tabs.len(), tabs);
        assert_eq!(requests.try_iter().count(), 0);
    });
}

#[gpui::test]
fn browser_tabs_and_remote_servers_without_a_backend_report_why(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        without(model, &[Feature::BrowserPanes, Feature::SshRemotes]);
        let tabs = model.state.home().tabs.len();
        model.new_browser_tab(cx);
        assert_eq!(model.error, Some(reason(Feature::BrowserPanes)));
        assert_eq!(model.state.home().tabs.len(), tabs);
        assert!(!model.browser_enabled(), "browser commands are hidden");
        let form = DeviceForm {
            name: "box".into(),
            host: "box.local".into(),
            ..Default::default()
        };
        assert_eq!(
            model.save_remote_server(None, &form, cx),
            Err(reason(Feature::SshRemotes))
        );
        assert!(model.settings.servers.is_empty());
    });
}

/// What Windows builds lack, and why: everything outside the desktop MVP.
#[cfg(windows)]
#[test]
fn windows_reports_why_each_missing_feature_is_unavailable() {
    let backends = crate::model::capabilities::backends();
    for feature in [
        Feature::ExtensionScripts,
        Feature::ExtensionWebviews,
        Feature::Voice,
        Feature::QuickTerminal,
        Feature::ClipboardImages,
        Feature::CommandLineTool,
        Feature::Updates,
        Feature::AppHiding,
        Feature::BrowserPanes,
        Feature::SshRemotes,
    ] {
        assert_eq!(
            backends.unsupported(feature),
            Some(feature.unsupported_reason())
        );
    }
    assert!(backends.has_backend(Feature::Notifications));
}

#[gpui::test]
fn shortcuts_for_features_without_a_backend_report_why(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.new_tab(cx);
        without(model, &[Feature::Voice, Feature::AppHiding]);
    });
    for (shortcut, feature) in [
        (ShortcutId::ToggleVoiceRecording, Feature::Voice),
        (ShortcutId::HideApp, Feature::AppHiding),
        (ShortcutId::HideOthers, Feature::AppHiding),
    ] {
        view.update(cx, |model, _| model.error = None);
        cx.simulate_keystrokes(shortcut.definition().keys[0]);
        view.update(cx, |model, _| {
            assert!(model.voice.view.is_none());
            assert_eq!(model.error, Some(reason(feature)));
        });
    }
}

#[gpui::test]
fn composer_dictation_without_a_backend_reports_why_and_starts_no_recorder(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        without(model, &[Feature::Voice]);
        model.toggle_composer(window, cx);
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, cx| {
            composer.toggle_voice(cx);
            assert!(!composer.recording());
            assert_eq!(composer.error, Some(reason(Feature::Voice)));
        });
    });
}

#[gpui::test]
fn preferences_for_features_without_a_backend_survive_use_and_edits(cx: &mut TestAppContext) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let path = boot.state_path.with_file_name("settings.toml");
    boot.settings.composer.image_strategy = ImageSubmissionStrategy::Clipboard;
    boot.settings.composer.voice_auto_send = true;
    boot.settings.composer.language = "fr-FR".into();
    boot.settings.quick_terminal.enabled = true;
    boot.settings.quick_terminal.width = 901;
    std::fs::create_dir_all(path.parent().expect("profile")).expect("profile");
    boot.settings.save_composer(&path).expect("composer");
    boot.settings
        .quick_terminal
        .save(&path)
        .expect("quick terminal");
    let imported = Settings::load(&path).expect("imported settings");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        without(
            model,
            &[
                Feature::ClipboardImages,
                Feature::Voice,
                Feature::QuickTerminal,
            ],
        );
        model.toggle_voice(window, cx);
        model.toggle_quick_terminal(cx);
        model.change_preference(Change::Composer("composer-clear-after", true), cx);

        let saved = Settings::load(&path).expect("saved settings");
        assert!(saved.composer.clear_after_sending);
        assert_eq!(
            saved.composer.image_strategy,
            ImageSubmissionStrategy::Clipboard
        );
        assert!(saved.composer.voice_auto_send);
        assert_eq!(saved.composer.language, "fr-FR");
        assert_eq!(saved.quick_terminal, imported.quick_terminal);
        assert_eq!(model.settings.quick_terminal, imported.quick_terminal);
    });
}

#[gpui::test]
fn language_packs_load_while_extension_code_cannot_run(cx: &mut TestAppContext) {
    let package = tempfile::tempdir().expect("extension folder");
    for (file, contents) in [
        (
            "package.json",
            r#"{"name":"german","version":"1.0.0","muxy":{
                "background":"background.js",
                "commands":[{"id":"hello","title":"Hello"}],
                "localizations":[{"id":"de","language":"de","title":"Deutsch","bundle":"German.bundle"}]
            }}"#,
        ),
        ("background.js", "globalThis.started = true;"),
        ("German.bundle/Info.plist", INFO),
        (
            "German.bundle/de.lproj/Localizable.strings",
            "\"Settings\" = \"Einstellungen\";",
        ),
    ] {
        let file = package.path().join(file);
        std::fs::create_dir_all(file.parent().expect("folder")).expect("folder");
        std::fs::write(file, contents).expect("package file");
    }
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        without(
            model,
            &[Feature::ExtensionScripts, Feature::ExtensionWebviews],
        );
    });
    finish_extension(
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        cx,
    )
    .expect("load");
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("german", true, cx)
        }),
        cx,
    )
    .expect("enable");
    cx.run_until_parked();
    view.update_in(cx, |model, window, cx| {
        assert!(!model.background_running("german"));
        model.run_extension_command("german", "hello", window, cx);
        assert_eq!(model.error, Some(reason(Feature::ExtensionScripts)));
        assert_eq!(
            model
                .app_languages()
                .into_iter()
                .map(|(selection, _)| selection)
                .collect::<Vec<_>>(),
            vec!["german:de".to_owned()]
        );
    });
}
