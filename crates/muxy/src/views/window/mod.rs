mod commands;
mod lifecycle;
pub mod menu_bar;
mod overlays;
mod project_menu;
mod render;
mod repository;
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
    AnyWindowHandle, AppContext, Bounds, ClipboardItem, Context, Entity, Focusable, IntoElement,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, Task, Window, px,
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
use std::path::{Path, PathBuf};
use std::time::Duration;
const BELL_FLASH_DURATION: Duration = Duration::from_millis(1250);
const NOTIFICATION_SAVE_DEBOUNCE: Duration = Duration::from_secs(2);
const TEST_CLOSE_REQUEST_ENV: &str = "MUXY_TEST_P5_CLOSE_MAIN_WINDOW_REQUEST";
const TEST_CLOSE_REQUEST_FILE: &str = ".muxy-p5-close-main-window";
const WATCHER_DEBOUNCE_MS: u64 = 300;

fn test_close_request_path(
    is_test_process: bool,
    enabled: bool,
    app_support: &Path,
) -> Option<PathBuf> {
    (is_test_process && enabled).then(|| app_support.join(TEST_CLOSE_REQUEST_FILE))
}

fn staged_close_request_path() -> Option<PathBuf> {
    test_close_request_path(
        muxy_core::prefs::is_test_process(),
        matches!(std::env::var(TEST_CLOSE_REQUEST_ENV).as_deref(), Ok("1")),
        &muxy_core::prefs::app_support_dir(),
    )
}

pub fn key_bindings() -> Vec<gpui::KeyBinding> {
    let mut bindings = muxy_ui::text_input::key_bindings();
    bindings.extend(muxy_ui::command_popover::key_bindings());
    bindings.extend(project_picker::key_bindings());
    bindings.extend(crate::views::menu::key_bindings());
    bindings.extend(crate::views::notifications::panel::key_bindings());
    bindings.extend(menu_bar::key_bindings());
    bindings.extend(omnibox::view::key_bindings());
    bindings.extend(settings::key_bindings());
    bindings.extend(crate::views::repository::pull_request::key_bindings());
    bindings.extend(crate::views::repository::ai::key_bindings());
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
    pub(crate) window_handle: AnyWindowHandle,
    pub(crate) terminal_runtime: TerminalRuntime,
    pub(crate) notification_coordinator: crate::notifications::NotificationCoordinator,
    pub(crate) native_response_receiver: Option<async_channel::Receiver<String>>,
    project_runtime: ProjectRuntime,
    _socket_runtime: SocketRuntime,
    _native_response_pump: Option<Task<()>>,
    _notification_authorization_probe: Option<Task<()>>,
    picker_search: muxy_api::picker::search::SearchService,
}

impl MainWindow {
    pub fn new(
        state: AppState,
        socket: SocketBootstrap,
        mode: muxy_core::environment::BuildMode,
        execution_environment: muxy_api::execution_environment::ExecutionEnvironmentSource,
        desktop_notifications: (
            crate::notifications::desktop::DesktopNotificationService,
            async_channel::Receiver<String>,
        ),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let menu_focus = cx.focus_handle();
        let workspace_focus = cx.focus_handle();
        let mut terminals = TerminalSurfaces::with_socket_path(socket.socket_path());
        let combos = terminal_shortcut_combos(&state);
        if let Err(error) = terminals.backend_mut().attach(
            combos,
            mode,
            socket.socket_path(),
            state.theme.bg.into(),
            window,
        ) {
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
        let hydration = execution_environment.start_hydration();
        let hydrated_environment = execution_environment.clone();
        let environment_task = cx.spawn(async move |window, cx| {
            let Some(hydration) = hydration else {
                return;
            };
            let Ok(outcome) = hydration.recv().await else {
                return;
            };
            if !matches!(
                outcome,
                muxy_api::execution_environment::HydrationOutcome::Upgraded { .. }
            ) {
                return;
            }
            let environment = hydrated_environment.snapshot();
            let _ = window.update(cx, |window, cx| {
                window.apply_environment_upgrade(environment, cx);
            });
        });
        let mut view = ViewState::new(menu_focus, workspace_focus, state.prefs.sidebar_expanded);
        view.window_active = window.is_window_active();
        let (desktop_notifications, native_response_receiver) = desktop_notifications;
        let notification_coordinator =
            crate::notifications::NotificationCoordinator::new(desktop_notifications);
        let mut native_response_receiver = Some(native_response_receiver);
        let response_receiver = native_response_receiver.take().unwrap();
        let native_response_pump = cx.spawn(async move |window, cx| {
            while let Ok(notification_id) = response_receiver.recv().await {
                if window
                    .update(cx, |window, cx| {
                        cx.activate(true);
                        window.navigate_notification(&notification_id, cx);
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
        let socket_runtime = SocketRuntime::attach(socket, cx);
        let mut main_window = Self {
            state,
            view,
            window_handle: window.window_handle(),
            terminal_runtime: TerminalRuntime::new(terminals, terminal_tasks),
            notification_coordinator,
            native_response_receiver,
            project_runtime: ProjectRuntime::new(
                watchers,
                watcher_task,
                execution_environment,
                environment_task,
            ),
            _socket_runtime: socket_runtime,
            _native_response_pump: Some(native_response_pump),
            _notification_authorization_probe: None,
            picker_search: muxy_api::picker::search::SearchService::new(),
        };
        debug_assert!(main_window.native_response_receiver.is_none());
        let authorization = main_window
            .notification_coordinator
            .query_desktop_authorization();
        main_window._notification_authorization_probe = Some(cx.spawn(async move |_, _| {
            let _ = authorization.recv().await;
        }));
        main_window.view.notification_quit_subscription = Some(cx.on_app_quit(|window, _| {
            window.flush_notification_store();
            async {}
        }));
        main_window.view.activation_subscription = Some(cx.observe_window_activation(
            window,
            |window, app_window, cx| {
                window.view.window_active = app_window.is_window_active();
                if window.view.window_active {
                    window.sync_active_notification_read_state(cx);
                    window.refresh_repository_on_activation(cx);
                }
            },
        ));
        if let Some(request_path) = staged_close_request_path() {
            let window_handle = main_window.window_handle;
            cx.spawn(async move |_, cx| {
                for _ in 0..600 {
                    if request_path.is_file() {
                        let _ = std::fs::remove_file(&request_path);
                        let _ = window_handle.update(cx, |_, window, _| window.remove_window());
                        return;
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                }
            })
            .detach();
        }
        main_window.refresh_project_truth(None, cx);
        cx.set_menus(menu_bar::menus(&main_window.state));
        main_window
    }

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.view.sidebar_expanded = !self.view.sidebar_expanded;
        cx.notify();
    }

    pub(crate) fn record_notification_anchor(&mut self, bounds: Bounds<Pixels>) {
        self.view.notification_anchor = Some(bounds);
    }

    pub(crate) fn toggle_notifications(&mut self, cx: &mut Context<Self>) {
        if matches!(self.view.overlay, Overlay::Notifications { .. }) {
            self.dismiss_overlay(cx);
            return;
        }
        let Some(anchor) = self.view.notification_anchor else {
            return;
        };
        self.view.subscriptions.clear();
        self.view.pending_focus = Some(self.view.menu_focus.clone());
        self.view.overlay = Overlay::Notifications { anchor };
        cx.notify();
    }

    pub(crate) fn clear_notifications(&mut self, cx: &mut Context<Self>) {
        if self.state.notification_store.clear() {
            self.schedule_notification_save(cx);
            cx.notify();
        }
    }

    pub(crate) fn remove_notification(&mut self, notification_id: &str, cx: &mut Context<Self>) {
        if self.state.notification_store.remove(notification_id) {
            self.schedule_notification_save(cx);
            cx.notify();
        }
    }

    pub(crate) fn activate_notification_row(
        &mut self,
        notification_id: &str,
        cx: &mut Context<Self>,
    ) {
        self.navigate_notification(notification_id, cx);
        self.dismiss_overlay(cx);
    }

    pub(crate) fn sync_active_notification_read_state(&mut self, cx: &mut Context<Self>) {
        if crate::notifications::navigation::mark_active_tab_read(
            &mut self.state,
            self.view.window_active,
        ) {
            self.schedule_notification_save(cx);
        }
    }

    pub(crate) fn show_toast(
        &mut self,
        content: crate::toast::ToastContent,
        origin: crate::toast::ToastOrigin,
        cx: &mut Context<Self>,
    ) {
        let notification_toasts_enabled =
            muxy_core::prefs::settings::bool_value("muxy.notifications.toastEnabled", true);
        if !origin.should_present(notification_toasts_enabled) {
            return;
        }
        let generation = self.view.toast.replace(content);
        let timer = crate::toast::dismissal_task(generation, cx, |window, generation, cx| {
            if window.view.toast.dismiss_generation(generation) {
                cx.notify();
            }
        });
        self.view.toast.set_timer(timer);
        cx.notify();
    }

    pub(crate) fn feedback(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
        tone: crate::toast::ToastTone,
        cx: &mut Context<Self>,
    ) {
        self.show_toast(
            crate::toast::ToastContent::new(title, body, tone, None),
            crate::toast::ToastOrigin::Feedback,
            cx,
        );
    }

    pub(crate) fn submit_notification(
        &mut self,
        event: crate::notifications::ResolvedNotificationEvent,
        focused_osc: bool,
        cx: &mut Context<Self>,
    ) {
        let inputs = crate::notifications::DeliveryInputs {
            focused_osc,
            toast_enabled: muxy_core::prefs::settings::bool_value(
                "muxy.notifications.toastEnabled",
                true,
            ),
            desktop_enabled: muxy_core::prefs::settings::bool_value(
                "muxy.notifications.desktopEnabled",
                false,
            ),
            sound: muxy_core::prefs::settings::string_value("muxy.notifications.sound", "Funk"),
        };
        let effects = self.notification_coordinator.decide(&event, inputs);
        if let Some(record) = effects.record.as_ref() {
            self.state.notification_store.insert(record.clone());
        }
        if effects.schedule_save {
            self.schedule_notification_save(cx);
        }
        if let Some(toast) = effects.toast {
            self.show_toast(
                crate::toast::ToastContent::new(
                    toast.title,
                    toast.body,
                    crate::toast::ToastTone::Success,
                    Some(crate::toast::ToastAction::NavigateNotification(
                        toast.notification_id,
                    )),
                ),
                crate::toast::ToastOrigin::Notification,
                cx,
            );
        }
        if effects.desktop
            && let Some(record) = effects.record.as_ref()
            && let Some(request) = crate::notifications::desktop::DesktopRequest::new(
                &record.id,
                &record.title,
                &record.body,
            )
        {
            self.notification_coordinator.schedule_desktop(request);
        }
        if let Some(sound) = effects.sound {
            self.notification_coordinator.play_sound(&sound);
        }
        if effects.notify {
            cx.notify();
        }
    }

    fn schedule_notification_save(&mut self, cx: &mut Context<Self>) {
        let revision = self.state.notification_store.dirty_revision();
        let generation = self.notification_coordinator.next_save_generation();
        let task = cx.spawn(async move |window, cx| {
            cx.background_executor()
                .timer(NOTIFICATION_SAVE_DEBOUNCE)
                .await;
            let _ = window.update(cx, |window, _| {
                if window
                    .notification_coordinator
                    .save_generation_is_current(generation)
                    && let Err(error) = window.state.notification_store.flush_if_revision(revision)
                {
                    log::warn!("failed to save notifications: {error}");
                }
            });
        });
        self.notification_coordinator.set_save_task(task);
    }

    fn flush_notification_store(&mut self) {
        if let Err(error) = self.state.notification_store.flush() {
            log::warn!("failed to flush notifications: {error}");
        }
    }

    pub(crate) fn navigate_notification(&mut self, notification_id: &str, cx: &mut Context<Self>) {
        let outcome = crate::notifications::navigation::navigate(&mut self.state, notification_id);
        if let Some(error) = &outcome.error {
            log::warn!("failed to navigate notification: {error}");
        }
        if outcome.read_changed {
            self.schedule_notification_save(cx);
        }
        if outcome.changed() {
            cx.notify();
        }
    }

    pub(crate) fn activate_toast(&mut self, cx: &mut Context<Self>) {
        let action = self.view.toast.dismiss();
        cx.notify();
        if let Some(action) = action {
            self.dispatch_toast_action(action, cx);
        }
    }

    fn dispatch_toast_action(&mut self, action: crate::toast::ToastAction, cx: &mut Context<Self>) {
        match action {
            crate::toast::ToastAction::NavigateNotification(notification_id) => {
                self.navigate_notification(&notification_id, cx);
            }
        }
    }
}

impl Drop for MainWindow {
    fn drop(&mut self) {
        self.flush_notification_store();
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

#[cfg(test)]
mod feedback_tests {
    #[test]
    fn feedback_migration_inventory_and_retained_surfaces_are_exact() {
        let commands = include_str!("commands.rs");
        let lifecycle = include_str!("lifecycle.rs");
        let overlays = include_str!("overlays.rs");
        let repository = include_str!("repository.rs");

        for needle in [
            "window.feedback(title, message, crate::toast::ToastTone::Error, cx)",
            "Muxy could not read this layout file.",
            "This layout has no panes to open.",
        ] {
            assert!(
                commands.contains(needle),
                "missing command feedback: {needle}"
            );
        }
        for needle in [
            "Refresh Worktrees",
            "The primary worktree cannot be removed.",
            "native_removal_warning(&effects.warnings)",
            "native_creation_warning(&effects.warnings)",
        ] {
            assert!(
                lifecycle.contains(needle),
                "missing lifecycle feedback: {needle}"
            );
        }
        for needle in [
            "AI Provider Unavailable",
            "Could Not Save Project Prompt",
            "Repository Context Changed",
            "Repository Action Unavailable",
            "Repository AI Action Complete",
            "Repository AI Action Failed",
            "Could Not Open Pull Request",
            "Worktrees require an existing local Git project.",
        ] {
            assert!(
                overlays.contains(needle),
                "missing overlay feedback: {needle}"
            );
        }
        for needle in [
            "Stash Updated",
            "Branch Updated",
            "Changes Updated",
            "Pull Request Updated",
        ] {
            assert!(
                repository.contains(needle),
                "missing repository feedback: {needle}"
            );
        }

        assert!(commands.contains("let answer = self.ask("));
        assert!(lifecycle.contains("let answer = window.ask("));
        assert!(overlays.contains("RepositoryAiPanel::confirmation"));
        assert!(repository.contains("set_branch_operation_error"));
        assert!(repository.contains("set_changes_operation_error"));
        assert!(repository.contains("set_pull_request_operation_error"));
        assert!(repository.contains("set_changes_discard_in_flight"));
    }

    #[test]
    fn feedback_path_is_toast_only_and_bypasses_notification_policy() {
        let source = include_str!("mod.rs");
        let start = source.find("pub(crate) fn feedback(").unwrap();
        let end = source[start..]
            .find("pub(crate) fn submit_notification")
            .unwrap()
            + start;
        let feedback = &source[start..end];

        assert!(feedback.contains("ToastOrigin::Feedback"));
        assert!(feedback.contains("self.show_toast("));
        assert!(!feedback.contains("notification_coordinator"));
        assert!(!feedback.contains("notification_store"));
        assert!(!feedback.contains("play_sound"));
        assert!(!feedback.contains("schedule_desktop"));
    }

    #[test]
    fn notifications_main_window_lifecycle_wires_both_final_flush_paths() {
        let source = include_str!("mod.rs");
        let quit = source.find("cx.on_app_quit(|window, _|").unwrap();
        let drop = source.find("impl Drop for MainWindow").unwrap();

        assert!(source[quit..drop].contains("window.flush_notification_store();"));
        assert!(source[drop..].contains("self.flush_notification_store();"));
    }

    #[test]
    fn notifications_staged_close_request_is_test_only_and_root_local() {
        let root = std::path::Path::new("/tmp/muxy-test-support");
        assert_eq!(super::test_close_request_path(false, true, root), None);
        assert_eq!(super::test_close_request_path(true, false, root), None);
        assert_eq!(
            super::test_close_request_path(true, true, root),
            Some(root.join(".muxy-p5-close-main-window"))
        );
    }
}
