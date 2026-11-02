use super::*;
use muxy_core::capability::{Capability, Feature};
use muxy_core::shortcuts::ShortcutId;

const TERMINAL: [ShortcutId; 10] = [
    ShortcutId::Find,
    ShortcutId::FindNext,
    ShortcutId::FindPrevious,
    ShortcutId::PreviousPrompt,
    ShortcutId::NextPrompt,
    ShortcutId::SelectCommandOutput,
    ShortcutId::ScrollToBottom,
    ShortcutId::IncreaseFontSize,
    ShortcutId::DecreaseFontSize,
    ShortcutId::DetachTerminal,
];
const PANE_FOCUS: [ShortcutId; 4] = [
    ShortcutId::FocusPaneLeft,
    ShortcutId::FocusPaneRight,
    ShortcutId::FocusPaneUp,
    ShortcutId::FocusPaneDown,
];
const APP: [ShortcutId; 6] = [
    ShortcutId::ExistingTerminals,
    ShortcutId::ToggleFullScreen,
    ShortcutId::Minimize,
    ShortcutId::Zoom,
    ShortcutId::Quit,
    ShortcutId::EndAllSessionsAndQuit,
];

/// Whether the palette lists each command, and lists it disabled.
fn listed(
    model: &mut AppModel,
    ids: &[ShortcutId],
    cx: &mut Context<AppModel>,
) -> Vec<Option<bool>> {
    let registry = model.command_registry(true, cx);
    ids.iter()
        .map(|id| {
            registry
                .command(id.name())
                .map(muxy_ui::command_palette::Command::is_disabled)
        })
        .collect()
}

#[gpui::test]
fn the_palette_lists_the_commands_only_a_menu_bar_offered(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(listed(model, &APP, cx), [Some(false); 6]);
        assert_eq!(listed(model, &TERMINAL, cx), [Some(true); 10]);
        assert_eq!(listed(model, &PANE_FOCUS, cx), [Some(true); 4]);

        model.new_tab(cx);
        assert_eq!(listed(model, &TERMINAL, cx), [Some(false); 10]);
        assert_eq!(listed(model, &PANE_FOCUS, cx), [Some(true); 4]);

        model.split_pane(Direction::Right, cx);
        assert_eq!(listed(model, &PANE_FOCUS, cx), [Some(false); 4]);
    });
}

#[gpui::test]
fn the_palette_leaves_out_commands_this_build_cannot_run(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let install = ShortcutId::InstallCommandLineTool.name();
    view.update(cx, |model, cx| {
        for feature in [Feature::Updates, Feature::CommandLineTool] {
            model.capabilities.set(feature, Capability::Supported);
        }
        let registry = model.command_registry(true, cx);
        assert!(registry.command("check_for_updates").is_some());
        assert!(registry.command(install).is_some());

        for feature in [Feature::Updates, Feature::CommandLineTool] {
            model.capabilities.set(feature, Capability::Unsupported);
        }
        let registry = model.command_registry(true, cx);
        assert!(registry.command("check_for_updates").is_none());
        assert!(registry.command(install).is_none());
    });
}

#[gpui::test]
fn palette_queries_still_run_the_commands_they_found_first(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.new_tab(cx);
        model.split_pane(Direction::Right, cx);
    });
    cx.simulate_keystrokes(ShortcutId::ToggleCommandPalette.definition().keys[0]);
    cx.simulate_input("zoom");
    cx.simulate_keystrokes("enter");
    view.read_with(cx, |model, _| {
        let tab = model.active_tab().and_then(|tab| model.tab(tab));
        assert!(tab.expect("tab").zoomed.is_some());
    });
}

fn menu_actions(items: &[gpui::MenuItem], actions: &mut Vec<(String, Box<dyn gpui::Action>)>) {
    for item in items {
        match item {
            gpui::MenuItem::Submenu(menu) => menu_actions(&menu.items, actions),
            gpui::MenuItem::Action {
                name,
                action,
                os_action: None,
            } => actions.push((name.to_string(), action.boxed_clone())),
            _ => {}
        }
    }
}

#[gpui::test]
fn every_menu_item_has_a_label_and_a_handler_in_the_workspace(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, AppModel::new_tab);
    cx.run_until_parked();
    cx.update(|window, cx| {
        let mut actions = Vec::new();
        for menu in crate::menus() {
            menu_actions(&menu.items, &mut actions);
        }
        assert!(!actions.is_empty());
        for (name, action) in actions {
            assert!(!name.is_empty(), "{action:?} has an empty label");
            assert!(!name.contains("::"), "{name} has no row in the table");
            assert!(window.is_action_available(&*action, cx), "{name}");
        }
    });
}

#[gpui::test]
fn a_failed_save_leaves_the_state_as_it_was(cx: &mut TestAppContext) {
    use crate::model::tabs::OpenTerminal;
    use muxy_app_core::store;

    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let directory = tempfile::tempdir().expect("directory");
    view.update(cx, |model, cx| {
        let before = model.state.clone();
        let path = std::mem::replace(&mut model.path, directory.path().to_path_buf());

        model.split_pane(Direction::Right, cx);
        assert_eq!(model.state, before, "a converted edit rolls back");

        let opened = model.open_terminal(
            OpenTerminal {
                project: before.home().id,
                title: Some("Title"),
                command: Some("echo hi"),
                directory: Some(std::path::Path::new("/chosen")),
            },
            cx,
        );
        assert_eq!(opened, Ok(None));
        assert_eq!(model.state, before);

        model.path = path;
        let (tab, pane) = model
            .open_terminal(
                OpenTerminal {
                    project: before.home().id,
                    title: Some("Title"),
                    command: Some("echo hi"),
                    directory: Some(std::path::Path::new("/chosen")),
                },
                cx,
            )
            .expect("opened")
            .expect("saved");
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(model.active_tab(), Some(tab));
        let mut saved = store::load(&model.path).expect("saved");
        assert_eq!(saved.startup_command(pane), Some("echo hi"));
        assert_eq!(
            saved
                .prepare_creation(pane, std::path::Path::new("/project"))
                .0,
            b"/chosen"
        );
    });
}
