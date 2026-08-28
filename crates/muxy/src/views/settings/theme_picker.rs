use crate::themes::ThemeEntry;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Subscription,
};
use muxy_core::fold::fold;
use muxy_core::prefs::settings;
use muxy_ui::command_popover::{
    CommandPopover, CommandPopoverConfig, CommandPopoverDensity, CommandPopoverEvent,
    CommandPopoverItem, CommandPopoverPresentation, CommandPopoverRow, CommandPopoverStatus,
    CommandPopoverTab,
};
use muxy_ui::theme::{Appearance, Metrics, Theme};

pub const PICKER_WIDTH: f32 = 340.0;
pub const PICKER_MAX_HEIGHT: f32 = 360.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    CurrentAppearance,
}

pub enum ThemeBrowserEvent {
    Applied,
    Dismiss,
}

pub struct ThemeBrowser {
    mode: ThemeMode,
    entries: Vec<ThemeEntry>,
    appearance: Appearance,
    picker: Entity<CommandPopover>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ThemeBrowserEvent> for ThemeBrowser {}

impl Focusable for ThemeBrowser {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.picker.focus_handle(cx)
    }
}

impl ThemeBrowser {
    pub fn new(
        mode: ThemeMode,
        appearance: Appearance,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let picker = cx.new(|cx| {
            CommandPopover::new(
                CommandPopoverConfig {
                    id: "theme-browser".into(),
                    presentation: CommandPopoverPresentation::Popover,
                    density: CommandPopoverDensity::Compact,
                    tabs: vec![CommandPopoverTab::new("themes", "Themes")],
                    placeholder: "Search themes…".into(),
                    footer_actions: Vec::new(),
                    footer_hints: Vec::new(),
                    width: Some(PICKER_WIDTH),
                    height: None,
                    max_height: Some(PICKER_MAX_HEIGHT),
                    completion_on_tab: false,
                    confirm_on_click: true,
                },
                theme,
                metrics,
                cx,
            )
        });
        let subscription = cx.subscribe(
            &picker,
            |browser: &mut Self, picker, event, cx| match event {
                CommandPopoverEvent::QueryChanged { query, .. } => {
                    browser.sync_picker(query.as_ref(), cx)
                }
                CommandPopoverEvent::Confirmed(selection)
                | CommandPopoverEvent::SecondaryConfirmed(selection) => {
                    let Some(index) = selection
                        .id
                        .strip_prefix("theme-")
                        .and_then(|index| index.parse::<usize>().ok())
                    else {
                        return;
                    };
                    if let Some(name) = browser.entries.get(index).map(|entry| entry.name.clone()) {
                        browser.select(name, cx);
                    }
                }
                CommandPopoverEvent::Dismissed => cx.emit(ThemeBrowserEvent::Dismiss),
                _ => {
                    let _ = picker;
                }
            },
        );
        let browser = Self {
            mode,
            entries: crate::themes::catalog(),
            appearance,
            picker,
            _subscriptions: vec![subscription],
        };
        browser.sync_picker("", cx);
        browser
    }

    pub fn picker(&self) -> &Entity<CommandPopover> {
        &self.picker
    }

    pub fn set_appearance(&mut self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        self.picker
            .update(cx, |picker, cx| picker.set_appearance(theme, metrics, cx));
        let query = self.picker.read(cx).query().to_owned();
        self.sync_picker(&query, cx);
    }

    fn is_dark(&self) -> bool {
        match self.mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::CurrentAppearance => self.appearance == Appearance::Dark,
        }
    }

    fn active_name(&self) -> String {
        settings::string_value(
            if self.is_dark() {
                "muxy.theme.dark"
            } else {
                "muxy.theme.light"
            },
            "",
        )
    }

    fn sync_picker(&self, query: &str, cx: &mut Context<Self>) {
        let query = fold(query.trim());
        let active = self.active_name();
        let items = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| query.is_empty() || fold(&entry.name).contains(&query))
            .map(|(index, entry)| {
                let mut row = CommandPopoverRow::new(format!("theme-{index}"), entry.name.clone());
                row.current = entry.name == active;
                row.swatches = (0..16)
                    .filter_map(|slot| entry.scheme.palette_color(slot).map(Into::into))
                    .collect();
                CommandPopoverItem::Row(row)
            })
            .collect::<Vec<_>>();
        let status = if items.is_empty() {
            CommandPopoverStatus::Empty("No themes found".into())
        } else {
            CommandPopoverStatus::Ready
        };
        self.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
        });
    }

    fn select(&mut self, name: String, cx: &mut Context<Self>) {
        let (dark, light) = muxy_core::store::ghostty_conf::theme_selection();
        let (dark, light) = if self.is_dark() {
            (name.clone(), light.unwrap_or(name))
        } else {
            (dark.unwrap_or_else(|| name.clone()), name)
        };
        muxy_core::store::ghostty_conf::set_theme(&dark, &light);
        settings::sync();
        cx.emit(ThemeBrowserEvent::Applied);
        cx.notify();
    }
}

impl Render for ThemeBrowser {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        self.picker.clone()
    }
}
