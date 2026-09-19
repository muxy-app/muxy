use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::task::{Context, Poll, Waker};

use muxy_protocol::{ReplyBody, RequestId};

use crate::{ClientError, requests::Pending};

type Result = std::result::Result<ReplyBody, ClientError>;
type Completion = Box<dyn FnOnce(Result) + Send>;

#[derive(Default)]
struct State {
    result: Option<Result>,
    waker: Option<Waker>,
    completion: Option<Completion>,
}

#[derive(Default)]
pub(crate) struct Response {
    state: Mutex<State>,
}

impl Response {
    pub(crate) fn resolve(&self, result: Result) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(completion) = state.completion.take() {
            drop(state);
            completion(result);
            return;
        }
        state.result = Some(result);
        let waker = state.waker.take();
        drop(state);
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

pub struct Request<T> {
    response: Arc<Response>,
    pending: Option<(Weak<Pending>, RequestId)>,
    decode: fn(ReplyBody) -> std::result::Result<T, ClientError>,
}

impl<T> std::fmt::Debug for Request<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Request").finish_non_exhaustive()
    }
}

impl<T> Request<T> {
    pub(crate) fn new(
        response: Arc<Response>,
        pending: Weak<Pending>,
        id: RequestId,
        decode: fn(ReplyBody) -> std::result::Result<T, ClientError>,
    ) -> Self {
        Self {
            response,
            pending: Some((pending, id)),
            decode,
        }
    }

    pub(crate) fn failed(
        error: ClientError,
        decode: fn(ReplyBody) -> std::result::Result<T, ClientError>,
    ) -> Self {
        let response = Arc::new(Response::default());
        response.resolve(Err(error));
        Self {
            response,
            pending: None,
            decode,
        }
    }

    pub fn on_complete(
        mut self,
        completion: impl FnOnce(std::result::Result<T, ClientError>) + Send + 'static,
    ) where
        T: 'static,
    {
        self.pending = None;
        let decode = self.decode;
        let mut state = self
            .response
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(result) = state.result.take() {
            drop(state);
            completion(result.and_then(decode));
        } else {
            state.completion = Some(Box::new(move |result| completion(result.and_then(decode))));
        }
    }
}

impl<T> Future for Request<T> {
    type Output = std::result::Result<T, ClientError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self
            .response
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(result) = state.result.take() {
            Poll::Ready(result.and_then(self.decode))
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T> Drop for Request<T> {
    fn drop(&mut self) {
        if let Some((pending, id)) = self.pending.take()
            && let Some(pending) = pending.upgrade()
        {
            pending.forget(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::task::Wake;
    use std::time::Duration;

    struct Notify(mpsc::Sender<()>);
    impl Wake for Notify {
        fn wake(self: Arc<Self>) {
            let _ = self.0.send(());
        }
    }

    fn request(pending: &Arc<Pending>, timeout: Duration) -> (RequestId, Request<ReplyBody>) {
        let (id, response) = pending.register_async(timeout).expect("register");
        (id, Request::new(response, Arc::downgrade(pending), id, Ok))
    }

    #[test]
    fn reply_timeout_and_disconnect_wake_waiters_without_holding_pending_lock() {
        let pending = Arc::new(Pending::default());
        let (notify, wake) = mpsc::channel();
        let waker = Waker::from(Arc::new(Notify(notify)));
        let mut cx = Context::from_waker(&waker);
        let (id, mut reply) = request(&pending, Duration::from_secs(5));
        assert!(Pin::new(&mut reply).poll(&mut cx).is_pending());
        pending.resolve(id, ReplyBody::Pong);
        wake.recv_timeout(Duration::from_secs(1))
            .expect("reply wake");
        assert!(matches!(
            Pin::new(&mut reply).poll(&mut cx),
            Poll::Ready(Ok(ReplyBody::Pong))
        ));
        let (_, mut expired) = request(&pending, Duration::ZERO);
        assert!(Pin::new(&mut expired).poll(&mut cx).is_pending());
        pending.expire();
        wake.recv_timeout(Duration::from_secs(1))
            .expect("timeout wake");
        assert!(matches!(
            Pin::new(&mut expired).poll(&mut cx),
            Poll::Ready(Err(ClientError::Timeout))
        ));
        let (_, reply) = request(&pending, Duration::from_secs(5));
        let checked = pending.clone();
        let (done, received) = mpsc::channel();
        reply.on_complete(move |result| {
            assert!(checked.is_closed());
            let _ = done.send(matches!(result, Err(ClientError::Disconnected)));
        });
        pending.close();
        assert!(
            received
                .recv_timeout(Duration::from_secs(1))
                .expect("disconnect")
        );
    }

    #[test]
    fn cancellation_releases_capacity_and_late_replies_are_ignored() {
        let pending = Arc::new(Pending::default());
        let mut requests: Vec<_> = (0..128)
            .map(|_| request(&pending, Duration::from_secs(5)))
            .collect();
        assert!(pending.register_async(Duration::from_secs(5)).is_err());
        assert!(
            pending.register().is_ok(),
            "async capacity is separate from synchronous app work"
        );
        let (id, cancelled) = requests.pop().expect("request");
        drop(cancelled);
        assert!(!pending.contains_async(id));
        pending.resolve(id, ReplyBody::Pong);
        assert!(pending.register_async(Duration::from_secs(5)).is_ok());
    }
}
