use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::Profile;

const INTERVAL: Duration = Duration::from_secs(1);
const SAMPLE_INTERVAL: Duration = Duration::from_secs(15);
const HIGH_CPU_PERCENT: f64 = 20.0;
const MAX_LOG_BYTES: u64 = 8 * 1024 * 1024;
const SAMPLE_SLOTS: u64 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StackMode {
    Auto,
    Always,
    Off,
}

impl StackMode {
    pub(super) fn parse(value: Option<&std::ffi::OsStr>) -> io::Result<Self> {
        match value.map(std::ffi::OsStr::to_str) {
            None | Some(Some("auto")) => Ok(Self::Auto),
            Some(Some("always")) => Ok(Self::Always),
            Some(Some("0")) => Ok(Self::Off),
            _ => Err(io::Error::other(
                "MUXY_PROFILE_STACKS must be auto, always, or 0",
            )),
        }
    }
}

pub(super) struct Capture {
    pub(super) directory: PathBuf,
    log: Log,
    started: Instant,
    previous_tick: Instant,
    previous_usage: Option<(Instant, Duration)>,
    busy_ticks: u32,
    last_sample: Option<Instant>,
    sample: Option<(Child, PathBuf)>,
    sample_number: u64,
    stack_mode: StackMode,
}

impl Capture {
    pub(super) fn new(
        parent: &Path,
        bypass_row_cache: bool,
        stack_mode: StackMode,
    ) -> io::Result<Self> {
        fs::create_dir_all(parent)?;
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let directory = parent.join(format!("muxy-profile-{}-{unix_ms}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let executable = std::env::current_exe().ok();
        let executable_metadata = executable.as_ref().and_then(|path| fs::metadata(path).ok());
        let metadata = json!({
            "schema_version": 1,
            "pid": std::process::id(),
            "started_unix_ms": unix_ms,
            "build": muxy_protocol::BuildInfo::current(),
            "executable": executable,
            "executable_bytes": executable_metadata.as_ref().map(fs::Metadata::len),
            "executable_modified_unix_ms": executable_metadata.and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok()).map(|elapsed| elapsed.as_millis()),
            "debug_assertions": cfg!(debug_assertions),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "row_cache_enabled": !bypass_row_cache,
            "interval_ms": INTERVAL.as_millis(),
            "cpu_percent_semantics": "100 percent is one fully occupied CPU core; derived from process CPU-time deltas",
            "timing_semantics": "inclusive wall time; nested scopes overlap; concurrent interval boundaries are approximate",
            "memory_semantics": "rss_kib is resident memory, not macOS physical footprint",
            "stack_sampling": {"available": cfg!(target_os = "macos"), "mode": format!("{stack_mode:?}"), "cpu_threshold_percent": HIGH_CPU_PERCENT,
                "consecutive_intervals": 2, "minimum_spacing_seconds": SAMPLE_INTERVAL.as_secs(),
                "duration_seconds": 2, "sampling_interval_ms": 10, "retained_files": SAMPLE_SLOTS},
            "retention": {"metrics_files": 2, "max_file_bytes": MAX_LOG_BYTES},
        });
        private_file(&directory.join("session.json"))?
            .write_all(serde_json::to_string_pretty(&metadata)?.as_bytes())?;
        let now = Instant::now();
        Ok(Self {
            log: Log::new(&directory)?,
            directory,
            started: now,
            previous_tick: now,
            previous_usage: None,
            busy_ticks: 0,
            last_sample: None,
            sample: None,
            sample_number: 0,
            stack_mode,
        })
    }

    pub(super) fn run(&mut self, profile: &Profile, stop: &Receiver<()>) -> io::Result<()> {
        self.previous_usage = usage().ok().map(|(cpu, _)| (Instant::now(), cpu));
        loop {
            let stopping = match stop.recv_timeout(INTERVAL) {
                Err(RecvTimeoutError::Timeout) => false,
                Ok(()) | Err(RecvTimeoutError::Disconnected) => true,
            };
            self.tick(profile, stopping)?;
            if stopping {
                return Ok(());
            }
        }
    }

    fn tick(&mut self, profile: &Profile, stopping: bool) -> io::Result<()> {
        let now = Instant::now();
        let interval = now.duration_since(self.previous_tick).as_secs_f64();
        self.previous_tick = now;
        let (cpu_percent, rss, error) = match usage() {
            Ok((cpu, rss)) => {
                let measured = Instant::now();
                let percent = self.previous_usage.and_then(|(previous, old_cpu)| {
                    cpu.checked_sub(old_cpu).map(|delta| {
                        delta.as_secs_f64() / measured.duration_since(previous).as_secs_f64()
                            * 100.0
                    })
                });
                self.previous_usage = Some((measured, cpu));
                (percent, Some(rss), None)
            }
            Err(error) => (None, None, Some(error.to_string())),
        };
        self.log.write(&json!({
            "schema_version": 1, "kind": if stopping { "shutdown" } else { "interval" },
            "elapsed_ms": self.started.elapsed().as_millis(), "interval_seconds": interval,
            "cpu_percent": cpu_percent, "rss_kib": rss, "resource_error": error,
            "stack_sample_running": self.sample.is_some(), "metrics": profile.take(),
        }))?;
        self.poll_sample()?;
        if !stopping && cfg!(target_os = "macos") && self.stack_mode != StackMode::Off {
            self.busy_ticks = if cpu_percent.is_some_and(|cpu| cpu >= HIGH_CPU_PERCENT) {
                self.busy_ticks.saturating_add(1)
            } else {
                0
            };
            if (self.stack_mode == StackMode::Always || self.busy_ticks >= 2)
                && self.sample.is_none()
                && self
                    .last_sample
                    .is_none_or(|last| now.duration_since(last) >= SAMPLE_INTERVAL)
            {
                self.start_sample(now)?;
            }
        }
        Ok(())
    }

    fn start_sample(&mut self, now: Instant) -> io::Result<()> {
        self.last_sample = Some(now);
        let path = self
            .directory
            .join(format!("stacks-{}.txt", self.sample_number % SAMPLE_SLOTS));
        self.sample_number += 1;
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        let result = Command::new("/usr/bin/sample")
            .arg(std::process::id().to_string())
            .args(["2", "10", "-file"])
            .arg(&path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let error = match result {
            Ok(child) => {
                self.sample = Some((child, path.clone()));
                None
            }
            Err(error) => Some(error.to_string()),
        };
        self.log
            .write(&json!({"schema_version": 1, "kind": "stack_sample_start",
            "elapsed_ms": self.started.elapsed().as_millis(), "sequence": self.sample_number,
            "file": path.file_name(), "error": error}))
    }

    fn poll_sample(&mut self) -> io::Result<()> {
        let Some((child, _)) = &mut self.sample else {
            return Ok(());
        };
        let Some(status) = child.try_wait()? else {
            return Ok(());
        };
        let Some((_, path)) = self.sample.take() else {
            return Ok(());
        };
        let truncated = fs::metadata(&path).is_ok_and(|metadata| metadata.len() > MAX_LOG_BYTES);
        if truncated {
            OpenOptions::new()
                .write(true)
                .open(&path)?
                .set_len(MAX_LOG_BYTES)?;
        }
        self.log
            .write(&json!({"schema_version": 1, "kind": "stack_sample_end",
            "elapsed_ms": self.started.elapsed().as_millis(), "sequence": self.sample_number,
            "file": path.file_name(), "success": status.success(), "exit_code": status.code(), "truncated": truncated}))
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some((mut child, _)) = self.sample.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

struct Log {
    directory: PathBuf,
    file: File,
    bytes: u64,
}

impl Log {
    fn new(directory: &Path) -> io::Result<Self> {
        Ok(Self {
            directory: directory.into(),
            file: private_file(&directory.join("metrics.jsonl"))?,
            bytes: 0,
        })
    }

    fn write(&mut self, value: &Value) -> io::Result<()> {
        let mut line = serde_json::to_vec(value)?;
        line.push(b'\n');
        let length = u64::try_from(line.len()).unwrap_or(u64::MAX);
        if self.bytes + length > MAX_LOG_BYTES {
            fs::rename(
                self.directory.join("metrics.jsonl"),
                self.directory.join("metrics.previous.jsonl"),
            )?;
            self.file = private_file(&self.directory.join("metrics.jsonl"))?;
            self.bytes = 0;
        }
        self.file.write_all(&line)?;
        self.bytes += length;
        Ok(())
    }
}

fn usage() -> io::Result<(Duration, u64)> {
    let output = Command::new("/bin/ps")
        .args([
            "-p",
            &std::process::id().to_string(),
            "-o",
            "time=",
            "-o",
            "rss=",
        ])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("ps could not read this process"));
    }
    parse_usage(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| io::Error::other("unrecognized ps resource output"))
}

fn parse_usage(value: &str) -> Option<(Duration, u64)> {
    let mut fields = value.split_whitespace();
    let time = fields.next()?;
    let rss = fields.next()?.parse().ok()?;
    let (days, time) = match time.split_once('-') {
        Some((days, time)) => (days.parse::<u64>().ok()?, time),
        None => (0, time),
    };
    let mut parts = time.rsplit(':');
    let seconds = parts.next()?.parse::<f64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let hours = parts
        .next()
        .map(str::parse::<u64>)
        .transpose()
        .ok()?
        .unwrap_or(0);
    if parts.next().is_some() || fields.next().is_some() {
        return None;
    }
    let whole = days
        .checked_mul(86400)?
        .checked_add(hours.checked_mul(3600)?)?
        .checked_add(minutes.checked_mul(60)?)?;
    Some((
        Duration::from_secs(whole).checked_add(Duration::try_from_secs_f64(seconds).ok()?)?,
        rss,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    #[ignore = "manual verification of macOS sampling; samples only this headless test process"]
    fn native_stack_capture_samples_this_process() {
        let directory = tempfile::tempdir().unwrap();
        let mut capture = Capture::new(directory.path(), false, StackMode::Always).unwrap();
        capture.start_sample(Instant::now()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while capture.sample.is_some() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
            capture.poll_sample().unwrap();
        }
        assert!(capture.sample.is_none());
        let sample = fs::read_to_string(capture.directory.join("stacks-0.txt")).unwrap();
        assert!(
            sample.contains(&format!("pid {}", std::process::id())),
            "{sample}"
        );
        assert!(sample.contains("Call graph:"));
    }
}
