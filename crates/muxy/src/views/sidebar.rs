use crate::state::AppState;
use crate::views::app::AppLayout;
use crate::views::project_row::{collapsed_row, expanded_row};
use crate::views::sidebar_footer::sidebar_footer;
use crate::views::workspace_switcher::{sort_button, workspace_switcher};
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;

use crate::views::window::MainWindow;

pub fn sidebar(state: &AppState, layout: AppLayout, cx: &mut Context<MainWindow>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h(px(0.0))
        .child(project_list(state, layout, cx))
        .child(sidebar_footer(state, layout, cx))
        .into_any_element()
}

fn project_list(state: &AppState, layout: AppLayout, cx: &mut Context<MainWindow>) -> AnyElement {
    let metrics = &state.metrics;
    let wide = layout.wide_sidebar;
    let horizontal = if wide {
        metrics.spacing3()
    } else {
        metrics.spacing4()
    };

    let mut header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing2())
        .px(horizontal)
        .pt(metrics.spacing2())
        .child(workspace_switcher(state, layout, cx));
    if wide {
        header = header.child(sort_button(state));
    }

    let mut rows = div()
        .flex()
        .flex_col()
        .gap(metrics.spacing3())
        .px(horizontal)
        .pt(if wide { px(0.0) } else { metrics.spacing2() })
        .pb(metrics.spacing2());

    if !wide {
        rows = rows.items_center();
    }

    let visible = state.workspace.visible_projects();
    let pinned_boundary = visible
        .iter()
        .rposition(|project| project.is_pinned)
        .filter(|index| *index + 1 < visible.len());

    for (index, project) in visible.iter().enumerate() {
        let id = project.id.clone();
        let row = if wide {
            expanded_row(state, project)
        } else {
            collapsed_row(state, project)
        };
        let menu_id = id.clone();
        let mut entry = div().flex().flex_col().child(
            div()
                .id(gpui::ElementId::Name(SharedString::from(format!(
                    "row-{id}"
                ))))
                .on_click(cx.listener(move |window: &mut MainWindow, _, _, cx| {
                    window.state.select_project(&id);
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(
                        move |window: &mut MainWindow, event: &MouseDownEvent, view, cx| {
                            window.open_project_menu(&menu_id, event.position, view, cx);
                        },
                    ),
                )
                .child(row),
        );

        if pinned_boundary == Some(index) {
            entry = entry.child(
                div()
                    .h(px(1.0))
                    .mx(metrics.spacing2())
                    .mt(metrics.spacing3() / 2.0)
                    .bg(state.theme.border),
            );
        }

        rows = rows.child(entry);
    }

    rows = rows.child(add_project_button(state, layout, cx));

    let mut list = div()
        .flex()
        .flex_col()
        .flex_grow()
        .min_h(px(0.0))
        .gap(metrics.spacing3())
        .child(header);

    if wide && state.prefs.show_project_search {
        list = list.child(search_field(state));
    }

    list.child(
        div()
            .id("project-scroll")
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .child(rows),
    )
    .into_any_element()
}

fn search_field(state: &AppState) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    div()
        .px(metrics.spacing3())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(metrics.spacing2())
                .px(metrics.spacing4())
                .h(metrics.control_medium())
                .rounded(metrics.radius_md())
                .bg(theme.surface)
                .child(IconGlyph::new(
                    Icon::Search,
                    metrics.font_footnote(),
                    theme.fg_muted,
                ))
                .child(
                    div()
                        .flex_grow()
                        .text_size(metrics.font_caption())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg_muted)
                        .child(SharedString::from("Search projects")),
                ),
        )
        .into_any_element()
}

fn add_project_button(
    state: &AppState,
    layout: AppLayout,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let wide = layout.wide_sidebar;

    let tile = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(metrics.icon_xxl())
        .rounded(metrics.radius_md())
        .bg(if wide { theme.surface } else { theme.hover })
        .child(IconGlyph::new(
            Icon::Plus,
            metrics.font_emphasis(),
            theme.fg_muted,
        ));

    if !wide {
        return div()
            .id("add-project")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .p(metrics.scaled(3.0))
            .cursor_pointer()
            .on_click(cx.listener(
                |window: &mut MainWindow, event: &gpui::ClickEvent, view, cx| {
                    window.open_add_project_menu(event.position(), view, cx);
                },
            ))
            .child(tile)
            .into_any_element();
    }

    div()
        .id("add-project")
        .on_click(cx.listener(
            |window: &mut MainWindow, event: &gpui::ClickEvent, view, cx| {
                window.open_add_project_menu(event.position(), view, cx);
            },
        ))
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4())
        .p(metrics.spacing2())
        .rounded(metrics.radius_lg())
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover))
        .child(tile)
        .child(
            div()
                .text_size(metrics.font_body())
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.fg_muted)
                .child(SharedString::from("Add Project")),
        )
        .into_any_element()
}
