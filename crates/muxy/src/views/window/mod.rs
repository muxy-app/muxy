mod commands;
mod lifecycle;
pub mod menu_bar;
mod overlays;
mod project_menu;
mod render;
mod terminal;
mod view_state;
mod workspace;

use crate::command::Command;
use crate::socket::runtime::{SocketBootstrap, SocketRuntime};
use crate::state::AppState;
use crate::terminal::{
    ConfirmationKind, PointerInput, SurfaceAction, SurfaceSignal, TerminalEvent, TerminalSurfaces,
    dispatch_for_query,
};
use crate::views::menu::{Item, Menu};
use crate::views::{
    create_worktree_overlay, omnibox, overlay, project_picker, settings, workspace_view,
};
use gpui::{
    AppContext, Bounds, ClipboardItem, Context, Entity, Focusable, IntoElement, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Render, Task, Window, px,
};
use muxy_core::prefs::Prefs;
use muxy_core::shortcuts::KeyCombo;
use muxy_core::store::logo;
use muxy_ui::scrollbar::{
    MINIMUM_THUMB_LENGTH as SCROLLBAR_MIN_THUMB, TRACK_INSET as SCROLLBAR_TRACK_INSET,
    ThumbGeometry, WIDTH as SCROLLBAR_WIDTH,
};
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use overlay::Overlay;
use project_picker::PickerEvent;
use project_picker::ProjectPicker;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
const BELL_FLASH_DURATION: Duration = Duration::from_millis(1250);
const WATCHER_DEBOUNCE_MS: u64 = 300;

pub fn key_bindings() -> Vec<gpui::KeyBinding> {
    let mut bindings = project_picker::key_bindings();
    bindings.extend(crate::views::menu::key_bindings());
    bindings.extend(menu_bar::key_bindings());
    bindings.extend(omnibox::view::key_bindings());
    bindings.extend(settings::key_bindings());
    bindings.push(gpui::KeyBinding::new(
        "shift-enter",
        crate::views::app::SearchPrevious,
        Some(muxy_ui::text_input::SEARCH_CONTEXT),
    ));
    bindings
}

use lifecycle::ProjectRuntime;
use terminal::TerminalRuntime;
use view_state::{ScrollbarDrag, ViewState, WorkspaceGesture};

pub struct MainWindow {
    pub state: AppState,
    view: ViewState,
    pub(crate) terminal_runtime: TerminalRuntime,
    project_runtime: ProjectRuntime,
    _socket_runtime: SocketRuntime,
    picker_search: muxy_api::picker::search::SearchService,
}

impl MainWindow {
    pub fn new(
        state: AppState,
        socket: SocketBootstrap,
        mode: muxy_core::environment::BuildMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let menu_focus = cx.focus_handle();
        let workspace_focus = cx.focus_handle();
        let mut terminals = TerminalSurfaces::with_socket_path(socket.socket_path());
        let combos = terminal_shortcut_combos(&state);
        if let Err(error) =
            terminals
                .backend_mut()
                .attach(combos, mode, socket.socket_path(), window)
        {
            log::warn!("terminal backend unavailable: {error}");
        }
        let terminal_tasks = lifecycle::spawn_terminal_pumps(&mut terminals, cx);
        let (watchers, watcher_events) = muxy_api::watcher::Watchers::new();
        let watcher_task = cx.spawn(async move |window, cx| {
            while let Ok(project_id) = watcher_events.recv().await {
                cx.background_executor()
                    .timer(Duration::from_millis(WATCHER_DEBOUNCE_MS))
                    .await;
                let mut ids = HashSet::from([project_id]);
                while let Ok(extra) = watcher_events.try_recv() {
                    ids.insert(extra);
                }
                let updated = window.update(cx, |window, cx| {
                    window.refresh_project_truth(Some(&ids), cx);
                });
                if updated.is_err() {
                    return;
                }
            }
        });
        let view = ViewState::new(menu_focus, workspace_focus, state.prefs.sidebar_expanded);
        let socket_runtime = SocketRuntime::attach(socket, cx);
        let mut main_window = Self {
            state,
            view,
            terminal_runtime: TerminalRuntime::new(terminals, terminal_tasks),
            project_runtime: ProjectRuntime::new(watchers, watcher_task),
            _socket_runtime: socket_runtime,
            picker_search: muxy_api::picker::search::SearchService::new(),
        };
        main_window.refresh_project_truth(None, cx);
        cx.set_menus(menu_bar::menus(&main_window.state));
        main_window
    }

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.view.sidebar_expanded = !self.view.sidebar_expanded;
        cx.notify();
    }
}

fn terminal_shortcut_combos(state: &AppState) -> Vec<KeyCombo> {
    let mut combos = state.shortcuts.assigned_combos();
    combos.extend(menu_bar::reserved_combos());
    combos.push(state.command_shortcuts.prefix_combo.clone());
    combos.extend(
        state
            .command_shortcuts
            .shortcuts
            .iter()
            .map(|shortcut| shortcut.combo.clone()),
    );
    combos
}
