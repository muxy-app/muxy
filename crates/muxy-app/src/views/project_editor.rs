use muxy_core::shortcuts::ShortcutId;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, StatefulInteractiveElement, Styled, Window, actions,
    div, px, size,
};
use muxy_app_core::{PROJECT_COLORS, ProjectId, ProjectStatus, TabId};
use muxy_ui::components::{SymbolGlyph, Tooltip};
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::theme::parse_hex;

use super::overlays::{Overlay, clamp};
use crate::model::AppModel;

actions!(
    project_colors,
    [PreviousColor, NextColor, ChooseColor, DismissColors]
);

pub(crate) fn register_shortcuts(registry: &mut muxy_ui::shortcuts::Registry<'_>) {
    registry.register(ShortcutId::ProjectColorsPreviousColor, &PreviousColor);
    registry.register(ShortcutId::ProjectColorsNextColor, &NextColor);
    registry.register(ShortcutId::ProjectColorsChooseColor, &ChooseColor);
    registry.register(ShortcutId::ProjectColorsDismissColors, &DismissColors);
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Field {
    Name,
    Icon,
}

#[derive(Clone, Copy)]
enum Target {
    Project(ProjectId),
    Tab(TabId),
}

pub(crate) struct Editor {
    target: Target,
    field: Field,
    input: Entity<TextInput>,
    position: Point<Pixels>,
    error: Option<String>,
}

pub(crate) struct Colors {
    target: Target,
    position: Point<Pixels>,
    selected: Option<usize>,
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
        let text = match field {
            Field::Name => project.name.clone(),
            Field::Icon => project.icon.clone().unwrap_or_default(),
        };
        self.open_metadata_editor(Target::Project(id), field, text, position, window, cx);
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
        let text = tab.title(self.state.window().active_pane).to_owned();
        self.open_metadata_editor(Target::Tab(id), Field::Name, text, position, window, cx);
    }

    fn open_metadata_editor(
        &mut self,
        target: Target,
        field: Field,
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
            field,
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
        let field = editor.field;
        let text = editor.input.read(cx).text().trim().to_owned();
        let saved = match target {
            Target::Tab(id) => self.edit_tab(|state| state.set_tab_title(id, Some(text)), cx),
            Target::Project(id) => self.edit_project(
                |state| match field {
                    Field::Name => state.rename_project(id, &text),
                    Field::Icon => state.set_project_icon(id, (!text.is_empty()).then_some(text)),
                },
                cx,
            ),
        };
        if saved {
            self.dismiss_overlay(cx);
        } else if let Some(Overlay::ProjectEditor(editor)) = &mut self.overlay {
            editor.error.clone_from(&self.error);
        }
        cx.notify();
    }

    pub(crate) fn open_project_colors(
        &mut self,
        id: ProjectId,
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
        let selected = PROJECT_COLORS
            .iter()
            .position(|(_, color)| *color == project.color.as_str());
        self.open_metadata_colors(Target::Project(id), selected, position, window, cx);
    }

    pub(crate) fn open_tab_colors(
        &mut self,
        id: TabId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tab(id) else {
            return;
        };
        let selected = PROJECT_COLORS.iter().position(|(_, hex)| {
            tab.color
                .as_ref()
                .is_some_and(|color| color.as_str() == *hex)
        });
        self.open_metadata_colors(Target::Tab(id), selected, position, window, cx);
    }

    fn open_metadata_colors(
        &mut self,
        target: Target,
        selected: Option<usize>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::ProjectColors(Colors {
            target,
            position,
            selected,
        }));
        self.overlay_focus.focus(window);
        cx.notify();
    }

    fn move_color(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(Overlay::ProjectColors(colors)) = &mut self.overlay {
            colors.selected = Some(
                (colors.selected.unwrap_or(0) + if forward { 1 } else { PROJECT_COLORS.len() - 1 })
                    % PROJECT_COLORS.len(),
            );
            cx.notify();
        }
    }

    fn choose_color(&mut self, selected: Option<usize>, cx: &mut Context<Self>) {
        let Some(Overlay::ProjectColors(colors)) = &self.overlay else {
            return;
        };
        let target = colors.target;
        let index = selected.or(colors.selected).unwrap_or(0);
        if let Some((_, hex)) = PROJECT_COLORS.get(index)
            && let Ok(color) = hex.parse()
            && match target {
                Target::Project(id) => {
                    self.edit_project(|state| state.set_project_color(id, color), cx)
                }
                Target::Tab(id) => self.edit_tab(|state| state.set_tab_color(id, Some(color)), cx),
            }
        {
            self.dismiss_overlay(cx);
        }
    }

    fn reset_tab_color(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::ProjectColors(Colors {
            target: Target::Tab(id),
            ..
        })) = &self.overlay
        {
            let id = *id;
            if self.edit_tab(|state| state.set_tab_color(id, None), cx) {
                self.dismiss_overlay(cx);
            }
        }
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
        size(px(300.0), px(190.0)),
        window.viewport_size(),
    );
    div()
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .w(px(300.0))
        .flex()
        .flex_col()
        .gap(m.spacing4())
        .p(m.spacing5())
        .rounded(m.radius_lg())
        .bg(theme.raised())
        .border_1()
        .border_color(theme.border)
        .shadow_lg()
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .text_size(m.font_body())
                .font_weight(FontWeight::SEMIBOLD)
                .child(match (editor.target, editor.field) {
                    (Target::Tab(_), _) => "Rename Tab",
                    (_, Field::Name) => "Rename Project",
                    (_, Field::Icon) => "Change Icon",
                }),
        )
        .child(editor.input.clone())
        .children(matches!(editor.field, Field::Icon).then(|| {
            div()
                .text_size(m.font_footnote())
                .text_color(theme.fg_muted)
                .child("Enter one emoji, or leave empty to use the default.")
        }))
        .children(editor.error.as_ref().map(|error| {
            div()
                .text_size(m.font_footnote())
                .text_color(theme.danger)
                .child(error.clone())
        }))
        .child(
            div().flex().justify_end().child(
                div()
                    .id("save-project-editor")
                    .cursor_pointer()
                    .px(m.spacing4())
                    .py(m.spacing2())
                    .rounded(m.radius_md())
                    .bg(theme.accent)
                    .text_color(theme.accent_foreground)
                    .text_size(m.font_body())
                    .child("Save")
                    .on_click(cx.listener(|model, _, _, cx| model.submit_project_editor(cx))),
            ),
        )
        .into_any_element()
}

pub(crate) fn render_colors(
    colors: &Colors,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    let tab = match colors.target {
        Target::Tab(id) => model.tab(id),
        Target::Project(_) => None,
    };
    let can_reset = tab.is_some_and(|tab| tab.color.is_some());
    let origin = clamp(
        colors.position,
        size(
            m.scaled(216.0),
            m.scaled(if tab.is_some() { 144.0 } else { 108.0 }),
        ),
        window.viewport_size(),
    );
    div()
        .debug_selector(|| "color-picker".into())
        .key_context("ProjectColors")
        .track_focus(&model.overlay_focus)
        .on_action(cx.listener(|model, _: &PreviousColor, _, cx| model.move_color(false, cx)))
        .on_action(cx.listener(|model, _: &NextColor, _, cx| model.move_color(true, cx)))
        .on_action(cx.listener(|model, _: &ChooseColor, _, cx| model.choose_color(None, cx)))
        .on_action(cx.listener(|model, _: &DismissColors, _, cx| model.dismiss_overlay(cx)))
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .w(m.scaled(216.0))
        .flex()
        .flex_col()
        .gap(m.spacing5())
        .p(m.spacing6())
        .rounded(m.radius_lg())
        .bg(theme.raised())
        .border_1()
        .border_color(theme.border)
        .shadow_lg()
        .occlude()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .text_size(m.font_body())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg)
                .child(if tab.is_some() {
                    "Tab Color"
                } else {
                    "Project Color"
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(m.spacing4())
                .children(PROJECT_COLORS.chunks(6).enumerate().map(|(row, swatches)| {
                    div()
                        .flex()
                        .gap(m.spacing4())
                        .children(swatches.iter().enumerate().map(|(column, _)| {
                            color_swatch(row * 6 + column, colors.selected, model, cx)
                        }))
                })),
        )
        .when(tab.is_some(), |picker| {
            picker.child(div().h(px(1.0)).bg(theme.border)).child(
                div()
                    .id("reset-tab-color")
                    .debug_selector(|| "reset-tab-color".into())
                    .flex()
                    .items_center()
                    .gap(m.spacing3())
                    .text_size(m.font_footnote())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.fg_muted)
                    .opacity(if can_reset { 1.0 } else { 0.4 })
                    .when(can_reset, |button| {
                        button
                            .cursor_pointer()
                            .on_click(cx.listener(|model, _, _, cx| model.reset_tab_color(cx)))
                    })
                    .child(SymbolGlyph::new(
                        "arrow.uturn.backward",
                        m.font_caption(),
                        theme.fg_muted,
                    ))
                    .child("Reset to Default"),
            )
        })
        .into_any_element()
}

fn color_swatch(
    index: usize,
    selected: Option<usize>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let (name, hex) = PROJECT_COLORS[index];
    let theme = &model.theme;
    let m = model.metrics;
    let color = parse_hex(hex).unwrap_or(gpui::rgb(0));
    let foreground = if 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b > 0.6 {
        gpui::black()
    } else {
        gpui::white()
    };
    let tooltip_theme = theme.clone();
    div()
        .id(("project-color", index))
        .debug_selector(move || format!("color-swatch-{index}"))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .size(m.control_medium())
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(m.scaled(22.0))
                .rounded_full()
                .bg(color)
                .when(selected == Some(index), |swatch| {
                    swatch.child(
                        div()
                            .debug_selector(|| "color-selection-ring".into())
                            .size(m.scaled(18.0))
                            .rounded_full()
                            .border_2()
                            .border_color(foreground),
                    )
                }),
        )
        .tooltip(move |_, cx| {
            cx.new(|_| {
                Tooltip::new(
                    name,
                    tooltip_theme.raised(),
                    tooltip_theme.fg,
                    tooltip_theme.border,
                )
            })
            .into()
        })
        .on_click(cx.listener(move |model, _, _, cx| model.choose_color(Some(index), cx)))
        .into_any_element()
}
