//! Who gets the keyboard next.

use gpui::{FocusHandle, WeakFocusHandle, Window};

/// Where focus goes next. Model code asks here, often without a window, and
/// the workspace render moves focus.
#[derive(Default)]
pub(crate) struct FocusOwner {
    next: Option<Target>,
}

enum Target {
    /// The active pane, or the workspace when there is none. It waits for
    /// any open overlay to close.
    ActivePane,
    /// What a closed overlay gives the keyboard back to. Like the active
    /// pane, it waits for any open overlay to close.
    Return(WeakFocusHandle),
    /// One element, such as an overlay's input, focused at once.
    Element(WeakFocusHandle),
}

/// What render focuses now.
pub(crate) enum Next {
    ActivePane,
    Element(FocusHandle),
}

impl FocusOwner {
    /// Gives the keyboard to the active pane once no overlay is open. An
    /// element asked for by name, or an overlay's return, still goes first.
    pub(crate) fn request_active_pane(&mut self) {
        if self.next.is_none() {
            self.next = Some(Target::ActivePane);
        }
    }

    /// Gives the keyboard to `focus` at the next render.
    pub(crate) fn request(&mut self, focus: &FocusHandle) {
        self.next = Some(Target::Element(focus.downgrade()));
    }

    /// Gives the keyboard to `focus` now, in place of any request.
    pub(crate) fn place(&mut self, focus: &FocusHandle, window: &mut Window) {
        self.next = None;
        focus.focus(window);
    }

    /// Drops any request, as the caller has just placed focus itself.
    pub(crate) fn settle(&mut self) {
        self.next = None;
    }

    /// An overlay closed: the keyboard goes back to what it names, or to the
    /// active pane, in place of requests made while it was open.
    pub(crate) fn overlay_closed(&mut self, returns_to: Option<&FocusHandle>) {
        self.next = Some(match returns_to {
            Some(focus) => Target::Return(focus.downgrade()),
            None => Target::ActivePane,
        });
    }

    /// `focus` is going away: a request for it goes to the active pane
    /// instead.
    pub(crate) fn forget(&mut self, focus: &FocusHandle) {
        if matches!(
            &self.next,
            Some(Target::Return(other) | Target::Element(other)) if other == focus
        ) {
            self.next = Some(Target::ActivePane);
        }
    }

    /// The request to apply now. One that waits for overlays stays while one
    /// is open; one for an element that no longer exists goes to the active
    /// pane.
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
