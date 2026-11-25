use gpui::{FocusHandle, WeakFocusHandle, Window};

#[derive(Default)]
pub(crate) struct FocusOwner {
    next: Option<Target>,
}

enum Target {
    ActivePane,
    Return(WeakFocusHandle),
    Element(WeakFocusHandle),
}

pub(crate) enum Next {
    ActivePane,
    Element(FocusHandle),
}

impl FocusOwner {
    pub(crate) fn request_active_pane(&mut self) {
        if self.next.is_none() {
            self.next = Some(Target::ActivePane);
        }
    }

    pub(crate) fn request(&mut self, focus: &FocusHandle) {
        self.next = Some(Target::Element(focus.downgrade()));
    }

    pub(crate) fn place(&mut self, focus: &FocusHandle, window: &mut Window) {
        self.next = None;
        focus.focus(window);
    }

    pub(crate) fn wants_active_pane(&self) -> bool {
        matches!(self.next, Some(Target::ActivePane))
    }

    pub(crate) fn settle(&mut self) {
        self.next = None;
    }

    pub(crate) fn overlay_closed(&mut self, returns_to: Option<&FocusHandle>) {
        self.next = Some(match returns_to {
            Some(focus) => Target::Return(focus.downgrade()),
            None => Target::ActivePane,
        });
    }

    pub(crate) fn forget(&mut self, focus: &FocusHandle) {
        if matches!(
            &self.next,
            Some(Target::Return(other) | Target::Element(other)) if other == focus
        ) {
            self.next = Some(Target::ActivePane);
        }
    }

    pub(crate) fn take(&mut self, overlay_open: bool) -> Option<Next> {
        let next = self.next.take()?;
        if overlay_open && !matches!(next, Target::Element(_)) {
            self.next = Some(next);
            return None;
        }
        if let Target::Return(focus) | Target::Element(focus) = next
            && let Some(focus) = focus.upgrade()
        {
            return Some(Next::Element(focus));
        }
        if overlay_open {
            self.next = Some(Target::ActivePane);
            return None;
        }
        Some(Next::ActivePane)
    }
}
