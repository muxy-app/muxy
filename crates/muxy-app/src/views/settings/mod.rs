mod ai;
mod appearance;
mod backup;
mod catalog;
mod commands;
mod composer;
pub(crate) mod extensions;
mod keyboard;
mod languages;
mod layout;
pub(crate) mod mobile;
mod pickers;
mod quick_terminal;
mod results;
mod server;
mod terminal;
pub(crate) mod window;

pub(crate) use ai::prompt_action;
pub(crate) use pickers::{PickerAnchor, PickerKind, PickerRequest};

use std::collections::{HashMap, HashSet};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription, Window, canvas,
    div, px,
};
use muxy_app_core::settings::{Settings, TerminalSettings};
use muxy_core::l10n::{searchable, translate};
use muxy_core::tr_key;
use muxy_ui::controls::{self, Style};
use muxy_ui::form;
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::tr;

pub(crate) fn register_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &crate::model::AppModel,
) {
    use super::command_palette::action;
    use muxy_core::shortcuts::ShortcutId;
    registry.register(action(
        model,
        ShortcutId::OpenSettings,
        tr_key!("Open Settings"),
        super::workspace::OpenSettings,
    ));
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Category {
    General,
    Ai,
    Composer,
    QuickTerminal,
    Appearance,
    Terminal,
    Keyboard,
    Commands,
    Server,
    Mobile,
    Extensions,
    Backup,
}

impl Category {
    pub(crate) const ALL: [Self; 12] = [
        Self::General,
        Self::QuickTerminal,
        Self::Composer,
        Self::Appearance,
        Self::Keyboard,
        Self::Commands,
        Self::Terminal,
        Self::Server,
        Self::Mobile,
        Self::Extensions,
        Self::Ai,
        Self::Backup,
    ];

    /// The English name; also the key of its translation.
    fn label(self) -> &'static str {
        match self {
            Self::General => tr_key!("General"),
            Self::Ai => tr_key!("AI"),
            Self::Composer => tr_key!("Composer"),
            Self::QuickTerminal => tr_key!("Quick Terminal"),
            Self::Appearance => tr_key!("Appearance"),
            Self::Terminal => tr_key!("Terminal"),
            Self::Keyboard => tr_key!("Keyboard"),
            Self::Commands => tr_key!("Commands"),
            Self::Server => tr_key!("Server"),
            Self::Mobile => tr_key!("Mobile"),
            Self::Extensions => tr_key!("Extensions"),
            Self::Backup => tr_key!("Backup & Restore"),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Change {
    Composer(&'static str, bool),
    QuickTerminal(muxy_app_core::settings::QuickTerminalSettings),
    Theme(bool, String),
    Sidebar(bool),
    SidebarVibrancy(bool),
    /// The extension whose sidebar replaces the built-in one; empty for built-in.
    ExtensionSidebar(String),
    /// The app language as `<extension>:<localization>`; empty for English.
    Language(String),
    Worktrees(&'static str, bool),
    SidebarCollapsedStyle(muxy_app_core::settings::SidebarCollapsedStyle),
    StatusBar(bool),
    Tips(bool),
    ConfirmProcess(bool),
    CloseBehavior(muxy_app_core::settings::CloseBehavior),
    CopyOnSelect(bool),
    /// `[openers] file`: a built-in opener, or `<extension>:<opener>`.
    FileOpener(String),
    Directory(muxy_app_core::settings::NewPaneDirectory),
    Field(&'static str, String),
    Binding(String, Option<muxy_app_core::settings::KeyChord>),
    Command(muxy_app_core::settings::CustomCommand),
    RemoveCommand(String),
    /// Removes an extension command's shortcut.
    Unassign(String),
    ShellIntegration(bool),
    MobileAccess(bool),
}

pub(crate) enum SettingsEvent {
    Backup(backup::Action),
    Change(Change),
    Picker(PickerKind, PickerAnchor),
    ServerControl { restart: bool },
    ReadServer,
    ReadMobile,
    PairPhone,
    CancelPairing,
    RevokeDevice(muxy_protocol::DeviceId),
    Connect,
    OpenConfiguration(&'static str),
    ReloadConfiguration,
}

#[derive(Clone)]
pub(crate) struct Snapshot {
    pub(crate) settings: Settings,
    pub(crate) quick_shortcut_status: String,
    pub(crate) terminal: TerminalSettings,
    pub(crate) server: Option<muxy_protocol::ServerSettingsDoc>,
    pub(crate) connected: bool,
    pub(crate) server_busy: bool,
    pub(crate) server_update: Option<String>,
    pub(crate) pending_server_fields: HashSet<String>,
    pub(crate) mobile: Option<muxy_protocol::RemoteAccessState>,
    pub(crate) mobile_pairing: Option<mobile::Pairing>,
    pub(crate) mobile_busy: bool,
    pub(crate) mobile_error: Option<String>,
    pub(crate) ai_installed: Vec<&'static str>,
    /// Enabled extensions that provide a sidebar, as `(extension, label)`.
    pub(crate) sidebars: Vec<(String, String)>,
    /// Enabled extensions' file openers, as `(setting value, label)`.
    pub(crate) file_openers: Vec<(String, String)>,
    /// Languages enabled extensions provide, as `(selection, label)`.
    pub(crate) languages: Vec<(String, String)>,
    pub(crate) extension_shortcuts: Vec<crate::model::extensions::ExtensionShortcut>,
}

pub(crate) struct SettingsView {
    backup_busy: bool,
    backup_import: Option<crate::backup::PreparedImport>,
    backup_status: String,
    pub(crate) extensions: Option<Entity<extensions::ExtensionsView>>,
    navigation_view: Entity<super::cached::CachedView<Self>>,
    content_view: Entity<super::cached::CachedView<Self>>,
    quick_recording: Option<(muxy_ui::quick_terminal::ShortcutRecording, gpui::Task<()>)>,
    quick_slider: Option<(&'static str, gpui::Bounds<gpui::Pixels>)>,
    pub(crate) focus: FocusHandle,
    snapshot: Snapshot,
    theme: Theme,
    metrics: Metrics,
    search: Entity<TextInput>,
    query: String,
    results: results::Results,
    scrollbar: muxy_ui::scrollbar::ListScrollbar,
    shortcut_names: Vec<String>,
    category: Category,
    section: Option<&'static str>,
    expanded: HashSet<Category>,
    navbar_focus: FocusHandle,
    fields: HashMap<&'static str, Entity<TextInput>>,
    dirty: HashSet<&'static str>,
    pub(crate) errors: HashMap<String, String>,
    pub(crate) notes: HashMap<String, String>,
    recording: Option<String>,
    command_editor: Option<muxy_app_core::settings::CustomCommand>,
    focus_initialized: bool,
    compact: bool,
    picker_anchors: HashMap<PickerKind, PickerAnchor>,
    subscriptions: Vec<Subscription>,
    #[cfg(test)]
    pub(crate) render_count: usize,
    #[cfg(test)]
    pub(crate) shortcut_row_count: usize,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl SettingsView {
    pub(crate) fn new(
        snapshot: Snapshot,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(InputStyle::field(&theme, &metrics), cx)
                .with_placeholder_key(tr_key!("Search settings…"))
        });
        let search_subscription = cx.subscribe(&search, |pane: &mut Self, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                pane.query = pane.search.read(cx).text().trim().to_lowercase();
                pane.results.reset();
            }
            cx.notify();
        });
        let focus = cx.focus_handle();
        let weak = cx.weak_entity();
        let recorder = cx.intercept_keystrokes(move |event, window, cx| {
            let _ = weak.update(cx, |pane: &mut Self, cx| {
                if pane.focus.is_focused(window) {
                    pane.record_key(&event.keystroke, cx);
                }
            });
        });
        let mut pane = Self {
            backup_busy: false,
            backup_import: None,
            backup_status: String::new(),
            extensions: None,
            navigation_view: super::cached::CachedView::new(|view, _, cx| view.navigation(cx), cx),
            content_view: super::cached::CachedView::new(|view, _, cx| view.content(cx), cx),
            quick_recording: None,
            quick_slider: None,
            focus,
            snapshot,
            theme,
            metrics,
            search,
            query: String::new(),
            results: results::Results::new(cx),
            scrollbar: muxy_ui::scrollbar::ListScrollbar::default(),
            shortcut_names: muxy_core::shortcuts::ALL
                .iter()
                .map(muxy_core::shortcuts::Shortcut::label)
                .collect(),
            category: Category::General,
            section: None,
            expanded: HashSet::new(),
            navbar_focus: cx.focus_handle().tab_stop(true),
            fields: HashMap::new(),
            dirty: HashSet::new(),
            errors: HashMap::new(),
            notes: HashMap::new(),
            recording: None,
            command_editor: None,
            focus_initialized: false,
            compact: false,
            picker_anchors: [
                PickerKind::FontFamily,
                PickerKind::Language,
                PickerKind::Theme(false),
                PickerKind::Theme(true),
                PickerKind::AiProvider(crate::repository_actions::Action::Commit),
                PickerKind::AiProvider(crate::repository_actions::Action::CreatePullRequest),
                PickerKind::ExtensionSidebar,
                PickerKind::FileOpener,
                PickerKind::AppLanguage,
            ]
            .into_iter()
            .map(|kind| (kind, PickerAnchor::default()))
            .collect(),
            subscriptions: vec![search_subscription, recorder],
            #[cfg(test)]
            render_count: 0,
            #[cfg(test)]
            shortcut_row_count: 0,
        };
        pane.create_fields(cx);
        pane.sync_fields(cx);
        pane
    }

    fn create_fields(&mut self, cx: &mut Context<Self>) {
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        for (id, multiline) in [
            "command-name",
            "command-text",
            "composer-font",
            "composer-line-height",
            "quick-width",
            "quick-height",
            "sidebar-vibrancy-level",
            "worktree-template",
            "worktree-folder",
            "width",
            "height",
            "font-size",
            "adjust-cell-height",
            "default-shell",
            "history-budget",
            "sandbox-executable",
            "sandbox-network",
            "sandbox-domains",
            "sandbox-tools",
            "sandbox-environment",
            "mobile-port",
        ]
        .into_iter()
        .map(|id| (id, false))
        .chain(ai::PROMPTS.iter().map(|(_, id)| (*id, true)))
        {
            let input = cx.new(|cx| {
                let input = TextInput::new(InputStyle::field(&theme, &metrics), cx);
                if multiline { input.multiline() } else { input }
            });
            let changed = cx.subscribe(&input, move |pane: &mut Self, _, event, cx| {
                if matches!(id, "command-name" | "command-text") {
                    match event {
                        InputEvent::Submitted => pane.save_command(cx),
                        InputEvent::Cancelled => pane.cancel_command(cx),
                        InputEvent::Changed => {
                            cx.notify();
                        }
                    }
                    return;
                }
                match event {
                    InputEvent::Changed => {
                        pane.dirty.insert(id);
                        pane.results.dirty = true;
                        cx.notify();
                    }
                    InputEvent::Submitted => pane.commit_field(id, cx),
                    InputEvent::Cancelled => {
                        pane.dirty.remove(id);
                        pane.errors.remove(id);
                        pane.results.dirty = true;
                        pane.sync_fields(cx);
                        cx.notify();
                    }
                }
            });
            self.fields.insert(id, input);
            self.subscriptions.push(changed);
        }
    }

    pub(crate) fn sync(&mut self, snapshot: Snapshot, theme: Theme, cx: &mut Context<Self>) {
        self.results.dirty = true;
        self.snapshot = snapshot;
        self.theme = theme;
        if let Some(extensions) = &self.extensions {
            extensions.update(cx, |view, cx| view.sync_theme(self.theme.clone(), cx));
        }
        let style = InputStyle::field(&self.theme, &self.metrics);
        self.search
            .update(cx, |input, cx| input.set_style(style, cx));
        for input in self.fields.values() {
            input.update(cx, |input, cx| input.set_style(style, cx));
        }
        self.sync_fields(cx);
        cx.notify();
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Synchronize the settings field inventory together"
    )]
    fn sync_fields(&self, cx: &mut Context<Self>) {
        let settings = &self.snapshot.settings;
        let terminal = &self.snapshot.terminal;
        let server = self.snapshot.server.as_ref();
        let shell = server
            .and_then(|server| server.default_shell.as_ref())
            .map_or_else(String::new, |path| {
                String::from_utf8_lossy(&path.0).into_owned()
            });
        let budget = server.map_or_else(String::new, |server| {
            (server.history_budget_bytes / (1024 * 1024)).to_string()
        });
        let sandbox = server
            .and_then(|server| server.sandbox.as_ref())
            .cloned()
            .unwrap_or_default();
        let mobile_port = self
            .snapshot
            .mobile
            .as_ref()
            .map_or_else(String::new, |mobile| mobile.settings.port.to_string());
        for (id, value) in [
            ("composer-font", settings.composer.font_family.clone()),
            (
                "composer-line-height",
                settings.composer.line_height.to_string(),
            ),
            ("quick-width", settings.quick_terminal.width.to_string()),
            ("quick-height", settings.quick_terminal.height.to_string()),
            (
                "sidebar-vibrancy-level",
                settings.appearance.sidebar_vibrancy_level.to_string(),
            ),
            (
                "worktree-template",
                settings.worktrees.default_location.path_template.clone(),
            ),
            (
                "worktree-folder",
                settings.worktrees.default_location.parent_path.clone(),
            ),
            ("width", settings.window.default_size[0].to_string()),
            ("height", settings.window.default_size[1].to_string()),
            ("font-size", terminal.font_size.to_string()),
            ("adjust-cell-height", terminal.cell_height.to_string()),
            ("default-shell", shell),
            ("history-budget", budget),
            (
                "sandbox-executable",
                sandbox
                    .executable
                    .as_ref()
                    .map(|p| String::from_utf8_lossy(&p.0).into_owned())
                    .unwrap_or_default(),
            ),
            (
                "sandbox-network",
                match sandbox.policy.network {
                    muxy_protocol::SandboxNetwork::Blocked => "blocked",
                    muxy_protocol::SandboxNetwork::Domains => "domains",
                    _ => "unrestricted",
                }
                .into(),
            ),
            ("sandbox-domains", sandbox.policy.domains.join(", ")),
            (
                "sandbox-tools",
                sandbox
                    .policy
                    .read_paths
                    .iter()
                    .map(|p| String::from_utf8_lossy(&p.0))
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
            ("sandbox-environment", sandbox.policy.environment.join(", ")),
            ("mobile-port", mobile_port),
        ]
        .into_iter()
        .chain(ai::PROMPTS.iter().map(|(action, id)| {
            (
                *id,
                settings
                    .ai
                    .prompts
                    .get(action.key())
                    .filter(|prompt| !prompt.trim().is_empty())
                    .cloned()
                    .unwrap_or_else(|| action.default_prompt().into()),
            )
        })) {
            if !self.dirty.contains(id)
                && !self.errors.contains_key(id)
                && !self.snapshot.pending_server_fields.contains(id)
            {
                self.fields[id].update(cx, |input, cx| {
                    if input.text() != value {
                        input.set_text(value, cx);
                    }
                });
            }
        }
    }

    fn initialize_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_initialized {
            return;
        }
        self.focus_initialized = true;
        self.subscriptions
            .push(cx.on_focus_out(&self.focus, window, |pane, _, _, cx| {
                pane.recording = None;
                pane.quick_recording = None;
                pane.results.dirty = true;
                cx.notify();
            }));
        for input in std::iter::once(&self.search).chain(self.fields.values()) {
            self.subscriptions
                .push(cx.on_focus(&input.focus_handle(cx), window, |pane, _, cx| {
                    pane.recording = None;
                    pane.results.dirty = true;
                    cx.notify();
                }));
        }
        for (&id, input) in &self.fields {
            self.subscriptions.push(cx.on_blur(
                &input.focus_handle(cx),
                window,
                move |pane, _, cx| {
                    pane.results.dirty = true;
                    pane.commit_field(id, cx);
                    cx.notify();
                },
            ));
        }
    }

    pub(crate) fn take_changes(&mut self, cx: &Context<Self>) -> Vec<Change> {
        self.recording = None;
        self.results.dirty = true;
        self.dirty
            .drain()
            .map(|id| Change::Field(id, self.fields[id].read(cx).text().trim().to_owned()))
            .collect()
    }

    pub(crate) fn set_included_keys(&mut self, keys: &HashSet<String>) {
        self.results.dirty = true;
        self.notes.clear();
        let note = tr!(
            "A config-file include supplies this setting. Edit the included file to change its value."
        );
        for key in keys {
            self.notes.insert(key.clone(), note.to_string());
        }
    }

    pub(crate) fn set_error(&mut self, id: &str, error: Option<&str>, cx: &mut Context<Self>) {
        if let Some(error) = error {
            self.errors.insert(id.into(), error.to_owned());
        } else {
            self.errors.remove(id);
            if id == "commands" {
                self.command_editor = None;
            }
        }
        self.results.dirty = true;
        cx.notify();
    }

    fn commit_field(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if self.dirty.remove(id) {
            let value = self.fields[id].read(cx).text().trim().to_owned();
            cx.emit(SettingsEvent::Change(Change::Field(id, value)));
        }
    }

    /// Shows Extensions → Browse limited to language packs.
    fn browse_languages(&mut self, cx: &mut Context<Self>) {
        self.show_extensions(cx);
        if let Some(extensions) = &self.extensions {
            extensions.update(cx, extensions::ExtensionsView::browse_languages);
        }
    }

    pub(crate) fn show_extensions(&mut self, cx: &mut Context<Self>) {
        self.category = Category::Extensions;
        self.query.clear();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        cx.notify();
    }

    fn matches(&self, category: Category, label: &str) -> bool {
        let setting = catalog::SETTINGS
            .iter()
            .find(|setting| setting.category == category && setting.label == label);
        if self.query.is_empty() {
            category == self.category
                && self
                    .section
                    .is_none_or(|section| setting.is_none_or(|setting| setting.section == section))
        } else {
            let text = setting.map_or_else(
                || format!("{} {}", searchable(category.label()), searchable(label)),
                |setting| {
                    format!(
                        "{} {} {} {} {}",
                        searchable(category.label()),
                        searchable(setting.section),
                        setting.id,
                        searchable(label),
                        searchable(setting.description)
                    )
                },
            );
            self.query
                .split_whitespace()
                .all(|word| text.contains(word))
        }
    }

    /// The file behind this category's settings; mobile access is managed only here.
    fn configuration_file(&self) -> Option<&'static str> {
        match self.category {
            Category::Terminal => Some("ghostty.conf"),
            Category::Server => Some("server.toml"),
            Category::Mobile => None,
            _ => Some("settings.toml"),
        }
    }

    fn style(&self) -> Style<'_> {
        Style {
            theme: &self.theme,
            metrics: &self.metrics,
        }
    }

    fn layout_style(&self) -> Style<'_> {
        const METRICS: Metrics = Metrics::new(1.0);
        Style {
            theme: &self.theme,
            metrics: &METRICS,
        }
    }

    /// A setting row. `label` is the English key it is listed and searched
    /// by; an explicit `description` is already translated.
    fn row(&self, id: &str, label: &str, control: AnyElement) -> AnyElement {
        self.row_with(id, label, control, self.compact)
    }

    fn row_with(&self, id: &str, label: &str, control: AnyElement, stacked: bool) -> AnyElement {
        self.row_with_description(id, label, None, control, stacked)
    }

    fn row_with_description(
        &self,
        id: &str,
        label: &str,
        description: Option<&str>,
        control: AnyElement,
        stacked: bool,
    ) -> AnyElement {
        self.text_row(id, &translate(label), description, control, stacked)
    }

    /// A row whose label is shown as given, such as a custom command's name.
    fn text_row(
        &self,
        id: &str,
        label: &str,
        description: Option<&str>,
        control: AnyElement,
        stacked: bool,
    ) -> AnyElement {
        let setting = catalog::setting(id);
        let catalog_description = setting.map(|setting| translate(setting.description));
        let description = description.or(catalog_description.as_deref());
        let section = setting
            .filter(|setting| {
                !catalog::SETTINGS
                    .iter()
                    .take_while(|previous| previous.id != id)
                    .any(|previous| {
                        previous.category == setting.category
                            && previous.section == setting.section
                            && self.matches(previous.category, previous.label)
                    })
            })
            .map(|setting| setting.section);
        let focus = self.results.row_focus[id].clone();
        let reveal = self.results.reveal_focus.clone();
        div()
            .relative()
            .track_focus(&focus)
            .debug_selector({
                let id = id.to_owned();
                move || format!("settings-row-{id}")
            })
            .min_w(px(0.0))
            .border_b_1()
            .border_color(self.theme.border)
            .when_some(section, |row, section| {
                row.child(self.subsection_heading(section))
            })
            .child(form::row(
                self.layout_style(),
                label,
                description,
                control,
                stacked,
            ))
            .when_some(self.errors.get(id), |row, error| {
                row.child(self.note(error, true).pt_0())
            })
            .when(id == "quick-size", |row| {
                row.children(
                    ["quick-width", "quick-height"]
                        .into_iter()
                        .filter_map(|id| {
                            self.errors.get(id).map(|error| {
                                self.note(error, true)
                                    .pt_0()
                                    .debug_selector(move || format!("settings-error-{id}"))
                            })
                        }),
                )
            })
            .when_some(self.notes.get(id), |row, note| {
                row.child(self.note(note, false).pt_0())
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        if reveal.get() && focus.contains_focused(window, cx) {
                            reveal.set(false);
                            window.request_autoscroll(bounds);
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }

    fn note(&self, text: &str, error: bool) -> gpui::Div {
        form::note(self.style(), text, error)
    }

    fn field(&self, id: &'static str) -> AnyElement {
        let selector = format!("settings-field-{id}");
        div()
            .flex()
            .min_w(px(0.0))
            .debug_selector(move || selector.clone())
            .child(controls::text_field(
                self.style(),
                id,
                &self.fields[id],
                if matches!(id, "quick-width" | "quick-height") {
                    Some(64.0)
                } else {
                    (!self.compact).then_some(210.0)
                },
            ))
            .into_any_element()
    }

    fn toggle(
        &self,
        id: &'static str,
        value: bool,
        change: Change,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        controls::toggle(
            self.style(),
            id,
            value,
            cx.listener(move |_, _, _, cx| {
                cx.emit(SettingsEvent::Change(change.clone()));
            }),
        )
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(test)]
        {
            self.render_count += 1;
        }
        self.initialize_focus(window, cx);
        let view = cx.entity();
        let reveal = self.results.reveal_focus.clone();
        div()
            .id("settings-view")
            .on_mouse_move(cx.listener(|view, event: &gpui::MouseMoveEvent, _, cx| {
                if let Some((id, bounds)) = view.quick_slider {
                    view.move_quick_slider(id, bounds, event.position, cx);
                }
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|view, _, _, _| {
                    view.quick_slider = None;
                }),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|view, _, _, _| {
                    view.quick_slider = None;
                }),
            )
            .debug_selector(|| "settings-view".into())
            .track_focus(&self.focus)
            .tab_group()
            .font_family(".SystemUIFont")
            .line_height(gpui::relative(1.3))
            .on_action(cx.listener(|view, _: &SearchSettings, window, cx| {
                view.search.focus_handle(cx).focus(window);
            }))
            .on_action(cx.listener(|view, _: &FocusSettingsNavbar, window, _| {
                view.navbar_focus.focus(window);
            }))
            .on_action(cx.listener(|view, _: &NextSettingsControl, window, cx| {
                view.advance_focus(false, window, cx);
            }))
            .on_action(
                cx.listener(|view, _: &PreviousSettingsControl, window, cx| {
                    view.advance_focus(true, window, cx);
                }),
            )
            .size_full()
            .relative()
            .flex()
            .overflow_hidden()
            .bg(self.theme.bg)
            .text_color(self.theme.fg)
            .text_size(self.metrics.font_body())
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let mut content = view
                            .update(cx, |pane, cx| pane.layout_content(bounds.size, window, cx));
                        content.layout_as_root(bounds.size.into(), window, cx);
                        content.prepaint_at(bounds.origin, window, cx);
                        reveal.set(false);
                        content
                    },
                    |_, mut content, window, cx| content.paint(window, cx),
                )
                .size_full(),
            )
    }
}

gpui::actions!(
    settings,
    [
        CloseSettings,
        SearchSettings,
        FocusSettingsNavbar,
        NextSettingsControl,
        PreviousSettingsControl
    ]
);

pub(crate) fn register_shortcuts(registry: &mut muxy_ui::shortcuts::Registry<'_>) {
    use muxy_core::shortcuts::ShortcutId;
    registry.register(ShortcutId::CloseSettings, &CloseSettings);
    registry.register(ShortcutId::SearchSettings, &SearchSettings);
    registry.register(ShortcutId::FocusSettingsNavbar, &FocusSettingsNavbar);
    registry.register(ShortcutId::NextSettingsControl, &NextSettingsControl);
    registry.register(
        ShortcutId::PreviousSettingsControl,
        &PreviousSettingsControl,
    );
}
