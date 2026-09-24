use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Subscription,
    Window, div, px, svg,
};
use muxy_protocol::ProjectId;
use muxy_ui::components::{ButtonInteraction, SymbolGlyph};
use muxy_ui::controls::{self, Style};
use muxy_ui::popover;
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};

use crate::ai::{PROVIDERS, Provider};
use crate::model::AppModel;
use crate::repository_actions::Action;
use crate::views::{menu, overlays::Overlay};

pub(crate) struct AiProviderMenu {
    pub(crate) action: Action,
    pub(crate) project: ProjectId,
    pub(crate) prompt: Option<Entity<TextInput>>,
    highlighted: Option<usize>,
    subscription: Option<Subscription>,
}

impl AppModel {
    pub(crate) fn open_ai_provider_menu(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = self.state.current_project().id;
        if self.ai.running(project) {
            return;
        }
        if matches!(&self.overlay, Some(Overlay::AiProvider(menu)) if menu.action == action) {
            self.dismiss_overlay(cx);
            return;
        }
        #[cfg(not(test))]
        self.ai.refresh_installed();
        self.dismiss_overlay(cx);
        self.overlay = Some(Overlay::AiProvider(AiProviderMenu {
            action,
            project: self.ai_prompt_project(project),
            prompt: None,
            highlighted: None,
            subscription: None,
        }));
        self.overlay_focus.focus(window);
        cx.notify();
    }

    fn move_ai_provider(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(Overlay::AiProvider(menu)) = &mut self.overlay {
            let count = PROVIDERS.len() + 1 + usize::from(menu.action == Action::CreatePullRequest);
            menu.highlighted = Some(match (menu.highlighted, forward) {
                (Some(index), true) => (index + 1) % count,
                (Some(index), false) => (index + count - 1) % count,
                (None, true) => 0,
                (None, false) => count - 1,
            });
            cx.notify();
        }
    }

    fn confirm_ai_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::AiProvider(menu)) = &self.overlay else {
            return;
        };
        if menu.prompt.is_some() {
            self.save_project_pr_prompt(false, cx);
        } else if let Some(index) = menu.highlighted {
            self.select_ai_provider(index, window, cx);
        }
    }

    pub(crate) fn select_ai_provider(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::AiProvider(menu)) = &self.overlay else {
            return;
        };
        if index == PROVIDERS.len() + 1 && menu.action == Action::CreatePullRequest {
            self.edit_project_pr_prompt(window, cx);
            return;
        }
        let action = menu.action;
        let id = if index == 0 {
            ""
        } else if let Some(provider) = PROVIDERS.get(index - 1) {
            provider.id
        } else {
            return;
        };
        self.set_ai_provider(action, id, cx);
        self.dismiss_overlay(cx);
    }

    pub(crate) fn edit_project_pr_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::AiProvider(menu)) = &self.overlay else {
            return;
        };
        let text = self.configured_ai_prompt(Action::CreatePullRequest, Some(menu.project));
        let input = cx.new(|cx| {
            TextInput::new(InputStyle::field(&self.theme, &self.metrics), cx)
                .with_font_family("monospace")
                .multiline()
                .with_text(text)
        });
        let subscription =
            cx.subscribe_in(&input, window, |model, _, event, window, cx| match event {
                InputEvent::Submitted => model.save_project_pr_prompt(false, cx),
                InputEvent::Cancelled => model.cancel_project_pr_prompt(window, cx),
                InputEvent::Changed => cx.notify(),
            });
        input.focus_handle(cx).focus(window);
        if let Some(Overlay::AiProvider(menu)) = &mut self.overlay {
            menu.prompt = Some(input);
            menu.subscription = Some(subscription);
        }
        cx.notify();
    }

    fn cancel_project_pr_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Overlay::AiProvider(menu)) = &mut self.overlay {
            menu.prompt = None;
            menu.subscription = None;
        }
        self.overlay_focus.focus(window);
        cx.notify();
    }
}

fn provider_icon(provider: Option<Provider>, model: &AppModel) -> AnyElement {
    let size = model.metrics.icon_md();
    match provider {
        None => SymbolGlyph::new("sparkles", size, model.theme.fg).into_any_element(),
        Some(provider) => {
            let name = if provider.id == "droid" {
                "factory"
            } else {
                provider.id
            };
            let path = SharedString::from(format!("icons/provider-{name}.svg"));
            if provider.id == "claude" {
                gpui::img(path).size(size).into_any_element()
            } else {
                svg()
                    .path(path)
                    .size(size)
                    .text_color(model.theme.fg)
                    .into_any_element()
            }
        }
    }
}

fn provider_row(
    index: usize,
    provider: Option<Provider>,
    menu: &AiProviderMenu,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let configured = model
        .settings
        .ai
        .providers
        .get(menu.action.key())
        .map_or("", String::as_str);
    let selected = if configured.is_empty() {
        model.ai.installed.first().copied()
    } else {
        crate::ai::provider(configured)
    };
    let (id, title) = provider.map_or_else(
        || {
            (
                "",
                selected.map_or_else(
                    || "Auto".into(),
                    |provider| format!("Auto · {}", provider.name),
                ),
            )
        },
        |provider| {
            (
                provider.id,
                if model.ai.installed.contains(&provider) {
                    provider.name.into()
                } else {
                    format!("{} · Not installed", provider.name)
                },
            )
        },
    );
    popover::row(
        &model.theme,
        m,
        ("ai-provider-row", index),
        true,
        menu.highlighted == Some(index),
    )
    .debug_selector(move || format!("ai-provider-{id}"))
    .min_w(m.scaled(220.0))
    .button_interaction(
        cx.listener(move |model, _, window, cx| model.select_ai_provider(index, window, cx)),
    )
    .child(provider_icon(provider, model))
    .child(div().text_size(m.font_body()).child(title))
    .child(div().flex_1().min_w(m.spacing6()))
    .child(
        div()
            .opacity(if configured == id { 1.0 } else { 0.0 })
            .child(SymbolGlyph::new("checkmark", m.font_footnote(), model.theme.accent).bold()),
    )
    .into_any_element()
}

#[allow(
    clippy::too_many_lines,
    reason = "Declarative provider menu and project prompt editor"
)]
pub(crate) fn render_provider_menu(
    menu: &AiProviderMenu,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let project_name = model
        .state
        .project(menu.project)
        .map_or("", |project| project.name.as_str());
    let overridden = model
        .settings
        .ai
        .project_pr_prompts
        .get(&menu.project.to_string())
        .is_some_and(|prompt| !prompt.trim().is_empty());
    let mut view = popover::surface(theme, m)
        .id("ai-provider-menu")
        .debug_selector(|| "ai-provider-menu".into())
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .max_h((window.viewport_size().height - m.scaled(40.0)).max(px(0.0)))
        .overflow_y_scroll()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_action(cx.listener(|model, _: &menu::DismissMenu, window, cx| {
            if matches!(&model.overlay, Some(Overlay::AiProvider(menu)) if menu.prompt.is_some()) {
                model.cancel_project_pr_prompt(window, cx);
            } else {
                model.dismiss_overlay(cx);
            }
        }))
        .on_action(
            cx.listener(|model, _: &menu::HighlightNext, _, cx| model.move_ai_provider(true, cx)),
        )
        .on_action(cx.listener(|model, _: &menu::HighlightPrevious, _, cx| {
            model.move_ai_provider(false, cx);
        }))
        .on_action(
            cx.listener(|model, _: &menu::ConfirmHighlighted, window, cx| {
                model.confirm_ai_provider(window, cx);
            }),
        );
    if let Some(input) = &menu.prompt {
        view = view
            .w(m.scaled(380.0).min(window.viewport_size().width - px(16.0)))
            .gap(m.spacing4())
            .child(
                popover::header(theme, m)
                    .flex_col()
                    .items_start()
                    .gap(m.spacing1())
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Create PR Prompt"),
                    )
                    .child(
                        div()
                            .text_size(m.font_caption())
                            .text_color(theme.fg_muted)
                            .child(project_name.to_owned()),
                    ),
            )
            .child(controls::text_area(
                style,
                "ai-project-pr-prompt",
                input,
                Some(150.0),
            ))
            .child(
                div()
                    .text_size(m.font_caption())
                    .text_color(theme.fg_muted)
                    .child("This prompt overrides Settings → AI only for this project."),
            )
            .child(
                popover::footer(theme, m)
                    .child(
                        controls::button(
                            style,
                            "ai-project-prompt-reset",
                            "Use Global Prompt",
                            overridden,
                            cx.listener(|model, _, _, cx| model.save_project_pr_prompt(true, cx)),
                        )
                        .text_color(theme.accent),
                    )
                    .child(div().flex_1())
                    .child(controls::button(
                        style,
                        "ai-project-prompt-cancel",
                        "Cancel",
                        true,
                        cx.listener(|model, _, window, cx| {
                            model.cancel_project_pr_prompt(window, cx);
                        }),
                    ))
                    .child(controls::button(
                        style,
                        "ai-project-prompt-save",
                        "Save",
                        !input.read(cx).text().trim().is_empty(),
                        cx.listener(|model, _, _, cx| model.save_project_pr_prompt(false, cx)),
                    )),
            );
    } else {
        view = view
            .child(popover::header(theme, m).child(menu.action.settings_title()))
            .child(provider_row(0, None, menu, model, cx))
            .child(popover::divider(theme, m));
        for (index, provider) in PROVIDERS.iter().enumerate() {
            view = view.child(provider_row(index + 1, Some(*provider), menu, model, cx));
        }
        if menu.action == Action::CreatePullRequest {
            view = view.child(popover::divider(theme, m)).child(
                popover::row(
                    theme,
                    m,
                    "ai-project-prompt-edit",
                    true,
                    menu.highlighted == Some(PROVIDERS.len() + 1),
                )
                .debug_selector(|| "ai-project-prompt-edit".into())
                .h(m.scaled(44.0))
                .min_w(m.scaled(220.0))
                .button_interaction(cx.listener(|model, _, window, cx| {
                    model.edit_project_pr_prompt(window, cx);
                }))
                .child(SymbolGlyph::new("text.quote", m.icon_md(), theme.fg))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(m.spacing1())
                        .child("Edit Project Prompt…")
                        .child(
                            div()
                                .text_size(m.font_caption())
                                .text_color(theme.fg_muted)
                                .child(project_name.to_owned()),
                        ),
                )
                .child(div().flex_1().min_w(m.spacing6()))
                .when(overridden, |row| {
                    row.child(div().size(m.scaled(6.0)).rounded_full().bg(theme.accent))
                }),
            );
        }
    }
    view.into_any_element()
}
