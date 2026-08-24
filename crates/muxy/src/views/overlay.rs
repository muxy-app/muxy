use crate::state::AppState;
use crate::terminal::{ConfirmationId, ConfirmationKind};
use crate::views::menu::{self, Menu};
use crate::views::window::MainWindow;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use muxy_core::store::ICON_PALETTE;
use muxy_ui::components::SymbolGlyph;
use muxy_ui::symbols;
use muxy_ui::text_input::TextInput;

const SYMBOL_COLUMNS: usize = 7;

pub enum Overlay {
    None,
    Menu(Menu),
    Rename {
        input: Entity<TextInput>,
        anchor: Point<Pixels>,
    },
    GroupRename {
        group_id: Option<String>,
        input: Entity<TextInput>,
        anchor: Point<Pixels>,
    },
    TabRename {
        input: Entity<TextInput>,
        bounds: gpui::Bounds<Pixels>,
    },
    Symbols {
        project_id: String,
        search: Entity<TextInput>,
        anchor: Point<Pixels>,
    },
    Colors {
        project_id: String,
        anchor: Point<Pixels>,
    },
    TabColors {
        tab_id: String,
        anchor: Point<Pixels>,
    },
    TerminalConfirm {
        tab_id: String,
        id: ConfirmationId,
        kind: ConfirmationKind,
    },
    Picker(Entity<crate::views::project_picker::ProjectPicker>),
    Omnibox(Entity<crate::views::omnibox::Omnibox>),
    Settings(Entity<crate::views::settings::SettingsModal>),
    ThemePicker(Entity<crate::views::settings::theme_picker::ThemeBrowser>),
}

impl Overlay {
    pub fn is_open(&self) -> bool {
        !matches!(self, Self::None)
    }
}

fn confirmation_copy(kind: ConfirmationKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        ConfirmationKind::Paste => (
            "Paste multiple lines?",
            "The clipboard contains text that may execute more than one command.",
            "Paste",
        ),
        ConfirmationKind::Osc52Read => (
            "Allow clipboard read?",
            "A terminal program requested access to the system clipboard.",
            "Allow",
        ),
        ConfirmationKind::Osc52Write => (
            "Allow clipboard write?",
            "A terminal program requested permission to replace the system clipboard.",
            "Allow",
        ),
        ConfirmationKind::ActiveProcessClose => (
            "Close active terminal?",
            "A process is still running in this terminal.",
            "Close",
        ),
    }
}

fn confirmation_dialog(
    kind: ConfirmationKind,
    state: &AppState,
    focus: &gpui::FocusHandle,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let (title_text, body, approve) = confirmation_copy(kind);

    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .child(
            div()
                .key_context(menu::KEY_CONTEXT)
                .track_focus(focus)
                .on_action(cx.listener(
                    move |window: &mut MainWindow, _: &crate::views::menu::DismissMenu, _, cx| {
                        window.resolve_terminal_confirmation(false, cx);
                    },
                ))
                .w(metrics.scaled(420.0))
                .flex()
                .flex_col()
                .p(metrics.spacing6())
                .gap(metrics.spacing4())
                .rounded(metrics.radius_lg())
                .bg(theme.raised())
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(title(title_text, state))
                .child(
                    div()
                        .text_size(metrics.font_footnote())
                        .text_color(theme.fg_muted)
                        .child(SharedString::from(body)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap(metrics.spacing3())
                        .child(
                            dialog_button("Cancel", "confirm-cancel", false, state).on_click(
                                cx.listener(|window: &mut MainWindow, _, _, cx| {
                                    window.resolve_terminal_confirmation(false, cx);
                                }),
                            ),
                        )
                        .child(
                            dialog_button(approve, "confirm-approve", true, state).on_click(
                                cx.listener(|window: &mut MainWindow, _, _, cx| {
                                    window.resolve_terminal_confirmation(true, cx);
                                }),
                            ),
                        ),
                ),
        )
        .into_any_element()
}

fn dialog_button(
    label: &'static str,
    id: &'static str,
    primary: bool,
    state: &AppState,
) -> gpui::Stateful<gpui::Div> {
    let metrics = &state.metrics;
    let theme = &state.theme;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(metrics.control_medium())
        .px(metrics.spacing5())
        .rounded(metrics.radius_sm())
        .cursor_pointer()
        .text_size(metrics.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .when(primary, |element| {
            element.bg(theme.accent).text_color(theme.bg)
        })
        .when(!primary, |element| {
            element
                .bg(theme.surface)
                .text_color(theme.fg)
                .border_1()
                .border_color(theme.border)
        })
        .child(SharedString::from(label))
}

pub fn layer(
    overlay: &Overlay,
    state: &AppState,
    focus: &gpui::FocusHandle,
    window: &mut gpui::Window,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let viewport = window.viewport_size();
    let content = match overlay {
        Overlay::None => return div().into_any_element(),
        Overlay::Menu(open) => {
            let mut open = open.clone();
            open.position = clamp(open.position, menu_size(&open, state), viewport, state);
            menu::render(&open, state, focus, cx)
        }
        Overlay::Rename { input, anchor } => {
            let anchor = clamp(
                *anchor,
                gpui::size(state.metrics.scaled(200.0), state.metrics.scaled(104.0)),
                viewport,
                state,
            );
            rename_popover("Rename Project", input, anchor, state, cx)
        }
        Overlay::GroupRename {
            group_id,
            input,
            anchor,
        } => {
            let anchor = clamp(
                *anchor,
                gpui::size(state.metrics.scaled(200.0), state.metrics.scaled(104.0)),
                viewport,
                state,
            );
            let heading = if group_id.is_some() {
                "Rename Workspace"
            } else {
                "New Workspace"
            };
            rename_popover(heading, input, anchor, state, cx)
        }
        Overlay::TabRename { input, bounds } => tab_rename_inline(input, *bounds, state),
        Overlay::Symbols {
            project_id,
            search,
            anchor,
        } => {
            let anchor = clamp(
                *anchor,
                gpui::size(state.metrics.scaled(300.0), state.metrics.scaled(400.0)),
                viewport,
                state,
            );
            symbol_popover(project_id, search, anchor, state, cx)
        }
        Overlay::Colors { project_id, anchor } => {
            let anchor = clamp(
                *anchor,
                gpui::size(state.metrics.scaled(216.0), state.metrics.scaled(240.0)),
                viewport,
                state,
            );
            color_popover(project_id, anchor, state, focus, cx)
        }
        Overlay::TabColors { tab_id, anchor } => {
            let anchor = clamp(
                *anchor,
                gpui::size(state.metrics.scaled(216.0), state.metrics.scaled(96.0)),
                viewport,
                state,
            );
            tab_color_popover(tab_id, anchor, state, cx)
        }
        Overlay::TerminalConfirm { kind, .. } => confirmation_dialog(*kind, state, focus, cx),
        Overlay::Picker(picker) => picker.clone().into_any_element(),
        Overlay::Omnibox(omnibox) => omnibox.clone().into_any_element(),
        Overlay::Settings(modal) => modal.clone().into_any_element(),
        Overlay::ThemePicker(browser) => div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(browser.clone())
            .into_any_element(),
    };

    let backdrop = matches!(
        overlay,
        Overlay::Picker(_)
            | Overlay::Omnibox(_)
            | Overlay::Settings(_)
            | Overlay::ThemePicker(_)
            | Overlay::TerminalConfirm { .. }
    );

    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .when(backdrop, |element| {
                    element.bg(gpui::hsla(0.0, 0.0, 0.0, 0.3))
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|window: &mut MainWindow, _, _, cx| {
                        window.dismiss_overlay(cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|window: &mut MainWindow, _, _, cx| {
                        window.dismiss_overlay(cx);
                    }),
                ),
        )
        .child(content)
        .into_any_element()
}

fn with_opacity(color: gpui::Hsla, opacity: f32) -> gpui::Hsla {
    gpui::Hsla {
        a: color.a * opacity,
        ..color
    }
}

fn clamp(
    anchor: Point<Pixels>,
    size: gpui::Size<Pixels>,
    viewport: gpui::Size<Pixels>,
    state: &AppState,
) -> Point<Pixels> {
    let margin = state.metrics.spacing4();
    let horizontal = (viewport.width - size.width - margin).max(margin);
    let vertical = (viewport.height - size.height - margin).max(margin);
    gpui::point(anchor.x.min(horizontal), anchor.y.min(vertical))
}

fn menu_size(menu: &Menu, state: &AppState) -> gpui::Size<Pixels> {
    let metrics = &state.metrics;
    let height = menu
        .items
        .iter()
        .fold(metrics.spacing2() * 2.0, |total, item| {
            total
                + match item {
                    crate::views::menu::Item::Separator => px(1.0) + metrics.spacing2() * 2.0,
                    _ => metrics.scaled(22.0),
                }
        });
    gpui::size(metrics.scaled(180.0), height)
}

fn panel(anchor: Point<Pixels>, state: &AppState) -> gpui::Div {
    let metrics = &state.metrics;
    let theme = &state.theme;
    div()
        .absolute()
        .left(anchor.x)
        .top(anchor.y)
        .occlude()
        .flex()
        .flex_col()
        .p(metrics.spacing6())
        .gap(metrics.spacing5())
        .rounded(metrics.radius_lg())
        .bg(theme.raised())
        .border_1()
        .border_color(theme.border)
        .shadow_lg()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

fn title(text: &str, state: &AppState) -> AnyElement {
    div()
        .text_size(state.metrics.font_body())
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(state.theme.fg)
        .child(SharedString::from(text.to_owned()))
        .into_any_element()
}

fn field_frame(state: &AppState) -> gpui::Div {
    let metrics = &state.metrics;
    div()
        .flex()
        .flex_row()
        .items_center()
        .h(metrics.control_medium())
        .px(metrics.spacing3())
        .rounded(metrics.radius_sm())
        .bg(state.theme.bg)
        .border_1()
        .border_color(state.theme.border)
}

fn rename_popover(
    heading: &str,
    input: &Entity<TextInput>,
    anchor: Point<Pixels>,
    state: &AppState,
    _cx: &mut Context<MainWindow>,
) -> AnyElement {
    panel(anchor, state)
        .w(state.metrics.scaled(200.0))
        .gap(state.metrics.spacing4())
        .child(title(heading, state))
        .child(field_frame(state).child(input.clone()))
        .into_any_element()
}

fn tab_rename_inline(
    input: &Entity<TextInput>,
    bounds: gpui::Bounds<Pixels>,
    state: &AppState,
) -> AnyElement {
    div()
        .absolute()
        .left(bounds.origin.x)
        .top(bounds.origin.y)
        .w(bounds.size.width)
        .h(bounds.size.height)
        .px(state.metrics.spacing6())
        .flex()
        .items_center()
        .bg(state.theme.surface)
        .border_b(px(2.0))
        .border_color(state.theme.accent)
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(input.clone())
        .into_any_element()
}

fn symbol_popover(
    project_id: &str,
    search: &Entity<TextInput>,
    anchor: Point<Pixels>,
    state: &AppState,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let query = search.read(cx).text().to_owned();
    let selected = state
        .workspace
        .project(project_id)
        .and_then(|project| project.icon.clone());
    let matches = symbols::matching(&query);

    let mut grid = div().flex().flex_col().gap(metrics.spacing4());
    for row in matches.chunks(SYMBOL_COLUMNS) {
        let mut line = div().flex().flex_row().gap(metrics.spacing4());
        for symbol in row {
            let name = symbol.symbol;
            let is_selected = selected.as_deref() == Some(name);
            let project_id = project_id.to_owned();
            line = line.child(
                div()
                    .id(SharedString::from(format!("symbol-{name}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(metrics.control_large())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .when(is_selected, |element| {
                        element.bg(with_opacity(theme.accent, 0.2))
                    })
                    .hover(|style| style.bg(theme.hover))
                    .child(SymbolGlyph::new(
                        name,
                        metrics.font_title_large(),
                        if is_selected { theme.accent } else { theme.fg },
                    ))
                    .on_click(cx.listener(move |window: &mut MainWindow, _, _, cx| {
                        window.set_icon(&project_id, Some(name.to_owned()), cx);
                    })),
            );
        }
        grid = grid.child(line);
    }

    let project = project_id.to_owned();
    panel(anchor, state)
        .w(metrics.scaled(300.0))
        .child(title("Icon", state))
        .child(field_frame(state).child(search.clone()))
        .child(
            div()
                .id("symbol-grid")
                .h(metrics.scaled(260.0))
                .overflow_y_scroll()
                .child(grid),
        )
        .child(div().h(px(1.0)).bg(theme.border))
        .child(
            div()
                .id("remove-icon")
                .flex()
                .flex_row()
                .items_center()
                .gap(metrics.spacing3())
                .cursor_pointer()
                .text_size(metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.fg_muted)
                .when(selected.is_none(), |element| element.opacity(0.4))
                .child(SymbolGlyph::new(
                    "xmark.circle",
                    metrics.font_caption(),
                    theme.fg_muted,
                ))
                .child(SharedString::from("Remove Icon"))
                .on_click(cx.listener(move |window: &mut MainWindow, _, _, cx| {
                    window.set_icon(&project, None, cx);
                })),
        )
        .into_any_element()
}

fn color_popover(
    project_id: &str,
    anchor: Point<Pixels>,
    state: &AppState,
    focus: &gpui::FocusHandle,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let selected = state
        .workspace
        .project(project_id)
        .and_then(|project| project.icon_color.clone());

    let mut grid = div().flex().flex_col().gap(metrics.spacing4());
    for row in ICON_PALETTE.chunks(6) {
        let mut line = div().flex().flex_row().gap(metrics.spacing4());
        for swatch in row {
            let id = swatch.id;
            let is_selected = selected.as_deref() == Some(id);
            let color: gpui::Hsla = crate::views::swatches::icon_color(Some(id))
                .map(Into::into)
                .unwrap_or(theme.fg_muted);
            let project_id = project_id.to_owned();
            let foreground: gpui::Hsla = crate::views::swatches::icon_foreground(Some(id))
                .map(Into::into)
                .unwrap_or(theme.fg);
            let mut swatch_circle = div()
                .flex()
                .items_center()
                .justify_center()
                .size(metrics.scaled(22.0))
                .rounded_full()
                .bg(color);
            if is_selected {
                swatch_circle = swatch_circle.child(
                    div()
                        .size(metrics.scaled(18.0))
                        .rounded_full()
                        .border_2()
                        .border_color(foreground),
                );
            }
            line = line.child(
                div()
                    .id(SharedString::from(format!("swatch-{id}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(metrics.control_medium())
                    .cursor_pointer()
                    .child(swatch_circle)
                    .on_click(cx.listener(move |window: &mut MainWindow, _, _, cx| {
                        window.set_icon_color(&project_id, Some(id.to_owned()), cx);
                    })),
            );
        }
        grid = grid.child(line);
    }

    let project = project_id.to_owned();
    panel(anchor, state)
        .key_context(menu::KEY_CONTEXT)
        .track_focus(focus)
        .on_action(cx.listener(
            |window: &mut MainWindow, _: &crate::views::menu::DismissMenu, _, cx| {
                window.dismiss_overlay(cx);
            },
        ))
        .w(metrics.scaled(216.0))
        .child(title("Icon Color", state))
        .child(grid)
        .child(div().h(px(1.0)).bg(theme.border))
        .child(
            div()
                .id("reset-icon-color")
                .flex()
                .flex_row()
                .items_center()
                .gap(metrics.spacing3())
                .cursor_pointer()
                .text_size(metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.fg_muted)
                .when(selected.is_none(), |element| element.opacity(0.4))
                .child(SymbolGlyph::new(
                    "arrow.uturn.backward",
                    metrics.font_caption(),
                    theme.fg_muted,
                ))
                .child(SharedString::from("Reset to Default"))
                .on_click(cx.listener(move |window: &mut MainWindow, _, _, cx| {
                    window.set_icon_color(&project, None, cx);
                })),
        )
        .into_any_element()
}

fn tab_color_popover(
    tab_id: &str,
    anchor: Point<Pixels>,
    state: &AppState,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let selected = state
        .active_tab_workspace()
        .and_then(|workspace| workspace.tab(tab_id))
        .and_then(|tab| tab.color_id.as_deref());
    let mut grid = div().flex().flex_col().gap(metrics.spacing4());
    for row in ICON_PALETTE.chunks(6) {
        let mut line = div().flex().flex_row().gap(metrics.spacing4());
        for swatch in row {
            let id = swatch.id;
            let tab_id = tab_id.to_owned();
            let color: gpui::Hsla = crate::views::swatches::icon_color(Some(id))
                .map(Into::into)
                .unwrap_or(state.theme.fg_muted);
            let foreground: gpui::Hsla = crate::views::swatches::icon_foreground(Some(id))
                .map(Into::into)
                .unwrap_or(state.theme.fg);
            let mut circle = div()
                .flex()
                .items_center()
                .justify_center()
                .size(metrics.scaled(22.0))
                .rounded_full()
                .bg(color);
            if selected == Some(id) {
                circle = circle.child(
                    div()
                        .size(metrics.scaled(18.0))
                        .rounded_full()
                        .border_2()
                        .border_color(foreground),
                );
            }
            line = line.child(
                div()
                    .id(SharedString::from(format!("tab-swatch-{id}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(metrics.control_medium())
                    .cursor_pointer()
                    .child(circle)
                    .on_click(cx.listener(move |window: &mut MainWindow, _, _, cx| {
                        window.set_tab_color(&tab_id, Some(id.to_owned()), cx);
                    })),
            );
        }
        grid = grid.child(line);
    }
    panel(anchor, state)
        .w(metrics.scaled(216.0))
        .child(title("Tab Color", state))
        .child(grid)
        .into_any_element()
}
