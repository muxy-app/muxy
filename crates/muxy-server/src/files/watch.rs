use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use muxy_protocol::{FileChanges, FilesAction, FilesReply, FilesRequest, Message, ProjectId};
use notify::{EventKind, RecursiveMode, Watcher};

use crate::{Registry, connection::Outbox};

use super::{Result, error, path, root::Root, wire_path};

#[derive(Default)]
pub(crate) struct Subscriptions {
    watches: HashMap<ProjectId, FileWatch>,
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
        if self.watches.len() >= 32 && !self.watches.contains_key(&request.project) {
            return Err(error("Too many file subscriptions"));
        }
        let project = registry.catalog.project(request.project)?;
        let root = Root::open(path(&project.directory))?;
        let events = Arc::downgrade(output);
        let project = project.id;
        let watch = FileWatch::new(&root.path, move |changes| {
            if let Some(events) = events.upgrade() {
                events.push_control(Message::FilesChanged { project, changes });
            }
        })?;
        self.watches.insert(project, watch);
        Ok(FilesReply::Done)
    }
}

struct FileWatch {
    _watcher: notify::RecommendedWatcher,
    stop: Arc<AtomicBool>,
    wake: mpsc::SyncSender<()>,
    task: Option<JoinHandle<()>>,
}

impl FileWatch {
    fn new(root: &Path, publish: impl Fn(FileChanges) + Send + 'static) -> Result<Self> {
        let (wake, events) = mpsc::sync_channel(1);
        let pending = Arc::new(Mutex::new(FileChanges::default()));
        let notify = wake.clone();
        let changes = Arc::clone(&pending);
        let directory = root.to_owned();
        let mut watcher = notify::RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| {
                let mut pending = changes.lock().unwrap_or_else(PoisonError::into_inner);
                match event {
                    Ok(event) if matches!(event.kind, EventKind::Access(_)) => return,
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
                            let path = wire_path(relative);
                            if path.0.len() > 4096 {
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
                if pending.rescan || !pending.paths.is_empty() {
                    let _ = notify.try_send(());
                }
            },
            notify::Config::default().with_follow_symlinks(false),
        )
        .map_err(error)?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(error)?;
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::clone(&stop);
        let task = std::thread::Builder::new()
            .name("file-watch".into())
            .spawn(move || {
                while events.recv().is_ok() {
                    let deadline = Instant::now() + Duration::from_millis(300);
                    while !cancelled.load(Ordering::Acquire) {
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() || events.recv_timeout(remaining).is_err() {
                            break;
                        }
                    }
                    if cancelled.load(Ordering::Acquire) {
                        return;
                    }
                    let changes = std::mem::take(
                        &mut *pending.lock().unwrap_or_else(PoisonError::into_inner),
                    );
                    if changes.rescan || !changes.paths.is_empty() {
                        publish(changes);
                    }
                }
            })
            .map_err(error)?;
        Ok(Self {
            _watcher: watcher,
            stop,
            wake,
            task: Some(task),
        })
    }
}

impl Drop for FileWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}
