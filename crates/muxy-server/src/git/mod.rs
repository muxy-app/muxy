mod command;
mod details;
mod diff;
mod github;
mod metadata;
mod mutate;
mod operations;
mod processes;
mod read;
#[cfg(test)]
mod tests;
pub(crate) mod watch;
mod worktree;

use crate::{Registry, ServerError};
use command::run;
use muxy_protocol::{GitAction, GitReply, GitRequest, ProjectKind, ServerPath};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, RwLock, Weak};

type Result<T> = std::result::Result<T, ServerError>;
fn error(message: impl std::fmt::Display) -> ServerError {
    ServerError::new(muxy_protocol::ErrorCode::BadRequest, message.to_string())
}
fn text(bytes: &[u8]) -> Result<String> {
    std::str::from_utf8(bytes)
        .map(|s| s.trim_end_matches('\n').to_owned())
        .map_err(error)
}
fn path(bytes: &ServerPath) -> &Path {
    Path::new(OsStr::from_bytes(&bytes.0))
}
fn server_path(path: &Path) -> ServerPath {
    ServerPath(path.as_os_str().as_bytes().to_vec())
}

#[derive(Debug, Default)]
pub(crate) struct Git {
    locks: Mutex<HashMap<PathBuf, Weak<RwLock<()>>>>,
    github: github::Github,
    pub(crate) operations: operations::Operations,
}
impl Git {
    fn lock_for(&self, path: &Path) -> Result<Arc<RwLock<()>>> {
        let common = run(
            path,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let common = Path::new(OsStr::from_bytes(
            common.strip_suffix(b"\n").unwrap_or(&common),
        ))
        .canonicalize()
        .map_err(error)?;
        Ok(self.lock_at(common))
    }

    fn lock_at(&self, common: PathBuf) -> Arc<RwLock<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        locks.retain(|_, lock| lock.strong_count() > 0);
        let lock = locks
            .get(&common)
            .and_then(Weak::upgrade)
            .unwrap_or_default();
        locks.insert(common, Arc::downgrade(&lock));
        lock
    }
}

pub(crate) fn is_read(action: &GitAction) -> bool {
    use muxy_protocol::GitPullRequestAction as Pr;
    matches!(
        action,
        GitAction::Summary
            | GitAction::Branches
            | GitAction::Changes
            | GitAction::Worktrees
            | GitAction::InspectRemoval
            | GitAction::Status { .. }
            | GitAction::RepoInfo
            | GitAction::RemoteBranches
            | GitAction::Log { .. }
            | GitAction::Diff(_)
            | GitAction::PullRequest(Pr::Info | Pr::Number | Pr::Diff { .. } | Pr::List { .. })
    )
}

impl Registry {
    pub fn git(&self, request: &GitRequest) -> Result<GitReply> {
        request
            .validate()
            .map_err(|code| ServerError::new(code, "Invalid Git request"))?;
        if let GitAction::Worktree(intent) = &request.action {
            return self.git_worktree(request.project, intent);
        }
        let project = self.catalog.project(request.project)?;
        let directory = path(&project.directory);
        if !directory.is_dir() {
            return Err(error("Project directory is missing"));
        }
        if request.action == GitAction::Init {
            if project.kind == Some(ProjectKind::Worktree) {
                return Err(error("Cannot initialize a registered worktree"));
            }
            let lock = if is_repository(directory)? {
                self.git.lock_for(directory)?
            } else {
                self.git
                    .lock_at(directory.canonicalize().map_err(error)?.join(".git"))
            };
            let _guard = lock.write().unwrap_or_else(PoisonError::into_inner);
            self.catalog.project(request.project)?;
            run(directory, &["init"])?;
            return Ok(GitReply::Done);
        }
        if !is_repository(directory)? {
            return if matches!(request.action, GitAction::Summary)
                && project.kind != Some(ProjectKind::Worktree)
            {
                Ok(GitReply::Summary(None))
            } else {
                Err(error("Git worktree is missing"))
            };
        }
        if project.kind == Some(ProjectKind::Worktree) {
            let parent = self.catalog.project(
                project
                    .parent_id
                    .ok_or_else(|| error("Missing worktree parent"))?,
            )?;
            validate_member(path(&parent.directory), directory)?;
        }
        let root_bytes = run(directory, &["rev-parse", "--show-toplevel"])?;
        let root = PathBuf::from(OsStr::from_bytes(
            root_bytes.strip_suffix(b"\n").unwrap_or(&root_bytes),
        ));
        let directory = root.as_path();
        let lock = self.git.lock_for(directory)?;
        let read = is_read(&request.action);
        let _read = read.then(|| lock.read().unwrap_or_else(PoisonError::into_inner));
        let _write = (!read).then(|| lock.write().unwrap_or_else(PoisonError::into_inner));
        // The project may have been deleted while waiting for another repository operation.
        self.catalog.project(request.project)?;
        self.dispatch_git(&project, directory, &request.action)
    }

    fn dispatch_git(
        &self,
        project: &muxy_protocol::ProjectDescriptor,
        directory: &Path,
        action: &GitAction,
    ) -> Result<GitReply> {
        match action {
            GitAction::Summary => Ok(GitReply::Summary(Some(read::summary(directory)?))),
            GitAction::Branches => Ok(GitReply::Branches(read::branches(directory)?)),
            GitAction::Changes => Ok(GitReply::Changes(read::changes(directory)?)),
            GitAction::Status { local } => {
                let pull_request = if *local {
                    None
                } else {
                    match self
                        .git
                        .github
                        .apply(directory, &muxy_protocol::GitPullRequestAction::Info)
                    {
                        Ok(GitReply::PullRequest(pr)) => pr.map(|pr| *pr),
                        _ => None,
                    }
                };
                Ok(GitReply::Status(Box::new(muxy_protocol::GitStatus {
                    summary: read::summary(directory)?,
                    default_branch: self.git.github.default_branch(directory),
                    branches: read::branches(directory)?,
                    files: read::file_status(directory)?,
                    pull_request,
                })))
            }
            GitAction::RepoInfo => Ok(GitReply::RepoInfo(details::repo_info(directory)?)),
            GitAction::RemoteBranches => Ok(GitReply::RemoteBranches(details::remote_branches(
                directory,
            )?)),
            GitAction::Log { max_count, skip } => {
                Ok(GitReply::Log(details::log(directory, *max_count, *skip)?))
            }
            GitAction::Diff(request) => diff::read(directory, request),
            GitAction::PullRequest(action) => self.git.github.apply(directory, action),
            GitAction::Commit { .. }
            | GitAction::Push { .. }
            | GitAction::Pull
            | GitAction::Checkout(_)
            | GitAction::CherryPick(_)
            | GitAction::Revert(_)
            | GitAction::DeleteLocalBranch { .. }
            | GitAction::DeleteRemoteBranch(_)
            | GitAction::CreateTag { .. } => mutate::apply(directory, action),
            GitAction::Worktrees => {
                let mut worktrees = read::worktrees(directory)?;
                for worktree in &mut worktrees {
                    worktree.registered =
                        self.catalog.child_at(project.id, path(&worktree.directory));
                }
                Ok(GitReply::Worktrees(worktrees))
            }
            GitAction::InspectRemoval => Ok(GitReply::Removal(self.inspect_removal(project)?)),
            GitAction::SwitchBranch(branch)
            | GitAction::CreateBranch(branch)
            | GitAction::DeleteBranch(branch) => {
                validate_branch(directory, branch)?;
                match action {
                    GitAction::SwitchBranch(_) => {
                        run(directory, &["switch", "--", branch])?;
                    }
                    GitAction::CreateBranch(_) => {
                        run(directory, &["switch", "-c", branch])?;
                    }
                    _ => {
                        run(directory, &["branch", "-D", "--", branch])?;
                    }
                }
                Ok(GitReply::Done)
            }
            GitAction::Stage(paths) | GitAction::Unstage(paths) | GitAction::Discard(paths) => {
                mutate_files(directory, action, paths)?;
                Ok(GitReply::Done)
            }
            GitAction::Worktree(_) | GitAction::Init => unreachable!(),
            GitAction::Watch => Err(error("Git watches require a client connection")),
        }
    }
}

fn is_repository(directory: &Path) -> Result<bool> {
    match run(directory, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(value) => Ok(value == b"true\n"),
        Err(e) if e.message().contains("not a git repository") => Ok(false),
        Err(e) => Err(e),
    }
}
fn validate_branch(directory: &Path, branch: &str) -> Result<()> {
    if branch.starts_with('-') || branch.contains('@') && branch.contains('{') {
        return Err(error("Invalid branch name"));
    }
    run(directory, &["check-ref-format", "--branch", branch]).map(|_| ())
}
fn validate_member(repository: &Path, directory: &Path) -> Result<muxy_protocol::GitWorktree> {
    let target = directory.canonicalize().map_err(error)?;
    read::worktrees(repository)?
        .into_iter()
        .find(|w| path(&w.directory).canonicalize().is_ok_and(|p| p == target))
        .ok_or_else(|| error("Directory is not a registered Git worktree"))
}

fn safe_path(repository: &Path, relative: &ServerPath) -> Result<PathBuf> {
    let relative = path(relative);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || relative.as_os_str().is_empty()
        || relative.components().any(|c| c.as_os_str() == ".git")
    {
        return Err(error("Invalid repository-relative path"));
    }
    let mut current = repository.to_path_buf();
    let components: Vec<_> = relative.components().collect();
    for component in &components[..components.len() - 1] {
        current.push(component);
        if std::fs::symlink_metadata(&current).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(error("Path traverses a symbolic link"));
        }
    }
    Ok(repository.join(relative))
}
fn mutate_files(repository: &Path, action: &GitAction, paths: &[ServerPath]) -> Result<()> {
    if paths.is_empty() {
        match action {
            GitAction::Stage(_) => {
                run(repository, &["add", "-A"])?;
            }
            GitAction::Unstage(_) => {
                if read::summary(repository)?.head.is_some() {
                    run(repository, &["reset", "HEAD", "--", "."])?;
                } else {
                    run(
                        repository,
                        &["rm", "--cached", "-r", "-f", "--ignore-unmatch", "--", "."],
                    )?;
                }
            }
            _ => (),
        }
        return Ok(());
    }
    let current = read::files(&read::status(repository)?)?;
    let mut tracked = Vec::new();
    let mut untracked = Vec::new();
    for relative in paths {
        let absolute = safe_path(repository, relative)?;
        let file = current
            .iter()
            .find(|f| f.path == *relative)
            .ok_or_else(|| error("File status changed; refresh and try again"))?;
        if matches!(action, GitAction::Discard(_)) {
            if file.conflicted() || file.staged() && !file.unstaged() {
                return Err(error("Unstage or resolve the file before discarding"));
            }
            if file.untracked() {
                let meta = std::fs::symlink_metadata(&absolute).map_err(error)?;
                if meta.is_dir() {
                    return Err(error("Refusing to recursively discard a directory"));
                }
                untracked.push(absolute);
                continue;
            }
        }
        tracked.push(OsString::from(OsStr::from_bytes(&relative.0)));
        if let Some(original) = &file.original_path
            && matches!(action, GitAction::Unstage(_))
        {
            safe_path(repository, original)?;
            tracked.push(OsString::from(OsStr::from_bytes(&original.0)));
        }
    }
    if !tracked.is_empty() {
        let args = match action {
            GitAction::Stage(_) => vec!["add", "--"],
            GitAction::Unstage(_)
                if run(repository, &["rev-parse", "--verify", "HEAD"]).is_err() =>
            {
                vec!["rm", "--cached", "-f", "--"]
            }
            GitAction::Unstage(_) => vec!["reset", "HEAD", "--"],
            GitAction::Discard(_) => vec!["restore", "--worktree", "--"],
            _ => unreachable!(),
        };
        let args: Vec<OsString> = args
            .into_iter()
            .map(OsString::from)
            .chain(tracked)
            .collect();
        run(repository, &args)?;
    }
    for absolute in untracked {
        std::fs::remove_file(absolute).map_err(error)?;
    }
    Ok(())
}

pub(crate) fn ignored_names(
    directory: &Path,
    entries: &[muxy_protocol::FileEntry],
) -> std::collections::HashSet<Vec<u8>> {
    if entries.is_empty() {
        return std::collections::HashSet::new();
    }
    let mut input = Vec::new();
    for entry in entries {
        input.extend_from_slice(&entry.name.0);
        input.push(0);
    }
    command::ignored(directory, input)
        .unwrap_or_default()
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(<[u8]>::to_vec)
        .collect()
}
