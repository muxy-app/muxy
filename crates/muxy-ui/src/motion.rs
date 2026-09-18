use gpui::{Animation, AnimationExt as _, ElementId, Hsla, IntoElement, Pixels, Styled, div};
use std::time::Duration;

pub struct VisibilityMotion {
    clock: gpui::BackgroundExecutor,
    started: std::time::Instant,
    duration: Duration,
    from: f32,
    target: f32,
}

impl std::fmt::Debug for VisibilityMotion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VisibilityMotion")
            .field("duration", &self.duration)
            .field("from", &self.from)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl VisibilityMotion {
    pub fn showing(duration: Duration, clock: gpui::BackgroundExecutor) -> Self {
        Self {
            started: clock.now(),
            clock,
            duration,
            from: 0.0,
            target: 1.0,
        }
    }

    pub fn hide(&mut self) {
        self.from = self.value();
        self.target = 0.0;
        self.started = self.clock.now();
    }

    fn value(&self) -> f32 {
        let progress = (self.clock.now().duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32())
        .min(1.0);
        self.from + (self.target - self.from) * ease_in_out(progress)
    }

    pub fn sample(&self, window: &gpui::Window) -> f32 {
        if self.clock.now().duration_since(self.started) < self.duration {
            window.request_animation_frame();
        }
        self.value()
    }
}

fn ease_in_out(progress: f32) -> f32 {
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..16 {
        let time = (low + high) * 0.5;
        let inverse = 1.0 - time;
        let x = 3.0 * inverse * inverse * time * 0.42
            + 3.0 * inverse * time * time * 0.58
            + time * time * time;
        if x < progress {
            low = time;
        } else {
            high = time;
        }
    }
    let time = (low + high) * 0.5;
    3.0 * (1.0 - time) * time * time + time * time * time
}

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
