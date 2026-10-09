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
        .spawn(move || Live(worker).run())?;
    state.live += 1;
    state.started += 1;
    Ok(())
}

/// One running thread; counted in `State::live` until it exits, even by panic.
struct Live(Arc<Shared>);

impl Live {
    fn run(self) {
        let shared = &self.0;
        let mut state = shared.lock();
        loop {
            if let Some(job) = state.queue.pop_front() {
                drop(state);
                job();
                state = shared.lock();
                continue;
            }
            if state.closed {
                return;
            }
            state.idle += 1;
            let (next, wait) = shared
                .ready
                .wait_timeout(state, shared.idle_timeout)
                .unwrap_or_else(PoisonError::into_inner);
            state = next;
            state.idle -= 1;
            if wait.timed_out() && state.queue.is_empty() {
                return;
            }
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.live -= 1;
        if thread::panicking() && !state.queue.is_empty() {
            let _ = start(&self.0, &mut state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Barrier, mpsc};
    use std::time::Instant;

    const SHORT_IDLE: Duration = Duration::from_millis(50);
    const WAIT: Duration = Duration::from_secs(5);

    fn live(pool: &WorkerPool) -> usize {
        pool.handle.shared.lock().live
    }

    fn started(pool: &WorkerPool) -> usize {
        pool.handle.shared.lock().started
    }

    fn wait_until(condition: impl Fn() -> bool) {
        let deadline = Instant::now() + WAIT;
        while !condition() {
            assert!(Instant::now() < deadline, "condition not reached");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn threads_start_with_work_and_stop_when_idle() -> io::Result<()> {
        let pool = WorkerPool::with_idle_timeout("idle-test", 4, 8, SHORT_IDLE)?;
        assert_eq!(live(&pool), 0);
        let (done, finished) = mpsc::channel();
        pool.try_spawn(move || done.send(()).unwrap())?;
        finished.recv_timeout(WAIT).unwrap();
        assert_eq!(started(&pool), 1);
        wait_until(|| live(&pool) == 0);
        Ok(())
    }

    #[test]
    fn one_thread_runs_jobs_in_order_across_restarts() -> io::Result<()> {
        let pool = WorkerPool::with_idle_timeout("order-test", 1, 64, SHORT_IDLE)?;
        let order = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicUsize::new(0));
        let overlap = Arc::new(AtomicUsize::new(0));
        let push = |value| {
            let (order, running, overlap) = (order.clone(), running.clone(), overlap.clone());
            pool.try_spawn(move || {
                if running.fetch_add(1, Ordering::SeqCst) > 0 {
                    overlap.fetch_add(1, Ordering::SeqCst);
                }
                thread::sleep(Duration::from_millis(1));
                order.lock().unwrap().push(value);
                running.fetch_sub(1, Ordering::SeqCst);
            })
        };
        for value in 0..20 {
            push(value)?;
        }
        wait_until(|| order.lock().unwrap().len() == 20);
        wait_until(|| live(&pool) == 0);
        for value in 20..40 {
            push(value)?;
        }
        wait_until(|| order.lock().unwrap().len() == 40);
        assert_eq!(*order.lock().unwrap(), (0..40).collect::<Vec<_>>());
        assert_eq!(overlap.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[test]
    fn jobs_run_in_parallel_up_to_the_thread_limit() -> io::Result<()> {
        let pool = WorkerPool::with_idle_timeout("parallel-test", 3, 8, SHORT_IDLE)?;
        let barrier = Arc::new(Barrier::new(3));
        let (done, finished) = mpsc::channel();
        for _ in 0..3 {
            let (barrier, done) = (barrier.clone(), done.clone());
            pool.try_spawn(move || {
                barrier.wait();
                done.send(()).unwrap();
            })?;
        }
        for _ in 0..3 {
            finished.recv_timeout(WAIT).unwrap();
        }
        assert_eq!(started(&pool), 3);
        Ok(())
    }

    #[test]
    fn full_queue_is_rejected() -> io::Result<()> {
        let pool = WorkerPool::with_idle_timeout("full-test", 1, 1, SHORT_IDLE)?;
        let (started, running) = mpsc::channel();
        let (release, released) = mpsc::channel::<()>();
        pool.try_spawn(move || {
            started.send(()).unwrap();
            released.recv().unwrap();
        })?;
        running.recv_timeout(WAIT).unwrap();
        pool.try_spawn(|| {})?;
        let error = pool.try_spawn(|| {}).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        release.send(()).unwrap();
        Ok(())
    }

    #[test]
    fn dropping_the_pool_still_runs_queued_jobs() -> io::Result<()> {
        let pool = WorkerPool::with_idle_timeout("drop-test", 1, 16, Duration::from_secs(60))?;
        let (done, finished) = mpsc::channel();
        for value in 0..10 {
            let done = done.clone();
            pool.try_spawn(move || done.send(value).unwrap())?;
        }
        let shared = Arc::clone(&pool.handle.shared);
        drop(pool);
        let values: Vec<_> = (0..10)
            .map(|_| finished.recv_timeout(WAIT).unwrap())
            .collect();
        assert_eq!(values, (0..10).collect::<Vec<_>>());
        wait_until(|| shared.lock().live == 0);
        Ok(())
    }

    #[test]
    fn a_panicking_job_does_not_strand_queued_work() -> io::Result<()> {
        let pool = WorkerPool::with_idle_timeout("panic-test", 1, 16, SHORT_IDLE)?;
        let (done, finished) = mpsc::channel();
        pool.try_spawn(|| panic!("job failed"))?;
        pool.try_spawn(move || done.send(()).unwrap())?;
        finished.recv_timeout(WAIT).unwrap();
        Ok(())
    }
}
