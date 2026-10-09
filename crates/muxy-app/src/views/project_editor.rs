pub(crate) mod icons;
pub(crate) mod logo;

use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    ParentElement, Pixels, Point, StatefulInteractiveElement, Styled, Window, div, px, size,
};
use muxy_app_core::{ProjectId, ProjectStatus, TabId, WorkspaceId};
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::tr;

use super::overlays::{Overlay, clamp};
use crate::model::AppModel;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Field {
    Name,
    Icon,
}

#[derive(Clone, Copy)]
enum Target {
    Project(ProjectId),
    Tab(TabId),
    Workspace(WorkspaceId),
    NewWorkspace(Option<ProjectId>),
}

pub(crate) struct Editor {
    target: Target,
    input: Entity<TextInput>,
    position: Point<Pixels>,
    error: Option<String>,
}

impl AppModel {
    pub(crate) fn open_project_editor(
        &mut self,
        id: ProjectId,
        field: Field,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self
            .state
            .project(id)
            .filter(|project| project.status() == ProjectStatus::Available)
        else {
            return;
        };
        if matches!(field, Field::Icon) {
            self.open_project_icons(id, position, window, cx);
            return;
        }
        let text = project.name.clone();
        self.open_metadata_editor(Target::Project(id), text, position, window, cx);
    }

    pub(crate) fn open_tab_editor(
        &mut self,
        id: TabId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tab(id) else {
            return;
        };
        let text = tab.title(self.state.shown_pane(tab)).to_owned();
        self.open_metadata_editor(Target::Tab(id), text, position, window, cx);
    }

    pub(crate) fn open_workspace_editor(
        &mut self,
        id: WorkspaceId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(workspace) = self.state.workspace(id) {
            let text = workspace.name.clone();
            self.open_metadata_editor(Target::Workspace(id), text, position, window, cx);
        }
    }

    /// Creates a workspace, adding `project` when it starts from a project menu.
    pub(crate) fn open_new_workspace_editor(
        &mut self,
        project: Option<ProjectId>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = Target::NewWorkspace(project);
        self.open_metadata_editor(target, String::new(), position, window, cx);
    }

    fn open_metadata_editor(
        &mut self,
        target: Target,
        text: String,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            TextInput::new(InputStyle::field(&self.theme, &self.metrics), cx).with_text(text)
        });
        input.update(cx, TextInput::select_all_text);
        input.focus_handle(cx).focus(window);
        self.overlay_subscription = Some(cx.subscribe(&input, |model, _, event, cx| match event {
            InputEvent::Submitted => model.submit_project_editor(cx),
            InputEvent::Cancelled => model.dismiss_overlay(cx),
            InputEvent::Changed => {
                if let Some(Overlay::ProjectEditor(editor)) = &mut model.overlay {
                    editor.error = None;
                }
                cx.notify();
            }
        }));
        self.overlay = Some(Overlay::ProjectEditor(Editor {
            target,
            input,
            position,
            error: None,
        }));
        cx.notify();
    }

    fn submit_project_editor(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::ProjectEditor(editor)) = &self.overlay else {
            return;
        };
        let target = editor.target;
        let text = editor.input.read(cx).text().trim().to_owned();
        let saved = match target {
            Target::Tab(id) => self.edit_tab(|state| state.set_tab_title(id, Some(text)), cx),
            Target::Project(id) => self.edit_project(|state| state.rename_project(id, &text), cx),
            Target::Workspace(id) => {
                self.edit_workspaces(|state| state.rename_workspace(id, &text), cx)
            }
            Target::NewWorkspace(project) => {
                let created = self.create_workspace(&text, project, cx);
                if project.is_none() && created.is_some() {
                    self.select_workspace(created, cx);
                }
                created.is_some()
            }
        };
        if saved {
            self.dismiss_overlay(cx);
        } else if let Some(Overlay::ProjectEditor(editor)) = &mut self.overlay {
            editor.error.clone_from(&self.error);
        }
        cx.notify();
    }
}

pub(crate) fn render(
    editor: &Editor,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let origin = clamp(
        editor.position,
        size(m.scaled(320.0), m.scaled(220.0)),
        window.viewport_size(),
    );
    muxy_ui::popover::surface(theme, m)
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .w(m.scaled(320.0)
            .min((window.viewport_size().width - px(16.0)).max(px(0.0))))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(model.webview_occlusion())
        .child(
            muxy_ui::popover::header(theme, m).child(match editor.target {
                Target::Tab(_) => tr!("Rename Tab"),
                Target::Project(_) => tr!("Rename Project"),
                Target::Workspace(_) => tr!("Rename Workspace"),
                Target::NewWorkspace(_) => tr!("New Workspace"),
            }),
        )
        .child(
            muxy_ui::popover::body(m)
                .child(muxy_ui::controls::text_field(
                    muxy_ui::controls::Style { theme, metrics: &m },
                    "project-editor",
                    &editor.input,
                    None,
                ))
                .children(editor.error.as_ref().map(|error| {
                    div()
                        .text_size(m.font_footnote())
                        .text_color(theme.danger)
                        .child(error.clone())
                })),
        )
        .child(
            muxy_ui::popover::footer(theme, m)
                .child(muxy_ui::controls::button(
                    muxy_ui::controls::Style { theme, metrics: &m },
                    "project-editor-cancel",
                    &tr!("Cancel"),
                    true,
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                ))
                .child(
                    div()
                        .id("save-project-editor")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(m.control_medium())
                        .px(m.spacing5())
                        .rounded(m.radius_md())
                        .bg(theme.accent)
                        .text_color(theme.accent_foreground)
                        .text_size(m.font_body())
                        .child(if matches!(editor.target, Target::NewWorkspace(_)) {
                            tr!("Create")
                        } else {
                            tr!("Save")
                        })
                        .on_click(cx.listener(|model, _, _, cx| model.submit_project_editor(cx))),
                ),
        )
        .into_any_element()
}
