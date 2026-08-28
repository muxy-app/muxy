use crate::state::AppState;
use crate::views::menu::Item;
use crate::views::window::MainWindow;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, canvas, div, px,
};
use muxy_core::prefs::home_dir;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use std::cell::Cell;
use std::rc::Rc;

const PATH_MAX_CHARACTERS: usize = 40;

pub fn status_bar(
    state: &AppState,
    working_directory: Option<&str>,
    repository_controls: &[crate::repository::RepositoryControl],
    cx: &mut Context<MainWindow>,
) -> AnyElement {
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
    for control in status_controls(path.is_some(), repository_controls) {
        match control {
            StatusControl::Path => {
                let path = path.expect("path control requires a path");
                let remote = state
                    .active_project()
                    .is_some_and(|project| project.is_remote());
                left = left.child(path_chip(state, path, remote, cx));
            }
            StatusControl::Separator => {
                left = left.child(status_separator(state));
            }
            StatusControl::Branch => {
                if let Some(control) = repository_controls.iter().find(|control| {
                    control.kind == crate::repository::RepositoryControlKind::Branch
                }) {
                    left = left.child(branch_chip(state, control, cx));
                }
            }
        }
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
        .into_any_element()
}

#[derive(Clone, Copy)]
enum StatusControl {
    Path,
    Separator,
    Branch,
}

fn status_controls(
    has_path: bool,
    repository_controls: &[crate::repository::RepositoryControl],
) -> Vec<StatusControl> {
    let mut controls = Vec::new();
    if has_path {
        controls.push(StatusControl::Path);
    }
    if repository_controls
        .iter()
        .any(|control| control.kind == crate::repository::RepositoryControlKind::Branch)
    {
        if !controls.is_empty() {
            controls.push(StatusControl::Separator);
        }
        controls.push(StatusControl::Branch);
    }
    controls
}

fn status_separator(state: &AppState) -> AnyElement {
    div()
        .w(px(1.0))
        .h_full()
        .flex_none()
        .bg(state.theme.border)
        .into_any_element()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusPathAction {
    Copy,
    Reveal,
}

fn primary_path_action(remote: bool) -> StatusPathAction {
    if remote {
        StatusPathAction::Copy
    } else {
        StatusPathAction::Reveal
    }
}

fn context_path_actions(remote: bool) -> Vec<StatusPathAction> {
    let mut actions = vec![StatusPathAction::Copy];
    if !remote {
        actions.push(StatusPathAction::Reveal);
    }
    actions
}

fn path_command(action: StatusPathAction, path: String) -> crate::command::Command {
    match action {
        StatusPathAction::Copy => crate::command::Command::CopyStatusPath(path),
        StatusPathAction::Reveal => crate::command::Command::RevealStatusPath(path),
    }
}

pub(crate) fn path_menu_items(path: String, remote: bool) -> Vec<Item> {
    context_path_actions(remote)
        .into_iter()
        .map(|action| {
            let label = match action {
                StatusPathAction::Copy => "Copy Path",
                StatusPathAction::Reveal => "Reveal",
            };
            Item::action(label, path_command(action, path.clone()))
        })
        .collect()
}

pub(crate) fn reveal_failure(path: &str, error: impl std::fmt::Display) -> (String, String) {
    (
        "Could Not Reveal Path".to_owned(),
        format!("Muxy could not reveal {path}: {error}"),
    )
}

fn path_chip(
    state: &AppState,
    path: &str,
    remote: bool,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let primary_path = path.to_owned();
    let menu_path = path.to_owned();
    let primary = primary_path_action(remote);

    div()
        .id("status-path")
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .h_full()
        .text_color(theme.fg_muted)
        .cursor_pointer()
        .on_click(cx.listener(move |window: &mut MainWindow, _, view, cx| {
            window.perform(path_command(primary, primary_path.clone()), view, cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(
                move |window: &mut MainWindow, event: &MouseDownEvent, view, cx| {
                    window.open_status_path_menu(
                        menu_path.clone(),
                        remote,
                        event.position,
                        view,
                        cx,
                    );
                },
            ),
        )
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

fn branch_chip(
    state: &AppState,
    control: &crate::repository::RepositoryControl,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let enabled = control.enabled;
    let bounds = Rc::new(Cell::new(None));
    let recorder = bounds.clone();
    let click_bounds = bounds.clone();
    div()
        .id("status-branch")
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .h_full()
        .text_color(if enabled {
            theme.fg_muted
        } else {
            theme.fg_dim
        })
        .when(enabled, |element| {
            element
                .cursor_pointer()
                .hover(|style| style.text_color(theme.fg))
        })
        .when(enabled, |element| {
            element.on_mouse_down(
                MouseButton::Left,
                cx.listener(
                    move |window: &mut MainWindow, _: &MouseDownEvent, view, cx| {
                        if let Some(bounds) = click_bounds.get() {
                            window.open_branch_popover(bounds, view, cx);
                        }
                    },
                ),
            )
        })
        .child(
            canvas(
                move |bounds, _, _| recorder.set(Some(bounds)),
                |_, _: (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(muxy_ui::components::SymbolGlyph::new(
            "arrow.triangle.branch",
            metrics.font_caption(),
            if enabled {
                theme.fg_muted
            } else {
                theme.fg_dim
            },
        ))
        .child(
            div()
                .text_size(metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .child(SharedString::from(control.label.clone())),
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

#[cfg(test)]
fn status_control_ids(has_path: bool) -> Vec<&'static str> {
    status_controls(has_path, &[])
        .iter()
        .map(|control| match control {
            StatusControl::Path => "status-path",
            StatusControl::Separator => "status-separator",
            StatusControl::Branch => "status-branch",
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_path_local_and_remote_actions_are_truthful() {
        assert_eq!(primary_path_action(false), StatusPathAction::Reveal);
        assert_eq!(primary_path_action(true), StatusPathAction::Copy);
        assert_eq!(
            context_path_actions(false),
            vec![StatusPathAction::Copy, StatusPathAction::Reveal]
        );
        assert_eq!(context_path_actions(true), vec![StatusPathAction::Copy]);
    }

    #[test]
    fn status_path_reveal_errors_map_to_the_existing_alert_surface() {
        assert_eq!(
            reveal_failure("/missing/project", "launch failed"),
            (
                "Could Not Reveal Path".to_owned(),
                "Muxy could not reveal /missing/project: launch failed".to_owned(),
            )
        );
    }

    #[test]
    fn chrome_status_contract_excludes_later_owned_controls() {
        assert_eq!(status_control_ids(true), vec!["status-path"]);
        assert!(status_control_ids(false).is_empty());
        assert_eq!(status_control_ids(true), vec!["status-path"]);
    }

    #[test]
    fn phase_six_separates_the_path_and_branch_without_a_leading_separator() {
        let repository = vec![crate::repository::RepositoryControl {
            kind: crate::repository::RepositoryControlKind::Branch,
            label: "main".to_owned(),
            tooltip: "No upstream".to_owned(),
            enabled: true,
        }];
        assert_eq!(
            status_controls(true, &repository)
                .iter()
                .map(|control| match control {
                    StatusControl::Path => "status-path",
                    StatusControl::Separator => "status-separator",
                    StatusControl::Branch => "status-branch",
                })
                .collect::<Vec<_>>(),
            ["status-path", "status-separator", "status-branch"]
        );
        assert_eq!(
            status_controls(false, &repository)
                .iter()
                .map(|control| match control {
                    StatusControl::Path => "status-path",
                    StatusControl::Separator => "status-separator",
                    StatusControl::Branch => "status-branch",
                })
                .collect::<Vec<_>>(),
            ["status-branch"]
        );
    }
}
