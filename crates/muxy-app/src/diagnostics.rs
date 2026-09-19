use std::fmt::Arguments;
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::Instant;

static TRACE: OnceLock<Trace> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
static DROPPED: AtomicU64 = AtomicU64::new(0);

struct Trace {
    started: Instant,
    sender: SyncSender<String>,
}

pub(crate) fn init(directory: &Path) {
    if !cfg!(debug_assertions) && std::env::var_os("MUXY_DIAGNOSTICS").is_none() {
        return;
    }
    let path = directory.join(format!("app-diagnostics-{}.log", std::process::id()));
    let Ok(mut file) = std::fs::File::create(&path) else {
        return;
    };
    let (sender, receiver) = sync_channel::<String>(1024);
    let previous = path.with_extension("previous.log");
    let output = path.clone();
    let worker = std::thread::Builder::new()
        .name("muxy-diagnostics".into())
        .spawn(move || {
            let mut written = 0;
            for line in receiver {
                if written >= 8 * 1024 * 1024 {
                    if std::fs::rename(&output, &previous).is_err() {
                        break;
                    }
                    let Ok(next) = std::fs::File::create(&output) else {
                        break;
                    };
                    file = next;
                    written = 0;
                }
                if file.write_all(line.as_bytes()).is_err() {
                    break;
                }
                written += line.len();
            }
        });
    if worker.is_err() {
        return;
    }
    let _ = TRACE.set(Trace {
        started: Instant::now(),
        sender,
    });
    let _ = writeln!(std::io::stderr(), "Muxy diagnostics: {}", path.display());
    event("startup", format_args!("pid={}", std::process::id()));
}

pub(crate) fn enabled() -> bool {
    TRACE.get().is_some()
}

pub(crate) fn event(name: &str, details: Arguments<'_>) {
    let Some(trace) = TRACE.get() else {
        return;
    };
    let line = format!(
        "{:>10}ms {name} {details} dropped={}\n",
        trace.started.elapsed().as_millis(),
        DROPPED.load(Ordering::Relaxed),
    );
    if trace.sender.try_send(line).is_err() {
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

pub(crate) struct Span {
    id: u64,
    name: &'static str,
    started: Instant,
}

impl Span {
    pub(crate) fn new(name: &'static str, details: Arguments<'_>) -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        event(name, format_args!("id={id} phase=queued {details}"));
        Self {
            id,
            name,
            started: Instant::now(),
        }
    }

    pub(crate) fn stage(&self, details: Arguments<'_>) {
        event(
            self.name,
            format_args!(
                "id={} elapsed_ms={} {details}",
                self.id,
                self.started.elapsed().as_millis(),
            ),
        );
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        self.stage(format_args!("phase=finished"));
    }
}
