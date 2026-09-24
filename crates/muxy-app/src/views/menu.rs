use muxy_core::shortcuts::ShortcutId;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    SharedString, StatefulInteractiveElement, Styled, Window, actions, div, px,
};
use muxy_ui::components::SymbolGlyph;
use muxy_ui::popover;
use muxy_ui::theme::Metrics;

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
    SelectWorkspace(Option<muxy_app_core::WorkspaceId>),
    NewWorkspace(Option<muxy_app_core::ProjectId>),
    RenameWorkspace(muxy_app_core::WorkspaceId),
    DeleteWorkspace(muxy_app_core::WorkspaceId),
    ProjectWorkspaces(muxy_app_core::ProjectId),
    ToggleWorkspaceMember(muxy_app_core::WorkspaceId, muxy_app_core::ProjectId),
    SortProjects(muxy_app_core::settings::ProjectOrder),
    Worktrees(muxy_app_core::ProjectId),
    NewWorktree(muxy_app_core::ProjectId),
    NewProjectTab(muxy_app_core::ProjectId),
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
    ProjectLogo(muxy_app_core::ProjectId),
    RemoveProjectLogo(muxy_app_core::ProjectId),
    RemoveProject(muxy_app_core::ProjectId),
}

#[derive(Clone, Debug)]
pub(crate) struct Item {
    label: SharedString,
    command: Command,
    disabled: bool,
    checked: Option<bool>,
    separator_before: bool,
}

impl Item {
    pub(crate) fn action(label: impl Into<SharedString>, command: Command) -> Self {
        Self {
            label: label.into(),
            command,
            disabled: false,
            checked: None,
            separator_before: false,
        }
    }

    pub(crate) fn checked_if(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
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
    scroll: gpui::ScrollHandle,
}

impl Menu {
    pub(crate) fn new(items: Vec<Item>, position: Point<Pixels>) -> Self {
        Self {
            items,
            position,
            highlighted: None,
            scroll: gpui::ScrollHandle::new(),
        }
    }

    fn has_checkmarks(&self) -> bool {
        self.items.iter().any(|item| item.checked.is_some())
    }

    fn dimensions(
        &self,
        shortcuts: &[Option<String>],
        model: &AppModel,
        window: &Window,
    ) -> gpui::Size<Pixels> {
        let m = model.metrics;
        let mut width = m.scaled(
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
        let checkmark_width = if self.has_checkmarks() {
            m.scaled(12.0 + popover::ROW_PADDING)
        } else {
            px(0.0)
        };
        for (item, shortcut) in self.items.iter().zip(shortcuts) {
            let mut row_width = measure(&item.label, m.font_body())
                + checkmark_width
                + m.scaled(popover::ROW_PADDING) * 2.0
                + m.scaled(popover::PADDING) * 2.0
                + px(2.0);
            if let Some(shortcut) = shortcut {
                row_width += m.scaled(popover::ROW_PADDING) + measure(shortcut, m.font_footnote());
            }
            width = width.max(row_width);
        }
        let viewport = window.viewport_size();
        gpui::size(
            width.min((viewport.width - px(16.0)).max(px(0.0))),
            self.height(m)
                .min((viewport.height - px(16.0)).max(px(0.0))),
        )
    }

    fn height(&self, m: Metrics) -> Pixels {
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
        m.scaled(popover::PADDING) * 2.0
            + m.scaled(popover::ROW_HEIGHT) * count
            + (px(1.0) + m.spacing2() * 2.0) * separators
            + m.scaled(popover::ROW_GAP) * (count + separators - 1.0).max(0.0)
            + px(2.0)
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
        let index = selectable[next];
        self.highlighted = Some(index);
        let separators = self.items[..=index]
            .iter()
            .filter(|item| item.separator_before)
            .count();
        self.scroll.scroll_to_item(index + separators);
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

    #[allow(clippy::too_many_lines, reason = "Exhaustive menu command dispatch")]
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
            Command::SelectWorkspace(workspace) => self.select_workspace(workspace, cx),
            Command::NewWorkspace(project) => {
                self.open_new_workspace_editor(project, position, window, cx);
                return;
            }
            Command::RenameWorkspace(id) => {
                self.open_workspace_editor(id, position, window, cx);
                return;
            }
            Command::DeleteWorkspace(id) => self.confirm_delete_workspace(id, cx),
            Command::ProjectWorkspaces(project) => {
                let items = super::project_menu::workspace_items(&self.state, project);
                self.open_menu(items, position, window, cx);
                return;
            }
            Command::ToggleWorkspaceMember(workspace, project) => {
                let member = self
                    .state
                    .workspace(workspace)
                    .is_some_and(|workspace| workspace.projects.contains(&project));
                self.edit_workspaces(
                    |state| state.set_workspace_member(workspace, project, !member),
                    cx,
                );
            }
            Command::SortProjects(order) => {
                self.appearance.sidebar_project_order = order;
                self.save_appearance(cx);
                cx.notify();
            }
            Command::Dismiss => {}
            Command::NewWorktree(id) => {
                self.open_git_form(id, true, cx);
                return;
            }
            Command::NewProjectTab(id) => {
                self.select_project(id, cx);
                self.new_tab(cx);
            }
            Command::Worktrees(id) => {
                self.toggle_worktree_visibility(id, cx);
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
            Command::ProjectLogo(id) => self.choose_project_logo(id, cx),
            Command::RemoveProjectLogo(id) => {
                self.edit_project(|state| state.set_project_logo(id, None), cx);
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
    let mut panel = popover::surface(theme, m)
        .id("context-menu")
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
        .h(dimensions.height)
        .overflow_y_scroll()
        .track_scroll(&menu.scroll);
    let has_checkmarks = menu.has_checkmarks();
    for (index, item) in menu.items.iter().enumerate() {
        if item.separator_before {
            panel = panel.child(
                div()
                    .flex()
                    .flex_none()
                    .flex_col()
                    .child(popover::divider(theme, m)),
            );
        }
        panel = panel.child(item.render(
            index,
            menu.highlighted == Some(index),
            shortcuts[index].clone(),
            has_checkmarks,
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
        has_checkmarks: bool,
        model: &AppModel,
        cx: &Context<AppModel>,
    ) -> AnyElement {
        let item = self;
        let m = model.metrics;
        let theme = &model.theme;
        let command = item.command;
        popover::row(
            theme,
            m,
            SharedString::from(format!("menu-item-{index}")),
            !item.disabled,
            highlighted,
        )
        .debug_selector(move || format!("menu-item-{index}"))
        .font_weight(FontWeight::NORMAL)
        .when(has_checkmarks, |row| {
            row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(m.scaled(12.0))
                    .when(item.checked == Some(true), |mark| {
                        mark.child(SymbolGlyph::new("checkmark", m.font_caption(), theme.fg))
                    }),
            )
        })
        .child(
            div()
                .debug_selector({
                    let label = item.label.clone();
                    move || format!("menu-label-{label}")
                })
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .child(item.label.clone()),
        )
        .when_some(shortcut, |row, shortcut| {
            row.child(
                div()
                    .debug_selector({
                        let label = item.label.clone();
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
            row.on_click(cx.listener(move |model, _, window, cx| {
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
    fn menu_height_tracks_shared_rows_separators_and_scale() {
        let menu = Menu::new(
            vec![
                Item::action("First", Command::Dismiss),
                Item::action("Second", Command::Dismiss).separated(),
                Item::action("Third", Command::Dismiss),
            ],
            gpui::point(px(0.0), px(0.0)),
        );
        for scale in [1.0, 1.5, 2.0] {
            let m = Metrics::new(scale);
            assert_eq!(menu.height(m), m.scaled(91.0) + px(3.0));
            let origin = super::super::overlays::clamp(
                gpui::point(px(780.0), px(590.0)),
                gpui::size(m.scaled(180.0), menu.height(m)),
                gpui::size(px(800.0), px(600.0)),
            );
            assert!(origin.y + menu.height(m) <= px(592.0));
        }
        let empty = Menu::new(Vec::new(), gpui::point(px(0.0), px(0.0)));
        assert_eq!(empty.height(Metrics::new(1.0)), px(10.0));
    }

    #[test]
    fn worktrees_menu_tracks_the_visibility_checkbox() {
        let mut state = AppState::bootstrap().expect("state");
        let id = state.add_project(std::env::temp_dir()).expect("project");
        for checked in [true, false] {
            let items =
                super::super::project_menu::items(state.project(id).expect("project"), checked);
            let item = items
                .iter()
                .find(|item| item.label == "Worktrees")
                .expect("checkbox");
            assert_eq!(item.checked, Some(checked));
            assert!(matches!(item.command, Command::Worktrees(project) if project == id));
            assert!(!items.iter().any(|item| item.label == "Worktrees…"));
        }
        assert!(
            !super::super::project_menu::items(state.home(), true)
                .iter()
                .any(|item| matches!(item.command, Command::Worktrees(_)))
        );
    }

    #[test]
    fn workspace_menus_check_memberships_for_top_level_projects() {
        let mut state = AppState::bootstrap().expect("state");
        let project = state.add_project(std::env::temp_dir()).expect("project");
        let items = super::super::project_menu::workspace_items(&state, project);
        assert_eq!(items.len(), 1);
        assert!(!items[0].separator_before);
        assert!(matches!(items[0].command, Command::NewWorkspace(Some(id)) if id == project));
        let work = state.create_workspace("Work").expect("work");
        state.create_workspace("Other").expect("other");
        state
            .set_workspace_member(work, project, true)
            .expect("member");
        let items = super::super::project_menu::workspace_items(&state, project);
        let marks: Vec<_> = items
            .iter()
            .map(|item| (item.label.as_ref(), item.checked, item.separator_before))
            .collect();
        assert_eq!(
            marks,
            [
                ("Work", Some(true), false),
                ("Other", Some(false), false),
                ("New Workspace…", None, true),
            ]
        );
        assert!(
            matches!(items[0].command, Command::ToggleWorkspaceMember(id, target) if id == work && target == project)
        );
        let has_workspaces = |project: &muxy_app_core::Project| {
            super::super::project_menu::items(project, true)
                .iter()
                .any(|item| matches!(item.command, Command::ProjectWorkspaces(_)))
        };
        assert!(has_workspaces(state.project(project).expect("project")));
        assert!(!has_workspaces(state.home()));
    }

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
