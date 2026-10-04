use std::collections::HashMap;

use super::overlays::Overlay;
use crate::{boot::Work, model::AppModel};
use gpui::{AppContext, Context, Focusable, Window};
use muxy_app_core::ProjectId;
use muxy_protocol::{ProjectSession, ProjectSessions, SessionId};
use muxy_ui::icon::Icon;
use muxy_ui::picker::{
    Picker, PickerConfig, PickerEvent, PickerItem, PickerLeading, PickerRow, PickerStatus,
};

#[derive(Default)]
pub(crate) struct ExistingSessions {
    pub(crate) revision: u64,
    projects: HashMap<ProjectId, Listing>,
}

#[derive(Default)]
struct Listing {
    entries: Vec<ProjectSession>,
    revision: Option<u64>,
    pending: bool,
    error: Option<String>,
}

pub(crate) struct SessionPicker {
    pub(crate) project: ProjectId,
    pub(crate) picker: gpui::Entity<Picker>,
}

impl AppModel {
    pub(crate) fn open_session_picker(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig::new("project-terminals", "Filter terminals or owners…"),
                self.theme.clone(),
                self.metrics,
                cx,
            )
        });
        picker.focus_handle(cx).focus(window);
        self.overlay_subscription =
            Some(cx.subscribe(&picker, |model, _, event, cx| match event {
                PickerEvent::Confirmed(selection) => {
                    model.choose_existing_session(selection.id.as_ref(), cx);
                }
                PickerEvent::Dismissed => model.dismiss_overlay(cx),
                PickerEvent::QueryChanged { .. } => model.update_session_picker(cx),
                _ => {}
            }));
        self.overlay = Some(Overlay::Sessions(SessionPicker { project, picker }));
        self.refresh_session_picker(cx);
    }

    pub(crate) fn existing_terminal_count(&self) -> usize {
        self.available_sessions(self.state.current_project().id)
            .len()
    }

    fn available_sessions(&self, project: ProjectId) -> Vec<&ProjectSession> {
        let references = self
            .state
            .project_server(project)
            .map(|server| self.state.session_references(server))
            .unwrap_or_default();
        self.existing_sessions
            .projects
            .get(&project)
            .into_iter()
            .flat_map(|listing| &listing.entries)
            .filter(|session| !references.contains(&session.info.id) && !session.attached)
            .collect()
    }

    pub(crate) fn refresh_existing_sessions(&mut self, cx: &mut Context<Self>) {
        if !self.session_listing_ready() {
            return;
        }
        let project = self.state.current_project().id;
        self.request_sessions(project, cx);
        if let Some(Overlay::Sessions(picker)) = &self.overlay {
            self.request_sessions(picker.project, cx);
        }
        self.update_session_picker(cx);
    }

    pub(crate) fn refresh_session_picker(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::Sessions(picker)) = &self.overlay {
            self.existing_sessions
                .projects
                .entry(picker.project)
                .or_default()
                .revision = None;
        }
        self.refresh_existing_sessions(cx);
    }

    fn request_sessions(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let listing = self.existing_sessions.projects.entry(project).or_default();
        if listing.pending
            || listing
                .revision
                .is_some_and(|revision| revision >= self.existing_sessions.revision)
        {
            return;
        }
        if self.send_session_request(Work::ProjectSessions { project }, cx) {
            let listing = self.existing_sessions.projects.entry(project).or_default();
            listing.pending = true;
            listing.error = None;
        }
    }

    pub(crate) fn receive_session_page(
        &mut self,
        project: ProjectId,
        result: Result<ProjectSessions, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        let listing = self.existing_sessions.projects.entry(project).or_default();
        listing.pending = false;
        match result {
            Ok(page) => {
                listing.entries = page.sessions;
                listing.revision = Some(page.revision);
                listing.error = None;
            }
            Err(error) => {
                listing.entries.clear();
                listing.revision = Some(self.existing_sessions.revision);
                listing.error = Some(error.to_string());
            }
        }
        self.refresh_existing_sessions(cx);
        cx.notify();
    }

    pub(crate) fn update_session_picker(&self, cx: &mut Context<Self>) {
        let Some(Overlay::Sessions(picker)) = &self.overlay else {
            return;
        };
        let query = picker.picker.read(cx).query().to_lowercase();
        let available = self.available_sessions(picker.project);
        let items: Vec<_> = available
            .into_iter()
            .filter_map(|session| {
                let owner = session
                    .owner
                    .map_or_else(|| "No owner".to_owned(), |owner| format!("Owner: {owner}"));
                let directory = String::from_utf8_lossy(&session.info.directory.0);
                let title = format!("Terminal {}", session.info.id.get());
                if !format!("{title} {directory} {owner}")
                    .to_lowercase()
                    .contains(&query)
                {
                    return None;
                }
                let mut row = PickerRow::new(session.info.id.get().to_string(), title);
                row.detail = Some(directory.into_owned().into());
                row.leading = Some(PickerLeading::Icon(Icon::Terminal));
                row.trailing = Some(owner.into());
                Some(PickerItem::Row(row))
            })
            .collect();
        let listing = self.existing_sessions.projects.get(&picker.project);
        let status = if !self.session_listing_ready() {
            PickerStatus::Error("Reconnect to see existing terminals".into())
        } else if let Some(error) = listing.and_then(|listing| listing.error.as_ref()) {
            PickerStatus::Error(error.clone().into())
        } else if listing.is_none_or(|listing| listing.revision.is_none() && listing.pending) {
            PickerStatus::Loading("Loading terminals…".into())
        } else if items.is_empty() {
            PickerStatus::Empty(
                if query.is_empty() {
                    "No other terminals in this project"
                } else {
                    "No matching terminals"
                }
                .into(),
            )
        } else {
            PickerStatus::Ready
        };
        picker.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
        });
    }

    fn choose_existing_session(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(Overlay::Sessions(picker)) = &self.overlay else {
            return;
        };
        let project = picker.project;
        let Some(session) = id
            .parse()
            .ok()
            .and_then(SessionId::new)
            .and_then(|id| {
                self.available_sessions(project)
                    .into_iter()
                    .find(|entry| entry.info.id == id)
            })
            .cloned()
        else {
            return;
        };
        self.open_existing_session(project, &session, cx);
    }
}
