use super::{Arc, Duration, JoinHandle, Registry, Sender, io, mpsc, thread};

const INTERVAL: Duration = Duration::from_secs(60 * 60);

pub(super) struct Retention {
    stop: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl Retention {
    pub(super) fn start(registry: Arc<Registry>) -> io::Result<Self> {
        let (stop, stopped) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("session-retention".into())
            .spawn(move || {
                while matches!(
                    stopped.recv_timeout(INTERVAL),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ) {
                    if let Err(error) = registry.prune_expired_sessions() {
                        log::error!("session retention cleanup failed: {error}");
                    }
                }
            })?;
        Ok(Self {
            stop: Some(stop),
            worker: Some(worker),
        })
    }
}

impl Drop for Retention {
    fn drop(&mut self) {
        self.stop.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            log::error!("session retention thread panicked");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_worker_stops_without_waiting_for_the_hourly_interval() -> io::Result<()> {
        let (events, _) = mpsc::channel();
        let registry = Arc::new(Registry::new(
            muxy_server::ServerSettings::default(),
            events,
        ));
        let reference = Arc::downgrade(&registry);
        let worker = Retention::start(registry)?;
        drop(worker);
        assert!(reference.upgrade().is_none());
        Ok(())
    }
}
