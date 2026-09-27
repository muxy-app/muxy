use muxy_protocol::{ActivitySnapshot, HistoryPage, SearchPage};
use serde_json::{Value, json};

pub(super) fn activity(snapshot: &ActivitySnapshot) -> Value {
    let agents: Vec<_> = snapshot
        .agents
        .iter()
        .map(|agent| {
            json!({"session":agent.session.get().to_string(), "project":agent.project,
                "provider":agent.provider, "state":agent.state})
        })
        .collect();
    let events: Vec<_> = snapshot
        .events
        .iter()
        .map(|event| {
            json!({"id":event.id.to_string(), "session":event.session.get().to_string(),
                "project":event.project, "provider":event.provider, "timestamp":event.timestamp,
                "kind":event.kind, "read":event.read})
        })
        .collect();
    json!({"revision":snapshot.revision, "agents":agents, "events":events})
}

pub(super) fn history(page: &HistoryPage) -> Value {
    json!({"prompts":page.prompts, "rows":page.rows, "next":page.next.map(|c| c.0.to_string()),
        "total_rows":page.total_rows, "screen":page.screen})
}

pub(super) fn search(page: &SearchPage) -> Value {
    json!({"matches":page.matches, "next":page.next.map(|c| c.0.to_string()),
        "total_rows":page.total_rows, "scanned_rows":page.scanned_rows})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manage::args::{self, Action, Session};
    use muxy_protocol::{
        ActivityEvent, ActivityKind, AgentActivity, AgentProvider, AgentState, HistoryCursor,
        ProjectId, SessionId,
    };

    #[test]
    fn identifiers_and_cursors_roundtrip_above_javascript_integer_precision() -> super::super::Result
    {
        let id = u64::MAX - 1;
        let session = SessionId::new(id).ok_or("session")?;
        let project = ProjectId::new();
        let snapshot = ActivitySnapshot {
            revision: 1,
            agents: vec![AgentActivity {
                session,
                project,
                provider: AgentProvider::Xal,
                state: AgentState::Working,
            }],
            events: vec![ActivityEvent {
                id,
                session,
                project,
                provider: AgentProvider::Xal,
                timestamp: 1,
                kind: ActivityKind::Completed,
                read: false,
            }],
        };
        let activity = activity(&snapshot);
        assert_eq!(activity["agents"][0]["session"], id.to_string());
        assert_eq!(activity["events"][0]["session"], id.to_string());
        let event = activity["events"][0]["id"].as_str().ok_or("event")?;
        assert_eq!(
            args::parse(&["activity".into(), "ack".into(), event.into()])?.action,
            Action::Activity(vec![id])
        );
        let history = history(&HistoryPage {
            prompts: Vec::new(),
            rows: Vec::new(),
            next: Some(HistoryCursor(id)),
            total_rows: 0,
            screen: None,
        });
        let search = search(&SearchPage {
            matches: Vec::new(),
            next: Some(HistoryCursor(id)),
            total_rows: 0,
            scanned_rows: 0,
        });
        for page in [history, search] {
            let cursor = page["next"].as_str().ok_or("cursor")?;
            assert_eq!(
                args::parse(&[
                    "session".into(),
                    "history".into(),
                    "1".into(),
                    "--before".into(),
                    cursor.into(),
                ])?
                .action,
                Action::Session(Session::History {
                    session: SessionId::new(1).ok_or("session")?,
                    before: HistoryCursor(id),
                    limit: 200,
                    saved: false,
                })
            );
        }
        Ok(())
    }
}
