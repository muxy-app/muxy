use super::{Result, error, run, text};
use muxy_protocol::{GitBranch, GitFile, GitSummary, GitWorktree, ServerPath};
use std::collections::HashMap;
use std::path::Path;

pub(super) fn status(path: &Path) -> Result<Vec<u8>> {
    run(
        path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
}

pub(super) fn files(bytes: &[u8]) -> Result<Vec<GitFile>> {
    let mut records = bytes.split(|b| *b == 0).filter(|r| !r.is_empty());
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 || record[2] != b' ' {
            return Err(error("Invalid Git status"));
        }
        let original = if record[..2].iter().any(|b| matches!(b, b'R' | b'C')) {
            Some(ServerPath(
                records
                    .next()
                    .ok_or_else(|| error("Missing rename source"))?
                    .to_vec(),
            ))
        } else {
            None
        };
        files.push(GitFile {
            path: ServerPath(record[3..].to_vec()),
            original_path: original,
            index: record[0],
            worktree: record[1],
            added: None,
            removed: None,
        });
        if files.len() > 4096 {
            return Err(error(
                "More than 4096 changed files; narrow changes using Git",
            ));
        }
    }
    Ok(files)
}

pub(super) fn summary(path: &Path) -> Result<GitSummary> {
    let files = files(&status(path)?)?;
    let mut summary = GitSummary {
        branch: run(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .ok()
            .map(|b| text(&b))
            .transpose()?,
        head: run(path, &["rev-parse", "--verify", "HEAD"])
            .ok()
            .map(|b| text(&b))
            .transpose()?,
        upstream: run(
            path,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        )
        .ok()
        .map(|b| text(&b))
        .transpose()?,
        ..GitSummary::default()
    };
    if summary.upstream.is_some() {
        let counts = text(&run(
            path,
            &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        )?)?;
        let mut counts = counts.split_whitespace();
        summary.ahead = counts
            .next()
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| error("Invalid ahead count"))?;
        summary.behind = counts
            .next()
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| error("Invalid behind count"))?;
    }
    for file in files {
        summary.changed += 1;
        summary.staged += u32::from(file.staged());
        summary.unstaged += u32::from(file.unstaged());
        summary.untracked += u32::from(file.untracked());
        summary.conflicted += u32::from(file.conflicted());
    }
    Ok(summary)
}

pub(super) fn branches(path: &Path) -> Result<Vec<GitBranch>> {
    let default = run(
        path,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .ok()
    .and_then(|b| text(&b).ok())
    .and_then(|s| s.strip_prefix("origin/").map(str::to_owned));
    let bytes = run(
        path,
        &[
            "for-each-ref",
            "--format=%(refname:short)%00%(HEAD)",
            "refs/heads/",
        ],
    )?;
    let worktrees = worktrees(path)?;
    let mut result = Vec::new();
    for line in bytes.split(|b| *b == b'\n').filter(|r| !r.is_empty()) {
        let parts: Vec<_> = line.split(|b| *b == 0).collect();
        if parts.len() != 2 {
            return Err(error("Invalid Git branch record"));
        }
        let name = text(parts[0])?;
        result.push(GitBranch {
            current: parts[1] == b"*",
            checked_out: worktrees.iter().any(|w| w.branch.as_ref() == Some(&name)),
            default: default
                .as_ref()
                .map_or(matches!(name.as_str(), "main" | "master"), |d| *d == name),
            name,
        });
    }
    Ok(result)
}

pub(super) fn changes(path: &Path) -> Result<Vec<GitFile>> {
    let mut files = files(&status(path)?)?;
    let staged = line_stats(path, true)?;
    let unstaged = line_stats(path, false)?;
    for file in &mut files {
        if file.untracked() {
            if let Some(lines) = untracked_lines(&path.join(super::path(&file.path))) {
                file.added = Some(lines);
                file.removed = Some(0);
            }
            continue;
        }
        let first = staged
            .get(&file.path.0)
            .copied()
            .unwrap_or((Some(0), Some(0)));
        let second = unstaged
            .get(&file.path.0)
            .copied()
            .unwrap_or((Some(0), Some(0)));
        file.added = first.0.zip(second.0).and_then(|(a, b)| a.checked_add(b));
        file.removed = first.1.zip(second.1).and_then(|(a, b)| a.checked_add(b));
    }
    Ok(files)
}

type LineStats = HashMap<Vec<u8>, (Option<u64>, Option<u64>)>;

pub(super) fn file_status(path: &Path) -> Result<Vec<muxy_protocol::GitFileStatus>> {
    use muxy_protocol::{GitFileStatus, GitLineStat};
    let files = changes(path)?;
    let staged = line_stats(path, true)?;
    let unstaged = line_stats(path, false)?;
    let stat = |value: Option<&(Option<u64>, Option<u64>)>| {
        let (additions, deletions) = value.copied().unwrap_or((Some(0), Some(0)));
        GitLineStat {
            additions,
            deletions,
            binary: additions.is_none() && deletions.is_none(),
        }
    };
    Ok(files
        .into_iter()
        .map(|file| {
            let staged = stat(staged.get(&file.path.0));
            let unstaged = if file.untracked() {
                GitLineStat {
                    additions: file.added,
                    deletions: file.removed,
                    binary: untracked_binary(&path.join(super::path(&file.path))),
                }
            } else {
                stat(unstaged.get(&file.path.0))
            };
            GitFileStatus {
                file,
                staged,
                unstaged,
            }
        })
        .collect())
}

fn untracked_binary(path: &Path) -> bool {
    use std::io::Read;
    if !std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        return false;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .is_ok_and(|file| file.take(8000).read_to_end(&mut bytes).is_ok() && bytes.contains(&0))
}

fn line_stats(path: &Path, staged: bool) -> Result<LineStats> {
    let mut args = vec![
        "diff",
        "--numstat",
        "-z",
        "-M",
        "--no-ext-diff",
        "--no-textconv",
    ];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    let bytes = run(path, &args)?;
    let mut records = bytes.split(|b| *b == 0);
    let mut stats = HashMap::new();
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        let parts: Vec<_> = record.splitn(3, |b| *b == b'\t').collect();
        if parts.len() != 3 {
            return Err(error("Invalid Git line statistics"));
        }
        let target = if parts[2].is_empty() {
            records
                .next()
                .ok_or_else(|| error("Missing rename source"))?;
            records
                .next()
                .ok_or_else(|| error("Missing rename target"))?
        } else {
            parts[2]
        };
        let number = |bytes: &[u8]| -> Result<Option<u64>> {
            if bytes == b"-" {
                Ok(None)
            } else {
                text(bytes)?.parse().map(Some).map_err(error)
            }
        };
        stats.insert(target.to_vec(), (number(parts[0])?, number(parts[1])?));
    }
    Ok(stats)
}

pub(super) fn worktrees(path: &Path) -> Result<Vec<GitWorktree>> {
    let bytes = run(path, &["worktree", "list", "--porcelain", "-z"])?;
    let mut result: Vec<GitWorktree> = Vec::new();
    for record in bytes.split(|b| *b == 0) {
        if let Some(path) = record.strip_prefix(b"worktree ") {
            result.push(GitWorktree {
                directory: ServerPath(path.to_vec()),
                head: None,
                branch: None,
                primary: result.is_empty(),
                locked: false,
                bare: false,
                detached: false,
                prunable: false,
                registered: None,
            });
        } else if let Some(entry) = result.last_mut() {
            if let Some(head) = record.strip_prefix(b"HEAD ") {
                entry.head = Some(text(head)?);
            }
            if let Some(branch) = record.strip_prefix(b"branch refs/heads/") {
                entry.branch = Some(text(branch)?);
            }
            if record == b"locked" || record.starts_with(b"locked ") {
                entry.locked = true;
            }
            entry.bare |= record == b"bare";
            entry.detached |= record == b"detached";
            entry.prunable |= record == b"prunable" || record.starts_with(b"prunable ");
        }
    }
    Ok(result)
}

#[allow(
    clippy::naive_bytecount,
    reason = "Bounded one-megabyte file; avoid adding a dependency for line counting"
)]
fn untracked_lines(path: &Path) -> Option<u64> {
    use std::io::Read;
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 * 1024 || bytes.contains(&0) {
        return None;
    }
    u64::try_from(
        bytes.iter().filter(|b| **b == b'\n').count()
            + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n")),
    )
    .ok()
}
