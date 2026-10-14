//! The browser's toolbar, address suggestions, and find bar.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Focusable, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, canvas, deferred, div,
    img, px, relative,
};
use muxy_ui::components::{IconButton, IconGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::{BrowserView, BrowserViewEvent, Menu};

pub(super) fn render(
    view: &BrowserView,
    window: &mut Window,
    cx: &mut Context<BrowserView>,
) -> AnyElement {
    let theme = &view.theme;
    let metrics = view.metrics;
    let button = |id: &'static str, icon, tooltip: SharedString, enabled: bool| {
        IconButton::new(
            id,
            icon,
            metrics.icon_md(),
            metrics.control_medium(),
            if enabled {
                theme.fg_muted
            } else {
                theme.fg_dim
            },
            if enabled { theme.fg } else { theme.fg_dim },
        )
        .tooltip(tooltip, theme.surface, theme.fg, theme.border, theme.bg)
    };
    let loading = view.page.loading;
    let editing = view.address.focus_handle(cx).is_focused(window);
    let progress = view.page.progress.clamp(0.0, 1.0);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "A fraction of the toolbar width"
    )]
    let progress = progress as f32;
    let address = address(view, editing, cx);
    div()
        .flex()
        .flex_col()
        .flex_none()
        .child(
            div()
                .id("browser-toolbar")
                .debug_selector(|| "browser-toolbar".into())
                .flex()
                .items_center()
                .gap(metrics.spacing2())
                .h(metrics.header_height())
                .px(metrics.spacing3())
                .bg(theme.surface)
                .border_b_1()
                .border_color(theme.border)
                .child(
                    button(
                        "browser-back",
                        Icon::ChevronLeft,
                        tr!("Back"),
                        view.page.can_go_back,
                    )
                    .on_click(cx.listener(|view, _, _, cx| view.go_back(cx))),
                )
                .child(
                    button(
                        "browser-forward",
                        Icon::ChevronRight,
                        tr!("Forward"),
                        view.page.can_go_forward,
                    )
                    .on_click(cx.listener(|view, _, _, cx| view.go_forward(cx))),
                )
                .child(if loading {
                    button("browser-stop", Icon::X, tr!("Stop"), true)
                        .on_click(cx.listener(|view, _, _, cx| view.stop(cx)))
                } else {
                    button("browser-reload", Icon::RotateCw, tr!("Reload"), true)
                        .on_click(cx.listener(|view, _, _, cx| view.reload(cx)))
                })
                .child(address)
                .child(external_button(view, cx))
                .child(profile_button(view, cx)),
        )
        .child(
            div()
                .h(px(2.0))
                .w_full()
                .when(loading && progress < 1.0, |bar| {
                    bar.child(
                        div()
                            .h_full()
                            .w(relative(progress.max(0.05)))
                            .bg(theme.accent),
                    )
                }),
        )
        .into_any_element()
}

fn external_button(view: &BrowserView, cx: &mut Context<BrowserView>) -> AnyElement {
    let glyph = IconGlyph::new(
        Icon::ExternalLink,
        view.metrics.icon_md(),
        view.theme.fg_muted,
    );
    menu_button(
        view,
        "browser-external",
        div().child(glyph),
        tr!("Open in External Browser"),
        Menu::External,
        cx,
    )
}

/// The profile the page browses with, which a click lets you change.
fn profile_button(view: &BrowserView, cx: &mut Context<BrowserView>) -> AnyElement {
    let (theme, metrics) = (&view.theme, view.metrics);
    let content = div()
        .flex()
        .items_center()
        .gap(metrics.spacing2())
        .max_w(metrics.scaled(140.0))
        .child(IconGlyph::new(
            Icon::CircleUser,
            metrics.icon_md(),
            theme.fg_muted,
        ))
        .child(
            div()
                .truncate()
                .text_size(metrics.font_caption())
                .text_color(theme.fg_muted)
                .child(view.profile.name().to_owned()),
        );
    menu_button(
        view,
        "browser-profile",
        content,
        tr!("Browser Profile"),
        Menu::Profile,
        cx,
    )
}

/// The address field, with the page's icon and history suggestions.
fn address(
    view: &BrowserView,
    editing: bool,
    cx: &mut Context<BrowserView>,
) -> gpui::Stateful<gpui::Div> {
    let theme = &view.theme;
    let metrics = view.metrics;
    div()
        .id("browser-address")
        .debug_selector(|| "browser-address".into())
        .relative()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .items_center()
        .gap(metrics.spacing3())
        .h(metrics.control_medium())
        .px(metrics.spacing4())
        .rounded(metrics.radius_md())
        .border_1()
        .border_color(if editing { theme.accent } else { theme.border })
        .bg(theme.bg)
        .child(match view.favicon.clone().filter(|_| !editing) {
            Some(favicon) => img(favicon)
                .size(metrics.icon_md())
                .flex_none()
                .into_any_element(),
            None => IconGlyph::new(Icon::Globe, metrics.icon_sm(), theme.fg_dim).into_any_element(),
        })
        .child(div().flex_1().min_w(px(0.0)).child(view.address.clone()))
        .child({
            let width = view.address_width.clone();
            canvas(
                move |bounds, _, _| width.set(bounds.size.width),
                |_, (), _, _| (),
            )
            .absolute()
            .size_full()
        })
        .children(suggestions(view, cx))
        .capture_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _, cx| {
            let keystroke = &event.keystroke;
            if keystroke.modifiers.number_of_modifiers() == 0
                && matches!(keystroke.key.as_str(), "up" | "down")
                && view.move_suggestion(keystroke.key == "down", cx)
            {
                cx.stop_propagation();
            }
        }))
}

fn menu_button(
    view: &BrowserView,
    id: &'static str,
    content: gpui::Div,
    tooltip: SharedString,
    menu: Menu,
    cx: &mut Context<BrowserView>,
) -> AnyElement {
    let theme = &view.theme;
    let metrics = view.metrics;
    let (background, foreground, border, theme_background) =
        (theme.surface, theme.fg, theme.border, theme.bg);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(metrics.control_medium())
        .px(metrics.spacing3())
        .rounded(metrics.radius_sm())
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover))
        .tooltip(move |_, cx| {
            cx.new(|_| {
                muxy_ui::components::Tooltip::new(
                    tooltip.clone(),
                    background,
                    foreground,
                    border,
                    theme_background,
                )
            })
            .into()
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, event: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                cx.emit(BrowserViewEvent::Menu(menu, event.position));
            }),
        )
        .child(content)
        .into_any_element()
}

/// History matching the address field, shown under it. The page's native
/// view is told to stay out of the way.
fn suggestions(view: &BrowserView, cx: &mut Context<BrowserView>) -> Option<AnyElement> {
    if !view.suggestions.open || view.suggestions.entries.is_empty() {
        return None;
    }
    let theme = &view.theme;
    let metrics = view.metrics;
    let native = view.native.clone();
    let rows = view
        .suggestions
        .entries
        .iter()
        .enumerate()
        .map(|(index, suggestion)| {
            let selected = view.suggestions.selected == Some(index);
            let url = suggestion.url.clone();
            div()
                .id(("browser-suggestion", index))
                .flex()
                .flex_col()
                .px(metrics.spacing4())
                .py(metrics.spacing2())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .when(selected, |row| row.bg(theme.accent_soft))
                .hover(|style| style.bg(theme.hover))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |view, _, window, cx| {
                        cx.stop_propagation();
                        view.submit(&url, window, cx);
                    }),
                )
                .children(suggestion.title.clone().map(|title| {
                    div()
                        .w_full()
                        .truncate()
                        .text_size(metrics.font_body())
                        .text_color(theme.fg)
                        .child(title)
                }))
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .text_size(metrics.font_caption())
                        .text_color(theme.fg_muted)
                        .child(suggestion.url.clone()),
                )
        });
    Some(
        deferred(
            div()
                .id("browser-suggestions")
                .debug_selector(|| "browser-suggestions".into())
                .absolute()
                .top(metrics.control_medium() + metrics.spacing1())
                .left_0()
                .w(view.address_width.get())
                .flex()
                .flex_col()
                .p(metrics.spacing2())
                .rounded(metrics.radius_md())
                .border_1()
                .border_color(theme.border)
                .bg(theme.raised())
                .shadow_lg()
                .children(rows)
                .child(
                    canvas(
                        move |bounds, _, _| native.occlude_also(bounds),
                        |_, (), _, _| (),
                    )
                    .absolute()
                    .size_full(),
                ),
        )
        .with_priority(1)
        .into_any_element(),
    )
}

pub(super) fn find_bar(view: &BrowserView, cx: &mut Context<BrowserView>) -> Option<AnyElement> {
    let find = view.find.as_ref()?;
    let theme = &view.theme;
    let metrics = view.metrics;
    let button = |id: &'static str, icon, tooltip: SharedString| {
        IconButton::new(
            id,
            icon,
            metrics.icon_sm(),
            metrics.control_medium(),
            theme.fg_muted,
            theme.fg,
        )
        .tooltip(tooltip, theme.surface, theme.fg, theme.border, theme.bg)
    };
    let missing = find.found == Some(false);
    Some(
        div()
            .id("browser-find")
            .debug_selector(|| "browser-find".into())
            .flex()
            .flex_none()
            .items_center()
            .gap(metrics.spacing2())
            .p(metrics.spacing2())
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(IconGlyph::new(
                Icon::Search,
                metrics.icon_sm(),
                theme.fg_dim,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .px(metrics.spacing2())
                    .when(missing, |input| input.text_color(theme.warning))
                    .child(find.input.clone()),
            )
            .when(missing, |bar| {
                bar.child(
                    div()
                        .text_size(metrics.font_caption())
                        .text_color(theme.warning)
                        .child(tr!("Not found")),
                )
            })
            .child(
                button(
                    "browser-find-previous",
                    Icon::ChevronUp,
                    tr!("Previous match"),
                )
                .on_click(cx.listener(|view, _, _, cx| view.step_find(true, cx))),
            )
            .child(
                button("browser-find-next", Icon::ChevronDown, tr!("Next match"))
                    .on_click(cx.listener(|view, _, _, cx| view.step_find(false, cx))),
            )
            .child(
                button("browser-find-close", Icon::X, tr!("Close search"))
                    .on_click(cx.listener(|view, _, window, cx| view.close_find(window, cx))),
            )
            .into_any_element(),
    )
}
