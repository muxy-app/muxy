//! Panels that float freely over the window, side by side: the detached
//! agents panel, the detached agent sessions panel, and detached extension
//! panels. Clicking one brings it to the front.

use gpui::{Context, Pixels, Point, Size, Window};
use muxy_ui::floating::{Edges, FloatingDrag, FloatingFrame, FloatingStack, SizeLimits};
use muxy_ui::panel::PanelId;

use super::AppModel;

const SESSIONS_FRAME: [f32; 4] = [120.0, 120.0, 360.0, 420.0];
const SESSIONS_MIN: (f32, f32) = (260.0, 200.0);

#[derive(Default)]
pub(crate) struct Floating {
    pub(crate) stack: FloatingStack<PanelId>,
    /// The panel being moved or resized by the pointer.
    pub(crate) drag: Option<(PanelId, FloatingDrag)>,
}

impl AppModel {
    /// Window pixels per frame point of a floating panel: the app's zoom
    /// for built-in panels, and none for extension panels, whose frames are
    /// in the pixels their pages lay out in.
    pub(crate) fn floating_scale(&self, id: &PanelId) -> f32 {
        if self.webviews.panels.contains_key(id) {
            1.0
        } else {
            self.metrics.multiplier()
        }
    }

    /// The window's size in a floating panel's frame points.
    pub(crate) fn floating_area_for(&self, id: &PanelId, window: &Window) -> Size<f32> {
        let scale = self.floating_scale(id);
        let viewport = window.viewport_size();
        Size {
            width: f32::from(viewport.width) / scale,
            height: f32::from(viewport.height) / scale,
        }
    }

    /// The window's size in built-in floating panels' frame points.
    pub(crate) fn floating_area(&self, window: &Window) -> Size<f32> {
        let scale = self.metrics.multiplier();
        let viewport = window.viewport_size();
        Size {
            width: f32::from(viewport.width) / scale,
            height: f32::from(viewport.height) / scale,
        }
    }

    /// Every floating panel shown now, bottom first.
    pub(crate) fn floating_members(&self) -> Vec<PanelId> {
        let mut members = Vec::new();
        if self.agents_panel_placement() == super::agents_panel::Placement::Floating
            && self.appearance.agents_panel_visible
        {
            members.push(PanelId::new(super::agents_panel::PANEL));
        }
        if self.agent_sessions_detached_shown() {
            members.push(PanelId::new(super::agent_sessions::PANEL));
        }
        members.extend(self.detached_webview_panels());
        self.floating.stack.sort(&mut members, |id| id);
        members
    }

    /// Puts panels that just appeared on top and forgets hidden ones, so the
    /// stack always matches what shows.
    pub(crate) fn sync_floating_stack(&mut self) {
        let members = self.floating_members();
        for member in &members {
            if self.floating.stack.rank(member).is_none() {
                self.floating.stack.raise(member);
            }
        }
        self.floating.stack.retain(|id| members.contains(id));
        if self
            .floating
            .drag
            .as_ref()
            .is_some_and(|(id, _)| !members.contains(id))
        {
            self.floating.drag = None;
        }
    }

    pub(crate) fn raise_floating(&mut self, id: &PanelId, cx: &mut Context<Self>) {
        if self.floating.stack.raise(id) {
            cx.notify();
        }
    }

    pub(crate) fn agent_sessions_detached_shown(&self) -> bool {
        self.appearance.agent_sessions_visible && self.appearance.agent_sessions_detached
    }

    fn agent_sessions_limits() -> SizeLimits {
        SizeLimits::at_least(SESSIONS_MIN.0, SESSIONS_MIN.1)
    }

    /// The detached agent sessions panel's frame, inside the window.
    pub(crate) fn agent_sessions_frame(&self, window: &Window) -> FloatingFrame {
        let frame = self
            .appearance
            .agent_sessions_frame
            .map(FloatingFrame::from_array)
            .filter(|frame| frame.is_finite())
            .unwrap_or(FloatingFrame::from_array(SESSIONS_FRAME));
        frame.clamped(self.floating_area(window), Self::agent_sessions_limits())
    }

    /// The frame and size limits of a floating panel moved by the shared
    /// drag, inside the window.
    fn floating_geometry(
        &self,
        id: &PanelId,
        window: &Window,
    ) -> Option<(FloatingFrame, SizeLimits)> {
        if id.as_str() == super::agent_sessions::PANEL {
            return Some((
                self.agent_sessions_frame(window),
                Self::agent_sessions_limits(),
            ));
        }
        self.detached_webview_geometry(id, window)
    }

    fn set_floating_frame(&mut self, id: &PanelId, frame: FloatingFrame) {
        if id.as_str() == super::agent_sessions::PANEL {
            self.appearance.agent_sessions_frame = Some(frame.to_array());
            return;
        }
        self.set_detached_webview_frame(id, frame);
    }

    /// Starts moving a floating panel, or resizing it by `edges`.
    pub(crate) fn begin_floating_drag(
        &mut self,
        id: &PanelId,
        edges: Option<Edges>,
        pointer: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some((frame, _)) = self.floating_geometry(id, window) else {
            return;
        };
        let drag = match edges {
            Some(edges) => FloatingDrag::resizing(pointer, frame, edges),
            None => FloatingDrag::moving(pointer, frame),
        };
        self.floating.drag = Some((id.clone(), drag));
        self.raise_floating(id, cx);
        cx.notify();
    }

    pub(crate) fn move_floating_drag(
        &mut self,
        pointer: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, drag)) = self.floating.drag.clone() else {
            return;
        };
        let Some((_, limits)) = self.floating_geometry(&id, window) else {
            self.floating.drag = None;
            return;
        };
        let frame = drag.frame_at(
            pointer,
            self.floating_scale(&id),
            self.floating_area_for(&id, window),
            limits,
        );
        self.set_floating_frame(&id, frame);
        cx.notify();
    }

    /// Ends the drag, keeping where the panel ended up.
    pub(crate) fn finish_floating_drag(&mut self, cx: &mut Context<Self>) {
        let Some((id, _)) = self.floating.drag.take() else {
            return;
        };
        if id.as_str() == super::agent_sessions::PANEL {
            if self.appearance.agent_sessions_frame != self.settings.appearance.agent_sessions_frame
            {
                self.save_appearance(cx);
            }
        } else {
            self.save_detached_webview_frame(&id, cx);
        }
        cx.notify();
    }

    /// Pops the agent sessions panel out to float, or docks it back.
    pub(crate) fn set_agent_sessions_detached(
        &mut self,
        detached: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.appearance.agent_sessions_detached = detached;
        self.appearance.agent_sessions_visible = true;
        if detached && self.appearance.agent_sessions_frame.is_none() {
            self.appearance.agent_sessions_frame = Some(SESSIONS_FRAME);
        }
        self.agent_sessions.resize.end();
        self.save_appearance(cx);
        self.sync_agent_sessions(cx);
        if detached {
            self.raise_floating(&PanelId::new(super::agent_sessions::PANEL), cx);
        }
        if let Some(input) = &self.agent_sessions.input {
            self.focus_owner
                .place(&gpui::Focusable::focus_handle(input, cx), window);
        }
        cx.notify();
    }
}
