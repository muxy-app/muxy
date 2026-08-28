use notify::{RecursiveMode, Watcher};
use std::path::Path;

#[derive(Clone)]
pub struct RepositoryInvalidationBoundary {
    sender: async_channel::Sender<()>,
}

impl RepositoryInvalidationBoundary {
    pub fn new() -> (Self, async_channel::Receiver<()>) {
        let (sender, receiver) = async_channel::bounded(1);
        (Self { sender }, receiver)
    }

    pub fn invalidate(&self) {
        let _ = self.sender.try_send(());
    }
}

pub struct ActiveRepositoryWatcher {
    _watchers: Vec<notify::RecommendedWatcher>,
}

impl ActiveRepositoryWatcher {
    pub fn new(
        worktree_root: &Path,
        git_dir: &Path,
    ) -> notify::Result<(Self, async_channel::Receiver<()>)> {
        let (boundary, receiver) = RepositoryInvalidationBoundary::new();
        let git_dir_for_worktree = git_dir.to_path_buf();
        let working_tree_boundary = boundary.clone();
        let mut working_tree =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.is_ok_and(|event| {
                    event
                        .paths
                        .iter()
                        .any(|path| working_tree_event_relevant(path, &git_dir_for_worktree))
                }) {
                    working_tree_boundary.invalidate();
                }
            })?;
        working_tree.watch(worktree_root, RecursiveMode::Recursive)?;

        let mut watchers = vec![working_tree];
        for metadata_root in git_metadata_roots(git_dir) {
            let watched_root = metadata_root.path().to_path_buf();
            let scope = metadata_root.scope();
            let metadata_boundary = boundary.clone();
            let mut git_metadata =
                notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                    if event.is_ok_and(|event| {
                        event
                            .paths
                            .iter()
                            .any(|path| git_metadata_event_relevant(path, &watched_root, scope))
                    }) {
                        metadata_boundary.invalidate();
                    }
                })?;
            git_metadata.watch(metadata_root.path(), RecursiveMode::Recursive)?;
            watchers.push(git_metadata);
        }
        Ok((
            Self {
                _watchers: watchers,
            },
            receiver,
        ))
    }
}

pub fn working_tree_event_relevant(path: &Path, git_dir: &Path) -> bool {
    !path.starts_with(git_dir)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GitMetadataScope {
    Primary,
    Worktree,
    Common,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitMetadataRoot {
    path: std::path::PathBuf,
    scope: GitMetadataScope,
}

impl GitMetadataRoot {
    fn path(&self) -> &Path {
        &self.path
    }

    fn scope(&self) -> GitMetadataScope {
        self.scope
    }
}

fn git_metadata_event_relevant(path: &Path, git_dir: &Path, scope: GitMetadataScope) -> bool {
    let Ok(relative) = path.strip_prefix(git_dir) else {
        return false;
    };
    let mut components = relative.components();
    let Some(first) = components.next() else {
        return false;
    };
    let first = first.as_os_str();
    match scope {
        GitMetadataScope::Primary => {
            first == "HEAD"
                || first == "index"
                || first == "config"
                || first == "packed-refs"
                || first == "refs"
        }
        GitMetadataScope::Worktree => first == "HEAD" || first == "index",
        GitMetadataScope::Common => first == "config" || first == "packed-refs" || first == "refs",
    }
}

fn git_metadata_roots(git_dir: &Path) -> Vec<GitMetadataRoot> {
    if let Some(worktrees) = git_dir.parent()
        && worktrees
            .file_name()
            .is_some_and(|name| name == "worktrees")
        && let Some(common) = worktrees.parent()
    {
        return vec![
            GitMetadataRoot {
                path: git_dir.to_path_buf(),
                scope: GitMetadataScope::Worktree,
            },
            GitMetadataRoot {
                path: common.to_path_buf(),
                scope: GitMetadataScope::Common,
            },
        ];
    }
    vec![GitMetadataRoot {
        path: git_dir.to_path_buf(),
        scope: GitMetadataScope::Primary,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn repository_watcher_boundary_coalesces_thousands_at_capacity_one() {
        let (boundary, receiver) = RepositoryInvalidationBoundary::new();
        for _ in 0..10_000 {
            boundary.invalidate();
        }
        assert_eq!(receiver.len(), 1);
        assert!(receiver.try_recv().is_ok());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn repository_watcher_separates_worktree_and_git_metadata_sources() {
        let root = Path::new("/repo");
        let git_dir = Path::new("/repo/.git");
        assert!(working_tree_event_relevant(
            Path::new("/repo/src/main.rs"),
            git_dir
        ));
        assert!(!working_tree_event_relevant(
            Path::new("/repo/.git/HEAD"),
            git_dir
        ));
        assert!(git_metadata_event_relevant(
            Path::new("/repo/.git/HEAD"),
            git_dir,
            GitMetadataScope::Primary,
        ));
        assert!(git_metadata_event_relevant(
            Path::new("/repo/.git/refs/heads/main"),
            git_dir,
            GitMetadataScope::Primary,
        ));
        assert!(!git_metadata_event_relevant(
            Path::new("/repo/.git/objects/aa/bb"),
            git_dir,
            GitMetadataScope::Primary,
        ));
        assert!(root.starts_with("/"));
    }

    #[test]
    fn repository_watcher_includes_linked_worktree_and_common_git_metadata() {
        let git_dir = Path::new("/repo/.git/worktrees/secondary");
        assert_eq!(
            git_metadata_roots(git_dir),
            vec![
                GitMetadataRoot {
                    path: git_dir.to_path_buf(),
                    scope: GitMetadataScope::Worktree,
                },
                GitMetadataRoot {
                    path: Path::new("/repo/.git").to_path_buf(),
                    scope: GitMetadataScope::Common,
                },
            ]
        );
        assert!(git_metadata_event_relevant(
            Path::new("/repo/.git/worktrees/secondary/HEAD"),
            git_dir,
            GitMetadataScope::Worktree,
        ));
        assert!(git_metadata_event_relevant(
            Path::new("/repo/.git/refs/heads/topic"),
            Path::new("/repo/.git"),
            GitMetadataScope::Common,
        ));
        assert!(!git_metadata_event_relevant(
            Path::new("/repo/.git/worktrees/secondary/HEAD"),
            Path::new("/repo/.git"),
            GitMetadataScope::Common,
        ));
        assert!(!git_metadata_event_relevant(
            Path::new("/repo/.git/worktrees/other/index"),
            Path::new("/repo/.git"),
            GitMetadataScope::Common,
        ));
    }

    #[test]
    fn repository_watcher_installation_failure_is_nonfatal() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            ActiveRepositoryWatcher::new(
                &directory.path().join("missing-worktree"),
                &directory.path().join("missing-git-dir")
            )
            .is_err()
        );
    }
}
