use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use muxy_api::worktree_config::{ProjectHookApproval, ResolvedCommand};
use muxy_api::worktree_hooks::SetupPolicy;
use muxy_api::worktree_lifecycle::CreateWorktreeRequest;
use muxy_api::worktree_location::{
    LocationContext, WorktreeLocationRequest, resolve, sanitize_component, validate_template,
};
use muxy_core::store::Project;
use muxy_ui::command_popover::{
    CommandPopover, CommandPopoverConfig, CommandPopoverDensity, CommandPopoverEvent,
    CommandPopoverItem, CommandPopoverLeading, CommandPopoverPresentation, CommandPopoverRow,
    CommandPopoverStatus, CommandPopoverTab,
};
use muxy_ui::icon::Icon;
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput, growing_input};
use muxy_ui::theme::{Metrics, Theme};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocationChoice {
    Default,
    Template(String),
    Folder(PathBuf),
}

pub enum CreateWorktreeEvent {
    Dismiss,
    ChooseFolder,
    Submit {
        request: Box<CreateWorktreeRequest>,
        path_template: Option<String>,
        parent_path: Option<String>,
    },
}

#[derive(Clone, Copy)]
enum ModalAction {
    Dismiss,
    ChooseFolder,
    Submit,
}

pub struct CreateWorktreeModal {
    form: CreateWorktreeForm,
    name: Entity<TextInput>,
    branch: Entity<TextInput>,
    base_branch_picker: Entity<CommandPopover>,
    existing_branch_picker: Entity<CommandPopover>,
    location: Entity<TextInput>,
    branches: Vec<String>,
    theme: Theme,
    metrics: Metrics,
    focus: FocusHandle,
    focused: bool,
    subscriptions: Vec<Subscription>,
}

impl EventEmitter<CreateWorktreeEvent> for CreateWorktreeModal {}

impl Focusable for CreateWorktreeModal {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CreateWorktreeModal {
    pub fn new(
        mut form: CreateWorktreeForm,
        branches: Vec<String>,
        current_branch: Option<String>,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let style = InputStyle::field(&theme, &metrics);
        let name = cx.new(|cx| TextInput::new(style, cx).with_placeholder("feature-x"));
        let branch = cx.new(|cx| TextInput::new(style, cx).with_placeholder("feature-x"));
        let default_branch = ["main", "master", "develop"]
            .into_iter()
            .find(|candidate| branches.iter().any(|branch| branch == candidate))
            .map(str::to_owned)
            .or(current_branch)
            .or_else(|| branches.first().cloned())
            .unwrap_or_default();
        form.set_base_branch(&default_branch);
        form.set_existing_branch(branches.first().map(String::as_str).unwrap_or_default());
        let base_branch_picker = branch_picker(
            "worktree-base-branch-picker",
            "Search base branches…",
            &branches,
            Some(&default_branch),
            theme.clone(),
            metrics,
            cx,
        );
        let existing_branch_picker = branch_picker(
            "worktree-existing-branch-picker",
            "Search branches…",
            &branches,
            branches.first(),
            theme.clone(),
            metrics,
            cx,
        );
        let location_value = match form.location() {
            LocationChoice::Default => String::new(),
            LocationChoice::Template(value) => value.clone(),
            LocationChoice::Folder(path) => path.to_string_lossy().into_owned(),
        };
        let location = cx.new(|cx| {
            TextInput::new(style, cx)
                .with_placeholder(muxy_api::worktree_location::SUGGESTED_PATH_TEMPLATE)
                .with_text(location_value)
        });
        let name_subscription = cx.subscribe(&name, |modal: &mut Self, input, event, cx| {
            match event {
                InputEvent::Changed => {
                    modal.form.set_name(input.read(cx).text());
                    let branch = modal.form.branch().to_owned();
                    modal.branch.update(cx, |input, cx| {
                        if input.text() != branch {
                            input.set_text(branch, cx);
                        }
                    });
                }
                InputEvent::Submitted => modal.submit(cx),
                InputEvent::Cancelled => modal.cancel(cx),
            }
            cx.notify();
        });
        let branch_subscription = cx.subscribe(&branch, |modal: &mut Self, input, event, cx| {
            match event {
                InputEvent::Changed => modal.form.set_branch(input.read(cx).text()),
                InputEvent::Submitted => modal.submit(cx),
                InputEvent::Cancelled => modal.cancel(cx),
            }
            cx.notify();
        });
        let base_subscription = cx
            .subscribe(&base_branch_picker, |modal: &mut Self, _, event, cx| {
                modal.handle_branch_picker(true, event, cx)
            });
        let existing_subscription = cx
            .subscribe(&existing_branch_picker, |modal: &mut Self, _, event, cx| {
                modal.handle_branch_picker(false, event, cx)
            });
        let location_subscription =
            cx.subscribe(&location, |modal: &mut Self, input, event, cx| {
                match event {
                    InputEvent::Changed => {
                        let value = input.read(cx).text().to_owned();
                        match modal.form.location() {
                            LocationChoice::Template(_) => {
                                modal.form.set_location(LocationChoice::Template(value))
                            }
                            LocationChoice::Folder(_) => modal
                                .form
                                .set_location(LocationChoice::Folder(PathBuf::from(value))),
                            LocationChoice::Default => {}
                        }
                    }
                    InputEvent::Submitted => modal.submit(cx),
                    InputEvent::Cancelled => modal.cancel(cx),
                }
                cx.notify();
            });
        Self {
            form,
            name,
            branch,
            base_branch_picker,
            existing_branch_picker,
            location,
            branches,
            theme,
            metrics,
            focus: cx.focus_handle(),
            focused: false,
            subscriptions: vec![
                name_subscription,
                branch_subscription,
                base_subscription,
                existing_subscription,
                location_subscription,
            ],
        }
    }

    pub fn dismissible(&self) -> bool {
        self.form.dismissible()
    }

    pub fn set_running(&mut self, value: bool, cx: &mut Context<Self>) {
        self.form.set_running(value);
        cx.notify();
    }

    pub fn set_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.form.set_running(false);
        self.form.set_error(Some(error));
        cx.notify();
    }

    pub fn set_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.form.set_location(LocationChoice::Folder(path.clone()));
        let value = path.to_string_lossy().into_owned();
        self.location
            .update(cx, |input, cx| input.set_text(value, cx));
        cx.notify();
    }

    fn preferred_location(&self) -> (Option<String>, Option<String>) {
        match self.form.location() {
            LocationChoice::Default => (None, None),
            LocationChoice::Template(value) => (Some(value.trim().to_owned()), None),
            LocationChoice::Folder(path) => (None, Some(path.to_string_lossy().trim().to_owned())),
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.form.can_create() {
            return;
        }
        let Ok(request) = self.form.request() else {
            return;
        };
        let (path_template, parent_path) = self.preferred_location();
        cx.emit(CreateWorktreeEvent::Submit {
            request: Box::new(request),
            path_template,
            parent_path,
        });
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.form.dismissible() {
            cx.emit(CreateWorktreeEvent::Dismiss);
        }
    }

    fn handle_branch_picker(
        &mut self,
        base: bool,
        event: &CommandPopoverEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            CommandPopoverEvent::QueryChanged { .. } => self.sync_branch_picker(base, cx),
            CommandPopoverEvent::Confirmed(selection)
            | CommandPopoverEvent::SecondaryConfirmed(selection) => {
                let Some(index) = selection
                    .id
                    .strip_prefix("worktree-branch-")
                    .and_then(|value| value.parse::<usize>().ok())
                else {
                    return;
                };
                let Some(branch) = self.branches.get(index) else {
                    return;
                };
                if base {
                    self.form.set_base_branch(branch);
                } else {
                    self.form.set_existing_branch(branch);
                }
                self.sync_branch_picker(base, cx);
                cx.notify();
            }
            CommandPopoverEvent::Dismissed => self.cancel(cx),
            _ => {}
        }
    }

    fn sync_branch_picker(&self, base: bool, cx: &mut Context<Self>) {
        let picker = if base {
            &self.base_branch_picker
        } else {
            &self.existing_branch_picker
        };
        let query = picker.read(cx).query().trim().to_lowercase();
        let selected = if base {
            self.form.base_branch()
        } else {
            self.form.existing_branch()
        };
        let items = branch_items(&self.branches, &query, selected, self.form.running());
        picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            if picker.query().is_empty()
                && let Some(index) = self.branches.iter().position(|branch| branch == selected)
            {
                let _ = picker.select_row(&format!("worktree-branch-{index}"), cx);
            }
            picker.set_status(
                if self.branches.is_empty() {
                    CommandPopoverStatus::Empty("No branches available".into())
                } else {
                    CommandPopoverStatus::Ready
                },
                cx,
            );
        });
    }

    fn field(&self, label: &str, input: &Entity<TextInput>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing2())
            .child(
                div()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.fg_muted)
                    .child(SharedString::from(label.to_owned())),
            )
            .child(
                div()
                    .h(self.metrics.control_medium())
                    .px(self.metrics.spacing3())
                    .flex()
                    .items_center()
                    .rounded(self.metrics.radius_sm())
                    .bg(self.theme.surface)
                    .border_1()
                    .border_color(self.theme.border)
                    .child(growing_input(input)),
            )
            .into_any_element()
    }

    fn branch_picker_field(&self, label: &str, picker: &Entity<CommandPopover>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing2())
            .child(
                div()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.fg_muted)
                    .child(SharedString::from(label.to_owned())),
            )
            .child(picker.clone())
            .into_any_element()
    }

    fn segment(
        &self,
        id: &'static str,
        label: &'static str,
        selected: bool,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .h(self.metrics.control_medium())
            .px(self.metrics.spacing4())
            .rounded(self.metrics.radius_sm())
            .text_size(self.metrics.font_footnote())
            .text_color(if selected {
                self.theme.accent_foreground
            } else {
                self.theme.fg
            })
            .when(selected, |button| button.bg(self.theme.accent))
            .when(!selected, |button| {
                button.hover(|style| style.bg(self.theme.hover))
            })
            .cursor_pointer()
            .on_click(cx.listener(move |modal, _, _, cx| {
                if modal.form.running() {
                    return;
                }
                on_click(modal, cx);
                cx.notify();
            }))
            .child(label)
            .into_any_element()
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        primary: bool,
        enabled: bool,
        action: Option<ModalAction>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .h(self.metrics.control_medium())
            .px(self.metrics.spacing5())
            .rounded(self.metrics.radius_sm())
            .text_size(self.metrics.font_footnote())
            .font_weight(FontWeight::MEDIUM)
            .text_color(if primary {
                self.theme.accent_foreground
            } else {
                self.theme.fg
            })
            .bg(if primary {
                self.theme.accent
            } else {
                self.theme.surface
            })
            .border_1()
            .border_color(if primary {
                self.theme.accent
            } else {
                self.theme.border
            })
            .when(!enabled, |button| button.opacity(0.45))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(self.theme.hover))
                    .on_click(cx.listener(move |modal, _, _, cx| match action {
                        Some(ModalAction::Submit) => {
                            modal.submit(cx);
                        }
                        Some(ModalAction::Dismiss) if modal.form.dismissible() => modal.cancel(cx),
                        Some(ModalAction::ChooseFolder) => {
                            cx.emit(CreateWorktreeEvent::ChooseFolder)
                        }
                        _ => {}
                    }))
            })
            .child(label)
            .into_any_element()
    }
}

impl Render for CreateWorktreeModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = self.subscriptions.len();
        if !self.focused {
            self.focused = true;
            window.focus(&self.name.focus_handle(cx));
        }
        let branch_modes = div()
            .flex()
            .flex_row()
            .gap(px(1.0))
            .p(px(1.0))
            .rounded(self.metrics.radius_sm())
            .bg(self.theme.surface)
            .border_1()
            .border_color(self.theme.border)
            .child(self.segment(
                "create-new-branch",
                "Create new branch",
                self.form.create_new_branch(),
                |modal, _| modal.form.set_create_new_branch(true),
                cx,
            ))
            .child(self.segment(
                "use-existing-branch",
                "Use existing branch",
                !self.form.create_new_branch(),
                |modal, _| modal.form.set_create_new_branch(false),
                cx,
            ));
        let location_modes = div()
            .flex()
            .flex_row()
            .gap(px(1.0))
            .p(px(1.0))
            .rounded(self.metrics.radius_sm())
            .bg(self.theme.surface)
            .border_1()
            .border_color(self.theme.border)
            .child(self.segment(
                "create-location-default",
                "Default",
                matches!(self.form.location(), LocationChoice::Default),
                |modal, _| modal.form.set_location(LocationChoice::Default),
                cx,
            ))
            .child(self.segment(
                "create-location-template",
                "Template",
                matches!(self.form.location(), LocationChoice::Template(_)),
                |modal, cx| {
                    modal.form.set_location(LocationChoice::Template(
                        muxy_api::worktree_location::SUGGESTED_PATH_TEMPLATE.into(),
                    ));
                    modal.location.update(cx, |input, cx| {
                        input.set_text(muxy_api::worktree_location::SUGGESTED_PATH_TEMPLATE, cx)
                    });
                },
                cx,
            ))
            .child(self.segment(
                "create-location-folder",
                "Folder",
                matches!(self.form.location(), LocationChoice::Folder(_)),
                |modal, cx| {
                    modal
                        .form
                        .set_location(LocationChoice::Folder(PathBuf::new()));
                    modal
                        .location
                        .update(cx, |input, cx| input.set_text("", cx));
                },
                cx,
            ));
        let mut form = div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing4())
            .child(self.field("Name", &self.name))
            .child(branch_modes);
        form = if self.form.create_new_branch() {
            form.child(self.field("Branch Name", &self.branch))
                .child(self.branch_picker_field("Base Branch", &self.base_branch_picker))
        } else {
            form.child(self.branch_picker_field("Branch", &self.existing_branch_picker))
        };
        form = form.child(
            div()
                .flex()
                .flex_col()
                .gap(self.metrics.spacing2())
                .child(
                    div()
                        .text_size(self.metrics.font_footnote())
                        .text_color(self.theme.fg_muted)
                        .child("Location"),
                )
                .child(location_modes),
        );
        match self.form.location() {
            LocationChoice::Template(_) => {
                form = form.child(self.field("Path Template", &self.location));
            }
            LocationChoice::Folder(_) => {
                form = form.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_end()
                        .gap(self.metrics.spacing3())
                        .child(
                            div()
                                .flex_grow()
                                .child(self.field("Parent Folder", &self.location)),
                        )
                        .child(self.button(
                            "choose-worktree-folder",
                            "Choose Folder...",
                            false,
                            !self.form.running(),
                            Some(ModalAction::ChooseFolder),
                            cx,
                        )),
                );
            }
            LocationChoice::Default => {}
        }
        let preview = self
            .form
            .preview()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|error| error);
        form = form.child(
            div()
                .text_size(self.metrics.font_caption())
                .text_color(if self.form.preview().is_ok() {
                    self.theme.fg_muted
                } else {
                    self.theme.danger
                })
                .child(SharedString::from(preview)),
        );
        if !self.form.setup_commands().is_empty() {
            let mut commands = div()
                .flex()
                .flex_col()
                .gap(self.metrics.spacing2())
                .p(self.metrics.spacing4())
                .rounded(self.metrics.radius_md())
                .bg(self.theme.hover)
                .child(
                    div()
                        .text_size(self.metrics.font_footnote())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(self.theme.fg)
                        .child("Setup commands"),
                );
            for command in self.form.setup_commands() {
                let source = match command.source {
                    muxy_api::worktree_config::CommandSource::Global => "Per-machine",
                    muxy_api::worktree_config::CommandSource::Project => "Project",
                };
                commands = commands.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(self.metrics.spacing3())
                        .text_size(self.metrics.font_caption())
                        .text_color(self.theme.fg)
                        .child(
                            div()
                                .flex_grow()
                                .child(SharedString::from(command.command.command.clone())),
                        )
                        .child(SharedString::from(source)),
                );
            }
            let run_setup = self.form.run_setup();
            commands = commands.child(
                div()
                    .id("toggle-worktree-setup")
                    .cursor_pointer()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.fg)
                    .on_click(cx.listener(move |modal, _, _, cx| {
                        if modal.form.running() {
                            return;
                        }
                        modal.form.set_run_setup(!run_setup);
                        cx.notify();
                    }))
                    .child(if run_setup {
                        "☑ Run these commands after creating the worktree"
                    } else {
                        "☐ Run these commands after creating the worktree"
                    }),
            );
            form = form.child(commands);
        }
        if let Some(error) = self.form.error() {
            form = form.child(
                div()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.danger)
                    .child(SharedString::from(error.to_owned())),
            );
        }
        let footer = div()
            .flex()
            .flex_row()
            .justify_end()
            .gap(self.metrics.spacing3())
            .child(self.button(
                "cancel-worktree-create",
                "Cancel",
                false,
                self.form.dismissible(),
                Some(ModalAction::Dismiss),
                cx,
            ))
            .child(self.button(
                "confirm-worktree-create",
                if self.form.running() {
                    "Creating..."
                } else {
                    "Create"
                },
                true,
                self.form.can_create(),
                Some(ModalAction::Submit),
                cx,
            ));
        div()
            .id("create-worktree-scroll")
            .track_focus(&self.focus)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .occlude()
            .flex()
            .flex_col()
            .w(self.metrics.scaled(520.0))
            .max_h(window.viewport_size().height - self.metrics.scaled(80.0))
            .overflow_y_scroll()
            .p(self.metrics.spacing8())
            .gap(self.metrics.spacing5())
            .rounded(self.metrics.radius_lg())
            .bg(self.theme.raised())
            .border_1()
            .border_color(self.theme.border)
            .shadow_lg()
            .child(
                div()
                    .text_size(self.metrics.font_headline())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.theme.fg)
                    .child("New Worktree"),
            )
            .child(form)
            .child(footer)
    }
}

fn branch_picker(
    id: &'static str,
    placeholder: &'static str,
    branches: &[String],
    selected: Option<&String>,
    theme: Theme,
    metrics: Metrics,
    cx: &mut Context<CreateWorktreeModal>,
) -> Entity<CommandPopover> {
    let items = branch_items(branches, "", selected.map_or("", String::as_str), false);
    let selected = selected.and_then(|selected| {
        branches
            .iter()
            .position(|branch| branch == selected)
            .map(|index| format!("worktree-branch-{index}"))
    });
    cx.new(move |cx| {
        let mut picker = CommandPopover::new(
            CommandPopoverConfig {
                id: id.into(),
                presentation: CommandPopoverPresentation::Embedded,
                density: CommandPopoverDensity::Compact,
                tabs: vec![CommandPopoverTab::new("branches", "Branches")],
                placeholder: placeholder.into(),
                footer_actions: Vec::new(),
                footer_hints: Vec::new(),
                width: None,
                height: None,
                max_height: Some(220.0),
                completion_on_tab: false,
                confirm_on_click: true,
            },
            theme,
            metrics,
            cx,
        );
        picker.set_items(items, cx);
        if let Some(selected) = selected {
            let _ = picker.select_row(&selected, cx);
        }
        picker
    })
}

fn branch_items(
    branches: &[String],
    query: &str,
    selected: &str,
    disabled: bool,
) -> Vec<CommandPopoverItem> {
    branches
        .iter()
        .enumerate()
        .filter(|(_, branch)| query.is_empty() || branch.to_lowercase().contains(query))
        .map(|(index, branch)| {
            let mut row =
                CommandPopoverRow::new(format!("worktree-branch-{index}"), branch.clone());
            row.leading = Some(CommandPopoverLeading::Icon(Icon::GitBranch));
            row.current = branch == selected;
            row.disabled = disabled;
            CommandPopoverItem::Row(row)
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct CreateWorktreeForm {
    project: Project,
    location_context: LocationContext,
    setup_commands: Vec<ResolvedCommand>,
    name: String,
    branch: String,
    branch_edited: bool,
    base_branch: String,
    existing_branch: String,
    create_new_branch: bool,
    location: LocationChoice,
    run_setup: bool,
    running: bool,
    error: Option<String>,
}

impl CreateWorktreeForm {
    pub fn new(
        project: Project,
        location_context: LocationContext,
        setup_commands: Vec<ResolvedCommand>,
    ) -> Self {
        let location = if let Some(template) = project
            .preferred_worktree_path_template
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            LocationChoice::Template(template.to_owned())
        } else if let Some(folder) = project
            .preferred_worktree_parent_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            LocationChoice::Folder(PathBuf::from(folder))
        } else {
            LocationChoice::Default
        };
        Self {
            project,
            location_context,
            setup_commands,
            name: String::new(),
            branch: String::new(),
            branch_edited: false,
            base_branch: String::new(),
            existing_branch: String::new(),
            create_new_branch: true,
            location,
            run_setup: false,
            running: false,
            error: None,
        }
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn base_branch(&self) -> &str {
        &self.base_branch
    }

    pub fn existing_branch(&self) -> &str {
        &self.existing_branch
    }

    pub fn create_new_branch(&self) -> bool {
        self.create_new_branch
    }

    pub fn location(&self) -> &LocationChoice {
        &self.location
    }

    pub fn setup_commands(&self) -> &[ResolvedCommand] {
        &self.setup_commands
    }

    pub fn run_setup(&self) -> bool {
        self.run_setup
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn set_name(&mut self, value: &str) {
        self.name = value.to_owned();
        if self.create_new_branch && !self.branch_edited {
            self.branch = value.to_owned();
        }
    }

    pub fn set_branch(&mut self, value: &str) {
        self.branch = value.to_owned();
        self.branch_edited = self.branch != self.name;
    }

    pub fn set_base_branch(&mut self, value: &str) {
        self.base_branch = value.to_owned();
    }

    pub fn set_existing_branch(&mut self, value: &str) {
        self.existing_branch = value.to_owned();
    }

    pub fn set_create_new_branch(&mut self, value: bool) {
        self.create_new_branch = value;
        if value && !self.branch_edited {
            self.branch.clone_from(&self.name);
        }
    }

    pub fn set_location(&mut self, value: LocationChoice) {
        self.location = value;
    }

    pub fn set_run_setup(&mut self, value: bool) {
        self.run_setup = value;
    }

    pub fn set_running(&mut self, value: bool) {
        self.running = value;
    }

    pub fn set_error(&mut self, value: Option<String>) {
        self.error = value;
    }

    pub fn dismissible(&self) -> bool {
        !self.running
    }

    #[cfg(test)]
    pub fn validation_error(&self) -> Option<String> {
        self.request().err()
    }

    pub fn can_create(&self) -> bool {
        !self.running && self.request().is_ok()
    }

    pub fn preview(&self) -> Result<PathBuf, String> {
        let name = self.name.trim();
        let slug = sanitize_component(if name.is_empty() { "name" } else { name })
            .map_err(|error| error.to_string())?;
        let branch = self.selected_branch();
        let branch = if branch.trim().is_empty() {
            "branch"
        } else {
            branch.trim()
        };
        resolve(
            &self.project,
            &slug,
            branch,
            self.location_request()?,
            &self.location_context,
        )
        .map(|location| location.path)
        .map_err(|error| error.to_string())
    }

    pub fn request(&self) -> Result<CreateWorktreeRequest, String> {
        let name = self.name.trim();
        let branch = self.selected_branch().trim();
        if name.is_empty() || branch.is_empty() {
            return Err("Name and branch are required.".into());
        }
        let location = self.location_request()?;
        self.preview()?;
        let setup_policy = if self.run_setup {
            SetupPolicy::NativeApproved(ProjectHookApproval::from_resolved(&self.setup_commands))
        } else {
            SetupPolicy::SkipAll
        };
        Ok(CreateWorktreeRequest {
            project: self.project.clone(),
            name: name.to_owned(),
            branch: branch.to_owned(),
            create_branch: self.create_new_branch,
            base_branch: self
                .create_new_branch
                .then(|| self.base_branch.trim().to_owned())
                .filter(|branch| !branch.is_empty()),
            location,
            setup_policy,
        })
    }

    fn selected_branch(&self) -> &str {
        if self.create_new_branch {
            &self.branch
        } else {
            &self.existing_branch
        }
    }

    fn location_request(&self) -> Result<WorktreeLocationRequest, String> {
        match &self.location {
            LocationChoice::Default => Ok(WorktreeLocationRequest::NativeAppDefault),
            LocationChoice::Template(value) => validate_template(value)
                .map(WorktreeLocationRequest::NativeTemplate)
                .map_err(|error| error.to_string()),
            LocationChoice::Folder(path) if path.as_os_str().is_empty() => {
                Err("Parent folder is required.".into())
            }
            LocationChoice::Folder(path) => Ok(WorktreeLocationRequest::NativeFolder(path.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_api::worktree_config::{CommandSource, ResolvedCommand};
    use muxy_api::worktree_location::{LocationContext, WorktreeLocationRequest};
    use muxy_core::store::Project;
    use std::path::PathBuf;

    fn project() -> Project {
        let mut project = Project::new("Project".into(), "/repo".into(), 0);
        project.id = "PROJECT".into();
        project.is_git_repo = true;
        project
    }

    fn context() -> LocationContext {
        LocationContext {
            home: PathBuf::from("/home/test"),
            profile_worktree_root: PathBuf::from("/profile/worktrees"),
            default_path_template: None,
            default_parent_path: None,
        }
    }

    #[test]
    fn create_worktree_form_models_modes_preview_setup_snapshot_and_running_state() {
        let commands = vec![
            ResolvedCommand::new("global", None, CommandSource::Global),
            ResolvedCommand::new(" project ", Some("Project"), CommandSource::Project),
        ];
        let mut form = CreateWorktreeForm::new(project(), context(), commands);
        form.set_name(" Feature ");
        assert_eq!(form.branch(), " Feature ");
        form.set_branch("feature/one");
        form.set_base_branch("main");
        form.set_location(LocationChoice::Template("../{base-dir}.{branch}".into()));
        assert_eq!(form.preview().unwrap(), PathBuf::from("/repo.feature-one"));
        form.set_run_setup(true);
        let request = form.request().unwrap();
        assert!(request.create_branch);
        assert_eq!(request.base_branch.as_deref(), Some("main"));
        assert!(matches!(
            request.location,
            WorktreeLocationRequest::NativeTemplate(_)
        ));
        let approval = match request.setup_policy {
            muxy_api::worktree_hooks::SetupPolicy::NativeApproved(approval) => approval,
            _ => panic!("expected native approval"),
        };
        assert_eq!(approval.commands.len(), 1);
        assert_eq!(approval.commands[0].command, "project");
        form.set_running(true);
        assert!(!form.dismissible());
        assert!(!form.can_create());
    }

    #[test]
    fn create_worktree_form_validates_folder_and_existing_branch_choices() {
        let mut form = CreateWorktreeForm::new(project(), context(), Vec::new());
        form.set_name("Existing");
        form.set_create_new_branch(false);
        assert!(!form.can_create());
        form.set_existing_branch("release");
        form.set_location(LocationChoice::Folder(PathBuf::new()));
        assert!(form.validation_error().is_some());
        form.set_location(LocationChoice::Folder(PathBuf::from("/worktrees")));
        assert!(form.can_create());
        let request = form.request().unwrap();
        assert!(!request.create_branch);
        assert_eq!(request.branch, "release");
        assert_eq!(request.base_branch, None);
    }
}
