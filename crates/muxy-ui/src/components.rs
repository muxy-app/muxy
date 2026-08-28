use crate::icon::Icon;
use gpui::{
    App, AppContext, ClickEvent, Context, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct IconGlyph {
    icon: Icon,
    size: Pixels,
    color: Hsla,
    hover: Option<(SharedString, Hsla)>,
}

impl IconGlyph {
    pub fn new(icon: Icon, size: Pixels, color: Hsla) -> Self {
        Self {
            icon,
            size,
            color,
            hover: None,
        }
    }

    pub fn hover_in_group(mut self, group: impl Into<SharedString>, color: Hsla) -> Self {
        self.hover = Some((group.into(), color));
        self
    }
}

impl RenderOnce for IconGlyph {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let scale = window.scale_factor();
        let (width, height) = natural_size(self.icon, self.size, scale);
        match self.hover {
            None => layer(self.icon, self.size, self.color, scale).into_any_element(),
            Some((group, hover_color)) => div()
                .relative()
                .flex()
                .flex_none()
                .w(width)
                .h(height)
                .child(
                    layer(self.icon, self.size, self.color, scale)
                        .absolute()
                        .group_hover(group.clone(), |style| style.opacity(0.0)),
                )
                .child(
                    layer(self.icon, self.size, hover_color, scale)
                        .absolute()
                        .opacity(0.0)
                        .group_hover(group, |style| style.opacity(1.0)),
                )
                .into_any_element(),
        }
    }
}

#[cfg(target_os = "macos")]
fn natural_size(icon: Icon, size: Pixels, scale: f32) -> (Pixels, Pixels) {
    crate::icon::tinted(icon, size, gpui::black(), scale)
        .map(|glyph| (glyph.width, glyph.height))
        .unwrap_or((size, size))
}

#[cfg(not(target_os = "macos"))]
fn natural_size(_icon: Icon, size: Pixels, _scale: f32) -> (Pixels, Pixels) {
    (size, size)
}

#[cfg(target_os = "macos")]
fn layer(icon: Icon, size: Pixels, color: Hsla, scale: f32) -> gpui::Div {
    let Some(glyph) = crate::icon::tinted(icon, size, color, scale) else {
        return svg_layer(icon, size, color);
    };
    div().flex_none().w(glyph.width).h(glyph.height).child(
        gpui::img(glyph.image)
            .w(glyph.width)
            .h(glyph.height)
            .flex_none(),
    )
}

#[cfg(not(target_os = "macos"))]
fn layer(icon: Icon, size: Pixels, color: Hsla, _scale: f32) -> gpui::Div {
    svg_layer(icon, size, color)
}

fn svg_layer(icon: Icon, size: Pixels, color: Hsla) -> gpui::Div {
    div().flex_none().size(size).child(
        gpui::svg()
            .path(icon.path())
            .size(size)
            .flex_none()
            .text_color(color),
    )
}

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: Icon,
    glyph_size: Pixels,
    box_size: Pixels,
    color: Hsla,
    hover_color: Hsla,
    tooltip: Option<(SharedString, Hsla, Hsla, Hsla)>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(
        id: impl Into<ElementId>,
        icon: Icon,
        glyph_size: Pixels,
        box_size: Pixels,
        color: Hsla,
        hover_color: Hsla,
    ) -> Self {
        Self {
            id: id.into(),
            icon,
            glyph_size,
            box_size,
            color,
            hover_color,
            tooltip: None,
            on_click: None,
        }
    }

    pub fn tooltip(
        mut self,
        text: impl Into<SharedString>,
        background: Hsla,
        foreground: Hsla,
        border: Hsla,
    ) -> Self {
        self.tooltip = Some((text.into(), background, foreground, border));
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let group = SharedString::from(format!("icon-button-{}", self.id));
        let tooltip = self.tooltip;
        let mut button = div()
            .id(self.id)
            .group(group.clone())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.box_size)
            .cursor_pointer()
            .child(
                IconGlyph::new(self.icon, self.glyph_size, self.color)
                    .hover_in_group(group, self.hover_color),
            );
        if let Some(handler) = self.on_click {
            button = button.on_click(move |event, window, cx| handler(event, window, cx));
        }
        if let Some((text, background, foreground, border)) = tooltip {
            button = button.tooltip(move |_, cx| {
                cx.new(|_| Tooltip::new(text.clone(), background, foreground, border))
                    .into()
            });
        }
        button
    }
}

pub struct Tooltip {
    text: SharedString,
    background: Hsla,
    foreground: Hsla,
    border: Hsla,
}

impl Tooltip {
    pub fn new(
        text: impl Into<SharedString>,
        background: Hsla,
        foreground: Hsla,
        border: Hsla,
    ) -> Self {
        Self {
            text: text.into(),
            background,
            foreground,
            border,
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(5.0))
            .bg(self.background)
            .border_1()
            .border_color(self.border)
            .text_size(px(11.0))
            .text_color(self.foreground)
            .shadow_sm()
            .child(self.text.clone())
    }
}

#[derive(IntoElement)]
pub struct Separator {
    color: Hsla,
}

impl Separator {
    pub fn new(color: Hsla) -> Self {
        Self { color }
    }
}

impl RenderOnce for Separator {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().w(px(1.0)).h_full().flex_none().bg(self.color)
    }
}

#[derive(IntoElement)]
pub struct SymbolGlyph {
    symbol: SharedString,
    size: Pixels,
    color: Hsla,
}

impl SymbolGlyph {
    pub fn new(symbol: impl Into<SharedString>, size: Pixels, color: Hsla) -> Self {
        Self {
            symbol: symbol.into(),
            size,
            color,
        }
    }
}

impl RenderOnce for SymbolGlyph {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        symbol_layer(&self.symbol, self.size, self.color, window.scale_factor())
    }
}

#[cfg(target_os = "macos")]
fn symbol_layer(symbol: &SharedString, size: Pixels, color: Hsla, scale: f32) -> gpui::Div {
    let Some(glyph) = crate::icon::tinted_symbol(symbol, size, color, scale) else {
        return fallback_layer(symbol, size, color);
    };
    div().flex_none().w(glyph.width).h(glyph.height).child(
        gpui::img(glyph.image)
            .w(glyph.width)
            .h(glyph.height)
            .flex_none(),
    )
}

#[cfg(not(target_os = "macos"))]
fn symbol_layer(symbol: &SharedString, size: Pixels, color: Hsla, _scale: f32) -> gpui::Div {
    fallback_layer(symbol, size, color)
}

fn fallback_layer(symbol: &SharedString, size: Pixels, color: Hsla) -> gpui::Div {
    match Icon::from_symbol(symbol) {
        Some(icon) => svg_layer(icon, size, color),
        None => div().flex_none().size(size),
    }
}
