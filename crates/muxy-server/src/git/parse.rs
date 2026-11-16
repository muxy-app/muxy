use super::{Result, error, text};
use muxy_protocol::{GitFile, ServerPath};
use std::collections::HashMap;

#[cfg(test)]
mod tests;

pub(super) type LineStats = HashMap<Vec<u8>, (Option<u64>, Option<u64>)>;

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

pub(super) fn line_stats(bytes: &[u8]) -> Result<LineStats> {
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
