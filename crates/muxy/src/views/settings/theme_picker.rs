use crate::themes::ThemeEntry;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Render,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use muxy_core::fold::fold;
use muxy_core::prefs::settings;
use muxy_ui::components::SymbolGlyph;
use muxy_ui::text_input::{self, InputEvent, InputStyle, TextInput};
use muxy_ui::theme::{Appearance, Metrics, Theme};

const PANEL_WIDTH: f32 = 280.0;
const PANEL_HEIGHT: f32 = 400.0;
const PANEL_MARGIN: f32 = 80.0;
const MIN_PANEL_HEIGHT: f32 = 180.0;
const SWATCH_HEIGHT: f32 = 14.0;

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
    search: Entity<TextInput>,
    query: String,
    highlighted: usize,
    appearance: Appearance,
    theme: Theme,
    metrics: Metrics,
    scroll: ScrollHandle,
    focus_handle: FocusHandle,
    focused: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ThemeBrowserEvent> for ThemeBrowser {}

impl Focusable for ThemeBrowser {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
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
        let style = InputStyle::field(&theme, &metrics);
        let search = cx.new(|cx| {
            TextInput::new(style, cx)
                .with_key_context(text_input::BARE_CONTEXT)
                .with_placeholder("Search themes")
        });
        let subscription = cx.subscribe(&search, |browser: &mut Self, input, event, cx| {
            if matches!(event, InputEvent::Changed) {
                browser.query = input.read(cx).text().to_owned();
                browser.highlighted = 0;
                cx.notify();
            }
        });

        Self {
            mode,
            entries: crate::themes::catalog(),
            search,
            query: String::new(),
            highlighted: 0,
            appearance,
            theme,
            metrics,
            scroll: ScrollHandle::default(),
            focus_handle: cx.focus_handle(),
            focused: false,
            _subscriptions: vec![subscription],
        }
    }

    pub fn set_appearance(&mut self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        self.theme = theme;
        self.metrics = metrics;
        cx.notify();
    }

    fn is_dark(&self) -> bool {
        match self.mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::CurrentAppearance => self.appearance == Appearance::Dark,
        }
    }

    fn active_name(&self) -> String {
        let key = if self.is_dark() {
            "muxy.theme.dark"
        } else {
            "muxy.theme.light"
        };
        settings::string_value(key, "")
    }

    fn matches(&self) -> Vec<&ThemeEntry> {
        let query = fold(self.query.trim());
        self.entries
            .iter()
            .filter(|entry| query.is_empty() || fold(&entry.name).contains(&query))
            .collect()
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

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.matches().len();
        match event.keystroke.key.as_str() {
            "escape" => {
                cx.stop_propagation();
                cx.emit(ThemeBrowserEvent::Dismiss);
            }
            "down" if count > 0 => {
                cx.stop_propagation();
                self.highlighted = (self.highlighted + 1).min(count - 1);
                cx.notify();
            }
            "up" if count > 0 => {
                cx.stop_propagation();
                self.highlighted = self.highlighted.saturating_sub(1);
                cx.notify();
            }
            "enter" => {
                cx.stop_propagation();
                let Some(name) = self
                    .matches()
                    .get(self.highlighted)
                    .map(|entry| entry.name.clone())
                else {
                    return;
                };
                self.select(name, cx);
            }
            _ => {}
        }
    }

    fn row(&self, entry: &ThemeEntry, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let active = entry.name == self.active_name();
        let highlighted = index == self.highlighted;
        let name = entry.name.clone();

        let background = entry.scheme.background.map(Into::into).unwrap_or(theme.bg);
        let foreground = entry.scheme.foreground.map(Into::into).unwrap_or(theme.fg);

        let mut strip = div()
            .flex()
            .flex_row()
            .h(metrics.scaled(SWATCH_HEIGHT))
            .rounded(metrics.scaled(3.0))
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(metrics.control_medium())
                    .h_full()
                    .bg(background)
                    .text_size(metrics.font_xs())
                    .text_color(foreground)
                    .child(SharedString::from("Ab")),
            );
        for slot in 0..16 {
            let color = entry
                .scheme
                .palette_color(slot)
                .map(Into::into)
                .unwrap_or(background);
            strip = strip.child(div().flex_grow().h_full().bg(color));
        }

        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing2())
            .child(
                div()
                    .flex_grow()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(metrics.font_footnote())
                    .text_color(theme.fg)
                    .child(SharedString::from(entry.name.clone())),
            );
        if active {
            header = header.child(SymbolGlyph::new(
                "checkmark",
                metrics.font_xs(),
                theme.accent,
            ));
        }

        div()
            .id(SharedString::from(format!("theme-row-{}", entry.name)))
            .flex()
            .flex_col()
            .gap(metrics.spacing2())
            .px(metrics.spacing5())
            .py(metrics.scaled(5.0))
            .cursor_pointer()
            .when(highlighted, |element| element.bg(theme.surface))
            .when(!highlighted, |element| {
                element.hover(|style| style.bg(theme.hover))
            })
            .on_click(cx.listener(move |browser: &mut Self, _, _, cx| {
                browser.select(name.clone(), cx);
            }))
            .child(header)
            .child(strip)
            .into_any_element()
    }
}

impl Render for ThemeBrowser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = self.metrics;
        let theme = self.theme.clone();
        if !self.focused {
            self.focused = true;
            window.focus(&self.search.focus_handle(cx));
        }

        let matches = self.matches();
        let mut list = div().flex().flex_col();
        if matches.is_empty() {
            list = list.child(
                div()
                    .p(metrics.spacing6())
                    .text_size(metrics.font_footnote())
                    .text_color(theme.fg_muted)
                    .child(SharedString::from("No themes found")),
            );
        }
        for (index, entry) in matches.into_iter().enumerate() {
            list = list.child(self.row(entry, index, cx));
        }

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .occlude()
            .flex()
            .flex_col()
            .w(metrics.scaled(PANEL_WIDTH))
            .h(metrics.scaled(PANEL_HEIGHT).min(
                (window.viewport_size().height - metrics.scaled(PANEL_MARGIN))
                    .max(metrics.scaled(MIN_PANEL_HEIGHT)),
            ))
            .rounded(metrics.radius_lg())
            .bg(theme.raised())
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .px(metrics.spacing5())
                    .py(metrics.spacing4())
                    .child(muxy_ui::text_input::growing_input(&self.search)),
            )
            .child(div().h(px(1.0)).flex_none().bg(theme.border))
            .child(
                div()
                    .id("theme-browser-list")
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(list),
            )
    }
}
