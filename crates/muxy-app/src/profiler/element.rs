use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};

use super::{Metric, PROFILE, Profile, Span};

pub(crate) fn workspace(child: impl IntoElement) -> AnyElement {
    let child = child.into_any_element();
    let Some(profile) = PROFILE
        .get()
        .filter(|profile| profile.active.load(std::sync::atomic::Ordering::Relaxed))
    else {
        return child;
    };
    Measured { child, profile }.into_any_element()
}

struct Measured {
    child: AnyElement,
    profile: &'static Profile,
}

impl Measured {
    fn span(&self, metric: Metric) -> Span<'static> {
        Span(
            self.profile
                .active
                .load(std::sync::atomic::Ordering::Relaxed)
                .then(|| {
                    (
                        &self.profile.stats[metric as usize],
                        std::time::Instant::now(),
                    )
                }),
        )
    }
}

impl IntoElement for Measured {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Measured {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let _span = self.span(Metric::WorkspaceLayout);
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let _span = self.span(Metric::WorkspacePrepaint);
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut (),
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let _span = self.span(Metric::WorkspacePaint);
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
#[path = "element_tests.rs"]
mod tests;
