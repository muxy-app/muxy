use std::fmt::Write as _;

use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, div};
use muxy_app_core::settings::{DEFAULT_WORKTREE_FOLDER, WorktreeLocation};
use muxy_protocol::{
    GitAction, ProjectId, WorktreeHook, WorktreeIntent, WorktreeOptions, WorktreeRemoval,
};
use muxy_ui::controls::{self, Choice, Style};
use muxy_ui::text_input::TextInput;
use muxy_ui::tr;

use super::{AppModel, Form, Overlay, form::field};

impl Form {
    pub(super) fn branch_input(&self) -> &Entity<TextInput> {
        if self.worktree && self.existing {
            &self.existing_branch
        } else {
            &self.branch
        }
    }

    pub(super) fn location(&self, cx: &App) -> WorktreeLocation {
        WorktreeLocation {
            path_template: if self.location_mode == "template" {
                self.template.read(cx).text().trim().into()
            } else {
                String::new()
            },
            parent_path: if self.location_mode == "folder" {
                self.directory.read(cx).text().trim().into()
            } else {
                String::new()
            },
        }
    }
}

impl AppModel {
    pub(crate) fn worktree_directory(
        &self,
        form: &Form,
        cx: &App,
    ) -> Result<std::path::PathBuf, String> {
        let location = form.location(cx);
        if form.location_mode != "default" && location.is_default() {
            return Err(if form.location_mode == "template" {
                tr!("Path template is required.")
            } else {
                tr!("Folder is required.")
            }
            .into());
        }
        let project = self
            .state
            .project(form.project)
            .ok_or_else(|| tr!("Project is unavailable").to_string())?;
        self.settings
            .worktrees
            .directory(
                project,
                &location,
                form.name.read(cx).text().trim(),
                form.branch_input().read(cx).text().trim(),
                self.remote_home(project.server_id),
            )
            .map_err(|e| e.to_string())
    }

    pub(crate) fn receive_worktree_hooks(
        &mut self,
        project: ProjectId,
        result: Result<Vec<WorktreeHook>, String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(Overlay::GitForm(form)) = &mut self.overlay
            && form.project == project
        {
            form.run_setup = false;
            match result {
                Ok(hooks) => {
                    form.hooks = Some(hooks);
                    form.hooks_error = None;
                }
                Err(error) => {
                    form.hooks = None;
                    form.hooks_error = Some(error);
                }
            }
            cx.notify();
        }
    }

    pub(crate) fn confirm_worktree_removal(
        &mut self,
        project: ProjectId,
        expected: WorktreeRemoval,
        hooks: Result<Vec<WorktreeHook>, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self
            .state
            .project(project)
            .map(|record| record.name.clone())
        else {
            return;
        };
        let mut message = if expected.dirty {
            tr!("Remove worktree “%@” and permanently discard its uncommitted changes? Local processes will stop and its files will be deleted.", &name)
        } else {
            tr!("Remove worktree “%@” and delete its files? Local processes running from this worktree will stop.", &name)
        }
        .to_string();
        let hooks = match hooks {
            Ok(hooks) => {
                if !hooks.is_empty() {
                    let _ = write!(
                        message,
                        "\n\n{}\n",
                        tr!("Optional teardown commands (review before enabling):")
                    );
                    for hook in &hooks {
                        let _ = write!(
                            message,
                            "\n[{}] {}",
                            if hook.project {
                                tr!("Project")
                            } else {
                                tr!("Per-machine")
                            },
                            hook.command
                        );
                    }
                }
                Some(hooks)
            }
            Err(error) => {
                let _ = write!(
                    message,
                    "\n\n{}",
                    tr!(
                        "Teardown hooks could not be loaded: %@\nContinuing will remove without hooks.",
                        error
                    )
                );
                None
            }
        };
        self.confirm_git_action(
            project,
            GitAction::Worktree(WorktreeIntent {
                operation: muxy_protocol::OperationId::new(),
                action: muxy_protocol::WorktreeAction::Remove { expected },
                options: Some(WorktreeOptions { name: None, hooks }),
            }),
            message,
            cx,
        );
    }

    fn choose_worktree_folder(&mut self, cx: &mut Context<Self>) {
        let context = self.git.interaction;
        let result = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("Choose Folder")),
        });
        cx.spawn(async move |model, cx| {
            let result = result.await;
            let _ = model.update(cx, |model, cx| {
                if model.git.interaction != context {
                    return;
                }
                if let Some(Overlay::GitForm(form)) = &mut model.overlay {
                    match result {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.first() {
                                form.location_mode = "folder";
                                form.directory.update(cx, |input, cx| {
                                    input.set_text(path.to_string_lossy().into_owned(), cx);
                                });
                            }
                        }
                        Ok(Ok(None)) => (),
                        Ok(Err(error)) => form.error = Some(error.to_string()),
                        Err(error) => form.error = Some(error.to_string()),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

pub(super) fn location(form: &Form, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let style = Style {
        theme: &model.theme,
        metrics: &model.metrics,
    };
    let mut content =
        div()
            .flex()
            .flex_col()
            .gap(model.metrics.spacing3())
            .child(controls::segmented(
                style,
                "worktree-location",
                &[
                    Choice::new("default", tr!("Default")),
                    Choice::new("template", tr!("Template")),
                    Choice::new("folder", tr!("Folder")),
                ],
                form.location_mode,
                cx.listener(|model, value: &gpui::SharedString, _, cx| {
                    if let Some(Overlay::GitForm(form)) = &mut model.overlay {
                        form.location_mode = match value.as_ref() {
                            "template" => "template",
                            "folder" => "folder",
                            _ => "default",
                        };
                        form.error = None;
                    }
                    cx.notify();
                }),
            ));
    match form.location_mode {
        "template" => {
            content = content.child(controls::text_field(
                style,
                "git-template",
                &form.template,
                None,
            ));
        }
        "folder" => {
            content = content
                .child(controls::text_field(
                    style,
                    "git-directory",
                    &form.directory,
                    None,
                ))
                .when(model.project_is_local(form.project), |content| {
                    content.child(controls::button(
                        style,
                        "worktree-choose-folder",
                        &tr!("Choose Folder…"),
                        true,
                        cx.listener(|model, _, _, cx| model.choose_worktree_folder(cx)),
                    ))
                });
        }
        _ => {
            let default = &model.settings.worktrees.default_location;
            let description = if !default.path_template.is_empty() {
                tr!("Global template: %@", &default.path_template)
            } else if !default.parent_path.is_empty() {
                tr!("Global folder: %@", &default.parent_path)
            } else {
                DEFAULT_WORKTREE_FOLDER.into()
            };
            content = content.child(
                div()
                    .text_size(model.metrics.font_caption())
                    .child(description),
            );
        }
    }
    let (preview, color) = match model.worktree_directory(form, cx) {
        Ok(path) => (path.to_string_lossy().into_owned(), model.theme.fg_muted),
        Err(error) => (error, model.theme.danger),
    };
    content = content
        .child(
            div()
                .text_size(model.metrics.font_caption())
                .text_color(color)
                .child(preview),
        )
        .child(
            div()
                .text_size(model.metrics.font_caption())
                .text_color(model.theme.fg_muted)
                .child(tr!(
                    "Templates must include {branch}. Relative paths start from the project folder."
                )),
        );
    field(&tr!("Location"), content.into_any_element(), model).into_any_element()
}

pub(super) fn hooks(form: &Form, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let style = Style {
        theme: &model.theme,
        metrics: &model.metrics,
    };
    let mut content = div()
        .flex()
        .flex_col()
        .gap(model.metrics.spacing3())
        .text_size(model.metrics.font_caption());
    if let Some(error) = &form.hooks_error {
        content = content.child(div().text_color(model.theme.danger).child(tr!(
            "Setup hooks unavailable: %@. You can create without hooks.",
            error
        )));
    } else if let Some(hooks) = &form.hooks {
        if hooks.is_empty() {
            content = content.child(tr!("Optional setup commands: add setup to .muxy/worktree.json in the project or $XDG_CONFIG_HOME/muxy/worktree.json (defaults to ~/.config/muxy/worktree.json)."));
        } else {
            content = content.child(tr!(
                "Commands run in the new worktree. Review project commands before enabling them."
            ));
            for hook in hooks {
                content = content.child(format!(
                    "[{}] {}",
                    if hook.project {
                        tr!("Project")
                    } else {
                        tr!("Per-machine")
                    },
                    hook.command
                ));
            }
            content = content.child(
                div()
                    .flex()
                    .gap(model.metrics.spacing3())
                    .child(controls::toggle(
                        style,
                        "worktree-setup",
                        form.run_setup,
                        cx.listener(|model, _, _, cx| {
                            if let Some(Overlay::GitForm(form)) = &mut model.overlay {
                                form.run_setup = !form.run_setup;
                            }
                            cx.notify();
                        }),
                    ))
                    .child(tr!("Run these commands after creating the worktree")),
            );
        }
    } else {
        content = content.child(tr!("Loading setup commands…"));
    }
    field(&tr!("Setup commands"), content.into_any_element(), model).into_any_element()
}
