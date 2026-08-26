mod assets;
mod command;
mod git;
mod keymap;
#[cfg(target_os = "macos")]
mod native_compositor;
mod resources;
mod socket;
mod state;
mod terminal;
mod themes;
mod views;

use assets::Assets;
use gpui::AppContext;
use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions, point, px, size,
};
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
    let socket = socket::runtime::start(socket_path)
        .unwrap_or_else(|error| panic!("failed to start socket server: {error}"));

    Application::new()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            cx.bind_keys(muxy_ui::text_input::key_bindings());
            cx.bind_keys(views::window::key_bindings());
            register_app_actions(cx);

            let state = AppState::load(cx);
            muxy_core::prefs::settings::sync();
            cx.bind_keys(keymap::key_bindings(&state.shortcuts));
            cx.bind_keys(keymap::command_bindings(&state.command_shortcuts));
            let metrics = state.metrics;
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

            cx.open_window(options, move |window, cx| {
                cx.new(|cx| MainWindow::new(state, socket, mode, window, cx))
            })
            .expect("failed to open window");
            cx.activate(true);
        });
}
