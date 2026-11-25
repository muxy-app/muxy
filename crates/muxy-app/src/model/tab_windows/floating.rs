use gpui::{Context, Size, Window};
use muxy_app_core::{FloatScope, PaneId, Project, TabId, TabWindow, TabWindowId};
use muxy_ui::floating::{FloatingFrame, SizeLimits};
use muxy_ui::panel::PanelId;

use super::{AppModel, Surface};

const PANEL_PREFIX: &str = "muxy.floating-tab.";
const DEFAULT_SIZE: (f32, f32) = (560.0, 340.0);
const MIN_SIZE: (f32, f32) = (300.0, 160.0);
const CASCADE: f32 = 28.0;
const CASCADE_STEPS: usize = 6;

pub(crate) fn panel(id: TabWindowId) -> PanelId {
    PanelId::new(format!("{PANEL_PREFIX}{id}"))
}

pub(crate) fn panel_float(panel: &PanelId) -> Option<TabWindowId> {
    panel.as_str().strip_prefix(PANEL_PREFIX)?.parse().ok()
}

pub(crate) fn float_limits() -> SizeLimits {
    SizeLimits::at_least(MIN_SIZE.0, MIN_SIZE.1)
}

impl AppModel {
    pub(crate) fn is_float(&self, id: TabWindowId) -> bool {
        self.state
            .tab_window(id)
            .is_some_and(|(_, _, window)| window.is_floating())
    }

    pub(crate) fn pane_floats(&self, pane: PaneId) -> bool {
        self.state
            .pane_window(pane)
            .is_some_and(|id| self.is_float(id))
    }

    fn float_in_scope(&self, project: &Project, window: &TabWindow) -> bool {
        let Some(floating) = window.floating() else {
            return false;
        };
        !self.state.floats_hidden()
            && (floating.scope == FloatScope::AllProjects
                || project.id == self.state.current_project().id)
    }

    fn float_shows(&self, project: &Project, window: &TabWindow) -> bool {
        self.float_in_scope(project, window)
            && window
                .floating()
                .is_some_and(|floating| !floating.minimized)
    }

    pub(crate) fn minimized_floats(&self) -> Vec<TabWindowId> {
        self.state
            .floating_tabs()
            .filter(|(project, _, window)| {
                self.float_in_scope(project, window)
                    && window.floating().is_some_and(|floating| floating.minimized)
            })
            .map(|(_, _, window)| window.id)
            .collect()
    }

    pub(crate) fn float_shown(&self, id: TabWindowId) -> bool {
        self.state
            .tab_window(id)
            .is_some_and(|(project, _, window)| self.float_shows(project, window))
    }

    pub(crate) fn shown_floats(&self) -> Vec<TabWindowId> {
        self.state
            .floating_tabs()
            .filter(|(project, _, window)| self.float_shows(project, window))
            .map(|(_, _, window)| window.id)
            .collect()
    }

    pub(crate) fn shows_page(&self, pane: PaneId) -> bool {
        self.state.pane(pane).is_some_and(|pane| {
            !matches!(pane.content, muxy_app_core::PaneContent::Terminal { .. })
        })
    }

    pub(crate) fn tab_surface(&self, tab: TabId, window: &Window) -> Surface {
        self.tab(tab)
            .and_then(|tab| tab.window.as_ref())
            .filter(|tab_window| tab_window.is_floating() && self.float_shown(tab_window.id))
            .map_or_else(
                || self.surface_of(window),
                |tab_window| Surface::Tab(tab_window.id),
            )
    }

    pub(crate) fn focused_float(&self, window: &Window, cx: &gpui::App) -> Option<TabWindowId> {
        self.shown_floats().into_iter().find(|id| {
            self.state.tab_window(*id).is_some_and(|(_, tab, _)| {
                tab.panes.iter().any(|pane| {
                    self.grids
                        .get(&pane.id)
                        .is_some_and(|view| view.view.read(cx).focus.contains_focused(window, cx))
                })
            })
        })
    }

    pub(crate) fn key_float(&self) -> Option<TabWindowId> {
        self.tab_windows.key.filter(|id| self.is_float(*id))
    }

    pub(crate) fn float_scope(&self, id: TabWindowId) -> Option<FloatScope> {
        self.state
            .tab_window(id)
            .and_then(|(_, _, window)| window.floating())
            .map(|floating| floating.scope)
    }

    pub(crate) fn float_frame(&self, id: TabWindowId, window: &Window) -> Option<FloatingFrame> {
        let floating = self.state.tab_window(id)?.2.floating()?;
        let area = self.floating_area(window);
        let frame = floating
            .frame
            .map(FloatingFrame::from_array)
            .filter(|frame| frame.is_finite())
            .unwrap_or_else(|| self.default_float_frame(id, area));
        Some(frame.clamped(area, float_limits()))
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "The step is below CASCADE_STEPS"
    )]
    fn default_float_frame(&self, id: TabWindowId, area: Size<f32>) -> FloatingFrame {
        let step = self
            .state
            .floating_tabs()
            .position(|(_, _, window)| window.id == id)
            .unwrap_or_default()
            % CASCADE_STEPS;
        let width = DEFAULT_SIZE.0.min(area.width * 0.7);
        let height = DEFAULT_SIZE.1.min(area.height * 0.6);
        let offset = CASCADE * step as f32;
        FloatingFrame::new(
            (area.width - width) / 2.0 + offset,
            (area.height - height) / 3.0 + offset,
            width,
            height,
        )
    }

    pub(crate) fn place_float(&mut self, id: TabWindowId, window: &Window) {
        let placed = self
            .state
            .tab_window(id)
            .and_then(|(_, _, tab_window)| tab_window.floating())
            .is_some_and(|floating| floating.frame.is_some());
        if placed {
            return;
        }
        if let Some(frame) = self.float_frame(id, window) {
            let _ = self.state.set_float_frame(id, frame.to_array());
        }
    }

    pub(crate) fn set_float_frame(&mut self, id: TabWindowId, frame: FloatingFrame) {
        let _ = self.state.set_float_frame(id, frame.to_array());
    }

    pub(crate) fn float_pane(&mut self, pane: PaneId, scope: FloatScope, cx: &mut Context<Self>) {
        let from_window = matches!(self.surface(), Surface::Tab(window) if !self.is_float(window));
        let floated = self.apply_move(
            |state| {
                let id = state.float_pane(pane, scope)?;
                state.set_floats_hidden(false);
                Ok((id, Vec::new()))
            },
            cx,
        );
        if let Some(id) = floated {
            if from_window {
                self.raise_main_window(cx);
            }
            self.reveal_float(id, cx);
        }
    }

    pub(crate) fn float_active_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.active_pane().filter(|pane| !self.pane_floats(*pane)) {
            self.float_pane(pane, FloatScope::Project, cx);
        }
    }

    pub(crate) fn enter_float(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        self.tab_windows.key = Some(id);
        self.tab_windows.request_focus(id);
        self.raise_floating(&panel(id), cx);
        cx.notify();
    }

    pub(crate) fn minimize_float(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        if !self.float_shown(id) || self.state.set_float_minimized(id, true).is_err() {
            return;
        }
        self.changed(cx);
        self.sync_floating_stack();
        self.focus_owner.request_active_pane();
    }

    pub(crate) fn minimize_pane_float(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if let Some(id) = self.state.pane_window(pane).filter(|id| self.is_float(*id)) {
            self.minimize_float(id, cx);
        }
    }

    pub(crate) fn reveal_float(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        let Some((project, _, window)) = self.state.tab_window(id) else {
            return;
        };
        let elsewhere = window
            .floating()
            .is_some_and(|floating| floating.scope == FloatScope::Project)
            && project.id != self.state.current_project().id;
        let project = project.id;
        if self.state.set_float_minimized(id, false).is_err() {
            return;
        }
        self.state.set_floats_hidden(false);
        if elsewhere {
            self.select_project(project, cx);
        }
        self.changed(cx);
        self.enter_float(id, cx);
    }

    pub(crate) fn set_float_scope(
        &mut self,
        id: TabWindowId,
        scope: FloatScope,
        cx: &mut Context<Self>,
    ) {
        if self.float_scope(id) == Some(scope) || self.state.set_float_scope(id, scope).is_err() {
            return;
        }
        self.changed(cx);
    }

    pub(crate) fn toggle_float_scope(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        let scope = match self.float_scope(id) {
            Some(FloatScope::Project) => FloatScope::AllProjects,
            Some(FloatScope::AllProjects) => FloatScope::Project,
            None => return,
        };
        self.set_float_scope(id, scope, cx);
    }

    pub(crate) fn toggle_pane_float_scope(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if let Some(id) = self.state.pane_window(pane).filter(|id| self.is_float(*id)) {
            self.toggle_float_scope(id, cx);
        }
    }

    pub(crate) fn toggle_floats(&mut self, cx: &mut Context<Self>) {
        if self.state.floating_tabs().next().is_none() {
            return;
        }
        let hidden = !self.state.floats_hidden();
        if hidden {
            self.floating.front_float = self.key_float().or_else(|| {
                self.floating
                    .stack
                    .iter()
                    .filter_map(panel_float)
                    .filter(|id| self.float_shown(*id))
                    .last()
            });
        }
        self.state.set_floats_hidden(hidden);
        self.changed(cx);
        if hidden {
            return;
        }
        self.sync_floating_stack();
        let front = self
            .floating
            .front_float
            .take()
            .filter(|id| self.float_shown(*id))
            .or_else(|| self.shown_floats().last().copied());
        if let Some(id) = front {
            self.enter_float(id, cx);
        }
    }

    pub(crate) fn sync_float_focus(&mut self, window: &Window, cx: &gpui::App) {
        if !window.is_window_active() || self.overlay.is_some() || self.dialogs.is_open() {
            return;
        }
        match self.focused_float(window, cx) {
            Some(id) => self.tab_windows.key = Some(id),
            None if self.key_float().is_some() => self.tab_windows.key = None,
            None => {}
        }
    }

    pub(crate) fn enter_main_unless_overlay(&mut self, cx: &mut Context<Self>) {
        if self.overlay.is_some() && self.key_float().is_some() {
            return;
        }
        let leaving = self.key_float().is_some();
        self.enter_surface(Surface::Main);
        if leaving {
            cx.notify();
        }
    }

    pub(crate) fn sync_floats(&mut self) {
        let Some(id) = self.tab_windows.key else {
            return;
        };
        let stale = match self.state.tab_window(id) {
            Some((_, _, window)) => window.is_floating() && !self.float_shown(id),
            None => !self.tab_windows.open.contains_key(&id),
        };
        if stale {
            self.tab_windows.key = None;
            self.tab_windows.focus.remove(&id);
            self.focus_owner.request_active_pane();
        }
    }
}
