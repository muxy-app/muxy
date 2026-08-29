use super::QuickTerminalApplicationService;
use super::panel::{
    AccessibilityPreferences, PanelPresentation, QuickTerminalConfiguration, effective_appearance,
    transition_duration,
};
use super::platform::SystemMutation;
use super::session::{QuickTerminalSession, QuickTerminalSessionHandle};
use super::view::{
    BridgeAction, ConfirmationPrompt, QuickSetting, QuickTerminalSurface, QuickTerminalSurfaceSlot,
    QuickTerminalView, QuickTerminalViewModel,
};
use crate::terminal::surfaces::StandaloneLaunchContext;
use crate::terminal::{
    ConfirmationId, ConfirmationKind, StandaloneTerminal, SurfaceAction, SurfaceSignal,
};
use crate::views::window::MainWindow;
use gpui::{App, BorrowAppContext, Global, Subscription, Task, WindowHandle};
#[cfg(target_os = "macos")]
use gpui::{AppContext, Bounds, WindowBounds, WindowKind, WindowOptions, px, size};
use muxy_core::environment::BuildMode;
use muxy_core::quick_terminal::QuickTerminalShortcut;
use muxy_core::quick_terminal::presentation::PresentationTransition;
use muxy_core::shortcuts::{KeyCombo, ShortcutMap};
use muxy_core::store::CommandShortcuts;
use muxy_ui::theme::{Metrics, Theme};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const STAGED_SPIKE_ENV: &str = "MUXY_TEST_P6_SPIKE_CASE";
const STAGED_SPIKE_STATUS_FILE: &str = ".muxy-p6-spike-status.json";
const STAGED_SPIKE_CONTROL_FILE: &str = ".muxy-p6-spike-control.json";

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownStep {
    StopTriggers,
    HidePanel,
    StopPumps,
    TerminateSession,
    ClosePanel,
    StopShortcuts,
}

#[cfg(test)]
const SHUTDOWN_ORDER: [ShutdownStep; 6] = [
    ShutdownStep::StopTriggers,
    ShutdownStep::HidePanel,
    ShutdownStep::StopPumps,
    ShutdownStep::TerminateSession,
    ShutdownStep::ClosePanel,
    ShutdownStep::StopShortcuts,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuickTerminalSettingsState {
    pub enabled: bool,
    pub shortcut: QuickTerminalShortcut,
    pub shortcut_label: String,
    pub monitoring_label: String,
}

struct StagedSpikeControl {
    id: u64,
    command: String,
    text: Option<String>,
    bytes: Option<Vec<u8>>,
    last_lines: Option<usize>,
}

impl StagedSpikeControl {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        let object = value.as_object()?;
        let id = object.get("id")?.as_u64()?;
        let command = object.get("command")?.as_str()?.to_owned();
        let text = object
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let bytes = object.get("bytes").and_then(|value| {
            value
                .as_array()?
                .iter()
                .map(|byte| u8::try_from(byte.as_u64()?).ok())
                .collect::<Option<Vec<_>>>()
        });
        let last_lines = object
            .get("lastLines")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        Some(Self {
            id,
            command,
            text,
            bytes,
            last_lines,
        })
    }
}

#[derive(Default)]
struct PendingConfirmations {
    pending: VecDeque<ConfirmationPrompt>,
}

impl PendingConfirmations {
    fn enqueue(&mut self, prompt: ConfirmationPrompt) {
        if !self.pending.contains(&prompt) {
            self.pending.push_back(prompt);
        }
    }

    fn active(&self) -> Option<ConfirmationPrompt> {
        self.pending.front().copied()
    }

    fn resolve(&mut self, id: ConfirmationId) -> Option<ConfirmationPrompt> {
        if self.active()?.id != id {
            return None;
        }
        self.pending.pop_front()
    }

    fn cancel_all(&mut self) -> Vec<ConfirmationPrompt> {
        self.pending.drain(..).collect()
    }

    fn clear(&mut self) {
        self.pending.clear();
    }

    fn len(&self) -> usize {
        self.pending.len()
    }
}

pub struct QuickTerminalRuntime {
    shortcuts: QuickTerminalApplicationService,
    terminal: StandaloneTerminal,
    session: QuickTerminalSession<QuickTerminalSurface>,
    view_surface: QuickTerminalSurfaceSlot,
    mode: BuildMode,
    socket_path: PathBuf,
    panel: Option<WindowHandle<QuickTerminalView>>,
    main_window: Option<WindowHandle<MainWindow>>,
    theme: Option<Theme>,
    metrics: Metrics,
    configuration: QuickTerminalConfiguration,
    accessibility: AccessibilityPreferences,
    presentation: PanelPresentation,
    presentation_task: Option<Task<()>>,
    panel_generation: u64,
    visible: bool,
    status: String,
    confirmations: PendingConfirmations,
    notification_generation: u64,
    destructive_close_pending: bool,
    trigger_task: Option<Task<()>>,
    wakeup_task: Option<Task<()>>,
    event_task: Option<Task<()>>,
    terminal_shortcut_task: Option<Task<()>>,
    system_observers: Option<super::platform::SystemObservers>,
    system_task: Option<Task<()>>,
    staged_control_task: Option<Task<()>>,
    quit_subscription: Option<Subscription>,
    terminated: bool,
}

impl Global for QuickTerminalRuntime {}

impl QuickTerminalSessionHandle for QuickTerminalSurface {
    fn set_focused(&self, focused: bool) {
        self.borrow().set_focused(focused);
    }

    fn set_occluded(&self, occluded: bool) {
        self.borrow().set_occluded(occluded);
    }

    fn request_close(&self) {
        self.borrow().request_close();
    }
}

impl QuickTerminalRuntime {
    pub fn load(mode: BuildMode, socket_path: PathBuf) -> Self {
        #[cfg(target_os = "macos")]
        let accessibility = super::platform::macos::accessibility_preferences();
        #[cfg(not(target_os = "macos"))]
        let accessibility = AccessibilityPreferences::default();
        Self {
            shortcuts: QuickTerminalApplicationService::load(),
            terminal: StandaloneTerminal::new(),
            session: QuickTerminalSession::new(),
            view_surface: Rc::new(RefCell::new(None)),
            mode,
            socket_path,
            panel: None,
            main_window: None,
            theme: None,
            metrics: Metrics::new(1.0),
            configuration: QuickTerminalConfiguration::load(),
            accessibility,
            presentation: PanelPresentation::default(),
            presentation_task: None,
            panel_generation: 0,
            visible: false,
            status: "Ready".to_owned(),
            confirmations: PendingConfirmations::default(),
            notification_generation: 0,
            destructive_close_pending: false,
            trigger_task: None,
            wakeup_task: None,
            event_task: None,
            terminal_shortcut_task: None,
            system_observers: None,
            system_task: None,
            staged_control_task: None,
            quit_subscription: None,
            terminated: false,
        }
    }

    pub fn start(&mut self, cx: &mut App) {
        if let Err(error) = self.shortcuts.start() {
            log::warn!("failed to start Quick Terminal shortcut service: {error}");
        }
        self.shortcuts.run_staged_control();
        let (system_sender, system_receiver) = async_channel::bounded(16);
        match super::platform::observe_system_mutations(system_sender) {
            Ok(observers) => {
                self.system_observers = Some(observers);
                self.system_task = Some(cx.spawn(async move |cx| {
                    while let Ok(mutation) = system_receiver.recv().await {
                        if cx
                            .update_global::<Self, _>(|runtime, cx| {
                                runtime.handle_system_mutation(mutation, cx)
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }));
            }
            Err(error) => log::warn!("failed to observe Quick Terminal system changes: {error}"),
        }
        let triggers = self.shortcuts.trigger_receiver();
        self.trigger_task = Some(cx.spawn(async move |cx| {
            while triggers.recv().await.is_ok() {
                if cx
                    .update_global::<Self, _>(|runtime, cx| runtime.toggle(cx))
                    .is_err()
                {
                    return;
                }
            }
        }));
        self.quit_subscription = Some(cx.on_app_quit(|cx| {
            cx.update_global::<Self, _>(|runtime, cx| runtime.terminate(cx));
            async {}
        }));
    }

    pub fn register_main_window(
        &mut self,
        main_window: WindowHandle<MainWindow>,
        theme: Theme,
        metrics: Metrics,
        cx: &mut App,
    ) {
        self.main_window = Some(main_window);
        self.theme = Some(theme);
        self.metrics = metrics;
        self.refresh_view(cx);
    }

    pub fn update_appearance(&mut self, theme: Theme, metrics: Metrics, cx: &mut App) {
        self.theme = Some(theme);
        self.metrics = metrics;
        self.session.reload(|_| self.terminal.reload_config());
        self.refresh_view(cx);
    }

    pub fn settings_state(&self) -> QuickTerminalSettingsState {
        QuickTerminalSettingsState {
            enabled: self.configuration.enabled,
            shortcut: self.shortcuts.shortcut().clone(),
            shortcut_label: self.shortcuts.shortcut_label(),
            monitoring_label: self.shortcuts.monitoring_label(),
        }
    }

    pub fn validate_reverse_conflict(&self, combo: &KeyCombo) -> Result<(), String> {
        super::settings_transaction::validate_reverse_conflict(
            combo,
            self.shortcuts.shortcut(),
            super::platform::resolve_key,
        )
        .map_err(|error| error.to_string())
    }

    pub fn request_input_monitoring(&mut self, cx: &mut App) -> bool {
        let granted = self.shortcuts.request_input_monitoring_access();
        self.status = self.shortcuts.monitoring_label();
        self.refresh_view(cx);
        granted
    }

    pub fn refresh_input_monitoring(&mut self, cx: &mut App) -> bool {
        let granted = self.shortcuts.refresh_input_monitoring_access();
        self.status = self.shortcuts.monitoring_label();
        self.refresh_view(cx);
        granted
    }

    pub fn refresh_on_activation(&mut self, cx: &mut App) {
        self.refresh_input_monitoring(cx);
        self.handle_system_mutation(SystemMutation::KeyboardLayout, cx);
    }

    fn handle_system_mutation(&mut self, mutation: SystemMutation, cx: &mut App) {
        match mutation {
            SystemMutation::Accessibility => {
                #[cfg(target_os = "macos")]
                {
                    self.accessibility = super::platform::macos::accessibility_preferences();
                    self.refresh_view(cx);
                    self.prepare_visible_panel(cx);
                }
            }
            SystemMutation::KeyboardLayout => {
                let current = self.shortcuts.shortcut().clone();
                let Some(shortcut) =
                    refreshed_keyboard_layout_shortcut(&current, super::platform::resolve_key)
                else {
                    return;
                };
                if let Err(error) = self.apply_shortcut_setting(shortcut, cx) {
                    self.status = format!("Failed to refresh keyboard layout: {error}");
                    self.refresh_view(cx);
                }
            }
            SystemMutation::Screens => self.prepare_visible_panel(cx),
        }
    }

    fn prepare_visible_panel(&mut self, cx: &mut App) {
        if !self.visible {
            return;
        }
        let Some(panel) = self.panel else {
            return;
        };
        let result = panel
            .update(cx, |view, _, cx| {
                let telemetry = view.prepare();
                cx.notify();
                telemetry
            })
            .map_err(|error| error.to_string())
            .and_then(|result| result.map(|_| ()));
        if let Err(error) = result {
            self.status = format!("Failed to prepare Quick Terminal panel: {error}");
            self.refresh_view(cx);
        }
    }

    pub fn apply_shortcut_setting(
        &mut self,
        shortcut: QuickTerminalShortcut,
        cx: &mut App,
    ) -> Result<(), String> {
        let shortcuts = ShortcutMap::load();
        let commands = CommandShortcuts::load();
        self.shortcuts
            .update_shortcut(
                shortcut,
                &super::settings_transaction::conflict_candidates(&shortcuts, &commands),
            )
            .map_err(|error| error.to_string())?;
        self.status = "Quick Terminal shortcut updated".to_owned();
        self.refresh_view(cx);
        Ok(())
    }

    pub fn apply_live_setting(
        &mut self,
        key: &str,
        value: serde_json::Value,
        cx: &mut App,
    ) -> Result<(), String> {
        let previous = quick_terminal_setting_value(self.configuration, key)
            .ok_or_else(|| format!("unsupported Quick Terminal setting {key}"))?;
        muxy_core::prefs::settings::try_set(key, value)
            .map_err(|error| format!("failed to persist {key}: {error}"))?;
        let next = QuickTerminalConfiguration::load();
        if key == "muxy.quickTerminal.enabled"
            && let Err(error) = self.shortcuts.set_enabled(next.enabled)
        {
            let _ = muxy_core::prefs::settings::try_set(key, previous);
            return Err(error.to_string());
        }
        self.configuration = next;
        if self.configuration.enabled {
            self.status = "Ready".to_owned();
        } else {
            self.destroy_disabled_runtime(cx);
            return Ok(());
        }
        self.apply_live_configuration(cx);
        Ok(())
    }

    pub fn apply_json_settings(&mut self, text: &str, cx: &mut App) -> Result<(), String> {
        let proposal = muxy_core::prefs::settings::SettingsProposal::parse(text)
            .map_err(|error| error.to_string())?;
        let enabled = proposal
            .settings
            .iter()
            .find(|(key, _)| key == "muxy.quickTerminal.enabled")
            .and_then(|(_, value)| value.as_bool())
            .unwrap_or(self.configuration.enabled);
        super::settings_transaction::apply_proposal(
            &mut self.shortcuts,
            proposal,
            &ShortcutMap::load(),
            &CommandShortcuts::load(),
            enabled,
        )
        .map_err(|error| error.to_string())?;
        let next = QuickTerminalConfiguration::load();
        self.configuration = next;
        if self.configuration.enabled {
            self.status = "Settings applied".to_owned();
            self.apply_live_configuration(cx);
        } else {
            self.destroy_disabled_runtime(cx);
        }
        Ok(())
    }

    fn apply_live_configuration(&mut self, cx: &mut App) {
        self.session.reload(|_| self.terminal.reload_config());
        self.refresh_view(cx);
        if let Some(panel) = self.panel {
            let _ = panel.update(cx, |view, _, cx| {
                let _ = view.prepare();
                cx.notify();
            });
        }
    }

    pub fn run_staged_spike(&mut self, cx: &mut App) {
        if !muxy_core::prefs::is_test_process() {
            return;
        }
        let Ok(case_name) = std::env::var(STAGED_SPIKE_ENV) else {
            return;
        };
        if !matches!(
            case_name.as_str(),
            "panel"
                | "spike"
                | "panel-lifecycle"
                | "live-settings"
                | "phase-5"
                | "phase-6"
                | "final-debug"
                | "final-release"
        ) {
            return;
        }
        let status = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.show(cx)))
        {
            Ok(Ok(())) => self.staged_status(&case_name, 0, None, 200, cx),
            Ok(Err(error)) => self.staged_status(&case_name, 0, Some(error), 200, cx),
            Err(payload) => {
                self.staged_status(&case_name, 0, Some(panic_message(payload)), 200, cx)
            }
        };
        self.write_staged_status(&status);
        if case_name != "panel" {
            self.start_staged_control_pump(cx);
        }
    }

    pub fn toggle(&mut self, cx: &mut App) {
        let result = if self.visible {
            self.hide(cx);
            Ok(())
        } else {
            self.show(cx)
        };
        if let Err(error) = result {
            self.status = error.clone();
            self.refresh_view(cx);
            log::warn!("failed to toggle Quick Terminal: {error}");
        }
    }

    pub fn bridge_action(&mut self, action: BridgeAction, cx: &mut App) {
        match action {
            BridgeAction::Close => self.hide(cx),
            BridgeAction::ToggleQuickSettings => {
                if let Some(panel) = self.panel {
                    let _ = panel.update(cx, |view, _, cx| {
                        view.toggle_quick_settings();
                        cx.notify();
                    });
                }
            }
            BridgeAction::ToggleShortcutSettings | BridgeAction::OpenSettings => {
                self.hide_with_focus(false, cx);
                self.open_settings(cx);
            }
            BridgeAction::SetQuickSetting { setting, value } => {
                self.set_quick_setting(setting, value, cx);
            }
            BridgeAction::Reset => {
                if let Err(error) = self.reset_configuration(cx) {
                    self.status = error;
                    self.refresh_view(cx);
                }
            }
            BridgeAction::ResolveConfirmation { id, approved } => {
                self.resolve_confirmation(id, approved, cx)
            }
        }
    }

    pub fn show(&mut self, cx: &mut App) -> Result<(), String> {
        if self.terminated {
            return Err("Quick Terminal runtime is terminated".to_owned());
        }
        if !self.configuration.enabled {
            return Err("Quick Terminal is disabled".to_owned());
        }
        self.ensure_panel(cx)?;
        let panel = self.panel.expect("panel was created");
        panel
            .update(cx, |view, _, _| view.prepare())
            .map_err(|error| format!("failed to prepare Quick Terminal panel: {error}"))??;
        let launch = StandaloneLaunchContext::new(muxy_core::prefs::home_dir(), &self.socket_path);
        let generation = panel
            .update(cx, |_, window, cx| {
                self.session.show(|| {
                    let surface = Rc::new(RefCell::new(self.terminal.spawn(&launch, window, cx)?));
                    self.view_surface.replace(Some(surface.clone()));
                    Ok(surface)
                })
            })
            .map_err(|error| format!("failed to create Quick Terminal shell: {error}"))??;
        self.visible = true;
        self.status = format!("Ready · generation {generation}");
        self.refresh_view(cx);
        if let Some(transition) = self.presentation.request(true) {
            let duration = transition_duration(true, self.accessibility.reduce_motion);
            panel
                .update(cx, |view, window, cx| {
                    view.begin_show(duration, window, cx);
                    cx.notify();
                })
                .map_err(|error| format!("failed to show Quick Terminal panel: {error}"))?;
            self.schedule_transition(transition, duration, true, cx);
        } else {
            panel
                .update(cx, |view, window, cx| {
                    view.begin_show(Duration::ZERO, window, cx);
                    cx.notify();
                })
                .map_err(|error| format!("failed to show Quick Terminal panel: {error}"))?;
        }
        self.terminal.set_window_active(true);
        if let Some(surface) = self.session.surface() {
            surface.set_focused(true);
        }
        Ok(())
    }

    pub fn hide(&mut self, cx: &mut App) {
        self.hide_with_focus(true, cx);
    }

    pub fn hide_from_outside_click(&mut self, cx: &mut App) {
        if self.visible {
            self.hide_with_focus(false, cx);
        }
    }

    pub fn close_surface(&mut self, cx: &mut App) {
        if !self.visible || self.destructive_close_pending {
            return;
        }
        if !self.session.request_close() {
            self.release_panel_and_surface(true, true, cx);
            self.status = "Closed".to_owned();
            return;
        }
        self.destructive_close_pending = true;
        self.status = "Closing…".to_owned();
        self.refresh_view(cx);
    }

    fn close_surface_from_window(&mut self, cx: &mut App) {
        self.release_panel_and_surface(false, false, cx);
        self.status = "Closed".to_owned();
    }

    fn hide_with_focus(&mut self, restores_focus: bool, cx: &mut App) {
        self.destructive_close_pending = false;
        self.deny_pending_confirmations();
        self.terminal.set_window_active(false);
        self.session.hide();
        self.visible = false;
        let Some(panel) = self.panel else {
            return;
        };
        let Some(transition) = self.presentation.request(false) else {
            return;
        };
        let duration = transition_duration(false, self.accessibility.reduce_motion);
        let _ = panel.update(cx, |view, window, cx| {
            view.begin_hide(duration, window);
            cx.notify();
        });
        self.schedule_transition(transition, duration, restores_focus, cx);
    }

    pub fn terminate(&mut self, cx: &mut App) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        self.trigger_task.take();
        self.system_task.take();
        self.system_observers.take();
        self.presentation_task.take();
        if let Some(panel) = self.panel {
            let _ = panel.update(cx, |view, window, _| {
                view.begin_hide(Duration::ZERO, window);
                view.finish_hide(false);
            });
        }
        self.visible = false;
        self.destructive_close_pending = false;
        self.deny_pending_confirmations();
        self.wakeup_task.take();
        self.event_task.take();
        self.terminal_shortcut_task.take();
        self.staged_control_task.take();
        self.terminal.set_window_active(false);
        self.session.terminate();
        self.view_surface.replace(None);
        if let Some(panel) = self.panel.take() {
            let _ = panel.update(cx, |_, window, _| window.remove_window());
        }
        self.shortcuts.stop();
        self.status = "Terminated".to_owned();
    }

    #[cfg(not(target_os = "macos"))]
    fn ensure_panel(&mut self, _cx: &mut App) -> Result<(), String> {
        Err("Quick Terminal panels are unavailable on this platform".to_owned())
    }

    #[cfg(target_os = "macos")]
    fn ensure_panel(&mut self, cx: &mut App) -> Result<(), String> {
        if self.panel.is_some() {
            return Ok(());
        }
        let theme = self
            .theme
            .clone()
            .ok_or_else(|| "Quick Terminal main window is not registered".to_owned())?;
        let configuration = self.configuration;
        let appearance = effective_appearance(configuration, self.accessibility);
        let terminal_backdrop = terminal_backdrop(&theme, appearance);
        let metrics = self.metrics;
        let bounds = Bounds::centered(
            None,
            size(
                px(configuration.width as f32),
                px(configuration.height as f32),
            ),
            cx,
        );
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: false,
            show: false,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: appearance.background,
            ..Default::default()
        };
        let view_surface = self.view_surface.clone();
        let model = QuickTerminalViewModel {
            configuration,
            appearance,
            theme,
            metrics,
            status: self.status.clone(),
            shortcut: self.shortcuts.shortcut_label(),
            confirmation: self.confirmations.active(),
        };
        let panel = cx
            .open_window(options, move |window, cx| {
                window.on_window_should_close(cx, |_, cx| {
                    cx.update_global::<QuickTerminalRuntime, _>(|runtime, cx| {
                        runtime.close_surface_from_window(cx);
                    });
                    true
                });
                cx.new(|cx| QuickTerminalView::new(view_surface, model, window, cx))
            })
            .map_err(|error| format!("failed to open GPUI Quick Terminal panel: {error}"))?;
        let attached = panel
            .update(cx, |view, window, _| {
                view.panel_properties().map(|_| ())?;
                self.terminal.attach(self.mode, &self.socket_path, window)?;
                self.terminal.set_backdrop(terminal_backdrop);
                Ok(())
            })
            .map_err(|error| format!("failed to configure Quick Terminal panel: {error}"))?;
        if let Err(error) = attached {
            let _ = panel.update(cx, |_, window, _| window.remove_window());
            return Err(error);
        }
        self.panel_generation = next_panel_generation(self.panel_generation, false);
        self.panel = Some(panel);
        self.start_terminal_pumps(cx);
        self.refresh_view(cx);
        Ok(())
    }

    fn schedule_transition(
        &mut self,
        transition: PresentationTransition,
        duration: Duration,
        restores_focus: bool,
        cx: &mut App,
    ) {
        self.presentation_task.take();
        if duration.is_zero() {
            self.complete_transition(transition, restores_focus, cx);
            return;
        }
        self.presentation_task = Some(cx.spawn(async move |cx| {
            cx.background_executor().timer(duration).await;
            let _ = cx.update_global::<Self, _>(|runtime, cx| {
                runtime.complete_transition(transition, restores_focus, cx)
            });
        }));
    }

    fn complete_transition(
        &mut self,
        transition: PresentationTransition,
        restores_focus: bool,
        cx: &mut App,
    ) {
        if !self.presentation.complete(transition) || transition.shows_panel {
            return;
        }
        if let Some(panel) = self.panel {
            let _ = panel.update(cx, |view, _, _| view.finish_hide(restores_focus));
        }
    }

    fn refresh_view(&mut self, cx: &mut App) {
        let (Some(panel), Some(theme)) = (self.panel, self.theme.clone()) else {
            return;
        };
        let configuration = self.configuration;
        let appearance = effective_appearance(configuration, self.accessibility);
        self.terminal
            .set_backdrop(terminal_backdrop(&theme, appearance));
        let metrics = self.metrics;
        let status = self.status.clone();
        let shortcut = self.shortcuts.shortcut_label();
        let model = QuickTerminalViewModel {
            configuration,
            appearance,
            theme,
            metrics,
            status,
            shortcut,
            confirmation: self.confirmations.active(),
        };
        let _ = panel.update(cx, |view, window, cx| {
            view.update_model(model, window);
            cx.notify();
        });
    }

    fn open_settings(&mut self, cx: &mut App) {
        let Some(main_window) = self.main_window else {
            self.status = "Main window is unavailable".to_owned();
            self.refresh_view(cx);
            return;
        };
        if main_window
            .update(cx, |main_window, window, cx| {
                window.activate_window();
                main_window.open_settings(window, cx);
            })
            .is_err()
        {
            self.main_window = None;
            self.status = "Main window is unavailable".to_owned();
        }
    }

    fn set_quick_setting(&mut self, setting: QuickSetting, value: i64, cx: &mut App) {
        let (key, value) = normalized_quick_setting(self.configuration, setting, value);
        self.persist_quick_setting(key, value, cx);
    }

    fn persist_quick_setting(&mut self, key: &'static str, value: i64, cx: &mut App) {
        if let Err(error) = self.apply_live_setting(key, serde_json::json!(value), cx) {
            self.status = error;
            self.refresh_view(cx);
        }
    }

    fn reset_configuration(&mut self, cx: &mut App) -> Result<(), String> {
        let defaults = QuickTerminalConfiguration::default();
        muxy_core::prefs::settings::try_set_many(&[
            (
                "muxy.quickTerminal.width",
                serde_json::json!(defaults.width),
            ),
            (
                "muxy.quickTerminal.height",
                serde_json::json!(defaults.height),
            ),
            (
                "muxy.quickTerminal.transparency",
                serde_json::json!(defaults.transparency),
            ),
            ("muxy.quickTerminal.blur", serde_json::json!(defaults.blur)),
        ])
        .map_err(|error| format!("Failed to reset Quick Terminal settings: {error}"))?;
        self.configuration = QuickTerminalConfiguration {
            enabled: self.configuration.enabled,
            ..defaults
        };
        self.status = "Quick Terminal settings reset".to_owned();
        self.apply_live_configuration(cx);
        Ok(())
    }

    fn release_panel_and_surface(
        &mut self,
        remove_window: bool,
        restores_focus: bool,
        cx: &mut App,
    ) {
        self.presentation_task.take();
        self.destructive_close_pending = false;
        self.deny_pending_confirmations();
        self.wakeup_task.take();
        self.event_task.take();
        self.terminal_shortcut_task.take();
        self.terminal.set_window_active(false);
        self.session.terminate();
        self.view_surface.replace(None);
        if let Some(panel) = self.panel.take()
            && remove_window
        {
            let _ = panel.update(cx, |view, window, _| {
                view.begin_hide(Duration::ZERO, window);
                view.finish_hide(restores_focus);
                window.remove_window();
            });
        }
        self.terminal = StandaloneTerminal::new();
        self.session = QuickTerminalSession::new();
        self.presentation = PanelPresentation::default();
        self.visible = false;
    }

    fn destroy_disabled_runtime(&mut self, cx: &mut App) {
        self.release_panel_and_surface(true, false, cx);
        self.status = "Disabled".to_owned();
    }

    fn start_terminal_pumps(&mut self, cx: &mut App) {
        if let Some(wakeups) = self.terminal.wakeups() {
            self.wakeup_task = Some(cx.spawn(async move |cx| {
                while wakeups.recv().await {
                    if cx
                        .update_global::<Self, _>(|runtime, _| runtime.terminal.tick())
                        .is_err()
                    {
                        return;
                    }
                }
            }));
        }
        if let Some(events) = self.terminal.events() {
            self.event_task = Some(cx.spawn(async move |cx| {
                while let Some(event) = events.recv().await {
                    if cx
                        .update_global::<Self, _>(|runtime, cx| {
                            if let Some(signal) = runtime.terminal.route(event, cx) {
                                runtime.handle_terminal_signal(signal, cx);
                            }
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            }));
        }
        if let Some(shortcuts) = self.terminal.shortcuts() {
            self.terminal_shortcut_task = Some(cx.spawn(async move |cx| {
                while shortcuts.recv().await.is_ok() {
                    if cx
                        .update_global::<Self, _>(|runtime, cx| runtime.close_surface(cx))
                        .is_err()
                    {
                        return;
                    }
                }
            }));
        }
    }

    fn resolve_confirmation(&mut self, id: ConfirmationId, approved: bool, cx: &mut App) {
        let Some(active) = self.confirmations.active() else {
            return;
        };
        if active.id != id {
            return;
        }
        let resolved = self.session.surface().is_some_and(|surface| {
            surface
                .borrow()
                .perform(SurfaceAction::ClipboardDecision { id, approved })
        });
        if !resolved {
            return;
        }
        self.confirmations.resolve(id);
        if active.kind == ConfirmationKind::ActiveProcessClose {
            self.destructive_close_pending = false;
            if approved {
                self.release_panel_and_surface(true, true, cx);
                self.status = "Closed".to_owned();
            } else {
                self.status = "Ready".to_owned();
                self.refresh_view(cx);
            }
        } else {
            self.refresh_view(cx);
        }
    }

    fn deny_pending_confirmations(&mut self) {
        let Some(surface) = self.session.surface().cloned() else {
            self.confirmations.clear();
            return;
        };
        for confirmation in self.confirmations.cancel_all() {
            surface.borrow().perform(SurfaceAction::ClipboardDecision {
                id: confirmation.id,
                approved: false,
            });
        }
    }

    fn submit_notification(&mut self, title: String, body: String, cx: &mut App) {
        let Some(main_window) = self.main_window else {
            return;
        };
        if main_window
            .update(cx, |main_window, _, cx| {
                main_window.submit_quick_terminal_notification(title, body, cx);
            })
            .is_ok()
        {
            self.notification_generation = self.notification_generation.wrapping_add(1);
        } else {
            self.main_window = None;
        }
    }

    fn handle_terminal_signal(&mut self, signal: SurfaceSignal, cx: &mut App) {
        match signal {
            SurfaceSignal::Exited => {
                let generation = self.session.generation();
                if self.session.process_exited(generation) {
                    let destructive_close = self.destructive_close_pending;
                    self.destructive_close_pending = false;
                    self.confirmations.clear();
                    self.view_surface.replace(None);
                    if destructive_close {
                        self.release_panel_and_surface(true, true, cx);
                        self.status = "Closed".to_owned();
                    } else {
                        self.status = "Shell exited".to_owned();
                        self.hide(cx);
                    }
                }
            }
            SurfaceSignal::Confirm { id, kind } => {
                if self.visible && self.panel.is_some() {
                    self.confirmations.enqueue(ConfirmationPrompt { id, kind });
                } else if let Some(surface) = self.session.surface() {
                    surface.borrow().perform(SurfaceAction::ClipboardDecision {
                        id,
                        approved: false,
                    });
                }
            }
            SurfaceSignal::DesktopNotification { title, body } => {
                self.submit_notification(title, body, cx);
            }
            signal @ SurfaceSignal::Metadata(_) => {
                if let Some(surface) = self.session.surface_mut() {
                    surface.borrow_mut().apply(signal);
                }
            }
        }
        self.refresh_view(cx);
        if let Some(panel) = self.panel {
            let _ = panel.update(cx, |_, _, cx| cx.notify());
        }
    }

    fn start_staged_control_pump(&mut self, cx: &mut App) {
        let path = muxy_core::prefs::app_support_dir().join(STAGED_SPIKE_CONTROL_FILE);
        self.staged_control_task = Some(cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                let _ = std::fs::remove_file(&path);
                let Some(control) = StagedSpikeControl::parse(&bytes) else {
                    continue;
                };
                if cx
                    .update_global::<Self, _>(|runtime, cx| {
                        runtime.apply_staged_control(control, cx)
                    })
                    .is_err()
                {
                    return;
                }
            }
        }));
    }

    fn apply_staged_control(&mut self, control: StagedSpikeControl, cx: &mut App) {
        let result = match control.command.as_str() {
            "status" => Ok(()),
            "show" => self.show(cx),
            "hide" => {
                self.hide(cx);
                Ok(())
            }
            "sendText" => {
                let text = control
                    .text
                    .as_deref()
                    .ok_or_else(|| "sendText requires text".to_owned());
                text.and_then(|text| {
                    self.session
                        .surface()
                        .filter(|surface| surface.borrow().send_text(text))
                        .map(|_| ())
                        .ok_or_else(|| "Quick Terminal surface rejected text".to_owned())
                })
            }
            "sendBytes" => {
                let bytes = control
                    .bytes
                    .as_deref()
                    .filter(|bytes| bytes.len() <= 4096)
                    .ok_or_else(|| "sendBytes requires at most 4096 bytes".to_owned());
                bytes.and_then(|bytes| {
                    self.session
                        .surface()
                        .filter(|surface| surface.borrow().send_bytes(bytes))
                        .map(|_| ())
                        .ok_or_else(|| "Quick Terminal surface rejected bytes".to_owned())
                })
            }
            "sendLine" => {
                let text = control
                    .text
                    .as_deref()
                    .ok_or_else(|| "sendLine requires text".to_owned());
                text.and_then(|text| {
                    let surface = self
                        .session
                        .surface()
                        .ok_or_else(|| "Quick Terminal surface is unavailable".to_owned())?
                        .borrow();
                    if surface.send_bytes(&[21])
                        && surface.send_text(text)
                        && surface.send_bytes(&[13])
                    {
                        Ok(())
                    } else {
                        Err("Quick Terminal surface rejected line".to_owned())
                    }
                })
            }
            "reload" => {
                self.session.reload(|_| self.terminal.reload_config());
                Ok(())
            }
            "setWidth" => control
                .text
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .map(serde_json::Value::from)
                .ok_or_else(|| "setWidth requires an integer".to_owned())
                .and_then(|value| self.apply_live_setting("muxy.quickTerminal.width", value, cx)),
            "setTransparency" => control
                .text
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .map(serde_json::Value::from)
                .ok_or_else(|| "setTransparency requires an integer".to_owned())
                .and_then(|value| {
                    self.apply_live_setting("muxy.quickTerminal.transparency", value, cx)
                }),
            "applyJson" => control
                .text
                .as_deref()
                .ok_or_else(|| "applyJson requires text".to_owned())
                .and_then(|text| self.apply_json_settings(text, cx)),
            "close" => {
                self.bridge_action(BridgeAction::Close, cx);
                Ok(())
            }
            "closeSurface" => {
                self.close_surface(cx);
                Ok(())
            }
            "approveConfirmation" | "denyConfirmation" => {
                let confirmation = self
                    .confirmations
                    .active()
                    .ok_or_else(|| "Quick Terminal confirmation is unavailable".to_owned());
                confirmation.map(|confirmation| {
                    self.resolve_confirmation(
                        confirmation.id,
                        control.command == "approveConfirmation",
                        cx,
                    );
                })
            }
            "rapidShowHideShow" => self.show(cx).and_then(|_| {
                self.hide(cx);
                self.show(cx)
            }),
            "disable" => {
                self.apply_live_setting("muxy.quickTerminal.enabled", serde_json::json!(false), cx)
            }
            "enable" => {
                self.apply_live_setting("muxy.quickTerminal.enabled", serde_json::json!(true), cx)
            }
            "reset" => self.reset_configuration(cx),
            "requestInputMonitoring" => {
                self.shortcuts.request_input_monitoring_access();
                self.refresh_view(cx);
                Ok(())
            }
            "refreshInputMonitoring" => {
                self.shortcuts.refresh_input_monitoring_access();
                self.refresh_view(cx);
                Ok(())
            }
            "setAccessibilityOpaque" => {
                let enabled = control.text.as_deref() != Some("false");
                self.accessibility.reduce_transparency = enabled;
                self.refresh_view(cx);
                Ok(())
            }
            "quit" => {
                cx.quit();
                Ok(())
            }
            _ => Err(format!(
                "unknown staged Quick Terminal command: {}",
                control.command
            )),
        };
        let case_name = std::env::var(STAGED_SPIKE_ENV).unwrap_or_else(|_| "spike".to_owned());
        let status = self.staged_status(
            &case_name,
            control.id,
            result.err(),
            control.last_lines.unwrap_or(200),
            cx,
        );
        self.write_staged_status(&status);
    }

    fn staged_status(
        &self,
        case_name: &str,
        control_id: u64,
        error: Option<String>,
        last_lines: usize,
        cx: &mut App,
    ) -> serde_json::Value {
        let mut status = self.panel_status(cx);
        let Some(object) = status.as_object_mut() else {
            return status;
        };
        object.insert("case".to_owned(), serde_json::json!(case_name));
        object.insert("controlId".to_owned(), serde_json::json!(control_id));
        object.insert("runtimeStatus".to_owned(), serde_json::json!(self.status));
        object.insert("terminated".to_owned(), serde_json::json!(self.terminated));
        object.insert(
            "screenText".to_owned(),
            serde_json::json!(self.session.surface().and_then(|surface| {
                surface.borrow().read_screen_text(last_lines.clamp(1, 500))
            })),
        );
        if let Some(error) = error {
            object.insert("result".to_owned(), serde_json::json!("error"));
            object.insert("error".to_owned(), serde_json::json!(error));
        }
        status
    }

    #[cfg(target_os = "macos")]
    fn panel_status(&self, cx: &mut App) -> serde_json::Value {
        let Some(panel) = self.panel else {
            return serde_json::json!({
                "case": "panel",
                "result": "success",
                "enabled": self.configuration.enabled,
                "visible": false,
                "nativeVisible": false,
                "hasPanel": false,
                "hasSurface": false,
                "hasWakeupTask": self.wakeup_task.is_some(),
                "hasEventTask": self.event_task.is_some(),
                "pendingConfirmations": self.confirmations.len(),
                "notificationGeneration": self.notification_generation,
            });
        };
        match panel.update(cx, |view, _, _| {
            view.panel_properties().map(|properties| {
                (
                    properties,
                    view.is_visible(),
                    view.is_key(),
                    view.telemetry(),
                    view.native_frame(),
                )
            })
        }) {
            Ok(Ok((properties, native_visible, native_key, telemetry, native_frame))) => {
                let appearance = effective_appearance(self.configuration, self.accessibility);
                let appearance_name = match appearance.background {
                    gpui::WindowBackgroundAppearance::Opaque => "opaque",
                    gpui::WindowBackgroundAppearance::Transparent => "transparent",
                    gpui::WindowBackgroundAppearance::Blurred => "blurred",
                };
                let frame = telemetry.as_ref().map(|value| {
                    serde_json::json!({
                        "x": value.frame.origin.x,
                        "y": value.frame.origin.y,
                        "width": value.frame.size.width,
                        "height": value.frame.size.height,
                    })
                });
                let collapsed_cutout = telemetry
                    .as_ref()
                    .and_then(|value| value.collapsed_cutout)
                    .map(|rect| {
                        serde_json::json!({
                            "x": rect.origin.x,
                            "y": rect.origin.y,
                            "width": rect.size.width,
                            "height": rect.size.height,
                        })
                    });
                let native_frame = native_frame.map(|rect| {
                    serde_json::json!({
                        "x": rect.origin.x,
                        "y": rect.origin.y,
                        "width": rect.size.width,
                        "height": rect.size.height,
                    })
                });
                let mut status = serde_json::json!({
                    "case": "panel",
                    "result": "success",
                    "enabled": self.configuration.enabled,
                    "visible": self.visible,
                    "nativeVisible": native_visible,
                    "nativeKey": native_key,
                    "hasPanel": true,
                    "borderless": properties.borderless,
                    "nonactivating": properties.nonactivating,
                    "statusLevel": properties.status_level,
                    "joinsAllSpaces": properties.joins_all_spaces,
                    "fullScreenAuxiliary": properties.full_screen_auxiliary,
                    "ignoresCycle": properties.ignores_cycle,
                    "floating": properties.floating,
                    "visibleOnDeactivate": properties.visible_on_deactivate,
                    "movable": properties.movable,
                    "transparent": properties.transparent,
                    "keyCapable": properties.key_capable,
                    "mainCapable": properties.main_capable,
                });
                let surface_generation = self.session.generation();
                let foreground_pid = self
                    .session
                    .surface()
                    .and_then(|surface| surface.borrow().foreground_pid());
                let lifecycle = serde_json::json!({
                    "panelGeneration": self.panel_generation,
                    "surfaceGeneration": surface_generation,
                    "hasSurface": self.session.has_surface(),
                    "hasWakeupTask": self.wakeup_task.is_some(),
                    "hasEventTask": self.event_task.is_some(),
                    "targetVisible": self.presentation.target_is_visible(),
                    "foregroundPid": foreground_pid,
                    "foregroundProcessIdentity": surface_process_identity(surface_generation, foreground_pid),
                    "configuredWidth": self.configuration.width,
                    "configuredHeight": self.configuration.height,
                    "storedTransparency": self.configuration.transparency,
                    "storedBlur": self.configuration.blur,
                    "shortcut": self.shortcuts.shortcut_label(),
                    "monitoring": self.shortcuts.monitoring_label(),
                    "pendingConfirmations": self.confirmations.len(),
                    "notificationGeneration": self.notification_generation,
                });
                let policy = serde_json::json!({
                    "effectiveTransparency": appearance.effective_transparency,
                    "effectiveBlur": appearance.effective_blur,
                    "appearance": appearance_name,
                    "reduceMotion": self.accessibility.reduce_motion,
                    "reduceTransparency": self.accessibility.reduce_transparency,
                    "increaseContrast": self.accessibility.increase_contrast,
                    "accessibilityNodeCount": 12,
                    "screenIndex": telemetry.as_ref().map(|value| value.screen_index),
                    "screenName": telemetry.as_ref().map(|value| value.screen_name.clone()),
                    "activeSpaceIntent": telemetry.as_ref().is_some_and(|value| value.active_space_intent),
                    "frame": frame,
                    "nativeFrame": native_frame,
                    "collapsedCutout": collapsed_cutout,
                });
                let object = status
                    .as_object_mut()
                    .expect("panel status must be an object");
                object.extend(
                    lifecycle
                        .as_object()
                        .expect("panel lifecycle must be an object")
                        .clone(),
                );
                object.extend(
                    policy
                        .as_object()
                        .expect("panel policy must be an object")
                        .clone(),
                );
                status
            }
            Ok(Err(error)) => serde_json::json!({
                "case": "panel",
                "result": "error",
                "error": error,
            }),
            Err(error) => serde_json::json!({
                "case": "panel",
                "result": "error",
                "error": error.to_string(),
            }),
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn panel_status(&self, _cx: &mut App) -> serde_json::Value {
        serde_json::json!({
            "case": "panel",
            "result": "error",
            "error": "Quick Terminal panels are unavailable on this platform",
        })
    }

    fn write_staged_status(&self, status: &serde_json::Value) {
        let path = muxy_core::prefs::app_support_dir().join(STAGED_SPIKE_STATUS_FILE);
        if let Err(error) = muxy_core::store::write_private(
            &path,
            serde_json::to_string_pretty(status)
                .expect("staged panel status must encode")
                .as_bytes(),
        ) {
            log::warn!("failed to write staged Quick Terminal panel status: {error}");
        }
    }
}

fn next_panel_generation(current: u64, has_panel: bool) -> u64 {
    if has_panel {
        current
    } else {
        current.saturating_add(1).max(1)
    }
}

fn surface_process_identity(generation: u64, pid: Option<u64>) -> Option<String> {
    pid.map(|pid| format!("{generation}:{pid}"))
}

fn refreshed_keyboard_layout_shortcut(
    current: &QuickTerminalShortcut,
    key_resolver: impl FnMut(u16) -> Option<String>,
) -> Option<QuickTerminalShortcut> {
    current
        .canonicalized(key_resolver)
        .filter(|shortcut| shortcut != current)
}

fn normalized_quick_setting(
    mut configuration: QuickTerminalConfiguration,
    setting: QuickSetting,
    value: i64,
) -> (&'static str, i64) {
    let key = match setting {
        QuickSetting::Width => {
            configuration.width = value;
            "muxy.quickTerminal.width"
        }
        QuickSetting::Height => {
            configuration.height = value;
            "muxy.quickTerminal.height"
        }
        QuickSetting::Transparency => {
            configuration.transparency = value;
            "muxy.quickTerminal.transparency"
        }
        QuickSetting::Blur => {
            configuration.blur = value;
            "muxy.quickTerminal.blur"
        }
    };
    let configuration = configuration.normalized();
    let value = match setting {
        QuickSetting::Width => configuration.width,
        QuickSetting::Height => configuration.height,
        QuickSetting::Transparency => configuration.transparency,
        QuickSetting::Blur => configuration.blur,
    };
    (key, value)
}

fn quick_terminal_setting_value(
    configuration: QuickTerminalConfiguration,
    key: &str,
) -> Option<serde_json::Value> {
    match key {
        "muxy.quickTerminal.enabled" => Some(serde_json::json!(configuration.enabled)),
        "muxy.quickTerminal.width" => Some(serde_json::json!(configuration.width)),
        "muxy.quickTerminal.height" => Some(serde_json::json!(configuration.height)),
        "muxy.quickTerminal.transparency" => Some(serde_json::json!(configuration.transparency)),
        "muxy.quickTerminal.blur" => Some(serde_json::json!(configuration.blur)),
        _ => None,
    }
}

fn terminal_backdrop(theme: &Theme, appearance: super::panel::EffectiveAppearance) -> gpui::Rgba {
    let mut tint = theme.bg;
    tint.a = f32::from(appearance.tint_alpha_percent) / 100.0;
    tint.into()
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_owned();
    }
    "Quick Terminal panel adapter panicked".to_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        ConfirmationPrompt, PendingConfirmations, SHUTDOWN_ORDER, STAGED_SPIKE_ENV, ShutdownStep,
        next_panel_generation, normalized_quick_setting, refreshed_keyboard_layout_shortcut,
        surface_process_identity,
    };
    use crate::quick_terminal::panel::QuickTerminalConfiguration;
    use crate::quick_terminal::view::QuickSetting;
    use crate::terminal::ConfirmationKind;
    use muxy_core::quick_terminal::QuickTerminalShortcut;
    use muxy_core::shortcuts::{COMMAND, KeyCombo};
    use muxy_terminal::confirmation::ConfirmationQueue;

    #[test]
    fn quick_terminal_window_staged_spike_has_an_isolated_name() {
        assert_eq!(STAGED_SPIKE_ENV, "MUXY_TEST_P6_SPIKE_CASE");
    }

    #[test]
    fn quick_terminal_window_hide_show_retains_the_panel_generation() {
        let first = next_panel_generation(0, false);
        assert_eq!(first, 1);
        assert_eq!(next_panel_generation(first, true), first);
    }

    #[test]
    fn quick_terminal_surface_process_identity_is_generation_owned() {
        assert_eq!(
            surface_process_identity(4, Some(42)),
            Some("4:42".to_owned())
        );
        assert_eq!(surface_process_identity(4, None), None);
    }

    #[test]
    fn quick_terminal_keyboard_layout_refresh_updates_only_the_display_key() {
        let current = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("a", COMMAND),
            virtual_key_code: 0,
        };
        assert!(refreshed_keyboard_layout_shortcut(&current, |_| Some("a".to_owned())).is_none());
        let refreshed = refreshed_keyboard_layout_shortcut(&current, |_| Some("q".to_owned()))
            .expect("layout change should update the display key");
        assert_eq!(refreshed.key_combo(), Some(&KeyCombo::new("q", COMMAND)));
        assert_eq!(
            refreshed.registration_identity(),
            current.registration_identity()
        );
        assert!(refreshed_keyboard_layout_shortcut(&current, |_| None).is_none());
    }

    #[test]
    fn quick_terminal_panel_slider_values_use_live_keys_and_bounds() {
        let defaults = QuickTerminalConfiguration::default();
        assert_eq!(
            normalized_quick_setting(defaults, QuickSetting::Width, 960),
            ("muxy.quickTerminal.width", 960)
        );
        assert_eq!(
            normalized_quick_setting(defaults, QuickSetting::Height, 558),
            ("muxy.quickTerminal.height", 558)
        );
        assert_eq!(
            normalized_quick_setting(defaults, QuickSetting::Transparency, 100),
            ("muxy.quickTerminal.transparency", 55)
        );
        assert_eq!(
            normalized_quick_setting(defaults, QuickSetting::Blur, -100),
            ("muxy.quickTerminal.blur", 0)
        );
    }

    #[test]
    fn quick_terminal_confirmations_are_ordered_deduplicated_and_cancelled_together() {
        let mut ids = ConfirmationQueue::new();
        let first = ids.enqueue(ConfirmationKind::Osc52Write, ());
        let second = ids.enqueue(ConfirmationKind::ActiveProcessClose, ());
        let mut pending = PendingConfirmations::default();
        pending.enqueue(ConfirmationPrompt {
            id: first,
            kind: ConfirmationKind::Osc52Write,
        });
        pending.enqueue(ConfirmationPrompt {
            id: first,
            kind: ConfirmationKind::Osc52Write,
        });
        pending.enqueue(ConfirmationPrompt {
            id: second,
            kind: ConfirmationKind::ActiveProcessClose,
        });
        assert_eq!(pending.active().map(|prompt| prompt.id), Some(first));
        assert!(pending.resolve(second).is_none());
        assert_eq!(pending.resolve(first).map(|prompt| prompt.id), Some(first));
        assert_eq!(
            pending
                .cancel_all()
                .into_iter()
                .map(|prompt| prompt.id)
                .collect::<Vec<_>>(),
            vec![second]
        );
        assert!(pending.active().is_none());
    }

    #[test]
    fn quick_terminal_window_main_close_uses_whole_app_shutdown_order() {
        assert_eq!(
            SHUTDOWN_ORDER,
            [
                ShutdownStep::StopTriggers,
                ShutdownStep::HidePanel,
                ShutdownStep::StopPumps,
                ShutdownStep::TerminateSession,
                ShutdownStep::ClosePanel,
                ShutdownStep::StopShortcuts,
            ]
        );
    }
}
