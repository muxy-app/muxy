mod assets;
mod command;
mod composer;
mod git;
mod keymap;
#[cfg(target_os = "macos")]
mod native_compositor;
pub mod notifications;
pub mod panels;
mod pasteboard;
mod platform;
mod project_operations;
pub mod quick_terminal;
mod repository;
mod resources;
mod socket;
mod state;
mod terminal;
mod themes;
pub mod toast;
mod views;

use assets::Assets;
use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, point, px, size,
};
use gpui::{AppContext, BorrowAppContext};
use state::AppState;
use views::window::MainWindow;

fn register_app_actions(cx: &mut App) {
    use views::window::menu_bar;

    cx.on_action(|_: &menu_bar::HideApp, cx: &mut App| cx.hide());
    cx.on_action(|_: &menu_bar::HideOthers, cx: &mut App| cx.hide_other_apps());
    cx.on_action(|_: &menu_bar::ShowAll, cx: &mut App| cx.unhide_other_apps());
    cx.on_action(|_: &menu_bar::Quit, cx: &mut App| cx.quit());
    cx.on_action(|_: &menu_bar::OpenDocs, cx: &mut App| cx.open_url(menu_bar::DOCS_URL));
    cx.on_action(|_: &menu_bar::OpenRepo, cx: &mut App| cx.open_url(menu_bar::REPO_URL));
    cx.on_action(|_: &menu_bar::OpenMobileRepo, cx: &mut App| {
        cx.open_url(menu_bar::MOBILE_REPO_URL)
    });
    cx.on_action(|_: &menu_bar::OpenDiscord, cx: &mut App| cx.open_url(menu_bar::DISCORD_URL));
    cx.on_action(|_: &menu_bar::ReportIssue, cx: &mut App| cx.open_url(menu_bar::ISSUES_URL));
    cx.on_action(|_: &quick_terminal::CloseSurface, cx: &mut App| {
        cx.update_global::<quick_terminal::runtime::QuickTerminalRuntime, _>(|runtime, cx| {
            runtime.close_surface(cx);
        });
    });
}

const BASELINE_TITLE_BAR_HEIGHT: f32 = 32.0;
const TRAFFIC_LIGHT_HEIGHT: f32 = 14.0;
const TRAFFIC_LIGHT_X: f32 = 9.0;

fn main() {
    if let Err(error) = muxy_core::migration::run_startup() {
        eprintln!("failed to migrate Swift profile: {error}");
        std::process::exit(1);
    }
    let app_support = muxy_core::prefs::app_support_dir();
    let mode = muxy_core::build_mode!();
    let socket_path =
        muxy_core::environment::RuntimePathPolicy::new(mode).main_socket_path(&app_support);
    #[cfg(target_os = "macos")]
    terminal::install_development_cli_environment(mode, &socket_path)
        .unwrap_or_else(|error| panic!("failed to install development CLI environment: {error}"));
    let quick_terminal_socket_path = socket_path.clone();
    let socket = socket::runtime::start(socket_path)
        .unwrap_or_else(|error| panic!("failed to start socket server: {error}"));
    let execution_environment = git::environment_source();

    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            cx.bind_keys(views::window::key_bindings());
            register_app_actions(cx);

            let mut quick_terminal = quick_terminal::runtime::QuickTerminalRuntime::load(
                mode,
                quick_terminal_socket_path,
            );
            quick_terminal.start(cx);
            cx.set_global(quick_terminal);

            let desktop_notifications =
                notifications::desktop::DesktopNotificationService::prepare();
            let state = AppState::load(cx);
            muxy_core::prefs::settings::sync();
            cx.bind_keys(keymap::key_bindings(&state.shortcuts));
            cx.bind_keys(keymap::command_bindings(&state.command_shortcuts));
            let metrics = state.metrics;
            let quick_terminal_theme = state.theme.clone();
            let quick_terminal_appearance = state.appearance;
            let quick_terminal_metrics = state.metrics;
            let title_bar_height = f32::from(metrics.title_bar_height());
            let extra_vertical_space = title_bar_height - BASELINE_TITLE_BAR_HEIGHT;
            let from_bottom =
                (BASELINE_TITLE_BAR_HEIGHT - TRAFFIC_LIGHT_HEIGHT - extra_vertical_space) / 2.0;
            let traffic_light_y = BASELINE_TITLE_BAR_HEIGHT - from_bottom - TRAFFIC_LIGHT_HEIGHT;

            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Muxy".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(TRAFFIC_LIGHT_X), px(traffic_light_y))),
                }),
                kind: WindowKind::Normal,
                window_background: WindowBackgroundAppearance::Transparent,
                window_min_size: Some(size(px(640.0), px(400.0))),
                ..Default::default()
            };

            let main_window = cx
                .open_window(options, move |window, cx| {
                    window.on_window_should_close(cx, |_, cx| {
                        cx.quit();
                        true
                    });
                    cx.new(|cx| {
                        MainWindow::new(
                            state,
                            socket,
                            mode,
                            execution_environment,
                            desktop_notifications,
                            window,
                            cx,
                        )
                    })
                })
                .expect("failed to open window");
            cx.update_global::<quick_terminal::runtime::QuickTerminalRuntime, _>(|runtime, cx| {
                runtime.register_main_window(
                    main_window,
                    quick_terminal_theme,
                    quick_terminal_appearance,
                    quick_terminal_metrics,
                    cx,
                );
                runtime.run_staged_spike(cx);
            });
            cx.activate(true);
        });
}
