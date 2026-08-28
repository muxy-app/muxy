use gpui::{Animation, AnimationExt as _, ElementId, Hsla, IntoElement, Pixels, Styled, div};
use std::time::Duration;

pub fn timed_progress_fill(
    id: impl Into<ElementId>,
    duration: Duration,
    color: Hsla,
    radius: Pixels,
) -> impl IntoElement {
    div()
        .absolute()
        .left(gpui::px(0.0))
        .top(gpui::px(0.0))
        .bottom(gpui::px(0.0))
        .rounded(radius)
        .bg(color)
        .with_animation(id, Animation::new(duration), |fill, delta| {
            fill.w(gpui::relative(delta))
        })
}
