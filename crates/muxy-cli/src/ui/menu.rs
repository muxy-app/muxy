//! Right-click menus for terminals and tabs.

use muxy_app_core::PaneId;
use ratatui::{
    Frame,
    layout::{Position, Rect},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use super::{Target, Ui, theme};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    SplitRight(PaneId),
    SplitDown(PaneId),
    Zoom(PaneId),
    Copy,
    ScrollBack(PaneId),
    /// Gives the right-click the menu took to the app in the pane.
    RightClick(PaneId, Position),
    ClosePane(PaneId),
    NewTab,
    /// Commands for the tab holding the pane.
    RenameTab(PaneId),
    MoveTab(PaneId, bool),
    CloseTab(PaneId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Item {
    pub label: String,
    /// The keys that do the same, as a hint.
    pub keys: &'static str,
    pub command: Command,
    pub danger: bool,
}

impl Item {
    pub(crate) fn new(label: impl Into<String>, keys: &'static str, command: Command) -> Self {
        Self {
            label: label.into(),
            keys,
            command,
            danger: false,
        }
    }

    pub(crate) fn danger(self) -> Self {
        Self {
            danger: true,
            ..self
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Menu {
    /// Where it was opened.
    pub at: Position,
    /// Items, with `None` for a separator.
    pub items: Vec<Option<Item>>,
    pub selected: Option<usize>,
}

impl Menu {
    pub(crate) fn new(at: Position, items: Vec<Option<Item>>) -> Self {
        Self {
            at,
            items,
            selected: None,
        }
    }

    /// Moves the selection to the next or previous item, past separators.
    pub(crate) fn step(&mut self, forward: bool) {
        let count = self.items.len();
        let mut index = self.selected.unwrap_or(if forward { count - 1 } else { 0 });
        for _ in 0..count {
            index = if forward {
                (index + 1) % count
            } else {
                (index + count - 1) % count
            };
            if self.items[index].is_some() {
                self.selected = Some(index);
                return;
            }
        }
    }

    pub(crate) fn command(&self, index: usize) -> Option<Command> {
        self.items
            .get(index)
            .and_then(Option::as_ref)
            .map(|item| item.command)
    }
}

pub(super) fn draw(frame: &mut Frame<'_>, ui: &mut Ui, menu: &Menu) {
    let area = frame.area();
    let label_width = menu
        .items
        .iter()
        .flatten()
        .map(|item| Span::raw(&item.label).width() + Span::raw(item.keys).width() + 3)
        .max()
        .unwrap_or(0);
    let width = u16::try_from(label_width + 4)
        .unwrap_or(u16::MAX)
        .max(16)
        .min(area.width);
    let height = u16::try_from(menu.items.len() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height);
    let rect = Rect::new(
        menu.at.x.min(area.right().saturating_sub(width)),
        menu.at.y.min(area.bottom().saturating_sub(height)),
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    let block = Block::bordered().border_style(theme::accent());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    ui.hits.push((rect, Target::Panel));
    for (index, item) in menu.items.iter().enumerate() {
        let Some(y) = u16::try_from(index)
            .ok()
            .map(|offset| inner.y + offset)
            .filter(|y| *y < inner.bottom())
        else {
            break;
        };
        let row = Rect::new(inner.x, y, inner.width, 1);
        let Some(item) = item else {
            frame.render_widget(
                Paragraph::new("─".repeat(usize::from(row.width))).style(theme::line()),
                row,
            );
            continue;
        };
        let label = format!(" {}", item.label);
        let gap = usize::from(row.width)
            .saturating_sub(Span::raw(&label).width() + Span::raw(item.keys).width() + 1);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    label,
                    if item.danger {
                        theme::danger()
                    } else {
                        theme::text()
                    },
                ),
                Span::raw(" ".repeat(gap)),
                Span::styled(item.keys, theme::muted()),
            ])),
            row,
        );
        if menu.selected == Some(index) {
            frame.buffer_mut().set_style(
                row,
                if item.danger {
                    theme::danger_pill()
                } else {
                    theme::pill()
                },
            );
        }
        ui.hits.push((row, Target::Item(index)));
    }
}
