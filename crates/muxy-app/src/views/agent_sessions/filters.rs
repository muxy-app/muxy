use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_protocol::AgentProvider;
use muxy_ui::components::IconGlyph;
use muxy_ui::controls::{Style, button as text_button};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::{AppModel, icon_button, provider_icon};
use muxy_app_core::agent_sessions::{PROVIDERS, supported};
use muxy_ui::popover;

fn all_selected(model: &AppModel) -> bool {
    PROVIDERS
        .into_iter()
        .filter(|provider| supported(*provider))
        .all(|provider| {
            model
                .appearance
                .agent_sessions_providers
                .contains(&provider)
        })
}

pub(super) fn button(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let theme = &model.theme;
    let anchor = model.agent_sessions.anchor.clone();
    let filtered = !all_selected(model);
    div()
        .flex_none()
        .debug_selector(|| "session-provider-filter".into())
        .child(
            icon_button(
                "session-provider-filter",
                Icon::Filter,
                tr!("Filter providers"),
                if filtered {
                    theme.accent
                } else {
                    theme.fg_muted
                },
                model,
            )
            .on_click(
                cx.listener(|model, _, window, cx| model.toggle_session_providers(window, cx)),
            ),
        )
        .on_children_prepainted(move |bounds, _, _| anchor.set(bounds.first().copied()))
}

fn supported_providers() -> Vec<AgentProvider> {
    PROVIDERS
        .into_iter()
        .filter(|provider| supported(*provider))
        .collect()
}

pub(crate) fn providers(
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let mut menu = popover::surface(theme, m)
        .id("session-providers")
        .debug_selector(|| "session-providers".into())
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .w(m.scaled(240.0))
        .max_h((window.viewport_size().height - m.scaled(40.0)).max(px(0.0)))
        .overflow_y_scroll()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_action(
            cx.listener(|model, _: &super::super::menu::DismissMenu, _, cx| {
                model.dismiss_overlay(cx);
            }),
        )
        .on_action(
            cx.listener(|model, _: &super::super::menu::HighlightNext, _, cx| {
                model.agent_sessions.highlighted_provider =
                    (model.agent_sessions.highlighted_provider + 1) % supported_providers().len();
                cx.notify();
            }),
        )
        .on_action(
            cx.listener(|model, _: &super::super::menu::HighlightPrevious, _, cx| {
                let count = supported_providers().len();
                model.agent_sessions.highlighted_provider =
                    (model.agent_sessions.highlighted_provider + count - 1) % count;
                cx.notify();
            }),
        )
        .on_action(
            cx.listener(|model, _: &super::super::menu::ConfirmHighlighted, _, cx| {
                if let Some(provider) =
                    supported_providers().get(model.agent_sessions.highlighted_provider)
                {
                    model.toggle_session_provider(*provider, cx);
                }
            }),
        )
        .child(popover::header(theme, m).child(tr!("Include providers")));
    for (index, provider) in supported_providers().into_iter().enumerate() {
        menu = menu.child(provider_row(index, provider, model, cx));
    }
    menu.child(popover::divider(theme, m))
        .child(
            popover::row(theme, m, "session-unsupported", true, false)
                .text_color(theme.fg_muted)
                .child(IconGlyph::new(
                    if model.agent_sessions.show_unsupported {
                        Icon::ChevronDown
                    } else {
                        Icon::ChevronRight
                    },
                    m.icon_sm(),
                    theme.fg_dim,
                ))
                .child(tr!("Unavailable providers"))
                .on_click(cx.listener(|model, _, _, cx| {
                    model.agent_sessions.show_unsupported = !model.agent_sessions.show_unsupported;
                    cx.notify();
                })),
        )
        .when(model.agent_sessions.show_unsupported, |menu| {
            menu.child(unsupported(model))
        })
        .child(
            popover::footer(theme, m)
                .child(
                    text_button(
                        style,
                        "session-select-none",
                        muxy_ui::controls::Button::secondary(&tr!("Clear"))
                            .enabled(!model.appearance.agent_sessions_providers.is_empty()),
                        cx.listener(|model, _, _, cx| model.set_session_providers(Vec::new(), cx)),
                    )
                    .debug_selector(|| "session-select-none".into()),
                )
                .child(
                    text_button(
                        style,
                        "session-select-all",
                        muxy_ui::controls::Button::secondary(&tr!("Select all"))
                            .enabled(!all_selected(model)),
                        cx.listener(|model, _, _, cx| model.all_session_providers(cx)),
                    )
                    .debug_selector(|| "session-select-all".into()),
                ),
        )
        .into_any_element()
}

fn provider_row(
    index: usize,
    provider: AgentProvider,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> gpui::Stateful<Div> {
    let m = model.metrics;
    let theme = &model.theme;
    let selected = model
        .appearance
        .agent_sessions_providers
        .contains(&provider);
    let count = PROVIDERS
        .iter()
        .position(|candidate| *candidate == provider)
        .map_or(0, |index| model.agent_sessions.list.counts[index]);
    popover::row(
        theme,
        m,
        ("session-provider", index),
        true,
        model.agent_sessions.highlighted_provider == index,
    )
    .debug_selector(move || format!("session-provider-{index}"))
    .child(div().flex_none().w(m.icon_sm()).when(selected, |slot| {
        slot.child(IconGlyph::new(Icon::Check, m.icon_sm(), theme.accent))
    }))
    .child(provider_icon(provider, m.icon_sm(), model))
    .child(div().flex_1().child(provider.name()))
    .child(
        div()
            .text_size(m.font_footnote())
            .text_color(theme.fg_dim)
            .child(count.to_string()),
    )
    .on_click(cx.listener(move |model, _, _, cx| model.toggle_session_provider(provider, cx)))
}

fn unsupported(model: &AppModel) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
    div().children(
        PROVIDERS
            .into_iter()
            .enumerate()
            .filter(|(_, provider)| !supported(*provider))
            .map(|(index, provider)| {
                popover::row(
                    theme,
                    m,
                    ("unsupported-session-provider", index),
                    false,
                    false,
                )
                .child(div().flex_none().w(m.icon_sm()))
                .child(provider_icon(provider, m.icon_sm(), model))
                .child(div().flex_1().child(provider.name()))
                .child(
                    div()
                        .text_size(m.font_footnote())
                        .child(tr!("Not supported")),
                )
            }),
    )
}
