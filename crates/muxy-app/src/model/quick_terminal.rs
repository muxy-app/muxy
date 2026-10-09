use gpui::{
    AppContext, Bounds, Context, Task, WindowBounds, WindowHandle, WindowKind, WindowOptions, px,
    size,
};
use muxy_app_core::{PaneId, ServerId, TabId};
use muxy_core::quick_terminal::keys::{COMMAND, CONTROL, KeyCombo, OPTION, SHIFT};
use muxy_core::quick_terminal::{ConflictCandidate, QuickTerminalShortcut};
use muxy_core::shortcuts::ShortcutSettings;
use muxy_ui::quick_terminal::{
    panel::{
        AccessibilityPreferences, PanelPresentation, QuickTerminalConfiguration,
        effective_appearance, transition_duration,
    },
    platform::{self, SystemMutation},
    shortcut_service::QuickTerminalShortcutService,
};
use muxy_ui::tr;

use super::{AppModel, ConnectionState, Quitting};
use crate::boot::Work;
use crate::views::quick_terminal::{QuickTerminalView, QuickTerminalViewModel};

pub(crate) struct QuickTerminalRuntime {
    pub(crate) panel: Option<WindowHandle<QuickTerminalView>>,
    pub(crate) visible: bool,
    shortcuts: QuickTerminalShortcutService,
    accessibility: AccessibilityPreferences,
    presentation: PanelPresentation,
    transition: Option<Task<()>>,
    _triggers: Task<()>,
    _system: Task<()>,
    _observers: Option<platform::SystemObservers>,
    pub(crate) closing: Option<(TabId, PaneId)>,
}

impl QuickTerminalRuntime {
    fn accessibility() -> AccessibilityPreferences {
        #[cfg(all(target_os = "macos", not(test)))]
        {
            platform::macos::accessibility_preferences()
        }
        #[cfg(any(not(target_os = "macos"), test))]
        {
            AccessibilityPreferences::default()
        }
    }

    pub(super) fn new(
        settings: &muxy_app_core::settings::QuickTerminalSettings,
        cx: &mut Context<AppModel>,
    ) -> Self {
        let mut shortcuts = QuickTerminalShortcutService::new(
            settings.shortcut.clone(),
            settings.enabled,
            platform::factory(),
            Box::new(|_| Ok(())),
            Box::new(platform::resolve_key),
        );
        // Settings owns persistence; a prepared native registration commits only after a successful save.
        let _ = shortcuts.start();
        let triggers = shortcuts.trigger_receiver();
        let trigger_task = cx.spawn(async move |model, cx| {
            while triggers.recv().await.is_ok() {
                if model.update(cx, AppModel::toggle_quick_terminal).is_err() {
                    break;
                }
            }
        });
        let (sender, receiver) = async_channel::bounded(16);
        #[cfg(not(test))]
        let observers = platform::observe_system_mutations(sender).ok();
        #[cfg(test)]
        let observers = {
            drop(sender);
            None
        };
        let system_task = cx.spawn(async move |model, cx| {
            while let Ok(mutation) = receiver.recv().await {
                let _ = model.update(cx, |model, cx| {
                    match mutation {
                        SystemMutation::Accessibility => {
                            model.quick.accessibility = Self::accessibility();
                            cx.notify();
                        }
                        SystemMutation::KeyboardLayout => {
                            if let Some(shortcut) = model
                                .quick
                                .shortcuts
                                .shortcut()
                                .canonicalized(platform::resolve_key)
                            {
                                let mut settings = model.settings.quick_terminal.clone();
                                settings.shortcut = shortcut;
                                if let Err(error) = model.apply_quick_settings(settings, cx) {
                                    model.fail(error, cx);
                                }
                            }
                        }
                        SystemMutation::Screens => {}
                    }
                    model.refresh_quick_terminal(cx);
                    model.prepare_quick_terminal(cx);
                });
            }
        });
        Self {
            panel: None,
            visible: false,
            shortcuts,
            accessibility: Self::accessibility(),
            presentation: PanelPresentation::default(),
            transition: None,
            _triggers: trigger_task,
            _system: system_task,
            _observers: observers,
            closing: None,
        }
    }
}

impl AppModel {
    #[cfg(all(target_os = "macos", not(test)))]
    pub(crate) fn terminal_effects_allowed(&self) -> bool {
        !self.quick.accessibility.reduce_transparency && !self.quick.accessibility.increase_contrast
    }

    pub(crate) fn is_quick_terminal(&self, pane: PaneId) -> bool {
        self.state
            .quick_terminal()
            .is_some_and(|quick| quick.id == pane)
    }

    pub(super) fn attached_panes(&self) -> Vec<PaneId> {
        let mut panes = self.visible_panes();
        if self.quick.visible {
            panes.extend(self.state.quick_terminal().map(|pane| pane.id));
        }
        panes
    }

    fn quick_configuration(&self) -> QuickTerminalConfiguration {
        let settings = &self.settings.quick_terminal;
        QuickTerminalConfiguration {
            enabled: settings.enabled,
            width: i64::from(settings.width),
            height: i64::from(settings.height),
            transparency: i64::from(settings.transparency),
            blur: i64::from(settings.blur),
        }
    }

    fn quick_view_model(&self) -> QuickTerminalViewModel {
        let configuration = self.quick_configuration();
        QuickTerminalViewModel {
            configuration,
            appearance: effective_appearance(configuration, self.quick.accessibility),
            theme: self.theme.clone(),
            metrics: self.metrics,
            status: match self.connection(ServerId::local()) {
                ConnectionState::Connecting => tr!("Connecting…"),
                ConnectionState::Disconnected => tr!("Disconnected"),
                ConnectionState::Ready => tr!("Ready"),
            }
            .to_string(),
            shortcut: self.quick_shortcut_label(),
        }
    }

    pub(crate) fn quick_shortcut_label(&self) -> String {
        match self.quick.shortcuts.shortcut() {
            QuickTerminalShortcut::Unassigned => tr!("Unassigned").to_string(),
            QuickTerminalShortcut::KeyCombo { key_combo, .. } => key_combo.display(),
        }
    }

    pub(crate) fn quick_shortcut_status(&self) -> String {
        use muxy_ui::quick_terminal::shortcut_service::ShortcutState;
        self.quick.shortcuts.error_message().map_or_else(
            || {
                match self.quick.shortcuts.state() {
                    ShortcutState::Stopped => tr!("Inactive"),
                    ShortcutState::Unavailable => tr!("Unavailable"),
                    ShortcutState::Registered => tr!("Active system-wide"),
                }
                .to_string()
            },
            str::to_owned,
        )
    }

    pub(crate) fn toggle_quick_terminal(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle || !self.settings.quick_terminal.enabled {
            return;
        }
        if self.quick.visible {
            self.hide_quick_terminal(true, cx);
        } else if let Err(error) = self.show_quick_terminal(cx) {
            self.fail(error, cx);
        }
    }

    fn show_quick_terminal(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.connection(ServerId::local()) == ConnectionState::Disconnected {
            self.connect(cx);
        }
        if self.quick.panel.is_none() {
            let model = self.quick_view_model();
            let owner = cx.weak_entity();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(
                        px(f32::from(self.settings.quick_terminal.width)),
                        px(f32::from(self.settings.quick_terminal.height)),
                    ),
                    cx,
                ))),
                titlebar: None,
                focus: false,
                show: false,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: model.appearance.background,
                ..Default::default()
            };
            let panel = cx
                .open_window(options, |window, cx| {
                    window.set_window_title(&tr!("Muxy Quick Terminal"));
                    let closing_owner = owner.clone();
                    window.on_window_should_close(cx, move |_, cx| {
                        let owner = closing_owner.clone();
                        cx.defer(move |cx| {
                            let _ = owner.update(cx, AppModel::request_close_quick_terminal);
                        });
                        false
                    });
                    cx.new(|cx| QuickTerminalView::new(owner, model, window, cx))
                })
                .map_err(|error| error.to_string())?;
            self.quick.panel = Some(panel);
        }
        let Some(panel) = self.quick.panel else {
            return Err(tr!("Quick Terminal panel unavailable").to_string());
        };
        panel
            .update(cx, |view, window, _| view.prepare(window))
            .map_err(|error| error.to_string())??;
        self.state.ensure_quick_terminal();
        if !self.save(cx) {
            return Err(tr!("Could not save Quick Terminal session").to_string());
        }
        self.quick.visible = true;
        self.sync_visible(cx);
        self.refresh_quick_terminal(cx);
        let duration = transition_duration(true, self.quick.accessibility.reduce_motion);
        let transition = self.quick.presentation.request(true);
        panel
            .update(cx, |view, window, cx| {
                view.begin_show(duration, window, cx);
                view.focus_terminal(window, cx);
                cx.notify();
            })
            .map_err(|error| error.to_string())?;
        self.quick.transition = None;
        if let Some(transition) = transition {
            self.quick.presentation.complete(transition);
        }
        Ok(())
    }

    pub(crate) fn hide_quick_terminal(&mut self, restore_focus: bool, cx: &mut Context<Self>) {
        self.quick.visible = false;
        self.quick.closing = None;
        let Some(panel) = self.quick.panel else {
            return;
        };
        let Some(transition) = self.quick.presentation.request(false) else {
            return;
        };
        let duration = transition_duration(false, self.quick.accessibility.reduce_motion);
        let _ = panel.update(cx, |view, window, cx| {
            view.begin_hide(duration, window);
            cx.notify();
        });
        if let Some(pane) = self
            .state
            .quick_terminal()
            .and_then(|pane| self.terminal(&pane.id))
        {
            pane.view.update(cx, |pane, cx| pane.set_focused(false, cx));
        }
        self.quick.transition = Some(cx.spawn(async move |model, cx| {
            cx.background_executor().timer(duration).await;
            let _ = model.update(cx, |model, cx| {
                if model.quick.presentation.complete(transition) {
                    let _ = panel.update(cx, |view, _, _| view.finish_hide(restore_focus));
                    model.sync_visible(cx);
                    model.refresh_quick_terminal(cx);
                }
            });
        }));
    }

    pub(super) fn prepare_quick_terminal(&mut self, cx: &mut Context<Self>) {
        if self.quick.visible
            && let Some(panel) = self.quick.panel
        {
            let _ = panel.update(cx, |view, window, _| view.prepare(window));
        }
    }

    pub(crate) fn refresh_quick_terminal(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.quick.panel else {
            return;
        };
        let model = self.quick_view_model();
        let terminal = self
            .state
            .quick_terminal()
            .and_then(|pane| self.terminal(&pane.id))
            .map(|pane| pane.view.clone());
        if let Some(pane) = &terminal {
            let alpha = f32::from(model.appearance.tint_alpha_percent) / 100.0;
            pane.update(cx, |pane, cx| {
                pane.background_opacity = alpha;
                cx.notify();
            });
        }
        let _ = panel.update(cx, |view, window, cx| {
            view.update_model(model, terminal, window);
            cx.notify();
        });
    }

    pub(crate) fn open_quick_terminal_settings(&mut self, cx: &mut Context<Self>) {
        self.hide_quick_terminal(false, cx);
        self.show_settings(cx);
        if let Some(settings) = &self.settings_window {
            settings.view.update(
                cx,
                super::super::views::settings::SettingsView::show_quick_terminal,
            );
        }
    }

    pub(super) fn quick_terminal_menu(
        &self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some(panel) = self.quick.panel {
            let _ = panel.update(cx, |view, window, cx| view.open_menu(position, window, cx));
        }
    }

    pub(crate) fn close_quick_terminal(&mut self, cx: &mut Context<Self>) {
        self.hide_quick_terminal(true, cx);
        self.state.close_quick_terminal();
        if self.save(cx) {
            self.discard_pending(ServerId::local(), cx);
        }
    }

    pub(super) fn restore_quick_terminal(
        &mut self,
        sessions: &[muxy_protocol::SessionInfo],
        cx: &mut Context<Self>,
    ) {
        let stale = self.state.quick_terminal().is_some_and(|pane| {
            self.pane_session(pane.id)
                .is_some_and(|id| !sessions.iter().any(|session| session.id == id))
        });
        if stale || !self.settings.quick_terminal.enabled {
            self.close_quick_terminal(cx);
        }
    }

    pub(crate) fn request_close_quick_terminal(&mut self, cx: &mut Context<Self>) {
        if self.quick.closing.is_some() {
            return;
        }
        let Some(pane) = self.state.quick_terminal().map(|pane| pane.id) else {
            return;
        };
        let Some(session) = self.pane_session(pane) else {
            self.close_quick_terminal(cx);
            return;
        };
        if self.session_used_outside(ServerId::local(), session, &[pane])
            || !self.settings.window.confirm_running_process
        {
            self.close_quick_terminal(cx);
            return;
        }
        let tab = TabId::new();
        self.quick.closing = Some((tab, pane));
        if !self.ready(ServerId::local()) {
            let process = self
                .terminal(&pane)
                .and_then(|pane| pane.view.read(cx).process.clone());
            self.check_quick_close(Ok(process), cx);
            return;
        }

        let viewport = self
            .terminal(&pane)
            .and_then(|pane| pane.view.read(cx).viewport())
            .unwrap_or(muxy_protocol::Size { cols: 80, rows: 24 });
        if !self.send(
            ServerId::local(),
            Work::CheckClose {
                tab,
                session,
                size: viewport,
            },
            cx,
        ) {
            self.quick.closing = None;
        }
    }

    pub(super) fn check_quick_close(
        &mut self,
        result: Result<Option<muxy_protocol::ForegroundProcess>, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Some(process)) if !process.is_shell => {
                if let Some(panel) = self.quick.panel {
                    let _ = panel.update(cx, QuickTerminalView::show_close_confirmation);
                }
            }
            Ok(_) => self.close_quick_terminal(cx),
            Err(error) if crate::boot::missing_session(&error) => self.close_quick_terminal(cx),
            Err(error) => {
                self.quick.closing = None;
                self.fail(error.to_string(), cx);
            }
        }
    }

    pub(crate) fn apply_quick_settings(
        &mut self,
        settings: muxy_app_core::settings::QuickTerminalSettings,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        settings.validate().map_err(|error| error.to_string())?;
        let mut conflicts = shortcut_conflicts(&self.settings.keymap);
        conflicts.extend(self.custom_bindings().filter_map(|(command, chord)| {
            combo_from_chord(chord.as_str()).map(|combo| ConflictCandidate {
                label: format!("command {}", command.id),
                combo,
            })
        }));
        let prepared = self
            .quick
            .shortcuts
            .prepare_shortcut_for_enabled(settings.shortcut.clone(), &conflicts, settings.enabled)
            .map_err(|error| error.to_string())?;
        if let Err(error) = settings.save(&self.path.with_file_name("settings.toml")) {
            self.quick.shortcuts.cancel_prepared(prepared);
            return Err(error.to_string());
        }
        self.quick.shortcuts.commit_prepared(prepared);
        self.settings.quick_terminal = settings;
        if !self.settings.quick_terminal.enabled {
            self.close_quick_terminal(cx);
        }
        self.refresh_quick_terminal(cx);
        self.prepare_quick_terminal(cx);
        self.sync_preferences(cx);
        Ok(())
    }

    pub(super) fn validate_quick_conflict(
        &self,
        chord: &muxy_app_core::settings::KeyChord,
    ) -> Result<(), String> {
        if let Some(combo) = combo_from_chord(chord.as_str())
            && self
                .quick
                .shortcuts
                .shortcut()
                .key_combo()
                .is_some_and(|quick| quick.conflicts_with(&combo))
        {
            return Err(tr!("Shortcut conflicts with Quick Terminal").to_string());
        }
        Ok(())
    }
}

fn combo_from_chord(chord: &str) -> Option<KeyCombo> {
    let mut rest = chord;
    let mut flags = 0;
    while let Some((modifier, tail)) = rest.split_once('-') {
        flags |= match modifier {
            "cmd" => COMMAND,
            "ctrl" => CONTROL,
            "alt" => OPTION,
            "shift" => SHIFT,
            _ => break,
        };
        rest = tail;
    }
    (!rest.is_empty()).then(|| KeyCombo::new(rest, flags).canonicalized())
}

fn shortcut_conflicts(keymap: &muxy_app_core::settings::Keymap) -> Vec<ConflictCandidate> {
    muxy_core::shortcuts::ALL
        .iter()
        .flat_map(|shortcut| {
            shortcut.contexts.iter().flat_map(move |context| {
                keymap
                    .keys(shortcut.id, *context)
                    .into_iter()
                    .filter_map(move |chord| {
                        combo_from_chord(&chord).map(|combo| ConflictCandidate {
                            label: shortcut.id.replace('_', " "),
                            combo,
                        })
                    })
            })
        })
        .collect()
}
