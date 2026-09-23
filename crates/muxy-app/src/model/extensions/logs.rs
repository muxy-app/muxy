//! Extension log lines: a short in-memory tail per extension for Settings, and
//! main's `<resource root>/logs/output.log`, trimmed like main once it passes 5 MB.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{SyncSender, sync_channel};

const TAIL: usize = 200;
const MAX_FILE: u64 = 5 * 1024 * 1024;
const TRIMMED_FILE: u64 = 1_250_000;

pub(crate) struct Logs {
    lines: HashMap<String, VecDeque<String>>,
    writer: Option<SyncSender<(PathBuf, String)>>,
}

impl Logs {
    pub(super) fn new() -> Self {
        let (writer, lines) = sync_channel::<(PathBuf, String)>(1024);
        let started = std::thread::Builder::new()
            .name("extension-logs".into())
            .spawn(move || {
                for (path, line) in lines {
                    let _ = append(&path, &line);
                }
            });
        Self {
            lines: HashMap::new(),
            writer: started.ok().map(|_| writer),
        }
    }

    /// The most recent lines for one extension, oldest first.
    pub(crate) fn tail(&self, owner: &str) -> impl Iterator<Item = &str> {
        self.lines
            .get(owner)
            .into_iter()
            .flatten()
            .map(String::as_str)
    }

    /// Records a line; `root` is the extension's resource root when it is loaded.
    pub(super) fn append(&mut self, owner: &str, root: Option<&Path>, line: String) {
        let tail = self.lines.entry(owner.into()).or_default();
        if tail.len() == TAIL {
            tail.pop_front();
        }
        tail.push_back(line.clone());
        if let (Some(root), Some(writer)) = (root, &self.writer) {
            let _ = writer.try_send((root.join("logs").join("output.log"), line));
        }
    }
}

fn append(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")?;
    if file.metadata()?.len() > MAX_FILE {
        trim(path)?;
    }
    Ok(())
}

/// Keeps roughly the newest 1.25 MB, starting at a line boundary.
fn trim(path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let length = file.metadata()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(TRIMMED_FILE)))?;
    let mut kept = Vec::new();
    file.read_to_end(&mut kept)?;
    let start = kept
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(kept.len(), |index| index + 1);
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, &kept[start..])?;
    std::fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_logs_keep_whole_recent_lines() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("logs/output.log");
        let line = "x".repeat(99);
        for _ in 0..53_000 {
            append(&path, &line).expect("append");
        }
        let text = std::fs::read_to_string(&path).expect("log");
        assert!((text.len() as u64) < MAX_FILE);
        assert!(text.lines().all(|kept| kept == line));
    }
}
