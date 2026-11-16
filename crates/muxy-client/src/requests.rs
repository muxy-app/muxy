use crate::async_request::Response;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use muxy_protocol::{ChannelId, ReplyBody, RequestId, SessionId};

use crate::ClientError;

#[allow(
    clippy::large_enum_variant,
    reason = "A reply moves once, unboxed, as replies always have"
)]
pub(crate) enum Reply {
    Body(ReplyBody),
    Attached(ChannelId),
}

#[derive(Clone, Copy)]
pub(crate) struct Attaching {
    pub(crate) tag: u64,
    pub(crate) session: SessionId,
}

enum PendingRequest {
    Waiting(Sender<Reply>),
    Attaching {
        reply: Option<Sender<Reply>>,
        attaching: Attaching,
    },
    Asynchronous {
        response: Arc<Response>,
        deadline: Instant,
    },
}

#[derive(Default)]
struct State {
    next: u32,
    requests: HashMap<RequestId, PendingRequest>,
    closed: bool,
}

impl State {
    fn next_id(&mut self) -> RequestId {
        loop {
            let id = RequestId(self.next);
            self.next = self.next.wrapping_add(1);
            if !self.requests.contains_key(&id) {
                return id;
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Pending {
    state: Mutex<State>,
    deadlines: Condvar,
}

impl Pending {
    pub(crate) fn register(&self) -> Result<(RequestId, Receiver<Reply>), ClientError> {
        self.register_waiting(None)
    }

    pub(crate) fn register_attach(
        &self,
        attaching: Attaching,
    ) -> Result<(RequestId, Receiver<Reply>), ClientError> {
        self.register_waiting(Some(attaching))
    }

    fn register_waiting(
        &self,
        attaching: Option<Attaching>,
    ) -> Result<(RequestId, Receiver<Reply>), ClientError> {
        let mut state = self.lock();
        if state.closed {
            return Err(ClientError::Disconnected);
        }
        let id = state.next_id();
        let (sender, receiver) = mpsc::channel();
        let request = match attaching {
            Some(attaching) => PendingRequest::Attaching {
                reply: Some(sender),
                attaching,
            },
            None => PendingRequest::Waiting(sender),
        };
        state.requests.insert(id, request);
        Ok((id, receiver))
    }

    pub(crate) fn take_attaching(&self, id: RequestId) -> Option<Attaching> {
        let mut state = self.lock();
        match state.requests.remove(&id)? {
            PendingRequest::Attaching { reply, attaching } => {
                if let Some(reply) = reply {
                    state.requests.insert(id, PendingRequest::Waiting(reply));
                }
                Some(attaching)
            }
            request => {
                state.requests.insert(id, request);
                None
            }
        }
    }

    pub(crate) fn resolve(&self, id: RequestId, body: ReplyBody) {
        let request = self.lock().requests.remove(&id);
        match request {
            Some(PendingRequest::Asynchronous { response, .. }) => {
                response.resolve(match body {
                    ReplyBody::Error(error) => Err(ClientError::Server(error)),
                    body => Ok(body),
                });
            }
            Some(
                PendingRequest::Waiting(sender)
                | PendingRequest::Attaching {
                    reply: Some(sender),
                    ..
                },
            ) => {
                let _ = sender.send(Reply::Body(body));
            }
            _ => {}
        }
    }

    pub(crate) fn resolve_attached(&self, id: RequestId, channel: ChannelId) {
        let mut state = self.lock();
        let sender = match state.requests.remove(&id) {
            Some(PendingRequest::Waiting(sender)) => Some(sender),
            Some(PendingRequest::Attaching { reply, attaching }) => {
                state.requests.insert(
                    id,
                    PendingRequest::Attaching {
                        reply: None,
                        attaching,
                    },
                );
                reply
            }
            Some(request) => {
                state.requests.insert(id, request);
                None
            }
            None => None,
        };
        if let Some(sender) = sender {
            let _ = sender.send(Reply::Attached(channel));
        }
    }

    pub(crate) fn give_up_attach(&self, id: RequestId) -> bool {
        let mut state = self.lock();
        if let Some(PendingRequest::Attaching { reply, .. }) = state.requests.get_mut(&id) {
            reply.take();
            true
        } else {
            false
        }
    }

    pub(crate) fn forget(&self, id: RequestId) {
        let mut state = self.lock();
        if let Some(PendingRequest::Attaching { reply, .. }) = state.requests.get_mut(&id) {
            reply.take();
        } else {
            state.requests.remove(&id);
        }
    }

    pub(crate) fn abandon(&self, id: RequestId) {
        self.lock().requests.remove(&id);
    }

    pub(crate) fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        self.deadlines.notify_one();
        let responses: Vec<_> = state
            .requests
            .drain()
            .filter_map(|(_, request)| match request {
                PendingRequest::Asynchronous { response, .. } => Some(response),
                _ => None,
            })
            .collect();
        drop(state);
        for response in responses {
            response.resolve(Err(ClientError::Disconnected));
        }
    }

    pub(crate) fn register_async(
        &self,
        timeout: Duration,
    ) -> Result<(RequestId, Arc<Response>), ClientError> {
        let mut state = self.lock();
        if state.closed {
            return Err(ClientError::Disconnected);
        }
        if state
            .requests
            .values()
            .filter(|request| matches!(request, PendingRequest::Asynchronous { .. }))
            .count()
            >= 128
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "too many pending asynchronous requests",
            )
            .into());
        }
        let id = state.next_id();
        let response = Arc::new(Response::default());
        state.requests.insert(
            id,
            PendingRequest::Asynchronous {
                response: response.clone(),
                deadline: Instant::now() + timeout,
            },
        );
        self.deadlines.notify_one();
        Ok((id, response))
    }

    pub(crate) fn contains_async(&self, id: RequestId) -> bool {
        matches!(
            self.lock().requests.get(&id),
            Some(PendingRequest::Asynchronous { .. })
        )
    }

    pub(crate) fn fail_async(&self, id: RequestId, error: ClientError) {
        let mut state = self.lock();
        let response = match state.requests.remove(&id) {
            Some(PendingRequest::Asynchronous { response, .. }) => Some(response),
            Some(request) => {
                state.requests.insert(id, request);
                None
            }
            None => None,
        };
        drop(state);
        if let Some(response) = response {
            response.resolve(Err(error));
        }
    }

    pub(crate) fn watch_deadlines(&self) {
        loop {
            let mut state = self.lock();
            while !state.closed {
                let next = state
                    .requests
                    .values()
                    .filter_map(|request| match request {
                        PendingRequest::Asynchronous { deadline, .. } => Some(*deadline),
                        _ => None,
                    })
                    .min();
                if next.is_some_and(|deadline| deadline <= Instant::now()) {
                    break;
                }
                state = if let Some(deadline) = next {
                    self.deadlines
                        .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                } else {
                    self.deadlines
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner)
                };
            }
            if state.closed {
                return;
            }
            drop(state);
            self.expire();
        }
    }

    pub(crate) fn expire(&self) {
        let now = Instant::now();
        let mut state = self.lock();
        let responses: Vec<_> = state
            .requests
            .extract_if(|_, request| {
                matches!(request, PendingRequest::Asynchronous { deadline, .. } if *deadline <= now)
            })
            .filter_map(|(_, request)| match request {
                PendingRequest::Asynchronous { response, .. } => Some(response),
                _ => None,
            })
            .collect();
        drop(state);
        for response in responses {
            response.resolve(Err(ClientError::Timeout));
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.lock().closed
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::{ErrorCode, ErrorReply};

    fn asynchronous(
        pending: &Arc<Pending>,
        timeout: Duration,
    ) -> (RequestId, crate::Request<ReplyBody>) {
        let (id, response) = pending.register_async(timeout).expect("register");
        (
            id,
            crate::Request::new(response, Arc::downgrade(pending), id, Ok),
        )
    }

    fn attaching() -> Attaching {
        Attaching {
            tag: 1,
            session: SessionId::new(7).expect("session"),
        }
    }

    #[test]
    fn an_attach_given_up_before_its_reply_leaves_the_reply_to_the_event_stream() {
        let pending = Pending::default();
        let (id, reply) = pending.register_attach(attaching()).expect("register");
        assert!(pending.give_up_attach(id));
        assert!(pending.take_attaching(id).is_some());
        pending.resolve_attached(id, ChannelId(3));
        assert!(reply.try_recv().is_err());
    }

    #[test]
    fn an_attach_whose_reply_is_being_announced_still_gets_its_channel() {
        let pending = Pending::default();
        let (id, reply) = pending.register_attach(attaching()).expect("register");
        assert!(pending.take_attaching(id).is_some());
        assert!(!pending.give_up_attach(id));
        pending.resolve_attached(id, ChannelId(3));
        assert!(matches!(
            reply.try_recv(),
            Ok(Reply::Attached(ChannelId(3)))
        ));
    }

    #[test]
    fn wrapped_ids_skip_waiters_late_attachments_and_async_requests() {
        let pending = Arc::new(Pending::default());
        pending.lock().next = u32::MAX;
        let (waiting, reply) = pending.register().expect("register");
        let (late, attachment) = pending.register_attach(attaching()).expect("attach");
        pending.forget(late);
        assert!(matches!(
            attachment.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        let (asynchronous, request) = asynchronous(&pending, Duration::from_secs(5));
        pending.lock().next = u32::MAX;
        let (next, _) = pending.register().expect("register after wrap");
        assert!(![waiting, late, asynchronous].contains(&next));
        pending.resolve(waiting, ReplyBody::Pong);
        assert!(matches!(reply.try_recv(), Ok(Reply::Body(ReplyBody::Pong))));
        let tag = pending.take_attaching(late).expect("late attachment tag");
        assert_eq!(tag.tag, attaching().tag);
        assert_eq!(tag.session, attaching().session);
        pending.lock().next = late.0;
        let (reused, _) = pending.register().expect("released ID");
        assert_eq!(reused, late);
        assert!(pending.contains_async(asynchronous));
        drop(request);
    }

    #[test]
    fn abandoning_an_attach_releases_both_its_waiter_and_late_reply_tag() {
        let pending = Pending::default();
        let (id, reply) = pending.register_attach(attaching()).expect("attach");
        pending.abandon(id);
        assert!(matches!(
            reply.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        assert!(!pending.give_up_attach(id));
        assert!(pending.take_attaching(id).is_none());
        pending.resolve_attached(id, ChannelId(3));
        pending.lock().next = id.0;
        assert_eq!(pending.register().expect("released ID").0, id);
    }

    #[test]
    fn error_replies_clear_attachment_tags_and_complete_async_requests_once() {
        let pending = Arc::new(Pending::default());
        let error = ErrorReply {
            code: ErrorCode::Unauthorized,
            message: "permission denied".into(),
        };
        let (id, reply) = pending.register_attach(attaching()).expect("attach");
        pending.resolve(id, ReplyBody::Error(error.clone()));
        assert!(matches!(
            reply.try_recv(),
            Ok(Reply::Body(ReplyBody::Error(found))) if found == error
        ));
        assert!(pending.take_attaching(id).is_none());
        let (id, request) = asynchronous(&pending, Duration::ZERO);
        let (done, received) = mpsc::channel();
        let checked = pending.clone();
        request.on_complete(move |result| {
            assert!(!checked.contains_async(id));
            done.send(result).expect("completion");
        });
        pending.resolve(id, ReplyBody::Error(error.clone()));
        pending.expire();
        pending.resolve(id, ReplyBody::Pong);
        assert!(matches!(
            received.recv_timeout(Duration::from_secs(1)),
            Ok(Err(ClientError::Server(found))) if found == error
        ));
        assert!(matches!(
            received.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }

    #[test]
    fn failed_async_requests_release_capacity_without_completing_other_requests() {
        let pending = Arc::new(Pending::default());
        let (waiting, reply) = pending.register().expect("register");
        pending.fail_async(waiting, ClientError::Timeout);
        pending.resolve(waiting, ReplyBody::Pong);
        assert!(matches!(reply.try_recv(), Ok(Reply::Body(ReplyBody::Pong))));
        let (id, request) = asynchronous(&pending, Duration::from_secs(5));
        let (done, received) = mpsc::channel();
        let checked = pending.clone();
        request.on_complete(move |result| {
            assert!(!checked.contains_async(id));
            done.send(result).expect("completion");
        });
        pending.fail_async(id, ClientError::Protocol("send failed".into()));
        pending.resolve(id, ReplyBody::Pong);
        assert!(matches!(
            received.recv_timeout(Duration::from_secs(1)),
            Ok(Err(ClientError::Protocol(message))) if message == "send failed"
        ));
    }

    #[test]
    fn deadline_watcher_expires_new_requests_and_close_releases_all_waiters() {
        let pending = Arc::new(Pending::default());
        let worker_pending = pending.clone();
        let watcher = std::thread::spawn(move || worker_pending.watch_deadlines());
        let (waiting, reply) = pending.register().expect("register");
        let (attaching, attachment) = pending.register_attach(attaching()).expect("attach");
        let (later, request) = asynchronous(&pending, Duration::from_secs(30));
        let (done, received) = mpsc::channel();
        request.on_complete(move |result| done.send(result).expect("completion"));
        let (expired, request) = asynchronous(&pending, Duration::ZERO);
        let (done, received_expired) = mpsc::channel();
        let checked = pending.clone();
        request.on_complete(move |result| {
            assert!(!checked.contains_async(expired));
            done.send(result).expect("completion");
        });
        let result = received_expired.recv_timeout(Duration::from_secs(5));
        pending.close();
        watcher.join().expect("watcher exits after close");
        assert!(matches!(result, Ok(Err(ClientError::Timeout))));
        assert!(matches!(
            received.try_recv(),
            Ok(Err(ClientError::Disconnected))
        ));
        for reply in [reply, attachment] {
            assert!(matches!(
                reply.try_recv(),
                Err(mpsc::TryRecvError::Disconnected)
            ));
        }
        assert!(!pending.contains_async(later));
        assert!(pending.take_attaching(attaching).is_none());
        pending.resolve(waiting, ReplyBody::Pong);
        assert!(matches!(pending.register(), Err(ClientError::Disconnected)));
        assert!(matches!(
            pending.register_async(Duration::ZERO),
            Err(ClientError::Disconnected)
        ));
    }
}
