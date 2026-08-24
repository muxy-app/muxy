use super::items::{Item, ItemAction, Scope};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, actions, div, px,
};
use muxy_ui::components::SymbolGlyph;
use muxy_ui::text_input::{self, InputEvent, InputStyle, TextInput};
use muxy_ui::theme::{Metrics, Theme};

const PANEL_WIDTH: f32 = 720.0;
const PANEL_HEIGHT: f32 = 460.0;
const PANEL_TOP: f32 = 60.0;
const ROW_HEIGHT: f32 = 40.0;
pub const KEY_CONTEXT: &str = "Omnibox";

actions!(omnibox, [MoveUp, MoveDown, Confirm, Dismiss]);

pub fn key_bindings() -> Vec<KeyBinding> {
    let context = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("up", MoveUp, context),
        KeyBinding::new("down", MoveDown, context),
        KeyBinding::new("shift-tab", MoveUp, context),
        KeyBinding::new("tab", MoveDown, context),
        KeyBinding::new("enter", Confirm, context),
        KeyBinding::new("escape", Dismiss, context),
    ]
}

pub enum OmniboxEvent {
    QueryChanged,
    Confirm(ItemAction),
    Dismiss,
}

pub struct Omnibox {
    scope: Scope,
    rows: Vec<Item>,
    highlighted: Option<usize>,
    input: Entity<TextInput>,
    focus_handle: FocusHandle,
    theme: Theme,
    metrics: Metrics,
    focused: bool,
    scroll: gpui::ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<OmniboxEvent> for Omnibox {}

impl Focusable for Omnibox {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Omnibox {
    pub fn new(scope: Scope, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) -> Self {
        let style = InputStyle::field(&theme, &metrics);
        let input = cx.new(|cx| {
            TextInput::new(style, cx)
                .with_key_context(text_input::BARE_CONTEXT)
                .with_placeholder(scope.placeholder())
        });
        let subscription = cx.subscribe(&input, |_: &mut Self, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                cx.emit(OmniboxEvent::QueryChanged);
            }
        });

        Self {
            scope,
            rows: Vec::new(),
            highlighted: None,
            input,
            focus_handle: cx.focus_handle(),
            theme,
            metrics,
            focused: false,
            scroll: gpui::ScrollHandle::default(),
            _subscriptions: vec![subscription],
        }
    }

    pub fn scope(&self) -> Scope {
        self.scope
    }

    pub fn query(&self, cx: &App) -> String {
        self.input.read(cx).text().to_owned()
    }

    pub fn apply_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        self.scope = scope;
        self.input.update(cx, |input, cx| {
            input.set_placeholder(scope.placeholder());
            input.set_text(String::new(), cx);
        });
        self.highlighted = None;
    }

    pub fn set_rows(&mut self, rows: Vec<Item>, reset_highlight: bool, cx: &mut Context<Self>) {
        let count = rows.len();
        self.rows = rows;
        self.highlighted = if count == 0 {
            None
        } else if reset_highlight {
            Some(0)
        } else {
            Some(self.highlighted.unwrap_or(0).min(count - 1))
        };
        cx.notify();
    }

    fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() - 1;
        let next = match self.highlighted {
            Some(current) => (current as i32 + delta).clamp(0, last as i32) as usize,
            None if delta > 0 => 0,
            None => last,
        };
        self.highlighted = Some(next);
        self.scroll.scroll_to_item(next);
        cx.notify();
    }

    fn move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(-1, cx);
    }

    fn move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(1, cx);
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self
            .highlighted
            .and_then(|index| self.rows.get(index))
            .map(|item| item.action.clone())
        else {
            return;
        };
        cx.emit(OmniboxEvent::Confirm(action));
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(OmniboxEvent::Dismiss);
    }

    fn results(&self, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        if self.rows.is_empty() {
            return div()
                .flex()
                .flex_grow()
                .items_center()
                .justify_center()
                .size_full()
                .child(
                    div()
                        .text_size(metrics.font_body())
                        .text_color(theme.fg_muted)
                        .child(SharedString::from(self.scope.empty_state())),
                )
                .into_any_element();
        }

        let mut list = div().flex().flex_col();
        let mut previous: Option<&str> = None;
        for (index, item) in self.rows.iter().enumerate() {
            if previous != Some(item.section.as_str()) {
                list = list.child(self.section_header(&item.section));
                previous = Some(item.section.as_str());
            }
            list = list.child(self.row(index, item, cx));
        }

        div()
            .id("omnibox-rows")
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(list)
            .into_any_element()
    }

    fn section_header(&self, title: &str) -> AnyElement {
        let metrics = &self.metrics;
        div()
            .flex()
            .flex_row()
            .items_center()
            .px(metrics.spacing6())
            .pt(metrics.spacing4())
            .pb(metrics.scaled(3.0))
            .child(
                div()
                    .text_size(metrics.font_caption())
                    .font_weight(FontWeight::BOLD)
                    .text_color(self.theme.fg_dim)
                    .child(SharedString::from(title.to_uppercase())),
            )
            .into_any_element()
    }

    fn row(&self, index: usize, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let highlighted = self.highlighted == Some(index);
        let action = item.action.clone();

        let mut labels = div().flex().flex_col().flex_grow().min_w(px(0.0)).child(
            div()
                .text_size(metrics.font_body())
                .text_color(theme.fg)
                .truncate()
                .child(SharedString::from(item.title.clone())),
        );
        if let Some(subtitle) = item.subtitle.as_deref().filter(|value| !value.is_empty()) {
            labels = labels.child(
                div()
                    .text_size(metrics.font_footnote())
                    .text_color(theme.fg_muted)
                    .truncate()
                    .child(SharedString::from(subtitle.to_owned())),
            );
        }

        div()
            .id(SharedString::from(format!("omnibox-row-{}", item.id)))
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing3())
            .px(metrics.spacing6())
            .h(metrics.scaled(ROW_HEIGHT))
            .flex_none()
            .cursor_pointer()
            .when(highlighted, |element| element.bg(theme.surface))
            .when(!highlighted, |element| {
                element.hover(|style| style.bg(theme.hover))
            })
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(OmniboxEvent::Confirm(action.clone()));
            }))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(metrics.scaled(16.0))
                    .child(SymbolGlyph::new(
                        &item.symbol,
                        metrics.font_body(),
                        theme.fg_muted,
                    )),
            )
            .child(labels)
            .into_any_element()
    }

    fn footer(&self) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let hint = |text: &str, label: &str| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(metrics.scaled(4.0))
                .child(
                    div()
                        .px(metrics.scaled(4.0))
                        .py(metrics.scaled(2.0))
                        .rounded(metrics.radius_sm())
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.border)
                        .text_size(metrics.font_caption())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg_muted)
                        .child(SharedString::from(text.to_owned())),
                )
                .child(
                    div()
                        .text_size(metrics.font_footnote())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.fg_dim)
                        .child(SharedString::from(label.to_owned())),
                )
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(metrics.scaled(18.0))
            .px(metrics.spacing5())
            .py(metrics.spacing4())
            .child(hint("↩", self.scope.return_label()))
            .child(hint("Tab/⇧Tab", "Navigate"))
            .child(hint("Esc", "Close"))
            .into_any_element()
    }

    fn search_field(&self) -> AnyElement {
        let metrics = &self.metrics;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing4())
            .px(metrics.spacing6())
            .py(metrics.spacing5())
            .child(SymbolGlyph::new(
                "magnifyingglass",
                metrics.font_emphasis(),
                self.theme.fg_muted,
            ))
            .child(
                div()
                    .flex_grow()
                    .min_w(px(0.0))
                    .h(metrics.scaled(28.0))
                    .flex()
                    .items_center()
                    .child(self.input.clone()),
            )
            .into_any_element()
    }
}

impl Render for Omnibox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = self.metrics;
        let theme = self.theme.clone();
        if !self.focused {
            self.focused = true;
            window.focus(&self.input.focus_handle(cx));
        }

        let divider = || div().h(px(1.0)).flex_none().bg(theme.border);

        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .key_context(KEY_CONTEXT)
                    .track_focus(&self.focus_handle)
                    .on_action(cx.listener(Self::move_up))
                    .on_action(cx.listener(Self::move_down))
                    .on_action(cx.listener(Self::confirm))
                    .on_action(cx.listener(Self::dismiss))
                    .occlude()
                    .mt(metrics.scaled(PANEL_TOP))
                    .flex()
                    .flex_col()
                    .w(metrics.scaled(PANEL_WIDTH))
                    .h(metrics.scaled(PANEL_HEIGHT))
                    .rounded(metrics.radius_xl())
                    .bg(theme.bg)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .overflow_hidden()
                    .child(self.search_field())
                    .child(divider())
                    .child(self.results(cx))
                    .child(divider())
                    .child(self.footer()),
            )
    }
}
