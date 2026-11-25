use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Role,
    StatefulInteractiveElement, Styled, canvas, div, px,
};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::titlebar::with_tooltip;
use crate::model::AppModel;
use crate::model::project_apps::Listed;

fn shown(model: &AppModel) -> Option<&Listed> {
    if !model.appearance.open_in_app_button {
        return None;
    }
    model.project_apps_directory()?;
    model.default_project_app()
}

fn width(model: &AppModel) -> f32 {
    let m = model.metrics;
    f32::from(
        m.icon_md() + m.spacing4() * 2.0 + px(1.0) + m.control_medium() + px(2.0) + m.spacing2(),
    )
}

pub(super) fn control(model: &AppModel, cx: &mut Context<AppModel>) -> Option<(AnyElement, f32)> {
    let shown = shown(model)?;
    Some((render(shown, model, cx), width(model)))
}

fn render(shown: &Listed, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
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
        .child(primary_button(shown, model, cx))
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

fn primary_button(
    shown: &Listed,
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
    let name = tr!("Open Project in %@", shown.app.name.clone());
    with_tooltip(
        div()
            .id("open-in-ide-primary")
            .role(Role::Button)
            .accessibility_id("open-in-ide-primary")
            .aria_label(name.clone())
            .debug_selector(|| "open-in-ide-primary".into())
            .flex()
            .items_center()
            .h_full()
            .px(m.spacing4())
            .cursor_pointer()
            .hover(|style| style.bg(theme.hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|model, _, _, cx| {
                cx.stop_propagation();
                model.open_project_in_default(cx);
            }))
            .child(icon),
        model.tooltip(name, ShortcutId::OpenProjectInDefaultApp),
        model,
    )
}

fn menu_button(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let name = tr!("Choose App to Open Project");
    with_tooltip(
        div()
            .id("open-in-ide-menu")
            .role(Role::Button)
            .accessibility_id("open-in-ide-menu")
            .aria_label(name.clone())
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
        name,
        model,
    )
}
