//! Bounded blocking work, kept separate from input and connection readers.
//!
//! Threads start when work arrives and stop after idling, so a pool that is
//! never used costs no threads. A one-thread pool runs its jobs in order.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

type Job = Box<dyn FnOnce() + Send>;

const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct WorkerPool {
    handle: Arc<Handle>,
}

/// Closes the pool when the last clone is dropped; queued jobs still run.
struct Handle {
    shared: Arc<Shared>,
}

struct Shared {
    name: String,
    threads: usize,
    capacity: usize,
    idle_timeout: Duration,
    state: Mutex<State>,
    ready: Condvar,
}

#[derive(Default)]
struct State {
    queue: VecDeque<Job>,
    live: usize,
    idle: usize,
    started: usize,
    closed: bool,
}

impl std::fmt::Debug for WorkerPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerPool").finish_non_exhaustive()
    }
}

impl WorkerPool {
    pub fn new(name: &str, threads: usize, capacity: usize) -> io::Result<Self> {
        Self::with_idle_timeout(name, threads, capacity, IDLE_TIMEOUT)
    }

    fn with_idle_timeout(
        name: &str,
        threads: usize,
        capacity: usize,
        idle_timeout: Duration,
    ) -> io::Result<Self> {
        if threads == 0 || capacity == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "workers and capacity must be positive",
            ));
        }
        let shared = Arc::new(Shared {
            name: name.to_owned(),
            threads,
            capacity,
            idle_timeout,
            state: Mutex::default(),
            ready: Condvar::new(),
        });
        Ok(Self {
            handle: Arc::new(Handle { shared }),
        })
    }

    pub fn try_spawn(&self, job: impl FnOnce() + Send + 'static) -> io::Result<()> {
        let shared = &self.handle.shared;
        let mut state = shared.lock();
        if state.queue.len() >= shared.capacity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "background work queue is full",
            ));
        }
        state.queue.push_back(Box::new(job));
        shared.ready.notify_one();
        if state.queue.len() > state.idle
            && state.live < shared.threads
            && let Err(error) = start(shared, &mut state)
            && state.live == 0
        {
            state.queue.pop_back();
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.shared.lock().closed = true;
        self.shared.ready.notify_all();
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn start(shared: &Arc<Shared>, state: &mut State) -> io::Result<()> {
    let worker = Arc::clone(shared);
    thread::Builder::new()
        .name(format!("{}-{}", shared.name, state.started))
        .spawn(move || work(&worker))?;
    state.live += 1;
    state.started += 1;
    Ok(())
}

fn work(shared: &Arc<Shared>) {
    let mut state = shared.lock();
    loop {
        if let Some(job) = state.queue.pop_front() {
            drop(state);
            {
                let _unwind = Unwind(shared);
                job();
            }
            state = shared.lock();
            continue;
        }
        if state.closed {
            break;
        }
        state.idle += 1;
        let (next, wait) = shared
            .ready
            .wait_timeout(state, shared.idle_timeout)
            .unwrap_or_else(PoisonError::into_inner);
        state = next;
        state.idle -= 1;
        if wait.timed_out() && state.queue.is_empty() {
            break;
        }
    }
    // Under the same lock as the decision to stop, so new work starts a new thread.
    state.live -= 1;
}

/// Takes a thread whose job panicked out of the count and replaces it.
struct Unwind<'a>(&'a Arc<Shared>);

impl Drop for Unwind<'_> {
    fn drop(&mut self) {
        if !thread::panicking() {
            return;
        }
        let mut state = self.0.lock();
        state.live -= 1;
        if state.queue.is_empty() {
            return;
        }
        if start(self.0, &mut state).is_err() && state.live == 0 {
            // Nothing can run the queued jobs; dropping them releases whatever their callers wait on.
            let stranded = std::mem::take(&mut state.queue);
            drop(state);
            drop(stranded);
        }
    }
}
