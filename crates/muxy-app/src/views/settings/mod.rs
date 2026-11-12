mod about;
mod ai;
mod appearance;
mod backup;
pub(crate) mod browser;
mod catalog;
mod commands;
mod composer;
pub(crate) mod extensions;
mod keyboard;
mod layout;
mod pickers;
mod quick_terminal;
mod results;
mod server;
mod slider;
pub(crate) mod tab;
mod terminal;
mod terminal_lists;

pub(crate) use ai::prompt_action;
pub(crate) use pickers::{PickerAnchor, PickerKind, PickerRequest};

use std::collections::{HashMap, HashSet};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, canvas, div, px,
};
use muxy_app_core::settings::{Settings, TerminalSettings};
use muxy_core::l10n::{searchable, translate};
use muxy_core::tr_key;
use muxy_ui::accessibility;
use muxy_ui::controls::{self, Style};
use muxy_ui::form;
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::theme::{Metrics, Theme};

use crate::model::preferences::Change;

pub(crate) fn register_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &crate::model::AppModel,
    cx: &gpui::App,
) {
    super::workspace::commands::register_placed(
        registry,
        model,
        muxy_core::shortcuts::ShortcutId::OpenSettings,
        cx,
    );
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
    Browser,
    Servers,
    Extensions,
    Backup,
    About,
}

impl Category {
    pub(crate) const ALL: [Self; 13] = [
        Self::General,
        Self::QuickTerminal,
        Self::Composer,
        Self::Appearance,
        Self::Keyboard,
        Self::Commands,
        Self::Terminal,
        Self::Browser,
        Self::Servers,
        Self::Extensions,
        Self::Ai,
        Self::Backup,
        Self::About,
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
            Self::Browser => tr_key!("Browser"),
            Self::Servers => tr_key!("Servers"),
            Self::Extensions => tr_key!("Extensions"),
            Self::Backup => tr_key!("Backup & Restore"),
            Self::About => tr_key!("About"),
        }
    }
}

pub(crate) enum SettingsEvent {
    QuickRecording(bool),
    Backup(backup::Action),
    Change(Change),
    PreviewTerminal(&'static str, String),
    ResetTerminal,
    Picker(PickerKind, PickerAnchor),
    ServerControl { restart: bool },
    OpenLoginItems,
    ReadServer,
    CheckExtensionUpdates,
    CheckForUpdates,
    Connect,
    OpenConfiguration(&'static str),
    DismissProblem,
    ShowWelcomeTour,
    AskAgent,
}

#[derive(Clone)]
pub(crate) struct Snapshot {
    pub(crate) settings: Settings,
    pub(crate) capabilities: muxy_core::capability::Capabilities,
    pub(crate) quick_shortcut_status: String,
    pub(crate) terminal: TerminalSettings,
    pub(crate) server: Option<muxy_protocol::ServerSettingsDoc>,
    pub(crate) connected: bool,
    pub(crate) server_busy: bool,
    pub(crate) server_update: Option<String>,
    pub(crate) pending_server_fields: HashSet<String>,
    pub(crate) ai_installed: Vec<&'static str>,
    /// The installed extensions have been read from disk.
    pub(crate) extensions_loaded: bool,
    /// Enabled extensions that provide a sidebar, as `(extension, label)`.
    pub(crate) sidebars: Vec<(String, String)>,
    /// Enabled extensions' file openers, as `(setting value, label)`.
    pub(crate) file_openers: Vec<(String, String)>,
    /// Languages enabled extensions provide, as `(selection, label)`.
    pub(crate) languages: Vec<(String, String)>,
    pub(crate) extension_shortcuts: Vec<crate::model::extensions::ExtensionShortcut>,
    pub(crate) browser_profiles: Vec<browser::Profile>,
    /// Other browsers' profiles that can be imported, by name, and whether
    /// their cookies can be.
    pub(crate) browser_sources: Vec<(String, bool)>,
}

pub(crate) struct SettingsView {
    backup_busy: bool,
    backup_import: Option<crate::backup::PreparedImport>,
    backup_status: String,
    pub(crate) extensions: Option<Entity<extensions::ExtensionsView>>,
    pub(crate) servers: Option<Entity<super::cached::CachedView<crate::model::AppModel>>>,
    navigation_view: Entity<super::cached::CachedView<Self>>,
    content_view: Entity<super::cached::CachedView<Self>>,
    quick_recording: Option<(muxy_ui::quick_terminal::ShortcutRecording, gpui::Task<()>)>,
    slider: Option<slider::Drag>,
    codepoint_editor: Option<terminal_lists::CodepointEditor>,
    binding_editor: Option<terminal_lists::BindingEditor>,
    pub(crate) focus: FocusHandle,
    snapshot: Snapshot,
    theme: Theme,
    metrics: Metrics,
    layout_metrics: Metrics,
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

#[cfg(test)]
impl SettingsView {
    pub(crate) fn scale(&self) -> (u32, u32) {
        (
            self.layout_metrics.multiplier().to_bits(),
            self.metrics.multiplier().to_bits(),
        )
    }
}

const CONTENT_SCALE: f32 = 1.15;

/// Settings rows are drawn larger than the rest of the app.
pub(crate) fn content_metrics(metrics: Metrics) -> Metrics {
    metrics.times(CONTENT_SCALE)
}

fn layout_metrics(content: Metrics) -> Metrics {
    Metrics::new(content.multiplier() / CONTENT_SCALE)
}

impl SettingsView {
    pub(crate) fn new(
        snapshot: Snapshot,
        theme: Theme,
        layout_metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let metrics = content_metrics(layout_metrics);
        let search = cx.new(|cx| {
            TextInput::new(InputStyle::field(&theme, &metrics), cx)
                .with_placeholder_key(tr_key!("Search settings…"))
                .with_accessibility_id("settings-search")
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
                    if pane
                        .quick_recording
                        .as_ref()
                        .is_some_and(|(recorder, _)| recorder.record(&event.keystroke))
                    {
                        cx.stop_propagation();
                    } else {
                        pane.record_key(&event.keystroke, cx);
                    }
                }
            });
        });
        let mut pane = Self {
            backup_busy: false,
            backup_import: None,
            backup_status: String::new(),
            extensions: None,
            servers: None,
            navigation_view: super::cached::CachedView::new(
                |view, window, cx| view.navigation(window, cx),
                cx,
            ),
            content_view: super::cached::CachedView::new(|view, _, cx| view.content(cx), cx),
            quick_recording: None,
            slider: None,
            codepoint_editor: None,
            binding_editor: None,
            focus,
            snapshot,
            theme,
            metrics,
            layout_metrics,
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
            recording: None,
            command_editor: None,
            focus_initialized: false,
            compact: false,
            picker_anchors: [
                PickerKind::Theme(false),
                PickerKind::Theme(true),
                PickerKind::QuickTheme(false),
                PickerKind::QuickTheme(true),
                PickerKind::AiProvider(crate::repository_actions::Action::Commit),
                PickerKind::AiProvider(crate::repository_actions::Action::CreatePullRequest),
                PickerKind::ExtensionSidebar,
                PickerKind::FileOpener,
                PickerKind::AppLanguage,
            ]
            .into_iter()
            .chain(pickers::FontSlot::ALL.map(PickerKind::Font))
            .chain(terminal::PICKERS.iter().copied().map(PickerKind::Terminal))
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
        for (id, multiline) in field_ids() {
            let input = cx.new(|cx| {
                let mut input = TextInput::new(InputStyle::field(&theme, &metrics), cx)
                    .with_accessibility_id(format!("settings-field-{id}"));
                if let Some(label) = catalog::control_label(id) {
                    input = input.with_accessible_label(label);
                } else if let Some(index) = palette_index(id) {
                    input.set_accessible_name(muxy_ui::tr!("Palette color %@", index));
                }
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
                if terminal_lists::EDITOR_FIELDS.contains(&id) {
                    match event {
                        InputEvent::Submitted => pane.save_terminal_editor(id, cx),
                        InputEvent::Cancelled => pane.cancel_terminal_editor(id, cx),
                        InputEvent::Changed => cx.notify(),
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

    pub(crate) fn sync(
        &mut self,
        snapshot: Snapshot,
        theme: Theme,
        layout_metrics: Metrics,
        cx: &mut Context<Self>,
    ) {
        self.results.dirty = true;
        self.snapshot = snapshot;
        self.theme = theme;
        if self.layout_metrics != layout_metrics {
            self.results.reveal_row.set(Some("interface-size"));
        }
        self.layout_metrics = layout_metrics;
        self.metrics = content_metrics(layout_metrics);
        if let Some(extensions) = &self.extensions {
            extensions.update(cx, |view, cx| {
                view.sync_appearance(self.theme.clone(), self.metrics, cx);
            });
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

    fn sync_fields(&self, cx: &mut Context<Self>) {
        let settings = &self.snapshot.settings;
        let server = self.snapshot.server.as_ref();
        let shell = server
            .and_then(|server| server.default_shell.as_ref())
            .map_or_else(String::new, |path| {
                String::from_utf8_lossy(&path.0).into_owned()
            });
        let budget = server.map_or_else(String::new, |server| {
            (server.history_budget_bytes / (1024 * 1024)).to_string()
        });
        for (id, value) in [
            ("composer-font", settings.composer.font_family.clone()),
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
            ("default-shell", shell),
            ("history-budget", budget),
            ("browser-home-page", settings.browser.home_page.clone()),
        ]
        .into_iter()
        .chain(terminal::field_values(&self.snapshot.terminal))
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
                && self.dragging() != Some(id)
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
                pane.cancel_quick_recording(cx);
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
            .collect::<Vec<_>>()
            .into_iter()
            .chain(self.take_slider_change())
            .collect()
    }

    pub(crate) fn set_error(&mut self, id: &str, error: Option<&str>, cx: &mut Context<Self>) {
        if let Some(error) = error {
            self.errors.insert(id.into(), error.to_owned());
        } else {
            self.errors.remove(id);
            match id {
                "commands" => self.command_editor = None,
                "font-codepoint-map" => self.codepoint_editor = None,
                "terminal-keybindings" => self.binding_editor = None,
                "terminal-reset" => {
                    self.codepoint_editor = None;
                    self.binding_editor = None;
                    self.errors.retain(|id, _| !terminal::owns(id));
                }
                _ => (),
            }
        }
        self.results.dirty = true;
        cx.notify();
    }

    /// Clears a page-level error; the application error is dismissed everywhere.
    fn dismiss_error(&mut self, id: &str, cx: &mut Context<Self>) {
        self.set_error(id, None, cx);
        if id == "application" {
            cx.emit(SettingsEvent::DismissProblem);
        }
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

    pub(crate) fn show_browser(&mut self, cx: &mut Context<Self>) {
        self.category = Category::Browser;
        self.section = None;
        self.query.clear();
        self.results.reset();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        cx.notify();
    }

    pub(crate) fn show_extensions(&mut self, cx: &mut Context<Self>) {
        self.category = Category::Extensions;
        self.query.clear();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        cx.emit(SettingsEvent::CheckExtensionUpdates);
        cx.notify();
    }

    pub(crate) fn show_servers(&mut self, cx: &mut Context<Self>) {
        self.category = Category::Servers;
        self.section = None;
        self.query.clear();
        self.results.reset();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        cx.emit(SettingsEvent::ReadServer);
        cx.notify();
    }

    /// Whether the Servers page is on screen; search results replace it.
    pub(crate) fn servers_shown(&self) -> bool {
        self.category == Category::Servers && self.query.is_empty()
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
                        "{} {} {} {} {} {}",
                        searchable(category.label()),
                        searchable(setting.section),
                        setting.id,
                        searchable(label),
                        searchable(setting.description),
                        catalog::search_aliases(setting.id)
                    )
                },
            );
            self.query
                .split_whitespace()
                .all(|word| text.contains(word))
        }
    }

    /// The file behind this category's settings.
    fn configuration_file(&self) -> &'static str {
        match self.category {
            Category::Servers => "server.toml",
            Category::Terminal => "terminal.toml",
            _ => "settings.toml",
        }
    }

    fn style(&self) -> Style<'_> {
        Style {
            theme: &self.theme,
            metrics: &self.metrics,
        }
    }

    fn layout_style(&self) -> Style<'_> {
        Style {
            theme: &self.theme,
            metrics: &self.layout_metrics,
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
        self.framed_row(id, description, |description| {
            form::row(self.layout_style(), label, description, control, stacked)
        })
    }

    /// A list setting's row: `action` sits beside the label, `items` below.
    fn list_row(
        &self,
        id: &str,
        label: &str,
        action: AnyElement,
        items: Option<AnyElement>,
    ) -> AnyElement {
        let label = translate(label);
        self.framed_row(id, None, |description| {
            form::list_row(
                self.layout_style(),
                &label,
                description,
                action,
                items,
                self.compact,
            )
        })
    }

    /// Wraps a row's body with its section heading, error, and focus reveal.
    /// `body` gets the description to show, falling back to the catalog's.
    fn framed_row(
        &self,
        id: &str,
        description: Option<&str>,
        body: impl FnOnce(Option<&str>) -> gpui::Div,
    ) -> AnyElement {
        let setting = catalog::setting(id);
        let catalog_description = setting.map(|setting| translate(setting.description));
        let description = description.or(catalog_description.as_deref());
        let section = setting.filter(|setting| {
            !catalog::SETTINGS
                .iter()
                .take_while(|previous| previous.id != id)
                .any(|previous| {
                    previous.category == setting.category
                        && previous.section == setting.section
                        && self.matches(previous.category, previous.label)
                })
        });
        let focus = self.results.row_focus[id].clone();
        let reveal = self.results.reveal_focus.clone();
        let reveal_row = self.results.reveal_row.get() == Some(id);
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
            .when_some(section, |row, setting| {
                row.child(self.subsection_heading(setting.category, setting.section))
            })
            .child(body(description))
            .when_some(self.errors.get(id), |row, error| {
                row.child(announced_error(self.note(error, true).pt_0(), id, error))
            })
            .when(id == "quick-size", |row| {
                row.children(
                    ["quick-width", "quick-height"]
                        .into_iter()
                        .filter_map(|id| {
                            self.errors.get(id).map(|error| {
                                announced_error(
                                    self.note(error, true)
                                        .pt_0()
                                        .debug_selector(move || format!("settings-error-{id}")),
                                    id,
                                    error,
                                )
                            })
                        }),
                )
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        if reveal.get() && focus.contains_focused(window, cx) {
                            reveal.set(false);
                            window.request_autoscroll(bounds);
                        } else if reveal_row {
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

    /// A note clients can read, for a list's empty state.
    fn readable_note(&self, id: &'static str, text: &str) -> AnyElement {
        self.note(text, false)
            .id(id)
            .role(gpui::Role::Label)
            .aria_value(text.to_owned())
            .into_any_element()
    }

    /// A stored value shown without a control, for a setting whose feature cannot run here.
    fn stored(&self, id: &str, value: &str) -> AnyElement {
        controls::button(
            self.style(),
            id,
            controls::Button::secondary(value)
                .named(&muxy_ui::tr!("%@: %@", &control_name(id), value))
                .enabled(false),
            |_, _, _| {},
        )
        .into_any_element()
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
                (!self.compact).then_some(210.0),
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
            &control_name(id),
            value,
            cx.listener(move |_, _, _, cx| {
                cx.emit(SettingsEvent::Change(change.clone()));
            }),
        )
        .into_any_element()
    }
}

/// A note saying why `id`'s control was refused, announced when it appears.
fn announced_error(note: gpui::Div, id: &str, error: &str) -> accessibility::LiveRegion {
    accessibility::announced(
        note.id(gpui::SharedString::from(format!("settings-error-{id}"))),
        error.to_owned(),
        true,
    )
}

/// Every text field Settings shows, and whether it takes several lines.
fn field_ids() -> impl Iterator<Item = (&'static str, bool)> {
    [
        "command-name",
        "command-text",
        "composer-font",
        "worktree-template",
        "worktree-folder",
        "width",
        "height",
        "default-shell",
        "history-budget",
        "browser-home-page",
        "browser-profile-name",
    ]
    .into_iter()
    .chain(terminal::fields())
    .chain(terminal_lists::EDITOR_FIELDS)
    .map(|id| (id, false))
    .chain(ai::PROMPTS.iter().map(|(_, id)| (*id, true)))
}

fn palette_index(id: &str) -> Option<&str> {
    id.strip_prefix("palette-")
}

/// What accessibility clients read for the control `id`.
fn control_name(id: &str) -> gpui::SharedString {
    catalog::control_label(id).map_or_else(Default::default, muxy_ui::l10n::translate)
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
        let reveal_row = self.results.reveal_row.clone();
        div()
            .id("settings-view")
            .on_mouse_move(cx.listener(|view, event: &gpui::MouseMoveEvent, _, cx| {
                view.move_slider(event.position, cx);
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|view, _, _, cx| view.finish_slider(cx)),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|view, _, _, cx| view.finish_slider(cx)),
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
                        reveal_row.set(None);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interface_size_is_found_by_its_other_names() {
        assert!(catalog::search_aliases("interface-size").contains("zoom"));
        assert!(catalog::search_aliases("interface-size").contains("density"));
        assert!(catalog::search_aliases("sidebar").is_empty());
    }

    #[test]
    fn every_text_field_has_a_name() {
        for (id, _) in field_ids() {
            assert!(
                catalog::control_label(id).is_some() || palette_index(id).is_some(),
                "{id} has no name"
            );
        }
    }

    #[test]
    fn every_picker_and_slider_has_a_name() {
        let pickers = pickers::FontSlot::ALL
            .map(PickerKind::Font)
            .into_iter()
            .chain(terminal::PICKERS.iter().copied().map(PickerKind::Terminal))
            .chain([
                PickerKind::Theme(false),
                PickerKind::Theme(true),
                PickerKind::QuickTheme(false),
                PickerKind::QuickTheme(true),
                PickerKind::AiProvider(crate::repository_actions::Action::Commit),
                PickerKind::AiProvider(crate::repository_actions::Action::CreatePullRequest),
                PickerKind::ExtensionSidebar,
                PickerKind::FileOpener,
                PickerKind::AppLanguage,
            ])
            .map(PickerKind::id);
        let padding = [
            "padding-left",
            "padding-right",
            "padding-top",
            "padding-bottom",
        ];
        for id in pickers.chain(padding) {
            assert!(catalog::control_label(id).is_some(), "{id} has no name");
        }
    }
}
