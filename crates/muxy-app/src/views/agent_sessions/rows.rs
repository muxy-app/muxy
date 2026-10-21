use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, Focusable, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div,
};
use muxy_ui::tr;

use super::{AppModel, provider_icon};
use crate::model::agent_sessions::{Entry, Recency};
use muxy_app_core::agent_sessions::Session;

pub(super) fn list(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let owner = cx.weak_entity();
    let list = gpui::list(
        model.agent_sessions.list.state.clone(),
        move |index, window, cx| {
            owner
                .update(cx, |model, cx| {
                    match model.agent_sessions.list.entries.get(index).cloned() {
                        Some(Entry::Section(recency)) => section(recency, model).into_any_element(),
                        Some(Entry::Session { index, key }) => {
                            let focused = model
                                .agent_sessions
                                .input
                                .as_ref()
                                .is_some_and(|input| input.focus_handle(cx).is_focused(window));
                            let selected = focused
                                && model.agent_sessions.list.selected.as_ref() == Some(&key);
                            row(
                                index,
                                model.agent_sessions.listing.sessions[index].clone(),
                                selected,
                                model,
                                cx,
                            )
                        }
                        None => div().into_any_element(),
                    }
                })
                .unwrap_or_else(|_| div().into_any_element())
        },
    )
    .size_full();
    div()
        .relative()
        .size_full()
        .child(list)
        .child(
            div().absolute().top_0().right_0().bottom_0().child(
                model
                    .agent_sessions
                    .list
                    .scrollbar
                    .element(&model.agent_sessions.list.state, model.theme.fg_dim),
            ),
        )
        .into_any_element()
}

fn section(recency: Recency, model: &AppModel) -> Div {
    let m = model.metrics;
    let label = match recency {
        Recency::Recent => tr!("Last 24 hours"),
        Recency::Week => tr!("Previous 7 days"),
        Recency::Older => tr!("Older sessions"),
    };
    div()
        .px(m.spacing2())
        .pt(m.spacing3())
        .pb(m.spacing1())
        .child(
            div()
                .px(m.spacing3())
                .text_size(m.font_micro())
                .font_weight(FontWeight::BOLD)
                .text_color(model.theme.fg_dim)
                .child(label.to_uppercase()),
        )
}

fn age(session: &Session) -> SharedString {
    age_label(session.modified.elapsed().unwrap_or_default().as_secs())
}

pub(crate) fn age_label(age: u64) -> SharedString {
    match age {
        0..60 => tr!("Just now"),
        60..3600 => tr!("%lldm ago", age / 60),
        3600..86_400 => tr!("%lldh ago", age / 3600),
        _ => tr!("%lldd ago", age / 86_400),
    }
}

fn row(
    index: usize,
    session: Session,
    selected: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let title: SharedString = if session.title == session.id {
        tr!("Untitled session")
    } else {
        session.title.clone().into()
    };
    let tooltip = super::super::status_bar::tooltip(
        format!(
            "{title}\n{} · {}\n{}",
            session.provider.name(),
            session.id,
            tr!("Resume in a new terminal")
        ),
        model,
    );
    div()
        .px(m.spacing2())
        .pb(m.scaled(1.0))
        .child(
            div()
                .id(("agent-session", index))
                .debug_selector(move || format!("agent-session-{index}"))
                .flex()
                .items_center()
                .gap(m.spacing3())
                .px(m.spacing3())
                .py(m.spacing2())
                .w_full()
                .min_w_0()
                .rounded(m.radius_md())
                .cursor_pointer()
                .when(selected, |row| row.bg(theme.hover))
                .hover(|row| row.bg(theme.hover))
                .tooltip(tooltip)
                .child(
                    div()
                        .flex_none()
                        .child(provider_icon(session.provider, m.icon_sm(), model)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .debug_selector(move || format!("agent-session-title-{index}"))
                                .text_ellipsis()
                                .line_clamp(1)
                                .text_size(m.font_body())
                                .text_color(theme.fg)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_ellipsis()
                                .line_clamp(1)
                                .text_size(m.font_footnote())
                                .text_color(theme.fg_muted)
                                .child(format!("{} · {}", session.provider.name(), age(&session))),
                        ),
                )
                .on_click(
                    cx.listener(move |model, _, _, cx| model.resume_agent_session(&session, cx)),
                ),
        )
        .into_any_element()
}
