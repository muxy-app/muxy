use crate::state::AppState;
use gpui::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, Styled,
    div, px,
};
use muxy_core::prefs::home_dir;
use muxy_ui::components::{IconGlyph, Separator};
use muxy_ui::icon::Icon;

const PATH_MAX_CHARACTERS: usize = 40;

pub fn status_bar(state: &AppState, working_directory: Option<&str>) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    let mut left = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .h_full()
        .flex_grow();

    let path =
        working_directory.or_else(|| state.active_project().map(|project| project.path.as_str()));
    if let Some(path) = path {
        left = left
            .child(path_chip(state, path))
            .child(Separator::new(theme.border));
    }

    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .h(metrics.status_bar_height())
        .bg(theme.bg)
        .border_t(px(1.0))
        .border_color(theme.border)
        .child(left)
        .child(
            div()
                .flex()
                .flex_row()
                .flex_none()
                .items_center()
                .gap(px(8.0))
                .h_full()
                .child(Separator::new(theme.border))
                .child(status_button(state, "extension-output", Icon::Bug))
                .child(Separator::new(theme.border))
                .child(status_button(state, "resource-usage", Icon::Cpu)),
        )
        .into_any_element()
}

fn path_chip(state: &AppState, path: &str) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    div()
        .id("status-path")
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .h_full()
        .text_color(theme.fg_muted)
        .cursor_pointer()
        .child(IconGlyph::new(
            Icon::Folder,
            metrics.font_caption(),
            theme.fg_muted,
        ))
        .child(
            div()
                .text_size(metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .child(SharedString::from(truncate_path(&abbreviate_path(path)))),
        )
        .into_any_element()
}

fn status_button(state: &AppState, id: &'static str, icon: Icon) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    div()
        .id(id)
        .group(id)
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .px(px(4.0))
        .cursor_pointer()
        .child(
            IconGlyph::new(icon, metrics.font_caption(), theme.fg_muted)
                .hover_in_group(id, theme.fg),
        )
        .into_any_element()
}

fn abbreviate_path(path: &str) -> String {
    let home = home_dir();
    let home = home.to_string_lossy();
    match !home.is_empty() && path.starts_with(home.as_ref()) {
        true => format!("~{}", &path[home.len()..]),
        false => path.to_owned(),
    }
}

fn truncate_path(path: &str) -> String {
    if path.chars().count() <= PATH_MAX_CHARACTERS {
        return path.to_owned();
    }
    let suffix: String = path
        .chars()
        .skip(path.chars().count() - (PATH_MAX_CHARACTERS - 1))
        .collect();
    format!("…{suffix}")
}
