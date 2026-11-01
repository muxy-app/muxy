//! Who gets the keyboard next.

use gpui::{FocusHandle, WeakFocusHandle, Window};

/// Where focus goes next. Model code asks here, often without a window, and
/// the workspace render moves focus.
#[derive(Default)]
pub(crate) struct FocusOwner {
    next: Option<Target>,
    /// What the open overlay gives the keyboard back to when it closes,
    /// instead of the active pane.
    overlay_return: Option<WeakFocusHandle>,
}

enum Target {
    /// The active pane, or the workspace when there is none. It waits for
    /// any open overlay to close.
    ActivePane,
    /// One element, such as an overlay's input.
    Element(WeakFocusHandle),
}

/// What render focuses now.
pub(crate) enum Next {
    ActivePane,
    Element(FocusHandle),
}

impl FocusOwner {
    /// Gives the keyboard to the active pane once no overlay is open. An
    /// element asked for by name still goes first.
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

    /// What the overlay being opened gives the keyboard back to.
    pub(crate) fn return_after_overlay(&mut self, focus: Option<&FocusHandle>) {
        self.overlay_return = focus.map(FocusHandle::downgrade);
    }

    /// The open overlay closed: the keyboard goes back to what it named, or
    /// to the active pane.
    pub(crate) fn overlay_closed(&mut self) {
        match self.overlay_return.take() {
            Some(focus) => self.next = Some(Target::Element(focus)),
            None => self.request_active_pane(),
        }
    }

    /// `focus` is going away or out of sight: nothing gives it the keyboard
    /// back, and a request for it goes to the active pane instead.
    pub(crate) fn forget(&mut self, focus: &FocusHandle) {
        if self
            .overlay_return
            .as_ref()
            .is_some_and(|other| other == focus)
        {
            self.overlay_return = None;
        }
        if matches!(&self.next, Some(Target::Element(other)) if other == focus) {
            self.next = Some(Target::ActivePane);
        }
    }

    /// The request to apply now. One for the active pane keeps waiting while
    /// an overlay is open, as does one for an element that no longer exists.
    pub(crate) fn take(&mut self, overlay_open: bool) -> Option<Next> {
        if let Target::Element(focus) = self.next.take()?
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
