mod appearance;
mod catalog;
mod composer;
pub(crate) mod extensions;
mod keyboard;
mod layout;
mod pickers;
mod quick_terminal;
mod results;
mod server;
mod terminal;
pub(crate) mod window;

pub(crate) use pickers::{PickerAnchor, PickerKind, PickerRequest};

use std::collections::{HashMap, HashSet};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription, Window, canvas,
    div, px,
};
use muxy_app_core::settings::{Settings, TerminalSettings};
use muxy_ui::controls::{self, Style};
use muxy_ui::form;
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::theme::{Metrics, Theme};

pub(crate) fn register_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &crate::model::AppModel,
) {
    use super::command_palette::action;
    use muxy_core::shortcuts::ShortcutId;
    registry.register(action(
        model,
        ShortcutId::OpenSettings,
        "Open Settings",
        super::workspace::OpenSettings,
    ));
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Category {
    General,
    Composer,
    QuickTerminal,
    Appearance,
    Terminal,
    Keyboard,
    Server,
    Extensions,
}

impl Category {
    const ALL: [Self; 8] = [
        Self::General,
        Self::QuickTerminal,
        Self::Composer,
        Self::Appearance,
        Self::Keyboard,
        Self::Terminal,
        Self::Server,
        Self::Extensions,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Composer => "Composer",
            Self::QuickTerminal => "Quick Terminal",
            Self::Appearance => "Appearance",
            Self::Terminal => "Terminal",
            Self::Keyboard => "Keyboard",
            Self::Server => "Server",
            Self::Extensions => "Extensions",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Change {
    Composer(&'static str, bool),
    QuickTerminal(muxy_app_core::settings::QuickTerminalSettings),
    Theme(bool, String),
    Sidebar(bool),
    SidebarCollapsedStyle(muxy_app_core::settings::SidebarCollapsedStyle),
    StatusBar(bool),
    ConfirmProcess(bool),
    CloseBehavior(muxy_app_core::settings::CloseBehavior),
    CopyOnSelect(bool),
    Directory(muxy_app_core::settings::NewPaneDirectory),
    Field(&'static str, String),
    Binding(String, Option<muxy_app_core::settings::KeyChord>),
    ShellIntegration(bool),
}

pub(crate) enum SettingsEvent {
    DictationLanguage,
    Change(Change),
    Picker(PickerKind, PickerAnchor),
    ServerControl { restart: bool },
    ReadServer,
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
}

pub(crate) struct SettingsView {
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
    #[cfg(test)]
    pub(crate) fn region_render_counts(&self, cx: &gpui::App) -> (usize, usize) {
        (
            self.navigation_view.read(cx).render_count,
            self.content_view.read(cx).render_count,
        )
    }

    pub(crate) fn new(
        snapshot: Snapshot,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(InputStyle::field(&theme, &metrics), cx)
                .with_placeholder("Search settings…")
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
                .map(|shortcut| shortcut.id.replace(['_', '.'], " "))
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
            focus_initialized: false,
            compact: false,
            picker_anchors: [
                PickerKind::FontFamily,
                PickerKind::Theme(false),
                PickerKind::Theme(true),
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
        for id in [
            "composer-font",
            "composer-line-height",
            "quick-width",
            "quick-height",
            "width",
            "height",
            "font-size",
            "adjust-cell-height",
            "default-shell",
            "history-budget",
        ] {
            let input =
                cx.new(|cx| TextInput::new(InputStyle::field(&pane.theme, &pane.metrics), cx));
            let changed = cx.subscribe(&input, move |pane: &mut Self, _, event, cx| match event {
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
            });
            pane.fields.insert(id, input);
            pane.subscriptions.push(changed);
        }
        pane.sync_fields(cx);
        pane
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

    #[cfg(test)]
    pub(crate) fn focused_shortcut(&self, window: &Window) -> Option<(&'static str, bool)> {
        self.results
            .shortcut_focus
            .iter()
            .enumerate()
            .find_map(|(index, handles)| {
                handles
                    .iter()
                    .position(|handle| handle.is_focused(window))
                    .map(|control| (muxy_core::shortcuts::ALL[index].id, control == 1))
            })
    }

    #[cfg(test)]
    pub(crate) fn theme(&self) -> &Theme {
        &self.theme
    }

    #[cfg(test)]
    pub(crate) fn dictation_language(&self) -> &str {
        &self.snapshot.settings.composer.language
    }

    #[cfg(test)]
    pub(crate) fn matching_setting_ids(&self) -> Vec<&'static str> {
        catalog::SETTINGS
            .iter()
            .filter(|setting| self.matches(setting.category, setting.label))
            .map(|setting| setting.id)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn field_value(&self, id: &str, cx: &gpui::App) -> String {
        self.fields[id].read(cx).text().to_owned()
    }

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
        for (id, value) in [
            ("composer-font", settings.composer.font_family.clone()),
            (
                "composer-line-height",
                settings.composer.line_height.to_string(),
            ),
            ("quick-width", settings.quick_terminal.width.to_string()),
            ("quick-height", settings.quick_terminal.height.to_string()),
            ("width", settings.window.default_size[0].to_string()),
            ("height", settings.window.default_size[1].to_string()),
            ("font-size", terminal.font_size.to_string()),
            ("adjust-cell-height", terminal.cell_height.to_string()),
            ("default-shell", shell),
            ("history-budget", budget),
        ] {
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
        for key in keys {
            self.notes.insert(key.clone(), "A config-file include supplies this setting. Edit the included file to change its value.".into());
        }
    }

    pub(crate) fn set_error(&mut self, id: &str, error: Option<&str>, cx: &mut Context<Self>) {
        if let Some(error) = error {
            self.errors.insert(id.into(), error.to_owned());
        } else {
            self.errors.remove(id);
        }
        self.results.dirty = true;
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn results_state(&self) -> &gpui::ListState {
        &self.results.state
    }

    fn commit_field(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if self.dirty.remove(id) {
            let value = self.fields[id].read(cx).text().trim().to_owned();
            cx.emit(SettingsEvent::Change(Change::Field(id, value)));
        }
    }

    pub(crate) fn show_extensions(&mut self, cx: &mut Context<Self>) {
        self.category = Category::Extensions;
        self.query.clear();
        self.search.update(cx, |input, cx| input.set_text("", cx));
        cx.notify();
    }

    pub(crate) fn show_server(&mut self, cx: &mut Context<Self>) {
        self.category = Category::Server;
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
            let text = setting
                .map_or_else(
                    || format!("{} {label}", category.label()),
                    |setting| {
                        format!(
                            "{} {} {} {} {}",
                            category.label(),
                            setting.section,
                            setting.id,
                            label,
                            setting.description
                        )
                    },
                )
                .to_lowercase();
            self.query
                .split_whitespace()
                .all(|word| text.contains(word))
        }
    }

    fn configuration_file(&self) -> &'static str {
        match self.category {
            Category::Terminal => "ghostty.conf",
            Category::Server => "server.toml",
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
        const METRICS: Metrics = Metrics::new(1.0);
        Style {
            theme: &self.theme,
            metrics: &METRICS,
        }
    }

    fn row(&self, id: &str, label: &str, control: AnyElement) -> AnyElement {
        let setting = catalog::setting(id);
        let description = setting.map(|setting| setting.description);
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
                self.compact,
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
