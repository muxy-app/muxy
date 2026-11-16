use super::{Result, branches, command, diff, error, read, run, text};
use muxy_protocol::{GitChangesPreview, GitPreviewFile, ServerPath};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const FILE_LIMIT: usize = 4096;

pub(super) fn preview(
    repository: &Path,
    line_limit: Option<u32>,
    staged: bool,
) -> Result<GitChangesPreview> {
    let summary = read::summary(repository)?;
    if summary.conflicted > 0 {
        return Err(error("Resolve merge conflicts before committing"));
    }
    let untracked: HashSet<Vec<u8>> = read::files(&read::status(repository)?)?
        .into_iter()
        .filter(muxy_protocol::GitFile::untracked)
        .map(|file| file.path.0)
        .collect();
    let tree = if staged {
        staged_snapshot(repository)?
    } else {
        snapshot(repository)?
    };
    let base = match &summary.head {
        Some(head) => head.clone(),
        None => text(&run(
            repository,
            &["hash-object", "-t", "tree", "/dev/null"],
        )?)?,
    };
    let (bytes, truncated) = command::diff(
        repository,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            &base,
            &tree,
            "--",
        ],
        false,
    )?;
    let files = numstat(repository, &base, &tree, &untracked)?;
    let destination = match &summary.branch {
        Some(branch) => branches::destination(repository, branch)?,
        None => None,
    };
    Ok(GitChangesPreview {
        branch: summary.branch,
        head: summary.head,
        tree,
        diff: diff::bounded(&bytes, truncated, line_limit),
        files,
        destination,
    })
}

pub(super) fn commit_all(
    repository: &Path,
    message: &str,
    expected_head: Option<&str>,
    expected_tree: &str,
) -> Result<String> {
    ensure_committable(repository, expected_head)?;
    if snapshot(repository)? != expected_tree {
        return Err(error(
            "Files changed after the draft was made; review the changes again",
        ));
    }
    let staging = TemporaryIndex::copy(repository)?;
    run(repository, &["add", "-A"])?;
    if let Err(cause) = commit_index(repository, message, expected_tree) {
        return Err(match staging.restore(repository) {
            Ok(()) => cause,
            Err(_) => error(format!("{}. All changes were left staged", cause.message())),
        });
    }
    text(&run(repository, &["rev-parse", "HEAD"])?)
}

pub(super) fn commit_staged(
    repository: &Path,
    message: &str,
    expected_head: Option<&str>,
    expected_tree: &str,
) -> Result<String> {
    ensure_committable(repository, expected_head)?;
    if staged_snapshot(repository)? != expected_tree {
        return Err(error(
            "Staged changes changed after the draft was made; review the changes again",
        ));
    }
    commit_index(repository, message, expected_tree)?;
    text(&run(repository, &["rev-parse", "HEAD"])?)
}

fn ensure_committable(repository: &Path, expected_head: Option<&str>) -> Result<()> {
    let summary = read::summary(repository)?;
    if summary.branch.is_none() {
        return Err(error("Switch to a branch before committing"));
    }
    if summary.head.as_deref() != expected_head {
        return Err(error(
            "The branch moved after the draft was made; review the changes again",
        ));
    }
    if summary.conflicted > 0 {
        return Err(error("Resolve merge conflicts before committing"));
    }
    Ok(())
}

fn commit_index(repository: &Path, message: &str, expected_tree: &str) -> Result<()> {
    if text(&run(repository, &["write-tree"])?)? != expected_tree {
        return Err(error(
            "Files changed while committing; review the changes again",
        ));
    }
    run(repository, &["commit", "-m", message.trim()]).map(|_| ())
}

fn snapshot(repository: &Path) -> Result<String> {
    let index = TemporaryIndex::copy(repository)?;
    command::run_with_index(repository, &index.path, &["add", "-A"])?;
    text(&command::run_with_index(
        repository,
        &index.path,
        &["write-tree"],
    )?)
}

fn staged_snapshot(repository: &Path) -> Result<String> {
    let index = TemporaryIndex::copy(repository)?;
    text(&command::run_with_index(
        repository,
        &index.path,
        &["write-tree"],
    )?)
}

fn numstat(
    repository: &Path,
    base: &str,
    tree: &str,
    untracked: &HashSet<Vec<u8>>,
) -> Result<Vec<GitPreviewFile>> {
    let bytes = run(
        repository,
        &["diff", "--numstat", "-z", "--no-renames", base, tree, "--"],
    )?;
    let count = |value: &[u8]| std::str::from_utf8(value).ok()?.parse().ok();
    let mut files = Vec::new();
    for record in bytes.split(|byte| *byte == 0).filter(|r| !r.is_empty()) {
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let (Some(added), Some(removed), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(error("Invalid Git change summary"));
        };
        files.push(GitPreviewFile {
            path: ServerPath(path.to_vec()),
            added: count(added),
            removed: count(removed),
            untracked: untracked.contains(path),
        });
        if files.len() > FILE_LIMIT {
            return Err(error(
                "More than 4096 changed files; narrow changes using Git",
            ));
        }
    }
    Ok(files)
}

struct TemporaryIndex {
    path: PathBuf,
}

impl TemporaryIndex {
    fn copy(repository: &Path) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let index = git_path(repository, "index")?;
        let path = git_path(
            repository,
            &format!(
                "muxy-preview-{}-{}.index",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ),
        )?;
        let temporary = Self { path };
        if index.exists() {
            std::fs::copy(&index, &temporary.path).map_err(error)?;
        }
        Ok(temporary)
    }

    fn restore(&self, repository: &Path) -> Result<()> {
        let index = git_path(repository, "index")?;
        let lock = lock_path(&index);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(error)?;
        let restored = if self.path.exists() {
            std::fs::File::open(&self.path)
                .and_then(|mut copy| std::io::copy(&mut copy, &mut file))
                .and_then(|_| std::fs::rename(&lock, &index))
        } else {
            std::fs::remove_file(&index).and_then(|()| std::fs::remove_file(&lock))
        };
        if restored.is_err() {
            let _ = std::fs::remove_file(&lock);
        }
        restored.map_err(error)
    }
}

impl Drop for TemporaryIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(lock_path(&self.path));
    }
}

fn lock_path(path: &Path) -> PathBuf {
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    PathBuf::from(lock)
}

fn git_path(repository: &Path, name: &str) -> Result<PathBuf> {
    let path = super::git_path(&run(repository, &["rev-parse", "--git-path", name])?)?;
    Ok(if path.is_absolute() {
        path
    } else {
        repository.join(path)
    })
}
