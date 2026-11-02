//! What accessibility clients read beyond what's on screen.

use gpui::accesskit::{Live, Node, Role};
use gpui::{
    AccessibleAction, AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId,
    InspectorElementId, InteractiveElement, IntoElement, LayoutId, ParentElement, Pixels,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};

/// A button only accessibility clients find, for an action the pointer reaches
/// by hovering or right-clicking. It never draws, and it's one point wide
/// because clients refuse to press a control with no bounds.
pub fn hidden_button(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    on_press: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .role(Role::Button)
        .accessibility_id(id)
        .aria_label(name)
        .absolute()
        .size(px(1.0))
        .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
            on_press(window, cx);
        })
}

/// Text accessibility clients can read where it stands, such as an empty state.
pub fn label(id: impl Into<ElementId>, text: impl Into<SharedString>) -> AnyElement {
    gpui::Text::new(id.into(), text.into()).into_any_element()
}

/// The element's text, announced when it appears and when it changes. Errors
/// interrupt what's being read; anything else waits its turn.
pub fn announced(
    element: Stateful<Div>,
    text: impl Into<SharedString>,
    is_problem: bool,
) -> LiveRegion {
    let role = if is_problem {
        Role::Alert
    } else {
        Role::Status
    };
    LiveRegion {
        element: element.role(role).aria_label(text),
        live: if is_problem {
            Live::Assertive
        } else {
            Live::Polite
        },
    }
}

pub struct LiveRegion {
    element: Stateful<Div>,
    live: Live,
}

impl std::fmt::Debug for LiveRegion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiveRegion")
            .field("live", &self.live)
            .finish_non_exhaustive()
    }
}

impl LiveRegion {
    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.element = self.element.child(child);
        self
    }
}

impl IntoElement for LiveRegion {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for LiveRegion {
    type RequestLayoutState = <Stateful<Div> as Element>::RequestLayoutState;
    type PrepaintState = <Stateful<Div> as Element>::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.element)
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        Element::source_location(&self.element)
    }

    fn a11y_role(&self) -> Option<Role> {
        self.element.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut Node) {
        self.element.write_a11y_info(node);
        node.set_live(self.live);
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.element.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.element
            .prepaint(id, inspector_id, bounds, request_layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.element.paint(
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_problem_interrupts_and_anything_else_waits() {
        let problem = announced(div().id("problem"), "Failed", true);
        let mut node = Node::new(Role::Alert);
        problem.write_a11y_info(&mut node);
        assert_eq!(problem.a11y_role(), Some(Role::Alert));
        assert_eq!(node.live(), Some(Live::Assertive));
        assert_eq!(node.label(), Some("Failed"));

        let status = announced(div().id("status"), "Saved", false);
        let mut node = Node::new(Role::Status);
        status.write_a11y_info(&mut node);
        assert_eq!(status.a11y_role(), Some(Role::Status));
        assert_eq!(node.live(), Some(Live::Polite));
    }

    #[test]
    fn a_hidden_button_can_only_be_pressed_by_a_client() {
        let button = hidden_button("row-actions", "Actions for gamma", |_, _| {});
        let mut node = Node::new(Role::Button);
        button.write_a11y_info(&mut node);
        assert_eq!(button.a11y_role(), Some(Role::Button));
        assert_eq!(node.label(), Some("Actions for gamma"));
        assert_eq!(node.author_id(), Some("row-actions"));
        assert!(node.supports_action(AccessibleAction::Click));
    }
}
