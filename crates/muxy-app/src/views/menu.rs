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
    NewWorkspace(Option<muxy_app_core::ProjectId>),
    RenameWorkspace(muxy_app_core::WorkspaceId),
    DeleteWorkspace(muxy_app_core::WorkspaceId),
    ToggleWorkspaceMember(muxy_app_core::WorkspaceId, muxy_app_core::ProjectId),
    SortProjects(muxy_app_core::settings::ProjectOrder),
    Worktrees(muxy_app_core::ProjectId),
    NewWorktree(muxy_app_core::ProjectId),
    NewProjectTab(muxy_app_core::ProjectId),
    RemoveWorktree(muxy_app_core::ProjectId),
    #[cfg(test)]
    Dismiss,
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

    fn command(&self) -> Option<Command> {
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
            let item = Item::action(*name, command(index)).checked_if(current == Some(*hex));
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
            #[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::render::{height, submenu_origin};
    use super::*;
    use crate::views::{project_menu, tab_menu};
    use gpui::{point, size};
    use muxy_app_core::{AppState, ServerId};
    use muxy_ui::theme::Metrics;

    fn labels(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|item| {
                let mut label = item.label.to_string();
                if item.separator_before {
                    label.insert_str(0, "— ");
                }
                if item.submenu_items().is_some() {
                    label.push_str(" ›");
                }
                label
            })
            .collect()
    }

    fn submenu<'a>(items: &'a [Item], label: &str) -> &'a [Item] {
        items
            .iter()
            .find(|item| item.label == label)
            .and_then(Item::submenu_items)
            .unwrap_or_default()
    }

    fn nested() -> Menu {
        Menu::new(
            vec![
                Item::action("Leaf", Command::Dismiss),
                Item::submenu(
                    "Colors",
                    vec![
                        Item::action("Off", Command::Dismiss).disabled(),
                        Item::action("Red", Command::Dismiss),
                        Item::action("Blue", Command::Dismiss),
                    ],
                ),
                Item::submenu("Empty", Vec::new()).disabled(),
                Item::action("Last", Command::Dismiss).separated(),
            ],
            point(px(0.0), px(0.0)),
        )
    }

    fn highlights(menu: &Menu) -> Vec<Option<usize>> {
        menu.levels.iter().map(|level| level.highlighted).collect()
    }

    #[test]
    fn menu_height_tracks_shared_rows_separators_and_scale() {
        let items = vec![
            Item::action("First", Command::Dismiss),
            Item::action("Second", Command::Dismiss).separated(),
            Item::action("Third", Command::Dismiss),
        ];
        for scale in [1.0, 1.5, 2.0] {
            let m = Metrics::new(scale);
            assert_eq!(height(&items, m), m.scaled(91.0) + px(3.0));
            let origin = super::super::overlays::clamp(
                point(px(780.0), px(590.0)),
                size(m.scaled(180.0), height(&items, m)),
                size(px(800.0), px(600.0)),
            );
            assert!(origin.y + height(&items, m) <= px(592.0));
        }
        assert_eq!(height(&[], Metrics::new(1.0)), px(10.0));
    }

    #[test]
    fn project_menus_group_actions_and_nest_appearance_workspaces_and_worktrees() {
        let mut state = AppState::bootstrap().expect("state");
        let id = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        let project = state.project(id).expect("project");
        let items = project_menu::items(&state, project, Some(true));
        assert_eq!(
            labels(&items),
            [
                "New Terminal Tab",
                "Existing Terminals…",
                "Apply Layout…",
                "— Rename…",
                "Icon ›",
                "Color ›",
                "Workspaces ›",
                "— Worktrees ›",
                "— Reveal in Finder",
                "Copy Path",
                "— Remove Project…",
            ]
        );
        assert_eq!(
            labels(submenu(&items, "Icon")),
            ["Choose Icon…", "Set Logo…"]
        );
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01".to_vec();
        png.resize(33, 0);
        let mut logo = state.clone();
        logo.set_project_logo(id, Some(png.into())).expect("logo");
        let items = project_menu::items(&logo, logo.project(id).expect("project"), None);
        assert_eq!(
            labels(submenu(&items, "Icon")),
            ["Choose Icon…", "Set Logo…", "Remove Logo"]
        );
        let items = project_menu::items(&state, project, Some(true));
        let worktrees = submenu(&items, "Worktrees");
        assert_eq!(labels(worktrees), ["Show Worktrees", "— New Worktree…"]);
        assert_eq!(worktrees[0].checked, Some(true));
        assert!(
            matches!(worktrees[0].command(), Some(Command::Worktrees(project)) if project == id)
        );

        let hidden = project_menu::items(&state, project, Some(false));
        let worktrees = submenu(&hidden, "Worktrees");
        assert_eq!(labels(worktrees), ["Show Worktrees"]);
        assert_eq!(worktrees[0].checked, Some(false));

        let plain = project_menu::items(&state, project, None);
        assert!(
            !labels(&plain)
                .iter()
                .any(|label| label.contains("Worktrees"))
        );

        let home = project_menu::items(&state, state.home(), Some(true));
        assert_eq!(
            labels(&home),
            [
                "New Terminal Tab",
                "Existing Terminals…",
                "Apply Layout…",
                "— Rename…",
                "Icon ›",
                "Color ›",
                "— Reveal in Finder",
                "Copy Path",
            ]
        );
    }

    #[test]
    fn missing_projects_and_worktrees_only_offer_removal() {
        let mut state = AppState::bootstrap().expect("state");
        let folder = tempfile::tempdir().expect("folder");
        let id = state
            .add_project(ServerId::local(), folder.path().to_owned())
            .expect("project");
        drop(folder);
        state.refresh_project_statuses();
        let project = state.project(id).expect("project");
        assert_eq!(project.status(), muxy_app_core::ProjectStatus::Missing);
        for items in [
            project_menu::items(&state, project, Some(true)),
            project_menu::worktree_items(project, false),
        ] {
            assert_eq!(labels(&items), ["Remove Project…"]);
        }
    }

    #[test]
    fn worktree_rows_share_one_menu_and_the_primary_row_skips_removal() {
        let mut state = AppState::bootstrap().expect("state");
        let id = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        let project = state.project(id).expect("project");
        assert_eq!(
            labels(&project_menu::worktree_items(project, false)),
            [
                "New Terminal Tab",
                "Existing Terminals…",
                "Apply Layout…",
                "— Rename Worktree…",
                "— Reveal in Finder",
                "Copy Path",
                "— Remove Worktree and Files…",
                "Remove Project…",
            ]
        );
        assert_eq!(
            labels(&project_menu::worktree_items(project, true)),
            [
                "New Terminal Tab",
                "Existing Terminals…",
                "Apply Layout…",
                "— Reveal in Finder",
                "Copy Path",
            ]
        );
    }

    #[test]
    fn workspace_menus_check_memberships_for_top_level_projects() {
        let mut state = AppState::bootstrap().expect("state");
        let project = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        let items = project_menu::workspace_items(&state, project);
        assert_eq!(items.len(), 1);
        assert!(!items[0].separator_before);
        assert!(
            matches!(items[0].command(), Some(Command::NewWorkspace(Some(id))) if id == project)
        );
        let work = state.create_workspace("Work").expect("work");
        state.create_workspace("Other").expect("other");
        state
            .set_workspace_member(work, project, true)
            .expect("member");
        let items = project_menu::workspace_items(&state, project);
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
            matches!(items[0].command(), Some(Command::ToggleWorkspaceMember(id, target)) if id == work && target == project)
        );
    }

    #[test]
    fn color_menus_list_the_palette_with_dots_and_check_the_current_color() {
        let mut state = AppState::bootstrap().expect("state");
        let id = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        let color = PROJECT_COLORS[3].1.parse().expect("color");
        state.set_project_color(id, color).expect("color");
        let items = project_menu::items(&state, state.project(id).expect("project"), None);
        let colors = submenu(&items, "Color");
        assert_eq!(colors.len(), PROJECT_COLORS.len());
        for (index, item) in colors.iter().enumerate() {
            assert_eq!(item.label, PROJECT_COLORS[index].0);
            assert!(item.swatch.is_some());
            assert_eq!(item.checked, Some(index == 3));
            assert!(
                matches!(item.command(), Some(Command::ProjectColor(project, color)) if project == id && color == index)
            );
        }

        let tab = state.open_terminal_tab(id).expect("tab");
        let colors = |state: &AppState| {
            submenu(
                &tab_menu::items(state.project(id).expect("project"), tab),
                "Color",
            )
            .to_vec()
        };
        let plain = colors(&state);
        assert_eq!(plain.len(), PROJECT_COLORS.len() + 1);
        assert_eq!(
            (plain[0].label.as_ref(), plain[0].checked, plain[0].swatch),
            ("Default", Some(true), None)
        );
        assert!(plain[1].separator_before);
        assert!(plain[1..].iter().all(|item| item.checked == Some(false)));
        state
            .set_tab_color(tab, Some(PROJECT_COLORS[5].1.parse().expect("color")))
            .expect("tab color");
        let tinted = colors(&state);
        assert_eq!(tinted[0].checked, Some(false));
        assert_eq!(tinted[6].checked, Some(true));
    }

    #[test]
    fn tab_menu_only_exposes_applicable_resets_and_closes() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut state = AppState::bootstrap()?;
        let tab = state.open_terminal_tab(state.home().id)?;
        let items = tab_menu::items(state.home(), tab);
        assert!(!items.iter().any(|item| item.label == "Reset Title"));
        assert!(
            items
                .iter()
                .filter(|item| matches!(
                    item.command(),
                    Some(Command::Tab(_, tab_menu::Action::CloseTabs(_)))
                ))
                .all(|item| item.disabled)
        );
        state.toggle_tab_pin(tab)?;
        let items = tab_menu::items(state.home(), tab);
        assert!(!items.iter().any(|item| item.label == "Close Tab"));
        assert!(items.iter().any(|item| item.label == "Unpin Tab"));
        Ok(())
    }

    #[test]
    fn pointing_opens_submenus_and_the_keyboard_walks_in_and_out() {
        let mut menu = nested();
        assert!(menu.point_at(0, 1));
        assert_eq!(highlights(&menu), [Some(1), None]);
        assert!(!menu.point_at(0, 1));
        assert!(menu.point_at(0, 0));
        assert_eq!(highlights(&menu), [Some(0)]);
        assert!(!menu.open_submenu());
        assert!(menu.point_at(0, 2));
        assert_eq!(highlights(&menu), [None], "disabled rows never open");

        menu.move_highlight(true);
        menu.move_highlight(true);
        assert_eq!(highlights(&menu), [Some(1)], "arrows don't open submenus");
        assert!(menu.open_submenu());
        assert_eq!(
            highlights(&menu),
            [Some(1), Some(1)],
            "skips the disabled row"
        );
        menu.move_highlight(true);
        assert_eq!(highlights(&menu), [Some(1), Some(2)]);
        menu.move_highlight(true);
        assert_eq!(
            highlights(&menu),
            [Some(1), Some(1)],
            "wraps inside the submenu"
        );
        assert!(matches!(menu.highlighted_item(), Some(item) if item.label == "Red"));
        assert!(menu.close_submenu());
        assert_eq!(highlights(&menu), [Some(1)]);
        assert!(!menu.close_submenu());

        assert!(menu.point_at(0, 1));
        assert!(menu.point_at(1, 2));
        assert!(
            menu.point_at(0, 1),
            "returning to the parent row clears the submenu"
        );
        assert_eq!(highlights(&menu), [Some(1), None]);
        menu.move_highlight(true);
        assert_eq!(
            highlights(&menu),
            [Some(3)],
            "arrows act where the highlight is"
        );
    }

    #[test]
    fn leaving_a_row_drops_its_highlight_unless_its_submenu_is_open() {
        let mut menu = nested();
        menu.point_at(0, 0);
        assert!(menu.leave(0, 0));
        assert_eq!(highlights(&menu), [None]);
        menu.point_at(0, 1);
        assert!(!menu.leave(0, 1));
        menu.point_at(1, 1);
        assert!(menu.leave(1, 1));
        assert_eq!(highlights(&menu), [Some(1), None]);
    }

    #[test]
    fn crossing_rows_toward_an_open_submenu_waits_until_the_pointer_settles() {
        let mut menu = nested();
        let parent = Bounds::new(point(px(100.0), px(100.0)), size(px(180.0), px(120.0)));
        let submenu = Bounds::new(point(px(276.0), px(120.0)), size(px(180.0), px(90.0)));
        *menu.panels.borrow_mut() = vec![parent, submenu];
        assert_eq!(
            menu.hover(0, 1, point(px(200.0), px(135.0))),
            Hover::Changed
        );
        assert_eq!(highlights(&menu), [Some(1), None]);
        assert_eq!(
            menu.hover(0, 3, point(px(240.0), px(170.0))),
            Hover::Waiting
        );
        assert_eq!(highlights(&menu), [Some(1), None]);
        assert!(menu.settle());
        assert_eq!(highlights(&menu), [Some(3)]);
        assert!(!menu.settle());

        menu.point_at(0, 1);
        menu.pointer = Some(point(px(200.0), px(135.0)));
        assert_eq!(
            menu.hover(0, 3, point(px(190.0), px(170.0))),
            Hover::Changed,
            "moving away switches at once"
        );
        menu.point_at(0, 1);
        menu.pointer = Some(point(px(200.0), px(135.0)));
        assert_eq!(
            menu.hover(0, 3, point(px(240.0), px(170.0))),
            Hover::Waiting
        );
        assert_eq!(
            menu.hover(1, 1, point(px(280.0), px(170.0))),
            Hover::Changed
        );
        assert!(!menu.settle(), "reaching the submenu cancels the switch");
        assert_eq!(highlights(&menu), [Some(1), Some(1)]);
    }

    #[test]
    fn keyboard_moves_cancel_a_pending_pointer_switch() {
        let parent = Bounds::new(point(px(100.0), px(100.0)), size(px(180.0), px(120.0)));
        let submenu = Bounds::new(point(px(276.0), px(120.0)), size(px(180.0), px(90.0)));
        let keys: [fn(&mut Menu) -> bool; 3] = [Menu::open_submenu, Menu::close_submenu, |menu| {
            menu.move_highlight(true)
        }];
        for key in keys {
            let mut menu = nested();
            *menu.panels.borrow_mut() = vec![parent, submenu];
            menu.hover(0, 1, point(px(200.0), px(135.0)));
            assert_eq!(
                menu.hover(0, 3, point(px(240.0), px(170.0))),
                Hover::Waiting
            );
            key(&mut menu);
            let moved = highlights(&menu);
            assert!(!menu.settle());
            assert_eq!(highlights(&menu), moved);
        }
    }

    #[test]
    fn aiming_follows_the_side_the_submenu_opened_on() {
        let parent = Bounds::new(point(px(400.0), px(100.0)), size(px(180.0), px(200.0)));
        let right = Bounds::new(point(px(576.0), px(120.0)), size(px(160.0), px(100.0)));
        let left = Bounds::new(point(px(244.0), px(120.0)), size(px(160.0), px(100.0)));
        let from = point(px(500.0), px(130.0));
        assert!(heading_into(
            from,
            point(px(520.0), px(150.0)),
            parent,
            right
        ));
        assert!(!heading_into(
            from,
            point(px(520.0), px(260.0)),
            parent,
            right
        ));
        assert!(!heading_into(from, from, parent, right));
        assert!(heading_into(
            from,
            point(px(480.0), px(140.0)),
            parent,
            left
        ));
        assert!(!heading_into(
            from,
            point(px(520.0), px(150.0)),
            parent,
            left
        ));
    }

    #[test]
    fn submenus_open_beside_their_row_and_flip_or_clamp_at_window_edges() {
        let m = Metrics::new(1.0);
        let viewport = size(px(800.0), px(600.0));
        let panel = size(px(180.0), px(100.0));
        let parent = Bounds::new(point(px(100.0), px(100.0)), size(px(180.0), px(200.0)));
        let origin = submenu_origin(parent, px(150.0), panel, viewport, m);
        assert_eq!(origin, point(px(276.0), px(145.0)));
        let parent = Bounds::new(point(px(560.0), px(100.0)), size(px(180.0), px(200.0)));
        let origin = submenu_origin(parent, px(150.0), panel, viewport, m);
        assert_eq!(origin.x, px(384.0));
        let origin = submenu_origin(parent, px(580.0), panel, viewport, m);
        assert_eq!(origin.y, px(492.0));
    }
}
