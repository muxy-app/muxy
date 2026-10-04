use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{AppContext, Context, Entity, Focusable, Window};
use muxy_app_core::project_layouts::{Config, Descriptor};
use muxy_app_core::{PaneId, ProjectId, ProjectStatus, TabId};
use muxy_ui::icon::Icon;
use muxy_ui::picker::{
    Picker, PickerConfig, PickerEvent, PickerItem, PickerLeading, PickerRow, PickerStatus,
};

use super::{AppModel, ConnectionState, Quitting};
use crate::boot::Work;
use crate::views::overlays::Overlay;

#[derive(Default)]
pub(crate) struct ProjectLayouts {
    active: Option<(ProjectId, PathBuf)>,
    next: u64,
    listings: HashMap<ProjectId, Listing>,
}

struct Listing {
    request: u64,
    entries: Vec<Descriptor>,
    pending: bool,
    error: Option<String>,
}

pub(crate) struct LayoutPicker {
    pub(crate) picker: Entity<Picker>,
    project: ProjectId,
    directory: PathBuf,
    request: u64,
    loading: bool,
}

impl AppModel {
    pub(crate) fn has_project_layouts(&self) -> bool {
        self.project_layouts
            .listings
            .get(&self.state.current_project().id)
            .is_some_and(|listing| !listing.entries.is_empty())
    }

    pub(super) fn sync_project_layouts(&mut self, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready
            || !self
                .state
                .project_intents(self.state.current_project().server_id)
                .is_empty()
            || self.catalog.restore.is_some()
        {
            return;
        }
        self.project_layouts
            .listings
            .retain(|id, _| self.state.project(*id).is_some());
        let project = self.state.current_project();
        let active = (project.id, project.directory.clone());
        if project.status() != ProjectStatus::Available
            || self.project_layouts.active.as_ref() == Some(&active)
        {
            return;
        }
        self.project_layouts.active = Some(active.clone());
        if !matches!(&self.overlay, Some(Overlay::Layouts(picker)) if picker.project == active.0) {
            self.request_project_layouts(active.0, cx);
        }
    }

    fn request_project_layouts(&mut self, project: ProjectId, cx: &mut Context<Self>) -> u64 {
        self.project_layouts.next = self.project_layouts.next.wrapping_add(1);
        let request = self.project_layouts.next;
        let sent = self.send(Work::ProjectLayouts { project, request }, cx);
        self.project_layouts.listings.insert(
            project,
            Listing {
                request,
                entries: Vec::new(),
                pending: sent,
                error: (!sent).then(|| "Reconnect to load project layouts".into()),
            },
        );
        request
    }

    pub(crate) fn open_layout_picker(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some()
            || self.close_request.is_some()
            || self.quitting != Quitting::Idle
        {
            return;
        }
        let Some(target) = self
            .state
            .project(project)
            .filter(|project| project.status() == ProjectStatus::Available)
        else {
            return;
        };
        let directory = target.directory.clone();
        self.dismiss_overlay(cx);
        let request = self.request_project_layouts(project, cx);
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig::new("project-layouts", "Filter layouts…"),
                self.theme.clone(),
                self.metrics,
                cx,
            )
        });
        picker.focus_handle(cx).focus(window);
        self.overlay_subscription =
            Some(cx.subscribe(&picker, |model, _, event, cx| match event {
                PickerEvent::Confirmed(selection) => {
                    model.choose_project_layout(selection.id.as_ref(), cx);
                }
                PickerEvent::Dismissed => model.dismiss_overlay(cx),
                PickerEvent::QueryChanged { .. } => model.update_layout_picker(cx),
                _ => {}
            }));
        self.overlay = Some(Overlay::Layouts(LayoutPicker {
            picker,
            project,
            directory,
            request,
            loading: false,
        }));
        self.update_layout_picker(cx);
        cx.notify();
    }

    pub(super) fn receive_project_layouts(
        &mut self,
        project: ProjectId,
        request: u64,
        result: Result<Vec<Descriptor>, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(listing) = self
            .project_layouts
            .listings
            .get_mut(&project)
            .filter(|listing| listing.request == request)
        else {
            return;
        };
        listing.pending = false;
        match result {
            Ok(entries) => {
                listing.entries = entries;
                listing.error = None;
            }
            Err(error) => {
                listing.entries.clear();
                listing.error = Some(error);
            }
        }
        self.update_layout_picker(cx);
        cx.notify();
    }

    fn update_layout_picker(&self, cx: &mut Context<Self>) {
        let Some(Overlay::Layouts(overlay)) = &self.overlay else {
            return;
        };
        let Some(listing) = self.project_layouts.listings.get(&overlay.project) else {
            return;
        };
        let query = overlay.picker.read(cx).query().to_lowercase();
        let items: Vec<_> = listing
            .entries
            .iter()
            .enumerate()
            .filter(|(_, layout)| layout.name.to_lowercase().contains(&query))
            .map(|(index, layout)| {
                let mut row = PickerRow::new(index.to_string(), layout.name.clone());
                row.leading = Some(PickerLeading::Icon(Icon::LayoutSplit));
                row.detail = Some(String::from_utf8_lossy(&layout.path.0).into_owned().into());
                PickerItem::Row(row)
            })
            .collect();
        let status = if overlay.loading {
            PickerStatus::Loading("Loading layout…".into())
        } else if listing.pending {
            PickerStatus::Loading("Loading layouts…".into())
        } else if let Some(error) = &listing.error {
            PickerStatus::Error(format!("Could not load .muxy/layouts: {error}").into())
        } else if items.is_empty() {
            PickerStatus::Empty("No layouts found in .muxy/layouts".into())
        } else {
            PickerStatus::Ready
        };
        overlay.picker.update(cx, |picker, cx| {
            picker.set_items(items, cx);
            picker.set_status(status, cx);
        });
    }

    pub(super) fn choose_project_layout(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(Overlay::Layouts(overlay)) = &mut self.overlay else {
            return;
        };
        if overlay.loading {
            return;
        }
        let Some(layout) = id
            .parse::<usize>()
            .ok()
            .and_then(|index| {
                self.project_layouts
                    .listings
                    .get(&overlay.project)?
                    .entries
                    .get(index)
            })
            .cloned()
        else {
            return;
        };
        let project = overlay.project;
        let request = overlay.request;
        overlay.loading = true;
        self.send(
            Work::LoadProjectLayout {
                project,
                request,
                layout,
            },
            cx,
        );
        self.update_layout_picker(cx);
    }

    pub(super) fn receive_project_layout(
        &mut self,
        project: ProjectId,
        request: u64,
        layout: &Descriptor,
        result: Result<Config, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::Layouts(overlay)) = &mut self.overlay else {
            return;
        };
        if overlay.project != project || overlay.request != request || !overlay.loading {
            return;
        }
        overlay.loading = false;
        let directory = overlay.directory.clone();
        let config = match result {
            Ok(config) => config,
            Err(error) => {
                self.fail(
                    format!("Could not load layout “{}”: {error}", layout.name),
                    cx,
                );
                self.update_layout_picker(cx);
                return;
            }
        };
        let Some(target) = self.state.project(project) else {
            return;
        };
        let panes: Vec<_> = target
            .tabs
            .iter()
            .flat_map(|tab| tab.panes.iter().map(|pane| (tab.id, pane.id)))
            .collect();
        let message = format!(
            "Apply “{}” to “{}”? This replaces all of this project's tabs, including pinned tabs, and runs the layout's commands. Terminals no longer shown by any app will end. Only apply layouts you trust.",
            layout.name, target.name
        );
        self.confirm(
            "Apply Layout?",
            message,
            "Apply Layout",
            cx,
            move |model, cx| {
                model.apply_project_layout(project, &directory, &panes, &config, cx);
            },
        );
    }

    fn apply_project_layout(
        &mut self,
        project: ProjectId,
        directory: &std::path::Path,
        panes: &[(TabId, PaneId)],
        config: &Config,
        cx: &mut Context<Self>,
    ) {
        if self.quitting != Quitting::Idle || self.close_request.is_some() {
            return;
        }
        let Some(target) = self.state.project(project) else {
            return;
        };
        if target.directory != directory
            || !target
                .tabs
                .iter()
                .flat_map(|tab| tab.panes.iter().map(|pane| (tab.id, pane.id)))
                .eq(panes.iter().copied())
        {
            self.fail(
                "The project's tabs changed. Choose the layout again to confirm replacement."
                    .into(),
                cx,
            );
            return;
        }
        let mut panes = Vec::new();
        if !self.edit_tab(
            |state| {
                panes = state.apply_project_layout(project, config)?;
                Ok(())
            },
            cx,
        ) {
            return;
        }
        self.cancel_titlebar_drag(cx);
        self.changed(cx);
        self.discard_pending(cx);
        self.focus_requested = true;
        if self.connection == ConnectionState::Ready {
            for pane in panes {
                self.start_attach(pane, muxy_protocol::Size { cols: 80, rows: 24 }, cx);
            }
        } else if self.connection == ConnectionState::Disconnected {
            self.connect(cx);
        }
    }
}
