use crate::keymap;
use crate::state::AppState;
use gpui::{Menu, MenuItem, OsAction, SystemMenuType, actions};
use muxy_core::shortcuts;
use muxy_ui::text_input;

actions!(
    menu_bar,
    [
        HideApp,
        HideOthers,
        ShowAll,
        Quit,
        Minimize,
        Zoom,
        OpenDocs,
        OpenRepo,
        OpenMobileRepo,
        OpenDiscord,
        ReportIssue,
        OpenSettings,
        OpenConfiguration,
        Unavailable
    ]
);

#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = menu_bar, no_json)]
pub struct OpenInIde {
    pub bundle_identifier: String,
}

pub const DOCS_URL: &str = "https://muxy.app/docs";
pub const REPO_URL: &str = "https://github.com/muxy-app/muxy";
pub const MOBILE_REPO_URL: &str = "https://github.com/muxy-app/mobile";
pub const DISCORD_URL: &str = "https://discord.gg/4eMXAmJQ2n";
pub const ISSUES_URL: &str = "https://github.com/muxy-app/muxy/issues";

pub fn key_bindings() -> Vec<gpui::KeyBinding> {
    vec![
        gpui::KeyBinding::new("cmd-,", OpenSettings, None),
        gpui::KeyBinding::new("cmd-h", HideApp, None),
        gpui::KeyBinding::new("alt-cmd-h", HideOthers, None),
        gpui::KeyBinding::new("cmd-q", Quit, None),
        gpui::KeyBinding::new("cmd-m", Minimize, None),
    ]
}

pub fn reserved_combos() -> Vec<shortcuts::KeyCombo> {
    vec![
        shortcuts::KeyCombo {
            key: "h".to_owned(),
            modifiers: shortcuts::COMMAND,
        },
        shortcuts::KeyCombo {
            key: "h".to_owned(),
            modifiers: shortcuts::COMMAND | shortcuts::OPTION,
        },
        shortcuts::KeyCombo {
            key: "q".to_owned(),
            modifiers: shortcuts::COMMAND,
        },
        shortcuts::KeyCombo {
            key: "m".to_owned(),
            modifiers: shortcuts::COMMAND,
        },
    ]
}

pub fn menus(state: &AppState) -> Vec<Menu> {
    vec![
        Menu {
            name: "Muxy".into(),
            items: vec![
                MenuItem::action("Settings...", OpenSettings),
                MenuItem::action("Open Configuration...", OpenConfiguration),
                MenuItem::action("Reload Configuration", keymap::ReloadConfig),
                MenuItem::separator(),
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Hide Muxy", HideApp),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit Muxy", Quit),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Cut", text_input::Cut, OsAction::Cut),
                MenuItem::os_action("Copy", text_input::Copy, OsAction::Copy),
                MenuItem::os_action("Paste", text_input::Paste, OsAction::Paste),
                MenuItem::os_action("Select All", text_input::SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find", keymap::FindInTerminal),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open Project...", keymap::OpenProject),
                MenuItem::submenu(open_in_ide(state)),
                MenuItem::separator(),
                MenuItem::action("New Tab", keymap::NewTab),
                MenuItem::action("New Home Tab", keymap::NewHomeTab),
                MenuItem::action("New Browser Tab", keymap::NewBrowserTab),
                MenuItem::submenu(custom_commands(state)),
                MenuItem::separator(),
                MenuItem::action("Close Tab", keymap::CloseTab),
                MenuItem::separator(),
                MenuItem::action("Rename Tab", keymap::RenameTab),
                MenuItem::action("Pin/Unpin Tab", keymap::PinUnpinTab),
                MenuItem::separator(),
                MenuItem::action("Split Right", keymap::SplitRight),
                MenuItem::action("Split Down", keymap::SplitDown),
                MenuItem::action("Close Pane", keymap::ClosePane),
                MenuItem::action("Focus Pane Left", keymap::FocusPaneLeft),
                MenuItem::action("Focus Pane Right", keymap::FocusPaneRight),
                MenuItem::action("Focus Pane Up", keymap::FocusPaneUp),
                MenuItem::action("Focus Pane Down", keymap::FocusPaneDown),
                MenuItem::action(
                    "Cycle Next Tab (All Panes)",
                    keymap::CycleNextTabAcrossPanes,
                ),
                MenuItem::action(
                    "Cycle Previous Tab (All Panes)",
                    keymap::CyclePreviousTabAcrossPanes,
                ),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Toggle Sidebar", keymap::ToggleSidebar),
                MenuItem::action("Toggle Full Screen", keymap::ToggleFullScreen),
                MenuItem::separator(),
                MenuItem::action("Next Project", keymap::NextProject),
                MenuItem::action("Previous Project", keymap::PreviousProject),
                MenuItem::separator(),
                MenuItem::action("Project 1", keymap::SelectProject1),
                MenuItem::action("Project 2", keymap::SelectProject2),
                MenuItem::action("Project 3", keymap::SelectProject3),
                MenuItem::action("Project 4", keymap::SelectProject4),
                MenuItem::action("Project 5", keymap::SelectProject5),
                MenuItem::action("Project 6", keymap::SelectProject6),
                MenuItem::action("Project 7", keymap::SelectProject7),
                MenuItem::action("Project 8", keymap::SelectProject8),
                MenuItem::action("Project 9", keymap::SelectProject9),
                MenuItem::separator(),
                MenuItem::action("Theme Picker", keymap::ToggleThemePicker),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
                MenuItem::separator(),
                MenuItem::action("Next Tab", keymap::NextTab),
                MenuItem::action("Previous Tab", keymap::PreviousTab),
                MenuItem::separator(),
                MenuItem::action("Tab 1", keymap::SelectTab1),
                MenuItem::action("Tab 2", keymap::SelectTab2),
                MenuItem::action("Tab 3", keymap::SelectTab3),
                MenuItem::action("Tab 4", keymap::SelectTab4),
                MenuItem::action("Tab 5", keymap::SelectTab5),
                MenuItem::action("Tab 6", keymap::SelectTab6),
                MenuItem::action("Tab 7", keymap::SelectTab7),
                MenuItem::action("Tab 8", keymap::SelectTab8),
                MenuItem::action("Tab 9", keymap::SelectTab9),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![
                MenuItem::action("Documentation", OpenDocs),
                MenuItem::action("GitHub Repository", OpenRepo),
                MenuItem::action("Mobile App Repository", OpenMobileRepo),
                MenuItem::action("Discord", OpenDiscord),
                MenuItem::separator(),
                MenuItem::action("Report an Issue...", ReportIssue),
            ],
        },
    ]
}

fn open_in_ide(state: &AppState) -> Menu {
    let has_project = state.active_project().is_some();
    let mut items = Vec::new();
    if !has_project {
        items.push(MenuItem::action("Open in IDE", Unavailable));
        return Menu {
            name: "Open in IDE".into(),
            items,
        };
    }

    let finder = muxy_api::ide::finder();
    items.push(MenuItem::action(
        finder.display_name.clone(),
        OpenInIde {
            bundle_identifier: finder.bundle_identifier,
        },
    ));
    items.push(MenuItem::separator());

    let installed = muxy_api::ide::installed();
    if installed.is_empty() {
        items.push(MenuItem::action("No supported IDEs found", Unavailable));
    } else {
        for entry in installed {
            items.push(MenuItem::action(
                entry.display_name,
                OpenInIde {
                    bundle_identifier: entry.bundle_identifier,
                },
            ));
        }
    }

    Menu {
        name: "Open in IDE".into(),
        items,
    }
}

fn custom_commands(state: &AppState) -> Menu {
    let shortcuts = &state.command_shortcuts.shortcuts;
    let items = if shortcuts.is_empty() {
        vec![MenuItem::action("No Custom Commands", Unavailable)]
    } else {
        shortcuts
            .iter()
            .map(|shortcut| {
                let name = shortcut.display_name();
                if shortcut.trimmed_command().is_empty() {
                    MenuItem::action(name, Unavailable)
                } else {
                    MenuItem::action(
                        name,
                        keymap::RunCommandShortcut {
                            id: shortcut.id.clone(),
                        },
                    )
                }
            })
            .collect()
    };
    Menu {
        name: "Custom Commands".into(),
        items,
    }
}
