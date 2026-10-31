//! Whether closing panes would end a running program. Closing the last pane
//! that shows a session ends it, so only those sessions are worth asking
//! about.

use muxy_protocol::{ForegroundProcess, SessionId};

use crate::{AppState, PaneId, ServerId};

impl AppState {
    /// The sessions that closing `panes` would end, each once, with the
    /// closing pane that shows it. A session another pane shows keeps
    /// running.
    pub fn sessions_ended_by_closing(
        &self,
        panes: &[PaneId],
    ) -> Vec<(PaneId, ServerId, SessionId)> {
        let shown: Vec<_> = self
            .panes()
            .filter_map(|placed| Some((placed.pane.id, (placed.server, placed.pane.session()?))))
            .collect();
        sessions_ended_by_closing(&shown, panes)
            .into_iter()
            .map(|(pane, (server, session))| (pane, server, session))
            .collect()
    }
}

/// The sessions that closing `panes` would end, each once, with the closing
/// pane that shows it. `shown` pairs every pane with the session it shows.
pub fn sessions_ended_by_closing<S: Copy + PartialEq>(
    shown: &[(PaneId, S)],
    panes: &[PaneId],
) -> Vec<(PaneId, S)> {
    let mut ended: Vec<(PaneId, S)> = Vec::new();
    for &pane in panes {
        let Some(&(_, session)) = shown.iter().find(|(shown, _)| *shown == pane) else {
            continue;
        };
        let kept = shown
            .iter()
            .any(|(other, shown)| *shown == session && !panes.contains(other));
        if !kept && !ended.iter().any(|(_, ending)| *ending == session) {
            ended.push((pane, session));
        }
    }
    ended
}

/// Whether ending a session whose foreground process is `process` stops a
/// program other than its shell. An unknown process counts as the shell.
pub fn runs_program(process: Option<&ForegroundProcess>) -> bool {
    process.is_some_and(|process| !process.is_shell)
}
