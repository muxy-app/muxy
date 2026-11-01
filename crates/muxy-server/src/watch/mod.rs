//! Folder watches that report changes once they settle, shared by every
//! connection that watches the same project.

mod shared;

pub(crate) use shared::{Shared, Subscribers, Subscription};

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use muxy_protocol::ErrorCode;
use notify::{RecursiveMode, Watcher};

use crate::ServerError;

/// The longest a notice waits while changes keep arriving.
pub(crate) const MAX_DELAY: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Timing {
    /// How long changes must stop before a notice goes out.
    pub(crate) quiet: Duration,
    pub(crate) max_delay: Duration,
}

/// Gathers events into a `T` and hands it to `flush` once events stop for the
/// quiet period, or once the maximum delay passes while they keep arriving.
pub(crate) struct DebouncedWatch {
    _watcher: notify::RecommendedWatcher,
    stop: Arc<AtomicBool>,
    wake: SyncSender<()>,
    task: Option<JoinHandle<()>>,
}

impl DebouncedWatch {
    /// `record` adds an event to the pending changes and returns whether any
    /// are worth reporting.
    pub(crate) fn new<T: Default + Send + 'static>(
        name: &str,
        timing: Timing,
        config: notify::Config,
        roots: &[&Path],
        mut record: impl FnMut(&mut T, notify::Result<notify::Event>) -> bool + Send + 'static,
        mut flush: impl FnMut(T) + Send + 'static,
    ) -> Result<Self, ServerError> {
        let (wake, wakes) = mpsc::sync_channel(1);
        let pending = Arc::new(Mutex::new(T::default()));
        let notify = wake.clone();
        let recorded = Arc::clone(&pending);
        let mut watcher = notify::RecommendedWatcher::new(
            move |event| {
                let mut pending = recorded.lock().unwrap_or_else(PoisonError::into_inner);
                if record(&mut pending, event) {
                    let _ = notify.try_send(());
                }
            },
            config,
        )
        .map_err(failed)?;
        for root in roots {
            watcher
                .watch(root, RecursiveMode::Recursive)
                .map_err(failed)?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let task = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                while wakes.recv().is_ok() {
                    if !settle(&wakes, timing, &stopped) {
                        return;
                    }
                    let changes = std::mem::take(
                        &mut *pending.lock().unwrap_or_else(PoisonError::into_inner),
                    );
                    flush(changes);
                }
            })
            .map_err(failed)?;
        Ok(Self {
            _watcher: watcher,
            stop,
            wake,
            task: Some(task),
        })
    }
}

impl Drop for DebouncedWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}

/// Waits out one burst of wakes. Returns false once the watch stops.
fn settle(wakes: &Receiver<()>, timing: Timing, stopped: &AtomicBool) -> bool {
    let started = Instant::now();
    loop {
        if stopped.load(Ordering::Acquire) {
            return false;
        }
        let wait = timing
            .quiet
            .min(timing.max_delay.saturating_sub(started.elapsed()));
        if wait.is_zero() {
            return true;
        }
        match wakes.recv_timeout(wait) {
            Ok(()) => (),
            Err(RecvTimeoutError::Timeout) => return !stopped.load(Ordering::Acquire),
            Err(RecvTimeoutError::Disconnected) => return false,
        }
    }
}

fn failed(error: impl std::fmt::Display) -> ServerError {
    ServerError::new(ErrorCode::BadRequest, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_changes_settle_within_the_maximum_delay() {
        let (wake, wakes) = mpsc::sync_channel(1);
        let done = Arc::new(AtomicBool::new(false));
        let writing = Arc::clone(&done);
        let writer = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(5);
            while Instant::now() < until && !writing.load(Ordering::Acquire) {
                let _ = wake.try_send(());
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let started = Instant::now();
        let settled = settle(
            &wakes,
            Timing {
                quiet: Duration::from_millis(500),
                max_delay: Duration::from_millis(300),
            },
            &AtomicBool::new(false),
        );
        let waited = started.elapsed();
        done.store(true, Ordering::Release);
        writer.join().expect("writer");
        assert!(settled);
        assert!(waited < Duration::from_secs(3), "waited {waited:?}");
    }
}
