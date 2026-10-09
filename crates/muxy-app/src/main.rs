mod ai;
mod askpass;
mod backup;
mod boot;
mod cli_install;
mod diagnostics;
mod extensions;
mod model;
mod navigation;
mod opener;
mod picker;
mod profiler;
mod remote_install;
mod repository_actions;
mod server;
mod theme;
mod updater;
mod views {
    pub(crate) mod banners;
    pub(crate) mod cached;
    pub(crate) mod command_palette;
    pub(crate) mod composer;
    pub(crate) mod confirm;
    pub(crate) mod disconnected;
    pub(crate) mod font_picker;
    pub(crate) mod git;
    pub(crate) mod menu;
    pub(crate) mod native_modal;
    pub(crate) mod new_tab;
    pub(crate) mod overlays;
    pub(crate) mod project_editor;
    pub(crate) mod project_layouts;
    pub(crate) mod project_menu;
    pub(crate) mod project_picker;
    pub(crate) mod quick_terminal;
    pub(crate) mod remote_servers;
    pub(crate) mod server_status;
    pub(crate) mod session_picker;
    pub(crate) mod settings;
    pub(crate) mod shortcut_hints;
    pub(crate) mod sidebar;
    pub(crate) mod splits;
    pub(crate) mod status_bar;
    pub(crate) mod tab_activity;
    pub(crate) mod tab_menu;
    pub(crate) mod tab_sidebar;
    pub(crate) mod tab_strip;
    pub(crate) mod theme_picker;
    pub(crate) mod titlebar;
    pub(crate) mod updates;
    pub(crate) mod terminal {
        pub(crate) mod clipboard;
        pub(crate) mod colors;
        pub(crate) mod cursor;
        pub(crate) mod element;
        pub(crate) mod find;
        pub(crate) mod ime;
        pub(crate) mod input;
        pub(crate) mod links;
        pub(crate) mod pane;
        pub(crate) mod scroll;
        pub(crate) mod selection;
    }
    pub(crate) mod voice;
    pub(crate) mod webview;
    pub(crate) mod workspace;
}

use std::io::{self, Write};
use std::process::ExitCode;

use gpui::{
    App, AppContext, Application, Bounds, Menu, MenuItem, OsAction, SystemMenuType,
    TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowOptions, point, px, size,
};

use model::AppModel;
use muxy_ui::tr;
use views::workspace::{
    AddProject, CheckForUpdates, ClosePane, CloseTab, DecreaseFontSize, EndAllSessionsAndQuit,
    Find, FindNext, FindPrevious, FocusPaneDown, FocusPaneLeft, FocusPaneRight, FocusPaneUp,
    HideApp, HideOthers, IncreaseFontSize, InstallCommandLineTool, Minimize, NewHomeTab, NewTab,
    NewTabInProject, NextProject, NextPrompt, NextTab, OpenConfiguration, OpenSettings,
    PreviousProject, PreviousPrompt, PreviousTab, Quit, SelectCommandOutput, SelectProject,
    SelectTab, ShowAll, SplitDown, SplitRight, ToggleCommandPalette, ToggleComposer,
    ToggleFullScreen, ToggleSidebar, ToggleThemePicker, ToggleVoiceRecording, ToggleZoomPane, Zoom,
    bind_keys,
};

fn main() -> ExitCode {
    if let Some(socket) = std::env::var_os(askpass::SOCKET) {
        let prompt = std::env::args_os()
            .nth(1)
            .map(|prompt| prompt.to_string_lossy().into_owned())
            .unwrap_or_default();
        return askpass::run(&socket, &prompt);
    }
    let profile = profiler::init();
    let boot = {
        let _span = profiler::span(profiler::Metric::BootLoad);
        match boot::Boot::load() {
            Ok(boot) => boot,
            Err(error) => {
                let _ = writeln!(io::stderr(), "muxy-app: {error}");
                return ExitCode::FAILURE;
            }
        }
    };
    #[cfg(target_os = "macos")]
    muxy_ui::native_scroll::use_overlay_scrollers();
    let application = Application::new().with_assets(muxy_ui::assets::Assets);
    let (folders, opened_folders) = async_channel::unbounded();
    application.on_open_urls(move |urls| {
        let _ = folders.try_send(urls);
    });
    application.run(move |cx: &mut App| {
        if let Some(profile) = profile {
            profile.finish_on_quit(cx);
        }
        bind_keys(&boot.settings.keymap, cx);
        let config_path = boot.state_path.with_file_name("ghostty.conf");
        cx.on_action(move |_: &OpenConfiguration, _| {
            if let Err(error) = std::process::Command::new("/usr/bin/open")
                .arg("-t")
                .arg(&config_path)
                .status()
                .and_then(|status| {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(io::Error::other(format!("open exited with {status}")))
                    }
                })
            {
                let _ = writeln!(
                    io::stderr(),
                    "muxy-app: could not open configuration: {error}"
                );
            }
        });
        cx.on_action(|_: &HideApp, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.set_menus(menus());
        let bounds = restored_bounds(&boot, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(640.0), px(400.0))),
            window_background: WindowBackgroundAppearance::Transparent,
            // Empty titlebar space explicitly starts native movement, not tab presses.
            is_movable: !cfg!(target_os = "macos"),
            titlebar: Some(TitlebarOptions {
                title: Some(app_name().into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.0), px(9.0))),
            }),
            ..WindowOptions::default()
        };
        if let Err(error) = cx.open_window(options, |window, cx| {
            let model = cx.new(|cx| AppModel::new(boot, window, cx));
            model.update(cx, |_, cx| AppModel::open_folders_from(opened_folders, cx));
            let configuration = model.downgrade();
            cx.on_action(move |_: &views::workspace::ReloadConfiguration, cx| {
                let _ = configuration.update(cx, AppModel::reload_configuration);
            });
            let weak = model.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                if weak.update(cx, AppModel::quit).is_err() {
                    cx.quit();
                }
                false
            });
            model
        }) {
            let _ = writeln!(io::stderr(), "muxy-app: could not open window: {error}");
            cx.quit();
        }
        cx.activate(true);
    });
    ExitCode::SUCCESS
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Saved coordinates are checked against the GPUI pixel range"
)]
fn restored_bounds(boot: &boot::Boot, cx: &App) -> Bounds<gpui::Pixels> {
    if let Some(bounds) = boot.state.window().bounds
        && [bounds.x, bounds.y, bounds.width, bounds.height]
            .into_iter()
            .all(|value| value.abs() <= f64::from(f32::MAX))
    {
        return Bounds::new(
            point(px(bounds.x as f32), px(bounds.y as f32)),
            size(px(bounds.width as f32), px(bounds.height as f32)),
        );
    }
    let [width, height] = boot.settings.window.default_size;
    Bounds::centered(None, size(px(width), px(height)), cx)
}

fn app_name() -> &'static str {
    muxy_core::release::Channel::current().app_name()
}

#[allow(clippy::too_many_lines, reason = "One native menu registration list")]
fn menus() -> Vec<Menu> {
    use muxy_ui::text_input;
    let mut window_items = vec![
        MenuItem::action(tr!("Minimize"), Minimize),
        MenuItem::action(tr!("Zoom"), Zoom),
        MenuItem::separator(),
        MenuItem::action(tr!("Split Right"), SplitRight),
        MenuItem::action(tr!("Split Down"), SplitDown),
        MenuItem::action(tr!("Focus Pane Left"), FocusPaneLeft),
        MenuItem::action(tr!("Focus Pane Right"), FocusPaneRight),
        MenuItem::action(tr!("Focus Pane Up"), FocusPaneUp),
        MenuItem::action(tr!("Focus Pane Down"), FocusPaneDown),
        MenuItem::action(tr!("Toggle Pane Zoom"), ToggleZoomPane),
        MenuItem::action(tr!("Close Pane"), ClosePane),
        MenuItem::action(tr!("Close Tab"), CloseTab),
        MenuItem::separator(),
        MenuItem::action(tr!("Next Tab"), NextTab),
        MenuItem::action(tr!("Previous Tab"), PreviousTab),
        MenuItem::separator(),
        MenuItem::action(tr!("Previous Project"), PreviousProject),
        MenuItem::action(tr!("Next Project"), NextProject),
        MenuItem::separator(),
    ];
    window_items.extend(
        (0..9).map(|index| MenuItem::action(tr!("Tab %lld", index + 1), SelectTab { index })),
    );
    window_items.push(MenuItem::separator());
    window_items
        .extend((0..9).map(|index| {
            MenuItem::action(tr!("Project %lld", index + 1), SelectProject { index })
        }));
    vec![
        Menu {
            name: app_name().into(),
            items: vec![
                MenuItem::action(tr!("Settings…"), OpenSettings),
                MenuItem::action(tr!("Open Configuration…"), OpenConfiguration),
                MenuItem::action(
                    tr!("Reload Configuration"),
                    views::workspace::ReloadConfiguration,
                ),
                MenuItem::action(tr!("Check for Updates…"), CheckForUpdates),
                MenuItem::action(tr!("Install Command Line Tool…"), InstallCommandLineTool),
                MenuItem::separator(),
                MenuItem::os_submenu(tr!("Services"), SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action(tr!("Hide %@", app_name()), HideApp),
                MenuItem::action(tr!("Hide Others"), HideOthers),
                MenuItem::action(tr!("Show All"), ShowAll),
                MenuItem::separator(),
                MenuItem::action(tr!("Quit %@", app_name()), Quit),
                MenuItem::action(tr!("End All Sessions and Quit"), EndAllSessionsAndQuit),
            ],
        },
        Menu {
            name: tr!("Edit"),
            items: vec![
                MenuItem::os_action(tr!("Cut"), text_input::Cut, OsAction::Cut),
                MenuItem::os_action(tr!("Copy"), text_input::Copy, OsAction::Copy),
                MenuItem::os_action(tr!("Paste"), text_input::Paste, OsAction::Paste),
                MenuItem::os_action(
                    tr!("Select All"),
                    text_input::SelectAll,
                    OsAction::SelectAll,
                ),
                MenuItem::separator(),
                MenuItem::action(tr!("Find…"), Find),
                MenuItem::action(tr!("Find Next"), FindNext),
                MenuItem::action(tr!("Find Previous"), FindPrevious),
                MenuItem::separator(),
                MenuItem::action(tr!("Previous Prompt"), PreviousPrompt),
                MenuItem::action(tr!("Next Prompt"), NextPrompt),
                MenuItem::action(tr!("Select Command Output"), SelectCommandOutput),
            ],
        },
        Menu {
            name: tr!("File"),
            items: vec![
                MenuItem::action(tr!("New Tab"), NewTab),
                MenuItem::action(tr!("New Home Tab"), NewHomeTab),
                MenuItem::action(tr!("New Tab in Project…"), NewTabInProject),
                MenuItem::action(tr!("Open Project…"), AddProject),
                MenuItem::separator(),
                MenuItem::action(tr!("Close Tab"), CloseTab),
            ],
        },
        Menu {
            name: tr!("View"),
            items: vec![
                MenuItem::action(tr!("Command Palette…"), ToggleCommandPalette),
                MenuItem::separator(),
                MenuItem::action(tr!("Toggle Composer"), ToggleComposer),
                MenuItem::action(tr!("Toggle Voice Recording"), ToggleVoiceRecording),
                MenuItem::separator(),
                MenuItem::action(tr!("Toggle Sidebar"), ToggleSidebar),
                MenuItem::action(tr!("Toggle Full Screen"), ToggleFullScreen),
                MenuItem::separator(),
                MenuItem::action(tr!("Theme Picker"), ToggleThemePicker),
                MenuItem::separator(),
                MenuItem::action(tr!("Increase Font Size"), IncreaseFontSize),
                MenuItem::action(tr!("Decrease Font Size"), DecreaseFontSize),
            ],
        },
        Menu {
            // GPUI hands macOS the window list only for a menu named "Window".
            name: "Window".into(),
            items: window_items,
        },
    ]
}
