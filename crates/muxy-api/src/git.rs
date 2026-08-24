use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

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
}

pub(crate) fn run_git(
    options: &GitOptions,
    path: &Path,
    args: &[&str],
) -> Result<String, GitError> {
    let output = Command::new(&options.executable)
        .arg("-C")
        .arg(path)
        .args(args)
        .envs(&options.environment)
        .output()
        .map_err(|source| GitError::Execute {
            executable: options.executable.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(GitError::Status {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| GitError::NonUtf8)
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
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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
}
