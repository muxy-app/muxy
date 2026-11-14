use std::sync::OnceLock;

use muxy_core::worker::WorkerPool;

static WORKERS: OnceLock<Result<WorkerPool, String>> = OnceLock::new();
static NETWORK: OnceLock<Result<WorkerPool, String>> = OnceLock::new();
static LOOKUPS: OnceLock<Result<WorkerPool, String>> = OnceLock::new();
static PICTURES: OnceLock<Result<WorkerPool, String>> = OnceLock::new();

fn queue<T: Send + 'static>(
    pool: &'static OnceLock<Result<WorkerPool, String>>,
    name: &'static str,
    threads: usize,
    capacity: usize,
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
    let (reply, result) = async_channel::bounded(1);
    let failed = reply.clone();
    let queued = pool
        .get_or_init(|| WorkerPool::new(name, threads, capacity).map_err(|error| error.to_string()))
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|workers| {
            workers
                .try_spawn(move || {
                    let _ = reply.try_send(operation());
                })
                .map_err(|error| error.to_string())
        });
    if let Err(error) = queued {
        let _ = failed.try_send(Err(error));
    }
    async move {
        result
            .recv()
            .await
            .unwrap_or_else(|_| Err("extension worker stopped".into()))
    }
}

/// Short local file work for extensions.
pub(crate) fn run<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
    queue(&WORKERS, "extension-io", 4, 64, operation)
}

/// Network requests, kept apart so slow hosts cannot delay local work.
pub(crate) fn network<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
    queue(&NETWORK, "extension-http", 8, 64, operation)
}

/// Host name checks before consent, so slow requests cannot delay prompts.
pub(crate) fn lookup<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
    queue(&LOOKUPS, "extension-dns", 2, 64, operation)
}

/// Listing icons and screenshots, kept apart so a page of them cannot delay
/// installs or marketplace searches.
pub(crate) fn pictures<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
    queue(&PICTURES, "extension-pictures", 4, 256, operation)
}
