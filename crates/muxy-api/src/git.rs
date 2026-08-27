use crate::subprocess::{Deadline, SubprocessError, SubprocessRequest};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const DEFAULT_GIT_TIMEOUT: Duration = Duration::from_secs(30);
const RECONCILIATION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct GitOptions {
    pub executable: PathBuf,
    pub environment: HashMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("could not execute {executable}: {source}")]
    Execute {
        executable: PathBuf,
        source: std::io::Error,
    },
    #[error("git exited with {status}: {stderr}")]
    Status {
        status: std::process::ExitStatus,
        stderr: String,
    },
    #[error("git produced non-utf-8 output")]
    NonUtf8,
    #[error("invalid branch name")]
    InvalidBranch,
    #[error(transparent)]
    Process(#[from] SubprocessError),
}

pub(crate) fn run_git(
    options: &GitOptions,
    path: &Path,
    args: &[&str],
) -> Result<String, GitError> {
    let deadline = Deadline::new(DEFAULT_GIT_TIMEOUT);
    run_git_with_deadline(options, path, args, &deadline)
}

fn run_git_with_deadline(
    options: &GitOptions,
    path: &Path,
    args: &[&str],
    deadline: &Deadline,
) -> Result<String, GitError> {
    let output = run_git_output(options, path, args, deadline)?;
    if !output.status.success() {
        return Err(GitError::Status {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| GitError::NonUtf8)
}

fn run_git_output(
    options: &GitOptions,
    path: &Path,
    args: &[&str],
    deadline: &Deadline,
) -> Result<crate::subprocess::SubprocessOutput, GitError> {
    let mut command_args = vec![OsString::from("-C"), path.as_os_str().to_owned()];
    command_args.extend(args.iter().map(OsString::from));
    crate::subprocess::run(
        SubprocessRequest {
            executable: options.executable.clone(),
            args: command_args,
            current_dir: None,
            environment: options
                .environment
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                .collect(),
        },
        Some(deadline),
    )
    .map_err(|error| match error {
        SubprocessError::Spawn(source) => GitError::Execute {
            executable: options.executable.clone(),
            source,
        },
        other => GitError::Process(other),
    })
}

pub(crate) fn is_git_repo(options: &GitOptions, path: &Path) -> bool {
    run_git(options, path, &["rev-parse", "--is-inside-work-tree"])
        .is_ok_and(|output| output.trim() == "true")
}

pub(crate) fn git_dir(path: &Path) -> Option<PathBuf> {
    let dot_git = path.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let contents = std::fs::read_to_string(&dot_git).ok()?;
    let target = contents
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("gitdir:"))?
        .trim();
    Some(truncate_at_worktrees(&normalize(&path.join(target))))
}

pub(crate) fn canonical_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        let normalized = normalize(path);
        let Some(parent) = normalized.parent() else {
            return normalized;
        };
        let Some(name) = normalized.file_name() else {
            return normalized;
        };
        std::fs::canonicalize(parent)
            .map(|parent| parent.join(name))
            .unwrap_or(normalized)
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitWorktreeRecord {
    pub path: PathBuf,
    pub branch: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoveWorktreeOutcome {
    pub reconciled: bool,
}

pub fn validate_branch(branch: &str) -> Result<(), GitError> {
    if branch.is_empty()
        || branch.starts_with('-')
        || !branch
            .chars()
            .all(|character| character.is_alphanumeric() || "._/-".contains(character))
    {
        return Err(GitError::InvalidBranch);
    }
    Ok(())
}

pub fn add_worktree(
    options: &GitOptions,
    repo_path: &Path,
    worktree_path: &Path,
    branch: &str,
    create_branch: bool,
    base_branch: Option<&str>,
    deadline: &Deadline,
) -> Result<(), GitError> {
    validate_branch(branch)?;
    let worktree_path = worktree_path.to_string_lossy();
    let mut args = vec!["worktree", "add"];
    if create_branch {
        if let Some(base_branch) = base_branch {
            validate_branch(base_branch)?;
        }
        args.extend(["-b", branch, worktree_path.as_ref()]);
        if let Some(base_branch) = base_branch {
            args.push(base_branch);
        }
    } else {
        args.extend(["--", worktree_path.as_ref(), branch]);
    }
    run_git_with_deadline(options, repo_path, &args, deadline).map(|_| ())
}

pub fn is_worktree_dirty(
    options: &GitOptions,
    worktree_path: &Path,
    deadline: &Deadline,
) -> Result<bool, GitError> {
    run_git_with_deadline(
        options,
        worktree_path,
        &["status", "--porcelain=1", "--untracked-files=all"],
        deadline,
    )
    .map(|output| !output.trim().is_empty())
}

pub fn list_worktrees(
    options: &GitOptions,
    repo_path: &Path,
    deadline: &Deadline,
) -> Result<Vec<GitWorktreeRecord>, GitError> {
    let output = run_git_with_deadline(
        options,
        repo_path,
        &["worktree", "list", "--porcelain"],
        deadline,
    )?;
    let mut records = Vec::new();
    let mut path = None;
    let mut branch = None;
    for line in output.lines().chain(std::iter::once("")) {
        if let Some(value) = line.strip_prefix("worktree ") {
            if let Some(path) = path.take() {
                records.push(GitWorktreeRecord { path, branch });
                branch = None;
            }
            path = Some(PathBuf::from(value));
        } else if let Some(value) = line.strip_prefix("branch ") {
            branch = Some(
                value
                    .strip_prefix("refs/heads/")
                    .unwrap_or(value)
                    .to_owned(),
            );
        } else if line.is_empty()
            && let Some(path) = path.take()
        {
            records.push(GitWorktreeRecord { path, branch });
            branch = None;
        }
    }
    Ok(records)
}

pub fn list_local_branches(
    options: &GitOptions,
    repo_path: &Path,
    deadline: &Deadline,
) -> Result<Vec<String>, GitError> {
    let output = run_git_with_deadline(
        options,
        repo_path,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
        deadline,
    )?;
    let mut branches = output
        .lines()
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    branches.sort();
    branches.dedup();
    Ok(branches)
}

pub fn current_branch(
    options: &GitOptions,
    repo_path: &Path,
    deadline: &Deadline,
) -> Result<Option<String>, GitError> {
    let output =
        run_git_with_deadline(options, repo_path, &["branch", "--show-current"], deadline)?;
    Ok((!output.trim().is_empty()).then(|| output.trim().to_owned()))
}

pub fn is_worktree_registered(
    options: &GitOptions,
    repo_path: &Path,
    worktree_path: &Path,
    deadline: &Deadline,
) -> Result<bool, GitError> {
    let target = canonical_path(worktree_path);
    list_worktrees(options, repo_path, deadline).map(|records| {
        records
            .iter()
            .any(|record| canonical_path(&record.path) == target)
    })
}

pub fn prune_worktrees(
    options: &GitOptions,
    repo_path: &Path,
    deadline: &Deadline,
) -> Result<(), GitError> {
    run_git_with_deadline(options, repo_path, &["worktree", "prune"], deadline).map(|_| ())
}

pub fn remove_worktree(
    options: &GitOptions,
    repo_path: &Path,
    worktree_path: &Path,
    deadline: &Deadline,
) -> Result<RemoveWorktreeOutcome, GitError> {
    let path = worktree_path.to_string_lossy();
    let removal = run_git_with_deadline(
        options,
        repo_path,
        &["worktree", "remove", "--force", "--", path.as_ref()],
        deadline,
    );
    match removal {
        Ok(_) => {
            let verification = retained_deadline(deadline);
            if is_worktree_registered(options, repo_path, worktree_path, &verification)? {
                return Err(GitError::Process(SubprocessError::Wait(io::Error::other(
                    "worktree remains registered after removal",
                ))));
            }
            Ok(RemoveWorktreeOutcome { reconciled: false })
        }
        Err(error) => {
            if worktree_path.exists() {
                return Err(error);
            }
            let reconciliation = retained_deadline(deadline);
            if reconcile_removed(options, repo_path, worktree_path, &reconciliation) {
                Ok(RemoveWorktreeOutcome { reconciled: true })
            } else {
                Err(error)
            }
        }
    }
}

fn retained_deadline(deadline: &Deadline) -> Deadline {
    if deadline.is_expired() {
        Deadline::new(RECONCILIATION_TIMEOUT)
    } else {
        deadline.clone()
    }
}

fn reconcile_removed(
    options: &GitOptions,
    repo_path: &Path,
    worktree_path: &Path,
    deadline: &Deadline,
) -> bool {
    let _ = prune_worktrees(options, repo_path, deadline);
    is_worktree_registered(options, repo_path, worktree_path, deadline)
        .is_ok_and(|registered| !registered)
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn truncate_at_worktrees(path: &Path) -> PathBuf {
    let components: Vec<Component> = path.components().collect();
    let cut = components
        .iter()
        .position(|component| component.as_os_str() == "worktrees");
    match cut {
        Some(index) if index + 1 < components.len() => components[..index]
            .iter()
            .map(|component| component.as_os_str())
            .collect(),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn options() -> GitOptions {
        GitOptions {
            executable: PathBuf::from("git"),
            environment: HashMap::new(),
        }
    }

    #[test]
    fn git_dir_follows_a_gitdir_file_and_truncates_at_worktrees() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();
        std::fs::write(root.join(".git"), "gitdir: ./real/.git/worktrees/wt\n").expect("write");

        let resolved = git_dir(root).expect("resolved");

        assert_eq!(resolved, root.join("real/.git"));
    }

    #[test]
    fn git_dir_returns_a_plain_dot_git_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();
        std::fs::create_dir(root.join(".git")).expect("create");

        let resolved = git_dir(root).expect("resolved");

        assert_eq!(resolved, root.join(".git"));
    }

    #[test]
    fn git_dir_is_none_without_a_dot_git() {
        let temp = tempfile::tempdir().expect("temp dir");

        assert!(git_dir(temp.path()).is_none());
    }

    #[test]
    fn is_git_repo_is_false_for_a_plain_directory() {
        let temp = tempfile::tempdir().expect("temp dir");

        assert!(!is_git_repo(&options(), temp.path()));
    }

    #[test]
    fn run_git_reports_a_missing_executable() {
        let temp = tempfile::tempdir().expect("temp dir");
        let missing = GitOptions {
            executable: PathBuf::from("/nonexistent/git-binary"),
            environment: HashMap::new(),
        };

        let result = run_git(&missing, temp.path(), &["status"]);

        assert!(matches!(result, Err(GitError::Execute { .. })));
    }

    #[test]
    fn run_git_reports_a_non_zero_status_with_stderr() {
        let temp = tempfile::tempdir().expect("temp dir");

        let result = run_git(
            &options(),
            temp.path(),
            &["rev-parse", "--is-inside-work-tree"],
        );

        match result {
            Err(GitError::Status { stderr, .. }) => assert!(!stderr.is_empty()),
            other => panic!("expected status error, got {other:?}"),
        }
    }

    fn initialize_repository() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.name", "Muxy Test"],
            vec!["config", "user.email", "test@muxy.invalid"],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&repo)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(repo.join("seed"), b"seed").unwrap();
        assert!(
            Command::new("git")
                .args(["add", "seed"])
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new("git")
                .args(["commit", "-q", "-m", "seed"])
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
        (temp, repo)
    }

    #[test]
    fn git_worktree_validates_branches_and_adds_new_and_existing_branches() {
        let (temp, repo) = initialize_repository();
        let new_path = temp.path().join("new-worktree");
        let existing_path = temp.path().join("existing-worktree");
        let deadline = crate::subprocess::Deadline::new(std::time::Duration::from_secs(5));

        assert!(validate_branch("feature/ä.1_test").is_ok());
        for invalid in ["", "-branch", "has space", "bad~branch"] {
            assert!(validate_branch(invalid).is_err());
        }
        add_worktree(
            &options(),
            &repo,
            &new_path,
            "new-branch",
            true,
            Some("HEAD"),
            &deadline,
        )
        .unwrap();
        assert!(is_worktree_registered(&options(), &repo, &new_path, &deadline).unwrap());
        remove_worktree(&options(), &repo, &new_path, &deadline).unwrap();

        assert!(
            Command::new("git")
                .args(["branch", "existing-branch"])
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
        add_worktree(
            &options(),
            &repo,
            &existing_path,
            "existing-branch",
            false,
            Some("ignored~invalid~base"),
            &deadline,
        )
        .unwrap();
        assert!(is_worktree_registered(&options(), &repo, &existing_path, &deadline).unwrap());
    }

    #[test]
    fn git_worktree_inspects_dirty_state_and_canonical_registration() {
        let (temp, repo) = initialize_repository();
        let path = temp.path().join("worktree");
        let deadline = crate::subprocess::Deadline::new(std::time::Duration::from_secs(5));
        add_worktree(
            &options(),
            &repo,
            &path,
            "dirty-branch",
            true,
            None,
            &deadline,
        )
        .unwrap();

        assert!(!is_worktree_dirty(&options(), &path, &deadline).unwrap());
        std::fs::write(path.join("untracked"), b"dirty").unwrap();
        assert!(is_worktree_dirty(&options(), &path, &deadline).unwrap());
        let lexical_alias = path.join("..").join(path.file_name().unwrap());
        assert!(is_worktree_registered(&options(), &repo, &lexical_alias, &deadline).unwrap());
    }

    #[test]
    fn git_worktree_force_removes_dirty_targets_and_prunes_stale_registration() {
        let (temp, repo) = initialize_repository();
        let dirty = temp.path().join("dirty");
        let stale = temp.path().join("stale");
        let deadline = crate::subprocess::Deadline::new(std::time::Duration::from_secs(5));
        add_worktree(
            &options(),
            &repo,
            &dirty,
            "dirty-remove",
            true,
            None,
            &deadline,
        )
        .unwrap();
        std::fs::write(dirty.join("untracked"), b"dirty").unwrap();

        let removed = remove_worktree(&options(), &repo, &dirty, &deadline).unwrap();
        assert!(!removed.reconciled);
        assert!(!dirty.exists());
        assert!(!is_worktree_registered(&options(), &repo, &dirty, &deadline).unwrap());

        add_worktree(
            &options(),
            &repo,
            &stale,
            "stale-remove",
            true,
            None,
            &deadline,
        )
        .unwrap();
        std::fs::remove_dir_all(&stale).unwrap();
        prune_worktrees(&options(), &repo, &deadline).unwrap();
        assert!(!is_worktree_registered(&options(), &repo, &stale, &deadline).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn git_worktree_removal_fails_when_retained_and_reconciles_nonzero_or_timeout_when_gone() {
        use std::os::unix::fs::PermissionsExt;

        let (temp, repo) = initialize_repository();
        let retained = temp.path().join("retained");
        let nonzero = temp.path().join("nonzero");
        let timed_out = temp.path().join("timed-out");
        let deadline = crate::subprocess::Deadline::new(std::time::Duration::from_secs(5));
        for (path, branch) in [
            (&retained, "retained-branch"),
            (&nonzero, "nonzero-branch"),
            (&timed_out, "timeout-branch"),
        ] {
            add_worktree(&options(), &repo, path, branch, true, None, &deadline).unwrap();
        }
        let script = temp.path().join("git-wrapper");
        std::fs::write(
            &script,
            b"#!/bin/sh\nmode=$MUXY_GIT_TEST_MODE\nif [ \"$3 $4\" = \"worktree remove\" ]; then\n  if [ \"$mode\" = retained ]; then exit 9; fi\n  /usr/bin/git \"$@\"\n  if [ \"$mode\" = timeout ]; then sleep 30; fi\n  exit 9\nfi\nexec /usr/bin/git \"$@\"\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let with_mode = |mode: &str| GitOptions {
            executable: script.clone(),
            environment: HashMap::from([("MUXY_GIT_TEST_MODE".into(), mode.into())]),
        };
        assert!(remove_worktree(&with_mode("retained"), &repo, &retained, &deadline).is_err());
        assert!(retained.exists());

        let outcome = remove_worktree(&with_mode("nonzero"), &repo, &nonzero, &deadline).unwrap();
        assert!(outcome.reconciled);
        assert!(!nonzero.exists());

        let timeout = crate::subprocess::Deadline::new(std::time::Duration::from_millis(200));
        let outcome = remove_worktree(&with_mode("timeout"), &repo, &timed_out, &timeout).unwrap();
        assert!(outcome.reconciled);
        assert!(!timed_out.exists());
    }

    #[cfg(unix)]
    #[test]
    fn git_worktree_reconciliation_uses_one_retained_deadline_window() {
        use std::os::unix::fs::PermissionsExt;

        let (temp, repo) = initialize_repository();
        let stale = temp.path().join("stale-window");
        let setup_deadline = crate::subprocess::Deadline::new(std::time::Duration::from_secs(5));
        add_worktree(
            &options(),
            &repo,
            &stale,
            "stale-window-branch",
            true,
            None,
            &setup_deadline,
        )
        .unwrap();
        std::fs::remove_dir_all(&stale).unwrap();
        let script = temp.path().join("slow-prune-git");
        std::fs::write(
            &script,
            b"#!/bin/sh\nif [ \"$3 $4\" = \"worktree prune\" ]; then /usr/bin/git \"$@\"; sleep 1; exit 0; fi\nexec /usr/bin/git \"$@\"\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();
        let options = GitOptions {
            executable: script,
            environment: HashMap::new(),
        };
        let reconciliation =
            crate::subprocess::Deadline::new(std::time::Duration::from_millis(100));

        assert!(!reconcile_removed(&options, &repo, &stale, &reconciliation));
    }
}
