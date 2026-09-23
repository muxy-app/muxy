use super::{Result, command, error, path, read, safe_path};
use muxy_protocol::{GitDiff, GitDiffKind, GitDiffRequest, GitDiffRow, GitRawDiff, GitReply};
use std::ffi::OsString;
use std::path::Path;

pub(super) fn read(repository: &Path, request: &GitDiffRequest) -> Result<GitReply> {
    let mut args: Vec<OsString> = [
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    let mut no_index = false;
    if let Some(relative) = &request.path {
        let absolute = safe_path(repository, relative)?;
        let file = if request.raw {
            None
        } else {
            read::files(&read::status(repository)?)?
                .into_iter()
                .find(|file| file.path == *relative)
        };
        if !request.staged
            && file
                .as_ref()
                .is_some_and(muxy_protocol::GitFile::conflicted)
        {
            args.push("--ours".into());
        }
        if !request.staged && file.as_ref().is_some_and(muxy_protocol::GitFile::untracked) {
            let metadata = std::fs::symlink_metadata(absolute).map_err(error)?;
            if !metadata.is_file() && !metadata.file_type().is_symlink() {
                return Err(error("Diff requires a file"));
            }
            no_index = true;
            args.push("--no-index".into());
        }
    }
    if request.staged {
        args.push("--cached".into());
    }
    args.push("--".into());
    if no_index {
        args.push("/dev/null".into());
    }
    if let Some(relative) = &request.path {
        args.push(path(relative).as_os_str().to_owned());
    }
    let (bytes, truncated) = command::diff(repository, &args, no_index)?;
    let raw = bounded(&bytes, truncated, request.line_limit);
    if request.raw {
        Ok(GitReply::RawDiff(raw))
    } else {
        Ok(GitReply::Diff(parse(&raw)))
    }
}

pub(super) fn branch(repository: &Path, base: &str, line_limit: Option<u32>) -> Result<GitReply> {
    let range = format!("refs/remotes/origin/{base}...HEAD");
    let (bytes, truncated) = command::diff(
        repository,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-textconv",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            &range,
            "--",
        ],
        false,
    )?;
    Ok(GitReply::RawDiff(bounded(&bytes, truncated, line_limit)))
}

pub(super) fn bounded(bytes: &[u8], mut truncated: bool, line_limit: Option<u32>) -> GitRawDiff {
    let mut end = bytes.len();
    if let Some(limit) = line_limit {
        let mut lines = 0;
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'\n' {
                lines += 1;
                if lines == limit {
                    end = index + 1;
                    truncated |= end < bytes.len();
                    break;
                }
            }
        }
    }
    GitRawDiff {
        diff: String::from_utf8_lossy(&bytes[..end]).into_owned(),
        truncated,
    }
}

fn parse(raw: &GitRawDiff) -> GitDiff {
    let mut result = GitDiff {
        rows: Vec::new(),
        additions: 0,
        deletions: 0,
        truncated: raw.truncated,
    };
    let mut old = 0;
    let mut new = 0;
    let mut in_hunk = false;
    for line in raw.diff.lines() {
        if line.starts_with("diff --git ") {
            in_hunk = false;
        }
        if line.starts_with("@@ ") {
            let mut parts = line.split_whitespace().skip(1);
            let number = |part: Option<&str>| {
                part.and_then(|part| part.get(1..))
                    .and_then(|part| part.split(',').next())
                    .and_then(|part| part.parse().ok())
                    .unwrap_or(0)
            };
            old = number(parts.next());
            new = number(parts.next());
            in_hunk = true;
            result.rows.push(GitDiffRow {
                kind: GitDiffKind::Hunk,
                old_line_number: None,
                new_line_number: None,
                old_text: None,
                new_text: None,
                text: line.into(),
            });
            continue;
        }
        if !in_hunk {
            continue;
        }
        let Some(content) = line.get(1..) else {
            continue;
        };
        let kind = match line.as_bytes()[0] {
            b' ' => GitDiffKind::Context,
            b'+' => GitDiffKind::Addition,
            b'-' => GitDiffKind::Deletion,
            _ => continue,
        };
        let has_old = kind != GitDiffKind::Addition;
        let has_new = kind != GitDiffKind::Deletion;
        result.rows.push(GitDiffRow {
            kind,
            old_line_number: has_old.then_some(old),
            new_line_number: has_new.then_some(new),
            old_text: has_old.then(|| content.into()),
            new_text: has_new.then(|| content.into()),
            text: line.into(),
        });
        old += u64::from(has_old);
        new += u64::from(has_new);
        result.additions += u64::from(kind == GitDiffKind::Addition);
        result.deletions += u64::from(kind == GitDiffKind::Deletion);
    }
    result
}
