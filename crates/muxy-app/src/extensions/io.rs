use std::sync::OnceLock;

use muxy_core::worker::WorkerPool;

static WORKERS: OnceLock<Result<WorkerPool, String>> = OnceLock::new();

pub(crate) fn run<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
    let (reply, result) = async_channel::bounded(1);
    let failed = reply.clone();
    let queued = WORKERS
        .get_or_init(|| WorkerPool::new("extension-io", 4, 64).map_err(|error| error.to_string()))
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
