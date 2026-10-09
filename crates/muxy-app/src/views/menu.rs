use std::cell::RefCell;
use std::time::Duration;

use muxy_core::shortcuts::ShortcutId;

use gpui::{Bounds, Context, Hsla, Pixels, Point, SharedString, Task, Window, actions, px};
use muxy_app_core::PROJECT_COLORS;

use super::overlays::Overlay;
use crate::model::AppModel;

mod render;
mod shortcuts;

pub(crate) use render::render;

actions!(
    menu,
    [
        DismissMenu,
        HighlightPrevious,
        HighlightNext,
        ConfirmHighlighted,
        OpenSubmenu,
        CloseSubmenu
    ]
);

pub(crate) fn register_shortcuts(registry: &mut muxy_ui::shortcuts::Registry<'_>) {
    registry.register(ShortcutId::MenuDismissMenu, &DismissMenu);
    registry.register(ShortcutId::MenuHighlightPrevious, &HighlightPrevious);
    registry.register(ShortcutId::MenuHighlightNext, &HighlightNext);
    registry.register(ShortcutId::MenuConfirmHighlighted, &ConfirmHighlighted);
    registry.register(ShortcutId::MenuOpenSubmenu, &OpenSubmenu);
    registry.register(ShortcutId::MenuCloseSubmenu, &CloseSubmenu);
}

/// How long the pointer may cross other rows on its way into an open submenu.
const SUBMENU_GRACE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug)]
pub(crate) enum Command {
    Tab(muxy_app_core::TabId, super::tab_menu::Action),
    Layout(muxy_app_core::settings::AppLayout),
    FocusProject(bool),
    SelectWorkspace(Option<muxy_app_core::WorkspaceId>),
    SelectRemoteServers,
    NewWorkspace(Option<muxy_app_core::ProjectId>),
    RenameWorkspace(muxy_app_core::WorkspaceId),
    DeleteWorkspace(muxy_app_core::WorkspaceId),
    ToggleWorkspaceMember(muxy_app_core::WorkspaceId, muxy_app_core::ProjectId),
    SortProjects(muxy_app_core::settings::ProjectOrder),
    Worktrees(muxy_app_core::ProjectId),
    NewWorktree(muxy_app_core::ProjectId),
    NewProjectTab(muxy_app_core::ProjectId),
    RemoveWorktree(muxy_app_core::ProjectId),
    ExistingSessions(muxy_app_core::ProjectId),
    ProjectLayouts(muxy_app_core::ProjectId),
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
    /// Sets a project's color to `PROJECT_COLORS[index]`.
    ProjectColor(muxy_app_core::ProjectId, usize),
    ProjectLogo(muxy_app_core::ProjectId),
    RemoveProjectLogo(muxy_app_core::ProjectId),
    RemoveProject(muxy_app_core::ProjectId),
    AddProject,
    AddRemoteProject(muxy_app_core::ServerId),
    ManageRemoteDevices,
    /// The open remote file link's menu: copy the file here and open it.
    OpenRemoteCopy,
    CopyRemotePath,
}

#[derive(Clone, Debug)]
pub(crate) struct Item {
    label: SharedString,
    kind: Kind,
    disabled: bool,
    checked: Option<bool>,
    swatch: Option<Hsla>,
    separator_before: bool,
}

#[derive(Clone, Debug)]
enum Kind {
    Action(Command),
    Submenu(Vec<Item>),
}

impl Item {
    pub(crate) fn action(label: impl Into<SharedString>, command: Command) -> Self {
        Self::new(label, Kind::Action(command))
    }

    pub(crate) fn submenu(label: impl Into<SharedString>, items: Vec<Item>) -> Self {
        Self::new(label, Kind::Submenu(items))
    }

    fn new(label: impl Into<SharedString>, kind: Kind) -> Self {
        Self {
            label: label.into(),
            kind,
            disabled: false,
            checked: None,
            swatch: None,
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

    /// Shows a color dot before the label.
    pub(crate) fn swatch(mut self, color: Hsla) -> Self {
        self.swatch = Some(color);
        self
    }

    pub(crate) fn command(&self) -> Option<Command> {
        match self.kind {
            Kind::Action(command) => Some(command),
            Kind::Submenu(_) => None,
        }
    }

    fn submenu_items(&self) -> Option<&[Item]> {
        match &self.kind {
            Kind::Action(_) => None,
            Kind::Submenu(items) => Some(items),
        }
    }
}

/// The project palette as rows, each with its color dot, checking `current`.
pub(crate) fn color_items(current: Option<&str>, command: impl Fn(usize) -> Command) -> Vec<Item> {
    PROJECT_COLORS
        .iter()
        .enumerate()
        .map(|(index, (name, hex))| {
            let item = Item::action(muxy_ui::l10n::translate(name), command(index))
                .checked_if(current == Some(*hex));
            match muxy_ui::theme::parse_hex(hex) {
                Some(color) => item.swatch(color.into()),
                None => item,
            }
        })
        .collect()
}

/// An open context menu and the submenus opened from it.
#[derive(Debug)]
pub(crate) struct Menu {
    pub(crate) items: Vec<Item>,
    pub(crate) position: Point<Pixels>,
    /// The root menu first, then one entry per open submenu, each opened from
    /// the row highlighted in the level before it.
    levels: Vec<Level>,
    pointer: Option<Point<Pixels>>,
    /// A row the pointer rests on while it heads for the open submenu.
    pending: Option<(usize, usize)>,
    grace: Option<Task<()>>,
    /// Where each level was last drawn.
    panels: RefCell<Vec<Bounds<Pixels>>>,
}

#[derive(Debug, Default)]
struct Level {
    highlighted: Option<usize>,
    scroll: gpui::ScrollHandle,
}

#[derive(Debug, PartialEq, Eq)]
enum Hover {
    Changed,
    Unchanged,
    Waiting,
}

impl Menu {
    pub(crate) fn new(items: Vec<Item>, position: Point<Pixels>) -> Self {
        Self {
            items,
            position,
            levels: vec![Level::default()],
            pointer: None,
            pending: None,
            grace: None,
            panels: RefCell::default(),
        }
    }

    fn items_at(&self, level: usize) -> Option<&[Item]> {
        if level >= self.levels.len() {
            return None;
        }
        let mut items = self.items.as_slice();
        for opened in &self.levels[..level] {
            items = opened
                .highlighted
                .and_then(|index| items.get(index))
                .and_then(Item::submenu_items)?;
        }
        Some(items)
    }

    fn item(&self, level: usize, index: usize) -> Option<&Item> {
        self.items_at(level)?.get(index)
    }

    /// The deepest level with a highlighted row. The keyboard acts there.
    fn active(&self) -> usize {
        self.levels
            .iter()
            .rposition(|level| level.highlighted.is_some())
            .unwrap_or(0)
    }

    fn highlighted_item(&self) -> Option<&Item> {
        let level = self.active();
        self.item(level, self.levels[level].highlighted?)
    }

    fn move_highlight(&mut self, forward: bool) -> bool {
        self.pending = None;
        let level = self.active();
        let Some(items) = self.items_at(level) else {
            return false;
        };
        let selectable: Vec<_> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.disabled)
            .map(|(index, _)| index)
            .collect();
        let current = self.levels[level]
            .highlighted
            .and_then(|index| selectable.iter().position(|item| *item == index));
        let next = match (current, forward, selectable.len()) {
            (_, _, 0) => None,
            (Some(index), true, count) => Some((index + 1) % count),
            (Some(index), false, count) => Some((index + count - 1) % count),
            (None, true, _) => Some(0),
            (None, false, count) => Some(count - 1),
        };
        let index = next.map(|next| selectable[next]);
        let separators = index.map(|index| {
            items[..=index]
                .iter()
                .filter(|item| item.separator_before)
                .count()
        });
        self.levels.truncate(level + 1);
        self.levels[level].highlighted = index;
        if let (Some(index), Some(separators)) = (index, separators) {
            self.levels[level].scroll.scroll_to_item(index + separators);
        }
        true
    }

    /// Opens the highlighted row's submenu and highlights its first row.
    fn open_submenu(&mut self) -> bool {
        self.pending = None;
        let level = self.active();
        let Some(first) = self
            .highlighted_item()
            .filter(|item| !item.disabled)
            .and_then(Item::submenu_items)
            .map(|items| items.iter().position(|item| !item.disabled))
        else {
            return false;
        };
        self.levels.truncate(level + 1);
        self.levels.push(Level {
            highlighted: first,
            ..Level::default()
        });
        true
    }

    /// Closes the innermost submenu. Its row stays highlighted in the parent.
    fn close_submenu(&mut self) -> bool {
        self.pending = None;
        if self.levels.len() < 2 {
            return false;
        }
        self.levels.truncate(self.active().max(1));
        true
    }

    /// Highlights a row and opens its submenu, closing any other.
    fn point_at(&mut self, level: usize, index: usize) -> bool {
        let Some(item) = self.item(level, index) else {
            return false;
        };
        let enabled = !item.disabled;
        let submenu = enabled && item.submenu_items().is_some();
        let highlighted = enabled.then_some(index);
        let open = self.levels.len() > level + 1;
        if self.levels[level].highlighted == highlighted && open == submenu {
            if !submenu
                || (self.levels.len() == level + 2 && self.levels[level + 1].highlighted.is_none())
            {
                return false;
            }
            self.levels.truncate(level + 2);
            self.levels[level + 1].highlighted = None;
            return true;
        }
        self.levels.truncate(level + 1);
        self.levels[level].highlighted = highlighted;
        if submenu {
            self.levels.push(Level::default());
        }
        true
    }

    fn hover(&mut self, level: usize, index: usize, position: Point<Pixels>) -> Hover {
        let previous = self.pointer.replace(position);
        if level >= self.levels.len() {
            return Hover::Unchanged;
        }
        let elsewhere =
            self.levels.len() > level + 1 && self.levels[level].highlighted != Some(index);
        if elsewhere
            && let Some(previous) = previous
            && let Some(&[.., parent, submenu]) = self.panels.borrow().get(..=level + 1)
            && heading_into(previous, position, parent, submenu)
        {
            self.pending = Some((level, index));
            return Hover::Waiting;
        }
        self.pending = None;
        if self.point_at(level, index) {
            Hover::Changed
        } else {
            Hover::Unchanged
        }
    }

    /// The pointer left a row. A row without an open submenu loses its highlight.
    fn leave(&mut self, level: usize, index: usize) -> bool {
        if self.pending == Some((level, index)) {
            self.pending = None;
        }
        if self.levels.len() == level + 1 && self.levels[level].highlighted == Some(index) {
            self.levels[level].highlighted = None;
            return true;
        }
        false
    }

    fn settle(&mut self) -> bool {
        self.pending
            .take()
            .is_some_and(|(level, index)| self.point_at(level, index))
    }
}

/// Whether the pointer, moving from `previous` to `current`, is heading into
/// `submenu` beside `parent`: it moves toward the submenu's near edge and stays
/// inside the triangle between where it was and that edge.
fn heading_into(
    previous: Point<Pixels>,
    current: Point<Pixels>,
    parent: Bounds<Pixels>,
    submenu: Bounds<Pixels>,
) -> bool {
    let (edge, toward) = if submenu.center().x >= parent.center().x {
        (submenu.left(), current.x > previous.x)
    } else {
        (submenu.right(), current.x < previous.x)
    };
    toward
        && inside_triangle(
            current,
            previous,
            gpui::point(edge, submenu.top() - px(1.0)),
            gpui::point(edge, submenu.bottom() + px(1.0)),
        )
}

fn inside_triangle(
    point: Point<Pixels>,
    a: Point<Pixels>,
    b: Point<Pixels>,
    c: Point<Pixels>,
) -> bool {
    let cross = |from: Point<Pixels>, to: Point<Pixels>| {
        f32::from(to.x - from.x) * f32::from(point.y - from.y)
            - f32::from(to.y - from.y) * f32::from(point.x - from.x)
    };
    let sides = [cross(a, b), cross(b, c), cross(c, a)];
    !(sides.iter().any(|side| *side < 0.0) && sides.iter().any(|side| *side > 0.0))
}

impl AppModel {
    fn update_menu(&mut self, cx: &mut Context<Self>, update: impl FnOnce(&mut Menu) -> bool) {
        if let Some(Overlay::Menu(menu)) = &mut self.overlay
            && update(menu)
        {
            cx.notify();
        }
    }

    fn dismiss_menu_level(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::Menu(menu)) = &mut self.overlay
            && menu.close_submenu()
        {
            cx.notify();
        } else {
            self.dismiss_overlay(cx);
        }
    }

    fn confirm_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Menu(menu)) = &mut self.overlay else {
            return;
        };
        let Some(item) = menu.highlighted_item().filter(|item| !item.disabled) else {
            return;
        };
        if let Some(command) = item.command() {
            self.perform_menu(command, window, cx);
        } else {
            menu.open_submenu();
            cx.notify();
        }
    }

    fn hover_menu_row(
        &mut self,
        level: usize,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::Menu(menu)) = &mut self.overlay else {
            return;
        };
        match menu.hover(level, index, position) {
            Hover::Changed => cx.notify(),
            Hover::Unchanged => {}
            Hover::Waiting => {
                menu.grace = Some(cx.spawn(async move |model, cx| {
                    cx.background_executor().timer(SUBMENU_GRACE).await;
                    let _ = model.update(cx, |model, cx| model.update_menu(cx, Menu::settle));
                }));
            }
        }
    }

    fn track_menu_pointer(&mut self, position: Point<Pixels>) {
        if let Some(Overlay::Menu(menu)) = &mut self.overlay {
            menu.pointer = Some(position);
        }
    }

    fn click_menu_row(
        &mut self,
        level: usize,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::Menu(menu)) = &mut self.overlay else {
            return;
        };
        let Some(item) = menu.item(level, index).filter(|item| !item.disabled) else {
            return;
        };
        if let Some(command) = item.command() {
            self.perform_menu(command, window, cx);
        } else {
            menu.pending = None;
            if menu.point_at(level, index) {
                cx.notify();
            }
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
                    Action::ResetTitle => {
                        self.edit_tab(|state| state.set_tab_title(id, None), cx);
                    }
                    Action::SetColor(index) => {
                        if let Some(color) = palette_color(index) {
                            self.edit_tab(|state| state.set_tab_color(id, Some(color)), cx);
                        }
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
            Command::SelectRemoteServers => self.select_remote_servers(cx),
            Command::NewWorkspace(project) => {
                self.open_new_workspace_editor(project, position, window, cx);
                return;
            }
            Command::RenameWorkspace(id) => {
                self.open_workspace_editor(id, position, window, cx);
                return;
            }
            Command::DeleteWorkspace(id) => self.confirm_delete_workspace(id, cx),
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
            Command::ProjectLayouts(project) => {
                self.open_layout_picker(project, window, cx);
                return;
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
                    && project.server_id.is_local()
                {
                    cx.reveal_path(&project.directory);
                }
            }
            Command::EditProject(id, field) => {
                self.open_project_editor(id, field, position, window, cx);
                return;
            }
            Command::ProjectColor(id, index) => {
                if let Some(color) = palette_color(index) {
                    self.edit_project(|state| state.set_project_color(id, color), cx);
                }
            }
            Command::ProjectLogo(id) => self.choose_project_logo(id, cx),
            Command::RemoveProjectLogo(id) => {
                self.edit_project(|state| state.set_project_logo(id, None), cx);
            }
            Command::RemoveProject(id) => self.confirm_remove_project(id, cx),
            Command::AddProject => {
                self.open_project_picker(window, cx);
                return;
            }
            Command::AddRemoteProject(server) => {
                self.open_remote_project_picker(server, cx);
                return;
            }
            Command::ManageRemoteDevices => {
                self.open_remote_servers(None, window, cx);
                return;
            }
            Command::OpenRemoteCopy => self.open_remote_copy(cx),
            Command::CopyRemotePath => self.copy_remote_path(cx),
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

fn palette_color(index: usize) -> Option<muxy_app_core::Color> {
    PROJECT_COLORS.get(index)?.1.parse().ok()
}

#[cfg(test)]
impl AppModel {
    /// The open menu's rows per level, with `>` before the highlighted row and
    /// `✓` before checked ones.
    pub(crate) fn menu_outline(&self) -> Vec<Vec<String>> {
        let Some(Overlay::Menu(menu)) = &self.overlay else {
            return Vec::new();
        };
        (0..menu.levels.len())
            .map(|level| {
                let items = menu.items_at(level).unwrap_or_default();
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let mut row = String::new();
                        if menu.levels[level].highlighted == Some(index) {
                            row.push('>');
                        }
                        if item.checked == Some(true) {
                            row.push('✓');
                        }
                        row.push_str(&item.label);
                        row
                    })
                    .collect()
            })
            .collect()
    }
}
