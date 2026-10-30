//! The titlebar's Open in IDE control: a button that opens the current
//! project in the chosen app, and a menu of the others.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, StatefulInteractiveElement, Styled, Window, canvas, div, px,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::titlebar::with_tooltip;
use crate::model::AppModel;
use crate::model::project_apps::Listed;

/// The app the main button opens, while the current project can be opened.
fn shown(model: &AppModel) -> Option<&Listed> {
    model.project_apps_directory()?;
    model.default_project_app()
}

fn label_width(model: &AppModel) -> Pixels {
    model.metrics.scaled(112.0)
}

/// The room the tab strip keeps before the control hides its app's name.
const TAB_ROOM: f32 = 160.0;

/// How wide the control is, with or without its app's name.
fn width(model: &AppModel, shown: &Listed, named: bool, window: &Window) -> f32 {
    let m = model.metrics;
    let name = if named {
        let mut style = window.text_style();
        style.font_weight = FontWeight::SEMIBOLD;
        window
            .text_system()
            .shape_line(
                shown.app.name.clone().into(),
                m.font_body(),
                &[style.to_run(shown.app.name.len())],
                None,
            )
            .width
            .min(label_width(model))
            + m.spacing3()
    } else {
        px(0.0)
    };
    f32::from(
        m.icon_md()
            + name
            + m.spacing4() * 2.0
            + px(1.0)
            + m.control_medium()
            + px(2.0)
            + m.spacing2(),
    )
}

/// The control and how wide it is, given the `room` the titlebar has left
/// for it and the tab strip. It shows only its app's icon when its name
/// would leave the strip too narrow.
pub(super) fn control(
    model: &AppModel,
    room: f32,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> Option<(AnyElement, f32)> {
    let shown = shown(model)?;
    let named_width = width(model, shown, true, window);
    let named = room - named_width >= TAB_ROOM;
    let width = if named {
        named_width
    } else {
        width(model, shown, false, window)
    };
    Some((render(shown, named, model, cx), width))
}

fn render(shown: &Listed, named: bool, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let anchor = model.project_apps.anchor.clone();
    div()
        .debug_selector(|| "open-in-ide".into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .h(m.control_medium())
        .mr(m.spacing2())
        .rounded(m.radius_md())
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .overflow_hidden()
        .text_size(m.font_body())
        .text_color(theme.fg)
        .child(
            canvas(
                move |bounds, _, _| anchor.set(Some(bounds)),
                |_, (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(primary_button(shown, named, model, cx))
        .child(
            div()
                .flex_none()
                .w(px(1.0))
                .h(m.scaled(14.0))
                .bg(theme.border),
        )
        .child(menu_button(model, cx))
        .into_any_element()
}

/// Opens the project in the app it shows.
fn primary_button(
    shown: &Listed,
    named: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let icon = shown.icon.clone().map_or_else(
        || {
            IconGlyph::new(
                if shown.app.is_file_manager() {
                    Icon::Folder
                } else {
                    Icon::Code
                },
                m.icon_md(),
                theme.fg,
            )
            .into_any_element()
        },
        |image| gpui::img(image).size(m.icon_md()).into_any_element(),
    );
    with_tooltip(
        div()
            .id("open-in-ide-primary")
            .debug_selector(|| "open-in-ide-primary".into())
            .flex()
            .items_center()
            .gap(m.spacing3())
            .h_full()
            .px(m.spacing4())
            .cursor_pointer()
            .hover(|style| style.bg(theme.hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|model, _, _, cx| {
                cx.stop_propagation();
                model.open_project_in_default(cx);
            }))
            .child(icon)
            .when(named, |button| {
                button.child(
                    div()
                        .debug_selector(|| "open-in-ide-name".into())
                        .max_w(label_width(model))
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(shown.app.name.clone()),
                )
            }),
        tr!("Open Project in %@", shown.app.name.clone()),
        model,
    )
}

/// Drops down the other apps.
fn menu_button(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    with_tooltip(
        div()
            .id("open-in-ide-menu")
            .debug_selector(|| "open-in-ide-menu".into())
            .flex()
            .items_center()
            .justify_center()
            .w(m.control_medium())
            .h_full()
            .cursor_pointer()
            .hover(|style| style.bg(theme.hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|model, _, window, cx| {
                cx.stop_propagation();
                model.open_project_apps_menu(window, cx);
            }))
            .child(IconGlyph::new(
                Icon::ChevronDown,
                m.icon_xs(),
                theme.fg_muted,
            )),
        tr!("Choose App to Open Project"),
        model,
    )
}
