use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Bounds, Context, Pixels, Point, Task};
use muxy_ui::motion::VisibilityMotion;

use super::AppModel;

pub(crate) const DWELL: Duration = Duration::from_millis(100);
pub(crate) const GRACE: Duration = Duration::from_millis(300);
pub(crate) const SLIDE: Duration = Duration::from_millis(220);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Phase {
    #[default]
    Hidden,
    Arming,
    Shown,
    Leaving,
    Pinned,
}

#[derive(Default)]
pub(crate) struct SidebarPeek {
    phase: Phase,
    pub(crate) motion: Option<VisibilityMotion>,
    pub(crate) edge: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub(crate) reach: Rc<Cell<Option<Bounds<Pixels>>>>,
    pointer: Option<Point<Pixels>>,
    pressed: bool,
    dismissed: bool,
    window_hovered: bool,
    timer: Option<Task<()>>,
}

impl AppModel {
    pub(crate) fn sidebar_auto_hides(&self) -> bool {
        self.appearance.sidebar_auto_hide && self.sidebar_natural_width() > 0.0
    }

    pub(crate) fn sidebar_docks_expanded(&self) -> bool {
        self.appearance.sidebar_expanded && !self.appearance.sidebar_auto_hide
    }

    pub(crate) fn sidebar_peeking(&self) -> bool {
        self.sidebar_auto_hides() && self.sidebar_peek.motion.is_some()
    }

    pub(crate) fn sidebar_peek_open(&self) -> bool {
        self.sidebar_auto_hides()
            && matches!(
                self.sidebar_peek.phase,
                Phase::Shown | Phase::Leaving | Phase::Pinned
            )
    }

    pub(crate) fn sidebar_shown(&self) -> bool {
        if self.appearance.sidebar_auto_hide {
            self.sidebar_peek_open()
        } else {
            self.appearance.sidebar_expanded
        }
    }

    pub(crate) fn toggle_sidebar_peek(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_peek_open() {
            self.dismiss_peek(cx);
        } else {
            self.show_peek(cx);
            self.sidebar_peek.phase = Phase::Pinned;
        }
    }

    pub(crate) fn track_sidebar_peek(
        &mut self,
        pointer: Option<Point<Pixels>>,
        pressed: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.sidebar_auto_hides() {
            return;
        }
        self.sidebar_peek.pointer = pointer;
        self.sidebar_peek.pressed = pressed;
        let at_edge = self.pointer_within(&self.sidebar_peek.edge);
        let within = self.pointer_within(&self.sidebar_peek.reach);
        match self.sidebar_peek.phase {
            Phase::Hidden
                if self.sidebar_peek.motion.is_some() && within && !self.sidebar_peek.dismissed =>
            {
                self.show_peek(cx);
            }
            Phase::Hidden if at_edge && self.window_active && !self.coverage().modal() => {
                self.sidebar_peek.phase = Phase::Arming;
                self.schedule_peek(DWELL, cx);
            }
            Phase::Arming if !at_edge => {
                self.sidebar_peek.phase = Phase::Hidden;
                self.settle_peek(cx);
            }
            Phase::Shown if !within => {
                self.sidebar_peek.phase = Phase::Leaving;
                self.schedule_peek(GRACE, cx);
            }
            Phase::Leaving | Phase::Pinned if within => {
                self.sidebar_peek.phase = Phase::Shown;
                self.sidebar_peek.timer = None;
            }
            _ => {}
        }
    }

    pub(crate) fn press_outside_sidebar_peek(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let below_title = self
            .sidebar_peek
            .reach
            .get()
            .is_some_and(|reach| position.y >= reach.top() && !reach.contains(&position));
        if self.sidebar_peek.phase == Phase::Pinned && below_title {
            self.dismiss_peek(cx);
        }
    }

    pub(crate) fn sidebar_pointer_left(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_peek.pointer.is_some() {
            self.track_sidebar_peek(None, false, cx);
        }
    }

    pub(crate) fn note_window_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let left = self.sidebar_peek.window_hovered && !hovered;
        self.sidebar_peek.window_hovered = hovered;
        if left {
            self.sidebar_pointer_left(cx);
        }
    }

    pub(crate) fn dismiss_sidebar_peek(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_peek.phase != Phase::Hidden {
            self.dismiss_peek(cx);
        }
    }

    pub(crate) fn reset_sidebar_peek(&mut self) {
        self.sidebar_peek.phase = Phase::Hidden;
        self.sidebar_peek.motion = None;
        self.sidebar_peek.timer = None;
    }

    fn pointer_within(&self, area: &Cell<Option<Bounds<Pixels>>>) -> bool {
        self.sidebar_peek
            .pointer
            .zip(area.get())
            .is_some_and(|(pointer, bounds)| bounds.contains(&pointer))
    }

    fn peek_held(&self) -> bool {
        self.sidebar_peek.pressed || self.coverage().modal() || self.layout_drag_pending()
    }

    fn schedule_peek(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.sidebar_peek.timer = Some(cx.spawn(async move |model, cx| {
            cx.background_executor().timer(delay).await;
            let _ = model.update(cx, Self::peek_elapsed);
        }));
    }

    fn peek_elapsed(&mut self, cx: &mut Context<Self>) {
        self.sidebar_peek.timer = None;
        if !self.sidebar_auto_hides() {
            self.reset_sidebar_peek();
            cx.notify();
            return;
        }
        match self.sidebar_peek.phase {
            Phase::Arming if self.pointer_within(&self.sidebar_peek.edge) => self.show_peek(cx),
            Phase::Arming => {
                self.sidebar_peek.phase = Phase::Hidden;
                self.settle_peek(cx);
            }
            Phase::Leaving if self.pointer_within(&self.sidebar_peek.reach) => {
                self.sidebar_peek.phase = Phase::Shown;
            }
            Phase::Leaving if self.peek_held() => self.schedule_peek(GRACE, cx),
            Phase::Leaving => self.hide_peek(cx),
            Phase::Hidden => {
                self.sidebar_peek.motion = None;
                cx.notify();
            }
            Phase::Shown | Phase::Pinned => {}
        }
    }

    fn show_peek(&mut self, cx: &mut Context<Self>) {
        self.sidebar_peek.phase = Phase::Shown;
        self.sidebar_peek.dismissed = false;
        self.sidebar_peek.timer = None;
        match &mut self.sidebar_peek.motion {
            Some(motion) => motion.show(),
            None => {
                self.sidebar_peek.motion = Some(VisibilityMotion::showing(
                    SLIDE,
                    cx.background_executor().clone(),
                ));
            }
        }
        cx.notify();
    }

    fn dismiss_peek(&mut self, cx: &mut Context<Self>) {
        self.hide_peek(cx);
        self.sidebar_peek.dismissed = true;
    }

    fn hide_peek(&mut self, cx: &mut Context<Self>) {
        self.sidebar_peek.phase = Phase::Hidden;
        if self.quick.reduce_motion() {
            self.sidebar_peek.motion = None;
            self.sidebar_peek.timer = None;
        } else if let Some(motion) = &mut self.sidebar_peek.motion {
            motion.hide();
            self.schedule_peek(SLIDE, cx);
        }
        cx.notify();
    }

    fn settle_peek(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_peek.motion.is_some() {
            self.schedule_peek(SLIDE, cx);
        } else {
            self.sidebar_peek.timer = None;
        }
    }
}
