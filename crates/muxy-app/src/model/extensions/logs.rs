//! Extension log lines: a short in-memory tail per extension for Settings, and
//! main's `<resource root>/logs/output.log`, trimmed like main once it passes 5 MB.
//! The tail starts with the file's newest lines, so earlier sessions show too.
//! The same writer appends the profile's audit log.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{SyncSender, sync_channel};

use muxy_app_core::extensions::{AuditEntry, AuditLog};
use muxy_ui::tr;

const TAIL: usize = 200;
const MAX_FILE: u64 = 5 * 1024 * 1024;
const TRIMMED_FILE: u64 = 1_250_000;
/// How much of the file's end is read for its newest lines.
const TAIL_WINDOW: u64 = 256 * 1024;

/// File work, done in order on one thread so a read sees every earlier write.
enum Job {
    Append(PathBuf, String),
    Tail(PathBuf, async_channel::Sender<Vec<String>>),
    Audit(AuditLog, AuditEntry),
    Flush(async_channel::Sender<()>),
}

pub(crate) struct Logs {
    lines: HashMap<String, VecDeque<String>>,
    /// Extensions whose earlier lines were already read from their file.
    read: HashSet<String>,
    writer: Option<SyncSender<Job>>,
}

impl Logs {
    pub(super) fn new() -> Self {
        let (writer, jobs) = sync_channel::<Job>(1024);
        let started = std::thread::Builder::new()
            .name("extension-logs".into())
            .spawn(move || {
                for job in jobs {
                    match job {
                        Job::Append(path, line) => {
                            let _ = append(&path, &line);
                        }
                        Job::Tail(path, reply) => {
                            let _ = reply.try_send(tail(&path).unwrap_or_default());
                        }
                        Job::Audit(log, entry) => {
                            let _ = log.append(&entry);
                        }
                        Job::Flush(reply) => {
                            let _ = reply.try_send(());
                        }
                    }
                }
            });
        Self {
            lines: HashMap::new(),
            read: HashSet::new(),
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
            let _ = writer.try_send(Job::Append(log_file(root), line));
        }
    }

    pub(super) fn audit(&self, log: &AuditLog, entry: AuditEntry) {
        if let Some(writer) = &self.writer {
            let _ = writer.try_send(Job::Audit(log.clone(), entry));
        }
    }

    pub(super) fn flush(&self) -> Result<async_channel::Receiver<()>, String> {
        let writer = self
            .writer
            .as_ref()
            .ok_or_else(|| tr!("Extension log writer is unavailable.").to_string())?;
        let (reply, done) = async_channel::bounded(1);
        writer
            .try_send(Job::Flush(reply))
            .map_err(|error| error.to_string())?;
        Ok(done)
    }

    /// Reads an extension's earlier lines from its file, once per session.
    /// Ask before the extension logs anything, so no line is read twice.
    pub(super) fn read_earlier(
        &mut self,
        owner: &str,
        root: &Path,
    ) -> Option<async_channel::Receiver<Vec<String>>> {
        let writer = self.writer.as_ref()?;
        if !self.read.insert(owner.into()) {
            return None;
        }
        let (reply, lines) = async_channel::bounded(1);
        writer.try_send(Job::Tail(log_file(root), reply)).ok()?;
        Some(lines)
    }

    /// Puts lines from earlier sessions before this session's.
    pub(super) fn prepend(&mut self, owner: &str, earlier: Vec<String>) {
        let tail = self.lines.entry(owner.into()).or_default();
        for line in earlier.into_iter().rev() {
            if tail.len() == TAIL {
                break;
            }
            tail.push_front(line);
        }
    }
}

pub(crate) fn log_file(root: &Path) -> PathBuf {
    root.join("logs").join("output.log")
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

/// The file's newest whole lines, oldest first.
fn tail(path: &Path) -> std::io::Result<Vec<String>> {
    let mut file = std::fs::File::open(path)?;
    let start = file.metadata()?.len().saturating_sub(TAIL_WINDOW);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let whole = if start == 0 {
        &text[..]
    } else {
        text.split_once('\n').map_or("", |(_, rest)| rest)
    };
    let lines: Vec<String> = whole.lines().map(str::to_owned).collect();
    Ok(lines[lines.len().saturating_sub(TAIL)..].to_vec())
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
    fn flushing_finishes_pending_writes_before_the_package_moves() {
        let directory = tempfile::tempdir().expect("directory");
        let package = directory.path().join("package");
        let mut logs = Logs::new();
        for index in 0..200 {
            logs.append("reader", Some(&package), format!("line {index}"));
        }
        logs.flush()
            .expect("flush queued")
            .recv_blocking()
            .expect("flushed");
        std::fs::rename(&package, directory.path().join("backup")).expect("move package");
        logs.flush()
            .expect("flush queued")
            .recv_blocking()
            .expect("flushed");
        assert!(!package.exists());
        let saved = std::fs::read_to_string(directory.path().join("backup/logs/output.log"))
            .expect("saved log");
        assert_eq!(saved.lines().count(), 200);
        assert!(saved.ends_with("line 199\n"));
    }

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

    #[test]
    fn the_tail_starts_with_earlier_sessions_and_reads_each_file_once() {
        let directory = tempfile::tempdir().expect("directory");
        let long = "y".repeat(2_000);
        for index in 0..300 {
            append(&log_file(directory.path()), &format!("{index} {long}")).expect("append");
        }
        let earlier = tail(&log_file(directory.path())).expect("tail");
        assert_eq!(earlier.len(), 130, "only whole lines inside the window");
        assert!(earlier[0].starts_with("170 "));
        assert!(earlier[129].starts_with("299 "));

        let mut logs = Logs::new();
        let lines = logs
            .read_earlier("ports", directory.path())
            .expect("first read");
        assert!(logs.read_earlier("ports", directory.path()).is_none());
        logs.append("ports", Some(directory.path()), "now".into());
        logs.prepend("ports", lines.recv_blocking().expect("lines"));
        let tail: Vec<_> = logs.tail("ports").collect();
        assert_eq!(tail.len(), 131);
        assert!(tail[0].starts_with("170 "));
        assert_eq!(tail[130], "now");
    }
}
