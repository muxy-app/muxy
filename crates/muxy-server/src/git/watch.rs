use super::{Result, command, error, is_repository, run};
use crate::Registry;
use crate::connection::Connection;
use crate::watch::{DebouncedWatch, MAX_DELAY, Subscribers, Subscription, Timing};
use muxy_protocol::{Message, ProjectId};
use notify::EventKind;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const TIMING: Timing = Timing {
    quiet: Duration::from_millis(800),
    max_delay: MAX_DELAY,
};
/// Changed paths checked against the ignore rules at once. A bigger batch
/// counts as a change without checking.
const MAX_CHECKED_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone)]
struct Roots {
    worktree: PathBuf,
    git: PathBuf,
    common: PathBuf,
}

enum Change<'a> {
    /// Git's own state, or anything else that always counts.
    Certain,
    /// A working-tree path, relative to its root. It counts unless Git ignores it.
    Worktree(&'a Path),
}

impl Roots {
    fn resolve(directory: &Path) -> Result<Self> {
        let resolve = |name: &str| -> Result<PathBuf> {
            let value = run(directory, &["rev-parse", "--path-format=absolute", name])?;
            Path::new(OsStr::from_bytes(
                value.strip_suffix(b"\n").unwrap_or(&value),
            ))
            .canonicalize()
            .map_err(error)
        };
        Ok(Self {
            worktree: resolve("--show-toplevel")?,
            git: resolve("--git-dir")?,
            common: resolve("--git-common-dir")?,
        })
    }

    fn classify<'a>(&self, path: &'a Path) -> Option<Change<'a>> {
        if let Ok(relative) = path.strip_prefix(&self.git) {
            return metadata_relevant(relative, true, self.git == self.common)
                .then_some(Change::Certain);
        }
        if let Ok(relative) = path.strip_prefix(&self.common) {
            return metadata_relevant(relative, false, true).then_some(Change::Certain);
        }
        let relative = path.strip_prefix(&self.worktree).ok()?;
        Some(if is_root_or_nested_metadata(relative) {
            Change::Certain
        } else {
            Change::Worktree(relative)
        })
    }

    fn folders(&self) -> Vec<PathBuf> {
        let mut folders = vec![self.worktree.clone()];
        if !self.common.starts_with(&self.worktree) {
            folders.push(self.common.clone());
        }
        if !self.git.starts_with(&self.worktree) && !self.git.starts_with(&self.common) {
            folders.push(self.git.clone());
        }
        folders
    }
}

/// Paths `git check-ignore` can't judge.
fn is_root_or_nested_metadata(relative: &Path) -> bool {
    relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|part| part == Component::Normal(OsStr::new(".git")))
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

/// What changed during one debounce window.
#[derive(Default)]
struct Changes {
    certain: bool,
    paths: HashSet<PathBuf>,
    bytes: usize,
}

impl Changes {
    /// Returns whether anything may need reporting.
    fn record(&mut self, roots: &Roots, event: notify::Result<notify::Event>) -> bool {
        match event {
            Ok(event) if matches!(event.kind, EventKind::Access(_)) => (),
            Ok(event) if event.need_rescan() => self.certain = true,
            Ok(event) => {
                for path in &event.paths {
                    match roots.classify(path) {
                        Some(Change::Certain) => self.certain = true,
                        Some(Change::Worktree(relative)) => self.add(relative),
                        None => (),
                    }
                }
            }
            Err(_) => (),
        }
        if self.certain {
            self.paths.clear();
        }
        self.certain || !self.paths.is_empty()
    }

    fn add(&mut self, relative: &Path) {
        if self.certain || self.paths.contains(relative) {
            return;
        }
        self.bytes += relative.as_os_str().len() + 1;
        if self.bytes > MAX_CHECKED_BYTES {
            self.certain = true;
        } else {
            self.paths.insert(relative.to_owned());
        }
    }

    /// Whether Git would report something new: changes to ignored files,
    /// such as build output, don't count. Asks Git once for the whole batch.
    fn matter(self, worktree: &Path) -> bool {
        if self.certain {
            return true;
        }
        if self.paths.is_empty() {
            return false;
        }
        let mut input = Vec::with_capacity(self.bytes);
        for path in &self.paths {
            input.extend_from_slice(path.as_os_str().as_bytes());
            input.push(0);
        }
        match command::ignored(worktree, &input) {
            Ok(ignored) => {
                ignored
                    .split(|byte| *byte == 0)
                    .filter(|path| !path.is_empty())
                    .count()
                    < self.paths.len()
            }
            Err(_) => true,
        }
    }
}

impl Registry {
    /// Subscribes `connection` to the project's repository changes. `None` when
    /// the project isn't a repository.
    pub(crate) fn watch_git(
        &self,
        project: ProjectId,
        connection: &Arc<Connection>,
    ) -> Result<Option<Subscription>> {
        let descriptor = self.catalog.project(project)?;
        let directory = descriptor.directory.as_path();
        self.git
            .watches
            .subscribe(project, connection, |subscribers| {
                if !directory.is_dir() || !is_repository(directory)? {
                    return Ok(None);
                }
                watch(Roots::resolve(directory)?, project, subscribers).map(Some)
            })
    }
}

fn watch(roots: Roots, project: ProjectId, subscribers: Subscribers) -> Result<DebouncedWatch> {
    let folders = roots.folders();
    let folders: Vec<&Path> = folders.iter().map(PathBuf::as_path).collect();
    let worktree = roots.worktree.clone();
    DebouncedWatch::new(
        "git-watch",
        TIMING,
        notify::Config::default(),
        &folders,
        move |changes: &mut Changes, event| changes.record(&roots, event),
        move |changes: Changes| {
            if changes.matter(&worktree) {
                subscribers.send(&Message::GitChanged { project });
            }
        },
    )
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
            assert!(roots.classify(Path::new(path)).is_some());
        }
        for path in [
            "/other/file",
            "/repo/.git/index.lock",
            "/repo/.git/objects/aa/bb",
            "/repo/.git/logs/HEAD",
            "/repo/.git/worktrees/topic/index",
            "/repo/.git/worktrees/topic/logs/HEAD",
        ] {
            assert!(roots.classify(Path::new(path)).is_none());
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
            assert!(roots.classify(Path::new(path)).is_some());
        }
        for path in [
            "/repo/.git/worktrees/topic/logs/HEAD",
            "/repo/.git/worktrees/other/index",
        ] {
            assert!(roots.classify(Path::new(path)).is_none());
        }
    }

    #[test]
    fn changes_to_ignored_files_alone_do_not_count() {
        let repository = super::super::tests::Repository::new();
        let batch = |paths: &[&str]| {
            let mut changes = Changes::default();
            for path in paths {
                changes.add(Path::new(path));
            }
            changes.matter(&repository.path)
        };
        assert!(!batch(&["target/debug/build.log", "target/gone/output"]));
        assert!(batch(&["target/debug/build.log", "src/main.rs"]));
        assert!(batch(&[".gitignore"]));
    }
}
