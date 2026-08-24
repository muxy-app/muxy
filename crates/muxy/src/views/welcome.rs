use crate::state::AppState;
use gpui::{
    AnyElement, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, Styled,
    div, px, rgb,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;

const ACCENT_BUTTON: u32 = 0x0a7cff;

pub fn workspace_content(state: &AppState) -> AnyElement {
    let theme = &state.theme;
    let content = match state.active_project() {
        Some(project) => empty_project(state, &project.name),
        None => welcome(state),
    };

    div()
        .flex()
        .flex_col()
        .flex_grow()
        .min_h(px(0.0))
        .bg(theme.bg)
        .child(content)
        .into_any_element()
}

fn welcome(state: &AppState) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .child(
            div()
                .text_size(metrics.font_emphasis())
                .text_color(theme.fg_dim)
                .child(SharedString::from("No project selected")),
        )
        .into_any_element()
}

fn empty_project(state: &AppState, name: &str) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(metrics.spacing7())
        .size_full()
        .child(IconGlyph::new(
            Icon::AppWindow,
            metrics.font_mega(),
            theme.fg_muted,
        ))
        .child(
            div()
                .text_size(metrics.font_headline())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg)
                .child(SharedString::from(format!("No tabs in {name}"))),
        )
        .child(
            div()
                .max_w(metrics.scaled(360.0))
                .text_size(metrics.font_body())
                .text_color(theme.fg_muted)
                .text_center()
                .child(SharedString::from(
                    "Open a new terminal tab to start working in this project.",
                )),
        )
        .child(
            div()
                .id("new-tab")
                .flex()
                .flex_row()
                .items_center()
                .gap(metrics.spacing4())
                .px(metrics.spacing6())
                .py(metrics.spacing3())
                .rounded(metrics.radius_md())
                .bg(rgb(ACCENT_BUTTON))
                .text_color(gpui::white())
                .text_size(metrics.font_body())
                .cursor_pointer()
                .child(SharedString::from("New Tab"))
                .child(
                    div()
                        .text_size(metrics.font_footnote())
                        .font_weight(FontWeight::MEDIUM)
                        .opacity(0.72)
                        .child(SharedString::from("⌘T")),
                ),
        )
        .into_any_element()
}
