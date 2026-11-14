use super::*;
use muxy_app_core::opener::{FileLocation, FileRule, OpenRequest, Target};
use muxy_app_core::settings::Settings;
use muxy_ui::picker::{PickerEvent, PickerSelection};

fn install(model: &mut AppModel, cx: &mut Context<AppModel>) -> OpenRequest {
    for name in ["preview", "files"] {
        let folder = model.path.with_file_name(format!("fixture-{name}"));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("index.html"),
            "<!doctype html><title>Viewer</title>",
        )
        .unwrap();
        std::fs::write(
            folder.join("package.json"),
            serde_json::json!({
                "name": name, "version": "1.0.0", "muxy": {
                    "tabTypes": [{"id": "viewer", "title": "Viewer", "entry": "index.html"}],
                "fileOpeners": [{"id": "open", "tabType": "viewer", "patterns": ["*"], "fileTypes": ["*.md"]}]
                }
            })
            .to_string(),
        )
        .unwrap();
        model.extensions.registry.load_unpacked(&folder).unwrap();
        model.extensions.registry.set_enabled(name, true).unwrap();
    }
    model.register_extension_surfaces(cx);
    model.settings.openers.project_target = Some("com.microsoft.VSCode".into());
    model.settings.openers.file = "system.editor".into();
    let pane = model.state.home().tabs[0].panes[0].id;
    let context = model.opener_context(pane).unwrap();
    OpenRequest {
        target: Target::File(FileLocation {
            path: context.project_directory.join("README.md"),
            line: Some(12),
            column: Some(3),
        }),
        context,
    }
}

fn chooser(model: &Entity<AppModel>, cx: &VisualTestContext) -> Entity<muxy_ui::picker::Picker> {
    model.read_with(cx, |model, _| {
        let Some(Overlay::FileOpener(open)) = &model.overlay else {
            panic!("file chooser")
        };
        open.picker.clone()
    })
}

#[gpui::test]
fn file_chooser_search_cancel_and_remember_preserve_the_source_project(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().unwrap();
    state.open_terminal_tab(state.home().id).unwrap();
    let (boot, _) = stub_boot(state);
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let request = model.update(cx, install);
    model.update(cx, |model, cx| {
        model.route_terminal_file(request.clone(), Some("README.md".into()), cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("file-opener").is_some());
    assert!(cx.debug_bounds("picker-row-preview:open").is_some());
    let picker = chooser(&model, cx);
    cx.simulate_input("preview");
    cx.run_until_parked();
    assert!(cx.debug_bounds("picker-row-preview:open").is_some());
    picker.read_with(cx, |picker, _| {
        assert_eq!(picker.query(), "preview", "search input");
        assert_eq!(
            picker.items().len(),
            1,
            "search results: {:?}",
            picker.items()
        );
    });
    assert!(
        picker
            .update(cx, |picker, cx| picker.select_row("files:open", cx))
            .is_err()
    );
    picker.update(cx, |_, cx| cx.emit(PickerEvent::Dismissed));
    model.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.home().tabs.len(), 1);
        assert!(model.settings.openers.file_rules.is_empty());
    });
    model.update(cx, |model, cx| {
        model.route_terminal_file(request.clone(), Some("README.md".into()), cx);
    });
    let picker = chooser(&model, cx);
    picker.update(cx, |_, cx| {
        cx.emit(PickerEvent::FooterAction("remember".into()));
    });
    model.update(cx, |model, _| {
        let other = model
            .state
            .add_project(ServerId::local(), crate::test_support::project_directory())
            .unwrap();
        model.state.select_project(other).unwrap();
    });
    picker.update(cx, |_, cx| {
        cx.emit(PickerEvent::Confirmed(PickerSelection::new("preview:open")));
    });
    model.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        let home = model.state.project(request.context.project).unwrap();
        let PaneContent::Webview(page) = &home.tabs.last().unwrap().panes[0].content else {
            panic!("extension tab")
        };
        assert_eq!(page.owner, "preview");
        assert_eq!(page.data["filePath"], "README.md");
        assert_eq!(page.data["line"], 12);
        assert_eq!(page.data["column"], 3);
        assert_eq!(
            model.settings.openers.file_rules[0],
            FileRule {
                pattern: "*.md".into(),
                opener: "preview:open".into()
            }
        );
        let saved = Settings::load(&model.configuration_path("settings.toml")).unwrap();
        assert_eq!(saved.openers.file_rules, model.settings.openers.file_rules);
    });
    model.update(cx, |model, cx| {
        model.route_terminal_file(request.clone(), Some("README.md".into()), cx);
    });
    model.read_with(cx, |model, _| {
        assert!(
            model.overlay.is_none(),
            "saved default bypasses the chooser"
        );
        assert_eq!(model.state.home().tabs.len(), 2, "singleton tab reused");
    });
    model.update(cx, |model, cx| {
        model.settings.openers.always_ask = true;
        model.route_terminal_file(request, Some("README.md".into()), cx);
    });
    chooser(&model, cx);
}

#[gpui::test]
fn file_chooser_revalidates_disabled_extensions_and_closed_sources(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().unwrap();
    state.open_terminal_tab(state.home().id).unwrap();
    let (boot, _) = stub_boot(state);
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let request = model.update(cx, install);
    model.update(cx, |model, cx| {
        model.route_terminal_file(request.clone(), Some("README.md".into()), cx);
    });
    let picker = chooser(&model, cx);
    model.update(cx, |model, _| {
        model
            .extensions
            .registry
            .set_enabled("preview", false)
            .unwrap();
    });
    picker.update(cx, |_, cx| {
        cx.emit(PickerEvent::Confirmed(PickerSelection::new("preview:open")));
    });
    model.read_with(cx, |model, _| {
        let PaneContent::Webview(page) = &model.state.home().tabs.last().unwrap().panes[0].content
        else {
            panic!("remaining extension")
        };
        assert_eq!(page.owner, "files");
        assert!(model.settings.openers.file_rules.is_empty());
    });
    model.update(cx, |model, cx| {
        model.settings.openers.always_ask = true;
        model.route_terminal_file(request.clone(), Some("README.md".into()), cx);
    });
    let picker = chooser(&model, cx);
    model.update(cx, |model, _| {
        model.state.close_pane(request.context.pane).unwrap();
    });
    picker.update(cx, |_, cx| {
        cx.emit(PickerEvent::Confirmed(PickerSelection::new("files:open")));
    });
    model.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.settings.openers.file_rules.is_empty());
    });
}

#[gpui::test]
fn file_rule_settings_edit_custom_types_and_persist_priority(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().unwrap();
    state.open_terminal_tab(state.home().id).unwrap();
    let (boot, _) = stub_boot(state);
    let (model, cx) = preferences::settings_tab(boot, cx);
    model.update(cx, |model, cx| {
        install(model, cx);
        model.sync_preferences(cx);
    });
    preferences::click_preference(cx, "settings-search");
    cx.simulate_input("File type rules");
    preferences::click_preference(cx, "settings-add-file-rule");
    cx.simulate_input("*.custom");
    preferences::click_preference(cx, "settings-picker-file-opener");
    preferences::click_preference(cx, "picker-row-files:open");
    preferences::click_preference(cx, "settings-save-file-rule");
    model.read_with(cx, |model, _| {
        assert_eq!(
            model.settings.openers.file_rules[0],
            FileRule {
                pattern: "*.custom".into(),
                opener: "files:open".into()
            }
        );
    });
    model.update(cx, |model, cx| {
        use crate::model::preferences::Change;
        model.change_preference(
            Change::FileRule {
                previous: None,
                rule: FileRule {
                    pattern: "*".into(),
                    opener: "ask".into(),
                },
            },
            cx,
        );
        model.change_preference(Change::MoveFileRule("*.custom".into(), true), cx);
        assert_eq!(model.settings.openers.file_rules[0].pattern, "*.custom");
        model.change_preference(
            Change::FileRule {
                previous: Some("*.custom".into()),
                rule: FileRule {
                    pattern: "Dockerfile".into(),
                    opener: "files:open".into(),
                },
            },
            cx,
        );
        assert_eq!(model.settings.openers.file_rules[0].pattern, "Dockerfile");
        model.change_preference(Change::RemoveFileRule("*".into()), cx);
        assert_eq!(model.settings.openers.file_rules.len(), 1);
        assert_eq!(
            Settings::load(&model.configuration_path("settings.toml"))
                .unwrap()
                .openers
                .file_rules,
            model.settings.openers.file_rules
        );
    });
}

#[gpui::test]
fn renamed_file_rules_keep_priority_when_replacing_an_existing_pattern(cx: &mut TestAppContext) {
    use crate::model::preferences::Change;
    use muxy_app_core::opener::{FileResolution, resolve_file};

    let mut state = AppState::bootstrap().unwrap();
    state.open_terminal_tab(state.home().id).unwrap();
    let (boot, _) = stub_boot(state);
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    model.update(cx, |model, cx| {
        install(model, cx);
        model.settings.openers.file_rules = [
            ("*.txt", "preview:open"),
            ("*.md", "ask"),
            ("Dockerfile", "files:open"),
            ("*", "system.finder"),
        ]
        .map(|(pattern, opener)| FileRule {
            pattern: pattern.into(),
            opener: opener.into(),
        })
        .to_vec();
        model.change_preference(
            Change::FileRule {
                previous: Some("Dockerfile".into()),
                rule: FileRule {
                    pattern: "*.TXT".into(),
                    opener: "files:open".into(),
                },
            },
            cx,
        );
        assert_eq!(
            model
                .settings
                .openers
                .file_rules
                .iter()
                .map(|rule| rule.pattern.as_str())
                .collect::<Vec<_>>(),
            ["*.md", "*.TXT", "*"]
        );
        let saved = Settings::load(&model.configuration_path("settings.toml")).unwrap();
        assert_eq!(saved.openers.file_rules, model.settings.openers.file_rules);
        assert_eq!(
            resolve_file("notes.txt", &model.extension_file_openers(), &saved.openers),
            FileResolution::Handler("files:open".into())
        );
    });
}
