use gpui::{AnyElement, AppContext, Context, Focusable, IntoElement, Window, point, px};
use muxy_app_core::{Project, ProjectId, ProjectStatus};
use muxy_protocol::ProgressState;
use muxy_ui::icon::Icon;
use muxy_ui::l10n::translate;
use muxy_ui::picker::{
    Picker, PickerAction, PickerConfig, PickerEvent, PickerItem, PickerLeading, PickerRow,
    PickerStatus,
};
use muxy_ui::tr;

use super::overlays::{Mark, Overlay, OverlayView};
use super::project_editor::Field;
use super::tab_activity::{self, Status};
use crate::model::AppModel;

const NEW_WORKTREE: &str = "new-worktree";
const RENAME: &str = "rename";
const REMOVE: &str = "remove";

pub(crate) struct WorktreePicker {
    pub(crate) project: ProjectId,
    pub(crate) picker: gpui::Entity<Picker>,
}

impl OverlayView for WorktreePicker {
    fn render(
        &self,
        _: Option<Mark>,
        _: &AppModel,
        _: &Window,
        _: &mut Context<AppModel>,
    ) -> AnyElement {
        self.picker.clone().into_any_element()
    }
}

impl AppModel {
    pub(crate) fn worktree_switcher_project(&self) -> Option<ProjectId> {
        let current = self.state.current_project();
        let project = self
            .state
            .project(current.parent_id.unwrap_or(current.id))?;
        (!project.home
            && project.status() == ProjectStatus::Available
            && self.worktrees_visible(project.id))
        .then_some(project.id)
    }

    pub(crate) fn toggle_worktree_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Worktrees(_))) {
            self.dismiss_overlay(cx);
            return;
        }
        if self.dialogs.is_open() {
            return;
        }
        let Some(project) = self.worktree_switcher_project() else {
            return;
        };
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig::new("worktree-switcher", tr!("Search worktrees…")),
                self.theme.clone(),
                self.metrics,
                cx,
            )
        });
        picker.focus_handle(cx).focus(window);
        self.overlay_subscription =
            Some(
                cx.subscribe_in(&picker, window, |model, _, event, window, cx| match event {
                    PickerEvent::Confirmed(selection) => {
                        model.choose_worktree(selection.id.as_ref(), None, window, cx);
                    }
                    PickerEvent::RowAction { row, action } => {
                        model.choose_worktree(row.as_ref(), Some(action.as_ref()), window, cx);
                    }
                    PickerEvent::FooterAction(action) if action.as_ref() == NEW_WORKTREE => {
                        if let Some(Overlay::Worktrees(picker)) = &model.overlay {
                            let project = picker.project;
                            model.dismiss_overlay(cx);
                            model.open_git_form(project, true, cx);
                            model.focus_git_form(window, cx);
                        }
                    }
                    PickerEvent::QueryChanged { .. } => model.update_worktree_switcher(cx),
                    PickerEvent::Dismissed => model.dismiss_overlay(cx),
                    _ => {}
                }),
            );
        self.open_overlay(Overlay::Worktrees(WorktreePicker { project, picker }), cx);
        self.update_worktree_switcher(cx);
        let current = self.state.current_project().id.to_string();
        if let Some(Overlay::Worktrees(switcher)) = &self.overlay {
            switcher.picker.update(cx, |picker, cx| {
                let _ = picker.select_row(&current, cx);
            });
        }
    }

    pub(crate) fn update_worktree_switcher(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::Worktrees(switcher)) = &self.overlay else {
            return;
        };
        let parent = switcher.project;
        if self.worktree_switcher_project() != Some(parent) {
            self.dismiss_overlay(cx);
            return;
        }
        let Some(project) = self.state.project(parent) else {
            return;
        };
        let query = switcher.picker.read(cx).query().to_lowercase();
        let current = self.state.current_project().id;
        let mut items: Vec<_> = self
            .worktree_switcher_order(project)
            .into_iter()
            .filter(|(project, _)| {
                query.is_empty()
                    || format!("{} {}", project.name, self.project_path_label(project))
                        .to_lowercase()
                        .contains(&query)
            })
            .map(|(project, primary)| {
                PickerItem::Row(self.worktree_row(project, primary, project.id == current))
            })
            .collect();
        items.extend(
            self.worktree_creations(parent)
                .filter(|creation| {
                    query.is_empty() || creation.name.to_lowercase().contains(&query)
                })
                .enumerate()
                .map(|(index, creation)| {
                    let mut row =
                        PickerRow::new(format!("creating-{index}"), creation.name.clone());
                    row.leading = Some(PickerLeading::Icon(Icon::GitBranch));
                    row.trailing = Some(translate(tab_activity::CREATING));
                    row.disabled = true;
                    PickerItem::Row(row)
                }),
        );
        let status = if items.is_empty() {
            PickerStatus::Empty(tr!("No matching worktrees"))
        } else {
            PickerStatus::Ready
        };
        let footer = vec![
            PickerAction::new(NEW_WORKTREE, tr!("New Worktree…"))
                .icon(PickerLeading::Icon(Icon::Plus))
                .disabled(self.new_worktree_target().is_none()),
        ];
        switcher.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
            picker.set_footer_actions(footer, cx);
        });
    }

    pub(crate) fn worktree_switcher_order<'a>(
        &'a self,
        project: &'a Project,
    ) -> Vec<(&'a Project, bool)> {
        let mut rows: Vec<_> = std::iter::once((project, true))
            .chain(
                self.worktree_children(project.id)
                    .into_iter()
                    .map(|child| (child, false)),
            )
            .collect();
        if self.appearance.worktree_recent_order {
            let current = self.state.current_project().id;
            let recent = &self.appearance.worktree_recent;
            rows.sort_by_key(|(row, _)| {
                if row.id == current {
                    0
                } else {
                    recent
                        .iter()
                        .position(|id| *id == row.id)
                        .map_or(usize::MAX, |index| index + 1)
                }
            });
        }
        rows
    }

    fn worktree_row(&self, project: &Project, primary: bool, current: bool) -> PickerRow {
        let id = project.id;
        let missing = project.status() != ProjectStatus::Available;
        let removing = self.worktree_removing(id);
        let mut row = PickerRow::new(id.to_string(), project.name.clone());
        row.detail = Some(self.project_path_label(project).into());
        row.leading = Some(PickerLeading::Icon(if primary {
            Icon::Folder
        } else {
            Icon::GitBranch
        }));
        row.badge = primary.then(|| tr!("PRIMARY"));
        row.current = current;
        row.trailing = match tab_activity::worktree_status(id, self) {
            Status::Busy(key) => Some(translate(key)),
            Status::Blocked => Some(tr!("Blocked")),
            Status::Progress(progress) => Some(match progress.state {
                ProgressState::Error => tr!("Work reported an error."),
                ProgressState::Paused => tr!("Work is paused."),
                ProgressState::Running
                | ProgressState::Indeterminate
                | ProgressState::Unrecognized(_) => tr!("Working"),
            }),
            Status::Unread(_) | Status::Completed => Some(tr!("Finished working")),
            Status::None => None,
        };
        row.disabled = removing || (missing && primary);
        if !primary {
            row.actions = vec![
                PickerAction::new(RENAME, tr!("Rename…")).disabled(missing || removing),
                PickerAction::new(REMOVE, tr!("Remove…"))
                    .destructive(true)
                    .disabled(removing),
            ];
        }
        row
    }

    fn choose_worktree(
        &mut self,
        row: &str,
        action: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::Worktrees(switcher)) = &self.overlay else {
            return;
        };
        let parent = switcher.project;
        let Some(project) = row
            .parse::<ProjectId>()
            .ok()
            .and_then(|id| self.state.project(id))
            .filter(|project| project.id == parent || project.parent_id == Some(parent))
        else {
            return;
        };
        let id = project.id;
        let available = project.status() == ProjectStatus::Available;
        if action.is_none() && !available {
            return;
        }
        self.dismiss_overlay(cx);
        match action {
            None => {
                self.select_project(id, cx);
                self.focus_active(window, cx);
            }
            Some(RENAME) => {
                let size = window.viewport_size();
                let position = point(size.width / 2.0 - px(120.0), size.height / 3.0);
                self.open_project_editor(id, Field::Name, position, window, cx);
            }
            Some(REMOVE) if available => {
                self.git_request(id, muxy_protocol::GitAction::InspectRemoval, cx);
            }
            Some(REMOVE) => self.confirm_remove_project(id, cx),
            Some(_) => {}
        }
    }
}
