use gpui::Context;

use super::AppModel;
use crate::diagnostics::event;
use crate::views::terminal::pane::TerminalPane;

impl AppModel {
    pub(super) fn start_diagnostics(cx: &mut Context<Self>) {
        if !crate::diagnostics::enabled() {
            return;
        }
        cx.spawn(async move |model, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                if model.update(cx, |model, cx| model.trace_state(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn trace_state(&self, cx: &gpui::App) {
        event(
            "model.state",
            format_args!(
                "generation={} connection={:?} quitting={:?} project={:?} tab={:?} active={:?} pending={:?} discarding={} intents={} restore={} replacing={} panels={}",
                self.generation,
                self.connection,
                self.quitting,
                self.state.current_project().id,
                self.active_tab(),
                self.active_pane(),
                self.pending,
                self.discarding.len(),
                self.state.project_intents().len(),
                self.catalog.restore.is_some(),
                self.updates.replacing(),
                self.webviews.panels.len(),
            ),
        );
        for id in self.attached_panes() {
            let pane = self.terminal(&id).map(|pane| pane.view.read(cx));
            event(
                "terminal.state",
                format_args!(
                    "pane={id} view={} channel={:?} grid={} viewport={:?} geometry={:?} retained={}",
                    pane.is_some(),
                    pane.and_then(TerminalPane::channel),
                    pane.is_some_and(|pane| pane.grid.is_some()),
                    pane.and_then(TerminalPane::viewport),
                    pane.and_then(|pane| pane.geometry),
                    self.retained.contains(&id),
                ),
            );
        }
    }
}
