use crate::state::AppState;
use crate::views::app::AppLayout;
use crate::views::menu::Item;
use crate::views::window::MainWindow;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, Styled, div,
};
use muxy_core::prefs::SortMode;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;

pub fn workspace_switcher(
    state: &AppState,
    layout: AppLayout,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    if !layout.wide_sidebar {
        return div()
            .id("workspace-switcher")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(metrics.icon_xxl())
            .rounded(metrics.radius_sm())
            .bg(theme.surface)
            .hover(|style| style.bg(theme.hover))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(
                    move |window: &mut MainWindow, event: &MouseDownEvent, view, cx| {
                        window.open_workspace_menu(event.position, view, cx);
                    },
                ),
            )
            .child(IconGlyph::new(
                Icon::ChevronDown,
                metrics.font_caption(),
                theme.fg_muted,
            ))
            .into_any_element();
    }

    div()
        .id("workspace-switcher")
        .flex()
        .flex_row()
        .flex_grow()
        .items_center()
        .gap(metrics.spacing2())
        .px(metrics.spacing4())
        .h(metrics.control_medium())
        .rounded(metrics.radius_md())
        .bg(theme.surface)
        .hover(|style| style.bg(theme.hover))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(
                move |window: &mut MainWindow, event: &MouseDownEvent, view, cx| {
                    window.open_workspace_menu(event.position, view, cx);
                },
            ),
        )
        .child(
            div()
                .flex_grow()
                .text_size(metrics.font_caption())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .truncate()
                .child(SharedString::from(state.workspace.active_group_name())),
        )
        .child(IconGlyph::new(
            Icon::ChevronDown,
            metrics.font_caption(),
            theme.fg_muted,
        ))
        .into_any_element()
}

pub fn sort_button(state: &AppState, cx: &mut Context<MainWindow>) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    div()
        .id("sort-projects")
        .group("sort-projects")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(metrics.control_medium())
        .rounded(metrics.radius_md())
        .bg(theme.surface)
        .hover(|style| style.bg(theme.hover))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(
                move |window: &mut MainWindow, event: &MouseDownEvent, view, cx| {
                    window.open_sort_menu(event.position, view, cx);
                },
            ),
        )
        .child(
            IconGlyph::new(Icon::ArrowUpDown, metrics.font_caption(), theme.fg_muted)
                .hover_in_group("sort-projects", theme.accent),
        )
        .into_any_element()
}

pub fn sort_menu_items(current: SortMode) -> Vec<Item> {
    [
        ("Manual", SortMode::Manual),
        ("Name (A–Z)", SortMode::NameAscending),
        ("Name (Z–A)", SortMode::NameDescending),
        ("Recently Active", SortMode::RecentlyActive),
        ("Date Added", SortMode::DateCreated),
    ]
    .into_iter()
    .map(|(label, mode)| {
        Item::action(label, crate::command::Command::SetProjectSort(mode)).checked(mode == current)
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::views::menu::Item;
    use muxy_core::prefs::SortMode;

    #[test]
    fn chrome_sort_menu_lists_all_modes_and_checks_the_current_one() {
        let items = sort_menu_items(SortMode::RecentlyActive);
        assert_eq!(items.len(), 5);
        let expected = [
            ("Manual", SortMode::Manual),
            ("Name (A–Z)", SortMode::NameAscending),
            ("Name (Z–A)", SortMode::NameDescending),
            ("Recently Active", SortMode::RecentlyActive),
            ("Date Added", SortMode::DateCreated),
        ];
        for (item, (label, mode)) in items.iter().zip(expected) {
            let Item::Action {
                label: actual_label,
                command,
                checked,
                ..
            } = item
            else {
                panic!("sort item must be an action");
            };
            assert_eq!(actual_label.as_ref(), label);
            assert_eq!(command, &Command::SetProjectSort(mode));
            assert_eq!(*checked, mode == SortMode::RecentlyActive);
        }
    }
}
