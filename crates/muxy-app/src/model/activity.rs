use std::collections::HashSet;

use gpui::Context;
use muxy_app_core::{PaneId, ServerId, TabId};
use muxy_client::ClientError;
use muxy_protocol::{ActivitySnapshot, AgentActivity, SessionId};
use muxy_ui::tr;

use super::{AppModel, Work};

#[derive(Default)]
pub(crate) struct ActivityView {
    pub(crate) snapshot: ActivitySnapshot,
    pub(crate) dirty: u64,
    pub(crate) pending: bool,
    pub(crate) loaded: bool,
    pub(super) navigation: Option<u64>,
    pub(super) pane_sessions: Vec<SessionId>,
    acknowledging: HashSet<u64>,
    ended_during_read: HashSet<SessionId>,
    ack_failed: bool,
}

/// Each server numbers its own events, so another computer's carry its server.
fn notification_id(server: ServerId, id: u64) -> String {
    if server.is_local() {
        id.to_string()
    } else {
        format!("{server}:{id}")
    }
}

#[cfg(target_os = "macos")]
fn activity_description(kind: muxy_protocol::ActivityKind) -> gpui::SharedString {
    use muxy_protocol::ActivityKind;
    match kind {
        ActivityKind::Attention => tr!("Needs your attention"),
        ActivityKind::Completed => tr!("Finished working"),
        ActivityKind::Unrecognized(_) => tr!("Has an update"),
    }
}

fn notification_event(identifier: &str) -> Option<(ServerId, u64)> {
    match identifier.split_once(':') {
        Some((server, id)) => Some((server.parse().ok()?, id.parse().ok()?)),
        None => Some((ServerId::local(), identifier.parse().ok()?)),
    }
}

impl AppModel {
    pub(crate) fn activity(&self, server: ServerId) -> Option<&ActivityView> {
        self.servers.get(server).map(|runtime| &runtime.activity)
    }

    fn activity_mut(&mut self, server: ServerId) -> Option<&mut ActivityView> {
        self.servers
            .get_mut(server)
            .map(|runtime| &mut runtime.activity)
    }

    /// The agent the pane's server reports for the pane's session.
    pub(crate) fn pane_agent(&self, pane: PaneId) -> Option<&AgentActivity> {
        let server = self.state.pane_server(pane)?;
        let session = self.pane_session(pane)?;
        self.activity(server)?
            .snapshot
            .agents
            .iter()
            .find(|agent| agent.session == session)
    }

    pub(crate) fn agent_tab_pane<'a>(
        &'a self,
        tab: &'a muxy_app_core::Tab,
    ) -> Option<(&'a muxy_app_core::Pane, &'a AgentActivity)> {
        tab.panes
            .iter()
            .filter_map(|pane| Some((pane, self.pane_agent(pane.id)?)))
            .min_by_key(|(pane, _)| Some(pane.id) != self.state.window().active_pane)
    }

    /// The first pane of `server`'s projects that shows `session`.
    pub(super) fn session_pane(
        &self,
        server: ServerId,
        session: SessionId,
    ) -> Option<(&muxy_app_core::Project, TabId, PaneId)> {
        self.state
            .projects()
            .iter()
            .filter(|project| project.server_id == server)
            .find_map(|project| {
                project.tabs.iter().find_map(|tab| {
                    tab.panes
                        .iter()
                        .find(|pane| {
                            matches!(pane.content,
                                muxy_app_core::PaneContent::Terminal { session: Some(id) } if id == session)
                        })
                        .map(|pane| (project, tab.id, pane.id))
                })
            })
    }

    pub(crate) fn refresh_activity(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !self.ready(server)
            || self
                .activity(server)
                .is_none_or(|activity| activity.pending)
        {
            return;
        }
        if self.send(server, Work::ReadActivity, cx)
            && let Some(activity) = self.activity_mut(server)
        {
            activity.pending = true;
        }
    }

    fn has_activity_pane(&self, server: ServerId, session: SessionId) -> bool {
        self.state.session_references(server).contains(&session)
    }

    pub(super) fn sync_activity_panes(&mut self, server: ServerId, sessions: Vec<SessionId>) {
        let Some(activity) = self.activity(server) else {
            return;
        };
        if activity.pane_sessions == sessions {
            return;
        }
        let removed: Vec<_> = activity
            .snapshot
            .events
            .iter()
            .filter(|event| {
                activity.pane_sessions.contains(&event.session)
                    && !sessions.contains(&event.session)
            })
            .map(|event| event.id)
            .collect();
        #[cfg(target_os = "macos")]
        if !removed.is_empty()
            && let Some(notifications) = &self.notifications
        {
            notifications.clear(
                &removed
                    .iter()
                    .map(|id| notification_id(server, *id))
                    .collect::<Vec<_>>(),
            );
        }
        if let Some(activity) = self.activity_mut(server) {
            if activity.navigation.is_some_and(|id| removed.contains(&id)) {
                activity.navigation = None;
            }
            activity.pane_sessions = sessions;
        }
    }

    fn activity_notification_events<'a>(
        &'a self,
        server: ServerId,
        ids: &'a [u64],
    ) -> impl Iterator<Item = &'a muxy_protocol::ActivityEvent> {
        let focused = self.focused_activity_session();
        self.activity(server)
            .into_iter()
            .flat_map(|activity| &activity.snapshot.events)
            .filter(move |event| {
                ids.contains(&event.id)
                    && !event.read
                    && self.has_activity_pane(server, event.session)
                    && Some((server, event.session)) != focused
            })
    }

    fn focused_activity_session(&self) -> Option<(ServerId, SessionId)> {
        if !self.window_active || self.overlay.is_some() || self.close_prompt.is_some() {
            return None;
        }
        let pane = self.active_pane()?;
        Some((self.state.pane_server(pane)?, self.pane_session(pane)?))
    }

    pub(super) fn receive_activity(
        &mut self,
        server: ServerId,
        result: Result<ActivitySnapshot, ClientError>,
        cx: &mut Context<Self>,
    ) {
        let focused = self
            .focused_activity_session()
            .filter(|(owner, _)| *owner == server)
            .map(|(_, session)| session);
        let Some(activity) = self.activity_mut(server) else {
            return;
        };
        activity.pending = false;
        match result {
            Ok(mut snapshot) => {
                snapshot
                    .agents
                    .retain(|agent| !activity.ended_during_read.contains(&agent.session));
                snapshot.events.retain(|event| {
                    !event.read && !activity.ended_during_read.contains(&event.session)
                });
                activity.ended_during_read.clear();
                #[cfg(target_os = "macos")]
                let removed: Vec<_> = {
                    let retained: HashSet<_> =
                        snapshot.events.iter().map(|event| event.id).collect();
                    activity
                        .snapshot
                        .events
                        .iter()
                        .filter(|event| !retained.contains(&event.id))
                        .map(|event| notification_id(server, event.id))
                        .collect()
                };
                let ids = muxy_app_core::activity::new_notifications(
                    activity.loaded.then_some(&activity.snapshot),
                    &snapshot,
                    focused,
                );
                activity.snapshot = snapshot;
                #[cfg(target_os = "macos")]
                if let Some(notifications) = &self.notifications {
                    notifications.clear(&removed);
                }
                self.activity_notifications_posted(server, &ids, cx);
                self.sync_activity_panes(server, self.state.session_references(server));
                let ids: Vec<_> = self
                    .activity_notification_events(server, &ids)
                    .map(|event| event.id)
                    .collect();
                let agent_panes: Vec<_> = self
                    .state
                    .projects()
                    .iter()
                    .filter(|project| project.server_id == server)
                    .flat_map(|project| &project.tabs)
                    .flat_map(|tab| &tab.panes)
                    .filter_map(|pane| self.pane_agent(pane.id).map(|_| pane.id))
                    .collect();
                for pane in agent_panes {
                    self.completions.remove(&pane);
                }
                let Some(activity) = self.activity_mut(server) else {
                    return;
                };
                activity.loaded = true;
                activity.ack_failed = false;
                self.resume_activity_navigation(server, cx);
                self.acknowledge_focused_activity(cx);
                if !ids.is_empty() {
                    self.send(server, Work::ClaimActivity(ids), cx);
                }
                if self
                    .activity(server)
                    .is_some_and(|activity| activity.dirty > activity.snapshot.revision)
                {
                    self.refresh_activity(server, cx);
                }
            }
            Err(error) => {
                self.server_problem(
                    server,
                    tr!("Could not read activity: %@", error.to_string()).to_string(),
                    cx,
                );
            }
        }
        self.sync_extension_events(cx);
        cx.notify();
    }

    pub(super) fn forget_session_activity(
        &mut self,
        server: ServerId,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        let Some(activity) = self.activity_mut(server) else {
            return;
        };
        if activity.pending {
            activity.ended_during_read.insert(session);
        }
        let ids: Vec<_> = activity
            .snapshot
            .events
            .iter()
            .filter(|event| event.session == session)
            .map(|event| event.id)
            .collect();
        activity
            .snapshot
            .agents
            .retain(|agent| agent.session != session);
        activity
            .snapshot
            .events
            .retain(|event| event.session != session);
        activity.acknowledging.retain(|id| !ids.contains(id));
        if activity.navigation.is_some_and(|id| ids.contains(&id)) {
            activity.navigation = None;
        }
        #[cfg(target_os = "macos")]
        if let Some(notifications) = &self.notifications {
            notifications.clear(
                &ids.iter()
                    .map(|id| notification_id(server, *id))
                    .collect::<Vec<_>>(),
            );
        }
        cx.notify();
    }

    pub(crate) fn acknowledge_focused_activity(&mut self, cx: &mut Context<Self>) {
        let Some((server, session)) = self.focused_activity_session() else {
            return;
        };
        let Some(activity) = self
            .activity(server)
            .filter(|activity| !activity.ack_failed)
        else {
            return;
        };
        let ids = activity
            .snapshot
            .events
            .iter()
            .filter(|event| event.session == session && !event.read)
            .map(|event| event.id)
            .collect();
        self.acknowledge_activity(server, ids, cx);
    }

    pub(crate) fn acknowledge_activity(
        &mut self,
        server: ServerId,
        ids: Vec<u64>,
        cx: &mut Context<Self>,
    ) {
        if !self.ready(server) {
            return;
        }
        let Some(activity) = self.activity(server) else {
            return;
        };
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| !activity.acknowledging.contains(id))
            .collect();
        if !ids.is_empty()
            && self.send(server, Work::AcknowledgeActivity(ids.clone()), cx)
            && let Some(activity) = self.activity_mut(server)
        {
            activity.acknowledging.extend(ids);
        }
    }

    pub(super) fn activity_acknowledged(
        &mut self,
        server: ServerId,
        ids: &[u64],
        result: Result<(), ClientError>,
        cx: &mut Context<Self>,
    ) {
        let Some(activity) = self.activity_mut(server) else {
            return;
        };
        for id in ids {
            activity.acknowledging.remove(id);
        }
        match result {
            Ok(()) => {
                activity
                    .snapshot
                    .events
                    .retain(|event| !ids.contains(&event.id));
                #[cfg(target_os = "macos")]
                if let Some(notifications) = &self.notifications {
                    notifications.clear(
                        &ids.iter()
                            .map(|id| notification_id(server, *id))
                            .collect::<Vec<_>>(),
                    );
                }
                self.refresh_activity(server, cx);
            }
            Err(error) => {
                activity.ack_failed = true;
                self.fail(
                    tr!("Could not mark activity read: %@", error.to_string()).to_string(),
                    cx,
                );
            }
        }
        cx.notify();
    }

    pub(super) fn deliver_activity(&self, server: ServerId, result: Result<Vec<u64>, ClientError>) {
        let Ok(ids) = result else {
            return;
        };
        #[cfg(target_os = "macos")]
        if let Some(notifications) = &self.notifications {
            for event in self.activity_notification_events(server, &ids) {
                let project = self
                    .state
                    .project(event.project)
                    .map_or("Muxy", |project| project.name.as_str());
                let title = match self.server_name(server) {
                    Some(name) if !server.is_local() => {
                        format!("{} · {project} · {name}", event.provider.name())
                    }
                    _ => format!("{} · {project}", event.provider.name()),
                };
                notifications.deliver(
                    &notification_id(server, event.id),
                    &title,
                    &activity_description(event.kind),
                );
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (server, ids);
    }

    pub(crate) fn navigate_activity(&mut self, server: ServerId, id: u64, cx: &mut Context<Self>) {
        let waiting = !self.ready(server)
            || self.servers.get(server).is_none_or(|runtime| {
                !runtime.activity.loaded
                    || runtime.catalog.restore.is_some()
                    || runtime.catalog.pending
            });
        if waiting {
            if let Some(activity) = self.activity_mut(server) {
                activity.navigation = Some(id);
            }
            return;
        }
        let Some(event) = self.activity(server).and_then(|activity| {
            activity
                .snapshot
                .events
                .iter()
                .find(|event| event.id == id)
                .cloned()
        }) else {
            return;
        };
        if !self.has_activity_pane(server, event.session) {
            return;
        }
        self.acknowledge_activity(server, vec![id], cx);
        if let Some((project, tab, pane)) = self
            .session_pane(server, event.session)
            .map(|(project, tab, pane)| (project.id, tab, pane))
        {
            self.select_project(project, cx);
            self.select_tab(tab, cx);
            self.focus_pane(pane, cx);
            self.dismiss_overlay(cx);
            self.focus_requested = true;
        } else if server.is_local() && !self.quick.visible {
            self.toggle_quick_terminal(cx);
        }
    }

    pub(super) fn resume_activity_navigation(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if let Some(id) = self
            .activity_mut(server)
            .and_then(|activity| activity.navigation.take())
        {
            self.navigate_activity(server, id, cx);
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn start_notifications(
        cx: &mut Context<Self>,
    ) -> Option<muxy_ui::notifications::Notifications> {
        let (notifications, events) = muxy_ui::notifications::Notifications::new()?;
        cx.spawn(async move |this, cx| {
            while let Ok(identifier) = events.recv().await {
                let Some((server, id)) = notification_event(&identifier) else {
                    continue;
                };
                if this
                    .update(cx, |model, cx| {
                        cx.activate(true);
                        model.navigate_activity(server, id, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Some(notifications)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_ids_keep_servers_apart() {
        let remote = ServerId::new();
        assert_eq!(notification_id(ServerId::local(), 7), "7");
        assert_eq!(
            notification_event(&notification_id(ServerId::local(), 7)),
            Some((ServerId::local(), 7))
        );
        assert_eq!(
            notification_event(&notification_id(remote, 7)),
            Some((remote, 7))
        );
        assert_eq!(notification_event("box:7"), None);
    }
}
