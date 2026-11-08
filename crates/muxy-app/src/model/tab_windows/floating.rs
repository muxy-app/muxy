//! Tabs floating over the main window instead of in a window of their own:
//! which show now, where they sit, and which one has the keyboard.

use gpui::{Context, Size, Window};
use muxy_app_core::{FloatScope, PaneId, Project, TabId, TabWindow, TabWindowId};
use muxy_ui::floating::{FloatingFrame, SizeLimits};
use muxy_ui::panel::PanelId;

use super::{AppModel, Surface};

const PANEL_PREFIX: &str = "muxy.floating-tab.";
const DEFAULT_SIZE: (f32, f32) = (560.0, 340.0);
const MIN_SIZE: (f32, f32) = (300.0, 160.0);
/// How far each floating tab placed by default sits from the one before.
const CASCADE: f32 = 28.0;
const CASCADE_STEPS: usize = 6;

/// The floating layer's name for floating tab `id`.
pub(crate) fn panel(id: TabWindowId) -> PanelId {
    PanelId::new(format!("{PANEL_PREFIX}{id}"))
}

/// The floating tab a floating layer member is, if it is one.
pub(crate) fn panel_float(panel: &PanelId) -> Option<TabWindowId> {
    panel.as_str().strip_prefix(PANEL_PREFIX)?.parse().ok()
}

pub(crate) fn float_limits() -> SizeLimits {
    SizeLimits::at_least(MIN_SIZE.0, MIN_SIZE.1)
}

impl AppModel {
    /// Whether tab window `id` floats over the main window.
    pub(crate) fn is_float(&self, id: TabWindowId) -> bool {
        self.state
            .tab_window(id)
            .is_some_and(|(_, _, window)| window.is_floating())
    }

    /// Whether `pane` is in a floating tab.
    pub(crate) fn pane_floats(&self, pane: PaneId) -> bool {
        self.state
            .pane_window(pane)
            .is_some_and(|id| self.is_float(id))
    }

    fn float_shows(&self, project: &Project, window: &TabWindow) -> bool {
        let Some(floating) = window.floating() else {
            return false;
        };
        !self.state.floats_hidden()
            && (floating.scope == FloatScope::AllProjects
                || project.id == self.state.current_project().id)
    }

    /// Whether floating tab `id` shows over the main window now.
    pub(crate) fn float_shown(&self, id: TabWindowId) -> bool {
        self.state
            .tab_window(id)
            .is_some_and(|(project, _, window)| self.float_shows(project, window))
    }

    /// The floating tabs the main window shows now.
    pub(crate) fn shown_floats(&self) -> Vec<TabWindowId> {
        self.state
            .floating_tabs()
            .filter(|(project, _, window)| self.float_shows(project, window))
            .map(|(_, _, window)| window.id)
            .collect()
    }

    /// Whether `pane` shows a page rather than a terminal, so it can't float.
    pub(crate) fn shows_page(&self, pane: PaneId) -> bool {
        self.state.pane(pane).is_some_and(|pane| {
            !matches!(pane.content, muxy_app_core::PaneContent::Terminal { .. })
        })
    }

    /// What `tab`, drawn in `window`, is to the model: a shown floating
    /// tab's own surface, or else the window's.
    pub(crate) fn tab_surface(&self, tab: TabId, window: &Window) -> Surface {
        self.tab(tab)
            .and_then(|tab| tab.window.as_ref())
            .filter(|tab_window| tab_window.is_floating() && self.float_shown(tab_window.id))
            .map_or_else(
                || self.surface_of(window),
                |tab_window| Surface::Tab(tab_window.id),
            )
    }

    /// The shown floating tab whose terminal has the keyboard in `window`.
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

    /// The floating tab that has the keyboard, if one does.
    pub(crate) fn key_float(&self) -> Option<TabWindowId> {
        self.tab_windows.key.filter(|id| self.is_float(*id))
    }

    /// Which projects floating tab `id` shows over.
    pub(crate) fn float_scope(&self, id: TabWindowId) -> Option<FloatScope> {
        self.state
            .tab_window(id)
            .and_then(|(_, _, window)| window.floating())
            .map(|floating| floating.scope)
    }

    /// Where floating tab `id` sits, inside the window.
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

    /// Where floating tab `id` first shows: near the middle of the window,
    /// each one a step down and right of the one before.
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

    /// Keeps where floating tab `id` was first drawn, so it stays there.
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

    /// Floats `pane` over the main window and gives it the keyboard.
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

    /// Floats the focused pane over its project.
    pub(crate) fn float_active_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.active_pane().filter(|pane| !self.pane_floats(*pane)) {
            self.float_pane(pane, FloatScope::Project, cx);
        }
    }

    /// Gives floating tab `id` the keyboard and brings it to the front.
    pub(crate) fn enter_float(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        self.tab_windows.key = Some(id);
        self.tab_windows.request_focus(id);
        self.raise_floating(&panel(id), cx);
        cx.notify();
    }

    /// Shows floating tab `id`, over its own project if it floats only
    /// there, and gives it the keyboard.
    pub(crate) fn reveal_float(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        let Some((project, _, window)) = self.state.tab_window(id) else {
            return;
        };
        let elsewhere = window
            .floating()
            .is_some_and(|floating| floating.scope == FloatScope::Project)
            && project.id != self.state.current_project().id;
        let project = project.id;
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

    /// Switches floating tab `id` between showing over its project and over
    /// every project.
    pub(crate) fn toggle_float_scope(&mut self, id: TabWindowId, cx: &mut Context<Self>) {
        let scope = match self.float_scope(id) {
            Some(FloatScope::Project) => FloatScope::AllProjects,
            Some(FloatScope::AllProjects) => FloatScope::Project,
            None => return,
        };
        self.set_float_scope(id, scope, cx);
    }

    /// Switches the floating tab showing `pane` between its project and
    /// every project.
    pub(crate) fn toggle_pane_float_scope(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if let Some(id) = self.state.pane_window(pane).filter(|id| self.is_float(*id)) {
            self.toggle_float_scope(id, cx);
        }
    }

    /// Hides every floating tab, keeping their terminals, or shows them
    /// again with the front one taking the keyboard.
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

    /// Makes the floating tab whose terminal has the keyboard the one the
    /// model works for, and the main window when the keyboard left every
    /// floating tab, since shortcuts reach the focused terminal before any
    /// click or key handler could say where the user works.
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

    /// The user works in the main window now, unless a floating tab has the
    /// keyboard while an overlay it opened is up: its commands stay its own.
    /// Redraws when the keyboard leaves a floating tab, so the panes show
    /// which one has it at once.
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

    /// Gives the keyboard back to the main window when the floating tab that
    /// had it stopped showing or went away.
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
