use crate::git;
use notify::{RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Watchers {
    watchers: HashMap<String, (Vec<PathBuf>, notify::RecommendedWatcher)>,
    sender: async_channel::Sender<String>,
}

impl Watchers {
    pub fn new() -> (Self, async_channel::Receiver<String>) {
        let (sender, receiver) = async_channel::unbounded();
        (
            Self {
                watchers: HashMap::new(),
                sender,
            },
            receiver,
        )
    }

    pub fn sync(&mut self, projects: &[(String, String)]) {
        self.watchers
            .retain(|id, _| projects.iter().any(|(project_id, _)| project_id == id));

        for (project_id, path) in projects {
            let desired = desired_paths(path);
            if desired.is_empty() {
                self.watchers.remove(project_id);
                continue;
            }
            if self
                .watchers
                .get(project_id)
                .is_some_and(|(watched, _)| watched == &desired)
            {
                continue;
            }
            if let Some(watcher) = self.install(project_id, &desired) {
                self.watchers.insert(project_id.clone(), (desired, watcher));
            }
        }
    }

    fn install(&self, project_id: &str, desired: &[PathBuf]) -> Option<notify::RecommendedWatcher> {
        let sender = self.sender.clone();
        let watched: Vec<PathBuf> = desired.to_vec();
        let project_id = project_id.to_owned();
        let mut watcher =
            match notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else { return };
                if event.paths.iter().any(|path| is_relevant(path, &watched)) {
                    let _ = sender.send_blocking(project_id.clone());
                }
            }) {
                Ok(watcher) => watcher,
                Err(error) => {
                    log::warn!("could not create git watcher: {error}");
                    return None;
                }
            };
        for path in desired {
            if let Err(error) = watcher.watch(path, RecursiveMode::NonRecursive) {
                log::warn!("could not watch {}: {error}", path.display());
                return None;
            }
        }
        Some(watcher)
    }
}

fn desired_paths(project_path: &str) -> Vec<PathBuf> {
    let Some(git_dir) = git::git_dir(Path::new(project_path)) else {
        return Vec::new();
    };
    let worktrees = git_dir.join("worktrees");
    let mut paths = vec![git_dir];
    if worktrees.is_dir() {
        paths.push(worktrees);
    }
    paths
}

fn is_relevant(path: &Path, watched: &[PathBuf]) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if text.ends_with("/HEAD") && !text.contains("/logs/") {
        return true;
    }
    if text.contains("/worktrees/") || path.file_name().is_some_and(|name| name == "worktrees") {
        return true;
    }
    watched.iter().any(|candidate| candidate == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watched() -> Vec<PathBuf> {
        vec![PathBuf::from("/repo/.git")]
    }

    #[test]
    fn head_writes_are_relevant() {
        assert!(is_relevant(Path::new("/repo/.git/HEAD"), &watched()));
    }

    #[test]
    fn reflog_writes_are_not_relevant() {
        assert!(!is_relevant(Path::new("/repo/.git/logs/HEAD"), &watched()));
    }

    #[test]
    fn worktree_children_are_relevant() {
        assert!(is_relevant(
            Path::new("/repo/.git/worktrees/foo"),
            &watched()
        ));
    }

    #[test]
    fn the_worktrees_directory_itself_is_relevant() {
        assert!(is_relevant(Path::new("/repo/.git/worktrees"), &watched()));
    }

    #[test]
    fn a_watched_path_itself_is_relevant() {
        assert!(is_relevant(Path::new("/repo/.git"), &watched()));
    }

    #[test]
    fn object_writes_are_not_relevant() {
        assert!(!is_relevant(
            Path::new("/repo/.git/objects/ab/cd"),
            &watched()
        ));
    }
}
