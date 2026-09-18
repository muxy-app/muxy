use super::{Result, command, error, read, run, server_path, text};
use muxy_protocol::{GitCommit, GitRef, GitRefKind, GitRepoInfo};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub(super) fn repo_info(repository: &Path) -> Result<GitRepoInfo> {
    let resolve = |flag| -> Result<std::path::PathBuf> {
        let bytes = run(repository, &["rev-parse", "--path-format=absolute", flag])?;
        Ok(Path::new(OsStr::from_bytes(
            bytes.strip_suffix(b"\n").unwrap_or(&bytes),
        ))
        .to_owned())
    };
    let git_dir = resolve("--git-dir")?;
    let common = resolve("--git-common-dir")?;
    Ok(GitRepoInfo {
        root: server_path(repository),
        is_worktree: git_dir != common,
        git_dir: server_path(&git_dir),
        current_branch: read::summary(repository)?.branch,
    })
}

pub(super) fn remote_branches(repository: &Path) -> Result<Vec<String>> {
    let bytes = command::network(repository, &["ls-remote", "--heads", "origin"])?;
    let mut branches = Vec::new();
    for line in text(&bytes)?.lines() {
        let (_, reference) = line
            .split_once('\t')
            .ok_or_else(|| error("Invalid remote branch"))?;
        let branch = reference
            .strip_prefix("refs/heads/")
            .ok_or_else(|| error("Invalid remote branch"))?;
        branches.push(branch.to_owned());
    }
    branches.sort();
    Ok(branches)
}

pub(super) fn default_branch(repository: &Path) -> Option<String> {
    if let Ok(bytes) = command::network(repository, &["ls-remote", "--symref", "origin", "HEAD"])
        && let Ok(output) = text(&bytes)
        && let Some(branch) = output.lines().find_map(|line| {
            line.strip_prefix("ref: refs/heads/")?
                .strip_suffix("\tHEAD")
        })
    {
        return Some(branch.to_owned());
    }
    let bytes = run(
        repository,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )
    .ok()?;
    text(&bytes)
        .ok()?
        .strip_prefix("origin/")
        .map(str::to_owned)
}

pub(super) fn log(repository: &Path, max_count: u32, skip: u32) -> Result<Vec<GitCommit>> {
    if max_count == 0 {
        return Ok(Vec::new());
    }
    let summary = read::summary(repository)?;
    if summary.head.is_none() && summary.branch.is_some() {
        return Ok(Vec::new());
    }
    let bytes = run(
        repository,
        &[
            "log",
            "--no-show-signature",
            "--no-notes",
            "-z",
            "--format=%H%x00%h%x00%s%x00%an%x00%aI%x00%P",
            &format!("--max-count={max_count}"),
            &format!("--skip={skip}"),
            "--",
        ],
    )?;
    let fields: Vec<_> = bytes
        .strip_suffix(&[0])
        .unwrap_or(&bytes)
        .split(|byte| *byte == 0)
        .collect();
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if fields.len() % 6 != 0 {
        return Err(error("Invalid Git log"));
    }
    let references = run(
        repository,
        &[
            "for-each-ref",
            "--format=%(objectname)%00%(*objectname)%00%(refname)",
            "refs/heads/",
            "refs/remotes/",
            "refs/tags/",
        ],
    )?;
    let mut refs: HashMap<String, Vec<GitRef>> = HashMap::new();
    for record in references
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let parts: Vec<_> = record.split(|byte| *byte == 0).collect();
        if parts.len() != 3 {
            return Err(error("Invalid Git ref"));
        }
        let reference = text(parts[2])?;
        let (name, kind) = if let Some(name) = reference.strip_prefix("refs/heads/") {
            (name, GitRefKind::LocalBranch)
        } else if let Some(name) = reference.strip_prefix("refs/remotes/") {
            (name, GitRefKind::RemoteBranch)
        } else if let Some(name) = reference.strip_prefix("refs/tags/") {
            (name, GitRefKind::Tag)
        } else {
            continue;
        };
        let hash = text(if parts[1].is_empty() {
            parts[0]
        } else {
            parts[1]
        })?;
        refs.entry(hash).or_default().push(GitRef {
            name: name.to_owned(),
            kind,
        });
    }
    if let Some(head) = summary.head {
        refs.entry(head).or_default().push(GitRef {
            name: "HEAD".into(),
            kind: GitRefKind::Head,
        });
    }
    fields
        .chunks_exact(6)
        .map(|parts| {
            let hash = text(parts[0])?;
            Ok(GitCommit {
                refs: refs.get(&hash).cloned().unwrap_or_default(),
                hash,
                short_hash: text(parts[1])?,
                subject: text(parts[2])?,
                author_name: text(parts[3])?,
                author_date: text(parts[4])?,
                parent_hashes: text(parts[5])?
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect(),
            })
        })
        .collect()
}
