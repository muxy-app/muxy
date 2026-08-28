use crate::state::AppState;
use crate::terminal::{ConfirmationId, ConfirmationKind};
use crate::views::menu::{self, Menu};
use crate::views::window::MainWindow;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use muxy_ui::text_input::TextInput;

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
        picker: Entity<muxy_ui::command_popover::CommandPopover>,
        anchor: Point<Pixels>,
    },
    Colors {
        picker: Entity<muxy_ui::command_popover::CommandPopover>,
        anchor: Point<Pixels>,
    },
    TabColors {
        picker: Entity<muxy_ui::command_popover::CommandPopover>,
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
    ThemePicker {
        browser: Entity<crate::views::settings::theme_picker::ThemeBrowser>,
        anchor: Option<gpui::Bounds<Pixels>>,
    },
    CreateWorktree(Entity<crate::views::create_worktree_overlay::CreateWorktreeModal>),
    Repository {
        kind: RepositoryPopoverKind,
        anchor: gpui::Bounds<Pixels>,
    },
}

pub enum RepositoryPopoverKind {
    Branch(Box<crate::views::repository::branch::BranchPopover>),
    Changes(Box<crate::views::repository::changes::ChangesPopover>),
    PullRequest(Box<crate::views::repository::pull_request::PullRequestPopover>),
    Ai(Box<crate::views::repository::ai::RepositoryAiPopover>),
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
    repository_state: &crate::repository::RepositoryState,
    repository_mutation_busy: bool,
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
        Overlay::Symbols { picker, anchor }
        | Overlay::Colors { picker, anchor }
        | Overlay::TabColors { picker, anchor } => anchored_picker(
            picker,
            *anchor,
            gpui::size(state.metrics.scaled(400.0), state.metrics.scaled(480.0)),
            viewport,
            state,
        ),
        Overlay::TerminalConfirm { kind, .. } => confirmation_dialog(*kind, state, focus, cx),
        Overlay::Picker(picker) => picker.clone().into_any_element(),
        Overlay::Omnibox(omnibox) => omnibox.clone().into_any_element(),
        Overlay::Settings(modal) => modal.clone().into_any_element(),
        Overlay::ThemePicker { browser, anchor } => {
            let picker = browser.read(cx).picker().clone();
            let offsets = theme_picker_offsets(
                *anchor,
                state
                    .metrics
                    .scaled(crate::views::settings::theme_picker::PICKER_WIDTH),
                viewport,
                state.metrics.spacing4(),
                state.metrics.spacing2(),
            );
            bottom_anchored_picker(&picker, offsets.x, offsets.y)
        }
        Overlay::CreateWorktree(modal) => div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(modal.clone())
            .into_any_element(),
        Overlay::Repository {
            kind: RepositoryPopoverKind::Branch(popover),
            anchor,
        } => {
            let policy = crate::views::repository::branch::branch_overlay_policy();
            let size = gpui::size(
                state.metrics.scaled(policy.target_width),
                state.metrics.scaled(policy.target_height),
            );
            let origin = clamp(
                gpui::point(
                    anchor.origin.x,
                    anchor.origin.y - size.height - state.metrics.spacing2(),
                ),
                size,
                viewport,
                state,
            );
            let bounds = gpui::Bounds { origin, size };
            let current_branch = match &repository_state.summary {
                crate::repository::LoadState::Ready(summary) if !summary.is_detached => {
                    Some(summary.branch.as_str())
                }
                _ => None,
            };
            crate::views::repository::branch::render(
                popover,
                &repository_state.branches,
                current_branch,
                repository_mutation_busy,
                bounds,
                state,
                cx,
            )
        }
        Overlay::Repository {
            kind: RepositoryPopoverKind::Changes(popover),
            anchor,
        } => {
            let policy = crate::views::repository::changes::changes_overlay_policy();
            let size = gpui::size(
                state.metrics.scaled(policy.target_width),
                state.metrics.scaled(policy.target_height),
            );
            let origin = clamp(
                gpui::point(
                    anchor.origin.x,
                    anchor.origin.y - size.height - state.metrics.spacing2(),
                ),
                size,
                viewport,
                state,
            );
            crate::views::repository::changes::render(popover, gpui::Bounds { origin, size })
        }
        Overlay::Repository {
            kind: RepositoryPopoverKind::PullRequest(popover),
            anchor,
        } => {
            let policy = popover.panel.read(cx).overlay_policy();
            let size = gpui::size(
                state.metrics.scaled(policy.target_width),
                state.metrics.scaled(policy.target_height),
            );
            let origin = clamp(
                gpui::point(
                    anchor.origin.x,
                    anchor.origin.y - size.height - state.metrics.spacing2(),
                ),
                size,
                viewport,
                state,
            );
            crate::views::repository::pull_request::render(popover, gpui::Bounds { origin, size })
        }
        Overlay::Repository {
            kind: RepositoryPopoverKind::Ai(popover),
            anchor,
        } => {
            if repository_state.key.as_ref() != Some(&popover.key(cx)) {
                return div().into_any_element();
            }
            let logical_size = popover.size(cx);
            let size = gpui::size(
                state.metrics.scaled(logical_size.0),
                state.metrics.scaled(logical_size.1),
            );
            let origin = clamp(
                gpui::point(
                    anchor.origin.x,
                    anchor.origin.y - size.height - state.metrics.spacing2(),
                ),
                size,
                viewport,
                state,
            );
            popover.render(origin)
        }
    };

    let backdrop = matches!(
        overlay,
        Overlay::Settings(_) | Overlay::CreateWorktree(_) | Overlay::TerminalConfirm { .. }
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

fn clamp(
    anchor: Point<Pixels>,
    size: gpui::Size<Pixels>,
    viewport: gpui::Size<Pixels>,
    state: &AppState,
) -> Point<Pixels> {
    let margin = state.metrics.spacing4();
    let horizontal = crate::views::repository::branch::clamp_axis(
        f32::from(anchor.x),
        f32::from(size.width),
        f32::from(viewport.width),
        f32::from(margin),
    );
    let vertical = crate::views::repository::branch::clamp_axis(
        f32::from(anchor.y),
        f32::from(size.height),
        f32::from(viewport.height),
        f32::from(margin),
    );
    gpui::point(px(horizontal), px(vertical))
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

fn anchored_picker(
    picker: &Entity<muxy_ui::command_popover::CommandPopover>,
    anchor: Point<Pixels>,
    size: gpui::Size<Pixels>,
    viewport: gpui::Size<Pixels>,
    state: &AppState,
) -> AnyElement {
    let origin = clamp(anchor, size, viewport, state);
    div()
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .child(picker.clone())
        .into_any_element()
}

fn theme_picker_offsets(
    anchor: Option<gpui::Bounds<Pixels>>,
    width: Pixels,
    viewport: gpui::Size<Pixels>,
    margin: Pixels,
    gap: Pixels,
) -> Point<Pixels> {
    let Some(anchor) = anchor else {
        return gpui::point(margin, margin);
    };
    let left = crate::views::repository::branch::clamp_axis(
        f32::from(anchor.origin.x),
        f32::from(width),
        f32::from(viewport.width),
        f32::from(margin),
    );
    let bottom = (viewport.height - anchor.origin.y + gap)
        .max(margin)
        .min(viewport.height - margin);
    gpui::point(px(left), bottom)
}

fn bottom_anchored_picker(
    picker: &Entity<muxy_ui::command_popover::CommandPopover>,
    left: Pixels,
    bottom: Pixels,
) -> AnyElement {
    div()
        .absolute()
        .left(left)
        .bottom(bottom)
        .child(picker.clone())
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_picker_keeps_full_and_filtered_results_attached_to_the_sidebar_anchor() {
        let viewport = gpui::size(px(1200.0), px(800.0));
        let anchor = gpui::Bounds {
            origin: gpui::point(px(196.0), px(744.0)),
            size: gpui::size(px(24.0), px(24.0)),
        };

        let offsets = theme_picker_offsets(Some(anchor), px(340.0), viewport, px(8.0), px(4.0));
        let attached_bottom = viewport.height - offsets.y;
        let full_top = attached_bottom - px(360.0);
        let filtered_top = attached_bottom - px(74.0);

        assert_eq!(offsets, gpui::point(px(196.0), px(60.0)));
        assert_eq!(attached_bottom, px(740.0));
        assert_eq!(full_top, px(380.0));
        assert_eq!(filtered_top, px(666.0));
        assert_eq!(
            theme_picker_offsets(None, px(340.0), viewport, px(8.0), px(4.0)),
            gpui::point(px(8.0), px(8.0))
        );
    }
}
