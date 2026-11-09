use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{Context, RenderImage, Task};
use muxy_app_core::{Project, ProjectId, ProjectStatus, ServerId};

use super::AppModel;
use crate::project_icon::Source;

#[derive(Default)]
pub(crate) struct ProjectIcons {
    entries: HashMap<ProjectId, Entry>,
    connection_attempts: HashMap<ServerId, (u64, Instant)>,
    revision: u64,
    next_request: u64,
}

#[derive(Clone, PartialEq, Eq)]
struct Key {
    directory: PathBuf,
    server: ServerId,
    generation: u64,
    connection: Option<u64>,
    revision: u64,
}

struct Entry {
    key: Key,
    request: u64,
    image: Option<Arc<RenderImage>>,
    _task: Task<()>,
}

fn eligible(project: &Project) -> bool {
    !project.home
        && project.icon.is_none()
        && project.logo.is_none()
        && project.status() == ProjectStatus::Available
}

impl ProjectIcons {
    pub(crate) fn image(&self, project: &Project) -> Option<Arc<RenderImage>> {
        if !eligible(project) {
            return None;
        }
        self.entries
            .get(&project.id)
            .filter(|entry| {
                entry.key.directory == project.directory && entry.key.server == project.server_id
            })
            .and_then(|entry| entry.image.clone())
    }

    pub(super) fn release(&mut self, cx: &mut gpui::App) {
        for (_, entry) in self.entries.drain() {
            if let Some(image) = entry.image {
                cx.defer(move |cx| cx.drop_image(image, None));
            }
        }
    }

    fn begin_connection_attempt(
        &mut self,
        server: ServerId,
        generation: u64,
        now: Instant,
    ) -> bool {
        if self
            .connection_attempts
            .get(&server)
            .is_some_and(|(previous, next)| *previous == generation && now < *next)
        {
            return false;
        }
        self.connection_attempts
            .insert(server, (generation, now + Duration::from_secs(30)));
        true
    }

    pub(super) fn refresh(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}

impl AppModel {
    pub(crate) fn sync_project_icons(&mut self, cx: &mut Context<Self>) {
        self.project_icons.connection_attempts.retain(|server, _| {
            self.state
                .projects()
                .iter()
                .any(|project| project.server_id == *server)
        });
        self.project_icons.entries.retain(|id, entry| {
            let keep = self.state.project(*id).is_some_and(eligible);
            if !keep && let Some(image) = entry.image.take() {
                cx.defer(move |cx| cx.drop_image(image, None));
            }
            keep
        });
        let projects: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| eligible(project))
            .filter_map(|project| {
                let key = Key {
                    directory: project.directory.clone(),
                    server: project.server_id,
                    generation: self.generation(project.server_id),
                    connection: (!project.server_id.is_local())
                        .then(|| self.extensions.connection_install(project.server_id))
                        .flatten(),
                    revision: if project.server_id.is_local() {
                        self.project_icons.revision
                    } else {
                        0
                    },
                };
                if self
                    .project_icons
                    .entries
                    .get(&project.id)
                    .is_some_and(|entry| entry.key == key)
                {
                    return None;
                }
                Some((project.id, key))
            })
            .collect();
        for (id, key) in projects {
            let Some(source) = self.project_icon_source(id, &key, cx) else {
                continue;
            };
            self.project_icons.next_request = self.project_icons.next_request.wrapping_add(1);
            let request = self.project_icons.next_request;
            let scan = cx
                .background_executor()
                .spawn(async move { crate::project_icon::discover(&source) });
            let task = cx.spawn(async move |model, cx| {
                let image = scan.await.map(Arc::new);
                let _ = model.update(cx, |model, cx| {
                    let Some(entry) = model
                        .project_icons
                        .entries
                        .get_mut(&id)
                        .filter(|entry| entry.request == request)
                    else {
                        return;
                    };
                    if let Some(previous) = std::mem::replace(&mut entry.image, image) {
                        cx.drop_image(previous, None);
                    }
                    cx.notify();
                });
            });
            let previous = self.project_icons.entries.remove(&id);
            let image = previous.and_then(|entry| {
                if entry.key.directory == key.directory && entry.key.server == key.server {
                    return entry.image;
                }
                if let Some(image) = entry.image {
                    cx.defer(move |cx| cx.drop_image(image, None));
                }
                None
            });
            self.project_icons.entries.insert(
                id,
                Entry {
                    key,
                    request,
                    image,
                    _task: task,
                },
            );
        }
    }

    fn project_icon_source(
        &mut self,
        id: ProjectId,
        key: &Key,
        cx: &mut Context<Self>,
    ) -> Option<Source> {
        if key.server.is_local() {
            return Some(Source::Local(key.directory.clone()));
        }
        if !self.ready(key.server)
            || !self.confirmed(key.server)
            || self.state.project_creation_pending(id)
        {
            return None;
        }
        let Some(client) = self.extensions.client(key.server) else {
            if self.project_icons.begin_connection_attempt(
                key.server,
                key.generation,
                Instant::now(),
            ) {
                self.extension_client(key.server, cx);
            }
            return None;
        };
        self.project_icons.connection_attempts.remove(&key.server);
        Some(Source::Remote(client, id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use muxy_app_core::AppState;

    #[test]
    fn project_icon_connection_attempts_back_off_per_server_and_reset_on_reconnect() {
        let mut icons = ProjectIcons::default();
        let server = ServerId::new();
        let other = ServerId::new();
        let now = Instant::now();
        assert!(icons.begin_connection_attempt(server, 1, now));
        assert!(!icons.begin_connection_attempt(server, 1, now));
        assert!(icons.begin_connection_attempt(other, 1, now));
        assert!(!icons.begin_connection_attempt(server, 1, now + Duration::from_secs(29)));
        assert!(icons.begin_connection_attempt(server, 1, now + Duration::from_secs(30)));
        assert!(icons.begin_connection_attempt(server, 2, now + Duration::from_secs(31)));
    }

    #[gpui::test]
    fn project_icons_refresh_cache_and_respect_manual_appearance(cx: &mut TestAppContext) {
        let root = tempfile::tempdir().expect("project directory");
        let path = root.path().join("favicon.png");
        let mut state = AppState::bootstrap().expect("state");
        let id = state
            .add_project(ServerId::local(), root.path().into())
            .expect("project");
        let (boot, _requests) = crate::model::tests::stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, AppModel::sync_project_icons);
        cx.run_until_parked();
        view.update(cx, |model, cx| {
            let request = model.project_icons.entries[&id].request;
            assert!(model.project_icons.entries[&id].image.is_none());
            model.sync_project_icons(cx);
            assert_eq!(model.project_icons.entries[&id].request, request);
            std::fs::write(
                &path,
                include_bytes!("../../../muxy-protocol/tests/fixtures/project-logo.png"),
            )
            .expect("icon");
            model.project_icons.refresh();
            model.sync_project_icons(cx);
        });
        cx.run_until_parked();
        view.update(cx, |model, cx| {
            let project = model.state.project(id).expect("project");
            assert!(model.project_icons.image(project).is_some());
            assert!(project.logo.is_none());
            assert!(project.icon.is_none());
            let mut moved = project.clone();
            moved.directory = root.path().join("moved");
            assert!(model.project_icons.image(&moved).is_none());
            let mut home = project.clone();
            home.home = true;
            assert!(model.project_icons.image(&home).is_none());
            model
                .state
                .set_project_icon(id, Some("sf:star".into()))
                .expect("icon override");
            assert!(
                model
                    .project_icons
                    .image(model.state.project(id).expect("project"))
                    .is_none()
            );
            model.sync_project_icons(cx);
            assert!(!model.project_icons.entries.contains_key(&id));
            model
                .state
                .set_project_icon(id, None)
                .expect("clear override");
            model.sync_project_icons(cx);
        });
        cx.run_until_parked();
        view.update(cx, |model, cx| {
            assert!(
                model
                    .project_icons
                    .image(model.state.project(id).expect("project"))
                    .is_some()
            );
            model
                .state
                .set_project_logo(
                    id,
                    Some(
                        include_bytes!("../../../muxy-protocol/tests/fixtures/project-logo.png")
                            .as_slice()
                            .into(),
                    ),
                )
                .expect("logo override");
            model.sync_project_icons(cx);
            assert!(!model.project_icons.entries.contains_key(&id));
            model.state.set_project_logo(id, None).expect("clear logo");
            model.sync_project_icons(cx);
        });
        cx.run_until_parked();
        view.update(cx, |model, cx| {
            std::fs::remove_file(&path).expect("remove icon");
            model.project_icons.refresh();
            model.sync_project_icons(cx);
        });
        cx.run_until_parked();
        view.update(cx, |model, cx| {
            assert!(model.project_icons.entries[&id].image.is_none());
            model.state.remove_project(id).expect("remove project");
            model.sync_project_icons(cx);
            assert!(!model.project_icons.entries.contains_key(&id));
        });
    }
}
