mod ai;
mod app_socket;
mod askpass;
mod backup;
mod boot;
mod cli_install;
mod diagnostics;
mod extensions;
mod favicon;
#[cfg(target_os = "linux")]
mod instance;
mod model;
mod navigation;
mod opener;
mod picker;
mod profiler;
mod proxy;
mod remote_install;
mod repository_actions;
mod server;
mod theme;
mod updater;
mod views {
    pub(crate) mod agent_sessions;
    pub(crate) mod agents_panel;
    pub(crate) mod banners;
    pub(crate) mod browser;
    pub(crate) mod cached;
    pub(crate) mod command_palette;
    pub(crate) mod composer;
    pub(crate) mod confirm;
    pub(crate) mod devices;
    pub(crate) mod disconnected;
    pub(crate) mod font_picker;
    pub(crate) mod git;
    pub(crate) mod menu;
    pub(crate) mod native_modal;
    pub(crate) mod overlays;
    pub(crate) mod pair_form;
    pub(crate) mod project_apps;
    pub(crate) mod project_editor;
    pub(crate) mod project_layouts;
    pub(crate) mod project_menu;
    pub(crate) mod project_picker;
    pub(crate) mod proxy;
    pub(crate) mod quick_terminal;
    pub(crate) mod remote_servers;
    pub(crate) mod server_menu;
    pub(crate) mod server_status;
    pub(crate) mod servers_tab;
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
    pub(crate) mod tips;
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
        pub(crate) mod scrollbar;
        pub(crate) mod selection;
    }
    pub(crate) mod webview;
    pub(crate) mod workspace;
    pub(crate) mod worktree_picker;
}

use std::cell::Cell;
use std::io::{self, Write};
use std::process::ExitCode;
use std::rc::Rc;

use gpui::{
    App, AppContext, Application, Bounds, Menu, MenuItem, OsAction, SystemMenuType,
    TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowOptions, point, px, size,
};

use model::AppModel;
use muxy_ui::tr;
use views::titlebar::initial_traffic_lights;
use views::workspace::{
    AddProject, CheckForUpdates, ClosePane, CloseTab, DecreaseFontSize, EndAllSessionsAndQuit,
    EqualizeSplits, Find, FindNext, FindPrevious, FocusPaneDown, FocusPaneLeft, FocusPaneRight,
    FocusPaneUp, HideApp, HideOthers, IncreaseFontSize, InstallCommandLineTool, Minimize,
    NewBrowserTab, NewHomeTab, NewTab, NewTabInProject, NewWorktree, NextProject, NextPrompt,
    NextTab, OpenSettings, PreviousProject, PreviousPrompt, PreviousTab, Quit, SelectCommandOutput,
    SelectProject, SelectTab, ShowAll, SplitDown, SplitRight, SwitchWorktree, ToggleAgentSessions,
    ToggleAgentsPanel, ToggleCommandPalette, ToggleComposer, ToggleFullScreen, ToggleSidebar,
    ToggleThemePicker, ToggleZoomPane, Zoom, bind_keys,
};

fn main() -> ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "--build-info")
    {
        return match serde_json::to_writer(
            io::stdout().lock(),
            &muxy_protocol::BuildInfo::current(),
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == proxy::TUNNEL)
    {
        return proxy::run_tunnel(std::env::args_os().skip(2));
    }
    run()
}

fn run_askpass() -> Option<ExitCode> {
    let socket = std::env::var_os(askpass::SOCKET)?;
    let prompt = std::env::args_os()
        .nth(1)
        .map(|prompt| prompt.to_string_lossy().into_owned())
        .unwrap_or_default();
    Some(askpass::run(&socket, &prompt))
}

fn run() -> ExitCode {
    #[cfg(target_os = "macos")]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == muxy_client::local::host::FLAG)
    {
        return muxy_client::local::host::run(std::env::args_os().skip(2));
    }
    if let Some(exit) = run_askpass() {
        return exit;
    }
    #[cfg(target_os = "linux")]
    let _installation_lease = match muxy_core::executable::current_path()
        .and_then(|executable| muxy_client::local::bundle::acquire_runtime(&executable))
    {
        Ok(lease) => lease,
        Err(error) => {
            let _ = writeln!(io::stderr(), "muxy-app: {error}");
            return ExitCode::FAILURE;
        }
    };
    let (folders, opened_folders) = async_channel::unbounded();
    #[cfg(target_os = "linux")]
    let _instance = match instance::claim(std::env::args_os().skip(1), &folders) {
        Ok(Some(instance)) => instance,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "muxy-app: {error}");
            return ExitCode::FAILURE;
        }
    };
    muxy_ui::browser::polybrowser::init();
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
    application.on_open_urls(move |urls| {
        let _ = folders.try_send(urls);
    });
    let exit_code = Rc::new(Cell::new(ExitCode::SUCCESS));
    let startup_exit_code = exit_code.clone();
    application.run(move |cx: &mut App| {
        if let Some(profile) = profile {
            profile.finish_on_quit(cx);
        }
        bind_keys(&boot.settings.keymap, cx);
        cx.on_action(|_: &HideApp, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.set_menus(menus());
        let bounds = restored_bounds(&boot, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(640.0), px(400.0))),
            window_background: WindowBackgroundAppearance::Transparent,
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            // Empty titlebar space explicitly starts native movement, not tab presses.
            is_movable: !cfg!(target_os = "macos"),
            titlebar: Some(TitlebarOptions {
                title: Some(app_name().into()),
                appears_transparent: true,
                traffic_light_position: Some(initial_traffic_lights(&boot.settings.appearance)),
            }),
            ..window_options()
        };
        if let Err(error) = cx.open_window(options, |window, cx| {
            #[cfg(target_os = "linux")]
            window.set_window_title(app_name());
            let model = cx.new(|cx| AppModel::new(boot, window, cx));
            model.update(cx, |_, cx| AppModel::open_folders_from(opened_folders, cx));
            model.update(cx, |model, cx| model.start_app_socket(window, cx));
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
            startup_exit_code.set(ExitCode::FAILURE);
            cx.quit();
        }
        cx.activate(true);
    });
    exit_code.get()
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

fn window_options() -> WindowOptions {
    WindowOptions {
        #[cfg(target_os = "linux")]
        app_id: Some(
            muxy_core::release::Channel::current()
                .bundle_identifier()
                .into(),
        ),
        ..WindowOptions::default()
    }
}

#[allow(clippy::too_many_lines, reason = "One native menu registration list")]
fn menus() -> Vec<Menu> {
    use muxy_core::capability::Feature;
    use muxy_ui::text_input;
    use views::workspace::commands::menu_item;
    let backends = model::capabilities::backends();
    let mut window_items = vec![
        menu_item(Minimize),
        menu_item(Zoom),
        MenuItem::separator(),
        menu_item(SplitRight),
        menu_item(SplitDown),
        menu_item(FocusPaneLeft),
        menu_item(FocusPaneRight),
        menu_item(FocusPaneUp),
        menu_item(FocusPaneDown),
        menu_item(ToggleZoomPane),
        menu_item(EqualizeSplits),
        menu_item(ClosePane),
        menu_item(CloseTab),
        MenuItem::separator(),
        menu_item(NextTab),
        menu_item(PreviousTab),
        MenuItem::separator(),
        menu_item(PreviousProject),
        menu_item(NextProject),
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
    let mut app_items = Vec::new();
    if cfg!(target_os = "macos") {
        app_items.extend([
            menu_item(views::workspace::OpenAbout),
            MenuItem::separator(),
        ]);
    }
    app_items.push(menu_item(OpenSettings));
    if backends.has_backend(Feature::Updates) {
        app_items.push(menu_item(CheckForUpdates));
    }
    if backends.has_backend(Feature::CommandLineTool) {
        app_items.push(menu_item(InstallCommandLineTool));
    }
    app_items.extend([
        MenuItem::separator(),
        MenuItem::os_submenu(tr!("Services"), SystemMenuType::Services),
        MenuItem::separator(),
        menu_item(HideApp),
        menu_item(HideOthers),
        menu_item(ShowAll),
        MenuItem::separator(),
        menu_item(Quit),
        menu_item(EndAllSessionsAndQuit),
    ]);
    let view_items = vec![
        menu_item(ToggleCommandPalette),
        MenuItem::separator(),
        menu_item(ToggleComposer),
        MenuItem::separator(),
        menu_item(ToggleSidebar),
        menu_item(ToggleAgentsPanel),
        menu_item(ToggleAgentSessions),
        menu_item(ToggleFullScreen),
        MenuItem::separator(),
        menu_item(ToggleThemePicker),
        MenuItem::separator(),
        menu_item(IncreaseFontSize),
        menu_item(DecreaseFontSize),
    ];
    vec![
        Menu {
            name: app_name().into(),
            items: app_items,
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
                menu_item(Find),
                menu_item(FindNext),
                menu_item(FindPrevious),
                MenuItem::separator(),
                menu_item(PreviousPrompt),
                menu_item(NextPrompt),
                menu_item(SelectCommandOutput),
            ],
        },
        Menu {
            name: tr!("File"),
            items: vec![
                menu_item(NewTab),
                menu_item(NewBrowserTab),
                menu_item(NewHomeTab),
                menu_item(NewTabInProject),
                menu_item(NewWorktree),
                menu_item(SwitchWorktree),
                menu_item(AddProject),
                MenuItem::separator(),
                menu_item(CloseTab),
            ],
        },
        Menu {
            name: tr!("View"),
            items: view_items,
        },
        Menu {
            // GPUI hands macOS the window list only for a menu named "Window".
            name: "Window".into(),
            items: window_items,
        },
    ]
}
