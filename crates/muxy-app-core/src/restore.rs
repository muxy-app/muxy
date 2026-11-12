use std::collections::HashMap;

use muxy_protocol::{SessionId, SessionInfo};

use crate::{AppState, PaneContent, PaneId, ProjectStatus, ServerId, SessionMove};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RestorePlan {
    pub attach: Vec<(PaneId, SessionId)>,
    pub create: Vec<PaneId>,
    /// Panes whose session the server no longer runs. Each closes if its
    /// shell exited or was ended, and otherwise starts a new terminal in its
    /// place, as after the server stopped or the computer restarted.
    pub restart: Vec<(PaneId, SessionId)>,
    /// Panes showing a session the server lists under another project.
    pub close: Vec<(PaneId, SessionId)>,
    /// Sessions the app moved to the project its pane shows them in while
    /// the server still lists them under another, as when the request never
    /// reached it. Their panes attach, and the server is asked again.
    pub moves: Vec<(SessionMove, muxy_protocol::ProjectId)>,
}

/// Plans `server`'s panes against the sessions that server listed.
pub fn plan(state: &AppState, server: ServerId, sessions: &[SessionInfo]) -> RestorePlan {
    let live: HashMap<_, _> = sessions
        .iter()
        .map(|session| (session.id, session.project))
        .collect();
    let mut plan = RestorePlan::default();
    for (project, pane) in state
        .projects()
        .iter()
        .filter(|project| project.server_id == server)
        .flat_map(|project| {
            project
                .tabs
                .iter()
                .flat_map(move |tab| tab.panes.iter().map(move |pane| (project, pane)))
        })
    {
        match pane.content {
            PaneContent::Terminal { session: Some(id) } if live.get(&id) == Some(&project.id) => {
                plan.attach.push((pane.id, id));
            }
            PaneContent::Terminal { session: Some(id) }
                if live.contains_key(&id)
                    && state
                        .pending_session_move(server, id)
                        .is_some_and(|pending| pending.project == project.id) =>
            {
                plan.attach.push((pane.id, id));
                if !plan.moves.iter().any(|(moved, _)| moved.session == id) {
                    plan.moves.push((
                        SessionMove {
                            server,
                            session: id,
                            project: project.id,
                        },
                        live[&id],
                    ));
                }
            }
            PaneContent::Terminal { session: Some(id) } if live.contains_key(&id) => {
                plan.close.push((pane.id, id));
            }
            PaneContent::Terminal { session: Some(id) } => plan.restart.push((pane.id, id)),
            PaneContent::Terminal { session: None }
                if project.status() == ProjectStatus::Available =>
            {
                plan.create.push(pane.id);
            }
            PaneContent::Terminal { session: None }
            | PaneContent::Settings
            | PaneContent::Webview(_)
            | PaneContent::Browser(_)
            | PaneContent::Servers => {}
        }
    }
    plan
}
