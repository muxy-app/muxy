use crate::{
    model::AppModel,
    views::overlays::{Overlay, clamp},
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px, size,
};
use muxy_app_core::ProjectId;
use muxy_ui::{
    components::{ButtonInteraction, SymbolGlyph, Tooltip},
    text_input::{InputEvent, InputStyle, TextInput},
};

pub(crate) struct Icons {
    project: ProjectId,
    input: Entity<TextInput>,
    position: Point<Pixels>,
    matches: Vec<&'static muxy_ui::symbols::Symbol>,
    error: Option<String>,
}

impl AppModel {
    pub(crate) fn open_project_icons(
        &mut self,
        project: ProjectId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            TextInput::new(InputStyle::compact(&self.theme, &self.metrics), cx)
                .with_placeholder("Search symbols…")
        });
        input.focus_handle(cx).focus(window);
        self.overlay_subscription =
            Some(cx.subscribe(&input, |model, input, event, cx| match event {
                InputEvent::Cancelled => model.dismiss_overlay(cx),
                InputEvent::Changed => {
                    if let Some(Overlay::ProjectIcons(picker)) = &mut model.overlay {
                        picker.matches = muxy_ui::symbols::matching(input.read(cx).text());
                        picker.error = None;
                    }
                    cx.notify();
                }
                InputEvent::Submitted => {
                    if let Some(Overlay::ProjectIcons(picker)) = &model.overlay
                        && let Some(symbol) = picker.matches.first()
                    {
                        model.choose_project_symbol(Some(symbol.symbol), cx);
                    }
                }
            }));
        self.overlay = Some(Overlay::ProjectIcons(Icons {
            project,
            input,
            position,
            matches: muxy_ui::symbols::matching(""),
            error: None,
        }));
        cx.notify();
    }

    fn choose_project_symbol(&mut self, symbol: Option<&str>, cx: &mut Context<Self>) {
        let Some(Overlay::ProjectIcons(picker)) = &self.overlay else {
            return;
        };
        let project = picker.project;
        if self.edit_project(
            |state| state.set_project_icon(project, symbol.map(|symbol| format!("sf:{symbol}"))),
            cx,
        ) {
            self.dismiss_overlay(cx);
        } else if let Some(Overlay::ProjectIcons(picker)) = &mut self.overlay {
            picker.error.clone_from(&self.error);
        }
    }
}

pub(crate) fn render(
    picker: &Icons,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let selected = model
        .state
        .project(picker.project)
        .and_then(|project| project.icon.as_deref());
    let width = m
        .scaled(300.0)
        .min((window.viewport_size().width - px(16.0)).max(px(0.0)));
    let height = m
        .scaled(376.0)
        .min((window.viewport_size().height - px(16.0)).max(px(0.0)));
    let origin = clamp(picker.position, size(width, height), window.viewport_size());
    muxy_ui::popover::surface(theme, m)
        .debug_selector(|| "project-symbol-picker".into())
        .absolute()
        .left(origin.x)
        .top(origin.y)
        .w(width)
        .h(height)
        .p(m.spacing6())
        .gap(m.spacing5())
        .on_key_down(cx.listener(|model, event: &gpui::KeyDownEvent, _, cx| {
            if event.keystroke.key == "escape" {
                model.dismiss_overlay(cx);
            }
        }))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Icon"),
        )
        .child(muxy_ui::controls::text_field(
            muxy_ui::controls::Style { theme, metrics: &m },
            "project-symbol-search",
            &picker.input,
            None,
        ))
        .child(
            div()
                .id("project-symbols-scroll")
                .debug_selector(|| "project-symbols-scroll".into())
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .child(
                    div().flex().flex_wrap().gap(m.spacing4()).children(
                        picker
                            .matches
                            .iter()
                            .copied()
                            .map(|symbol| symbol_button(symbol, selected, model, cx)),
                    ),
                )
                .when(picker.matches.is_empty(), |grid| {
                    grid.child(div().text_color(theme.fg_muted).child("No symbols found"))
                }),
        )
        .children(
            picker
                .error
                .as_ref()
                .map(|error| div().text_color(theme.danger).child(error.clone())),
        )
        .child(div().flex_none().h(px(1.0)).bg(theme.border))
        .child(
            div()
                .id("remove-project-icon")
                .debug_selector(|| "remove-project-icon".into())
                .flex_none()
                .flex()
                .items_center()
                .gap(m.spacing3())
                .text_color(theme.fg_muted)
                .text_size(m.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .opacity(if selected.is_some() { 1.0 } else { 0.4 })
                .when(selected.is_some(), |button| {
                    button.cursor_pointer().button_interaction(
                        cx.listener(|model, _, _, cx| model.choose_project_symbol(None, cx)),
                    )
                })
                .child(SymbolGlyph::new(
                    "xmark.circle",
                    m.font_caption(),
                    theme.fg_muted,
                ))
                .child("Remove Icon"),
        )
        .into_any_element()
}

fn symbol_button(
    symbol: &'static muxy_ui::symbols::Symbol,
    selected: Option<&str>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let name = symbol.symbol;
    let active = selected.and_then(|value| value.strip_prefix("sf:")) == Some(name);
    let tooltip_theme = theme.clone();
    div()
        .id(SharedString::from(format!("project-symbol-{name}")))
        .debug_selector(move || format!("project-symbol-{name}"))
        .size(m.control_large())
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(m.radius_sm())
        .cursor_pointer()
        .when(active, |button| button.bg(theme.accent_soft))
        .hover(|style| style.bg(theme.hover))
        .tooltip(move |_, cx| {
            cx.new(|_| {
                Tooltip::new(
                    symbol.name,
                    tooltip_theme.raised(),
                    tooltip_theme.fg,
                    tooltip_theme.border,
                )
            })
            .into()
        })
        .child(
            SymbolGlyph::new(
                name,
                m.font_title_large(),
                if active { theme.accent } else { theme.fg },
            )
            .regular(),
        )
        .button_interaction(cx.listener(move |model, _, _, cx| {
            model.choose_project_symbol(Some(name), cx);
        }))
        .into_any_element()
}
