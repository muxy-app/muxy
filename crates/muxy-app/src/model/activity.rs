use std::collections::HashSet;

use gpui::Context;
use muxy_client::ClientError;
use muxy_protocol::{ActivitySnapshot, SessionId};

use super::{AppModel, ConnectionState, Work};

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

impl AppModel {
    pub(crate) fn refresh_activity(&mut self, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready || self.activity.pending {
            return;
        }
        if self.send(Work::ReadActivity, cx) {
            self.activity.pending = true;
        }
    }

    fn has_activity_pane(&self, session: SessionId) -> bool {
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .chain(self.state.quick_terminal())
            .any(|pane| matches!(pane.content, muxy_app_core::PaneContent::Terminal { session: Some(id) } if id == session))
    }

    pub(super) fn sync_activity_panes(&mut self, sessions: Vec<SessionId>) {
        if self.activity.pane_sessions == sessions {
            return;
        }
        let removed: Vec<_> = self
            .activity
            .snapshot
            .events
            .iter()
            .filter(|event| {
                self.activity.pane_sessions.contains(&event.session)
                    && !sessions.contains(&event.session)
            })
            .map(|event| event.id)
            .collect();
        if self
            .activity
            .navigation
            .is_some_and(|id| removed.contains(&id))
        {
            self.activity.navigation = None;
        }
        #[cfg(target_os = "macos")]
        if !removed.is_empty()
            && let Some(notifications) = &self.notifications
        {
            notifications.clear(&removed.iter().map(u64::to_string).collect::<Vec<_>>());
        }
        self.activity.pane_sessions = sessions;
    }

    fn activity_notification_events<'a>(
        &'a self,
        ids: &'a [u64],
    ) -> impl Iterator<Item = &'a muxy_protocol::ActivityEvent> {
        self.activity.snapshot.events.iter().filter(move |event| {
            ids.contains(&event.id)
                && !event.read
                && self.has_activity_pane(event.session)
                && Some(event.session) != self.focused_activity_session()
        })
    }

    fn focused_activity_session(&self) -> Option<SessionId> {
        if !self.window_active || self.overlay.is_some() || self.close_prompt.is_some() {
            return None;
        }
        self.active_pane().and_then(|pane| self.pane_session(pane))
    }

    pub(super) fn receive_activity(
        &mut self,
        result: Result<ActivitySnapshot, ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.activity.pending = false;
        match result {
            Ok(mut snapshot) => {
                snapshot
                    .agents
                    .retain(|agent| !self.activity.ended_during_read.contains(&agent.session));
                snapshot.events.retain(|event| {
                    !event.read && !self.activity.ended_during_read.contains(&event.session)
                });
                self.activity.ended_during_read.clear();
                #[cfg(target_os = "macos")]
                if let Some(notifications) = &self.notifications {
                    let retained: HashSet<_> =
                        snapshot.events.iter().map(|event| event.id).collect();
                    let removed: Vec<_> = self
                        .activity
                        .snapshot
                        .events
                        .iter()
                        .filter(|event| !retained.contains(&event.id))
                        .map(|event| event.id.to_string())
                        .collect();
                    notifications.clear(&removed);
                }
                let ids = muxy_app_core::activity::new_notifications(
                    self.activity.loaded.then_some(&self.activity.snapshot),
                    &snapshot,
                    self.focused_activity_session(),
                );
                self.activity.snapshot = snapshot;
                self.sync_activity_panes(self.state.session_references());
                let ids: Vec<_> = self
                    .activity_notification_events(&ids)
                    .map(|event| event.id)
                    .collect();
                let agent_panes: Vec<_> = self
                    .state
                    .projects()
                    .iter()
                    .flat_map(|project| &project.tabs)
                    .flat_map(|tab| &tab.panes)
                    .filter_map(|pane| {
                        let session = self.pane_session(pane.id)?;
                        self.activity
                            .snapshot
                            .agents
                            .iter()
                            .any(|agent| agent.session == session)
                            .then_some(pane.id)
                    })
                    .collect();
                for pane in agent_panes {
                    self.completions.remove(&pane);
                }
                self.activity.loaded = true;
                self.activity.ack_failed = false;
                self.resume_activity_navigation(cx);
                self.acknowledge_focused_activity(cx);
                if !ids.is_empty() {
                    self.send(Work::ClaimActivity(ids), cx);
                }
                if self.activity.dirty > self.activity.snapshot.revision {
                    self.refresh_activity(cx);
                }
            }
            Err(error) => self.fail(format!("Could not read activity: {error}"), cx),
        }
        cx.notify();
    }

    pub(super) fn forget_session_activity(&mut self, session: SessionId, cx: &mut Context<Self>) {
        if self.activity.pending {
            self.activity.ended_during_read.insert(session);
        }
        let ids: Vec<_> = self
            .activity
            .snapshot
            .events
            .iter()
            .filter(|event| event.session == session)
            .map(|event| event.id)
            .collect();
        self.activity
            .snapshot
            .agents
            .retain(|agent| agent.session != session);
        self.activity
            .snapshot
            .events
            .retain(|event| event.session != session);
        self.activity.acknowledging.retain(|id| !ids.contains(id));
        if self.activity.navigation.is_some_and(|id| ids.contains(&id)) {
            self.activity.navigation = None;
        }
        #[cfg(target_os = "macos")]
        if let Some(notifications) = &self.notifications {
            notifications.clear(&ids.iter().map(u64::to_string).collect::<Vec<_>>());
        }
        cx.notify();
    }

    pub(crate) fn acknowledge_focused_activity(&mut self, cx: &mut Context<Self>) {
        if self.activity.ack_failed {
            return;
        }
        if let Some(session) = self.focused_activity_session() {
            let ids = self
                .activity
                .snapshot
                .events
                .iter()
                .filter(|event| event.session == session && !event.read)
                .map(|event| event.id)
                .collect();
            self.acknowledge_activity(ids, cx);
        }
    }

    pub(crate) fn acknowledge_activity(&mut self, ids: Vec<u64>, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready {
            return;
        }
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| !self.activity.acknowledging.contains(id))
            .collect();
        if !ids.is_empty() && self.send(Work::AcknowledgeActivity(ids.clone()), cx) {
            self.activity.acknowledging.extend(ids);
        }
    }

    pub(super) fn activity_acknowledged(
        &mut self,
        ids: &[u64],
        result: Result<(), ClientError>,
        cx: &mut Context<Self>,
    ) {
        for id in ids {
            self.activity.acknowledging.remove(id);
        }
        match result {
            Ok(()) => {
                self.activity
                    .snapshot
                    .events
                    .retain(|event| !ids.contains(&event.id));
                #[cfg(target_os = "macos")]
                if let Some(notifications) = &self.notifications {
                    notifications.clear(&ids.iter().map(u64::to_string).collect::<Vec<_>>());
                }
                self.refresh_activity(cx);
            }
            Err(error) => {
                self.activity.ack_failed = true;
                self.fail(format!("Could not mark activity read: {error}"), cx);
            }
        }
        cx.notify();
    }

    pub(super) fn deliver_activity(&self, result: Result<Vec<u64>, ClientError>) {
        let Ok(ids) = result else {
            return;
        };
        #[cfg(target_os = "macos")]
        if let Some(notifications) = &self.notifications {
            for event in self.activity_notification_events(&ids) {
                let project = self
                    .state
                    .project(event.project)
                    .map_or("Muxy", |project| project.name.as_str());
                notifications.deliver(
                    &event.id.to_string(),
                    &format!("{} · {}", event.provider.name(), project),
                    event.kind.description(),
                );
            }
        }
    }

    pub(crate) fn navigate_activity(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready
            || !self.activity.loaded
            || self.catalog.restore.is_some()
            || self.catalog.pending
        {
            self.activity.navigation = Some(id);
            return;
        }
        let Some(event) = self
            .activity
            .snapshot
            .events
            .iter()
            .find(|event| event.id == id)
            .cloned()
        else {
            return;
        };
        if !self.has_activity_pane(event.session) {
            return;
        }
        self.acknowledge_activity(vec![id], cx);
        let existing = self.state.projects().iter().flat_map(|p| p.tabs.iter().map(move |t| (p.id, t))).find_map(|(project, tab)| {
            tab.panes.iter().find(|pane| matches!(pane.content, muxy_app_core::PaneContent::Terminal { session: Some(session) } if session == event.session)).map(|pane| (project, tab.id, pane.id))
        });
        if let Some((project, tab, pane)) = existing {
            self.select_project(project, cx);
            self.select_tab(tab, cx);
            self.focus_pane(pane, cx);
            self.dismiss_overlay(cx);
            self.focus_requested = true;
        } else if !self.quick.visible {
            self.toggle_quick_terminal(cx);
        }
    }

    pub(super) fn resume_activity_navigation(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.activity.navigation.take() {
            self.navigate_activity(id, cx);
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn start_notifications(
        cx: &mut Context<Self>,
    ) -> Option<muxy_ui::notifications::Notifications> {
        let (notifications, events) = muxy_ui::notifications::Notifications::new()?;
        cx.spawn(async move |this, cx| {
            while let Ok(id) = events.recv().await {
                let Ok(id) = id.parse() else {
                    continue;
                };
                if this
                    .update(cx, |model, cx| {
                        cx.activate(true);
                        model.navigate_activity(id, cx);
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
