use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use muxy_protocol::{
    FileChanges, FilesAction, FilesReply, FilesRequest, Message, ProjectId, ServerPath,
};
use notify::EventKind;

use crate::watch::{DebouncedWatch, MAX_DELAY, Subscribers, Subscription, Timing};
use crate::{Registry, connection::Outbox};

use super::{Result, error, root::Root};

const TIMING: Timing = Timing {
    quiet: Duration::from_millis(300),
    max_delay: MAX_DELAY,
};

/// The projects one connection watches.
#[derive(Default)]
pub(crate) struct Subscriptions {
    watches: HashMap<ProjectId, Subscription>,
}

impl Subscriptions {
    pub(crate) fn request(
        &mut self,
        registry: &Registry,
        output: &Arc<Outbox>,
        request: &FilesRequest,
    ) -> Result<FilesReply> {
        if request.action == FilesAction::Unwatch {
            self.watches.remove(&request.project);
            return Ok(FilesReply::Done);
        }
        if self.watches.contains_key(&request.project) {
            return Ok(FilesReply::Done);
        }
        if self.watches.len() >= 32 {
            return Err(error("Too many file subscriptions"));
        }
        let project = registry.catalog.project(request.project)?;
        let root = Root::open(project.directory.as_path())?;
        let project = project.id;
        let subscription = registry
            .files
            .watches
            .subscribe(project, output, |subscribers| {
                watch(&root.path, project, subscribers).map(Some)
            })?
            .ok_or_else(|| error("Nothing to watch"))?;
        self.watches.insert(project, subscription);
        Ok(FilesReply::Done)
    }
}

fn watch(root: &Path, project: ProjectId, subscribers: Subscribers) -> Result<DebouncedWatch> {
    let directory = root.to_owned();
    DebouncedWatch::new(
        "file-watch",
        TIMING,
        notify::Config::default().with_follow_symlinks(false),
        &[root],
        move |pending: &mut FileChanges, event: notify::Result<notify::Event>| {
            match event {
                Ok(event) if matches!(event.kind, EventKind::Access(_)) => return false,
                Ok(event) => {
                    if event.need_rescan() {
                        pending.merge(&FileChanges {
                            paths: Vec::new(),
                            rescan: true,
                        });
                    }
                    for path in &event.paths {
                        let Ok(relative) = path.strip_prefix(&directory) else {
                            continue;
                        };
                        if relative.components().any(|part| part.as_os_str() == ".git") {
                            continue;
                        }
                        let path = ServerPath::from(relative);
                        if path.0.len() > muxy_protocol::MAX_PATH {
                            pending.merge(&FileChanges {
                                paths: Vec::new(),
                                rescan: true,
                            });
                        } else {
                            pending.merge(&FileChanges {
                                paths: vec![path],
                                rescan: false,
                            });
                        }
                    }
                }
                Err(_) => pending.merge(&FileChanges {
                    paths: Vec::new(),
                    rescan: true,
                }),
            }
            pending.rescan || !pending.paths.is_empty()
        },
        move |changes: FileChanges| {
            if changes.rescan || !changes.paths.is_empty() {
                subscribers.send(&Message::FilesChanged { project, changes });
            }
        },
    )
}
