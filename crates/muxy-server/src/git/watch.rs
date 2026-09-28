use super::{Result, error, is_repository, path, run};
use crate::Registry;
use muxy_protocol::ProjectId;
use notify::{EventKind, RecursiveMode, Watcher};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::thread::JoinHandle;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(800);

pub(crate) struct RepositoryWatch {
    _watcher: notify::RecommendedWatcher,
    stop: Arc<AtomicBool>,
    wake: SyncSender<()>,
    task: Option<JoinHandle<()>>,
}

#[derive(Clone)]
struct Roots {
    worktree: PathBuf,
    git: PathBuf,
    common: PathBuf,
}

impl Roots {
    fn relevant(&self, path: &Path) -> bool {
        if let Ok(relative) = path.strip_prefix(&self.git) {
            return metadata_relevant(relative, true, self.git == self.common);
        }
        if let Ok(relative) = path.strip_prefix(&self.common) {
            return metadata_relevant(relative, false, true);
        }
        path.starts_with(&self.worktree)
    }
}

fn metadata_relevant(relative: &Path, worktree: bool, common: bool) -> bool {
    let mut components = relative.components();
    let Some(first) = components.next() else {
        return true;
    };
    let first = first.as_os_str();
    (worktree && (first == "HEAD" || first == "index"))
        || (common
            && (first == "config"
                || first == "packed-refs"
                || first == "refs"
                // A linked worktree added or removed, not work inside one.
                || (first == "worktrees" && components.nth(1).is_none())))
}

impl Registry {
    pub(crate) fn watch_git(
        &self,
        project: ProjectId,
        invalidated: impl Fn() + Send + 'static,
    ) -> Result<Option<RepositoryWatch>> {
        let project = self.catalog.project(project)?;
        let directory = path(&project.directory);
        if !directory.is_dir() || !is_repository(directory)? {
            return Ok(None);
        }
        let resolve = |name: &str| -> Result<PathBuf> {
            let value = run(directory, &["rev-parse", "--path-format=absolute", name])?;
            Path::new(OsStr::from_bytes(
                value.strip_suffix(b"\n").unwrap_or(&value),
            ))
            .canonicalize()
            .map_err(error)
        };
        RepositoryWatch::new(
            &Roots {
                worktree: resolve("--show-toplevel")?,
                git: resolve("--git-dir")?,
                common: resolve("--git-common-dir")?,
            },
            invalidated,
        )
        .map(Some)
    }
}

impl RepositoryWatch {
    fn new(roots: &Roots, invalidated: impl Fn() + Send + 'static) -> Result<Self> {
        let (wake, events) = mpsc::sync_channel(1);
        let notify = wake.clone();
        let filter = roots.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.is_ok_and(|event| {
                    !matches!(event.kind, EventKind::Access(_))
                        && (event.need_rescan()
                            || event.paths.iter().any(|path| filter.relevant(path)))
                }) {
                    let _ = notify.try_send(());
                }
            })
            .map_err(error)?;
        watcher
            .watch(&roots.worktree, RecursiveMode::Recursive)
            .map_err(error)?;
        if !roots.common.starts_with(&roots.worktree) {
            watcher
                .watch(&roots.common, RecursiveMode::Recursive)
                .map_err(error)?;
        }
        if !roots.git.starts_with(&roots.worktree) && !roots.git.starts_with(&roots.common) {
            watcher
                .watch(&roots.git, RecursiveMode::Recursive)
                .map_err(error)?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::clone(&stop);
        let task = std::thread::Builder::new()
            .name("git-watch".into())
            .spawn(move || {
                while events.recv().is_ok() {
                    loop {
                        if cancelled.load(Ordering::Acquire) {
                            return;
                        }
                        match events.recv_timeout(DEBOUNCE) {
                            Ok(()) => (),
                            Err(RecvTimeoutError::Timeout) => {
                                if !cancelled.load(Ordering::Acquire) {
                                    invalidated();
                                }
                                break;
                            }
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
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

impl Drop for RepositoryWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_worktree_and_shared_metadata_without_self_triggering_on_git_reads() {
        let roots = Roots {
            worktree: "/repo".into(),
            git: "/repo/.git".into(),
            common: "/repo/.git".into(),
        };
        for path in [
            "/repo/src/code.rs",
            "/repo/.git/HEAD",
            "/repo/.git/index",
            "/repo/.git/refs/heads/main",
            "/repo/.git/worktrees",
            "/repo/.git/worktrees/topic",
        ] {
            assert!(roots.relevant(Path::new(path)));
        }
        for path in [
            "/other/file",
            "/repo/.git/index.lock",
            "/repo/.git/objects/aa/bb",
            "/repo/.git/logs/HEAD",
            "/repo/.git/worktrees/topic/index",
            "/repo/.git/worktrees/topic/logs/HEAD",
        ] {
            assert!(!roots.relevant(Path::new(path)));
        }
        let roots = Roots {
            worktree: "/checkout".into(),
            git: "/repo/.git/worktrees/topic".into(),
            common: "/repo/.git".into(),
        };
        for path in [
            "/checkout/file",
            "/repo/.git/worktrees/topic/index",
            "/repo/.git/refs/heads/main",
            "/repo/.git/packed-refs",
            "/repo/.git/worktrees/other",
        ] {
            assert!(roots.relevant(Path::new(path)));
        }
        for path in [
            "/repo/.git/worktrees/topic/logs/HEAD",
            "/repo/.git/worktrees/other/index",
        ] {
            assert!(!roots.relevant(Path::new(path)));
        }
    }
}
