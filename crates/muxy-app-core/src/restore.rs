use std::collections::HashSet;

use muxy_protocol::{SessionId, SessionInfo};

use crate::{AppState, PaneContent, PaneId, ProjectStatus};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RestorePlan {
    pub attach: Vec<(PaneId, SessionId)>,
    pub create: Vec<PaneId>,
    pub close: Vec<(PaneId, SessionId)>,
}

pub fn plan(state: &AppState, sessions: &[SessionInfo]) -> RestorePlan {
    let live: HashSet<_> = sessions
        .iter()
        .map(|session| (session.project, session.id))
        .collect();
    let mut plan = RestorePlan::default();
    for (project, pane) in state.projects().iter().flat_map(|project| {
        project
            .tabs
            .iter()
            .flat_map(move |tab| tab.panes.iter().map(move |pane| (project, pane)))
    }) {
        match pane.content {
            PaneContent::Terminal { session: Some(id) } if live.contains(&(project.id, id)) => {
                plan.attach.push((pane.id, id));
            }
            PaneContent::Terminal { session: Some(id) } => plan.close.push((pane.id, id)),
            PaneContent::Terminal { session: None }
                if project.status() == ProjectStatus::Available =>
            {
                plan.create.push(pane.id);
            }
            PaneContent::Terminal { session: None }
            | PaneContent::Settings
            | PaneContent::Webview(_) => {}
        }
    }
    plan
}
