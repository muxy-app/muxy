use gpui::{
    AnyElement, App, AppContext, Bounds, Context, FontWeight, IntoElement, ParentElement, Pixels,
    Point, Render, SharedString, Styled, WeakEntity, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, div, point, px, size,
};

use crate::model::AppModel;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::theme::{Metrics, Theme};

const PAD: f32 = 12.0;

pub(super) const GRAB: Point<Pixels> =
    point(px(PAD - super::CHIP_OFFSET), px(PAD - super::CHIP_OFFSET));

pub(super) const WIDTH: f32 = 260.0;

pub(super) struct Chip {
    pub(super) model: WeakEntity<AppModel>,
    pub(super) moving: super::Moving,
    pub(super) label: SharedString,
    pub(super) theme: Theme,
    pub(super) metrics: Metrics,
}

pub(super) struct Ghost {
    chip: Chip,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chip = &self.chip;
        let glyph = chip.model.upgrade().map_or_else(
            || {
                IconGlyph::new(Icon::Terminal, chip.metrics.icon_md(), chip.theme.fg_muted)
                    .into_any_element()
            },
            |model| super::glyph(chip.moving, model.read(cx), cx),
        );
        div().flex().items_start().p(px(PAD)).child(chip_body(
            &chip.label,
            glyph,
            &chip.theme,
            chip.metrics,
        ))
    }
}

pub(super) fn open(chip: Chip, cx: &mut App) -> Option<WindowHandle<Ghost>> {
    let cursor = super::cursor()?;
    let height = f32::from(chip.metrics.control_large());
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(WIDTH + PAD * 2.0), px(height + PAD * 2.0)),
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
        muxy_ui::screen::make_bare(window);
        muxy_ui::screen::set_window_alpha(window, 0.0);
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

pub(super) fn chip_body(
    label: &SharedString,
    glyph: AnyElement,
    theme: &Theme,
    metrics: Metrics,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(metrics.spacing3())
        .max_w(px(WIDTH))
        .h(metrics.control_large())
        .px(metrics.spacing5())
        .rounded(metrics.radius_lg())
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border_solid())
        .shadow(muxy_ui::theme::Elevation::Elevated.shadow(theme.bg))
        .text_color(theme.fg)
        .text_size(metrics.font_body())
        .child(div().flex().flex_none().items_center().child(glyph))
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .child(label.clone()),
        )
        .into_any_element()
}
