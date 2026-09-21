use muxy_core::shortcuts::ShortcutId;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement, Styled, Window, actions, div, px,
};
use muxy_ui::components::SymbolGlyph;

use super::overlays::Overlay;
use crate::model::AppModel;

mod shortcuts;

actions!(
    menu,
    [
        DismissMenu,
        HighlightPrevious,
        HighlightNext,
        ConfirmHighlighted
    ]
);

pub(crate) fn register_shortcuts(registry: &mut muxy_ui::shortcuts::Registry<'_>) {
    registry.register(ShortcutId::MenuDismissMenu, &DismissMenu);
    registry.register(ShortcutId::MenuHighlightPrevious, &HighlightPrevious);
    registry.register(ShortcutId::MenuHighlightNext, &HighlightNext);
    registry.register(ShortcutId::MenuConfirmHighlighted, &ConfirmHighlighted);
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Command {
    Tab(muxy_app_core::TabId, super::tab_menu::Action),
    Layout(muxy_app_core::settings::AppLayout),
    FocusProject(bool),
    SortProjects(muxy_app_core::settings::ProjectOrder),
    Worktrees(muxy_app_core::ProjectId),
    RemoveWorktree(muxy_app_core::ProjectId),
    Dismiss,
    ExistingSessions(muxy_app_core::ProjectId),
    DetachTerminal(muxy_app_core::PaneId),
    SplitPane(muxy_app_core::PaneId, muxy_app_core::Direction),
    ToggleZoomPane(muxy_app_core::PaneId),
    ClosePane(muxy_app_core::PaneId),
    TerminalCopy(muxy_app_core::PaneId),
    TerminalPaste(muxy_app_core::PaneId),
    TerminalSelectAll(muxy_app_core::PaneId),
    TerminalSelectCommandOutput(muxy_app_core::PaneId),
    CopyPath(muxy_app_core::ProjectId),
    RevealPath(muxy_app_core::ProjectId),
    EditProject(muxy_app_core::ProjectId, super::project_editor::Field),
    ProjectColor(muxy_app_core::ProjectId),
    RemoveProject(muxy_app_core::ProjectId),
}

#[derive(Clone, Debug)]
pub(crate) struct Item {
    label: &'static str,
    command: Command,
    disabled: bool,
    checked: bool,
    separator_before: bool,
}

impl Item {
    pub(crate) fn action(label: &'static str, command: Command) -> Self {
        Self {
            label,
            command,
            disabled: false,
            checked: false,
            separator_before: false,
        }
    }

    pub(crate) fn checked_if(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    pub(crate) fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub(crate) fn separated(mut self) -> Self {
        self.separator_before = true;
        self
    }
}

#[derive(Debug)]
pub(crate) struct Menu {
    pub(crate) items: Vec<Item>,
    pub(crate) position: Point<Pixels>,
    highlighted: Option<usize>,
}

impl Menu {
    pub(crate) fn new(items: Vec<Item>, position: Point<Pixels>) -> Self {
        Self {
            items,
            position,
            highlighted: None,
        }
    }

    fn dimensions(
        &self,
        shortcuts: &[Option<String>],
        model: &AppModel,
        window: &Window,
    ) -> gpui::Size<Pixels> {
        let count = f32::from(u16::try_from(self.items.len()).unwrap_or(u16::MAX));
        let separators = f32::from(
            u16::try_from(
                self.items
                    .iter()
                    .filter(|item| item.separator_before)
                    .count(),
            )
            .unwrap_or(u16::MAX),
        );
        let mut width = px(
            if self
                .items
                .iter()
                .any(|item| matches!(item.command, Command::Tab(..)))
            {
                230.0
            } else {
                180.0
            },
        );
        let m = model.metrics;
        let mut style = window.text_style();
        style.font_weight = FontWeight::NORMAL;
        let measure = |text: &str, font_size| {
            window
                .text_system()
                .shape_line(
                    text.to_owned().into(),
                    font_size,
                    &[style.to_run(text.len())],
                    None,
                )
                .width
        };
        for (item, shortcut) in self.items.iter().zip(shortcuts) {
            let mut row_width = measure(item.label, m.font_emphasis())
                + px(12.0)
                + m.spacing2() * 3.0
                + m.spacing3() * 2.0
                + px(2.0);
            if let Some(shortcut) = shortcut {
                row_width += m.spacing2() + measure(shortcut, m.font_footnote());
            }
            width = width.max(row_width);
        }
        gpui::size(width, px(count * 22.0 + separators * 9.0 + 10.0))
    }

    fn move_highlight(&mut self, forward: bool) {
        let selectable: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.disabled)
            .map(|(index, _)| index)
            .collect();
        if selectable.is_empty() {
            self.highlighted = None;
            return;
        }
        let current = self
            .highlighted
            .and_then(|index| selectable.iter().position(|item| *item == index));
        let next = match (current, forward) {
            (Some(index), true) => (index + 1) % selectable.len(),
            (Some(index), false) => (index + selectable.len() - 1) % selectable.len(),
            (None, true) => 0,
            (None, false) => selectable.len() - 1,
        };
        self.highlighted = Some(selectable[next]);
    }
}

impl AppModel {
    fn move_menu_highlight(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(Overlay::Menu(menu)) = &mut self.overlay {
            menu.move_highlight(forward);
            cx.notify();
        }
    }

    fn confirm_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let command = match &self.overlay {
            Some(Overlay::Menu(menu)) => menu
                .highlighted
                .and_then(|index| menu.items.get(index))
                .filter(|item| !item.disabled)
                .map(|item| item.command),
            _ => None,
        };
        if let Some(command) = command {
            self.perform_menu(command, window, cx);
        }
    }

    fn perform_menu(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        let position = match &self.overlay {
            Some(Overlay::Menu(menu)) => menu.position,
            _ => gpui::point(px(8.0), px(40.0)),
        };
        self.dismiss_overlay(cx);
        match command {
            Command::Tab(id, action) => {
                use super::tab_menu::Action;
                match action {
                    Action::New(side) => self.new_tab_adjacent(id, side, cx),
                    Action::Rename => {
                        self.open_tab_editor(id, position, window, cx);
                        return;
                    }
                    Action::Color => {
                        self.open_tab_colors(id, position, window, cx);
                        return;
                    }
                    Action::ResetTitle => {
                        self.edit_tab(|state| state.set_tab_title(id, None), cx);
                    }
                    Action::ResetColor => {
                        self.edit_tab(|state| state.set_tab_color(id, None), cx);
                    }
                    Action::TogglePin => {
                        self.edit_tab(|state| state.toggle_tab_pin(id), cx);
                    }
                    Action::Close => self.close_tab(id, cx),
                    Action::CloseTabs(scope) => self.close_tabs(id, scope, cx),
                }
            }
            Command::Layout(layout) => self.set_layout(layout, cx),
            Command::FocusProject(focused) => self.set_project_focus(focused, cx),
            Command::SortProjects(order) => {
                self.appearance.sidebar_project_order = order;
                self.save_appearance(cx);
                cx.notify();
            }
            Command::Dismiss => {}
            Command::Worktrees(id) => {
                self.git.worktrees_anchor.set(Some(gpui::Bounds::new(
                    position,
                    gpui::size(px(0.0), px(0.0)),
                )));
                self.open_git_picker(id, super::git::Kind::Worktrees, window, cx);
                return;
            }
            Command::RemoveWorktree(id) => {
                self.git_request(id, muxy_protocol::GitAction::InspectRemoval, cx);
            }
            Command::ExistingSessions(project) => {
                self.open_session_picker(project, window, cx);
                return;
            }
            Command::DetachTerminal(pane) => self.detach_terminal(pane, cx),
            Command::SplitPane(id, _)
            | Command::ToggleZoomPane(id)
            | Command::ClosePane(id)
            | Command::TerminalCopy(id)
            | Command::TerminalPaste(id)
            | Command::TerminalSelectAll(id)
            | Command::TerminalSelectCommandOutput(id) => {
                self.perform_pane_menu(id, command, position, cx);
            }
            Command::CopyPath(id) => {
                if let Some(project) = self.state.project(id)
                    && project.status() == muxy_app_core::ProjectStatus::Available
                {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                        project.directory.to_string_lossy().into_owned(),
                    ));
                }
            }
            Command::RevealPath(id) => {
                if let Some(project) = self.state.project(id)
                    && project.status() == muxy_app_core::ProjectStatus::Available
                {
                    cx.reveal_path(&project.directory);
                }
            }
            Command::EditProject(id, field) => {
                self.open_project_editor(id, field, position, window, cx);
                return;
            }
            Command::ProjectColor(id) => {
                self.open_project_colors(id, position, window, cx);
                return;
            }
            Command::RemoveProject(id) => self.confirm_remove_project(id, cx),
        }
        self.focus_active(window, cx);
    }

    fn perform_pane_menu(
        &mut self,
        id: muxy_app_core::PaneId,
        command: Command,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        match command {
            Command::SplitPane(_, direction) => {
                self.focus_pane(id, cx);
                if self.active_pane() == Some(id) {
                    self.split_pane(direction, cx);
                }
            }
            Command::ToggleZoomPane(_) => {
                self.focus_pane(id, cx);
                if self.active_pane() == Some(id) {
                    self.toggle_zoom_pane(cx);
                }
            }
            Command::ClosePane(_) => self.close_pane(id, cx),
            Command::TerminalCopy(id)
            | Command::TerminalPaste(id)
            | Command::TerminalSelectAll(id)
            | Command::TerminalSelectCommandOutput(id) => {
                if let Some(pane) = self.terminal(&id) {
                    pane.view.update(cx, |pane, cx| match command {
                        Command::TerminalCopy(_) => pane.copy_selection(cx),
                        Command::TerminalPaste(_) => pane.paste_clipboard(cx),
                        Command::TerminalSelectAll(_) => pane.select_all(cx),
                        Command::TerminalSelectCommandOutput(_) => {
                            pane.select_command_output(Some(position), cx);
                        }
                        _ => {}
                    });
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn render(
    menu: &Menu,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let shortcuts: Vec<_> = menu
        .items
        .iter()
        .map(|item| item.command.shortcut(model, cx))
        .collect();
    let dimensions = menu.dimensions(&shortcuts, model, window);
    let origin = super::overlays::clamp(menu.position, dimensions, window.viewport_size());
    let mut panel = muxy_ui::popover::surface(theme, m)
        .debug_selector(|| "context-menu".into())
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(cx.listener(|model, _: &DismissMenu, _, cx| model.dismiss_overlay(cx)))
        .on_action(
            cx.listener(|model, _: &HighlightPrevious, _, cx| model.move_menu_highlight(false, cx)),
        )
        .on_action(
            cx.listener(|model, _: &HighlightNext, _, cx| model.move_menu_highlight(true, cx)),
        )
        .on_action(
            cx.listener(|model, _: &ConfirmHighlighted, window, cx| model.confirm_menu(window, cx)),
        )
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .w(dimensions.width)
        .py(m.spacing2());
    for (index, item) in menu.items.iter().enumerate() {
        if item.separator_before {
            panel = panel.child(
                div()
                    .flex_none()
                    .h(px(1.0))
                    .my(px(4.0))
                    .mx(m.spacing3())
                    .bg(theme.border),
            );
        }
        panel = panel.child(item.render(
            index,
            menu.highlighted == Some(index),
            shortcuts[index].clone(),
            model,
            cx,
        ));
    }
    panel.into_any_element()
}

impl Item {
    fn render(
        &self,
        index: usize,
        highlighted: bool,
        shortcut: Option<String>,
        model: &AppModel,
        cx: &Context<AppModel>,
    ) -> AnyElement {
        let item = self;
        let m = model.metrics;
        let theme = &model.theme;
        let command = item.command;
        let mut mark = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(12.0));
        if item.checked {
            mark = mark.child(SymbolGlyph::new("checkmark", m.font_caption(), theme.fg));
        }
        div()
            .id(SharedString::from(format!("menu-item-{index}")))
            .debug_selector(move || format!("menu-item-{index}"))
            .flex()
            .items_center()
            .gap(m.spacing2())
            .h(px(22.0))
            .px(m.spacing3())
            .mx(m.spacing2())
            .rounded(m.radius_sm())
            .font_weight(FontWeight::NORMAL)
            .when(highlighted && !item.disabled, |row| {
                row.bg(theme.fg_alpha(0.1))
            })
            .child(mark)
            .child(
                div()
                    .debug_selector({
                        let label = item.label;
                        move || format!("menu-label-{label}")
                    })
                    .flex_grow()
                    .whitespace_nowrap()
                    .text_size(m.font_emphasis())
                    .text_color(if item.disabled {
                        theme.fg_dim
                    } else {
                        theme.fg
                    })
                    .child(item.label),
            )
            .when_some(shortcut, |row, shortcut| {
                row.child(
                    div()
                        .debug_selector({
                            let label = item.label;
                            move || format!("menu-shortcut-{label}")
                        })
                        .flex_none()
                        .whitespace_nowrap()
                        .text_size(m.font_footnote())
                        .text_color(if item.disabled {
                            theme.fg_dim
                        } else {
                            theme.fg_muted
                        })
                        .child(shortcut),
                )
            })
            .when(!item.disabled, |row| {
                row.cursor_pointer()
                    .hover(|style| style.bg(theme.fg_alpha(0.1)))
                    .on_click(cx.listener(move |model, _, window, cx| {
                        model.perform_menu(command, window, cx);
                    }))
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_app_core::AppState;

    #[test]
    fn tab_menu_only_exposes_applicable_resets_and_closes() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut state = AppState::bootstrap()?;
        let tab = state.open_terminal_tab(state.home().id)?;
        let items = super::super::tab_menu::items(state.home(), tab);
        assert!(
            !items
                .iter()
                .any(|item| item.label == "Reset Title" || item.label == "Reset Tab Color")
        );
        assert!(
            items
                .iter()
                .filter(|item| matches!(
                    item.command,
                    Command::Tab(_, super::super::tab_menu::Action::CloseTabs(_))
                ))
                .all(|item| item.disabled)
        );
        state.toggle_tab_pin(tab)?;
        let items = super::super::tab_menu::items(state.home(), tab);
        assert!(!items.iter().any(|item| item.label == "Close Tab"));
        assert!(items.iter().any(|item| item.label == "Unpin Tab"));
        Ok(())
    }
}
