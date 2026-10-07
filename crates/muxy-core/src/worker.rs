//! Bounded blocking work, kept separate from input and connection readers.

use std::io;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

type Job = Box<dyn FnOnce() + Send>;

#[derive(Clone)]
pub struct WorkerPool {
    jobs: mpsc::SyncSender<Job>,
}

impl std::fmt::Debug for WorkerPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerPool").finish_non_exhaustive()
    }
}

impl WorkerPool {
    pub fn new(name: &str, threads: usize, capacity: usize) -> io::Result<Self> {
        if threads == 0 || capacity == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "workers and capacity must be positive",
            ));
        }
        let (jobs, pending) = mpsc::sync_channel::<Job>(capacity);
        let pending = Arc::new(Mutex::new(pending));
        for index in 0..threads {
            let pending = Arc::clone(&pending);
            thread::Builder::new()
                .name(format!("{name}-{index}"))
                .spawn(move || {
                    loop {
                        let job = pending
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .recv();
                        match job {
                            Ok(job) => job(),
                            Err(_) => break,
                        }
                    }
                })?;
        }
        Ok(Self { jobs })
    }

    pub fn try_spawn(&self, job: impl FnOnce() + Send + 'static) -> io::Result<()> {
        self.jobs
            .try_send(Box::new(job))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => {
                    io::Error::new(io::ErrorKind::WouldBlock, "background work queue is full")
                }
                mpsc::TrySendError::Disconnected(_) => {
                    io::Error::new(io::ErrorKind::BrokenPipe, "background workers stopped")
                }
            })
    }
}
