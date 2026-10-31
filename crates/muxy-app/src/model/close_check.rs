//! Asking whether closing a pane would end a program running in its session.
//! Closing tabs and panes and closing the Quick Terminal share it.

use gpui::Context;
use muxy_app_core::closing::runs_program;
use muxy_app_core::{PaneId, ServerId};
use muxy_client::ClientError;
use muxy_protocol::{ForegroundProcess, SessionId, Size};
use muxy_ui::tr;

use super::AppModel;
use super::quick_terminal::QuickClose;
use crate::boot::{CloseCheckId, Work, missing_session};
use crate::views::terminal::pane::PaneState;

/// What is known about the program closing a pane would end.
pub(super) enum CloseCheck {
    /// A program other than the shell is running.
    Running,
    /// Nothing worth asking about is running.
    Safe,
    /// The server was asked. It answers with `Update::CloseChecked`.
    Asked(CloseCheckId),
    /// The question could not be sent.
    Unsent,
}

impl AppModel {
    /// Asks `server` what runs in `session`, which `pane` shows. Without a
    /// server to ask, the pane's last known process answers.
    pub(super) fn check_close(
        &mut self,
        server: ServerId,
        pane: PaneId,
        session: SessionId,
        cx: &mut Context<Self>,
    ) -> CloseCheck {
        if self.ready(server) && !self.retained.contains(&pane) {
            let size = self
                .terminal(&pane)
                .and_then(|pane| pane.view.read(cx).viewport())
                .unwrap_or(Size { cols: 80, rows: 24 });
            let check = CloseCheckId::new();
            return if self.send(
                server,
                Work::CheckClose {
                    check,
                    session,
                    size,
                },
                cx,
            ) {
                CloseCheck::Asked(check)
            } else {
                CloseCheck::Unsent
            };
        }
        let running = self.terminal(&pane).is_some_and(|pane| {
            let view = pane.view.read(cx);
            view.channel().is_some()
                && view.state == PaneState::Live
                && runs_program(view.process.as_ref())
        });
        if running {
            CloseCheck::Running
        } else {
            CloseCheck::Safe
        }
    }

    pub(super) fn receive_close_checked(
        &mut self,
        check: CloseCheckId,
        session: SessionId,
        result: Result<Option<ForegroundProcess>, ClientError>,
        cx: &mut Context<Self>,
    ) {
        let running = match result {
            Ok(process) => Ok(runs_program(process.as_ref())),
            Err(error) if missing_session(&error) => Ok(false),
            Err(error) => {
                Err(tr!("Could not check the running process: %@", error.to_string()).to_string())
            }
        };
        if self.quick.closing == Some(QuickClose::Checking(check)) {
            self.receive_quick_close_checked(running, cx);
        } else if self.pending_close == Some(check) {
            self.receive_tab_close_checked(session, running, cx);
        }
    }
}
