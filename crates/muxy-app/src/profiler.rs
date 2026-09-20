mod capture;
mod element;

pub(crate) use element::workspace;
#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

static PROFILE: OnceLock<Profile> = OnceLock::new();

macro_rules! metrics {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy)]
        pub(crate) enum Metric { $($variant),+ }
        const METRICS: &[(Metric, &str)] = &[$((Metric::$variant, $name)),+];
    };
}

metrics! {
    BootLoad => "boot.load",
    WorkspaceRender => "workspace.render",
    TerminalRender => "terminal.render",
    TerminalPrepaint => "terminal.prepaint",
    TerminalPrepare => "terminal.prepare",
    TerminalPaint => "terminal.paint",
    PaintQuads => "terminal.paint_quads",
    PaintDots => "terminal.paint_dots",
    WorkspaceLayout => "workspace.request_layout",
    WorkspacePrepaint => "workspace.prepaint",
    WorkspacePaint => "workspace.paint",
    WorkspaceSyncWebviews => "workspace.sync_webviews",
    WorkspacePrepare => "workspace.prepare",
    PaintPaths => "terminal.paint_paths",
    PaintLayers => "terminal.paint_layers",
    NativeScrollSync => "terminal.native_scroll_sync",
    FrameApply => "terminal.frame_apply",
    FrameRows => "terminal.received_rows",
    RowCacheHit => "terminal.row_cache_hit",
    RowCacheMiss => "terminal.row_cache_miss",
    CursorBlink => "terminal.cursor_blink",
}

struct Profile {
    active: AtomicBool,
    bypass_row_cache: bool,
    stats: [Stat; METRICS.len()],
}

#[derive(Default)]
struct Stat {
    calls: AtomicU64,
    total_ns: AtomicU64,
    max_ns: AtomicU64,
}

impl Stat {
    fn record(&self, duration: Duration) {
        let ns = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.total_ns.fetch_add(ns, Ordering::Relaxed);
        self.max_ns.fetch_max(ns, Ordering::Relaxed);
    }

    fn take(&self) -> Value {
        json!({
            "calls": self.calls.swap(0, Ordering::Relaxed),
            "total_ms": Duration::from_nanos(self.total_ns.swap(0, Ordering::Relaxed)).as_secs_f64() * 1000.0,
            "max_ms": Duration::from_nanos(self.max_ns.swap(0, Ordering::Relaxed)).as_secs_f64() * 1000.0,
        })
    }
}

impl Profile {
    fn new(bypass_row_cache: bool) -> Self {
        Self {
            active: AtomicBool::new(true),
            bypass_row_cache,
            stats: std::array::from_fn(|_| Stat::default()),
        }
    }

    fn take(&self) -> Value {
        METRICS
            .iter()
            .map(|&(metric, name)| (name.to_owned(), self.stats[metric as usize].take()))
            .collect::<serde_json::Map<_, _>>()
            .into()
    }
}

pub(crate) struct Session {
    stop: Sender<()>,
    done: Receiver<()>,
}

impl Session {
    pub(crate) fn finish_on_quit(self, cx: &gpui::App) {
        let mut session = Some(self);
        cx.on_app_quit(move |_| {
            drop(session.take());
            async {}
        })
        .detach();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        let _ = self.done.recv_timeout(Duration::from_millis(250));
    }
}

fn output_parent(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    let value = value?;
    if value.is_empty() || value == "0" {
        None
    } else if value == "1" {
        Some(std::env::temp_dir())
    } else {
        Some(value.into())
    }
}

pub(crate) fn init() -> Option<Session> {
    let parent = output_parent(std::env::var_os("MUXY_PROFILE"))?;
    match start(&parent) {
        Ok(session) => Some(session),
        Err(error) => {
            let _ = writeln!(io::stderr(), "Muxy profiler could not start: {error}");
            None
        }
    }
}

fn start(parent: &Path) -> io::Result<Session> {
    let bypass = std::env::var_os("MUXY_PROFILE_ROW_CACHE").is_some_and(|value| value == "0");
    let mode = capture::StackMode::parse(std::env::var_os("MUXY_PROFILE_STACKS").as_deref())?;
    let capture = capture::Capture::new(parent, bypass, mode)?;
    PROFILE
        .set(Profile::new(bypass))
        .map_err(|_| io::Error::other("profiler already initialized"))?;
    let (stop, receiver) = std::sync::mpsc::channel();
    let (finished, done) = std::sync::mpsc::channel();
    let directory = capture.directory.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("muxy-profiler".into())
        .spawn(move || {
            if let Some(profile) = PROFILE.get() {
                collect(capture, profile, &receiver, &finished);
            }
        })
    {
        if let Some(profile) = PROFILE.get() {
            profile.active.store(false, Ordering::Relaxed);
        }
        return Err(error);
    }
    let _ = writeln!(io::stderr(), "Muxy profiler: {}", directory.display());
    Ok(Session { stop, done })
}

fn collect(
    mut capture: capture::Capture,
    profile: &Profile,
    stop: &Receiver<()>,
    finished: &Sender<()>,
) {
    if let Err(error) = capture.run(profile, stop) {
        let _ = writeln!(io::stderr(), "Muxy profiler stopped: {error}");
    }
    profile.active.store(false, Ordering::Relaxed);
    drop(capture);
    let _ = finished.send(());
}

pub(crate) fn bypass_row_cache() -> bool {
    PROFILE
        .get()
        .is_some_and(|profile| profile.active.load(Ordering::Relaxed) && profile.bypass_row_cache)
}

pub(crate) fn count(metric: Metric, amount: usize) {
    if let Some(profile) = PROFILE.get().filter(|p| p.active.load(Ordering::Relaxed)) {
        profile.stats[metric as usize]
            .calls
            .fetch_add(u64::try_from(amount).unwrap_or(u64::MAX), Ordering::Relaxed);
    }
}

pub(crate) fn span(metric: Metric) -> Span<'static> {
    Span(
        PROFILE
            .get()
            .filter(|p| p.active.load(Ordering::Relaxed))
            .map(|profile| (&profile.stats[metric as usize], Instant::now())),
    )
}

pub(crate) struct Span<'a>(Option<(&'a Stat, Instant)>);

impl Drop for Span<'_> {
    fn drop(&mut self) {
        if let Some((stat, started)) = self.0 {
            stat.record(started.elapsed());
        }
    }
}
