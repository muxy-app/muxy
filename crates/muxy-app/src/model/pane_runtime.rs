//! What the app remembers about a pane while no view holds it. A pane with
//! nothing to remember has no record, so hidden panes cost nothing extra.

use muxy_app_core::{PaneId, ServerId};
use muxy_client::RunGrid;

use super::AppModel;
use crate::views::terminal::scroll::{Scroll, output_changed};

#[derive(Default)]
pub(crate) struct PaneRuntime {
    pub(crate) completed: bool,
    retained: bool,
    loaded: bool,
    zoom: Option<f32>,
    snapshot: Option<RunGrid>,
    scroll: Option<Scroll>,
}

impl PaneRuntime {
    fn is_default(&self) -> bool {
        !self.completed
            && !self.retained
            && !self.loaded
            && self.zoom.is_none()
            && self.snapshot.is_none()
            && self.scroll.is_none()
    }
}

impl AppModel {
    fn edit_pane_runtime<T>(
        &mut self,
        pane: PaneId,
        edit: impl FnOnce(&mut PaneRuntime) -> T,
    ) -> T {
        let runtime = self.pane_runtime.entry(pane).or_default();
        let result = edit(runtime);
        if runtime.is_default() {
            self.pane_runtime.remove(&pane);
        }
        result
    }

    fn pane_runtime_field<T: Default>(
        &self,
        pane: PaneId,
        field: impl FnOnce(&PaneRuntime) -> T,
    ) -> T {
        self.pane_runtime.get(&pane).map(field).unwrap_or_default()
    }

    pub(crate) fn pane_completed(&self, pane: PaneId) -> bool {
        self.pane_runtime_field(pane, |runtime| runtime.completed)
    }

    pub(crate) fn set_pane_completed(&mut self, pane: PaneId, completed: bool) {
        self.edit_pane_runtime(pane, |runtime| runtime.completed = completed);
    }

    pub(super) fn pane_retained(&self, pane: PaneId) -> bool {
        self.pane_runtime_field(pane, |runtime| runtime.retained)
    }

    pub(super) fn pane_loaded(&self, pane: PaneId) -> bool {
        self.pane_runtime_field(pane, |runtime| runtime.loaded)
    }

    pub(super) fn set_pane_loaded(&mut self, pane: PaneId, loaded: bool) {
        self.edit_pane_runtime(pane, |runtime| runtime.loaded = loaded);
    }

    pub(super) fn pane_zoom(&self, pane: PaneId) -> Option<f32> {
        self.pane_runtime_field(pane, |runtime| runtime.zoom)
    }

    pub(super) fn set_pane_zoom(&mut self, pane: PaneId, zoom: Option<f32>) {
        self.edit_pane_runtime(pane, |runtime| runtime.zoom = zoom);
    }

    pub(super) fn clear_pane_zooms(&mut self) {
        for runtime in self.pane_runtime.values_mut() {
            runtime.zoom = None;
        }
        self.pane_runtime.retain(|_, runtime| !runtime.is_default());
    }

    /// Keeps a hidden pane's screen, and its scroll position if it was scrolled.
    pub(super) fn keep_hidden_screen(&mut self, pane: PaneId, grid: RunGrid, scroll: Scroll) {
        self.edit_pane_runtime(pane, |runtime| {
            runtime.snapshot = Some(grid);
            if scroll.view.is_some() {
                runtime.scroll = Some(scroll);
            }
        });
    }

    pub(super) fn hidden_screen(&self, pane: PaneId) -> Option<&RunGrid> {
        self.pane_runtime.get(&pane)?.snapshot.as_ref()
    }

    pub(super) fn take_hidden_screen(&mut self, pane: PaneId) -> (Option<RunGrid>, Scroll) {
        self.edit_pane_runtime(pane, |runtime| {
            (
                runtime.snapshot.take(),
                runtime.scroll.take().unwrap_or_default(),
            )
        })
    }

    /// A hidden pane attached again and brought a newer screen. New output
    /// can end its saved scroll position.
    pub(super) fn refresh_hidden_screen(&mut self, pane: PaneId, grid: RunGrid) {
        let scroll_on_output = self.terminal.options.scroll_on_output;
        self.edit_pane_runtime(pane, |runtime| {
            if scroll_on_output
                && runtime
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| output_changed(snapshot, &grid))
            {
                runtime.scroll = None;
            }
            runtime.snapshot = Some(grid);
        });
    }

    /// Forgets what is only true while connected to `server`.
    pub(super) fn forget_pane_loads(&mut self, server: ServerId) {
        for (pane, runtime) in &mut self.pane_runtime {
            if self.state.pane_server(*pane) == Some(server) {
                runtime.loaded = false;
            }
        }
        self.pane_runtime.retain(|_, runtime| !runtime.is_default());
    }

    pub(super) fn forget_pane_completions(&mut self, server: ServerId) {
        for (pane, runtime) in &mut self.pane_runtime {
            if self.state.pane_server(*pane) == Some(server) {
                runtime.completed = false;
            }
        }
        self.pane_runtime.retain(|_, runtime| !runtime.is_default());
    }

    #[cfg(test)]
    pub(super) fn has_saved_scroll(&self) -> bool {
        self.pane_runtime
            .values()
            .any(|runtime| runtime.scroll.is_some())
    }
}
