use std::collections::HashSet;

use gpui::Context;
use muxy_client::ClientError;
use muxy_protocol::{ActivitySnapshot, ProjectSession, SessionId};

use super::{AppModel, ConnectionState, Work};

#[derive(Default)]
pub(crate) struct ActivityView {
    pub(crate) snapshot: ActivitySnapshot,
    pub(crate) dirty: u64,
    pub(crate) pending: bool,
    pub(crate) loaded: bool,
    pub(super) navigation: Option<u64>,
    acknowledging: HashSet<u64>,
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
            Ok(snapshot) => {
                #[cfg(target_os = "macos")]
                if let Some(notifications) = &self.notifications {
                    let read: Vec<_> = snapshot
                        .events
                        .iter()
                        .filter(|event| event.read)
                        .map(|event| event.id.to_string())
                        .collect();
                    notifications.clear(&read);
                }
                let ids = muxy_app_core::activity::new_notifications(
                    self.activity.loaded.then_some(&self.activity.snapshot),
                    &snapshot,
                    self.focused_activity_session(),
                );
                self.activity.snapshot = snapshot;
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

    pub(crate) fn unread_activity_count(&self) -> usize {
        self.activity
            .snapshot
            .events
            .iter()
            .filter(|event| !event.read)
            .count()
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
                // A server-confirmed cache update; the next snapshot remains authoritative.
                for event in &mut self.activity.snapshot.events {
                    if ids.contains(&event.id) {
                        event.read = true;
                    }
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
            for event in self.activity.snapshot.events.iter().filter(|event| {
                ids.contains(&event.id)
                    && !event.read
                    && Some(event.session) != self.focused_activity_session()
            }) {
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
        } else if self.state.project(event.project).is_some() {
            self.send(
                Work::OpenActivitySession {
                    project: event.project,
                    session: event.session,
                },
                cx,
            );
        }
    }

    pub(super) fn resume_activity_navigation(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.activity.navigation.take() {
            self.navigate_activity(id, cx);
        }
    }

    pub(super) fn open_activity_session(
        &mut self,
        result: Result<Option<ProjectSession>, ClientError>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Some(session)) if session.status == muxy_protocol::SessionStatus::Live => {
                self.select_project(session.info.project, cx);
                self.open_existing_session(session.info.project, &session, cx);
                self.focus_requested = true;
            }
            Ok(_) => self.fail("This terminal session has ended".into(), cx),
            Err(error) => self.fail(format!("Could not open terminal: {error}"), cx),
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
