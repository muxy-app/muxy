use std::path::Path;
use std::time::Duration;

use gpui::{Focusable, Modifiers, VisualTestContext};
use muxy_app_core::extensions::workspace;
use muxy_ui::picker::{Picker, PickerEvent};
use serde_json::{Map, Value, json};

use super::*;
use crate::ai::{PROVIDERS, provider};
use crate::extensions::authoring::AgentTask;
use crate::views::menu::Command;
use crate::views::overlays::Overlay;

fn open_model(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    model.update(cx, |model, _| model.ai.installed = PROVIDERS.to_vec());
    (model, cx)
}

fn pump_until(cx: &mut VisualTestContext, done: impl Fn(&mut VisualTestContext) -> bool) {
    cx.executor().allow_parking();
    for _ in 0..2_000 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the extension work did not finish");
}

fn write_package(root: &Path, name: &str, version: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("package.json"),
        json!({"name": name, "version": version, "muxy": {}}).to_string(),
    )
    .unwrap();
}

/// Runs a `muxy extension` command as the app socket would.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Every caller builds the arguments with json!"
)]
fn call(
    model: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    verb: &str,
    args: Value,
) -> Result<Value, String> {
    let args: Map<String, Value> = args.as_object().cloned().unwrap_or_default();
    let (reply, replied) = async_channel::bounded(1);
    let task = model.update(cx, |model, cx| model.extension_cli_call(verb, &args, cx));
    cx.spawn(|_| async move {
        let _ = reply.send(task.await).await;
    })
    .detach();
    cx.executor().allow_parking();
    for _ in 0..2_000 {
        cx.run_until_parked();
        if let Ok(result) = replied.try_recv() {
            return result;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("{verb} did not answer");
}

fn agent_rows(model: &Entity<AppModel>, cx: &VisualTestContext) -> Vec<&'static str> {
    model.read_with(cx, |model, _| {
        model
            .extension_agents()
            .iter()
            .map(|agent| agent.id)
            .collect()
    })
}

#[gpui::test]
fn the_sidebar_button_opens_a_menu_to_create_or_manage(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    cx.run_until_parked();
    let button = cx.debug_bounds("sidebar-extensions").expect("button");
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    model.read_with(cx, |model, _| {
        let Some(Overlay::Menu(menu)) = &model.overlay else {
            panic!("no menu");
        };
        let commands: Vec<_> = menu
            .items
            .iter()
            .filter_map(crate::views::menu::Item::command)
            .collect();
        assert!(matches!(
            commands.as_slice(),
            [Command::CreateExtension, Command::ManageExtensions]
        ));
        assert_eq!(menu.rises_from.map(|bounds| bounds.size), Some(button.size));
        assert!(
            model.settings_tab.is_none(),
            "the button no longer jumps to Settings"
        );
    });

    let create = cx.debug_bounds("menu-item-0").expect("Create Extension");
    let manage = cx.debug_bounds("menu-item-1").expect("Manage Extensions");
    assert!(
        manage.bottom() <= button.top(),
        "the menu opens above the button, not over the controls below it"
    );
    cx.simulate_click(create.center(), Modifiers::none());
    cx.run_until_parked();
    model.read_with(cx, |model, _| {
        assert!(matches!(
            &model.overlay,
            Some(Overlay::ExtensionAgent(picker)) if picker.task == AgentTask::Create
        ));
    });
    assert!(cx.debug_bounds("extension-agent").is_some());
}

#[gpui::test]
fn the_command_palette_can_create_and_manage_extensions(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    cx.update(|window, cx| {
        model.update(cx, |model, cx| {
            let registry = model.command_registry(true, cx);
            let create = registry.action("create_extension").expect("create").clone();
            create(model, window, cx);
            assert!(matches!(
                &model.overlay,
                Some(Overlay::ExtensionAgent(open)) if open.task == AgentTask::Create
            ));
            model.dismiss_overlay(cx);
            assert!(registry.action("manage_extensions").is_some());
        });
    });
}

#[gpui::test]
fn the_picker_lists_installed_agents_with_the_last_one_first(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let codex = provider("codex").expect("codex");
    let claude = provider("claude").expect("claude");
    model.update(cx, |model, _| model.ai.installed = vec![claude, codex]);
    assert_eq!(agent_rows(&model, cx), ["claude", "codex"]);
    model.update(cx, |model, cx| {
        let task = model.start_agent_for_test(codex, &AgentTask::Create, cx);
        task.expect("start");
    });
    assert_eq!(agent_rows(&model, cx), ["codex", "claude"]);
}

fn open_picker(model: &Entity<AppModel>, cx: &mut VisualTestContext) -> Entity<Picker> {
    model.update(cx, |model, cx| {
        model.open_extension_agent(AgentTask::Create, cx);
        let Some(Overlay::ExtensionAgent(open)) = &model.overlay else {
            panic!("no picker");
        };
        open.picker.clone()
    })
}

fn rows(picker: &Entity<Picker>, cx: &VisualTestContext) -> Vec<String> {
    picker.read_with(cx, |picker, _| {
        picker
            .items()
            .iter()
            .filter_map(|item| item.clone().row_data())
            .map(|row| row.id.to_string())
            .collect()
    })
}

#[gpui::test]
fn an_empty_picker_says_what_to_install(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    model.update(cx, |model, _| model.ai.installed = Vec::new());
    let picker = open_picker(&model, cx);
    cx.run_until_parked();
    assert!(cx.debug_bounds("extension-agent").is_some());
    assert!(
        cx.debug_bounds("picker-footer-check-again").is_some(),
        "Check Again"
    );
    picker.read_with(cx, |picker, _| assert!(picker.items().is_empty()));
}

#[gpui::test]
fn checking_again_lists_new_agents_and_keeps_the_keyboard(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    model.update(cx, |model, _| model.ai.installed = Vec::new());
    let picker = open_picker(&model, cx);
    cx.run_until_parked();
    cx.update(|window, cx| {
        model.update(cx, |model, _| model.focus.focus(window));
    });
    model.update(cx, |model, cx| {
        model.ai.installed = vec![provider("claude").expect("claude")];
        model.refresh_extension_agents(cx);
    });
    cx.run_until_parked();
    assert_eq!(rows(&picker, cx), ["claude"]);
    let focused = cx.update(|window, cx| picker.focus_handle(cx).contains_focused(window, cx));
    assert!(focused, "the picker has the keyboard");
}

#[gpui::test]
fn the_picker_filters_by_what_is_typed_and_escape_cancels(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let (claude, codex) = (
        provider("claude").expect("claude"),
        provider("codex").expect("codex"),
    );
    model.update(cx, |model, _| model.ai.installed = vec![claude, codex]);
    let picker = open_picker(&model, cx);
    cx.run_until_parked();
    assert!(cx.debug_bounds("picker-row-claude").is_some());
    assert!(cx.debug_bounds("picker-row-codex").is_some());
    picker.update(cx, |picker, cx| picker.set_query("codex", cx));
    model.update(cx, AppModel::update_extension_agents);
    assert_eq!(rows(&picker, cx), ["codex"]);
    picker.update(cx, |_, cx| cx.emit(PickerEvent::Dismissed));
    cx.run_until_parked();
    model.read_with(cx, |model, _| assert!(model.overlay.is_none()));
}

#[test]
fn each_task_says_what_the_agent_is_picked_for() {
    let folder = Path::new("/work").to_owned();
    let name = "files".to_owned();
    for (task, expected) in [
        (AgentTask::Create, "Create an extension with…"),
        (
            AgentTask::Edit {
                name: name.clone(),
                folder: folder.clone(),
            },
            "Edit files with…",
        ),
        (
            AgentTask::Clone {
                name: name.clone(),
                installed: folder.clone(),
            },
            "Clone files as files-custom with…",
        ),
        (AgentTask::Publish { name, folder }, "Publish files with…"),
        (
            AgentTask::Settings {
                profile: "/profile".into(),
            },
            "Change settings with…",
        ),
    ] {
        assert_eq!(
            crate::views::extension_agent::placeholder(&task).as_ref(),
            expected
        );
    }
}

#[gpui::test]
fn an_agent_starts_in_a_home_tab_at_the_workspace_with_its_prompt(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    model.update(cx, |model, cx| {
        let tabs = model
            .state
            .server_home(ServerId::local())
            .expect("home")
            .tabs
            .len();
        model
            .start_agent_for_test(provider("claude").expect("claude"), &AgentTask::Create, cx)
            .expect("start");
        assert!(model.overlay.is_none(), "the picker closed");
        let home = model.state.server_home(ServerId::local()).expect("home");
        assert_eq!(home.tabs.len(), tabs + 1);
        assert_eq!(model.state.current_project().id, home.id);
        let root = model.path.with_file_name("extension-workspace");
        let pane = model.active_pane().expect("pane");
        let command = model
            .state
            .startup_command(pane)
            .expect("command")
            .to_owned();
        assert!(command.starts_with("/test/agent '"), "{command}");
        assert!(
            command.contains(&workspace::skill_file(&root).display().to_string()),
            "{command}"
        );
        assert_eq!(
            model
                .state
                .clone()
                .prepare_creation(pane, Path::new("/elsewhere"))
                .0,
            root.display().to_string().into_bytes(),
        );
    });
}

#[gpui::test]
fn a_start_that_cant_happen_closes_the_picker_and_says_why(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    open_picker(&model, cx);
    model.update(cx, |model, cx| {
        model.choose_extension_agent(provider("claude").expect("claude"), cx);
    });
    cx.run_until_parked();
    model.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(
            model.error.is_some(),
            "a test binary has no muxy to give the agent"
        );
    });
}

#[gpui::test]
fn a_started_agent_closes_the_picker_and_a_failed_one_keeps_it(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let directory = tempfile::tempdir().unwrap();
    let unsaveable = directory.path().join("state-is-a-folder");
    std::fs::create_dir_all(&unsaveable).unwrap();
    open_picker(&model, cx);
    model.update(cx, |model, cx| {
        let claude = provider("claude").expect("claude");
        let tabs = model.state.current_project().tabs.len();
        let saved = std::mem::replace(&mut model.path, unsaveable);
        model
            .start_agent_for_test(claude, &AgentTask::Create, cx)
            .expect("saving failed, which has been reported");
        model.path = saved;
        let Some(Overlay::ExtensionAgent(open)) = &model.overlay else {
            panic!("the picker closed although no terminal opened");
        };
        assert_eq!(open.attempt, 0, "another agent can be picked");
        assert_eq!(model.state.current_project().tabs.len(), tabs);

        model
            .start_agent_for_test(claude, &AgentTask::Create, cx)
            .expect("start");
        assert!(model.overlay.is_none(), "a started agent closes the picker");
    });
}

#[gpui::test]
fn commands_load_enable_reload_and_unload_a_folder(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let source = tempfile::tempdir().unwrap();
    write_package(source.path(), "hello", "1.0.0");
    let path = source.path().display().to_string();

    let loaded = call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    assert_eq!(loaded["name"], "hello");

    let listed = call(&model, cx, "extension.list", json!({})).unwrap();
    let hello = &listed["extensions"][0];
    assert_eq!(hello["name"], "hello");
    assert_eq!(hello["enabled"], false, "a loaded folder starts disabled");
    assert_eq!(hello["unpacked"], true);

    call(&model, cx, "extension.enable", json!({"name": "hello"})).unwrap();
    let status = call(&model, cx, "extension.status", json!({"name": "hello"})).unwrap();
    assert_eq!(status["enabled"], true);

    write_package(source.path(), "hello", "1.1.0");
    call(&model, cx, "extension.reload", json!({"name": "hello"})).unwrap();
    let status = call(&model, cx, "extension.status", json!({"name": "hello"})).unwrap();
    assert_eq!(status["version"], "1.1.0");
    assert_eq!(status["enabled"], true, "a reload keeps it on");

    write_package(source.path(), "hello", "1.2.0");
    let again = call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    assert_eq!(
        again["name"], "hello",
        "loading the same folder again reloads it"
    );
    let status = call(&model, cx, "extension.status", json!({"name": "hello"})).unwrap();
    assert_eq!(status["version"], "1.2.0");

    call(&model, cx, "extension.disable", json!({"name": "hello"})).unwrap();
    call(&model, cx, "extension.unload", json!({"name": "hello"})).unwrap();
    assert!(
        source.path().join("package.json").is_file(),
        "unloading keeps the files"
    );
    assert!(call(&model, cx, "extension.status", json!({"name": "hello"})).is_err());
}

#[gpui::test]
fn a_broken_reload_reports_why_and_leaves_the_extension_running(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let source = tempfile::tempdir().unwrap();
    write_package(source.path(), "hello", "1.0.0");
    let path = source.path().display().to_string();
    call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    call(&model, cx, "extension.enable", json!({"name": "hello"})).unwrap();
    std::fs::write(source.path().join("package.json"), "not json").unwrap();

    let error = call(&model, cx, "extension.reload", json!({"name": "hello"})).unwrap_err();
    assert!(
        error.contains("package.json") || error.contains("expected"),
        "{error}"
    );

    let status = call(&model, cx, "extension.status", json!({"name": "hello"})).unwrap();
    assert_eq!(status["version"], "1.0.0");
    assert_eq!(status["enabled"], true);
}

#[gpui::test]
fn reloading_one_extension_keeps_the_others_as_they_were(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    write_package(first.path(), "first", "1.0.0");
    write_package(second.path(), "second", "1.0.0");
    for folder in [&first, &second] {
        let path = folder.path().display().to_string();
        call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    }
    write_package(first.path(), "first", "2.0.0");
    write_package(second.path(), "second", "2.0.0");
    call(&model, cx, "extension.reload", json!({"name": "first"})).unwrap();
    model.read_with(cx, |model, _| {
        let installed = &model.extensions.registry.extensions;
        assert_eq!(installed["first"].version, "2.0.0");
        assert_eq!(installed["second"].version, "1.0.0");
    });
}

#[gpui::test]
fn a_renamed_package_is_reported_without_stopping_the_running_copy(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let source = tempfile::tempdir().unwrap();
    write_package(source.path(), "hello", "1.0.0");
    let path = source.path().display().to_string();
    call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    call(&model, cx, "extension.enable", json!({"name": "hello"})).unwrap();
    let before = model.read_with(cx, |model, _| model.extension_epoch_for_test("hello"));

    write_package(source.path(), "renamed", "2.0.0");
    let error = call(&model, cx, "extension.reload", json!({"name": "hello"})).unwrap_err();
    assert!(error.contains("renamed"), "{error}");

    model.read_with(cx, |model, _| {
        assert_eq!(
            model.extension_epoch_for_test("hello"),
            before,
            "the extension wasn't stopped"
        );
    });
    let status = call(&model, cx, "extension.status", json!({"name": "hello"})).unwrap();
    assert_eq!(status["version"], "1.0.0");
    assert_eq!(status["enabled"], true);
}

#[gpui::test]
fn a_successful_reload_restarts_only_that_extension(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    write_package(first.path(), "first", "1.0.0");
    write_package(second.path(), "second", "1.0.0");
    for folder in [&first, &second] {
        let path = folder.path().display().to_string();
        call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    }
    let epochs = |model: &Entity<AppModel>, cx: &mut VisualTestContext| {
        model.read_with(cx, |model, _| {
            (
                model.extension_epoch_for_test("first"),
                model.extension_epoch_for_test("second"),
            )
        })
    };
    let before = epochs(&model, cx);
    call(&model, cx, "extension.reload", json!({"name": "first"})).unwrap();
    let after = epochs(&model, cx);
    assert!(after.0 > before.0, "the reloaded one restarted");
    assert_eq!(after.1, before.1, "the other one kept running");
}

#[gpui::test]
fn a_status_shows_only_that_extensions_load_errors(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let folder = tempfile::tempdir().unwrap();
    let hello = folder.path().join("hello");
    write_package(&hello, "hello", "1.0.0");
    let path = hello.canonicalize().unwrap().display().to_string();
    call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    model.update(cx, |model, _| {
        let errors = &mut model.extensions.registry.errors;
        errors.push(format!("{path}-two: invalid manifest"));
        errors.push(format!("{path}: invalid manifest"));
    });
    let status = call(&model, cx, "extension.status", json!({"name": "hello"})).unwrap();
    let errors = status["load_errors"].as_array().expect("errors");
    assert_eq!(errors.len(), 1, "{errors:?}");
}

#[gpui::test]
fn a_folder_with_control_characters_is_refused_before_anything_is_typed(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    model.update(cx, |model, cx| {
        let tabs = model.state.current_project().tabs.len();
        let task = AgentTask::Edit {
            name: "hello".into(),
            folder: "/work/x\u{3}touch p\r".into(),
        };
        let error = model
            .start_agent_for_test(provider("claude").expect("claude"), &task, cx)
            .unwrap_err();
        assert!(error.contains("can't be typed"), "{error}");
        assert_eq!(model.state.current_project().tabs.len(), tabs);
    });
}

#[gpui::test]
fn a_cancelled_agent_doesnt_start_when_the_same_task_is_reopened(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    open_picker(&model, cx);
    model.update(cx, |model, cx| {
        let attempt = 7;
        if let Some(Overlay::ExtensionAgent(open)) = &mut model.overlay {
            open.attempt = attempt;
        }
        assert!(model.extension_attempt_current(attempt));
        model.dismiss_overlay(cx);
        assert!(!model.extension_attempt_current(attempt));
        model.open_extension_agent(AgentTask::Create, cx);
        assert!(!model.extension_attempt_current(attempt));
    });
}

#[gpui::test]
fn a_command_that_arrives_before_extensions_are_read_waits_for_them(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let installed = model.read_with(cx, |model, _| model.extensions.registry.directory());
    write_package(&installed.join("store"), "store", "1.0.0");
    model.update(cx, |model, _| model.forget_extensions_loaded_for_test());
    let listed = call(&model, cx, "extension.list", json!({})).unwrap();
    assert_eq!(listed["extensions"][0]["name"], "store");
}

#[gpui::test]
fn typing_while_an_agent_gets_ready_keeps_the_progress_showing(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let picker = open_picker(&model, cx);
    let shown = rows(&picker, cx);
    model.update(cx, |model, cx| {
        if let Some(Overlay::ExtensionAgent(open)) = &mut model.overlay {
            open.attempt = 7;
        }
        model.ai.installed = Vec::new();
        model.update_extension_agents(cx);
    });
    assert_eq!(
        rows(&picker, cx),
        shown,
        "the list isn't rebuilt behind the progress"
    );
}

#[gpui::test]
fn a_renamed_folder_cant_be_loaded_a_second_time(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let source = tempfile::tempdir().unwrap();
    write_package(source.path(), "hello", "1.0.0");
    let path = source.path().display().to_string();
    call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    write_package(source.path(), "renamed", "1.0.0");
    let error = call(&model, cx, "extension.load", json!({"path": path})).unwrap_err();
    assert!(error.contains("'hello'"), "{error}");
}

#[gpui::test]
fn commands_refuse_what_agents_may_not_do(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let installed = model.read_with(cx, |model, _| model.extensions.registry.directory());
    write_package(&installed.join("store"), "store", "1.0.0");
    model.update(cx, AppModel::refresh_installed_extensions);
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model.extensions.registry.extensions.contains_key("store")
        })
    });

    let error = call(&model, cx, "extension.unload", json!({"name": "store"})).unwrap_err();
    assert!(error.contains("Settings"), "{error}");
    assert!(installed.join("store/package.json").is_file());
    assert!(call(&model, cx, "extension.publish", json!({"name": "store"})).is_err());
    assert!(call(&model, cx, "extension.uninstall", json!({"name": "store"})).is_err());

    let other = tempfile::tempdir().unwrap();
    write_package(other.path(), "store", "1.0.0");
    let path = other.path().display().to_string();
    let error = call(&model, cx, "extension.load", json!({"path": path})).unwrap_err();
    assert!(error.contains("marketplace"), "{error}");
    assert!(call(&model, cx, "extension.reload", json!({"name": "missing"})).is_err());
}

#[gpui::test]
fn validate_reports_what_the_store_needs(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let source = tempfile::tempdir().unwrap();
    write_package(source.path(), "hello", "1.0.0");
    let path = source.path().display().to_string();
    let reply = call(&model, cx, "extension.validate", json!({"path": path})).unwrap();
    let problems = reply["problems"].as_array().expect("problems");
    assert!(!problems.is_empty());
    assert_eq!(reply["name"], "hello");
}

#[gpui::test]
fn a_settings_agent_starts_in_a_home_tab_at_the_profile_with_its_skill(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    model.update(cx, |model, cx| {
        model.open_settings_agent(cx);
        let Some(Overlay::ExtensionAgent(open)) = &model.overlay else {
            panic!("no picker");
        };
        let task = open.task.clone();
        let profile = model.path.parent().expect("profile").to_owned();
        assert_eq!(
            task,
            AgentTask::Settings {
                profile: profile.clone()
            }
        );
        model
            .start_agent_for_test(provider("claude").expect("claude"), &task, cx)
            .expect("start");
        assert!(model.overlay.is_none(), "the picker closed");
        let home = model.state.server_home(ServerId::local()).expect("home");
        assert_eq!(model.state.current_project().id, home.id);
        let skill = muxy_app_core::settings::agent::skill_file(&profile);
        assert_eq!(
            std::fs::read_to_string(&skill).expect("skill"),
            muxy_app_core::settings::agent::SKILL
        );
        assert!(
            !model.path.with_file_name("extension-workspace").exists(),
            "no extension workspace is made"
        );
        let pane = model.active_pane().expect("pane");
        let command = model
            .state
            .startup_command(pane)
            .expect("command")
            .to_owned();
        assert!(command.contains(&skill.display().to_string()), "{command}");
        assert_eq!(
            model
                .state
                .clone()
                .prepare_creation(pane, Path::new("/elsewhere"))
                .0,
            profile.display().to_string().into_bytes(),
        );
    });
}
