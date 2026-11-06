//! A small window that follows the pointer outside the app while a tab or
//! pane is dragged there, showing where it will go: a window of its own.

use gpui::{
    AnyElement, App, AppContext, Bounds, Context, FontWeight, Hsla, IntoElement, ParentElement,
    Pixels, Point, Render, SharedString, Styled, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, div, point, px, size,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::tr;

/// Where in the ghost the pointer holds it: just above and left of its
/// corner, so the chip sits below and right of the pointer.
pub(super) const GRAB: Point<Pixels> = point(px(-14.0), px(-14.0));

const SIZE: (f32, f32) = (280.0, 64.0);

pub(super) struct Chip {
    pub(super) label: SharedString,
    pub(super) icon: Icon,
    pub(super) theme: Theme,
    pub(super) metrics: Metrics,
}

pub(super) struct Ghost {
    chip: Chip,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let chip = &self.chip;
        div().p(px(6.0)).child(chip_body(
            &chip.label,
            chip.icon,
            Some((tr!("New window"), chip.theme.accent)),
            &chip.theme,
            chip.metrics,
        ))
    }
}

pub(super) fn open(chip: Chip, cx: &mut App) -> Option<WindowHandle<Ghost>> {
    let cursor = super::cursor()?;
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(SIZE.0), px(SIZE.1)),
        ))),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..crate::window_options()
    };
    cx.open_window(options, move |window, cx| {
        muxy_ui::screen::set_ignores_mouse(window, true);
        muxy_ui::screen::move_window_to(window, cursor, GRAB);
        cx.new(|_| Ghost { chip })
    })
    .ok()
}

pub(super) fn close(handle: WindowHandle<Ghost>, cx: &mut App) {
    cx.defer(move |cx| {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    });
}

/// The chip that shows what moves and, under it, where it goes.
pub(super) fn chip_body(
    label: &SharedString,
    icon: Icon,
    caption: Option<(SharedString, Hsla)>,
    theme: &Theme,
    metrics: Metrics,
) -> AnyElement {
    let border = caption.as_ref().map_or(theme.border, |(_, color)| *color);
    div()
        .flex()
        .items_center()
        .gap(metrics.spacing3())
        .max_w(px(SIZE.0 - 12.0))
        .px(metrics.spacing4())
        .py(metrics.spacing2())
        .rounded(metrics.radius_lg())
        .bg(theme.surface)
        .border_1()
        .border_color(border.opacity(0.8))
        .shadow(muxy_ui::theme::Elevation::Elevated.shadow(theme.bg))
        .text_color(theme.fg)
        .text_size(metrics.font_body())
        .child(IconGlyph::new(icon, metrics.scaled(14.0), theme.fg_muted))
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(0.0))
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .child(label.clone()),
                )
                .children(caption.map(|(caption, color)| {
                    div()
                        .text_size(metrics.font_footnote())
                        .text_color(color)
                        .child(caption)
                })),
        )
        .into_any_element()
}
