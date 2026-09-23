mod ai;
mod form;
mod pr;
pub(crate) use ai::{ADDITIONAL_PROMPT_LIMIT, AiReview, AiSheet, AiStep, render_sheet};
pub(crate) use pr::{PullRequestPopover, render_pr};

use super::overlays::Overlay;
use crate::model::{AppModel, git::Repository};
use gpui::{AppContext, Context, Entity, Focusable, Window};
use muxy_protocol::{
    GitAction, OperationId, ProjectId, ServerPath, WorktreeAction, WorktreeIntent,
};
use muxy_ui::picker::{
    Picker, PickerAction, PickerConfig, PickerEvent, PickerItem, PickerRow, PickerSelectionStyle,
    PickerStatus,
};
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Branches,
    Changes,
}
impl Kind {
    pub(crate) fn action(self) -> GitAction {
        match self {
            Self::Branches => GitAction::Branches,
            Self::Changes => GitAction::Changes,
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Branches => "Branches",
            Self::Changes => "Changes",
        }
    }
}
pub(crate) struct GitPicker {
    pub(crate) project: ProjectId,
    pub(crate) kind: Kind,
    pub(crate) picker: Entity<Picker>,
    pub(crate) anchor: muxy_ui::popover::PopoverAnchor,
    selected: Vec<ServerPath>,
}
pub(crate) struct Form {
    project: ProjectId,
    worktree: bool,
    existing: bool,
    chooser: Option<Entity<Picker>>,
    branch: Entity<TextInput>,
    directory: Entity<TextInput>,
    base: Entity<TextInput>,
    subscriptions: Vec<gpui::Subscription>,
    error: Option<String>,
    suggested_directory: String,
}
impl AppModel {
    pub(crate) fn open_git_picker(
        &mut self,
        project: ProjectId,
        kind: Kind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(&self.overlay, Some(Overlay::Git(picker)) if picker.project == project && picker.kind == kind)
        {
            self.dismiss_overlay(cx);
            return;
        }
        self.git.interaction = self.git.interaction.wrapping_add(1);
        let anchor = match kind {
            Kind::Branches => self.git.branch_anchor.clone(),
            Kind::Changes => self.git.changes_anchor.clone(),
        };
        let width = match kind {
            Kind::Branches => 440.0,
            Kind::Changes => 400.0,
        };
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig {
                    width: Some(width),
                    ..PickerConfig::popover(
                        "git-picker",
                        format!("Search {}…", kind.title().to_lowercase()),
                    )
                },
                self.theme.clone(),
                self.metrics,
                cx,
            )
        });
        picker.focus_handle(cx).focus(window);
        self.overlay_subscription =
            Some(cx.subscribe(&picker, |model, _, event, cx| match event {
                PickerEvent::Dismissed => model.dismiss_overlay(cx),
                PickerEvent::QueryChanged { .. } => model.update_git_picker(cx),
                PickerEvent::Confirmed(selection) => {
                    model.git_selection(selection.id.as_ref(), None, cx);
                }
                PickerEvent::RowAction { row, action } => {
                    model.git_selection(row.as_ref(), Some(action.as_ref()), cx);
                }
                PickerEvent::FooterAction(action) => model.git_footer(action.as_ref(), cx),
                _ => (),
            }));
        self.overlay = Some(Overlay::Git(GitPicker {
            project,
            kind,
            picker,
            anchor,
            selected: vec![],
        }));
        self.queue_git_refresh(project, vec![GitAction::Summary, kind.action()], cx);
        self.update_git_picker(cx);
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Render the Git picker contents together"
    )]
    pub(crate) fn update_git_picker(&self, cx: &mut Context<Self>) {
        let Some(Overlay::Git(picker)) = &self.overlay else {
            return;
        };
        let repository = self.git.projects.get(&picker.project);
        let query = picker.picker.read(cx).query().to_lowercase();
        let busy = !self.session_listing_ready() || repository.is_some_and(Repository::busy);
        let mut items = Vec::new();
        if let Some(repository) = repository {
            match picker.kind {
                Kind::Branches => {
                    for branch in &repository.branches {
                        let mut row = PickerRow::new(branch.name.clone(), branch.name.clone());
                        row.current = branch.current;
                        row.trailing = branch.checked_out.then(|| "Checked out".into());
                        if !branch.checked_out {
                            row.actions.push(
                                PickerAction::new("delete", "Delete")
                                    .destructive(true)
                                    .disabled(busy),
                            );
                        }
                        row.disabled = busy;
                        items.push(row);
                    }
                }
                Kind::Changes => {
                    for file in &repository.files {
                        let mut row = PickerRow::new(
                            path_key(&file.path),
                            String::from_utf8_lossy(&file.path.0).into_owned(),
                        );
                        row.selected = picker.selected.contains(&file.path);
                        row.selection_style = PickerSelectionStyle::Highlight;
                        row.detail = Some(
                            if file.conflicted() {
                                "Conflict"
                            } else if file.untracked() {
                                "Untracked"
                            } else if file.staged() && file.unstaged() {
                                "Staged and unstaged"
                            } else if file.staged() {
                                "Staged"
                            } else {
                                "Unstaged"
                            }
                            .into(),
                        );
                        row.trailing = file
                            .added
                            .zip(file.removed)
                            .map(|(a, d)| format!("+{a} −{d}").into());
                        if file.unstaged() || file.untracked() {
                            row.actions
                                .push(PickerAction::new("stage", "Stage").disabled(busy));
                        }
                        if file.staged() {
                            row.actions
                                .push(PickerAction::new("unstage", "Unstage").disabled(busy));
                        }
                        if !file.conflicted() && (file.unstaged() || file.untracked()) {
                            row.actions.push(
                                PickerAction::new("discard", "Discard")
                                    .destructive(true)
                                    .disabled(busy),
                            );
                        }
                        row.disabled = busy;
                        items.push(row);
                    }
                }
            }
        }
        let items = picker_items(items, &query, picker.kind == Kind::Changes);
        let mut actions = vec![PickerAction::new("refresh", "Refresh").disabled(busy)];
        match picker.kind {
            Kind::Branches => {
                actions.push(PickerAction::new("create", "New Branch…").disabled(busy));
            }
            Kind::Changes => {
                let selected = !picker.selected.is_empty();
                actions.push(
                    PickerAction::new(
                        "stage",
                        if selected {
                            "Stage Selected"
                        } else {
                            "Stage All"
                        },
                    )
                    .disabled(busy),
                );
                actions.push(
                    PickerAction::new(
                        "unstage",
                        if selected {
                            "Unstage Selected"
                        } else {
                            "Unstage All"
                        },
                    )
                    .disabled(busy),
                );
            }
        }
        let status = if !self.session_listing_ready() {
            PickerStatus::Error("Reconnect to use Git".into())
        } else if let Some(error) = repository.and_then(|r| r.load_error(&picker.kind.action())) {
            PickerStatus::Error(error.clone().into())
        } else if repository.is_none_or(|r| !r.has_loaded(&picker.kind.action()))
            && items.is_empty()
        {
            PickerStatus::Loading("Loading…".into())
        } else if items.is_empty() {
            PickerStatus::Empty("No matches".into())
        } else {
            PickerStatus::Ready
        };
        picker.picker.update(cx, |view, cx| {
            view.set_items(items, cx);
            view.set_footer_actions(actions, cx);
            view.set_status(status, cx);
        });
    }

    fn git_selection(&mut self, row: &str, action: Option<&str>, cx: &mut Context<Self>) {
        let Some(Overlay::Git(picker)) = &mut self.overlay else {
            return;
        };
        let project = picker.project;
        let Some(repository) = self.git.projects.get(&project).filter(|r| !r.busy()) else {
            return;
        };
        let operation = match picker.kind {
            Kind::Branches => {
                let Some(branch) = repository.branches.iter().find(|branch| branch.name == row)
                else {
                    return;
                };
                if action == Some("delete") {
                    GitAction::DeleteBranch(branch.name.clone())
                } else if branch.current {
                    return;
                } else {
                    GitAction::SwitchBranch(branch.name.clone())
                }
            }
            Kind::Changes => {
                let Some(file) = repository
                    .files
                    .iter()
                    .find(|file| path_key(&file.path) == row)
                else {
                    return;
                };
                let paths = vec![file.path.clone()];
                match action {
                    Some("stage") => GitAction::Stage(paths),
                    Some("unstage") => GitAction::Unstage(paths),
                    Some("discard") => GitAction::Discard(paths),
                    _ => {
                        if picker.selected.contains(&file.path) {
                            picker.selected.retain(|p| *p != file.path);
                        } else {
                            picker.selected.push(file.path.clone());
                        }
                        self.update_git_picker(cx);
                        return;
                    }
                }
            }
        };
        match &operation {
            GitAction::DeleteBranch(branch) => self.confirm_git_action(project, operation.clone(), format!("Permanently delete branch “{branch}”? Unmerged commits may become unreachable."), cx),
            GitAction::Discard(paths) => self.confirm_git_action(project, operation.clone(), format!("Discard changes to “{}”? Untracked files will be permanently deleted; staged changes are preserved.", String::from_utf8_lossy(&paths[0].0)), cx),
            _ => self.git_request(project, operation, cx),
        }
    }

    fn git_footer(&mut self, action: &str, cx: &mut Context<Self>) {
        let Some(Overlay::Git(picker)) = &self.overlay else {
            return;
        };
        let project = picker.project;
        if action == "refresh" {
            self.queue_git_refresh(project, vec![GitAction::Summary, picker.kind.action()], cx);
            return;
        }
        if action == "create" {
            self.open_git_form(project, false, cx);
            return;
        }
        let Some(repository) = self.git.projects.get(&project) else {
            return;
        };
        let paths: Vec<_> = repository
            .files
            .iter()
            .filter(|file| picker.selected.is_empty() || picker.selected.contains(&file.path))
            .filter(|f| {
                if action == "stage" {
                    f.unstaged() || f.untracked()
                } else {
                    f.staged()
                }
            })
            .map(|f| f.path.clone())
            .collect();
        if paths.is_empty() {
            return;
        }
        self.git_request(
            project,
            if action == "stage" {
                GitAction::Stage(paths)
            } else {
                GitAction::Unstage(paths)
            },
            cx,
        );
    }

    pub(crate) fn confirm_git_action(
        &mut self,
        project: ProjectId,
        action: GitAction,
        message: String,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let window = self.window;
        let context = self.git.interaction;
        self.close_prompt = Some(cx.spawn(async move |this, cx| {
            let (send, receive) = async_channel::bounded(1);
            let dialog = window.update(cx, |_, window, _| {
                muxy_ui::dialog::confirm(
                    window,
                    "Confirm Git Operation",
                    &message,
                    "Confirm",
                    None,
                    move |answer| {
                        let _ = send.try_send(answer);
                    },
                )
            });
            let confirmed = if let Ok(Ok(_dialog)) = dialog {
                matches!(
                    receive.recv().await,
                    Ok(muxy_ui::dialog::ConfirmationResponse::Confirmed { .. })
                )
            } else {
                false
            };
            let _ = this.update(cx, |model, cx| {
                model.close_prompt = None;
                if confirmed
                    && model.git.interaction == context
                    && model.state.project(project).is_some()
                {
                    model.git_request(project, action, cx);
                }
            });
        }));
    }

    pub(crate) fn open_git_form(
        &mut self,
        project: ProjectId,
        worktree: bool,
        cx: &mut Context<Self>,
    ) {
        self.git.interaction = self.git.interaction.wrapping_add(1);
        let Some(record) = self.state.project(project) else {
            return;
        };
        let default = self
            .git
            .projects
            .get(&project)
            .and_then(|r| {
                r.branches
                    .iter()
                    .find(|b| b.default)
                    .or_else(|| r.branches.iter().find(|b| b.current))
            })
            .map_or_else(|| "HEAD".into(), |b| b.name.clone());
        let directory = record
            .directory
            .parent()
            .unwrap_or(&record.directory)
            .join(format!("{}-worktree", record.name));
        let suggested_directory = directory.to_string_lossy().into_owned();
        let branch = cx.new(|cx| {
            TextInput::new(InputStyle::field(&self.theme, &self.metrics), cx)
                .with_placeholder("feature-x")
        });
        let directory = cx.new(|cx| {
            TextInput::new(InputStyle::field(&self.theme, &self.metrics), cx)
                .with_text(directory.to_string_lossy().into_owned())
        });
        let base = cx.new(|cx| {
            TextInput::new(InputStyle::field(&self.theme, &self.metrics), cx).with_text(default)
        });
        let mut subscriptions = Vec::new();
        for input in [&branch, &directory, &base] {
            subscriptions.push(cx.subscribe(input, |model, _, event, cx| match event {
                InputEvent::Submitted => model.submit_git_form(cx),
                InputEvent::Cancelled => model.dismiss_overlay(cx),
                InputEvent::Changed => model.git_form_changed(cx),
            }));
        }
        let focus = branch.focus_handle(cx);
        let _ = self.window.update(cx, |_, window, _| focus.focus(window));
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::GitForm(Form {
            project,
            worktree,
            existing: false,
            chooser: None,
            branch,
            directory,
            base,
            subscriptions,
            error: None,
            suggested_directory,
        }));
        if worktree {
            self.git_request(project, GitAction::Branches, cx);
        }
        cx.notify();
    }

    fn set_worktree_branch_mode(&mut self, existing: bool, cx: &mut Context<Self>) {
        if let Some(Overlay::GitForm(form)) = &mut self.overlay {
            form.existing = existing;
            form.error = None;
            form.chooser = None;
        }
        cx.notify();
    }

    fn git_form_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::GitForm(form)) = &mut self.overlay {
            form.error = None;
            if form.worktree && form.directory.read(cx).text() == form.suggested_directory {
                let branch = form.branch.read(cx).text().trim();
                if let Some(project) = self.state.project(form.project) {
                    let suffix = if branch.is_empty() {
                        "worktree".to_owned()
                    } else {
                        branch.replace(['/', '\\'], "-")
                    };
                    let path = project
                        .directory
                        .parent()
                        .unwrap_or(&project.directory)
                        .join(format!("{}-{suffix}", project.name))
                        .to_string_lossy()
                        .into_owned();
                    if path != form.suggested_directory {
                        form.suggested_directory.clone_from(&path);
                        form.directory
                            .update(cx, |input, cx| input.set_text(path, cx));
                    }
                }
            }
        }
        cx.notify();
    }

    fn choose_worktree_branch(&mut self, base: bool, cx: &mut Context<Self>) {
        let Some(Overlay::GitForm(form)) = &self.overlay else {
            return;
        };
        let project = form.project;
        let chooser = cx.new(|cx| {
            Picker::new(
                PickerConfig {
                    width: Some(450.0),
                    ..PickerConfig::popover("worktree-branch", "Search branches…")
                },
                self.theme.clone(),
                self.metrics,
                cx,
            )
        });
        let target = if base {
            form.base.clone()
        } else {
            form.branch.clone()
        };
        let return_focus = if base {
            form.branch.focus_handle(cx)
        } else {
            form.directory.focus_handle(cx)
        };
        let subscription = cx.subscribe(&chooser, move |model, chooser, event, cx| {
            match event {
                PickerEvent::Confirmed(selection) => {
                    target.update(cx, |input, cx| input.set_text(selection.id.clone(), cx));
                    if let Some(Overlay::GitForm(form)) = &mut model.overlay {
                        form.chooser = None;
                    }
                    model.git_form_changed(cx);
                    let _ = model
                        .window
                        .update(cx, |_, window, _| return_focus.focus(window));
                }
                PickerEvent::Dismissed => {
                    if let Some(Overlay::GitForm(form)) = &mut model.overlay {
                        form.chooser = None;
                    }
                    let _ = model
                        .window
                        .update(cx, |_, window, _| return_focus.focus(window));
                }
                PickerEvent::QueryChanged { query, .. } => {
                    model.worktree_branch_choices(project, &chooser, query.as_ref(), base, cx);
                }
                _ => (),
            }
            cx.notify();
        });
        self.worktree_branch_choices(project, &chooser, "", base, cx);
        let focus = chooser.focus_handle(cx);
        let _ = self.window.update(cx, |_, window, _| focus.focus(window));
        if let Some(Overlay::GitForm(form)) = &mut self.overlay {
            form.chooser = Some(chooser);
            form.subscriptions.push(subscription);
        }
        cx.notify();
    }

    fn worktree_branch_choices(
        &self,
        project: ProjectId,
        chooser: &Entity<Picker>,
        query: &str,
        base: bool,
        cx: &mut Context<Self>,
    ) {
        let query = query.to_lowercase();
        let items = self
            .git
            .projects
            .get(&project)
            .map(|r| {
                r.branches
                    .iter()
                    .filter(|b| (base || !b.checked_out) && b.name.to_lowercase().contains(&query))
                    .map(|b| PickerItem::Row(PickerRow::new(b.name.clone(), b.name.clone())))
                    .collect()
            })
            .unwrap_or_default();
        chooser.update(cx, |picker, cx| picker.set_items(items, cx));
    }

    pub(crate) fn update_worktree_default(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(Overlay::GitForm(form)) = &self.overlay else {
            return;
        };
        if form.project != project {
            return;
        }
        if let Some(chooser) = &form.chooser {
            let query = chooser.read(cx).query().to_string();
            self.worktree_branch_choices(project, chooser, &query, !form.existing, cx);
        }
        if form.base.read(cx).text() != "HEAD" {
            return;
        }
        if let Some(branch) = self.git.projects.get(&project).and_then(|r| {
            r.branches
                .iter()
                .find(|b| b.default)
                .or_else(|| r.branches.iter().find(|b| b.current))
        }) {
            form.base
                .update(cx, |input, cx| input.set_text(branch.name.clone(), cx));
        }
    }

    fn submit_git_form(&mut self, cx: &mut Context<Self>) {
        use std::os::unix::ffi::OsStrExt;
        let Some(Overlay::GitForm(form)) = &self.overlay else {
            return;
        };
        if !self.session_listing_ready()
            || self
                .git
                .projects
                .get(&form.project)
                .is_some_and(Repository::busy)
        {
            return;
        }
        let branch = form.branch.read(cx).text().trim().to_owned();
        let error = if branch.is_empty() {
            Some("Enter a branch name.")
        } else if form.worktree && form.directory.read(cx).text().trim().is_empty() {
            Some("Choose a location for the worktree.")
        } else {
            None
        };
        if let Some(error) = error {
            if let Some(Overlay::GitForm(form)) = &mut self.overlay {
                form.error = Some(error.into());
            }
            cx.notify();
            return;
        }
        let project = form.project;
        let action = if form.worktree {
            let directory = form.directory.read(cx).text().trim().to_owned();
            let base = form.base.read(cx).text().trim().to_owned();
            GitAction::Worktree(WorktreeIntent {
                operation: OperationId::new(),
                action: WorktreeAction::Create {
                    project: ProjectId::new(),
                    directory: ServerPath(
                        std::path::Path::new(&directory)
                            .as_os_str()
                            .as_bytes()
                            .to_vec(),
                    ),
                    branch,
                    base: (!form.existing).then_some(if base.is_empty() {
                        "HEAD".into()
                    } else {
                        base
                    }),
                },
            })
        } else {
            GitAction::CreateBranch(branch)
        };
        self.git_request(project, action, cx);
    }
}

pub(crate) use form::render as render_form;

fn picker_items(mut rows: Vec<PickerRow>, query: &str, group_by_status: bool) -> Vec<PickerItem> {
    rows.retain(|row| {
        format!(
            "{} {}",
            row.title,
            row.detail.as_ref().map_or("", |s| s.as_ref())
        )
        .to_lowercase()
        .contains(query)
    });
    if !group_by_status {
        return rows.into_iter().map(PickerItem::Row).collect();
    }
    rows.sort_by(|a, b| a.detail.cmp(&b.detail));
    let mut items = Vec::new();
    let mut section = None;
    for mut row in rows {
        let status = row.detail.take();
        if status != section {
            if let Some(label) = &status {
                items.push(PickerItem::section(label.clone()));
            }
            section = status;
        }
        items.push(PickerItem::Row(row));
    }
    items
}

fn path_key(path: &ServerPath) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    path.0
        .iter()
        .flat_map(|byte| {
            [
                char::from(HEX[usize::from(byte >> 4)]),
                char::from(HEX[usize::from(byte & 15)]),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_are_grouped_by_searchable_status() {
        let rows: Vec<_> = [
            ("worktree.rs", "Unstaged"),
            ("index.rs", "Staged"),
            ("both.rs", "Staged and unstaged"),
            ("other.rs", "Unstaged"),
            ("conflict.rs", "Conflict"),
            ("new.rs", "Untracked"),
        ]
        .into_iter()
        .map(|(path, status)| {
            let mut row = PickerRow::new(path, path);
            row.detail = Some(status.into());
            row
        })
        .collect();
        let items = picker_items(rows.clone(), "", true);
        let mut group = "";
        let mut grouped_files = Vec::new();
        for item in &items {
            match item {
                PickerItem::Section(label) => group = label.as_ref(),
                PickerItem::Row(row) => {
                    grouped_files.push((group, row.id.as_ref()));
                }
            }
        }
        assert_eq!(
            grouped_files,
            vec![
                ("Conflict", "conflict.rs"),
                ("Staged", "index.rs"),
                ("Staged and unstaged", "both.rs"),
                ("Unstaged", "worktree.rs"),
                ("Unstaged", "other.rs"),
                ("Untracked", "new.rs"),
            ]
        );
        let filtered = picker_items(rows.clone(), "other", true);
        assert_eq!(
            filtered,
            vec![PickerItem::section("Unstaged"), PickerItem::row("other.rs"),]
        );
        assert_eq!(
            picker_items(rows.clone(), "untracked", true),
            vec![PickerItem::section("Untracked"), PickerItem::row("new.rs"),]
        );
        assert!(picker_items(rows, "missing", true).is_empty());
    }
}
