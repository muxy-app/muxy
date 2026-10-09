use gpui::{
    AppContext, Bounds, Context, Entity, TitlebarOptions, Window, WindowBounds, WindowHandle,
    WindowOptions, point, px, size,
};
use muxy_app_core::ServerId;
use muxy_ui::tr;

use super::{AppModel, Quitting};
use crate::boot::Work;
use crate::views::settings::window::SettingsWindow;
use crate::views::settings::{Change, SettingsView, Snapshot};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Default)]
pub(super) struct ServerPreferences {
    pub(super) document: Option<muxy_protocol::ServerSettingsDoc>,
    pub(super) busy: bool,
    pub(super) control_busy: bool,
    pub(super) row: String,
    pub(super) pending: std::collections::VecDeque<Change>,
}

pub(crate) struct SettingsWindowState {
    pub(crate) window: WindowHandle<SettingsWindow>,
    pub(crate) view: Entity<SettingsView>,
}

pub(super) struct TerminalPreview {
    settings: muxy_app_core::settings::TerminalSettings,
    zoom: std::collections::HashMap<super::PaneId, f32>,
}

#[derive(Default)]
pub(super) struct TerminalBackground {
    pub(super) available: bool,
    #[cfg(all(target_os = "macos", not(test)))]
    native: Option<muxy_ui::vibrancy::TerminalVibrancy>,
}

impl AppModel {
    pub(crate) fn open_settings(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.show_settings(cx);
    }

    pub(super) fn show_settings(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle || self.close_prompt.is_some() {
            return;
        }
        if let Some(settings) = &self.settings_window
            && settings
                .window
                .update(cx, |root, window, cx| {
                    window.activate_window();
                    root.focus(window, cx);
                })
                .is_ok()
        {
            return;
        }
        self.retain_extension_updates(cx);
        self.settings_window = None;
        let snapshot = self.preferences_snapshot();
        self.refresh_theme(cx);
        let model = cx.weak_entity();
        let extensions = self.pending_extension_updates.take().unwrap_or_else(|| {
            cx.new(|cx| {
                crate::views::settings::extensions::ExtensionsView::new(
                    model,
                    self.theme.clone(),
                    muxy_ui::theme::Metrics::new(1.15),
                    cx,
                )
            })
        });
        extensions.update(cx, |view, cx| view.sync_theme(self.theme.clone(), cx));
        let view = cx.new(|cx| {
            let mut view = SettingsView::new(
                snapshot,
                self.theme.clone(),
                muxy_ui::theme::Metrics::new(1.15),
                cx,
            );
            view.extensions = Some(extensions.clone());
            view.set_backup_pending(self.path.with_file_name("pending-import.muxy").exists());
            view
        });
        let weak = cx.weak_entity();
        let content = view.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1040.0), px(780.0)),
                cx,
            ))),
            window_min_size: Some(size(px(740.0), px(480.0))),
            is_movable: !cfg!(target_os = "macos"),
            titlebar: Some(TitlebarOptions {
                title: Some(tr!("Muxy Settings")),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.0), px(16.0))),
            }),
            ..WindowOptions::default()
        };
        match cx.open_window(options, move |window, cx| {
            let model = weak.clone();
            window.on_window_should_close(cx, move |_, cx| {
                let _ = model.update(cx, |model, cx| {
                    model.settings_closed(cx);
                });
                true
            });
            cx.new(|cx| SettingsWindow::new(weak, content, window, cx))
        }) {
            Ok(window) => {
                self.settings_window = Some(SettingsWindowState { window, view });
                self.dismiss_overlay(cx);
                self.read_server_settings(cx);
                self.read_remote_access(cx);
            }
            Err(error) => {
                if extensions.read(cx).updating_all() {
                    self.pending_extension_updates = Some(extensions);
                }
                self.fail(
                    tr!("Could not open Settings: %@", error.to_string()).to_string(),
                    cx,
                );
            }
        }
    }

    pub(crate) fn settings_closed(&mut self, cx: &mut Context<Self>) {
        self.flush_preferences(cx);
        self.withdraw_pairing(cx);
        self.retain_extension_updates(cx);
        self.settings_window = None;
        cx.notify();
    }

    fn retain_extension_updates(&mut self, cx: &Context<Self>) {
        if let Some(settings) = &self.settings_window
            && let Some(extensions) = &settings.view.read(cx).extensions
            && extensions.read(cx).updating_all()
        {
            self.pending_extension_updates = Some(extensions.clone());
        }
    }

    fn preferences_snapshot(&self) -> Snapshot {
        let mut settings = self.settings.clone();
        settings.appearance = self.appearance.clone();
        Snapshot {
            quick_shortcut_status: self.quick_shortcut_status(),
            settings,
            terminal: self.terminal.clone(),
            server: self.server_preferences.document.clone(),
            server_update: self.server_update_description(),
            connected: self.ready(ServerId::local()),
            server_busy: self.server_preferences.busy
                || self.server_preferences.control_busy
                || self.updates.replacing(),
            pending_server_fields: self.pending_server_fields(),
            mobile: self.mobile.state.clone(),
            mobile_pairing: self.mobile.pairing.clone(),
            mobile_busy: self.mobile.busy,
            mobile_error: self.mobile.error.clone(),
            ai_installed: self
                .ai
                .installed
                .iter()
                .map(|provider| provider.id)
                .collect(),
            sidebars: self.extension_sidebars(),
            file_openers: self.extension_file_openers(),
            languages: self.app_languages(),
            extension_shortcuts: self.extension_shortcuts(),
        }
    }

    pub(super) fn pending_server_fields(&self) -> std::collections::HashSet<String> {
        self.server_preferences
            .pending
            .iter()
            .map(|change| change_id(change).to_owned())
            .chain(
                self.server_preferences
                    .busy
                    .then(|| self.server_preferences.row.clone()),
            )
            .collect()
    }

    pub(crate) fn flush_preferences(&mut self, cx: &mut Context<Self>) {
        let changes = self
            .settings_window
            .as_ref()
            .map_or_else(Vec::new, |settings| {
                settings.view.update(cx, |view, cx| view.take_changes(cx))
            });
        for change in changes {
            self.change_preference(change, cx);
        }
    }

    pub(super) fn preferences_before_quit(&mut self, cx: &mut Context<Self>) -> bool {
        self.flush_preferences(cx);
        if self.server_preferences.busy || self.server_preferences.control_busy {
            self.fail(
                tr!("Wait for the server settings operation to finish before quitting").to_string(),
                cx,
            );
            return false;
        }
        true
    }

    pub(super) fn sync_preferences(&self, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings_window {
            settings.view.update(cx, |view, cx| {
                view.sync(self.preferences_snapshot(), self.theme.clone(), cx);
            });
        }
    }

    pub(crate) fn preference_result(&self, id: &str, error: Option<&str>, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings_window {
            settings
                .view
                .update(cx, |view, cx| view.set_error(id, error, cx));
        }
    }

    pub(crate) fn configuration_path(&self, filename: &str) -> std::path::PathBuf {
        self.path.with_file_name(filename)
    }

    pub(crate) fn change_preference(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle {
            return;
        }
        if let Change::Field(id, prompt) = &change
            && let Some(action) = crate::views::settings::prompt_action(id)
        {
            self.set_ai_prompt(action, prompt, cx);
            return;
        }
        let id = change_id(&change).to_owned();
        if matches!(
            &change,
            Change::MobileAccess(_) | Change::Field("mobile-port", _)
        ) {
            let result = self.write_mobile_preference(change, cx);
            self.preference_result(&id, result.err().as_deref(), cx);
        } else if matches!(
            &change,
            Change::ShellIntegration(_) | Change::Field("default-shell" | "history-budget", _)
        ) {
            if self.server_preferences.busy {
                self.server_preferences
                    .pending
                    .retain(|pending| change_id(pending) != id);
                self.server_preferences.pending.push_back(change);
            } else if let Err(error) = self.write_server_preference(change, cx) {
                self.preference_result(&id, Some(&error.to_string()), cx);
            }
        } else {
            let theme_change = matches!(change, Change::Theme(..));
            let result = self.apply_preference(change, cx);
            let error = result.err().map(|error| error.to_string());
            self.preference_result(&id, error.as_deref(), cx);
            if theme_change && let Some(error) = error {
                self.fail(tr!("Could not save theme: %@", &error).to_string(), cx);
            }
        }
        self.sync_preferences(cx);
        cx.notify();
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Exhaustive preference change dispatch"
    )]
    fn apply_preference(&mut self, change: Change, cx: &mut Context<Self>) -> Result<()> {
        let composer = matches!(&change, Change::Composer(..))
            || matches!(&change, Change::Field(id, _) if id.starts_with("composer-"));
        if composer {
            return self.apply_composer_preference(change, cx);
        }
        let path = self.path.with_file_name("settings.toml");
        let mut settings = self.settings.clone();
        settings.appearance = self.appearance.clone();
        let theme_changed = matches!(change, Change::Theme(..));
        let bindings_changed = matches!(
            change,
            Change::Binding(..)
                | Change::Unassign(_)
                | Change::Command(_)
                | Change::RemoveCommand(_)
        );
        match change {
            Change::QuickTerminal(quick) => {
                self.apply_quick_settings(quick, cx)?;
                return Ok(());
            }
            Change::Field(id @ ("quick-width" | "quick-height"), value) => {
                let mut quick = settings.quick_terminal;
                if id == "quick-width" {
                    quick.width = value.parse()?;
                } else {
                    quick.height = value.parse()?;
                }
                self.apply_quick_settings(quick, cx)?;
                return Ok(());
            }
            Change::Theme(dark, name) => {
                if dark {
                    settings.appearance.dark_theme = name;
                } else {
                    settings.appearance.light_theme = name;
                }
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Worktrees(key, value) => {
                match key {
                    "auto-expand-worktrees" => settings.appearance.auto_expand_worktrees = value,
                    "worktree-unread" => settings.appearance.worktree_show_unread = value,
                    _ => return Ok(()),
                }
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Sidebar(value) => {
                settings.appearance.sidebar_expanded = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::SidebarCollapsedStyle(style) => {
                settings.appearance.sidebar_collapsed_style = style;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::SidebarVibrancy(value) => {
                settings.appearance.sidebar_vibrancy = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::ExtensionSidebar(owner) => {
                settings.appearance.extension_sidebar = owner;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Language(selection) => {
                settings.appearance.language = selection;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Field("sidebar-vibrancy-level", value) => {
                let level: u8 = value
                    .parse()
                    .map_err(|_| tr!("Enter a whole number from 0 to 100").to_string())?;
                if level > 100 {
                    return Err(tr!("Enter a whole number from 0 to 100").to_string().into());
                }
                settings.appearance.sidebar_vibrancy_level = level;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::StatusBar(value) => {
                settings.appearance.status_bar_visible = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Tips(value) => {
                settings.appearance.tips_visible = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::ConfirmProcess(value) => {
                settings.window.confirm_running_process = value;
                settings.save_window(&path)?;
            }
            Change::CloseBehavior(value) => {
                settings.window.close_behavior = value;
                settings.save_window(&path)?;
            }
            Change::FileOpener(value) => {
                settings.openers.file = value;
                settings.save_openers(&path)?;
            }
            Change::Directory(value) => {
                settings.panes.new_pane_directory = value;
                settings.save_panes(&path)?;
            }
            Change::Command(command) => {
                command.validate()?;
                if let Some(existing) = settings
                    .commands
                    .iter_mut()
                    .find(|existing| existing.id == command.id)
                {
                    *existing = command;
                } else {
                    settings.commands.push(command);
                }
                settings.validate_command_shortcuts(&self.terminal)?;
                settings.save_commands(&path)?;
            }
            Change::RemoveCommand(id) => {
                settings.commands.retain(|command| command.id != id);
                settings.keymap = settings
                    .keymap
                    .with_binding(&format!("command.{id}"), None)?;
                settings.save_commands(&path)?;
            }
            Change::Binding(id, chord) => {
                if let Some(chord) = &chord {
                    self.validate_quick_conflict(chord)?;
                    if let Some(conflict) = self.custom_shortcut_conflict(&id, chord) {
                        return Err(conflict.into());
                    }
                    if (id.starts_with("extension.") || id.starts_with("command."))
                        && let Some(conflict) = self.extension_shortcut_conflict(&id, chord)
                    {
                        return Err(conflict.into());
                    }
                }
                settings.keymap = settings.keymap.with_binding(&id, chord)?;
                settings.validate_command_shortcuts(&self.terminal)?;
                settings.keymap.save(&path)?;
                cx.set_menus(crate::menus());
            }
            Change::Unassign(id) => {
                settings.keymap = settings.keymap.with_unassigned(&id)?;
                settings.keymap.save(&path)?;
            }
            Change::Field(id @ ("worktree-template" | "worktree-folder"), value) => {
                if id == "worktree-template" {
                    settings.worktrees.default_location.path_template = value;
                } else {
                    settings.worktrees.default_location.parent_path = value;
                }
                settings.save_worktrees(&path)?;
            }
            Change::Field(id @ ("width" | "height"), value) => {
                let index = usize::from(id == "height");
                settings.window.default_size[index] = value.parse()?;
                settings.save_window(&path)?;
            }
            Change::Terminal(id, value)
            | Change::Field(
                id @ ("font-family"
                | "font-size"
                | "adjust-cell-height"
                | "adjust-cell-width"
                | "background-opacity"
                | "window-padding-x"
                | "window-padding-y"
                | "adjust-cursor-thickness"
                | "scroll-precision"
                | "scroll-discrete"),
                value,
            ) => {
                self.save_terminal_preference(id, &value, cx)?;
            }
            _ => return Err(tr!("Unknown app setting").to_string().into()),
        }
        let theme_changed = theme_changed
            || self.appearance.dark_theme != settings.appearance.dark_theme
            || self.appearance.light_theme != settings.appearance.light_theme;
        let language_changed = self.appearance.language != settings.appearance.language;
        self.appearance = settings.appearance.clone();
        self.settings = settings;
        self.invalidate_webview_shortcuts();
        if bindings_changed {
            self.bind_extension_keys(cx);
        }
        if theme_changed {
            self.refresh_theme(cx);
        }
        if language_changed {
            self.sync_language(cx);
        }
        for pane in self.grids.values() {
            pane.view.update(cx, |pane, cx| {
                pane.copy_on_select = self.settings.clipboard.copy_on_select;
                cx.notify();
            });
        }
        Ok(())
    }

    fn apply_composer_preference(&mut self, change: Change, cx: &mut Context<Self>) -> Result<()> {
        let path = self.path.with_file_name("settings.toml");
        let mut settings = self.settings.clone();
        match change {
            Change::Composer(id, value) => {
                match id {
                    "composer-clear-after" => settings.composer.clear_after_sending = value,
                    "composer-clear-close" => settings.composer.clear_on_close = value,
                    "voice-auto-send" => settings.composer.voice_auto_send = value,
                    "composer-images" => {
                        settings.composer.image_strategy = if value {
                            muxy_app_core::composer::submission::ImageSubmissionStrategy::Clipboard
                        } else {
                            muxy_app_core::composer::submission::ImageSubmissionStrategy::InlinePath
                        }
                    }
                    _ => return Err(tr!("Unknown composer preference").to_string().into()),
                }
                settings.save_composer(&path)?;
            }
            Change::Field(
                id @ ("composer-font" | "composer-line-height" | "composer-language"),
                value,
            ) => {
                match id {
                    "composer-font" => settings.composer.font_family = value,
                    "composer-language" => settings.composer.language = value,
                    _ => settings.composer.line_height = value.parse()?,
                }
                settings.save_composer(&path)?;
            }
            _ => return Err(tr!("Unknown composer preference").to_string().into()),
        }
        self.settings.composer = settings.composer;
        self.composer.preferences_dirty = false;
        if let Some(view) = &self.composer.view {
            view.update(cx, |view, cx| {
                view.settings = self.settings.composer.clone();
                view.refresh_style(cx);
            });
        }
        Ok(())
    }

    #[cfg(all(target_os = "macos", not(test)))]
    pub(crate) fn sync_background_effects(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.sync_sidebar_vibrancy(window);
        let options = &self.terminal.options;
        if options.background_vibrancy == 0 || !self.terminal_effects_allowed() {
            self.terminal_background.native = None;
            self.set_terminal_vibrancy_available(false, cx);
            return;
        }
        let offset = self.sidebar_width();
        let width = (f32::from(window.viewport_size().width) - offset).max(0.0);
        let background = self.palette.background;
        let background = gpui::rgb(background).into();
        if let Some(effect) = &mut self.terminal_background.native {
            effect.update(offset, width, background);
        } else {
            self.terminal_background.native =
                muxy_ui::vibrancy::TerminalVibrancy::new(window, offset, width, background);
        }
        self.set_terminal_vibrancy_available(self.terminal_background.native.is_some(), cx);
    }

    #[cfg(any(target_os = "macos", test))]
    pub(super) fn set_terminal_vibrancy_available(
        &mut self,
        available: bool,
        cx: &mut Context<Self>,
    ) {
        if self.terminal_background.available == available {
            return;
        }
        self.terminal_background.available = available;
        for (id, pane) in &self.grids {
            let enabled = available && !self.is_quick_terminal(*id);
            pane.view.update(cx, |pane, cx| {
                if pane.native_vibrancy != enabled {
                    pane.native_vibrancy = enabled;
                    cx.notify();
                }
            });
        }
    }

    pub(crate) fn preview_terminal_preference(
        &mut self,
        id: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        let mut next = self.terminal.clone();
        if let Err(error) = next.set_preference(id, value) {
            self.preference_result(id, Some(&error.to_string()), cx);
            return;
        }
        if next == self.terminal {
            return;
        }
        self.terminal_preview
            .get_or_insert_with(|| TerminalPreview {
                settings: self.terminal.clone(),
                zoom: self
                    .grids
                    .iter()
                    .map(|(id, pane)| (*id, pane.view.read(cx).terminal.font_size))
                    .collect(),
            });
        self.apply_terminal_preferences(next, false, cx);
    }

    fn save_terminal_preference(
        &mut self,
        id: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let result = (|| {
            let path = self.path.with_file_name("terminal.toml");
            let mut requested = muxy_app_core::settings::TerminalSettings::load_native(&path)?;
            requested.set_preference(id, value)?;
            self.settings.validate_command_shortcuts(&requested)?;
            requested.save_native(&path)?;
            Ok(requested)
        })();
        let preview = self.terminal_preview.take();
        match result {
            Ok(requested) => {
                let original = preview
                    .as_ref()
                    .map_or(&self.terminal, |preview| &preview.settings);
                let changed_size = original.font_size.to_bits() != requested.font_size.to_bits();
                self.apply_terminal_preferences(requested, changed_size, cx);
                if !changed_size && let Some(preview) = preview {
                    self.restore_terminal_zoom(preview.zoom, cx);
                }
                self.set_configuration_error(None);
                Ok(())
            }
            Err(error) => {
                if let Some(preview) = preview {
                    self.apply_terminal_preferences(preview.settings, false, cx);
                    self.restore_terminal_zoom(preview.zoom, cx);
                }
                Err(error)
            }
        }
    }

    fn restore_terminal_zoom(
        &mut self,
        zoom: std::collections::HashMap<super::PaneId, f32>,
        cx: &mut Context<Self>,
    ) {
        for (id, size) in zoom {
            if let Some(pane) = self.grids.get(&id) {
                pane.view.update(cx, |pane, cx| {
                    pane.terminal.font_size = size;
                    cx.notify();
                });
            }
        }
    }

    fn apply_terminal_preferences(
        &mut self,
        effective: muxy_app_core::settings::TerminalSettings,
        reset_zoom: bool,
        cx: &mut Context<Self>,
    ) {
        let changed_size =
            reset_zoom || self.terminal.font_size.to_bits() != effective.font_size.to_bits();
        self.terminal = effective;
        if reset_zoom {
            self.font_sizes.clear();
        }
        for pane in self.grids.values() {
            pane.view.update(cx, |pane, cx| {
                let zoom = pane.terminal.font_size;
                pane.terminal = self.terminal.clone();
                pane.configured_font_size = self.terminal.font_size;
                if !changed_size {
                    pane.terminal.font_size = zoom;
                }
                cx.notify();
            });
        }
        self.refresh_theme(cx);
    }

    pub(crate) fn read_server_settings(&mut self, cx: &mut Context<Self>) {
        if self.quitting == Quitting::Idle
            && self.ready(ServerId::local())
            && !self.server_preferences.busy
            && !self.server_preferences.control_busy
        {
            self.server_preferences.busy =
                self.send(ServerId::local(), Work::ReadServerSettings, cx);
            self.server_preferences.row = "server".into();
            self.sync_preferences(cx);
        }
    }

    fn write_server_preference(&mut self, change: Change, cx: &mut Context<Self>) -> Result<()> {
        if !self.ready(ServerId::local()) || self.server_preferences.control_busy {
            return Err(tr!("Connect to the server before editing its settings")
                .to_string()
                .into());
        }
        let mut settings = self
            .server_preferences
            .document
            .clone()
            .ok_or_else(|| tr!("Load server settings first").to_string())?;
        let id = change_id(&change).to_owned();
        match change {
            Change::ShellIntegration(value) => settings.shell_integration = value,
            Change::Field("default-shell", value) => {
                settings.default_shell =
                    (!value.is_empty()).then(|| muxy_protocol::ServerPath(value.into_bytes()));
            }
            Change::Field("history-budget", value) => {
                settings.history_budget_bytes = value
                    .parse::<u64>()?
                    .checked_mul(1024 * 1024)
                    .ok_or_else(|| tr!("History budget is too large").to_string())?;
            }
            _ => return Err(tr!("Unknown server setting").to_string().into()),
        }
        settings.validate().map_err(|_| {
            tr!("Use an absolute shell path and a history budget between 0 and 65536 MiB")
                .to_string()
        })?;
        self.server_preferences.busy =
            self.send(ServerId::local(), Work::WriteServerSettings(settings), cx);
        self.server_preferences.row = id;
        if !self.server_preferences.busy {
            return Err(tr!("Could not send server settings").to_string().into());
        }
        Ok(())
    }

    pub(super) fn receive_server_settings(
        &mut self,
        result: std::result::Result<muxy_protocol::ServerSettingsDoc, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.server_preferences.busy = false;
        match result {
            Ok(settings) => {
                self.server_preferences.document = Some(settings);
                self.preference_result(&self.server_preferences.row, None, cx);
            }
            Err(error) => {
                self.preference_result(&self.server_preferences.row, Some(&error.to_string()), cx);
            }
        }
        while !self.server_preferences.busy {
            let Some(change) = self.server_preferences.pending.pop_front() else {
                break;
            };
            self.change_preference(change, cx);
        }
        self.sync_preferences(cx);
    }

    /// A stopped server counts as disconnected. Restarting connects again,
    /// which starts it, over SSH too.
    pub(super) fn receive_server_stopped(
        &mut self,
        server: ServerId,
        restart: bool,
        result: std::result::Result<(), muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        if server.is_local() {
            self.server_preferences.control_busy = false;
            self.server_update_control_finished(&result, cx);
        } else if let Some(runtime) = self.servers.get_mut(server) {
            runtime.stopping = false;
        }
        match result {
            Ok(()) => {
                self.disconnect(server, cx);
                if restart {
                    self.connect_server(server, cx);
                } else if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.retry.held = true;
                }
            }
            Err(error) => {
                let message = self
                    .server_message(server, &tr!("Could not stop server: %@", error.to_string()));
                if server.is_local() {
                    self.preference_result("server", Some(&message), cx);
                }
                self.fail(message, cx);
            }
        }
        self.sync_preferences(cx);
    }

    /// Asks before stopping or restarting `server`, which ends its sessions.
    pub(crate) fn confirm_server_control(
        &mut self,
        server: ServerId,
        restart: bool,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !self.control_enabled(server) {
            return;
        }
        self.dismiss_overlay(cx);
        let generation = self.generation(server);
        let name = (!server.is_local()).then(|| self.server_label(server));
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response =
                crate::views::confirm::prompt_server(window, restart, name.as_deref(), cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                model.focus_requested = true;
                if generation == model.generation(server) {
                    match response {
                        Ok(true) => model.stop_server(server, restart, cx),
                        Ok(false) => {}
                        Err(error) if server.is_local() => {
                            model.preference_result("server", Some(&error), cx);
                        }
                        Err(error) => model.fail(error, cx),
                    }
                }
                cx.notify();
            });
        }));
    }

    fn stop_server(&mut self, server: ServerId, restart: bool, cx: &mut Context<Self>) {
        let sent = self.send(server, Work::StopServer { restart }, cx);
        if server.is_local() {
            self.server_preferences.control_busy = sent;
            if sent {
                self.server_update_control_started(restart);
            }
        } else if let Some(runtime) = self.servers.get_mut(server) {
            runtime.stopping = sent;
        }
        self.sync_preferences(cx);
    }
}

fn change_id(change: &Change) -> &str {
    match change {
        Change::Command(_) | Change::RemoveCommand(_) => "commands",
        Change::QuickTerminal(_) => "quick-shortcut",
        Change::Theme(false, _) => "light-theme",
        Change::Theme(true, _) => "dark-theme",
        Change::Sidebar(_) => "sidebar",
        Change::SidebarVibrancy(_) => "sidebar-vibrancy",
        Change::ExtensionSidebar(_) => "extension-sidebar",
        Change::Language(_) => "app-language",
        Change::Worktrees(key, _) => key,
        Change::SidebarCollapsedStyle(_) => "sidebar-collapsed-style",
        Change::StatusBar(_) => "status-bar",
        Change::Tips(_) => "tips",
        Change::ConfirmProcess(_) => "confirm-process",
        Change::CloseBehavior(_) => "close-behavior",
        Change::FileOpener(_) => "file-opener",
        Change::Directory(_) => "directory",
        Change::Terminal("padding-left" | "padding-right", _) => "window-padding-x",
        Change::Terminal("padding-top" | "padding-bottom", _) => "window-padding-y",
        Change::Terminal(id, _) | Change::Composer(id, _) | Change::Field(id, _) => id,
        Change::Binding(id, _) | Change::Unassign(id) => id,
        Change::ShellIntegration(_) => "shell-integration",
        Change::MobileAccess(_) => "mobile-access",
    }
}
