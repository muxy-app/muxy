use gpui::{Context, ParentElement, Styled, div};
use muxy_protocol::ExitReason;
use muxy_ui::tr;

use super::terminal::pane::PaneState;
use crate::model::AppModel;

pub(crate) fn label(state: PaneState) -> Option<String> {
    match state {
        PaneState::Live => None,
        PaneState::Connecting => Some(tr!("Connecting…").to_string()),
        PaneState::Disconnected => Some(tr!("Disconnected").to_string()),
        PaneState::Exited {
            unavailable: true, ..
        } => Some(tr!("Session exited · Saved output unavailable").to_string()),
        PaneState::Exited { reason, .. } => Some(
            match reason {
                Some(ExitReason::Exited(code)) => tr!("Session exited · Exit code %lld", code),
                Some(ExitReason::Signaled(signal)) => {
                    tr!("Session exited · Signal %lld", signal)
                }
                Some(
                    ExitReason::Ended | ExitReason::ServerStopped | ExitReason::Unrecognized(_),
                )
                | None => tr!("Session exited"),
            }
            .to_string(),
        ),
    }
}

pub(crate) fn status(model: &AppModel, cx: &mut Context<AppModel>) -> Option<gpui::Div> {
    let state = model.status(cx);
    if !matches!(state, PaneState::Exited { .. }) {
        return None;
    }
    let text = label(state)?;
    let m = model.metrics;
    let theme = &model.theme;
    Some(
        div()
            .flex()
            .items_center()
            .gap(m.spacing4())
            .text_color(theme.fg_muted)
            .text_size(m.font_footnote())
            .child(text),
    )
}
